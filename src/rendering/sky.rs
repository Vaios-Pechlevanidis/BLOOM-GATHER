//! Sky dome with a soft vertical gradient, the sun and the moon, stars and
//! slowly drifting clouds, all following the time of day ([`Daylight`]).
//!
//! The horizon colour equals the fog colour, so the distant fields melt into
//! the sky. Sky and clouds are unlit and ignore fog.

use bevy::light::NotShadowCaster;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bloom_core::rng::SplitMix64;

use super::camera::MainCamera;
use super::daylight::Daylight;
use super::meshes::{hash3, mix, MeshBuilder};
use super::scene::SceneEntity;
use crate::game::{Content, RegionScene};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_sky.run_if(resource_exists_and_changed::<RegionScene>),
                (follow_camera, drift_clouds, paint_sky),
            )
                .chain(),
        );
    }
}

/// Follows the camera at the centre of the sky.
#[derive(Component)]
struct SkyDome;

/// The gradient dome, repainted as the light changes.
#[derive(Component)]
struct Dome {
    /// Colours last painted with (horizon, zenith, sun direction, dusk).
    painted: Option<(Vec4, Vec4, Vec3, f32)>,
}

#[derive(Component)]
struct Stars;

/// A disc on the sky that tracks the sun or the moon.
#[derive(Component, Clone, Copy, PartialEq)]
enum SkyBody {
    Sun,
    SunHalo,
    Moon,
    MoonHalo,
}

#[derive(Component)]
struct Cloud {
    speed: f32,
}

const DOME_RADIUS: f32 = 800.0;
const CLOUD_WRAP: f32 = 650.0;

/// The colour at the top of the sky for a region whose horizon is `horizon`.
pub fn zenith_for(horizon: LinearRgba) -> LinearRgba {
    // A deeper, slightly bluer version of the horizon.
    let blue = LinearRgba::new(0.22, 0.46, 0.82, 1.0);
    mix(horizon, blue, 0.72)
}

fn unlit(materials: &mut Assets<StandardMaterial>, alpha: AlphaMode) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        alpha_mode: alpha,
        ..default()
    })
}

fn spawn_sky(
    mut commands: Commands,
    scene: Res<RegionScene>,
    content: Res<Content>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let region = &content.regions[scene.region];
    let air = region.scenery.atmosphere;
    let cycle = region.scenery.day_cycle.is_some();
    let stars = cycle || air.night;
    let clouds = cycle || !air.night;

    let mut dome = Sphere::new(1.0).mesh().uv(48, 24);
    let n = dome.count_vertices();
    dome.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; n]);
    commands.spawn((
        SceneEntity,
        SkyDome,
        Dome { painted: None },
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(unlit(&mut materials, AlphaMode::Opaque)),
        Transform::from_scale(Vec3::splat(DOME_RADIUS)),
        NotShadowCaster,
    ));

    if stars {
        spawn_stars(&mut commands, &mut meshes, &mut materials, region.seed);
    }
    // Sun (in cycles) and moon: unit discs placed on the dome every frame.
    let disc = meshes.add(Circle::new(1.0).mesh().resolution(40));
    let mut bodies = vec![SkyBody::Moon, SkyBody::MoonHalo];
    if cycle {
        bodies.extend([SkyBody::Sun, SkyBody::SunHalo]);
    }
    for body in bodies {
        let mesh = if body == SkyBody::Moon {
            let mut m = MeshBuilder::default();
            let face = LinearRgba::rgb(1.0, 0.98, 0.9);
            m.add(
                &Mesh::from(Circle::new(1.0).mesh().resolution(40)),
                Transform::IDENTITY,
                move |p| {
                    mix(
                        face,
                        LinearRgba::rgb(0.82, 0.84, 0.86),
                        (hash3(p * 6.0) * 0.5 + 0.5) * 0.35,
                    )
                },
            );
            meshes.add(m.build())
        } else {
            disc.clone()
        };
        let alpha = match body {
            SkyBody::SunHalo | SkyBody::MoonHalo => AlphaMode::Add,
            _ => AlphaMode::Opaque,
        };
        commands.spawn((
            SceneEntity,
            body,
            Mesh3d(mesh),
            MeshMaterial3d(unlit(&mut materials, alpha)),
            Transform::from_scale(Vec3::ZERO),
            NotShadowCaster,
        ));
    }

    if !clouds {
        return;
    }
    // A few cloud shapes, each a cluster of soft puffs: white on top, a
    // gentle blue-grey underneath. The material tints them by time of day.
    let horizon = super::meshes::lin(super::color(region.palette.sky));
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
    let cloud_material = unlit(&mut materials, AlphaMode::Opaque);
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

