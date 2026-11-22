//! 3D models for landmarks: a wooden bench, a stone well with a little
//! roof, and a signpost. Placement and collision come from
//! `bloom_core::world::layout`; these are the visuals.
//!
//! Temporary procedural art (see TODO.md), in the same style as the trees.

use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bloom_core::world::LandmarkKind;

use super::meshes::{hash3, lin, mix, MeshBuilder};
use super::scene::SceneEntity;
use crate::game::{Content, RegionScene};

pub struct LandmarksPlugin;

impl Plugin for LandmarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            spawn_landmarks.run_if(resource_exists_and_changed::<RegionScene>),
        );
    }
}

fn spawn_landmarks(
    mut commands: Commands,
    scene: Res<RegionScene>,
    content: Res<Content>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.88,
        reflectance: 0.15,
        ..default()
    });
    let bench = meshes.add(bench());
    let well = meshes.add(well());
    let sign = meshes.add(signpost());
    let cottage = meshes.add(workshop());
    let lantern = meshes.add(lantern_post());
    let night = content.regions[scene.region].scenery.atmosphere.night;
    let glass = meshes.add(Cuboid::new(0.2, 0.3, 0.2));
    let lamp = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.7),
        emissive: if night {
            LinearRgba::rgb(4.0, 2.7, 1.2)
        } else {
            LinearRgba::rgb(0.4, 0.3, 0.15)
        },
        ..default()
    });
    for l in &scene.layout.landmarks {
        let mesh = match l.kind {
            LandmarkKind::Bench => bench.clone(),
            LandmarkKind::Well => well.clone(),
            LandmarkKind::Signpost => sign.clone(),
            LandmarkKind::Workshop => cottage.clone(),
            LandmarkKind::Lantern => lantern.clone(),
        };
        let mut e = commands.spawn((
            SceneEntity,
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(l.position.x, l.base, l.position.y)
                .with_rotation(Quat::from_rotation_y(l.yaw)),
        ));
        if l.kind == LandmarkKind::Lantern {
            e.with_children(|post| {
                post.spawn((
                    Mesh3d(glass.clone()),
                    MeshMaterial3d(lamp.clone()),
                    Transform::from_xyz(0.0, 2.05, 0.32),
                    bevy::pbr::NotShadowCaster,
                ));
                if night {
                    post.spawn((
                        PointLight {
                            color: Color::srgb(1.0, 0.8, 0.52),
                            intensity: 45_000.0,
                            range: 9.0,
                            shadows_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, 2.0, 0.4),
                    ));
                }
            });
        }
        if l.kind == LandmarkKind::Workshop {
            // A warm lamp under the roof so the interior isn't left in shadow.
            e.with_children(|w| {
                w.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.86, 0.66),
                        intensity: 90_000.0,
                        range: 9.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 2.35, -0.6),
                    RenderLayers::from_layers(&[0, crate::viewmodel::VIEW_LAYER]),
                ));
            });
        }
    }
}

