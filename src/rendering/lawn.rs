//! The meadow lawn, laid from a model of a lawn patch (`assets/lawn/lawn.glb`,
//! see `scripts/lawn_tile.py`): square tiles of short modelled grass blades
//! at the model's own scale, each turned at random so no pattern shows,
//! swaying in the wind.
//!
//! Blades this short only show close up, so the lawn is built around the
//! player alone, in small chunks made as the player comes near and dropped
//! again some way behind them; beyond, the terrain's own green carries on
//! (the meadow's colours in the game data match the lawn's).
//!
//! The lawn keeps off paths, beds, water, trunks, rocks and buildings; at
//! those edges it is cut triangle by triangle, so they look natural.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bloom_core::rng::SplitMix64;
use bloom_core::world::layout::RegionLayout;

use super::scene::SceneEntity;
use super::wind::{wind_material, Wind, WindMaterial};
use crate::flowers::imported::{self, Plant};
use crate::game::{Content, Quality, RegionScene, Settings};

pub struct LawnPlugin;

impl Plugin for LawnPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lawn>()
            .add_systems(Update, keep_lawn.run_if(resource_exists::<RegionScene>));
    }
}

/// Whether the meadow is laid with the lawn model (the old procedural grass
/// tufts then stay away from the meadow).
pub fn has_lawn() -> bool {
    imported::lawn_model().is_some()
}

/// Tiles per chunk side.
const CHUNK_TILES: usize = 8;
/// Chunks are dropped this much beyond the lawn's reach (metres), so
/// walking along the edge doesn't rebuild them over and over.
const SLACK: f32 = 6.0;

/// How far the lawn reaches, by graphics quality (metres, to the middle of
/// a chunk).
fn reach(q: Quality) -> f32 {
    match q {
        Quality::Low => 6.0,
        Quality::Medium => 8.0,
        Quality::High => 9.0,
    }
}

/// The lawn tile, in its own colours.
struct LawnModel {
    tile: Plant,
    /// Tile size (model units, taken as metres).
    side: f32,
}

impl LawnModel {
    fn load() -> Option<Self> {
        let bytes = imported::lawn_model()?;
        let (name, mut tile) = match imported::parse_named(bytes) {
            Ok(mut n) if !n.is_empty() => n.swap_remove(0),
            Ok(_) => return None,
            Err(e) => {
                warn!("Lawn model unreadable: {e}");
                return None;
            }
        };
        let side = name
            .split_whitespace()
            .find_map(|w| w.strip_prefix("side=")?.parse::<f32>().ok())
            .filter(|&s| s > 0.0)?;
        let top = tile.positions.iter().map(|p| p.y).fold(0.0f32, f32::max);
        for (c, p) in tile.colors.iter_mut().zip(&tile.positions) {
            // Rooted at the ground, freest at the tips.
            let t = (p.y / top.max(1e-4)).clamp(0.0, 1.0);
            c[3] = t * t * (3.0 - 2.0 * t);
        }
        Some(Self { tile, side })
    }
}

/// One tile on the ground.
#[derive(Clone, Copy)]
struct Tile {
    /// Centre (X, Z).
    at: Vec2,
    /// Quarter turns about +Y.
    turn: u8,
    /// Partly off the lawn: cut triangle by triangle.
    partial: bool,
}

struct Chunk {
    centre: Vec2,
    tiles: Vec<Tile>,
    entity: Option<Entity>,
}

#[derive(Resource, Default)]
struct Lawn {
    /// (region, quality) the lawn was laid for.
    key: Option<(usize, Quality)>,
    /// The model (outer `None`: not loaded yet).
    model: Option<Option<LawnModel>>,
    chunks: Vec<Chunk>,
    material: Option<Handle<WindMaterial>>,
    since: f32,
}

#[derive(Component)]
struct LawnMesh;

#[allow(clippy::too_many_arguments)]
fn keep_lawn(
    mut commands: Commands,
    time: Res<Time<Real>>,
    scene: Res<RegionScene>,
    settings: Res<Settings>,
    content: Res<Content>,
    mut lawn: ResMut<Lawn>,
    old: Query<Entity, With<LawnMesh>>,
    cameras: Query<&GlobalTransform, With<super::MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
) {
    let quality = settings.quality;
    let lawn = &mut *lawn;
    let key = (scene.region, quality);
    if lawn.key != Some(key) || scene.is_changed() {
        // A new region's scene already cleared the old meshes; a quality
        // change only replaces the lawn.
        for e in &old {
            commands.entity(e).despawn();
        }
        lawn.key = Some(key);
        lawn.model = Some(LawnModel::load());
        lawn.chunks = match &lawn.model {
            Some(Some(model)) => lay_out(&scene, &content, model.side),
            _ => Vec::new(),
        };
        let tiles: usize = lawn.chunks.iter().map(|c| c.tiles.len()).sum();
        info!("Laid {tiles} lawn tiles in {} chunks", lawn.chunks.len());
        lawn.since = f32::MAX;
    }
    lawn.since += time.delta_secs();
    if lawn.since < 0.15 {
        return;
    }
    lawn.since = 0.0;
    let Some(Some(model)) = &lawn.model else {
        return;
    };
    let material = lawn
        .material
        .get_or_insert_with(|| {
            materials.add(wind_material(
                StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.9,
                    reflectance: 0.08,
                    double_sided: false,
                    cull_mode: None,
                    ..default()
                },
                Wind::new(0.03, 1.9, 0.3),
            ))
        })
        .clone();
    let Ok(camera) = cameras.single() else { return };
    let eye = Vec2::new(camera.translation().x, camera.translation().z);
    let reach = reach(quality);
    for chunk in &mut lawn.chunks {
        let distance = chunk.centre.distance(eye);
        if chunk.entity.is_none() && distance < reach {
            let Some(mesh) = build(&chunk.tiles, model, &scene.layout) else {
                continue;
            };
            chunk.entity = Some(
                commands
                    .spawn((
                        SceneEntity,
                        LawnMesh,
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(material.clone()),
                        NotShadowCaster,
                    ))
                    .id(),
            );
        } else if distance > reach + SLACK {
            if let Some(e) = chunk.entity.take() {
                commands.entity(e).despawn();
            }
        }
    }
}