/// Stars scattered over the dome, thinning towards the horizon.
fn spawn_stars(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    seed: u64,
) {
    let mut rng = SplitMix64::new(seed ^ 0x57A2);
    let star = Mesh::from(Rectangle::new(1.0, 1.0));
    let mut b = MeshBuilder::default();
    for _ in 0..900 {
        let y = rng.range_f32(0.05, 1.0).sqrt();
        let a = rng.range_f32(0.0, std::f32::consts::TAU);
        let r = (1.0 - y * y).sqrt();
        let dir = Vec3::new(a.cos() * r, y, a.sin() * r);
        let size = rng.range_f32(0.0012, 0.0032);
        let bright = rng.range_f32(0.55, 1.0);
        let tint = if rng.next_f32() < 0.2 {
            LinearRgba::rgb(0.8, 0.86, 1.0)
        } else {
            LinearRgba::rgb(1.0, 0.97, 0.9)
        } * bright;
        let t = Transform::from_translation(dir * 0.97)
            .looking_at(Vec3::ZERO, Vec3::Y)
            .with_scale(Vec3::splat(size));
        b.add(&star, t, move |_| tint);
    }
    commands.spawn((
        SceneEntity,
        SkyDome,
        Stars,
        Mesh3d(meshes.add(b.build())),
        MeshMaterial3d(unlit(materials, AlphaMode::Add)),
        Transform::from_scale(Vec3::splat(DOME_RADIUS)),
        NotShadowCaster,
    ));
}

#[allow(clippy::type_complexity)]
fn follow_camera(
    cameras: Query<&Transform, (With<MainCamera>, Without<SkyDome>, Without<SkyBody>)>,
    mut domes: Query<&mut Transform, (With<SkyDome>, Without<SkyBody>)>,
    mut bodies: Query<(&SkyBody, &mut Transform), Without<SkyDome>>,
    daylight: Res<Daylight>,
) {
    let Ok(camera) = cameras.single() else { return };
    for mut dome in &mut domes {
        dome.translation = camera.translation;
    }
    for (body, mut t) in &mut bodies {
        let (dir, size, depth) = match body {
            SkyBody::Sun => (daylight.sun_dir, 0.028, 0.95),
            SkyBody::SunHalo => (daylight.sun_dir, 0.06 + 0.03 * daylight.dusk, 0.955),
            SkyBody::Moon => (daylight.moon_dir, 0.03, 0.95),
            SkyBody::MoonHalo => (daylight.moon_dir, 0.09, 0.955),
        };
        let at = camera.translation + dir * DOME_RADIUS * depth;
        *t = Transform::from_translation(at)
            .looking_at(camera.translation, Vec3::Y)
            .with_scale(Vec3::splat(size * DOME_RADIUS));
    }
}

