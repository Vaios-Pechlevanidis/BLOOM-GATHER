//! First-person hands and tools ("viewmodel").
//!
//! Drawn by a second camera on render layer 1, parented to the main camera,
//! so the hand and tools always sit on top of the world (never clipping into
//! trees or walls). The sun lights both layers.
//!
//! The model shown follows the selected harvesting mode:
//! hands → scissors (once owned) → basket → garden rake → flower cart rake.
//! Each has its own action animation, played when a pick actually happens:
//! reach-and-pick, snip, scoop, continuous sweep, and a long pull.
//!
//! The hands are models from `assets/hands` (`scripts/hand_model.py`): an
//! open hand, a loose grip for the scissors and the hand rake (holding the
//! rake's own handle) and a fist round the cart rake's long pole. Without
//! them, and for the tools themselves, the models are procedural.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bloom_core::data::HarvestMode;
use bloom_core::game::GameEvent;

use crate::flowers::imported::{self, Plant};
use crate::game::{AppState, Content, EventsForwarded, GameEventMsg, PlayState, Session, Settings};
use crate::player::Player;
use crate::rendering::meshes::{hash3, lin, mix, MeshBuilder};
use crate::rendering::MainCamera;
use crate::tools::{GroundAim, SelectedMode};

pub struct ViewmodelPlugin;

impl Plugin for ViewmodelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewAnim>()
            .add_systems(PostStartup, spawn_viewmodel)
            .add_systems(
                Update,
                (trigger_actions.after(EventsForwarded), animate).chain(),
            );
    }
}

/// Render layer for first-person models.
pub const VIEW_LAYER: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Model {
    Hand,
    Scissors,
    Basket,
    Rake,
    Cart,
}

impl Model {
    const ALL: [Model; 5] = [
        Model::Hand,
        Model::Scissors,
        Model::Basket,
        Model::Rake,
        Model::Cart,
    ];

    /// Rest pose in view space (camera looks along −Z).
    fn rest(self) -> Transform {
        match self {
            Model::Hand => Transform::from_xyz(0.2, -0.24, -0.42).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                0.35,
                0.25,
                -0.2,
            )),
            Model::Scissors => Transform::from_xyz(0.2, -0.23, -0.42)
                .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.25, 0.2, -0.3)),
            Model::Basket => Transform::from_xyz(-0.3, -0.31, -0.64)
                .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.3, 0.2, 0.05))
                .with_scale(Vec3::splat(0.72)),
            Model::Rake => Transform::from_xyz(0.2, -0.17, -0.42).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                0.25,
                0.1,
                -0.2,
            )),
            Model::Cart => Transform::from_xyz(0.24, -0.22, -0.3).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                0.1,
                0.0,
                -0.05,
            )),
        }
    }
}

#[derive(Component)]
struct ViewRoot;

#[derive(Component)]
struct ToolModel {
    model: Model,
    /// Rest pose in view space.
    rest: Transform,
}

/// Scissor blade: turns about the screw, from `base`, for the snip.
#[derive(Component)]
struct Blade {
    side: f32,
    base: Quat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    Pick,
    Snip,
    Scoop,
    Pull,
}

impl Action {
    fn secs(self) -> f32 {
        match self {
            Action::Pick => 0.38,
            Action::Snip => 0.3,
            Action::Scoop => 0.5,
            Action::Pull => 0.6,
        }
    }
}

#[derive(Resource, Default)]
struct ViewAnim {
    action: Option<(Action, f32)>,
    sway: Vec2,
    bob_phase: f32,
    sweep_phase: f32,
}

fn layer() -> RenderLayers {
    RenderLayers::layer(VIEW_LAYER)
}