/// The florist's cottage: plank floor, plaster walls with a timber frame and
/// windows, a door gap at the front (+Z), a gable roof with a chimney, and
/// the furniture the player uses (positions match the colliders and
/// interactables in `bloom_core::world::layout`).
fn workshop() -> Mesh {
    use bloom_core::world::layout::workshop::{DOOR_HALF_WIDTH, HALF, WALL, WALL_HEIGHT};
    let mut b = MeshBuilder::default();
    let plaster = lin(Color::srgb_u8(242, 232, 212));
    let plaster_shade = lin(Color::srgb_u8(222, 208, 182));
    let timber = lin(Color::srgb_u8(118, 86, 62));
    let roof = lin(Color::srgb_u8(170, 102, 82));
    let roof_dark = lin(Color::srgb_u8(132, 78, 64));
    let plank = move |p: Vec3| {
        mix(
            lin(Color::srgb_u8(150, 112, 80)),
            lin(Color::srgb_u8(186, 146, 106)),
            hash3(Vec3::new(p.x * 0.0, 0.0, (p.z * 3.0).floor())) * 0.5 + 0.5,
        )
    };
    let wall_paint = move |p: Vec3| {
        mix(
            plaster_shade,
            plaster,
            (p.y * 0.5 + 0.5).clamp(0.0, 1.0) + hash3(p * 4.0) * 0.05,
        )
    };
    let none = Quat::IDENTITY;
    let h = WALL_HEIGHT;
    let t = WALL;

    // Floor.
    cuboid(
        &mut b,
        Vec3::new(HALF.x * 2.0, 0.12, HALF.y * 2.0),
        Vec3::new(0.0, 0.02, 0.0),
        none,
        plank,
    );
    // Walls: back, sides, and the front split around the door.
    cuboid(
        &mut b,
        Vec3::new(HALF.x * 2.0 + t, h, t),
        Vec3::new(0.0, h / 2.0, -HALF.y),
        none,
        wall_paint,
    );
    for x in [-HALF.x, HALF.x] {
        cuboid(
            &mut b,
            Vec3::new(t, h, HALF.y * 2.0),
            Vec3::new(x, h / 2.0, 0.0),
            none,
            wall_paint,
        );
    }
    let side = HALF.x - DOOR_HALF_WIDTH;
    for dir in [-1.0f32, 1.0] {
        cuboid(
            &mut b,
            Vec3::new(side, h, t),
            Vec3::new(dir * (DOOR_HALF_WIDTH + side / 2.0), h / 2.0, HALF.y),
            none,
            wall_paint,
        );
    }
    // Lintel above the door.
    cuboid(
        &mut b,
        Vec3::new(DOOR_HALF_WIDTH * 2.0, 0.55, t),
        Vec3::new(0.0, h - 0.27, HALF.y),
        none,
        wall_paint,
    );
    // Timber frame: corner posts, sill and top beams, door frame.
    let tw = move |_: Vec3| timber;
    for (x, z) in [
        (-HALF.x, -HALF.y),
        (HALF.x, -HALF.y),
        (-HALF.x, HALF.y),
        (HALF.x, HALF.y),
    ] {
        cuboid(
            &mut b,
            Vec3::new(0.22, h, 0.22),
            Vec3::new(x, h / 2.0, z),
            none,
            tw,
        );
    }
    for z in [-HALF.y, HALF.y] {
        cuboid(
            &mut b,
            Vec3::new(HALF.x * 2.0 + 0.3, 0.18, 0.3),
            Vec3::new(0.0, h, z),
            none,
            tw,
        );
    }
    for x in [-HALF.x, HALF.x] {
        cuboid(
            &mut b,
            Vec3::new(0.3, 0.18, HALF.y * 2.0 + 0.3),
            Vec3::new(x, h, 0.0),
            none,
            tw,
        );
    }
    for x in [-DOOR_HALF_WIDTH, DOOR_HALF_WIDTH] {
        cuboid(
            &mut b,
            Vec3::new(0.14, h - 0.55, 0.32),
            Vec3::new(x, (h - 0.55) / 2.0, HALF.y),
            none,
            tw,
        );
    }
    // Windows: dark panes with a cross frame on the side walls and the back.
    let pane = lin(Color::srgb_u8(120, 150, 160));
    for (center, normal_x) in [
        (Vec3::new(-HALF.x, 1.45, -0.6), true),
        (Vec3::new(HALF.x, 1.45, 0.4), true),
        (Vec3::new(-1.6, 1.45, -HALF.y), false),
        (Vec3::new(1.6, 1.45, -HALF.y), false),
    ] {
        let (size, bar_v, bar_h) = if normal_x {
            (
                Vec3::new(t + 0.04, 0.8, 1.0),
                Vec3::new(t + 0.08, 0.86, 0.07),
                Vec3::new(t + 0.08, 0.07, 1.06),
            )
        } else {
            (
                Vec3::new(1.0, 0.8, t + 0.04),
                Vec3::new(0.07, 0.86, t + 0.08),
                Vec3::new(1.06, 0.07, t + 0.08),
            )
        };
        cuboid(&mut b, size, center, none, move |_| pane);
        cuboid(&mut b, bar_v, center, none, tw);
        cuboid(&mut b, bar_h, center, none, tw);
    }
    // Gable roof (two slopes) and gable ends.
    let pitch: f32 = 0.62;
    let run = HALF.x + 0.45;
    let slope = run / pitch.cos();
    for dir in [-1.0f32, 1.0] {
        let center = Vec3::new(dir * run / 2.0, h + run * pitch.tan() / 2.0, 0.0);
        let rot = Quat::from_rotation_z(-dir * pitch);
        cuboid(
            &mut b,
            Vec3::new(slope, 0.12, HALF.y * 2.0 + 0.7),
            center,
            rot,
            move |p| mix(roof_dark, roof, ((p.x * 9.0).sin() * 0.5 + 0.5) * 0.6 + 0.3),
        );
    }
    let peak = run * pitch.tan();
    for z in [-HALF.y, HALF.y] {
        // Stepped plaster triangle under each gable.
        for k in 0..6 {
            let y = h + (k as f32 + 0.5) * peak / 6.0;
            let w = (HALF.x * 2.0) * (1.0 - (k as f32 + 0.5) / 6.0);
            cuboid(
                &mut b,
                Vec3::new(w, peak / 6.0 + 0.01, t),
                Vec3::new(0.0, y, z),
                none,
                wall_paint,
            );
        }
    }
    // Chimney.
    let brick = lin(Color::srgb_u8(168, 110, 92));
    cuboid(
        &mut b,
        Vec3::new(0.55, 1.6, 0.55),
        Vec3::new(1.9, h + 1.1, -1.4),
        none,
        move |p| mix(brick * 0.85, brick, hash3(p * 6.0) * 0.5 + 0.5),
    );

    // --- Furniture (matches the colliders) ---
    // Workbench along the back wall, with a ribbon spool and loose flowers.
    cuboid(
        &mut b,
        Vec3::new(2.5, 0.08, 0.84),
        Vec3::new(0.0, 0.92, -2.35),
        none,
        wood(true),
    );
    for x in [-1.15f32, 1.15] {
        for z in [-2.7f32, -2.0] {
            cuboid(
                &mut b,
                Vec3::new(0.08, 0.9, 0.08),
                Vec3::new(x, 0.45, z),
                none,
                wood(false),
            );
        }
    }
    let pink = lin(Color::srgb_u8(238, 190, 190));
    b.add(
        &Mesh::from(Cylinder::new(0.09, 0.08)),
        Transform::from_xyz(-0.7, 1.0, -2.45)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        move |_| pink,
    );
    for (i, c) in [
        (250, 247, 238),
        (246, 224, 128),
        (226, 120, 104),
        (128, 150, 216),
        (226, 160, 196),
    ]
    .into_iter()
    .enumerate()
    {
        let col = lin(Color::srgb_u8(c.0, c.1, c.2));
        let x = 0.1 + i as f32 * 0.18;
        b.add(
            &Sphere::new(1.0).mesh().ico(1).expect("valid icosphere"),
            Transform::from_xyz(x, 0.98, -2.3 + (i % 2) as f32 * 0.12)
                .with_scale(Vec3::new(0.05, 0.02, 0.05)),
            move |_| col,
        );
    }
    // Tool shelf on the left wall with hanging tools.
    cuboid(
        &mut b,
        Vec3::new(0.5, 2.0, 2.3),
        Vec3::new(-3.05, 1.0, -0.6),
        none,
        wood(false),
    );
    for y in [0.6f32, 1.2, 1.8] {
        cuboid(
            &mut b,
            Vec3::new(0.62, 0.05, 2.36),
            Vec3::new(-2.98, y, -0.6),
            none,
            wood(true),
        );
    }
    let metal = lin(Color::srgb_u8(170, 176, 182));
    for (i, z) in [-1.4f32, -0.9, -0.4, 0.1].into_iter().enumerate() {
        cuboid(
            &mut b,
            Vec3::new(0.04, 0.45, 0.04),
            Vec3::new(-2.72, 1.45, z),
            none,
            wood(true),
        );
        let head = if i % 2 == 0 {
            Vec3::new(0.05, 0.05, 0.22)
        } else {
            Vec3::new(0.05, 0.16, 0.05)
        };
        cuboid(&mut b, head, Vec3::new(-2.72, 1.2, z), none, move |_| metal);
    }
    // Storage chest by the right wall.
    cuboid(
        &mut b,
        Vec3::new(0.84, 0.62, 0.68),
        Vec3::new(2.85, 0.33, -1.2),
        none,
        wood(false),
    );
    cuboid(
        &mut b,
        Vec3::new(0.88, 0.1, 0.72),
        Vec3::new(2.85, 0.68, -1.2),
        none,
        wood(true),
    );
    cuboid(
        &mut b,
        Vec3::new(0.9, 0.06, 0.05),
        Vec3::new(2.85, 0.5, -0.85),
        none,
        move |_| metal,
    );
    // Display table near the door, with three vases (bouquets go in them).
    cuboid(
        &mut b,
        Vec3::new(1.24, 0.07, 0.84),
        Vec3::new(2.5, 0.82, 1.5),
        none,
        wood(true),
    );
    for x in [1.98f32, 3.02] {
        for z in [1.15f32, 1.85] {
            cuboid(
                &mut b,
                Vec3::new(0.07, 0.8, 0.07),
                Vec3::new(x, 0.4, z),
                none,
                wood(false),
            );
        }
    }
    let glaze = lin(Color::srgb_u8(156, 178, 138));
    for slot in DISPLAY_SLOTS {
        b.add(
            &Mesh::from(Cylinder::new(0.075, 0.2)),
            Transform::from_xyz(slot.x, 0.95, slot.y),
            move |p| mix(glaze * 0.8, glaze, p.y + 0.5),
        );
    }
    b.build()
}

