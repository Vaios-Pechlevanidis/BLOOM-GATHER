//! The single 3D camera. In game it is driven by the player (see
//! `player`); behind the title screen it slowly circles the region.

use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::view::Hdr;

use crate::game::{AppState, RegionScene, Settings};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera).add_systems(
            Update,
            (
                apply_fov,
                apply_quality,
                menu_orbit.run_if(in_state(AppState::MainMenu)),
            ),
        );
    }
}

#[derive(Component)]
pub struct MainCamera;

fn spawn_camera(mut commands: Commands, settings: Res<Settings>) {
    commands.spawn((
        Camera3d::default(),
        // HDR so bright things (lanterns, glowing flowers, fireflies) can
        // bloom softly on Medium and High.
        Hdr,
        MainCamera,
        if settings.quality.msaa() {
            Msaa::Sample4
        } else {
            Msaa::Off
        },
        Projection::Perspective(PerspectiveProjection {
            fov: settings.fov_degrees.to_radians(),
            near: 0.05,
            far: 1_000.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 25.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn apply_fov(settings: Res<Settings>, mut cameras: Query<&mut Projection, With<MainCamera>>) {
    if !settings.is_changed() {
        return;
    }
    for mut projection in &mut cameras {
        if let Projection::Perspective(p) = &mut *projection {
            p.fov = settings.fov_degrees.to_radians();
        }
    }
}

type CameraQuality<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Msaa, Has<Bloom>, Has<MainCamera>), With<Camera3d>>;

/// Anti-aliasing and bloom follow the graphics quality.
fn apply_quality(
    mut commands: Commands,
    settings: Res<Settings>,
    // Every 3D camera (world and hands) shares one anti-aliasing mode.
    mut cameras: CameraQuality,
) {
    if !settings.is_changed() {
        return;
    }
    let q = settings.quality;
    for (e, mut msaa, has_bloom, main) in &mut cameras {
        let want = if q.msaa() { Msaa::Sample4 } else { Msaa::Off };
        if *msaa != want {
            *msaa = want;
        }
        if !main {
            continue;
        }
        if q.bloom() && !has_bloom {
            // Only truly bright things glow (lantern glass, fireflies,
            // glowing flowers); daylight scenes stay crisp.
            commands.entity(e).insert(Bloom {
                intensity: 0.18,
                low_frequency_boost: 0.4,
                prefilter: BloomPrefilter {
                    threshold: 1.0,
                    threshold_softness: 0.4,
                },
                composite_mode: BloomCompositeMode::Additive,
                ..Bloom::NATURAL
            });
        } else if !q.bloom() && has_bloom {
            commands.entity(e).remove::<Bloom>();
        }
    }
}

/// A slow, high orbit over the meadow behind the title screen.
fn menu_orbit(
    time: Res<Time<Real>>,
    scene: Option<Res<RegionScene>>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(scene) = scene else { return };
    let layout = &scene.layout;
    let ground = layout.terrain.height_at(Vec2::ZERO);
    let target = Vec3::new(0.0, ground + 2.0, 0.0);
    let radius = layout.half_size.min_element() * 0.85;
    let angle = time.elapsed_secs() * 0.035 + 0.6;
    let eye = Vec3::new(angle.sin() * radius, ground + 12.0, angle.cos() * radius);
    for mut transform in &mut cameras {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}