fn spawn_viewmodel(
    mut commands: Commands,
    main: Query<Entity, With<MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(main) = main.single() else { return };
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.75,
        reflectance: 0.2,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.entity(main).with_children(|cam| {
        cam.spawn((
            Camera3d::default(),
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            // Same HDR mode as the world camera it draws over.
            bevy::camera::Hdr,
            Projection::Perspective(PerspectiveProjection {
                fov: 62f32.to_radians(),
                near: 0.01,
                far: 10.0,
                ..default()
            }),
            Msaa::Sample4,
            layer(),
        ));
        cam.spawn((ViewRoot, Transform::IDENTITY, Visibility::Hidden))
            .with_children(|root| {
                for model in Model::ALL {
                    let Built { body, blade, rest } = build(model);
                    let mut e = root.spawn((
                        ToolModel { model, rest },
                        rest,
                        Visibility::Hidden,
                        Mesh3d(meshes.add(body)),
                        MeshMaterial3d(material.clone()),
                        layer(),
                        NotShadowCaster,
                        NotShadowReceiver,
                    ));
                    if let Some(Blades { mesh, screw, base }) = blade {
                        let blade = meshes.add(mesh);
                        e.with_children(|m| {
                            for side in [1.0f32, -1.0] {
                                m.spawn((
                                    Blade { side, base },
                                    Transform::from_translation(
                                        screw + base * Vec3::Y * 0.004 * side,
                                    )
                                    .with_rotation(base),
                                    Mesh3d(blade.clone()),
                                    MeshMaterial3d(material.clone()),
                                    layer(),
                                    NotShadowCaster,
                                    NotShadowReceiver,
                                ));
                            }
                        });
                    }
                }
            });
    });
}

/// Start the matching animation when a pick actually happened.
fn trigger_actions(
    mut events: MessageReader<GameEventMsg>,
    selected: Res<SelectedMode>,
    session: Option<Res<Session>>,
    content: Option<Res<Content>>,
    mut anim: ResMut<ViewAnim>,
) {
    let picked = events
        .read()
        .any(|GameEventMsg(e)| matches!(e, GameEvent::FlowerHarvested { .. }));
    let (Some(session), Some(content)) = (session, content) else {
        return;
    };
    if !picked {
        return;
    }
    let scissors = session.harvest_cooldown_secs() < content.harvesting.hand_cooldown_secs;
    let action = match selected.0 {
        HarvestMode::Single if scissors => Some(Action::Snip),
        HarvestMode::Single => Some(Action::Pick),
        HarvestMode::Radius => Some(Action::Scoop),
        HarvestMode::Area => Some(Action::Pull),
        HarvestMode::Sweep => None, // the sweep animates while held
    };
    if let Some(a) = action {
        anim.action = Some((a, 0.0));
    }
}

type GameRefs<'w> = (
    Option<Res<'w, Session>>,
    Option<Res<'w, Content>>,
    Option<Res<'w, Player>>,
);
type BladeQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Blade, &'static mut Transform),
    (Without<ToolModel>, Without<ViewRoot>),
>;