/// Vase positions on the display table (workshop-local x, z); bouquets are
/// placed in them.
pub const DISPLAY_SLOTS: [Vec2; 3] = [
    Vec2::new(2.12, 1.5),
    Vec2::new(2.5, 1.62),
    Vec2::new(2.88, 1.45),
];

/// An iron lantern post; the glowing glass box hangs from its arm at
/// (0, 2.05, 0.32), facing the path (+Z).
fn lantern_post() -> Mesh {
    let mut b = MeshBuilder::default();
    let iron = |p: Vec3| {
        mix(
            lin(Color::srgb_u8(36, 38, 46)),
            lin(Color::srgb_u8(74, 78, 92)),
            0.5 + hash3(p * 6.0) * 0.3,
        )
    };
    let none = Quat::IDENTITY;
    cuboid(
        &mut b,
        Vec3::new(0.28, 0.12, 0.28),
        Vec3::new(0.0, 0.06, 0.0),
        none,
        iron,
    );
    cuboid(
        &mut b,
        Vec3::new(0.09, 2.45, 0.09),
        Vec3::new(0.0, 1.22, 0.0),
        none,
        iron,
    );
    // Arm reaching over towards the path, with the lantern's cap and base.
    cuboid(
        &mut b,
        Vec3::new(0.05, 0.05, 0.4),
        Vec3::new(0.0, 2.4, 0.18),
        none,
        iron,
    );
    cuboid(
        &mut b,
        Vec3::new(0.03, 0.12, 0.03),
        Vec3::new(0.0, 2.3, 0.32),
        none,
        iron,
    );
    cuboid(
        &mut b,
        Vec3::new(0.3, 0.06, 0.3),
        Vec3::new(0.0, 2.23, 0.32),
        none,
        iron,
    );
    cuboid(
        &mut b,
        Vec3::new(0.26, 0.05, 0.26),
        Vec3::new(0.0, 1.88, 0.32),
        none,
        iron,
    );
    b.build()
}

