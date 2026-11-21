//! Development-only smoke test (debug builds only). Run it with
//! `scripts/smoke.sh`, which plays two sessions:
//!
//! 1. `BLOOM_SMOKE_TEST=<dir>`: fresh save. Walk, sprint, look, jump, pick
//!    five flowers by looking at them and pressing E, open the basket,
//!    pause (saves), return to the title screen.
//! 2. `BLOOM_SMOKE_TEST=<dir> BLOOM_SMOKE_RESUME=1`: restart and Continue.
//!    The five flowers must still be picked, still be in the basket, and not
//!    exist in the world; the player resumes where they stood.
//!
//! Input is injected through the same `ButtonInput` / `AccumulatedMouseMotion`
//! resources real devices feed. The save folder is `<dir>/save`, never the
//! player's real one. Screenshots of every step land in `<dir>`.

use std::path::PathBuf;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::input::InputSystem;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bloom_core::game::GameError;
use bloom_core::persistence::decode;
use bloom_core::player::{CharacterBody, Environment};

use crate::flowers::SpawnedFlowers;
use crate::game::{AppState, Content, PlayState, RegionScene, Saves, Session, Settings};
use crate::interaction::LookTarget;
use crate::player::Player;

/// Point the save folder at `<smoke dir>/save` before anything reads it.
pub fn isolate_save_dir() {
    if let Some(dir) = std::env::var_os("BLOOM_SMOKE_TEST") {
        // Called at startup before any other thread exists.
        std::env::set_var("BLOOM_SAVE_DIR", PathBuf::from(dir).join("save"));
    }
}

pub struct SmokeTestPlugin;

impl Plugin for SmokeTestPlugin {
    fn build(&self, app: &mut App) {
        let Ok(dir) = std::env::var("BLOOM_SMOKE_TEST") else {
            return;
        };
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("create smoke test output dir");
        let resume = std::env::var_os("BLOOM_SMOKE_RESUME").is_some();
        info!(
            "Smoke test ({}) writing to {}",
            if resume { "resume" } else { "play" },
            dir.display()
        );
        app.insert_resource(Smoke {
            dir,
            resume,
            step: 0,
            wait: 0.0,
            failures: Vec::new(),
            taps: Vec::new(),
            holds: Vec::new(),
            mouse: Vec2::ZERO,
            mark: Vec3::ZERO,
            yaw_mark: 0.0,
            picked: Vec::new(),
            aiming: None,
        })
        // The test drives the window itself; don't pause if the OS moves
        // focus elsewhere while it runs. Volume off: no surprise sounds.
        .insert_resource(Settings {
            pause_on_focus_loss: false,
            sound_volume: 0.0,
            ..default()
        })
        .add_systems(PreUpdate, inject_input.after(InputSystem))
        .add_systems(Update, drive);
    }
}

#[derive(Resource)]
struct Smoke {
    dir: PathBuf,
    resume: bool,
    step: usize,
    wait: f32,
    failures: Vec<String>,
    taps: Vec<(KeyCode, bool)>,
    holds: Vec<(KeyCode, f32)>,
    mouse: Vec2,
    mark: Vec3,
    yaw_mark: f32,
    /// Flowers picked in run 1.
    picked: Vec<u32>,
    /// The flower currently being aimed at.
    aiming: Option<u32>,
}

const PICKS: usize = 5;

impl Smoke {
    fn shot(&self, commands: &mut Commands, name: &str) {
        let path = self.dir.join(format!("{name}.png"));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }

    fn check(&mut self, label: &str, ok: bool, detail: impl std::fmt::Display) {
        if ok {
            info!("SMOKE ok   {label} ({detail})");
        } else {
            error!("SMOKE FAIL {label} ({detail})");
            self.failures.push(format!("{label}: {detail}"));
        }
    }

    fn expect<S: States>(&mut self, label: &str, actual: Option<&State<S>>, expected: S) {
        let actual = actual.map(|s| s.get().clone());
        let ok = actual.as_ref() == Some(&expected);
        self.check(
            label,
            ok,
            format!("expected {expected:?}, found {actual:?}"),
        );
    }

    fn tap(&mut self, key: KeyCode) {
        self.taps.push((key, false));
    }

    fn hold(&mut self, key: KeyCode, secs: f32) {
        self.holds.push((key, secs));
    }

