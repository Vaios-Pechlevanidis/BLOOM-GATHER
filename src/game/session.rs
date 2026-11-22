//! The running game session: the authoritative `bloom_core::Game`, loading
//! it from disk, autosaving it, and starting over.
//!
//! Saving happens:
//! * a couple of seconds after the last change (debounced autosave),
//! * immediately when pausing, leaving to the title screen and quitting.

use bevy::prelude::*;
use bloom_core::game::{GameEvent, PlayerPose};
use bloom_core::persistence::{LoadSource, SaveStore};
use bloom_core::Game;

use super::{ActiveRegion, AppState, Content, PlayState, Touring};
use crate::player::Player;

pub struct SessionPlugin;

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Saves::new(SaveStore::new(SaveStore::default_dir())))
            .add_event::<GameEventMsg>()
            .add_systems(
                OnEnter(AppState::MainMenu),
                ensure_session.in_set(SessionSet),
            )
            .add_systems(Update, forward_events.in_set(EventsForwarded))
            .add_systems(OnEnter(PlayState::Paused), save_now)
            .add_systems(OnExit(AppState::InGame), save_now)
            .add_systems(
                Update,
                (
                    // The workbench keeps binding while its screen is open.
                    tick_session
                        .run_if(in_state(PlayState::Playing).or(in_state(PlayState::Workshop))),
                    autosave.run_if(in_state(AppState::InGame)),
                ),
            )
            .add_systems(Last, save_on_exit);
    }
}

/// A `bloom_core` game event re-sent as a Bevy event for presentation
/// (sounds, toasts, animations).
#[derive(Event, Debug, Clone)]
pub struct GameEventMsg(pub GameEvent);

/// Commands run before this set; event consumers run after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventsForwarded;

fn forward_events(session: Option<ResMut<Session>>, mut out: EventWriter<GameEventMsg>) {
    if let Some(mut session) = session {
        for event in session.drain_events() {
            out.write(GameEventMsg(event));
        }
    }
}

/// Session loading; scene and player setup run after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionSet;

/// The authoritative game. `generation` changes when a new game starts, so
/// presentation knows to rebuild (e.g. respawn every flower).
#[derive(Resource, Deref, DerefMut)]
pub struct Session {
    #[deref]
    pub game: Game,
    pub generation: u64,
}

#[derive(Resource)]
pub struct Saves {
    pub store: SaveStore,
    /// A save slot with progress exists on disk.
    pub has_save: bool,
    /// Something the title screen should tell the player about their save.
    pub notice: Option<String>,
    /// Real time at which unsaved changes appeared.
    dirty_since: Option<f64>,
}

/// Debounce before an autosave, so a burst of picking is one write.
const AUTOSAVE_DELAY_SECS: f64 = 2.0;

impl Saves {
    fn new(store: SaveStore) -> Self {
        Self {
            has_save: store.exists(),
            store,
            notice: None,
            dirty_since: None,
        }
    }
}

fn ensure_session(
    mut commands: Commands,
    content: Res<Content>,
    session: Option<Res<Session>>,
    mut saves: ResMut<Saves>,
    mut active: ResMut<ActiveRegion>,
) {
    if session.is_some() {
        return;
    }
    let data = content.0.clone();
    let fresh = || Game::new(content.0.clone());
    let game = match saves.store.load() {
        Ok(None) => fresh(),
        Ok(Some(loaded)) => {
            if loaded.source == LoadSource::Backup {
                warn!(
                    "Main save unreadable ({:?}); restored the backup",
                    loaded.primary_error
                );
                saves.notice =
                    Some("Your last save was damaged, so the one before it was restored.".into());
            }
            match Game::from_state(data, loaded.state) {
                Ok(game) => {
                    info!("Loaded save from {}", saves.store.dir().display());
                    game
                }
                Err(err) => {
                    error!("{err}");
                    quarantine(&mut saves, &err.to_string());
                    fresh()
                }
            }
        }
        Err(err) => {
            error!("Could not load save: {err}");
            quarantine(&mut saves, &err.to_string());
            fresh()
        }
    };
    saves.has_save = game.state().stats.flowers_harvested > 0 || saves.store.exists();
    // Resume in the region the player was in (if it is still open).
    active.0 = game
        .state()
        .player
        .as_ref()
        .and_then(|pose| data_region(&game, &pose.region))
        .filter(|&r| game.is_region_unlocked(r))
        .unwrap_or(0);
    commands.insert_resource(Session {
        game,
        generation: 0,
    });
}

