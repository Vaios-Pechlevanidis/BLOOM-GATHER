//! 3D models for landmarks: a wooden bench, a stone well with a little
//! roof, and a signpost. Placement and collision come from
//! `bloom_core::world::layout`; these are the visuals.
//!
//! Temporary procedural art (see TODO.md), in the same style as the trees.

use bevy::prelude::*;
use bloom_core::world::LandmarkKind;

use super::meshes::{hash3, lin, mix, MeshBuilder};
use super::scene::SceneEntity;
use crate::game::RegionScene;

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
    for l in &scene.layout.landmarks {
        let mesh = match l.kind {
            LandmarkKind::Bench => bench.clone(),
            LandmarkKind::Well => well.clone(),
            LandmarkKind::Signpost => sign.clone(),
        };
        commands.spawn((
            SceneEntity,
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(l.position.x, l.base, l.position.y)
                .with_rotation(Quat::from_rotation_y(l.yaw)),
        ));
    }
}

fn wood(light: bool) -> impl Fn(Vec3) -> LinearRgba + Copy {
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

fn cuboid(
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
