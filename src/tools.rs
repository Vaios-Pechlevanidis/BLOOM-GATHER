//! Tool selection and the batch harvesting modes in first person.
//!
//! * Keys 1–4 (or the mouse wheel) pick the mode: hands/scissors, basket,
//!   garden rake (sweep) or flower cart (area). Only modes of owned tools.
//! * Batch modes aim at the ground in front of the player (terrain raycast
//!   within ground reach). A ring of markers shows the basket/rake circle or
//!   the cart's rectangle, and every flower it would gather is highlighted.
//! * E: basket gathers once; rake gathers while held; cart marks a corner on
//!   press and gathers the rectangle on release.
//!
//! All rules (reach, caps, cooldowns) are enforced by `bloom_core`.
//!
//! Aiming reads the camera's `Transform` (it has no parent, so this is its
//! world transform), which `player` has already updated this frame;
//! `GlobalTransform` would lag a frame behind.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bloom_core::data::HarvestMode;
use bloom_core::game::GameError;

use crate::audio::{PlaySfx, Sfx};
use crate::game::{AppState, EventsForwarded, PlayState, RegionScene, Session};
use crate::input::Bindings;
use crate::interaction::LookTarget;
use crate::player::CameraUpdated;
use crate::rendering::MainCamera;
use crate::ui::Toast;

pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedMode>()
            .init_resource::<GroundAim>()
            .add_systems(OnEnter(AppState::InGame), spawn_markers)
            .add_systems(OnExit(PlayState::Playing), reset_aim)
            .add_systems(
                Update,
                (
                    (keep_mode_valid, select_mode, aim, act)
                        .chain()
                        .after(CameraUpdated)
                        .before(EventsForwarded)
                        .run_if(in_state(PlayState::Playing)),
                    update_markers.run_if(in_state(AppState::InGame)),
                ),
            );
    }
}

/// The harvesting mode in hand.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedMode(pub HarvestMode);

impl Default for SelectedMode {
    fn default() -> Self {
        Self(HarvestMode::Single)
    }
}

/// Mode order on the tool bar (keys 1–4).
pub const MODE_ORDER: [HarvestMode; 4] = [
    HarvestMode::Single,
    HarvestMode::Radius,
    HarvestMode::Sweep,
    HarvestMode::Area,
];

/// Display name of a mode's tool.
pub fn mode_name(mode: HarvestMode, has_scissors: bool) -> &'static str {
    match mode {
        HarvestMode::Single if has_scissors => "Scissors",
        HarvestMode::Single => "Hands",
        HarvestMode::Radius => "Basket",
        HarvestMode::Sweep => "Garden rake",
        HarvestMode::Area => "Flower cart",
    }
}

/// Where a batch tool is aimed on the ground.
#[derive(Resource, Debug, Default, Clone)]
pub struct GroundAim {
    pub point: Option<Vec3>,
    /// First corner while the cart's area is being marked.
    pub area_start: Option<Vec2>,
    /// Last valid second corner while marking.
    pub area_end: Option<Vec2>,
    /// The marked rectangle (clamped to the cart's size).
    pub area: Option<(Vec2, Vec2)>,
    /// The rake is sweeping (E held).
    pub sweeping: bool,
}

#[derive(Component)]
struct Marker(usize);

const MARKERS: usize = 80;

fn reset_aim(mut aim: ResMut<GroundAim>) {
    *aim = GroundAim::default();
}

fn keep_mode_valid(session: Option<Res<Session>>, mut selected: ResMut<SelectedMode>) {
    if let Some(session) = session {
        if !session.available_modes().contains(&selected.0) {
            selected.0 = HarvestMode::Single;
        }
    }
}

