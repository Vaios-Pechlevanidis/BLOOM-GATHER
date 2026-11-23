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
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bloom_core::game::GameError;
use bloom_core::persistence::decode;
use bloom_core::player::{CharacterBody, Environment};

use crate::flowers::SpawnedFlowers;
use crate::game::{AppState, Content, PlayState, RegionScene, Saves, Session, Settings};
use crate::interaction::LookTarget;
use crate::player::Player;
use crate::tools::SelectedMode;
use bloom_core::data::HarvestMode;
use bloom_core::world::layout::to_world;
use bloom_core::world::{InteractableKind, LandmarkKind};

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
            mark_count: 0,
            group: 0,
            count: 0,
            mark_hour: 0.0,
        })
        // The test drives the window itself; don't pause if the OS moves
        // focus elsewhere while it runs. Volume off: no surprise sounds.
        .insert_resource(Settings {
            pause_on_focus_loss: false,
            master_volume: 0.0,
            ..default()
        })
        .add_systems(PreUpdate, inject_input.after(InputSystems))
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
    /// Harvest count before a tool action, and the group it targeted.
    mark_count: u64,
    group: usize,
    /// Repeats left for a step that taps a key several times.
    count: usize,
    /// Time of day before the smoke test jumped to sunset and night.
    mark_hour: f32,
}

/// Tools the smoke test buys (all harvesting modes, scissors and the
/// ribbon for binding bouquets).
const TOOLS: [&str; 5] = [
    "binding_ribbon",
    "garden_scissors",
    "harvesting_basket",
    "pro_gardening_tools",
    "advanced_harvesting_equipment",
];

fn harvested(session: Option<&Session>) -> u64 {
    session.map_or(0, |s| s.state().stats.flowers_harvested)
}