    fn expected_path(&self) -> PathBuf {
        self.dir.join("expected_picked.txt")
    }
}

fn inject_input(
    time: Res<Time<Real>>,
    mut smoke: ResMut<Smoke>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut motion: ResMut<AccumulatedMouseMotion>,
) {
    let dt = time.delta_secs();
    // Only injected input counts: whatever the person at the computer does
    // with the real keyboard and mouse while the test runs is discarded.
    keys.reset_all();
    motion.delta = Vec2::ZERO;
    let mut taps = std::mem::take(&mut smoke.taps);
    taps.retain_mut(|(key, pressed)| {
        if *pressed {
            keys.release(*key);
            false
        } else {
            keys.press(*key);
            *pressed = true;
            true
        }
    });
    smoke.taps = taps;
    let mut holds = std::mem::take(&mut smoke.holds);
    holds.retain_mut(|(key, remaining)| {
        if *remaining > 0.0 {
            keys.press(*key);
            *remaining -= dt;
            true
        } else {
            keys.release(*key);
            false
        }
    });
    smoke.holds = holds;
    motion.delta += std::mem::take(&mut smoke.mouse);
}

/// Stand ~1.4 m from the nearest unpicked flower and look straight at it.
fn aim_at_nearest(
    player: &mut Player,
    scene: &RegionScene,
    session: &Session,
    content: &Content,
) -> Option<u32> {
    let field = &scene.field;
    let here = player.body.ground_position();
    let target = (0..field.len() as u32)
        .filter(|&i| !session.is_harvested(scene.region, i))
        .min_by(|&a, &b| {
            field
                .position(a)
                .distance(here)
                .total_cmp(&field.position(b).distance(here))
        })?;
    let flower = field.position(target);
    let away = (here - flower).normalize_or(Vec2::X);
    let layout = &scene.layout;
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let body = CharacterBody::standing_at(flower + away * 1.4, &env);
    player.body = body;
    player.previous = body.position;
    let (center, _) = field.pick_sphere(target);
    let d = center - body.eye(&content.player);
    player.yaw = (-d.x).atan2(-d.z);
    player.pitch = (d.y / d.length()).asin();
    Some(target)
}