fn select_mode(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    scroll: Res<AccumulatedMouseScroll>,
    session: Option<Res<Session>>,
    mut selected: ResMut<SelectedMode>,
    mut aim: ResMut<GroundAim>,
    mut sfx: EventWriter<PlaySfx>,
    mut toasts: EventWriter<Toast>,
) {
    let Some(session) = session else { return };
    let available = session.available_modes();
    let mut wanted = None;
    for (slot, key) in bindings.tool_slots.iter().enumerate() {
        if keys.just_pressed(*key) {
            wanted = Some(MODE_ORDER[slot]);
        }
    }
    if scroll.delta.y.abs() > 0.0 && available.len() > 1 {
        let i = available.iter().position(|m| *m == selected.0).unwrap_or(0) as i32;
        let step = if scroll.delta.y > 0.0 { -1 } else { 1 };
        wanted = Some(available[(i + step).rem_euclid(available.len() as i32) as usize]);
    }
    let Some(mode) = wanted else { return };
    if mode == selected.0 {
        return;
    }
    if available.contains(&mode) {
        selected.0 = mode;
        *aim = GroundAim::default();
        sfx.write(PlaySfx::new(Sfx::Click));
    } else {
        sfx.write(PlaySfx::new(Sfx::Deny));
        toasts.write(Toast::info(format!(
            "You don't have the {} yet",
            mode_name(mode, false).to_lowercase()
        )));
    }
}

fn aim(
    cameras: Query<&Transform, With<MainCamera>>,
    scene: Option<Res<RegionScene>>,
    session: Option<ResMut<Session>>,
    selected: Res<SelectedMode>,
    mut aim: ResMut<GroundAim>,
    mut target: ResMut<LookTarget>,
) {
    let (Some(scene), Some(mut session), Ok(camera)) = (scene, session, cameras.single()) else {
        return;
    };
    if selected.0 == HarvestMode::Single {
        if aim.point.is_some() || !target.group.is_empty() {
            *aim = GroundAim::default();
            target.group.clear();
        }
        return;
    }
    let eye = camera.translation;
    let reach = session.ground_reach();
    aim.point = scene
        .layout
        .terrain
        .raycast(eye, camera.forward().as_vec3(), reach);
    let group = match (selected.0, aim.point) {
        (HarvestMode::Area, point) => {
            if let Some(p) = point {
                aim.area_end = Some(Vec2::new(p.x, p.z));
            }
            // While marking, keep the last valid corner if the crosshair
            // briefly leaves the ground in reach.
            match (aim.area_start, aim.area_end) {
                (Some(start), Some(end)) => match session.preview_area(scene.region, start, end) {
                    Some((min, max, hits)) => {
                        aim.area = Some((min, max));
                        hits
                    }
                    None => Vec::new(),
                },
                _ => {
                    aim.area = None;
                    Vec::new()
                }
            }
        }
        (mode, Some(p)) => session.preview_circle(scene.region, mode, Vec2::new(p.x, p.z)),
        (_, None) => Vec::new(),
    };
    if target.group != group {
        target.group = group;
    }
}

