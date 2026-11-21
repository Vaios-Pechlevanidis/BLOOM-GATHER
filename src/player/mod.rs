//! The first-person player: input → `bloom_core` character controller →
//! camera.
//!
//! * Movement runs in `FixedUpdate` (64 Hz) through the engine-independent
//!   controller, so it is frame-rate independent and identical to what the
//!   core tests verify.
//! * The camera is updated every frame, interpolating between the last two
//!   fixed steps for smooth motion at any refresh rate.
//! * Mouse look is applied every frame for minimum latency.
//! * The cursor is locked while playing and released in menus.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow, WindowFocused};
use bloom_core::player::{wish_direction, CharacterBody, Environment, MoveInput};

use crate::game::{AppState, Content, PlayState, RegionScene, Settings};
use crate::input::Bindings;
use crate::rendering::{MainCamera, SceneSet};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingInput>()
            .add_systems(
                OnEnter(AppState::InGame),
                (ensure_player, begin_camera_blend).chain().after(SceneSet),
            )
            .add_systems(OnEnter(PlayState::Playing), grab_cursor)
            .add_systems(OnExit(PlayState::Playing), release_cursor)
            .add_systems(
                FixedUpdate,
                step_player.run_if(in_state(PlayState::Playing)),
            )
            .add_systems(
                Update,
                (
                    (gather_input, mouse_look).run_if(in_state(PlayState::Playing)),
                    pause_on_focus_loss.run_if(in_state(PlayState::Playing)),
                    update_camera.run_if(in_state(AppState::InGame)),
                )
                    .chain(),
            );
    }
}

/// The one player character. Survives returning to the title screen, so
/// "Begin" continues where you were.
#[derive(Resource, Debug, Clone)]
pub struct Player {
    pub body: CharacterBody,
    /// Feet position before the most recent fixed step (for interpolation).
    pub previous: Vec3,
    /// Radians about +Y; 0 faces −Z.
    pub yaw: f32,
    /// Radians; positive looks up.
    pub pitch: f32,
    bob_phase: f32,
    bob_weight: f32,
    landing_dip: f32,
}

/// Input gathered each frame, consumed by the next fixed step.
#[derive(Resource, Debug, Default)]
struct PendingInput {
    local: Vec2,
    sprint: bool,
    /// Latched until a fixed step consumes it, so short taps are never lost.
    jump: bool,
}

/// Eases the camera from wherever it was (the menu orbit) into the eyes.
#[derive(Resource)]
struct CameraBlend {
    from: Transform,
    t: f32,
}

const MAX_PITCH: f32 = 1.54; // ~88°
const BLEND_SECS: f32 = 1.0;
const BOB_AMPLITUDE: f32 = 0.032;
const BOB_STRIDE: f32 = 2.6; // radians of bob phase per metre walked

fn ensure_player(mut commands: Commands, existing: Option<Res<Player>>, scene: Res<RegionScene>) {
    if existing.is_some() {
        return;
    }
    let layout = &scene.layout;
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let body = CharacterBody::standing_at(layout.spawn, &env);
    commands.insert_resource(Player {
        previous: body.position,
        body,
        yaw: layout.spawn_yaw,
        pitch: -0.08,
        bob_phase: 0.0,
        bob_weight: 0.0,
        landing_dip: 0.0,
    });
}

fn begin_camera_blend(mut commands: Commands, cameras: Query<&Transform, With<MainCamera>>) {
    if let Ok(from) = cameras.single() {
        commands.insert_resource(CameraBlend {
            from: *from,
            t: 0.0,
        });
    }
}

fn set_cursor_locked(window: &mut Window, locked: bool) {
    window.cursor_options.grab_mode = if locked {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    window.cursor_options.visible = !locked;
}

fn grab_cursor(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        set_cursor_locked(&mut window, true);
    }
}

fn release_cursor(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        set_cursor_locked(&mut window, false);
    }
}

fn pause_on_focus_loss(
    mut focus: EventReader<WindowFocused>,
    settings: Res<Settings>,
    mut next: ResMut<NextState<PlayState>>,
) {
    for event in focus.read() {
        if !event.focused && settings.pause_on_focus_loss {
            next.set(PlayState::Paused);
        }
    }
}

fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    mut pending: ResMut<PendingInput>,
) {
    pending.local = bindings.move_axis(&keys);
    pending.sprint = Bindings::pressed(&keys, &bindings.sprint);
    if Bindings::just_pressed(&keys, &bindings.jump) {
        pending.jump = true;
    }
}

fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    settings: Res<Settings>,
    player: Option<ResMut<Player>>,
) {
    let Some(mut player) = player else { return };
    if motion.delta == Vec2::ZERO {
        return;
    }
    let rate = Settings::BASE_LOOK_RATE * settings.mouse_sensitivity;
    let invert = if settings.invert_y { -1.0 } else { 1.0 };
    player.yaw -= motion.delta.x * rate;
    player.pitch = (player.pitch - motion.delta.y * rate * invert).clamp(-MAX_PITCH, MAX_PITCH);
    player.yaw = player.yaw.rem_euclid(std::f32::consts::TAU);
}

fn step_player(
    time: Res<Time>,
    content: Res<Content>,
    scene: Option<Res<RegionScene>>,
    player: Option<ResMut<Player>>,
    mut pending: ResMut<PendingInput>,
) {
    let (Some(scene), Some(mut player)) = (scene, player) else {
        return;
    };
    let layout = &scene.layout;
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let input = MoveInput {
        wish: wish_direction(player.yaw, pending.local),
        sprint: pending.sprint,
        jump: std::mem::take(&mut pending.jump),
    };
    player.previous = player.body.position;
    let events = player
        .body
        .step(&input, time.delta_secs(), &content.player, &env);
    if events.landed && events.landing_speed > 2.5 {
        let dip = ((events.landing_speed - 2.5) * 0.025).min(0.14);
        player.landing_dip = player.landing_dip.max(dip);
    }
}

fn update_camera(
    mut commands: Commands,
    time: Res<Time>,
    real: Res<Time<Real>>,
    fixed: Res<Time<Fixed>>,
    content: Res<Content>,
    settings: Res<Settings>,
    player: Option<ResMut<Player>>,
    blend: Option<ResMut<CameraBlend>>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(mut player) = player else { return };
    let cfg = &content.player;
    let dt = time.delta_secs();

    // Smooth position between the last two physics steps.
    let alpha = fixed.overstep_fraction().clamp(0.0, 1.0);
    let feet = player.previous.lerp(player.body.position, alpha);

    // Head bob: fades in with walking speed while grounded.
    let speed = player.body.horizontal_speed();
    let walking = player.body.grounded && speed > 0.3;
    let target_weight = if walking && settings.head_bob {
        (speed / cfg.walk_speed).min(1.3)
    } else {
        0.0
    };
    player.bob_weight += (target_weight - player.bob_weight) * (1.0 - (-8.0 * dt).exp());
    player.bob_phase = (player.bob_phase + speed * BOB_STRIDE * dt) % std::f32::consts::TAU;
    player.landing_dip *= (-9.0 * dt).exp();

    let rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    let mut eye = feet + Vec3::Y * cfg.eye_height;
    if settings.head_bob {
        let bob = player.bob_weight * BOB_AMPLITUDE;
        eye.y += (player.bob_phase * 2.0).sin().abs() * bob - bob * 0.5 - player.landing_dip;
        eye += rotation * Vec3::X * player.bob_phase.sin() * bob * 0.35;
    }
    let target = Transform::from_translation(eye).with_rotation(rotation);

    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    match blend {
        Some(mut blend) => {
            blend.t += real.delta_secs() / BLEND_SECS;
            let t = blend.t.min(1.0);
            let e = t * t * (3.0 - 2.0 * t);
            camera.translation = blend.from.translation.lerp(target.translation, e);
            camera.rotation = blend.from.rotation.slerp(target.rotation, e);
            if blend.t >= 1.0 {
                commands.remove_resource::<CameraBlend>();
            }
        }
        None => *camera = target,
    }
}