type SmokeWorld<'a> = (
    Option<Res<'a, Session>>,
    Option<Res<'a, RegionScene>>,
    Res<'a, Content>,
    Res<'a, Saves>,
);

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut smoke: ResMut<Smoke>,
    app_state: Option<Res<State<AppState>>>,
    play_state: Option<Res<State<PlayState>>>,
    mut player: Option<ResMut<Player>>,
    (session, scene, content, saves): SmokeWorld<'_>,
    (spawned, target, diagnostics): (
        Res<SpawnedFlowers>,
        Res<LookTarget>,
        Res<bevy::diagnostic::DiagnosticsStore>,
    ),
    windows: Query<&Window, With<PrimaryWindow>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
) {
    smoke.wait -= time.delta_secs();
    if smoke.wait > 0.0 {
        return;
    }
    let app_state = app_state.as_deref();
    let play_state = play_state.as_deref();
    let session = session.as_deref();
    let scene = scene.as_deref();
    let body = player.as_ref().map(|p| p.body);
    let locked = windows
        .single()
        .map(|w| w.cursor_options.grab_mode == CursorGrabMode::Locked)
        .unwrap_or(false);

    let next_wait = if smoke.resume {
        match smoke.step {
            0 => 3.5,
            1 => {
                smoke.expect("restart → title", app_state, AppState::MainMenu);
                smoke.check("save found", saves.has_save, saves.store.dir().display());
                smoke.shot(&mut commands, "11_title_continue");
                smoke.tap(KeyCode::Enter);
                1.6
            }
            2 => {
                verify_resumed(
                    &mut smoke,
                    app_state,
                    session,
                    scene,
                    &spawned,
                    player.as_deref(),
                );
                smoke.shot(&mut commands, "12_resumed");
                1.0
            }
            _ => finish(&mut smoke, &mut exit),
        }
    } else {
        match smoke.step {
            0 => 3.5, // window, content, scene + flower models, shader warm-up
            1 => {
                smoke.expect("boot → title", app_state, AppState::MainMenu);
                smoke.check(
                    "fresh save folder",
                    !saves.has_save,
                    saves.store.dir().display(),
                );
                smoke.shot(&mut commands, "01_title");
                smoke.tap(KeyCode::Enter);
                1.6
            }
            2 => {
                smoke.expect("Enter → in game", app_state, AppState::InGame);
                smoke.expect("starts playing", play_state, PlayState::Playing);
                smoke.check("cursor locked while playing", locked, locked);
                smoke.check(
                    "all 100 meadow flowers spawned",
                    spawned.map.len() == 100,
                    spawned.map.len(),
                );
                if let Some(b) = body {
                    smoke.check("spawned on the ground", b.grounded, b.position);
                    smoke.mark = b.position;
                }
                smoke.shot(&mut commands, "02_meadow");
                smoke.hold(KeyCode::KeyW, 1.5);
                1.8
            }
            3 => {
                if let Some(b) = body {
                    let walked = (b.position - smoke.mark).with_y(0.0).length();
                    smoke.check(
                        "W walks forward",
                        (5.0..7.0).contains(&walked),
                        format!("{walked:.2} m"),
                    );
                    smoke.mark = b.position;
                }
                smoke.hold(KeyCode::KeyW, 1.0);
                smoke.hold(KeyCode::ShiftLeft, 1.0);
                1.2
            }
            4 => {
                if let Some(b) = body {
                    let ran = (b.position - smoke.mark).with_y(0.0).length();
                    smoke.check("Shift sprints", ran > 5.5, format!("{ran:.2} m in ~1 s"));
                }
                smoke.yaw_mark = player.as_ref().map_or(0.0, |p| p.yaw);
                smoke.mouse = Vec2::new(-420.0, 60.0);
                0.3
            }
            5 => {
                let yaw = player.as_ref().map_or(0.0, |p| p.yaw);
                let turned = (yaw - smoke.yaw_mark).rem_euclid(std::f32::consts::TAU);
                smoke.check(
                    "mouse turns the view",
                    (0.5..1.5).contains(&turned),
                    format!("{turned:.2} rad"),
                );
                smoke.tap(KeyCode::Space);
                0.2
            }
            6 => {
                if let Some(b) = body {
                    smoke.check("Space jumps", !b.grounded, b.position);
                }
                1.0
            }
            // --- picking: aim (7) → press E (8) → verify (9), five times ---
            7 => {
                let aimed = match (player.as_deref_mut(), scene, session) {
                    (Some(p), Some(sc), Some(se)) => aim_at_nearest(p, sc, se, &content),
                    _ => None,
                };
                smoke.aiming = aimed;
                smoke.check(
                    "found a flower to pick",
                    aimed.is_some(),
                    format!("{aimed:?}"),
                );
                0.35
            }
            8 => {
                let aim = smoke.aiming;
                let on_target = target.flower == aim;
                smoke.check(
                    "crosshair is on the flower",
                    on_target,
                    format!("{:?} vs {aim:?}", target.flower),
                );
                if smoke.picked.is_empty() {
                    smoke.shot(&mut commands, "03_look_prompt");
                }
                smoke.tap(KeyCode::KeyE);
                0.5
            }
            9 => {
                if let (Some(flower), Some(session), Some(scene)) = (smoke.aiming, session, scene) {
                    let picked = session.is_harvested(scene.region, flower);
                    smoke.check("E picked it", picked, flower);
                    let gone = !spawned.map.contains_key(&flower);
                    smoke.check("it left the world", gone, flower);
                    let species = scene.field.species(flower);
                    let in_basket = session.inventory_count(species) >= 1;
                    smoke.check("it is in the basket", in_basket, species);
                    smoke.picked.push(flower);
                }
                if smoke.picked.len() < PICKS {
                    smoke.step = 6; // → 7 again
                }
                0.1
            }
            10 => {
                if let Some(session) = session {
                    let n = session.state().stats.flowers_harvested;
                    smoke.check("five flowers picked", n == PICKS as u64, n);
                    let held: u32 = (0..content.species.len())
                        .map(|s| session.inventory_count(s))
                        .sum();
                    smoke.check("five flowers in the basket", held == PICKS as u32, held);
                    smoke.check("XP gained", session.state().xp > 0, session.state().xp);
                }
                let left = spawned.map.len();
                smoke.check("95 flowers left in the world", left == 100 - PICKS, left);
                let fps = diagnostics
                    .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
                    .and_then(|d| d.smoothed())
                    .unwrap_or(0.0);
                // Debug build on a laptop GPU; release is faster.
                smoke.check("frame rate holds up", fps >= 30.0, format!("{fps:.1} FPS"));
                smoke.shot(&mut commands, "04_after_picking");
                smoke.tap(KeyCode::Tab);
                0.6
            }
            11 => {
                smoke.expect("Tab opens the basket", play_state, PlayState::Inventory);
                smoke.check("cursor free in the basket", !locked, locked);
                smoke.shot(&mut commands, "05_basket");
                smoke.tap(KeyCode::Tab);
                0.4
            }
            12 => {
                smoke.expect("Tab closes it", play_state, PlayState::Playing);
                smoke.tap(KeyCode::Escape);
                0.6
            }
            13 => {
                smoke.expect("Esc pauses", play_state, PlayState::Paused);
                let raw =
                    std::fs::read_to_string(saves.store.dir().join("save.ron")).unwrap_or_default();
                let saved = decode(&raw);
                let n = saved.as_ref().map_or(0, |s| s.stats.flowers_harvested);
                smoke.check(
                    "pausing saved the game",
                    n == PICKS as u64,
                    format!("{n} picked in save"),
                );
                smoke.shot(&mut commands, "06_paused");
                smoke.tap(KeyCode::Escape);
                0.4
            }
            14 => {
                smoke.expect("Esc resumes", play_state, PlayState::Playing);
                next_app.set(AppState::MainMenu);
                0.8
            }
            15 => {
                smoke.expect("back to the title", app_state, AppState::MainMenu);
                smoke.check("title now offers Continue", saves.has_save, saves.has_save);
                let list: Vec<String> = smoke.picked.iter().map(u32::to_string).collect();
                let written = std::fs::write(smoke.expected_path(), list.join(",")).is_ok();
                smoke.check("recorded picked flowers for run 2", written, list.join(","));
                1.0
            }
            _ => finish(&mut smoke, &mut exit),
        }
    };
    smoke.step += 1;
    smoke.wait = next_wait;
}

