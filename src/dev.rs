//! Development-only smoke test (debug builds only).
//!
//! `BLOOM_SMOKE_TEST=<dir> cargo run` launches the real game, plays it with
//! injected keyboard and mouse input (through the same `ButtonInput` and
//! `AccumulatedMouseMotion` resources real devices feed), asserts what
//! happened, saves a screenshot of each step into `<dir>` and exits non-zero
//! if anything went wrong.
//!
//! Note: while it runs, the game locks the mouse cursor like normal play.

use std::path::PathBuf;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::input::InputSystem;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::game::{AppState, PlayState, Settings};
use crate::player::Player;

pub struct SmokeTestPlugin;

impl Plugin for SmokeTestPlugin {
    fn build(&self, app: &mut App) {
        let Ok(dir) = std::env::var("BLOOM_SMOKE_TEST") else {
            return;
        };
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("create smoke test output dir");
        info!(
            "Smoke test enabled, writing screenshots to {}",
            dir.display()
        );
        app.insert_resource(Smoke {
            dir,
            step: 0,
            wait: 0.0,
            failures: Vec::new(),
            taps: Vec::new(),
            holds: Vec::new(),
            mouse: Vec2::ZERO,
            mark: Vec3::ZERO,
            yaw_mark: 0.0,
        })
        // The test drives the window itself; don't pause if the OS moves
        // focus elsewhere while it runs.
        .insert_resource(Settings {
            pause_on_focus_loss: false,
            ..default()
        })
        .add_systems(PreUpdate, inject_input.after(InputSystem))
        .add_systems(Update, drive);
    }
}

#[derive(Resource)]
struct Smoke {
    dir: PathBuf,
    step: usize,
    wait: f32,
    failures: Vec<String>,
    /// Keys to press for exactly one frame.
    taps: Vec<(KeyCode, bool)>,
    /// Keys to hold for a duration (seconds remaining).
    holds: Vec<(KeyCode, f32)>,
    /// Raw mouse motion to inject next frame.
    mouse: Vec2,
    mark: Vec3,
    yaw_mark: f32,
}

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
}

fn inject_input(
    time: Res<Time<Real>>,
    mut smoke: ResMut<Smoke>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut motion: ResMut<AccumulatedMouseMotion>,
) {
    let dt = time.delta_secs();
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

fn drive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut smoke: ResMut<Smoke>,
    app_state: Option<Res<State<AppState>>>,
    play_state: Option<Res<State<PlayState>>>,
    player: Option<Res<Player>>,
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
    let body = player.as_ref().map(|p| p.body);
    let locked = windows
        .single()
        .map(|w| w.cursor_options.grab_mode == CursorGrabMode::Locked)
        .unwrap_or(false);

    let next_wait = match smoke.step {
        0 => 3.0, // window, content, scene build, shader warm-up
        1 => {
            smoke.expect("boot → main menu", app_state, AppState::MainMenu);
            smoke.shot(&mut commands, "01_main_menu");
            0.5
        }
        2 => {
            smoke.tap(KeyCode::Enter);
            1.6 // camera blends into the player's eyes
        }
        3 => {
            smoke.expect("Enter → in game", app_state, AppState::InGame);
            smoke.expect("starts playing", play_state, PlayState::Playing);
            smoke.check("cursor locked while playing", locked, locked);
            match body {
                Some(b) => {
                    smoke.check("spawned on the ground", b.grounded, b.position);
                    smoke.mark = b.position;
                }
                None => smoke.check("player exists", false, "no Player resource"),
            }
            smoke.shot(&mut commands, "02_spawn_view");
            smoke.hold(KeyCode::KeyW, 1.5);
            1.8
        }
        4 => {
            if let Some(b) = body {
                let walked = (b.position - smoke.mark).with_y(0.0).length();
                // 1.5 s at 4.2 m/s minus acceleration ≈ 6 m.
                smoke.check(
                    "W walks forward",
                    (5.0..7.0).contains(&walked),
                    format!("{walked:.2} m"),
                );
                let north = b.position.z < smoke.mark.z;
                smoke.check("walked north (−Z)", north, b.position);
                smoke.mark = b.position;
            }
            smoke.hold(KeyCode::KeyW, 1.0);
            smoke.hold(KeyCode::ShiftLeft, 1.0);
            1.2
        }
        5 => {
            if let Some(b) = body {
                let ran = (b.position - smoke.mark).with_y(0.0).length();
                smoke.check("Shift sprints", ran > 6.0, format!("{ran:.2} m in ~1 s"));
            }
            smoke.yaw_mark = player.as_ref().map_or(0.0, |p| p.yaw);
            smoke.mouse = Vec2::new(-420.0, 60.0); // look left and a little down
            0.3
        }
        6 => {
            let yaw = player.as_ref().map_or(0.0, |p| p.yaw);
            let turned = (yaw - smoke.yaw_mark).rem_euclid(std::f32::consts::TAU);
            smoke.check(
                "mouse turns the view left",
                (0.5..1.5).contains(&turned),
                format!("{:.2} rad", turned),
            );
            smoke.shot(&mut commands, "03_looking_around");
            smoke.tap(KeyCode::Space);
            0.2
        }
        7 => {
            if let Some(b) = body {
                smoke.check(
                    "Space jumps",
                    !b.grounded && b.velocity.y != 0.0,
                    b.position,
                );
            }
            1.0
        }
        8 => {
            if let Some(b) = body {
                smoke.check("lands again", b.grounded, b.position);
            }
            smoke.tap(KeyCode::F3);
            0.6
        }
        9 => {
            smoke.shot(&mut commands, "04_debug_overlay");
            smoke.tap(KeyCode::F3);
            smoke.hold(KeyCode::KeyW, 4.0);
            smoke.hold(KeyCode::ShiftLeft, 4.0);
            4.3
        }
        10 => {
            smoke.shot(&mut commands, "05_after_run");
            smoke.tap(KeyCode::Escape);
            0.6
        }
        11 => {
            smoke.expect("Esc → paused", play_state, PlayState::Paused);
            smoke.check("cursor released when paused", !locked, locked);
            smoke.shot(&mut commands, "06_paused");
            smoke.mark = body.map_or(Vec3::ZERO, |b| b.position);
            smoke.hold(KeyCode::KeyW, 0.5);
            0.7
        }
        12 => {
            if let Some(b) = body {
                let still = b.position == smoke.mark;
                smoke.check("no movement while paused", still, b.position);
            }
            smoke.tap(KeyCode::Escape);
            0.4
        }
        13 => {
            smoke.expect("Esc → resumed", play_state, PlayState::Playing);
            smoke.check("cursor locked again", locked, locked);
            next_app.set(AppState::MainMenu);
            0.8
        }
        14 => {
            smoke.expect("back to main menu", app_state, AppState::MainMenu);
            smoke.check("cursor free in menu", !locked, locked);
            1.0 // let pending screenshots finish writing
        }
        _ => {
            let code = if smoke.failures.is_empty() {
                info!("SMOKE PASSED");
                exit.write(AppExit::Success);
                0
            } else {
                error!("SMOKE FAILED: {:#?}", smoke.failures);
                exit.write(AppExit::error());
                1
            };
            // Shutdown has been seen to hang once (see TODO.md). Never let
            // that turn a finished test into a hung process.
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                eprintln!("SMOKE WARNING: shutdown took over 10 s; forcing exit");
                std::process::exit(code);
            });
            f32::INFINITY
        }
    };
    smoke.step += 1;
    smoke.wait = next_wait;
}