pub(super) fn wood(light: bool) -> impl Fn(Vec3) -> LinearRgba + Copy {
    let dark = lin(Color::srgb_u8(128, 96, 70));
    let pale = lin(Color::srgb_u8(178, 140, 104));
    move |p: Vec3| {
        let grain = hash3(p * 9.0) * 0.12;
        mix(
            dark,
            pale,
            if light { 0.7 } else { 0.4 } + grain + p.y * 0.1,
        )
    }
}

pub(super) fn cuboid(
    b: &mut MeshBuilder,
    size: Vec3,
    at: Vec3,
    rot: Quat,
    paint: impl Fn(Vec3) -> LinearRgba,
) {
    b.add(
        &Mesh::from(Cuboid::from_size(size)),
        Transform::from_translation(at).with_rotation(rot),
        paint,
    );
}

/// Front (+Z) faces the path; length along X; seat at 0.45 m.
fn bench() -> Mesh {
    let mut b = MeshBuilder::default();
    let none = Quat::IDENTITY;
    for x in [-0.72, 0.72] {
        for z in [-0.16, 0.18] {
            cuboid(
                &mut b,
                Vec3::new(0.08, 0.45, 0.08),
                Vec3::new(x, 0.225, z),
                none,
                wood(false),
            );
        }
        cuboid(
            &mut b,
            Vec3::new(0.07, 0.55, 0.07),
            Vec3::new(x, 0.72, -0.2),
            Quat::from_rotation_x(-0.12),
            wood(false),
        );
        cuboid(
            &mut b,
            Vec3::new(0.08, 0.06, 0.48),
            Vec3::new(x, 0.42, 0.0),
            none,
            wood(false),
        );
    }
    for z in [-0.14, 0.0, 0.14] {
        cuboid(
            &mut b,
            Vec3::new(1.7, 0.045, 0.12),
            Vec3::new(0.0, 0.47, z),
            none,
            wood(true),
        );
    }
    for y in [0.66, 0.84] {
        cuboid(
            &mut b,
            Vec3::new(1.7, 0.11, 0.035),
            Vec3::new(0.0, y, -0.22 - (y - 0.66) * 0.12),
            Quat::from_rotation_x(-0.12),
            wood(true),
        );
    }
    b.build()
}