/// Stand 2.6 m from the densest remaining patch and look at the ground there.
fn aim_at_cluster(
    smoke: &mut Smoke,
    player: Option<&mut Player>,
    scene: Option<&RegionScene>,
    session: Option<&Session>,
    content: &Content,
) {
    let (Some(player), Some(scene), Some(session)) = (player, scene, session) else {
        smoke.check("can aim at a patch", false, "missing player/scene/session");
        return;
    };
    let field = &scene.field;
    let open: Vec<u32> = (0..field.len() as u32)
        .filter(|&i| !session.is_harvested(scene.region, i))
        .collect();
    let Some(&best) = open.iter().max_by_key(|&&i| {
        open.iter()
            .filter(|&&j| field.position(i).distance(field.position(j)) < 1.6)
            .count()
    }) else {
        smoke.check("can aim at a patch", false, "no flowers left");
        return;
    };
    let target = field.position(best);
    let layout = &scene.layout;
    let mut stand = target + Vec2::new(0.0, 2.6);
    if !layout.in_bounds(stand, 0.5) {
        stand = target - Vec2::new(0.0, 2.6);
    }
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let body = CharacterBody::standing_at(stand, &env);
    player.body = body;
    player.previous = body.position;
    let ground = Vec3::new(target.x, layout.terrain.height_at(target), target.y);
    let d = ground - body.eye(&content.player);
    player.yaw = (-d.x).atan2(-d.z);
    player.pitch = (d.y / d.length()).asin();
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
    // Injected held keys stay held (no fresh "just pressed" every frame).
    let ours: Vec<KeyCode> = smoke
        .holds
        .iter()
        .map(|h| h.0)
        .chain(smoke.taps.iter().map(|t| t.0))
        .collect();
    let real: Vec<KeyCode> = keys
        .get_pressed()
        .filter(|k| !ours.contains(k))
        .copied()
        .collect();
    for key in real {
        keys.release(key);
    }
    keys.clear();
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

type SmokeView<'a> = (
    Res<'a, SpawnedFlowers>,
    Res<'a, LookTarget>,
    Res<'a, bevy::diagnostic::DiagnosticsStore>,
    Res<'a, SelectedMode>,
);
type SmokeWorld<'a> = (
    Option<ResMut<'a, Session>>,
    Option<Res<'a, RegionScene>>,
    Res<'a, Content>,
    Res<'a, Saves>,
);
type SmokeButtons<'w, 's> = (
    Query<'w, 's, (Entity, &'static Children), With<bevy::ui_widgets::Button>>,
    Query<'w, 's, &'static Text>,
);
type SmokeSettings<'a> = (
    Res<'a, crate::ui::SettingsScreen>,
    Res<'a, Settings>,
    Res<'a, crate::input::Bindings>,
);

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut smoke: ResMut<Smoke>,
    app_state: Option<Res<State<AppState>>>,
    play_state: Option<Res<State<PlayState>>>,
    mut player: Option<ResMut<Player>>,
    (mut session_mut, scene, content, saves): SmokeWorld<'_>,
    (spawned, target, diagnostics, selected): SmokeView<'_>,
    (settings_screen, settings, bindings): SmokeSettings<'_>,
    (buttons, texts): SmokeButtons,
    windows: Query<&CursorOptions, With<PrimaryWindow>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut daylight: ResMut<crate::rendering::Daylight>,
) {
    smoke.wait -= time.delta_secs();
    if smoke.wait > 0.0 {
        return;
    }
    let app_state = app_state.as_deref();
    let play_state = play_state.as_deref();
    let session = session_mut.as_deref();
    let scene = scene.as_deref();
    let body = player.as_ref().map(|p| p.body);
    let locked = windows
        .single()
        .map(|w| w.grab_mode == CursorGrabMode::Locked)
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
            // --- the ending: finish the game through the real rules ---
            3 => {
                let file = std::fs::read_to_string(saves.store.dir().join("settings.ron"))
                    .unwrap_or_default();
                let saved: Option<Settings> = ron::from_str(&file).ok();
                let ok = saved.as_ref().is_some_and(|s| {
                    s.quality == crate::game::Quality::Medium
                        && s.keys.get("map").map(String::as_str) == Some("N")
                });
                smoke.check("settings and key bindings were saved", ok, ok);
                if let Some(session) = session_mut.as_deref_mut() {
                    match complete_game(&session.game) {
                        Ok(game) => session.game = game,
                        Err(e) => smoke.check("the game can be completed", false, e),
                    }
                    smoke.check(
                        "every requirement met",
                        session.completion().is_complete() && session.finale_pending(),
                        format!("{:?}", session.completion()),
                    );
                }
                4.0
            }
            4 => {
                smoke.expect("the ending begins", play_state, PlayState::Finale);
                smoke.shot(&mut commands, "30_finale_celebration");
                6.0
            }
            5 => {
                let here = scene.map(|s| s.region);
                smoke.check(
                    "the tour starts on the farm",
                    here == Some(0),
                    format!("{here:?}"),
                );
                smoke.shot(&mut commands, "31_finale_tour");
                9.4
            }
            6 => {
                let here = scene.map(|s| s.region);
                // With one map the tour stays on the farm.
                let next = (content.regions.len() > 1) as usize;
                smoke.check(
                    "the tour carries on",
                    here == Some(next),
                    format!("{here:?}"),
                );
                smoke.shot(&mut commands, "32_finale_tour_later");
                smoke.tap(KeyCode::Escape);
                3.0
            }
            7 => {
                smoke.expect("Esc skips to the summary", play_state, PlayState::Finale);
                let here = scene.map(|s| s.region);
                smoke.check(
                    "back in the player's region",
                    here == Some(0),
                    format!("{here:?}"),
                );
                smoke.shot(&mut commands, "33_journey_summary");
                smoke.tap(KeyCode::Enter);
                0.8
            }
            8 => {
                smoke.expect("Enter: keep wandering", play_state, PlayState::Playing);
                let seen = session_mut
                    .as_deref()
                    .is_some_and(|s| s.is_game_complete() && !s.finale_pending());
                smoke.check("free roam after the ending", seen, seen);
                smoke.shot(&mut commands, "34_free_roam");
                smoke.tap(KeyCode::KeyC);
                0.6
            }
            9 => {
                smoke.expect("C opens the herbarium", play_state, PlayState::Collection);
                smoke.shot(&mut commands, "35_herbarium");
                smoke.tap(KeyCode::KeyC);
                0.5
            }
            10 => {
                smoke.expect("C closes it", play_state, PlayState::Playing);
                smoke.tap(KeyCode::KeyJ);
                0.5
            }
            11 => {
                smoke.tap(KeyCode::Digit3);
                0.5
            }
            12 => {
                smoke.shot(&mut commands, "36_journal_complete");
                smoke.tap(KeyCode::KeyJ);
                0.4
            }
            13 => {
                smoke.tap(KeyCode::Escape);
                0.6
            }
            14 => {
                smoke.expect("Esc pauses (and saves)", play_state, PlayState::Paused);
                let raw =
                    std::fs::read_to_string(saves.store.dir().join("save.ron")).unwrap_or_default();
                let saved = decode(&raw).ok();
                let ok = saved
                    .as_ref()
                    .is_some_and(|s| s.completed.is_some() && s.finale_seen);
                smoke.check("the save remembers the ending", ok, ok);
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
                    "every flower of the farm spawned",
                    spawned.map.len() == content.regions[0].flower_total as usize,
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
                // A new player carries one flower at a time; the hut takes
                // it off their hands (standing in for the walk there).
                if let Some(s) = session_mut.as_deref_mut() {
                    smoke.check(
                        "hands full after one flower",
                        s.carry_space() == 0,
                        format!("{} / {}", s.carried(), s.carry_capacity()),
                    );
                    s.set_at_workshop(true);
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
                let total = content.regions[0].flower_total as usize;
                smoke.check(
                    "the picked flowers left the world",
                    left == total - PICKS,
                    left,
                );
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
                smoke.step = 29; // → the tool steps (30…), which return to 13
                0.1
            }
            // --- tools: buy them, open the shop, use every harvesting mode ---
            30 => {
                if let Some(session) = session_mut.as_deref_mut() {
                    // Coins and levels for the test, via a restored state (the
                    // game itself has no cheat API).
                    let mut state = session.state().clone();
                    state.coins = 20_000;
                    state.xp = content.progression.level_curve.total_xp_for_level(20);
                    match bloom_core::Game::from_state(session.data().clone(), state) {
                        Ok(game) => session.game = game,
                        Err(e) => smoke.check("restored test state", false, e),
                    }
                    for id in TOOLS {
                        let i = content
                            .tools
                            .iter()
                            .position(|t| t.id == id)
                            .unwrap_or(usize::MAX);
                        let bought = session.buy_tool(i);
                        smoke.check("bought a tool", bought.is_ok(), format!("{id}: {bought:?}"));
                    }
                }
                smoke.tap(KeyCode::Tab);
                0.5
            }
            31 => {
                smoke.expect("basket screen open", play_state, PlayState::Inventory);
                smoke.tap(KeyCode::Digit2);
                0.5
            }
            32 => {
                smoke.shot(&mut commands, "07_tool_shop");
                smoke.tap(KeyCode::Tab);
                0.4
            }
            33 => {
                smoke.tap(KeyCode::Digit2);
                0.2
            }
            34 => {
                smoke.check(
                    "key 2 selects the basket",
                    selected.0 == HarvestMode::Radius,
                    format!("{:?}", selected.0),
                );
                aim_at_cluster(
                    &mut smoke,
                    player.as_deref_mut(),
                    scene,
                    session_mut.as_deref(),
                    &content,
                );
                0.4
            }
            35 => {
                let n = target.group.len();
                smoke.check("basket highlights a group", n > 0, n);
                smoke.mark_count = harvested(session_mut.as_deref());
                smoke.group = n;
                smoke.shot(&mut commands, "08_basket_aim");
                smoke.tap(KeyCode::KeyE);
                0.6
            }
            36 => {
                let gained = harvested(session_mut.as_deref()) - smoke.mark_count;
                let group = smoke.group as u64;
                smoke.check(
                    "basket gathered the group",
                    gained == group,
                    format!("{gained} vs {group}"),
                );
                smoke.tap(KeyCode::Digit3);
                0.2
            }
            37 => {
                smoke.check(
                    "key 3 selects the rake",
                    selected.0 == HarvestMode::Sweep,
                    format!("{:?}", selected.0),
                );
                aim_at_cluster(
                    &mut smoke,
                    player.as_deref_mut(),
                    scene,
                    session_mut.as_deref(),
                    &content,
                );
                smoke.mark_count = harvested(session_mut.as_deref());
                smoke.hold(KeyCode::KeyE, 1.2);
                0.6
            }
            38 => {
                smoke.shot(&mut commands, "09_rake_sweeping");
                0.9
            }
            39 => {
                let gained = harvested(session_mut.as_deref()) - smoke.mark_count;
                smoke.check("holding E sweeps flowers up", gained > 0, gained);
                // Empty hands before the cart (as after a trip to the hut).
                if let Some(s) = session_mut.as_deref_mut() {
                    s.set_at_workshop(true);
                }
                smoke.tap(KeyCode::Digit4);
                0.2
            }
            40 => {
                smoke.check(
                    "key 4 selects the cart",
                    selected.0 == HarvestMode::Area,
                    format!("{:?}", selected.0),
                );
                aim_at_cluster(
                    &mut smoke,
                    player.as_deref_mut(),
                    scene,
                    session_mut.as_deref(),
                    &content,
                );
                smoke.mark_count = harvested(session_mut.as_deref());
                smoke.hold(KeyCode::KeyE, 1.3);
                0.3
            }
            41 => {
                // Turn while holding to drag out the rectangle.
                smoke.mouse = Vec2::new(280.0, 60.0);
                0.5
            }
            42 => {
                let n = target.group.len();
                smoke.check("cart marks an area", n > 0, n);
                smoke.shot(&mut commands, "10_cart_marking");
                0.9
            }
            43 => {
                let gained = harvested(session_mut.as_deref()) - smoke.mark_count;
                smoke.check("releasing E gathers the area", gained > 0, gained);
                smoke.tap(KeyCode::Digit1);
                smoke.step = 49; // → the workshop steps (50…)
                0.4
            }
            // --- workshop: walk up to the workbench, bind bouquets, sell tab ---
            50 => {
                stand_in_workshop(
                    &mut smoke,
                    player.as_deref_mut(),
                    scene,
                    &content,
                    Vec2::new(0.0, -0.7),
                    InteractableKind::Workbench,
                );
                0.5
            }
            51 => {
                smoke.check(
                    "looking at the workbench",
                    target.station == Some(InteractableKind::Workbench),
                    format!("{:?}", target.station),
                );
                smoke.shot(&mut commands, "11_workshop_inside");
                smoke.tap(KeyCode::KeyE);
                0.5
            }
            52 => {
                smoke.expect("E opens the workbench", play_state, PlayState::Workshop);
                smoke.tap(KeyCode::ArrowRight);
                0.2
            }
            53 => {
                smoke.tap(KeyCode::ArrowRight);
                0.4
            }
            54 => {
                smoke.shot(&mut commands, "12_workbench");
                smoke.tap(KeyCode::Enter);
                0.3
            }
            55 => {
                let job = session_mut.as_deref().and_then(|s| s.craft_job().cloned());
                smoke.check(
                    "Enter binds three Simple Bouquets",
                    job.as_ref()
                        .is_some_and(|j| j.recipe == "simple_bouquet" && j.count == 3),
                    format!("{job:?}"),
                );
                2.0
            }
            56 => {
                let n = session_mut.as_deref().map_or(0, |s| s.bouquet_count(0));
                smoke.check("the batch finished while the screen was open", n == 3, n);
                smoke.tap(KeyCode::Digit2);
                0.5
            }
            57 => {
                smoke.shot(&mut commands, "13_bouquets_tab");
                smoke.tap(KeyCode::Escape);
                0.4
            }
            58 => {
                smoke.expect("Esc closes the workbench", play_state, PlayState::Playing);
                stand_in_workshop(
                    &mut smoke,
                    player.as_deref_mut(),
                    scene,
                    &content,
                    Vec2::new(1.1, 0.2),
                    InteractableKind::BouquetDisplay,
                );
                0.8
            }
            59 => {
                smoke.check(
                    "looking at the bouquet display",
                    target.station == Some(InteractableKind::BouquetDisplay),
                    format!("{:?}", target.station),
                );
                smoke.shot(&mut commands, "14_bouquet_display");
                stand_in_workshop(
                    &mut smoke,
                    player.as_deref_mut(),
                    scene,
                    &content,
                    Vec2::new(-1.6, -0.6),
                    InteractableKind::ToolShelf,
                );
                0.5
            }
            // --- upgrades at the tool shelf, then the skill tree ---
            60 => {
                smoke.check(
                    "looking at the tool shelf",
                    target.station == Some(InteractableKind::ToolShelf),
                    format!("{:?}", target.station),
                );
                smoke.tap(KeyCode::KeyE);
                0.5
            }
            61 => {
                smoke.expect("E opens the shelf", play_state, PlayState::Inventory);
                smoke.tap(KeyCode::Digit3);
                0.5
            }
            62 => {
                smoke.shot(&mut commands, "15_upgrades");
                if let Some(s) = session_mut.as_deref() {
                    smoke.mark_count = s.state().coins;
                }
                smoke.tap(KeyCode::Enter);
                0.4
            }
            63 => {
                if let Some(s) = session_mut.as_deref() {
                    let level = s.upgrade_level(0);
                    let paid = smoke.mark_count - s.state().coins;
                    smoke.check(
                        "Enter buys the first upgrade",
                        level == 1 && paid == s.data().upgrades[0].base_price,
                        format!("level {level}, paid {paid}"),
                    );
                }
                smoke.tap(KeyCode::Escape);
                0.4
            }
            64 => {
                smoke.expect("Esc closes the shelf", play_state, PlayState::Playing);
                smoke.tap(KeyCode::KeyK);
                0.5
            }
            65 => {
                smoke.expect("K opens the skill tree", play_state, PlayState::Skills);
                smoke.shot(&mut commands, "16_skill_tree");
                smoke.tap(KeyCode::Enter);
                0.3
            }
            66 => {
                smoke.tap(KeyCode::ArrowRight);
                0.3
            }
            67 => {
                smoke.tap(KeyCode::Enter);
                0.3
            }
            68 => {
                smoke.tap(KeyCode::ArrowDown);
                0.3
            }
            69 => {
                smoke.tap(KeyCode::Enter);
                0.4
            }
            70 => {
                let learned: Vec<String> = session_mut
                    .as_deref()
                    .map(|s| s.state().skills.keys().cloned().collect())
                    .unwrap_or_default();
                smoke.check(
                    "Enter learns skills, arrows move between them",
                    learned == ["botanical_knowledge", "flower_recognition", "strong_arms"],
                    learned.join(","),
                );
                smoke.shot(&mut commands, "17_skills_learned");
                smoke.tap(KeyCode::KeyK);
                0.4
            }
            71 => {
                smoke.expect("K closes the skill tree", play_state, PlayState::Playing);
                aim_at_cluster(
                    &mut smoke,
                    player.as_deref_mut(),
                    scene,
                    session_mut.as_deref(),
                    &content,
                );
                0.8
            }
            72 => {
                smoke.shot(&mut commands, "18_new_species_sparkle");
                smoke.tap(KeyCode::KeyJ);
                0.5
            }
            // --- the journal: achievements, statistics, progress ---
            73 => {
                smoke.expect("J opens the journal", play_state, PlayState::Journal);
                if let Some(s) = session_mut.as_deref() {
                    let got = &s.state().achievements;
                    let want = [
                        "first_flower",
                        "first_discovery",
                        "first_bouquet",
                        "first_rare",
                    ];
                    let missing: Vec<&str> = want
                        .iter()
                        .copied()
                        .filter(|id| !got.contains(*id))
                        .collect();
                    // first_rare only if a rare flower was picked (the farm
                    // has rare ones, so it may or may not be unlocked yet).
                    let ok = missing.is_empty() || missing == ["first_rare"];
                    smoke.check(
                        "early achievements unlocked",
                        ok,
                        format!("missing {missing:?}"),
                    );
                    let walked = s.state().stats.distance_walked_m;
                    smoke.check(
                        "distance walked is counted",
                        walked > 1.0,
                        format!("{walked:.1} m"),
                    );
                }
                smoke.shot(&mut commands, "19_journal_achievements");
                smoke.tap(KeyCode::Digit2);
                0.5
            }
            74 => {
                smoke.shot(&mut commands, "20_journal_statistics");
                smoke.tap(KeyCode::Digit3);
                0.5
            }
            75 => {
                smoke.shot(&mut commands, "21_journal_progress");
                smoke.tap(KeyCode::KeyJ);
                0.4
            }
            76 => {
                smoke.expect("J closes the journal", play_state, PlayState::Playing);
                // Open every region for the test via a restored state (the
                // game has no cheat API), then open the world map.
                if let Some(session) = session_mut.as_deref_mut() {
                    let mut state = session.state().clone();
                    state.unlocked_regions = content.regions.iter().map(|r| r.id.clone()).collect();
                    match bloom_core::Game::from_state(session.data().clone(), state) {
                        Ok(game) => session.game = game,
                        Err(e) => smoke.check("restored test state", false, e),
                    }
                }
                smoke.tap(KeyCode::KeyM);
                0.6
            }
            // --- the farm map, the save, and a whole day ---
            77 => {
                smoke.expect("M opens the map", play_state, PlayState::Map);
                smoke.check("cursor free on the map", !locked, locked);
                smoke.shot(&mut commands, "22_farm_map");
                smoke.tap(KeyCode::KeyM);
                0.6
            }
            78 => {
                smoke.expect("M closes the map", play_state, PlayState::Playing);
                smoke.tap(KeyCode::Escape);
                0.6
            }
            79 => {
                smoke.expect("Esc pauses (and saves)", play_state, PlayState::Paused);
                let raw =
                    std::fs::read_to_string(saves.store.dir().join("save.ron")).unwrap_or_default();
                let region = decode(&raw)
                    .ok()
                    .and_then(|s| s.player)
                    .map(|p| p.region)
                    .unwrap_or_default();
                smoke.check(
                    "the save remembers the farm",
                    region == content.regions[0].id,
                    &region,
                );
                smoke.tap(KeyCode::Escape);
                0.5
            }
            80 => {
                smoke.check("the farm has a day cycle", daylight.cycle, daylight.clock());
                smoke.mark_hour = daylight.hour;
                daylight.hour = 20.4;
                1.5
            }
            81 => {
                smoke.check(
                    "the sun sets in the evening",
                    daylight.dusk > 0.3 && daylight.elevation_deg < 8.0,
                    format!("{} dusk {:.2}", daylight.clock(), daylight.dusk),
                );
                smoke.shot(&mut commands, "23_sunset");
                daylight.hour = 0.5;
                1.5
            }
            82 => {
                smoke.check(
                    "night falls with the moon up",
                    daylight.night > 0.9 && daylight.moon_dir.y > 0.3,
                    format!("{} night {:.2}", daylight.clock(), daylight.night),
                );
                smoke.shot(&mut commands, "24_night");
                daylight.hour = smoke.mark_hour;
                1.0
            }
            83 => {
                let passed = daylight.hour > smoke.mark_hour;
                smoke.check("time passes while playing", passed, daylight.clock());
                smoke.tap(KeyCode::Escape);
                0.6
            }
            // (Steps once used for travelling between regions.)
            84..=97 => 0.0,
            // --- the settings screen, all by keyboard ---
            98 => {
                smoke.expect("Esc pauses", play_state, PlayState::Paused);
                smoke.tap(KeyCode::KeyO);
                0.5
            }
            99 => {
                smoke.check(
                    "O opens the settings",
                    settings_screen.open,
                    settings_screen.open,
                );
                smoke.shot(&mut commands, "37_settings_controls");
                smoke.tap(KeyCode::Digit2);
                0.4
            }
            100 => {
                smoke.tap(KeyCode::ArrowLeft);
                0.4
            }
            101 => {
                let q = settings.quality;
                smoke.check(
                    "Left lowers the graphics quality",
                    q == crate::game::Quality::Medium,
                    format!("{q:?}"),
                );
                smoke.shot(&mut commands, "38_settings_video");
                smoke.tap(KeyCode::Digit4);
                smoke.count = 10; // down to "World map"
                0.4
            }
            102 => {
                if smoke.count > 0 {
                    smoke.count -= 1;
                    smoke.tap(KeyCode::ArrowDown);
                    smoke.step = 101; // → 102 again
                    0.15
                } else {
                    smoke.tap(KeyCode::Enter);
                    0.4
                }
            }
            103 => {
                smoke.tap(KeyCode::KeyN);
                0.5
            }
            104 => {
                let map = bindings.map.first().copied();
                smoke.check(
                    "the world map is rebound to N",
                    map == Some(KeyCode::KeyN)
                        && settings.keys.get("map").map(String::as_str) == Some("N"),
                    format!("{map:?}"),
                );
                smoke.shot(&mut commands, "39_settings_keys");
                smoke.tap(KeyCode::Escape);
                0.5
            }
            105 => {
                smoke.check(
                    "Esc closes the settings",
                    !settings_screen.open,
                    settings_screen.open,
                );
                smoke.expect("still paused", play_state, PlayState::Paused);
                smoke.step = 12; // → 13: the pause saved the game
                0.2
            }
            13 => {
                smoke.expect("Esc pauses", play_state, PlayState::Paused);
                let raw =
                    std::fs::read_to_string(saves.store.dir().join("save.ron")).unwrap_or_default();
                let saved = decode(&raw);
                let n = saved.as_ref().map_or(0, |s| s.stats.flowers_harvested);
                let live = harvested(session_mut.as_deref());
                smoke.check(
                    "pausing saved the game",
                    n == live && n > PICKS as u64,
                    format!("{n} picked in save, {live} live"),
                );
                smoke.shot(&mut commands, "06_paused");
                // Click "Resume" the way a mouse click does (the button
                // widget's Activate event).
                let clicked = click_button(&mut commands, &buttons, &texts, "Resume");
                smoke.check("found the Resume button", clicked, clicked);
                0.4
            }
            14 => {
                smoke.expect("clicking Resume resumes", play_state, PlayState::Playing);
                next_app.set(AppState::MainMenu);
                0.8
            }
            15 => {
                smoke.expect("back to the title", app_state, AppState::MainMenu);
                smoke.check("title now offers Continue", saves.has_save, saves.has_save);
                // Everything picked (by hand and with tools) and the tools owned.
                let growth = session_mut.as_deref().map_or(String::new(), |se| {
                    format!(
                        "{}\n{}\n{}\n{}",
                        levels_line(&se.state().upgrades),
                        levels_line(&se.state().skills),
                        se.state()
                            .achievements
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(","),
                        se.state()
                            .visited_regions
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                });
                let (picks, tools, bouquets, held): (Vec<String>, Vec<String>, Vec<String>, u32) =
                    match (session_mut.as_deref(), scene) {
                        (Some(se), Some(sc)) => (
                            (0..sc.field.len() as u32)
                                .filter(|&i| se.is_harvested(sc.region, i))
                                .map(|i| i.to_string())
                                .collect(),
                            se.state().tools.iter().cloned().collect(),
                            se.state()
                                .bouquets
                                .iter()
                                .map(|(id, n)| format!("{id}={n}"))
                                .collect(),
                            se.state().inventory.values().sum(),
                        ),
                        _ => (Vec::new(), Vec::new(), Vec::new(), 0),
                    };
                let written = std::fs::write(
                    smoke.expected_path(),
                    format!(
                        "{}\n{}\n{}\n{held}\n{growth}",
                        picks.join(","),
                        tools.join(","),
                        bouquets.join(",")
                    ),
                )
                .is_ok();
                smoke.check(
                    "recorded picks and tools for run 2",
                    written,
                    format!("{} picks", picks.len()),
                );
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
    let recorded = std::fs::read_to_string(smoke.expected_path()).unwrap_or_default();
    let mut lines = recorded.lines();
    let expected: Vec<u32> = lines
        .next()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    let tools: Vec<String> = lines
        .next()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let bouquets = lines.next().unwrap_or("").to_string();
    let held_before: u32 = lines
        .next()
        .and_then(|l| l.parse().ok())
        .unwrap_or(u32::MAX);
    let upgrades = lines.next().unwrap_or("").to_string();
    let skills = lines.next().unwrap_or("").to_string();
    let achievements = lines.next().unwrap_or("").to_string();
    let visited = lines.next().unwrap_or("").to_string();
    smoke.check(
        "run 1 recorded its picks",
        expected.len() > PICKS,
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
    smoke.check(
        "the rest still bloom",
        n == scene.field.len() - expected.len(),
        format!("{n} in the world"),
    );
    let owned: Vec<String> = session.state().tools.iter().cloned().collect();
    smoke.check(
        "tools are still owned",
        owned == tools && tools.len() == TOOLS.len(),
        owned.join(","),
    );
    let held: u32 = (0..session.data().species.len())
        .map(|s| session.inventory_count(s))
        .sum();
    smoke.check(
        "basket restored",
        held == held_before,
        format!("{held} vs {held_before}"),
    );
    let owned_bouquets: Vec<String> = session
        .state()
        .bouquets
        .iter()
        .map(|(id, n)| format!("{id}={n}"))
        .collect();
    let upgrades_now = levels_line(&session.state().upgrades);
    smoke.check(
        "upgrades restored",
        upgrades_now == upgrades && !upgrades.is_empty(),
        &upgrades,
    );
    let skills_now = levels_line(&session.state().skills);
    smoke.check(
        "skills restored",
        skills_now == skills && !skills.is_empty(),
        &skills,
    );
    let achievements_now: Vec<String> = session.state().achievements.iter().cloned().collect();
    smoke.check(
        "achievements restored",
        achievements_now.join(",") == achievements && !achievements.is_empty(),
        &achievements,
    );
    let visited_now: Vec<String> = session.state().visited_regions.iter().cloned().collect();
    smoke.check(
        "visited regions restored",
        visited_now.join(",") == visited && visited_now.len() == session.data().regions.len(),
        &visited,
    );
    let walked = session.state().stats.distance_walked_m;
    smoke.check(
        "distance walked restored",
        walked > 1.0,
        format!("{walked:.1} m"),
    );
    smoke.check(
        "bouquets restored",
        owned_bouquets.join(",") == bouquets && !bouquets.is_empty(),
        &bouquets,
    );
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

/// Play the rest of the game through the real rules (pick every flower,
/// buy every tool, bind every required bouquet, rarest first).
fn complete_game(game: &bloom_core::Game) -> Result<bloom_core::Game, String> {
    let mut g = game.clone();
    let d = g.data().clone();
    // Coins for the tools; everything else is earned.
    let mut state = g.state().clone();
    state.coins += 50_000;
    g = bloom_core::Game::from_state(d.clone(), state).map_err(|e| e.to_string())?;
    for r in 0..d.regions.len() {
        let field = g.region(r).field.clone();
        for i in 0..field.len() as u32 {
            if g.is_harvested(r, i) {
                continue;
            }
            let (c, _) = field.pick_sphere(i);
            g.tick(1.0);
            g.harvest(r, i, c + Vec3::Y).map_err(|e| e.to_string())?;
            // Carry limit: leave every flower at the hut straight away.
            g.set_at_workshop(true);
            g.set_at_workshop(false);
        }
    }
    g.tick(1_000.0);
    for t in 0..d.tools.len() {
        if !g.owns_tool(t) {
            g.buy_tool(t).map_err(|e| e.to_string())?;
        }
    }
    let mut order: Vec<usize> = (0..d.recipes.len())
        .filter(|&r| d.recipes[r].required_for_completion)
        .collect();
    order.sort_by_key(|&r| std::cmp::Reverse(d.recipes[r].tier));
    for r in order {
        if g.state()
            .crafted
            .get(&d.recipes[r].id)
            .copied()
            .unwrap_or(0)
            > 0
        {
            continue;
        }
        g.tick(100.0);
        g.craft(r, 1, &std::collections::BTreeSet::new())
            .map_err(|e| format!("{}: {e}", d.recipes[r].id))?;
    }
    g.tick(100.0);
    Ok(g)
}

/// Trigger `Activate` on the button whose label is `label` (what a mouse
/// click on it does). False if there is no such button.
fn click_button(
    commands: &mut Commands,
    buttons: &Query<(Entity, &Children), With<bevy::ui_widgets::Button>>,
    texts: &Query<&Text>,
    label: &str,
) -> bool {
    let Some(entity) = buttons.iter().find_map(|(e, kids)| {
        kids.iter()
            .any(|k| texts.get(k).is_ok_and(|t| t.0 == label))
            .then_some(e)
    }) else {
        return false;
    };
    commands.trigger(bevy::ui_widgets::Activate { entity });
    true
}

/// `id=level,...` for the expected-state file.
fn levels_line(levels: &std::collections::BTreeMap<String, u32>) -> String {
    levels
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Stand inside the workshop at `local` (workshop coordinates) and look at
/// one of its stations.
fn stand_in_workshop(
    smoke: &mut Smoke,
    player: Option<&mut Player>,
    scene: Option<&RegionScene>,
    content: &Content,
    local: Vec2,
    station: InteractableKind,
) {
    let (Some(player), Some(scene)) = (player, scene) else {
        smoke.check("can enter the workshop", false, "missing player/scene");
        return;
    };
    let layout = &scene.layout;
    let (Some(workshop), Some(item)) = (
        layout
            .landmarks
            .iter()
            .find(|l| l.kind == LandmarkKind::Workshop),
        layout.interactables.iter().find(|i| i.kind == station),
    ) else {
        smoke.check("the meadow has a workshop", false, "none");
        return;
    };
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let body = CharacterBody::standing_at(to_world(workshop.position, workshop.yaw, local), &env);
    player.body = body;
    player.previous = body.position;
    let d = item.position - body.eye(&content.player);
    player.yaw = (-d.x).atan2(-d.z);
    player.pitch = (d.y / d.length()).asin();
}

fn finish(smoke: &mut Smoke, exit: &mut MessageWriter<AppExit>) -> f32 {
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
