//! Builds the active region as a 3D scene: sky, fog, sunlight, chunked
//! terrain, trees, rocks, the boundary fence and scenery beyond it.
//!
//! The scene is built once per region and survives Main Menu ↔ In Game
//! transitions (the title screen shows the same world behind the menu).

use bevy::pbr::{CascadeShadowConfigBuilder, DistanceFog, FogFalloff};
use bevy::prelude::*;

use super::camera::MainCamera;
use super::color;
use super::meshes::{self, TREE_MODEL_HEIGHT};
use super::wind::{sway_by_height, wind_material, Wind, WindMaterial};
use crate::game::{ActiveRegion, AppState, Content, RegionScene, Session, SessionSet};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(OnEnter(AppState::MainMenu), SceneSet.after(SessionSet))
            .add_systems(OnEnter(AppState::MainMenu), ensure_scene.in_set(SceneSet))
            .add_systems(OnEnter(AppState::InGame), ensure_scene.in_set(SceneSet));
    }
}

/// Builds the scene and inserts [`RegionScene`]; player/camera setup runs
/// after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SceneSet;

/// Marks entities belonging to the current region scene.
#[derive(Component)]
pub struct SceneEntity;

fn ensure_scene(
    mut commands: Commands,
    content: Res<Content>,
    active: Res<ActiveRegion>,
    existing: Option<Res<RegionScene>>,
    mut session: ResMut<Session>,
    old: Query<Entity, With<SceneEntity>>,
    cameras: Query<Entity, With<MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    (mut materials, mut wind_materials): (
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<WindMaterial>>,
    ),
    (mut clear_color, mut ambient): (ResMut<ClearColor>, ResMut<AmbientLight>),
) {
    if existing.is_some_and(|scene| scene.region == active.0) {
        return;
    }
    for entity in &old {
        commands.entity(entity).despawn();
    }

    let region = active.def(&content);
    let started = std::time::Instant::now();
    let runtime = session.region(active.0).clone();
    let layout = runtime.layout.clone();
    let palette = region.palette;
    let sky = color(palette.sky);

    // One matte material for everything vertex-coloured: maximum batching.
    let painted = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.93,
        reflectance: 0.12,
        ..default()
    });

    // --- Atmosphere -------------------------------------------------------
    clear_color.0 = sky;
    *ambient = AmbientLight {
        color: Color::srgb(0.86, 0.9, 1.0),
        brightness: 420.0,
        ..default()
    };
    let extent = layout.half_size.max_element() * 2.0;
    for camera in &cameras {
        commands.entity(camera).insert(DistanceFog {
            color: sky,
            directional_light_color: Color::srgba(1.0, 0.93, 0.78, 0.35),
            directional_light_exponent: 24.0,
            falloff: FogFalloff::Linear {
                start: extent * 0.3,
                end: extent * 1.05,
            },
        });
    }
    // Late-afternoon sun from the south-west, so the player (facing north
    // at spawn) sees sunlit flowers and soft shadows falling away.
    commands.spawn((
        SceneEntity,
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.84),
            illuminance: 3_600.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-30.0, 46.0, 28.0).looking_at(Vec3::ZERO, Vec3::Y),
        // Lights the world and the first-person hands/tools layer.
        bevy::render::view::RenderLayers::from_layers(&[0, crate::viewmodel::VIEW_LAYER]),
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            first_cascade_far_bound: 12.0,
            maximum_distance: 110.0,
            ..default()
        }
        .build(),
    ));

    // --- Terrain ----------------------------------------------------------
    let chunks = meshes::terrain_chunks(&layout, &palette, content.world.chunk_size);
    let chunk_count = chunks.len();
    for mesh in chunks {
        commands.spawn((
            SceneEntity,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(painted.clone()),
        ));
    }

    // --- Props --------------------------------------------------------------
    // Canopies sway (trunks stay put): sway weight rises from the top of
    // the trunk to the crown.
    let tree_meshes = meshes::tree_variants(&palette).map(|mut m| {
        sway_by_height(&mut m, 2.3, TREE_MODEL_HEIGHT, 1.0);
        meshes.add(m)
    });
    let tree_material = wind_materials.add(wind_material(
        StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.93,
            reflectance: 0.12,
            ..default()
        },
        Wind::new(0.09, 1.1, 0.15),
    ));
    for tree in layout.trees.iter().chain(&layout.backdrop_trees) {
        commands.spawn((
            SceneEntity,
            Mesh3d(tree_meshes[tree.variant as usize % 3].clone()),
            MeshMaterial3d(tree_material.clone()),
            Transform::from_xyz(tree.position.x, tree.base - 0.15, tree.position.y)
                .with_rotation(Quat::from_rotation_y(tree.yaw))
                .with_scale(Vec3::splat(tree.height / TREE_MODEL_HEIGHT)),
        ));
    }
    let rock_meshes = meshes::rock_variants(&palette).map(|m| meshes.add(m));
    for rock in &layout.rocks {
        // Sunk so the visible top is at base + 0.8·height, matching the collider.
        let center_y = rock.base + rock.height * 0.8 - rock.height;
        commands.spawn((
            SceneEntity,
            Mesh3d(rock_meshes[rock.variant as usize % 3].clone()),
            MeshMaterial3d(painted.clone()),
            Transform::from_xyz(rock.position.x, center_y, rock.position.y)
                .with_rotation(Quat::from_rotation_y(rock.yaw))
                .with_scale(Vec3::new(rock.radius, rock.height, rock.radius)),
        ));
    }
    commands.spawn((
        SceneEntity,
        Mesh3d(meshes.add(meshes::fence(&layout))),
        MeshMaterial3d(painted),
    ));

    info!(
        "Built region '{}' in {:.1} ms: {} terrain chunks, {} trees (+{} scenery), {} rocks, {} colliders",
        region.id,
        started.elapsed().as_secs_f64() * 1000.0,
        chunk_count,
        layout.trees.len(),
        layout.backdrop_trees.len(),
        layout.rocks.len(),
        layout.colliders.len()
    );
    commands.insert_resource(RegionScene {
        region: active.0,
        layout,
        field: runtime.field,
    });
}