fn act(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    cameras: Query<&Transform, With<MainCamera>>,
    scene: Option<Res<RegionScene>>,
    session: Option<ResMut<Session>>,
    selected: Res<SelectedMode>,
    target: Res<LookTarget>,
    mut aim: ResMut<GroundAim>,
    mut sfx: EventWriter<PlaySfx>,
    mut toasts: EventWriter<Toast>,
) {
    let (Some(scene), Some(mut session), Ok(camera)) = (scene, session, cameras.single()) else {
        return;
    };
    let eye = camera.translation;
    // A workshop station in the crosshair takes E instead.
    if target.station.is_some() && aim.area_start.is_none() {
        aim.sweeping = false;
        return;
    }
    let pressed = Bindings::pressed(&keys, &bindings.interact);
    let just = Bindings::just_pressed(&keys, &bindings.interact);
    let mut feedback =
        |result: Result<bloom_core::BatchOutcome, GameError>, quiet_empty: bool| match result {
            Ok(_) | Err(GameError::CoolingDown { .. }) => {}
            Err(GameError::NothingInReach) if quiet_empty => {}
            Err(GameError::NothingInReach) => {
                sfx.write(PlaySfx::new(Sfx::Deny));
            }
            Err(err) => {
                sfx.write(PlaySfx::new(Sfx::Deny));
                toasts.write(Toast::info(capitalise(&err.to_string())));
            }
        };

    aim.sweeping = selected.0 == HarvestMode::Sweep && pressed;
    match selected.0 {
        HarvestMode::Single => {}
        HarvestMode::Radius => {
            if just {
                match aim.point {
                    Some(p) => feedback(session.harvest_radius(scene.region, p, eye), false),
                    None => feedback(Err(GameError::NothingInReach), false),
                }
            }
        }
        HarvestMode::Sweep => {
            if let (true, Some(p)) = (pressed, aim.point) {
                // Called every frame while held; the core paces the gathers.
                feedback(session.harvest_sweep(scene.region, p, eye), true);
            }
        }
        HarvestMode::Area => {
            if just {
                aim.area_start = aim.point.map(|p| Vec2::new(p.x, p.z));
                aim.area_end = aim.area_start;
            } else if !pressed {
                if let Some(start) = aim.area_start.take() {
                    let end = aim.area_end.take().unwrap_or(start);
                    feedback(session.harvest_area(scene.region, start, end, eye), false);
                    aim.area = None;
                }
            }
        }
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn spawn_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).expect("valid icosphere"));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.99, 0.96, 0.86, 0.85),
        emissive: LinearRgba::new(0.5, 0.45, 0.3, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    for i in 0..MARKERS {
        commands.spawn((
            StateScoped(AppState::InGame),
            Marker(i),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(Vec3::new(0.042, 0.018, 0.042)),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }
}

/// Dots on the ground (following the terrain) outlining the basket/rake
/// circle or the cart's rectangle.
fn update_markers(
    time: Res<Time<Real>>,
    aim: Res<GroundAim>,
    selected: Res<SelectedMode>,
    state: Res<State<PlayState>>,
    scene: Option<Res<RegionScene>>,
    session: Option<Res<Session>>,
    mut markers: Query<(&Marker, &mut Transform, &mut Visibility)>,
) {
    let (Some(scene), Some(session)) = (scene, session) else {
        return;
    };
    let mut points: Vec<Vec2> = Vec::new();
    if *state.get() == PlayState::Playing {
        match (selected.0, aim.point) {
            (HarvestMode::Radius | HarvestMode::Sweep, Some(p)) => {
                if let Some(params) = session.mode_params(selected.0) {
                    let n = 28;
                    let spin = time.elapsed_secs() * 0.3;
                    for k in 0..n {
                        let a = k as f32 / n as f32 * std::f32::consts::TAU + spin;
                        points.push(
                            Vec2::new(p.x, p.z) + Vec2::new(a.cos(), a.sin()) * params.radius,
                        );
                    }
                }
            }
            (HarvestMode::Area, Some(p)) => {
                if let Some((min, max)) = aim.area {
                    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
                    let perimeter = 2.0 * ((max - min).x + (max - min).y);
                    let spacing = (perimeter / MARKERS as f32).max(0.35);
                    for i in 0..4 {
                        let (a, b) = (corners[i], corners[(i + 1) % 4]);
                        let steps = ((b - a).length() / spacing).ceil().max(1.0) as usize;
                        for s in 0..steps {
                            points.push(a.lerp(b, s as f32 / steps as f32));
                        }
                    }
                } else {
                    // Not marking yet: a small cross shows where a corner would go.
                    for d in [
                        Vec2::ZERO,
                        Vec2::X * 0.3,
                        -Vec2::X * 0.3,
                        Vec2::Y * 0.3,
                        -Vec2::Y * 0.3,
                    ] {
                        points.push(Vec2::new(p.x, p.z) + d);
                    }
                }
            }
            _ => {}
        }
    }
    let terrain = &scene.layout.terrain;
    for (marker, mut transform, mut visibility) in &mut markers {
        match points.get(marker.0) {
            Some(&p) => {
                transform.translation = Vec3::new(p.x, terrain.height_at(p) + 0.05, p.y);
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}