#[allow(clippy::too_many_arguments)]
fn animate(
    time: Res<Time>,
    state: Option<Res<State<AppState>>>,
    selected: Res<SelectedMode>,
    aim: Res<GroundAim>,
    settings: Res<Settings>,
    motion: Res<AccumulatedMouseMotion>,
    play: Option<Res<State<PlayState>>>,
    (session, content, player): GameRefs<'_>,
    mut anim: ResMut<ViewAnim>,
    mut root: Query<&mut Visibility, (With<ViewRoot>, Without<ToolModel>)>,
    mut models: Query<(&ToolModel, &mut Transform, &mut Visibility), Without<ViewRoot>>,
    mut blades: BladeQuery,
) {
    // Hands and tools are hidden while the ending's camera tours the valley.
    let in_game = state.is_some_and(|s| *s.get() == AppState::InGame)
        && !play.as_ref().is_some_and(|p| *p.get() == PlayState::Finale);
    for mut v in &mut root {
        *v = if in_game {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let (Some(session), Some(content)) = (session, content) else {
        return;
    };
    let dt = time.delta_secs();
    let scissors = session.harvest_cooldown_secs() < content.harvesting.hand_cooldown_secs;
    let shown = match selected.0 {
        HarvestMode::Single if scissors => Model::Scissors,
        HarvestMode::Single => Model::Hand,
        HarvestMode::Radius => Model::Basket,
        HarvestMode::Sweep => Model::Rake,
        HarvestMode::Area => Model::Cart,
    };

    // Walk bob and a slight lag behind mouse movement.
    let playing = play.is_some_and(|p| *p.get() == PlayState::Playing);
    let speed = player.as_ref().map_or(0.0, |p| p.body.horizontal_speed());
    let grounded = player.as_ref().is_some_and(|p| p.body.grounded);
    anim.bob_phase = (anim.bob_phase + speed * 2.6 * dt) % (PI * 2.0);
    let bob_amount = if settings.head_bob && grounded {
        (speed / 4.2).min(1.3) * 0.012
    } else {
        0.0
    };
    let target_sway = if playing {
        -motion.delta * 0.00035
    } else {
        Vec2::ZERO
    };
    let k = 1.0 - (-10.0 * dt).exp();
    anim.sway = anim.sway.lerp(
        target_sway.clamp(Vec2::splat(-0.035), Vec2::splat(0.035)),
        k,
    );
    if aim.sweeping {
        anim.sweep_phase += dt * 7.0;
    } else {
        anim.sweep_phase = 0.0;
    }

    // One-shot action curve.
    let mut reach = 0.0;
    let mut current = None;
    if let Some((action, t)) = anim.action.as_mut() {
        *t += dt / action.secs();
        if *t >= 1.0 {
            anim.action = None;
        } else {
            reach = (*t * PI).sin();
            current = Some((*action, *t));
        }
    }

    for (model, mut transform, mut visibility) in &mut models {
        *visibility = if model.model == shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if model.model != shown {
            continue;
        }
        let rest = model.rest;
        let bob = Vec3::new(
            anim.bob_phase.sin() * bob_amount,
            -(anim.bob_phase * 2.0).sin().abs() * bob_amount,
            0.0,
        );
        let sway = Vec3::new(anim.sway.x, -anim.sway.y, 0.0);
        let (offset, tilt) = match current.map(|(a, _)| a) {
            Some(Action::Pick) => (
                Vec3::new(-0.06, -0.05, -0.16) * reach,
                Quat::from_rotation_x(-0.6 * reach),
            ),
            Some(Action::Snip) => (
                Vec3::new(-0.04, -0.03, -0.1) * reach,
                Quat::from_rotation_x(-0.35 * reach),
            ),
            Some(Action::Scoop) => (
                Vec3::new(0.05, -0.1, -0.14) * reach,
                Quat::from_rotation_x(0.9 * reach),
            ),
            Some(Action::Pull) => (
                Vec3::new(0.0, 0.03, 0.14) * reach,
                Quat::from_rotation_x(-0.45 * reach),
            ),
            None if model.model == Model::Rake && aim.sweeping => {
                let s = anim.sweep_phase.sin();
                (
                    // Mostly sideways: turning further would show the
                    // open end of the sleeve.
                    Vec3::new(s * 0.06, -0.02, -0.04),
                    Quat::from_rotation_y(s * 0.2) * Quat::from_rotation_x(-0.15),
                )
            }
            None => (Vec3::ZERO, Quat::IDENTITY),
        };
        let goal = Transform::from_translation(rest.translation + bob + sway + offset)
            .with_rotation(rest.rotation * tilt * Quat::from_rotation_z(anim.sway.x * 2.0))
            .with_scale(rest.scale);
        // Ease towards the goal so poses blend smoothly.
        let ease = 1.0 - (-22.0 * dt).exp();
        transform.translation = transform.translation.lerp(goal.translation, ease);
        transform.rotation = transform.rotation.slerp(goal.rotation, ease);
        transform.scale = goal.scale;
    }

    // Scissor blades: open, snapping shut mid-snip.
    let open = match current {
        Some((Action::Snip, t)) => 0.32 * ((t - 0.45).abs() * 2.2).min(1.0),
        _ => 0.32,
    };
    for (blade, mut transform) in &mut blades {
        transform.rotation = blade.base * Quat::from_rotation_y(open * blade.side * 0.5);
    }
}

// ---------------------------------------------------------------------------
// Models (view space, metres; −Z points away from the player)
// ---------------------------------------------------------------------------

fn cyl(b: &mut MeshBuilder, from: Vec3, to: Vec3, r: f32, paint: impl Fn(Vec3) -> LinearRgba) {
    let d = to - from;
    let t = Transform::from_translation((from + to) / 2.0)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize()))
        .with_scale(Vec3::new(r, d.length(), r));
    b.add(
        &Mesh::from(Cylinder::new(1.0, 1.0).mesh().resolution(10)),
        t,
        paint,
    );
}

