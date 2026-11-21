//! The single 3D camera. In game it is driven by the player (see
//! `player`); behind the title screen it slowly circles the region.

use bevy::prelude::*;

use crate::game::{AppState, RegionScene, Settings};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera).add_systems(
            Update,
            (apply_fov, menu_orbit.run_if(in_state(AppState::MainMenu))),
        );
    }
}

#[derive(Component)]
pub struct MainCamera;

fn spawn_camera(mut commands: Commands, settings: Res<Settings>) {
    commands.spawn((
        Camera3d::default(),
        MainCamera,
        Msaa::Sample4,
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