/// Ring of stones, water, two posts, a gable roof, a crossbar and a bucket.
fn well() -> Mesh {
    let mut b = MeshBuilder::default();
    let stone_dark = lin(Color::srgb_u8(150, 142, 130));
    let stone_light = lin(Color::srgb_u8(204, 196, 180));
    let n = 14;
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let at = Vec3::new(a.cos() * 0.86, 0.38, a.sin() * 0.86);
        let tint = hash3(at * 3.0) * 0.5 + 0.5;
        let col = mix(stone_dark, stone_light, tint);
        cuboid(
            &mut b,
            Vec3::new(0.32, 0.76, 0.42),
            at,
            Quat::from_rotation_y(-a),
            move |p| mix(col * 0.85, col, p.y + 0.5),
        );
    }
    // Capstones.
    for i in 0..n {
        let a = (i as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
        let at = Vec3::new(a.cos() * 0.86, 0.8, a.sin() * 0.86);
        cuboid(
            &mut b,
            Vec3::new(0.36, 0.1, 0.42),
            at,
            Quat::from_rotation_y(-a),
            move |_| stone_light,
        );
    }
    let water = lin(Color::srgb_u8(70, 96, 110));
    b.add(
        &Mesh::from(Cylinder::new(0.7, 0.04)),
        Transform::from_xyz(0.0, 0.5, 0.0),
        move |_| water,
    );
    for x in [-0.86, 0.86] {
        cuboid(
            &mut b,
            Vec3::new(0.11, 2.1, 0.11),
            Vec3::new(x, 1.15, 0.0),
            Quat::IDENTITY,
            wood(false),
        );
    }
    let crossbar = Transform::from_xyz(0.0, 1.7, 0.0)
        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
    b.add(&Mesh::from(Cylinder::new(0.05, 1.8)), crossbar, wood(true));
    let roof = lin(Color::srgb_u8(150, 96, 78));
    for side in [-1.0f32, 1.0] {
        let rot = Quat::from_rotation_x(side * 0.62);
        let at = Vec3::new(0.0, 2.3, side * 0.42);
        b.add(
            &Mesh::from(Cuboid::new(2.1, 0.06, 1.0)),
            Transform::from_translation(at).with_rotation(rot),
            move |p| mix(roof * 0.8, roof, p.x * 0.3 + 0.5 + hash3(p * 6.0) * 0.1),
        );
    }
    // Bucket on a rope.
    let rope = lin(Color::srgb_u8(196, 176, 140));
    b.add(
        &Mesh::from(Cylinder::new(0.012, 0.5)),
        Transform::from_xyz(0.25, 1.42, 0.0),
        move |_| rope,
    );
    b.add(
        &Mesh::from(Cylinder::new(0.13, 0.2)),
        Transform::from_xyz(0.25, 1.08, 0.0),
        wood(false),
    );
    b.build()
}

/// A post with two arrow boards.
fn signpost() -> Mesh {
    let mut b = MeshBuilder::default();
    cuboid(
        &mut b,
        Vec3::new(0.12, 2.25, 0.12),
        Vec3::new(0.0, 1.12, 0.0),
        Quat::IDENTITY,
        wood(false),
    );
    let cream = lin(Color::srgb_u8(238, 228, 204));
    for (y, dir) in [(1.9f32, 1.0f32), (1.55, -1.0)] {
        let tilt = Quat::from_rotation_z(dir * 0.04);
        cuboid(
            &mut b,
            Vec3::new(0.8, 0.2, 0.04),
            Vec3::new(dir * 0.42, y, 0.07),
            tilt,
            wood(true),
        );
        // Pointed tip and a pale painted band where the lettering would be.
        cuboid(
            &mut b,
            Vec3::new(0.15, 0.15, 0.04),
            Vec3::new(dir * 0.84, y, 0.07),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
            wood(true),
        );
        cuboid(
            &mut b,
            Vec3::new(0.52, 0.06, 0.01),
            Vec3::new(dir * 0.4, y, 0.095),
            tilt,
            move |_| cream,
        );
    }
    b.build()
}