fn blob(b: &mut MeshBuilder, at: Vec3, scale: Vec3, rot: Quat, paint: impl Fn(Vec3) -> LinearRgba) {
    let t = Transform::from_translation(at)
        .with_rotation(rot)
        .with_scale(scale);
    b.add(
        &Sphere::new(1.0).mesh().ico(2).expect("valid icosphere"),
        t,
        paint,
    );
}

/// A relaxed hand with a knitted sleeve, palm down, fingers towards −Z.
fn hand(b: &mut MeshBuilder, curl: f32) {
    let skin = lin(Color::srgb_u8(234, 192, 162));
    let skin_shade = lin(Color::srgb_u8(208, 160, 132));
    let knit = lin(Color::srgb_u8(232, 222, 198));
    let cuff = lin(Color::srgb_u8(156, 178, 138));
    let skin_paint = move |p: Vec3| mix(skin_shade, skin, p.y * 0.5 + 0.5);
    // Forearm in a knitted sleeve, running back and down out of view (so
    // tilting a tool never swings it into the camera).
    cyl(
        b,
        Vec3::new(0.0, -0.01, 0.09),
        Vec3::new(0.0, -0.17, 0.3),
        0.048,
        move |p| mix(knit * 0.9, knit, (p.y * 40.0).sin() * 0.5 + 0.5),
    );
    cyl(
        b,
        Vec3::new(0.0, 0.0, 0.07),
        Vec3::new(0.0, -0.02, 0.105),
        0.052,
        move |_| cuff,
    );
    // Wrist and palm.
    cyl(
        b,
        Vec3::new(0.0, 0.0, 0.03),
        Vec3::new(0.0, 0.0, 0.08),
        0.03,
        skin_paint,
    );
    blob(
        b,
        Vec3::ZERO,
        Vec3::new(0.046, 0.02, 0.052),
        Quat::IDENTITY,
        skin_paint,
    );
    // Fingers, gently curled.
    for (i, x) in [-0.03f32, -0.01, 0.01, 0.029].into_iter().enumerate() {
        let len = [0.042, 0.05, 0.048, 0.038][i];
        let base = Vec3::new(x, 0.0, -0.045);
        let mid = base + Vec3::new(0.0, -len * 0.3 * curl, -len * 0.6);
        let tip = mid + Vec3::new(0.0, -len * 0.5 * curl, -len * 0.4 * (1.0 - curl * 0.5));
        cyl(b, base, mid, 0.0105, skin_paint);
        cyl(b, mid, tip, 0.0095, skin_paint);
        blob(b, tip, Vec3::splat(0.0095), Quat::IDENTITY, skin_paint);
    }
    // Thumb.
    let t0 = Vec3::new(-0.042, -0.004, -0.005);
    let t1 = t0 + Vec3::new(-0.02, -0.008, -0.03);
    cyl(b, t0, t1, 0.012, skin_paint);
    blob(b, t1, Vec3::splat(0.011), Quat::IDENTITY, skin_paint);
}

/// A hand from `assets/hands/<name>.glb`: the hand itself and, if it holds
/// one, the handle (both in view-model space: fingers towards −Z, the back
/// of the hand up, a right hand).
struct HandModel {
    hand: Mesh,
    stick: Option<Stick>,
}

/// A straight handle held in the hand.
struct Stick {
    mesh: Mesh,
    /// The ends of its axis: the one in the hand and the free one.
    held: Vec3,
    free: Vec3,
    radius: f32,
}

fn hand_model(name: &str) -> Option<HandModel> {
    let parts = match imported::parse_named(imported::hand_model(name)?) {
        Ok(parts) => parts,
        Err(e) => {
            warn!("Hand model {name} unreadable: {e}");
            return None;
        }
    };
    let mut hand = None;
    let mut stick = None;
    for (part, mut plant) in parts {
        match part.as_str() {
            "hand" => {
                extend_sleeve(&mut plant);
                hand = Some(plant.into_mesh());
            }
            "stick" => stick = Some(Stick::measure(plant)),
            _ => {}
        }
    }
    Some(HandModel { hand: hand?, stick })
}

/// How far the forearm runs on past the end of the modelled sleeve (metres).
const FOREARM: f32 = 0.5;