fn data_region(game: &Game, id: &str) -> Option<usize> {
    game.data().region_index(id)
}

/// Move an unreadable save aside so starting fresh never destroys it.
fn quarantine(saves: &mut Saves, reason: &str) {
    match saves.store.quarantine() {
        Ok(Some(path)) => {
            saves.notice = Some(format!(
                "Your save couldn't be read ({reason}). It was kept as {} and a new game was started.",
                path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
            ));
        }
        Ok(None) => {}
        Err(err) => error!("Could not set the unreadable save aside: {err}"),
    }
    saves.has_save = false;
}

/// Start over: a fresh game, the old save removed, the player back at spawn.
pub fn start_new_game(
    commands: &mut Commands,
    content: &Content,
    session: &Session,
    saves: &mut Saves,
) {
    commands.insert_resource(ActiveRegion(0));
    if let Err(err) = saves.store.clear() {
        error!("Could not remove the old save: {err}");
    }
    saves.has_save = false;
    saves.dirty_since = None;
    commands.insert_resource(Session {
        game: Game::new(content.0.clone()),
        generation: session.generation + 1,
    });
    commands.remove_resource::<Player>();
}

fn tick_session(time: Res<Time>, mut session: ResMut<Session>) {
    session.tick(time.delta_secs());
}

fn write_save(
    session: &mut Session,
    saves: &mut Saves,
    player: Option<&Player>,
    active: ActiveRegion,
) {
    if let Some(p) = player {
        let region = session.data().regions[active.0].id.clone();
        let pos = p.body.position;
        session.set_player_pose(PlayerPose {
            region,
            position: pos.to_array(),
            yaw: p.yaw,
            pitch: p.pitch,
        });
    }
    match saves.store.save(session.state()) {
        Ok(report) => {
            session.mark_saved();
            saves.has_save = true;
            saves.dirty_since = None;
            debug!("Saved {} bytes in {:.2} ms", report.bytes, report.millis);
        }
        Err(err) => error!("Saving failed: {err}"),
    }
}

fn autosave(
    time: Res<Time<Real>>,
    session: Option<ResMut<Session>>,
    mut saves: ResMut<Saves>,
    player: Option<Res<Player>>,
    active: Res<ActiveRegion>,
    touring: Option<Res<Touring>>,
) {
    let active = touring.map_or(*active, |t| ActiveRegion(t.home));
    let Some(mut session) = session else { return };
    if !session.is_dirty() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let since = *saves.dirty_since.get_or_insert(now);
    if now - since >= AUTOSAVE_DELAY_SECS {
        write_save(&mut session, &mut saves, player.as_deref(), active);
    }
}

/// Save right away if anything changed since the last save. Also stores
/// the player's position, which changes without dirtying the game.
fn save_now(
    session: Option<ResMut<Session>>,
    mut saves: ResMut<Saves>,
    player: Option<Res<Player>>,
    active: Res<ActiveRegion>,
    touring: Option<Res<Touring>>,
) {
    let active = touring.map_or(*active, |t| ActiveRegion(t.home));
    let Some(mut session) = session else { return };
    if session.is_dirty() || (player.is_some() && session.state().stats.flowers_harvested > 0) {
        write_save(&mut session, &mut saves, player.as_deref(), active);
    }
}

fn save_on_exit(
    mut exits: EventReader<AppExit>,
    session: Option<ResMut<Session>>,
    saves: ResMut<Saves>,
    player: Option<Res<Player>>,
    active: Res<ActiveRegion>,
    touring: Option<Res<Touring>>,
) {
    if let Some(exit) = exits.read().next() {
        let code = match exit {
            AppExit::Success => 0,
            AppExit::Error(code) => code.get() as i32,
        };
        save_now(session, saves, player, active, touring);
        // Shutdown has occasionally hung (first launch after a build; see
        // TODO.md). The save is already safely on disk, so never let quitting
        // leave a frozen window behind: force the exit if it stalls.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(5));
            eprintln!("Shutdown stalled for 5 s after saving; exiting.");
            std::process::exit(code);
        });
    }
}
