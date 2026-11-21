//! Sky dome with a soft vertical gradient, and slowly drifting clouds.
//!
//! The horizon colour equals the fog colour, so distant hills melt into the
//! sky. Sky and clouds are unlit and ignore fog.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use bloom_core::rng::SplitMix64;

use super::camera::MainCamera;
use super::color;
use super::meshes::{lin, mix, MeshBuilder};
use super::scene::SceneEntity;
use crate::game::{Content, RegionScene};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_sky.run_if(resource_exists_and_changed::<RegionScene>),
                (follow_camera, drift_clouds),
            )
                .chain(),
        );
    }
}

#[derive(Component)]
struct SkyDome;

#[derive(Component)]
struct Cloud {
    speed: f32,
}

const DOME_RADIUS: f32 = 800.0;
const CLOUD_WRAP: f32 = 650.0;

/// The colour at the top of the sky for a region whose horizon is `horizon`.
fn zenith_for(horizon: LinearRgba) -> LinearRgba {
    // A deeper, slightly bluer version of the horizon.
    let blue = LinearRgba::new(0.22, 0.46, 0.82, 1.0);
    mix(horizon, blue, 0.72)
}

fn spawn_sky(
    mut commands: Commands,
    scene: Res<RegionScene>,
    content: Res<Content>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let region = &content.regions[scene.region];
    let horizon = lin(color(region.palette.sky));
    let zenith = zenith_for(horizon);

    let mut dome = Sphere::new(1.0).mesh().uv(48, 24);
    let ys: Vec<f32> = match dome.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(p)) => p.iter().map(|v| v[1]).collect(),
        _ => Vec::new(),
    };
    let colors: Vec<[f32; 4]> = ys
        .iter()
        .map(|&y| {
            let t = y.max(0.0).powf(0.55);
            mix(horizon, zenith, t).to_f32_array()
        })
        .collect();
    dome.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

    let unlit = |materials: &mut Assets<StandardMaterial>| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })
    };
    commands.spawn((
        SceneEntity,
        SkyDome,
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(unlit(&mut materials)),
        Transform::from_scale(Vec3::splat(DOME_RADIUS)),
        NotShadowCaster,
    ));

    // A few cloud shapes, each a cluster of soft puffs: white on top, a
    // gentle blue-grey underneath.
    let top = LinearRgba::new(1.0, 0.99, 0.97, 1.0);
    let under = mix(horizon, LinearRgba::new(0.62, 0.66, 0.74, 1.0), 0.5);
    let puff = Sphere::new(1.0).mesh().ico(2).expect("valid icosphere");
    let mut rng = SplitMix64::new(region.seed ^ 0xC10D);
    let shapes: Vec<Handle<Mesh>> = (0..4)
        .map(|_| {
            let mut b = MeshBuilder::default();
            let n = 5 + rng.below(4);
            for i in 0..n {
                let along = (i as f32 - n as f32 / 2.0) * 0.55 + rng.range_f32(-0.2, 0.2);
                let r = rng.range_f32(0.45, 0.85) * (1.0 - (along.abs() * 0.25).min(0.5));
                let c = Vec3::new(along, rng.range_f32(0.0, 0.25), rng.range_f32(-0.3, 0.3));
                let t = Transform::from_translation(c).with_scale(Vec3::new(r * 1.2, r * 0.75, r));
                b.add(&puff, t, move |p| {
                    mix(under, top, (p.y * 0.5 + 0.5).powf(0.7))
                });
            }
            meshes.add(b.build())
        })
        .collect();
    let cloud_material = unlit(&mut materials);
    for i in 0..16 {
        let angle = i as f32 / 16.0 * std::f32::consts::TAU + rng.range_f32(-0.2, 0.2);
        let dist = rng.range_f32(260.0, 560.0);
        let pos = Vec3::new(
            angle.cos() * dist,
            rng.range_f32(90.0, 150.0),
            angle.sin() * dist,
        );
        commands.spawn((
            SceneEntity,
            Cloud {
                speed: rng.range_f32(1.2, 2.4),
            },
            Mesh3d(shapes[i % shapes.len()].clone()),
            MeshMaterial3d(cloud_material.clone()),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(rng.range_f32(-0.4, 0.4)))
                .with_scale(Vec3::splat(rng.range_f32(16.0, 32.0))),
            NotShadowCaster,
        ));
    }
}

fn follow_camera(
    cameras: Query<&Transform, (With<MainCamera>, Without<SkyDome>)>,
    mut domes: Query<&mut Transform, With<SkyDome>>,
) {
    let Ok(camera) = cameras.single() else { return };
    for mut dome in &mut domes {
        dome.translation = camera.translation;
    }
}

fn drift_clouds(time: Res<Time>, mut clouds: Query<(&Cloud, &mut Transform)>) {
    for (cloud, mut t) in &mut clouds {
        t.translation.x += cloud.speed * time.delta_secs();
        if t.translation.x > CLOUD_WRAP {
            t.translation.x -= CLOUD_WRAP * 2.0;
        }
    }
}