/// The hand models end a hand's length behind the wrist, in an open sleeve
/// end that shows whenever the hand reaches forward. Carry the sleeve on as
/// a knitted forearm that runs back out of view: an elliptical tube that
/// starts inside the modelled sleeve (no seam), follows its axis and widens
/// slightly towards the elbow, in the sleeve's own colour.
fn extend_sleeve(plant: &mut Plant) {
    let p = &plant.positions;
    if p.is_empty() {
        return;
    }
    let back = p.iter().fold(f32::MIN, |m, q| m.max(q.z));
    let slice = |from: f32, to: f32| -> Vec<usize> {
        (0..p.len())
            .filter(|&i| (back - from..=back - to).contains(&p[i].z))
            .collect()
    };
    let centre = |idx: &[usize]| idx.iter().map(|&i| p[i]).sum::<Vec3>() / idx.len().max(1) as f32;
    // Inside the sleeve, a little before its end, and right at the end.
    let inner = slice(0.06, 0.025);
    let end = slice(0.02, 0.0);
    if inner.len() < 8 || end.is_empty() {
        return;
    }
    let start = centre(&inner);
    let axis = ((centre(&end) - start).normalize_or(Vec3::Z) + Vec3::Z).normalize();
    // Frame round the axis: `side` stays level, `up` towards the back of
    // the hand.
    let side = Vec3::Y.cross(axis).normalize_or(Vec3::X);
    let up = axis.cross(side);
    let (mut rx, mut ry) = (0.0f32, 0.0f32);
    for &i in &inner {
        let d = p[i] - start;
        rx = rx.max(d.dot(side).abs());
        ry = ry.max(d.dot(up).abs());
    }
    // The knit's light ribs: the brighter half of the sleeve's colours (the
    // darker ones are the baked grooves between the ribs).
    let mut colours: Vec<Vec3> = inner
        .iter()
        .map(|&i| {
            let c = plant.colors[i];
            Vec3::new(c[0], c[1], c[2])
        })
        .collect();
    colours.sort_by(|a, b| a.element_sum().total_cmp(&b.element_sum()));
    let light = &colours[colours.len() / 2..];
    let colour = light.iter().sum::<Vec3>() / light.len() as f32;
    // Just inside the sleeve where it starts, so it never pokes through.
    let (rx, ry) = (rx * 0.95, ry * 0.95);

    const AROUND: usize = 28;
    const ALONG: usize = 8;
    let first = plant.positions.len() as u32;
    for j in 0..=ALONG {
        let t = j as f32 / ALONG as f32;
        let at = start + axis * (t * (FOREARM + (back - start.z)));
        let grow = 1.0 + 0.15 * t;
        for k in 0..AROUND {
            let a = k as f32 / AROUND as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            let offset = side * (c * rx * grow) + up * (s * ry * grow);
            plant.positions.push(at + offset);
            // Normal of the ellipse at this point.
            let n = (side * (c / rx) + up * (s / ry)).normalize_or(up);
            plant.normals.push(n);
            // Knitted ribs along the arm.
            let rib = 0.86 + 0.14 * (a * 9.0).cos().abs();
            let c = colour * rib;
            plant.colors.push([c.x, c.y, c.z, 1.0]);
        }
    }
    for j in 0..ALONG as u32 {
        for k in 0..AROUND as u32 {
            let k1 = (k + 1) % AROUND as u32;
            let a = first + j * AROUND as u32;
            let b = a + AROUND as u32;
            plant
                .indices
                .extend_from_slice(&[a + k, b + k, a + k1, a + k1, b + k, b + k1]);
        }
    }
}