fn verify_resumed(
    smoke: &mut Smoke,
    app_state: Option<&State<AppState>>,
    session: Option<&Session>,
    scene: Option<&RegionScene>,
    spawned: &SpawnedFlowers,
    player: Option<&Player>,
) {
    smoke.expect("Enter continues", app_state, AppState::InGame);
    let expected: Vec<u32> = std::fs::read_to_string(smoke.expected_path())
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    smoke.check(
        "run 1 recorded its picks",
        expected.len() == PICKS,
        expected.len(),
    );
    let (Some(session), Some(scene)) = (session, scene) else {
        smoke.check("session and scene exist", false, "missing");
        return;
    };
    for &i in &expected {
        smoke.check(
            "picked flower stays picked",
            session.is_harvested(scene.region, i),
            i,
        );
        smoke.check("and does not respawn", !spawned.map.contains_key(&i), i);
    }
    let n = spawned.map.len();
    smoke.check("95 flowers in the world", n == 100 - PICKS, n);
    let held: u32 = (0..session.data().species.len())
        .map(|s| session.inventory_count(s))
        .sum();
    smoke.check("basket restored", held == PICKS as u32, held);
    if let Some(&first) = expected.first() {
        let mut probe = session.game.clone();
        probe.tick(5.0);
        let (c, _) = scene.field.pick_sphere(first);
        let again = probe.harvest(scene.region, first, c + Vec3::Y);
        smoke.check(
            "cannot pick it twice",
            again == Err(GameError::AlreadyHarvested),
            format!("{again:?}"),
        );
    }
    if let (Some(pose), Some(p)) = (&session.state().player, player) {
        let saved = Vec3::from_array(pose.position);
        let d = saved.with_y(0.0).distance(p.body.position.with_y(0.0));
        smoke.check(
            "resumed where the player stood",
            d < 0.5,
            format!("{d:.2} m"),
        );
    }
}

fn finish(smoke: &mut Smoke, exit: &mut EventWriter<AppExit>) -> f32 {
    if smoke.failures.is_empty() {
        info!("SMOKE PASSED");
        exit.write(AppExit::Success);
    } else {
        error!("SMOKE FAILED: {:#?}", smoke.failures);
        exit.write(AppExit::error());
    }
    // If shutdown stalls, the game's own quit watchdog ends the process with
    // this exit code after 5 s (see `game::session`).
    f32::INFINITY
}