/// Repaint the dome and tint the sun, moon, stars and clouds for the time
/// of day.
#[allow(clippy::type_complexity)]
fn paint_sky(
    daylight: Res<Daylight>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut domes: Query<(&mut Dome, &Mesh3d)>,
    stars: Query<&MeshMaterial3d<StandardMaterial>, With<Stars>>,
    mut bodies: Query<(&SkyBody, &MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
    clouds: Query<&MeshMaterial3d<StandardMaterial>, With<Cloud>>,
) {
    let d = &*daylight;
    let key = (d.horizon.to_vec4(), d.zenith.to_vec4(), d.sun_dir, d.dusk);
    for (mut dome, mesh) in &mut domes {
        let stale = dome.painted.is_none_or(|(h, z, s, k)| {
            h.distance(key.0) > 0.002
                || z.distance(key.1) > 0.002
                || s.angle_between(key.2) > 0.004
                || (k - key.3).abs() > 0.01
        });
        if !stale {
            continue;
        }
        dome.painted = Some(key);
        let Some(mut mesh) = meshes.get_mut(&mesh.0) else {
            continue;
        };
        let dirs: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => {
                p.iter().map(|v| Vec3::from_array(*v)).collect()
            }
            _ => continue,
        };
        let glow_color = d.sun_color;
        // The glow around the sun: wide and strong at sunrise and sunset, a
        // faint brightening by day, nothing at night.
        let glow_strength = (0.18 + 0.82 * d.dusk) * (1.0 - d.night);
        let sun_h = d.sun_dir.with_y(0.0).normalize_or(Vec3::Z);
        let colors: Vec<[f32; 4]> = dirs
            .iter()
            .map(|&v| {
                let up = v.y.max(0.0);
                let t = up.powf(0.55);
                let mut c = mix(d.horizon, d.zenith, t);
                // Close to the sun, and along the horizon beneath it.
                let near = v.dot(d.sun_dir).max(0.0).powf(10.0);
                let band = v
                    .with_y(0.0)
                    .normalize_or(Vec3::Z)
                    .dot(sun_h)
                    .max(0.0)
                    .powf(3.0)
                    * (1.0 - up).powf(4.0);
                let g = (near * 0.9 + band * 0.55 * d.dusk) * glow_strength;
                c = mix(c, glow_color, g.min(1.0));
                c.to_f32_array()
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }

    let star_level = d.night.powf(1.5);
    for m in &stars {
        if let Some(mut m) = materials.get_mut(&m.0) {
            m.base_color = Color::LinearRgba(LinearRgba::rgb(star_level, star_level, star_level));
        }
    }
    for (body, m, mut vis) in &mut bodies {
        let (shown, tint) = match body {
            SkyBody::Sun => (
                d.elevation_deg > -3.0,
                d.sun_color * (2.5 + 2.0 * (1.0 - d.dusk)),
            ),
            SkyBody::SunHalo => (d.elevation_deg > -3.0, d.sun_color * (0.12 + 0.12 * d.dusk)),
            SkyBody::Moon => (
                d.night > 0.05 && d.moon_dir.y > -0.05,
                LinearRgba::WHITE * (0.4 + 1.1 * d.night),
            ),
            SkyBody::MoonHalo => (
                d.night > 0.05 && d.moon_dir.y > -0.05,
                LinearRgba::rgb(0.7, 0.78, 1.0) * (0.12 * d.night),
            ),
        };
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if let Some(mut m) = materials.get_mut(&m.0) {
            m.base_color = Color::LinearRgba(tint.with_alpha(1.0));
        }
    }
    // Clouds: white by day, flushed pink and gold at dusk, dim blue at night.
    let day = LinearRgba::WHITE;
    let flushed = mix(LinearRgba::rgb(1.0, 0.72, 0.62), d.sun_color, 0.4);
    let night = LinearRgba::rgb(0.10, 0.12, 0.2);
    let tint = mix(mix(day, flushed, d.dusk * 0.85), night, d.night);
    if let Some(m) = clouds.iter().next() {
        if let Some(mut m) = materials.get_mut(&m.0) {
            m.base_color = Color::LinearRgba(tint);
        }
    }
}

fn drift_clouds(
    time: Res<Time>,
    weather: Res<super::Weather>,
    mut clouds: Query<(&Cloud, &mut Transform)>,
) {
    for (cloud, mut t) in &mut clouds {
        t.translation.x += cloud.speed * weather.wind * time.delta_secs();
        if t.translation.x > CLOUD_WRAP {
            t.translation.x -= CLOUD_WRAP * 2.0;
        }
    }
}