impl Stick {
    /// Find the axis of a roughly cylindrical handle.
    fn measure(plant: Plant) -> Stick {
        let p = &plant.positions;
        let n = p.len().max(1) as f32;
        let centre = p.iter().sum::<Vec3>() / n;
        let farthest = |from: Vec3| {
            p.iter()
                .copied()
                .max_by(|a, b| {
                    a.distance_squared(from)
                        .total_cmp(&b.distance_squared(from))
                })
                .unwrap_or(from)
        };
        let a = farthest(centre);
        let dir = (a - farthest(a)).normalize_or(Vec3::NEG_Z);
        let along = |q: Vec3| (q - centre).dot(dir);
        let (low, high) = p.iter().fold((f32::MAX, f32::MIN), |(l, h), q| {
            (l.min(along(*q)), h.max(along(*q)))
        });
        // Each end: the middle of the points around its cap.
        let end = |t: f32| {
            let cap: Vec<Vec3> = p
                .iter()
                .copied()
                .filter(|q| (along(*q) - t).abs() < (high - low) * 0.03)
                .collect();
            cap.iter().sum::<Vec3>() / cap.len().max(1) as f32
        };
        let (a, b) = (end(low), end(high));
        // The hand is round the origin.
        let (held, free) = if a.length() < b.length() {
            (a, b)
        } else {
            (b, a)
        };
        let axis = (free - held).normalize_or(Vec3::NEG_Z);
        let radius = p
            .iter()
            .map(|q| (*q - held).reject_from_normalized(axis).length())
            .sum::<f32>()
            / n;
        Stick {
            mesh: plant.into_mesh(),
            held,
            free,
            radius,
        }
    }
}

/// The rotation turning +X towards `x` and +Z towards `z` (made square to `x`).
fn frame(x: Vec3, z: Vec3) -> Quat {
    let x = x.normalize();
    let z = z.reject_from_normalized(x).normalize();
    Quat::from_mat3(&Mat3::from_cols(x, z.cross(x), z))
}

/// Both scissor blades share one mesh, turning about the screw.
struct Blades {
    mesh: Mesh,
    screw: Vec3,
    /// The blades point along this rotation's −Z.
    base: Quat,
}

struct Built {
    body: Mesh,
    blade: Option<Blades>,
    rest: Transform,
}