/// Whether the lawn grows at `p`.
fn grows(layout: &RegionLayout, p: Vec2) -> bool {
    if layout
        .fields
        .as_ref()
        .is_some_and(|f| f.in_planted_area(p, 0.3))
        || layout.water_level_at(p).is_some()
        || layout.distance_to_path(p) < layout.path_half_width - 0.15
    {
        return false;
    }
    !(layout
        .trees
        .iter()
        .any(|t| t.position.distance(p) < t.trunk_radius + 0.1)
        || layout
            .rocks
            .iter()
            .any(|r| r.position.distance(p) < r.radius * 0.85)
        || layout
            .landmarks
            .iter()
            .any(|l| l.position.distance(p) < l.footprint))
}

/// Tiles of `side` metres over the walkable area (up to the fence), in
/// chunks.
fn lay_out(scene: &RegionScene, content: &Content, side: f32) -> Vec<Chunk> {
    let layout = &scene.layout;
    let mut rng = SplitMix64::new(content.regions[scene.region].seed ^ 0x1A_3E);
    let min = layout.bounds_min - Vec2::splat(1.0);
    let max = layout.bounds_max + Vec2::splat(1.0);
    let span = side * CHUNK_TILES as f32;
    let mut chunks = Vec::new();
    let mut cz = min.y;
    while cz < max.y {
        let mut cx = min.x;
        while cx < max.x {
            let lo = Vec2::new(cx, cz);
            let mut tiles = Vec::new();
            for j in 0..CHUNK_TILES {
                for i in 0..CHUNK_TILES {
                    let at = lo + (Vec2::new(i as f32, j as f32) + 0.5) * side;
                    let turn = rng.below(4) as u8;
                    if at.x > max.x || at.y > max.y {
                        continue;
                    }
                    // Corners, edge middles and centre.
                    let on = (0..9)
                        .filter(|k| {
                            let o = Vec2::new((k % 3) as f32 - 1.0, (k / 3) as f32 - 1.0);
                            grows(layout, at + o * (side / 2.0))
                        })
                        .count();
                    if on > 0 {
                        tiles.push(Tile {
                            at,
                            turn,
                            partial: on < 9,
                        });
                    }
                }
            }
            if !tiles.is_empty() {
                chunks.push(Chunk {
                    centre: lo + Vec2::splat(span / 2.0),
                    tiles,
                    entity: None,
                });
            }
            cx += span;
        }
        cz += span;
    }
    chunks
}

/// One chunk's tiles, laid over the terrain.
fn build(tiles: &[Tile], model: &LawnModel, layout: &RegionLayout) -> Option<Mesh> {
    let tile_model = &model.tile;
    let (mut positions, mut normals, mut colors, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::<u32>::new());
    for tile in tiles {
        let turn = Quat::from_rotation_y(tile.turn as f32 * std::f32::consts::FRAC_PI_2);
        let base = positions.len() as u32;
        for ((p, n), c) in tile_model
            .positions
            .iter()
            .zip(&tile_model.normals)
            .zip(&tile_model.colors)
        {
            let local = turn * Vec3::new(p.x, 0.0, p.z);
            let at = tile.at + Vec2::new(local.x, local.z);
            let y = layout.terrain.height_at(at) + p.y - 0.01;
            positions.push([at.x, y, at.y]);
            normals.push((turn * *n).to_array());
            colors.push(*c);
        }
        for t in tile_model.indices.chunks_exact(3) {
            if tile.partial {
                let middle = t
                    .iter()
                    .map(|&i| {
                        let v = positions[(base + i) as usize];
                        Vec2::new(v[0], v[2])
                    })
                    .sum::<Vec2>()
                    / 3.0;
                if !grows(layout, middle) {
                    continue;
                }
            }
            indices.extend(t.iter().map(|&i| base + i));
        }
    }
    if indices.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lawn_model_loads() {
        let model = LawnModel::load().expect("assets/lawn/lawn.glb");
        assert!(model.side > 0.0);
        assert!(!model.tile.indices.is_empty());
        // Sway weights: 0 at the ground, up to 1 at the tips.
        assert!(model
            .tile
            .colors
            .iter()
            .all(|c| (0.0..=1.0).contains(&c[3])));
    }
}