fn build(model: Model) -> Built {
    let mut b = MeshBuilder::default();
    let wood = lin(Color::srgb_u8(150, 112, 80));
    let wood_light = lin(Color::srgb_u8(190, 150, 110));
    let metal = lin(Color::srgb_u8(170, 176, 182));
    let metal_dark = lin(Color::srgb_u8(118, 124, 132));
    let sage = lin(Color::srgb_u8(132, 160, 116));
    let wood_paint = move |p: Vec3| mix(wood, wood_light, hash3(p * 30.0) * 0.25 + 0.5);
    let mut blade = None;
    let mut rest = model.rest();
    match model {
        Model::Hand => match hand_model("open") {
            Some(h) => {
                b.add_coloured(&h.hand, Transform::IDENTITY);
                rest = Transform::from_xyz(0.18, -0.18, -0.36).with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    0.45,
                    0.2,
                    -0.35,
                ));
            }
            None => hand(&mut b, 0.6),
        },
        Model::Scissors => {
            // Where the scissors sit in the grip and which way they point.
            let (screw, base) = match hand_model("grip") {
                Some(HandModel {
                    hand,
                    stick: Some(stick),
                }) => {
                    b.add_coloured(&hand, Transform::IDENTITY);
                    rest = Transform::from_xyz(0.19, -0.17, -0.38)
                        .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.4, 0.0, -0.3))
                        .with_scale(Vec3::splat(0.9));
                    let axis = (stick.free - stick.held).normalize();
                    // Sage handles through the fist, to the screw just past it.
                    let screw = stick.held + axis * 0.1;
                    let side = axis.cross(Vec3::Y).normalize_or(Vec3::X) * 0.006;
                    for s in [side, -side] {
                        cyl(&mut b, stick.held + s, screw + s, 0.0065, move |_| sage);
                    }
                    let t = Transform::from_translation(screw - axis * 0.012).with_rotation(
                        Quat::from_rotation_arc(Vec3::Y, axis.cross(Vec3::Y).normalize_or(Vec3::X)),
                    );
                    b.add(&Mesh::from(Torus::new(0.006, 0.013)), t, move |_| sage);
                    blob(
                        &mut b,
                        screw,
                        Vec3::splat(0.005),
                        Quat::IDENTITY,
                        move |_| metal_dark,
                    );
                    (screw, frame(axis.cross(Vec3::Y), -axis))
                }
                _ => {
                    hand(&mut b, 0.95);
                    // Sage handle loops in the grip.
                    for x in [-0.014f32, 0.014] {
                        let t = Transform::from_xyz(x, 0.012, -0.03)
                            .with_rotation(Quat::from_rotation_x(FRAC_PI_2));
                        b.add(&Mesh::from(Torus::new(0.008, 0.016)), t, move |_| sage);
                    }
                    (Vec3::new(0.0, 0.0, -0.075), Quat::IDENTITY)
                }
            };
            let mut bl = MeshBuilder::default();
            let t =
                Transform::from_xyz(0.0, 0.0, -0.045).with_scale(Vec3::new(0.009, 0.003, 0.055));
            bl.add(
                &Sphere::new(1.0).mesh().ico(2).expect("valid icosphere"),
                t,
                move |p| mix(metal_dark, metal, p.x * 0.5 + 0.5),
            );
            blade = Some(Blades {
                mesh: bl.build(),
                screw,
                base,
            });
        }
        Model::Basket => {
            let wicker = lin(Color::srgb_u8(196, 156, 104));
            let wicker_dark = lin(Color::srgb_u8(150, 112, 70));
            let weave = move |p: Vec3| {
                let a = p.z.atan2(p.x);
                let w = ((a * 10.0).sin() * (p.y * 18.0).sin()).signum() * 0.5 + 0.5;
                mix(wicker_dark, wicker, w * 0.7 + 0.15)
            };
            let body = Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(0.15, 0.11, 0.15));
            b.add(
                &Mesh::from(Cylinder::new(1.0, 1.0).mesh().resolution(20)),
                body,
                weave,
            );
            let rim = Transform::from_xyz(0.0, 0.055, 0.0);
            b.add(&Mesh::from(Torus::new(0.14, 0.16)), rim, move |_| {
                wicker_dark
            });
            let handle =
                Transform::from_xyz(0.0, 0.06, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2));
            b.add(&Mesh::from(Torus::new(0.13, 0.142)), handle, move |_| {
                wicker
            });
            // A few flowers peeking out.
            for (i, c) in [
                (250, 247, 238),
                (226, 120, 104),
                (246, 224, 128),
                (128, 150, 216),
            ]
            .into_iter()
            .enumerate()
            {
                let col = lin(Color::srgb_u8(c.0, c.1, c.2));
                let a = i as f32 * 1.6;
                blob(
                    &mut b,
                    Vec3::new(a.cos() * 0.07, 0.07, a.sin() * 0.07),
                    Vec3::new(0.025, 0.012, 0.025),
                    Quat::IDENTITY,
                    move |_| col,
                );
            }
        }
        Model::Rake => {
            // Hand rake: short handle in the fist, small fan of tines ahead.
            let (end, axis) = match hand_model("grip") {
                Some(HandModel {
                    hand,
                    stick: Some(stick),
                }) => {
                    b.add_coloured(&hand, Transform::IDENTITY);
                    b.add_coloured(&stick.mesh, Transform::IDENTITY);
                    // Nearly level: tilting the handle down would swing
                    // the sleeve up into view, so the tines bend down.
                    rest = Transform::from_xyz(0.21, -0.2, -0.42)
                        .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.5, 0.15, -0.3))
                        .with_scale(Vec3::splat(0.9));
                    (stick.free, (stick.free - stick.held).normalize())
                }
                _ => {
                    hand(&mut b, 1.0);
                    let from = Vec3::new(0.0, 0.0, 0.05);
                    let end = Vec3::new(0.0, -0.08, -0.22);
                    cyl(&mut b, from, end, 0.012, wood_paint);
                    (end, (end - from).normalize())
                }
            };
            // Across the handle, level; the tines curl down and forward.
            let across = axis.cross(Vec3::Y).normalize_or(Vec3::X);
            let side = across.cross(axis).normalize();
            let down = if side.y > 0.0 { -side } else { side };
            cyl(
                &mut b,
                end - across * 0.06,
                end + across * 0.06,
                0.006,
                move |_| metal_dark,
            );
            for k in 0..7 {
                let at = end + across * (-0.06 + k as f32 * 0.02);
                let spread = across * (k as f32 - 3.0) * 0.004;
                let bend = at + down * 0.02 + axis * 0.03 + spread;
                cyl(&mut b, at, bend, 0.0035, move |_| metal);
                let tip = bend + down * 0.045 + axis * 0.005;
                cyl(&mut b, bend, tip, 0.0035, move |_| metal);
            }
        }
        Model::Cart => match hand_model("fist") {
            Some(HandModel {
                hand,
                stick: Some(stick),
            }) => {
                // A fist round the long, wide rake's pole, the arm coming in
                // from the right and the pole reaching well out ahead.
                b.add_coloured(&hand, Transform::IDENTITY);
                let scale = 0.65;
                // The pole crosses the fist (along +X with the back of the
                // hand up): turn it ahead, the forearm (+Z) to the right.
                let pole = (stick.free - stick.held).normalize();
                let forward = if pole.x >= 0.0 { pole } else { -pole };
                let mid = (stick.held + stick.free) / 2.0;
                let rotation = frame(Vec3::new(-0.1, -0.25, -1.0), Vec3::new(1.0, -0.6, 0.05))
                    * frame(forward, Vec3::Z).inverse();
                rest = Transform::from_xyz(0.19, -0.19, -0.36)
                    .with_rotation(rotation)
                    .with_scale(Vec3::splat(scale));
                // In metres of view space, through the fist.
                let to_model = |v: Vec3| rotation.inverse() * v / scale;
                let handle_end = mid + forward * (1.3 / scale);
                cyl(
                    &mut b,
                    // Behind the fist, out of the frame.
                    mid - forward * (0.3 / scale),
                    handle_end,
                    stick.radius,
                    wood_paint,
                );
                let level = to_model((rotation * forward).cross(Vec3::Y).normalize() * 0.28);
                let down = to_model(Vec3::new(0.0, -0.07, -0.03));
                let (left, right) = (handle_end - level, handle_end + level);
                cyl(&mut b, left, right, 0.012 / scale, wood_paint);
                for k in 0..12 {
                    let at = left.lerp(right, k as f32 / 11.0);
                    cyl(&mut b, at, at + down, 0.005 / scale, move |_| metal);
                }
            }
            _ => {
                // The cart's long, wide rake, held low to the right and
                // reaching well out ahead (the grip is below the frame).
                let handle_end = Vec3::new(-0.12, -0.3, -1.25);
                cyl(
                    &mut b,
                    Vec3::new(0.02, -0.02, 0.1),
                    handle_end,
                    0.016,
                    wood_paint,
                );
                let left = handle_end + Vec3::new(-0.28, 0.0, 0.02);
                let right = handle_end + Vec3::new(0.28, 0.0, -0.02);
                cyl(&mut b, left, right, 0.012, wood_paint);
                for k in 0..12 {
                    let at = left.lerp(right, k as f32 / 11.0);
                    cyl(
                        &mut b,
                        at,
                        at + Vec3::new(0.0, -0.07, -0.03),
                        0.005,
                        move |_| metal,
                    );
                }
            }
        },
    }
    Built {
        body: b.build(),
        blade,
        rest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_hands_load_with_their_handles() {
        for (name, holds) in [("open", false), ("grip", true), ("fist", true)] {
            let model = hand_model(name).unwrap_or_else(|| panic!("assets/hands/{name}.glb"));
            assert_eq!(model.stick.is_some(), holds, "{name}");
            if let Some(stick) = model.stick {
                let length = stick.held.distance(stick.free);
                assert!((0.1..0.6).contains(&length), "{name}: {length}");
                assert!((0.005..0.05).contains(&stick.radius), "{name}");
                // The held end is the one nearer the hand.
                assert!(stick.held.length() < stick.free.length(), "{name}");
            }
        }
    }

    #[test]
    fn the_sleeve_runs_on_out_of_view() {
        for name in ["open", "grip", "fist"] {
            let model = hand_model(name).unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
                model.hand.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("{name}: positions");
            };
            // The model ends 0.15 m behind its middle; the forearm carries on.
            let back = p.iter().fold(f32::MIN, |m, q| m.max(q[2]));
            assert!(back > 0.15 + FOREARM * 0.8, "{name}: {back}");
            // Still a sleeve, not a sail: no wider than a forearm round its
            // own axis (which may lean a little to follow the sleeve).
            let ring: Vec<Vec3> = p
                .iter()
                .map(|q| Vec3::from(*q))
                .filter(|q| (q.z - 0.4).abs() < 0.04)
                .collect();
            let centre = ring.iter().sum::<Vec3>() / ring.len() as f32;
            let wide = ring
                .iter()
                .fold(0.0f32, |m, q| m.max((*q - centre).truncate().length()));
            assert!((0.02..0.12).contains(&wide), "{name}: {wide}");
        }
    }
}
