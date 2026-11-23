//! Decorative meadow grass: thousands of tapered blades merged into 16 m
//! chunk meshes, swaying with the wind shader.
//!
//! Grass is pure decoration. It is never harvestable, never counted, and
//! deliberately leaves room around every flower so flowers stay easy to see
//! and pick. Chunks fade out with distance (visibility ranges) and don't cast
//! shadows, which keeps the cost low.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::VisibilityRange;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bloom_core::bitset::BitSet;
use bloom_core::rng::SplitMix64;

use super::color;
use super::meshes::{lin, mix};
use super::scene::SceneEntity;
use super::wind::{wind_material, Wind, WindMaterial};
use crate::game::{Content, Quality, RegionScene, Settings};

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_grass.run_if(resource_exists::<RegionScene>));
    }
}

const CHUNK: f32 = 16.0;
/// Distance between tuft sites (jittered).
const SPACING: f32 = 0.46;
/// Free radius kept around each flower.
const FLOWER_CLEARANCE: f32 = 0.32;

#[derive(Default)]
struct Blades {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Blades {
    /// One tapered, slightly bent blade. Vertex alpha carries the sway weight.
    fn blade(
        &mut self,
        root: Vec3,
        yaw: f32,
        height: f32,
        width: f32,
        lean: Vec2,
        base: LinearRgba,
        tip: LinearRgba,
    ) {
        let axis = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * (width / 2.0);
        let bend = Vec3::new(lean.x, 0.0, lean.y);
        let mid = root + Vec3::Y * height * 0.55 + bend * height * 0.3;
        let top = root + Vec3::Y * height + bend * height;
        let mid_col = mix(base, tip, 0.55);
        let i = self.positions.len() as u32;
        for (p, c, a) in [
            (root - axis, base, 0.0),
            (root + axis, base, 0.0),
            (mid - axis * 0.72, mid_col, 0.45),
            (mid + axis * 0.72, mid_col, 0.45),
            (top, tip, 1.0),
        ] {
            self.positions.push(p.to_array());
            // Upward normals light the whole field softly and evenly.
            self.normals.push([0.0, 1.0, 0.0]);
            self.colors.push([c.red, c.green, c.blue, a]);
        }
        self.indices
            .extend([i, i + 2, i + 1, i + 1, i + 2, i + 3, i + 2, i + 4, i + 3]);
    }

    fn into_mesh(self) -> Option<Mesh> {
        if self.indices.is_empty() {
            return None;
        }
        Some(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices)),
        )
    }
}

/// Smooth organic patchiness in [0, 1].
fn patches(p: Vec2) -> f32 {
    ((p.x * 0.11 + (p.y * 0.07).sin() * 2.3).sin() * 0.5 + (p.y * 0.13 + p.x * 0.05).cos() * 0.5)
        * 0.5
        + 0.5
}

/// A chunk of grass (rebuilt when the region or the graphics quality
/// changes).
#[derive(Component)]
struct GrassChunk;

#[allow(clippy::too_many_arguments)]
fn spawn_grass(
    mut commands: Commands,
    scene: Res<RegionScene>,
    settings: Res<Settings>,
    mut built: Local<Option<(usize, Quality)>>,
    old: Query<Entity, With<GrassChunk>>,
    content: Res<Content>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
) {
    let quality = settings.quality;
    if !scene.is_changed() && *built == Some((scene.region, quality)) {
        return;
    }
    *built = Some((scene.region, quality));
    // A new region's scene already cleared the old one; a quality change
    // only replaces the grass.
    if !scene.is_changed() {
        for e in &old {
            commands.entity(e).despawn();
        }
    }
    let started = std::time::Instant::now();
    let layout = &scene.layout;
    let field = &scene.field;
    let region = &content.regions[scene.region];
    let palette = region.palette;
    let grass_density = region.scenery.grass_density() * quality.grass_density();
    let (fade_from, fade_to) = quality.grass_distance();
    let none_picked = BitSet::new(field.len() as u32);

    let dark = lin(color(palette.ground_dark)) * 0.82;
    let mid = lin(color(palette.ground));
    let light = lin(color(palette.ground_light));
    let straw = lin(color(palette.path));

    let material = materials.add(wind_material(
        StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            reflectance: 0.08,
            // Both sides drawn, but normals are *not* flipped for back
            // faces: blades keep their upward normals and soft, even light.
            double_sided: false,
            cull_mode: None,
            ..default()
        },
        Wind::new(0.07, 1.9, 0.3),
    ));

    // Bulb beds carry broad, blue-green leaves instead of grass.
    let leaf_base = lin(Color::srgb_u8(58, 92, 52));
    let leaf_tip = lin(Color::srgb_u8(112, 152, 92));
    let fields = layout.fields.as_ref();

    // Grass grows up to the fence.
    let min = layout.bounds_min - Vec2::splat(1.0);
    let max = layout.bounds_max + Vec2::splat(1.0);
    let (mut chunks, mut blades_total) = (0, 0usize);
    let mut cz = min.y;
    while cz < max.y {
        let mut cx = min.x;
        while cx < max.x {
            let lo = Vec2::new(cx, cz);
            let hi = (lo + Vec2::splat(CHUNK)).min(max);
            let mut rng =
                SplitMix64::new(region.seed ^ ((cx.to_bits() as u64) << 32 | cz.to_bits() as u64));
            let mut b = Blades::default();
            let mut z = lo.y;
            while z < hi.y {
                let mut x = lo.x;
                while x < hi.x {
                    let p = Vec2::new(
                        x + rng.range_f32(0.0, SPACING),
                        z + rng.range_f32(0.0, SPACING),
                    );
                    x += SPACING;
                    let planted = fields.filter(|f| f.in_planted_area(p, 0.3));
                    if let Some(plan) = planted {
                        // Bare furrows; leaves on the beds, close around
                        // every flower (picked or not) like a real crop.
                        if plan.bed_weight(p) < 0.6
                            || rng.next_f32() > quality.grass_density().max(0.5)
                            || !field.query_radius(p, 0.12, &none_picked).is_empty()
                        {
                            continue;
                        }
                        let ground = layout.terrain.height_at(p);
                        let root = Vec3::new(p.x, ground - 0.02, p.y);
                        let tint = rng.next_f32();
                        for _ in 0..2 + rng.below(2) {
                            let lean =
                                Vec2::new(rng.range_f32(-0.3, 0.3), rng.range_f32(-0.3, 0.3));
                            let offset =
                                Vec3::new(rng.range_f32(-0.1, 0.1), 0.0, rng.range_f32(-0.1, 0.1));
                            b.blade(
                                root + offset,
                                rng.range_f32(0.0, std::f32::consts::TAU),
                                rng.range_f32(0.24, 0.42),
                                rng.range_f32(0.07, 0.1),
                                lean,
                                mix(leaf_base, dark, tint * 0.3),
                                mix(leaf_tip, mid, tint * 0.3),
                            );
                            blades_total += 1;
                        }
                        continue;
                    }
                    let density = (0.55 + 0.45 * patches(p)) * grass_density;
                    if rng.next_f32() > density || layout.water_level_at(p).is_some() {
                        continue;
                    }
                    let path = layout.distance_to_path(p);
                    if path < layout.path_half_width - 0.2
                        || (path < layout.path_half_width + 0.5 && rng.next_f32() < 0.6)
                    {
                        continue;
                    }
                    if layout
                        .trees
                        .iter()
                        .any(|t| t.position.distance(p) < t.trunk_radius + 0.12)
                        || layout
                            .rocks
                            .iter()
                            .any(|r| r.position.distance(p) < r.radius * 0.8)
                        || layout
                            .landmarks
                            .iter()
                            .any(|l| l.position.distance(p) < l.footprint)
                        || !field
                            .query_radius(p, FLOWER_CLEARANCE, &none_picked)
                            .is_empty()
                    {
                        continue;
                    }
                    let ground = layout.terrain.height_at(p);
                    let root = Vec3::new(p.x, ground - 0.02, p.y);
                    let lush = patches(p * 1.7);
                    let tint = rng.next_f32();
                    let base = mix(mix(dark, mid, 0.55), mid, tint * 0.5);
                    let tip = if rng.next_f32() < 0.06 {
                        mix(light, straw, 0.6)
                    } else {
                        mix(mid, light, 0.45 + tint * 0.55)
                    };
                    let blades = 4 + rng.below(3);
                    let tuft_lean =
                        Vec2::new(rng.range_f32(-0.25, 0.25), rng.range_f32(-0.25, 0.25));
                    for _ in 0..blades {
                        let height = rng.range_f32(0.16, 0.34) * (0.8 + lush * 0.5);
                        let lean = tuft_lean
                            + Vec2::new(rng.range_f32(-0.2, 0.2), rng.range_f32(-0.2, 0.2));
                        let offset =
                            Vec3::new(rng.range_f32(-0.06, 0.06), 0.0, rng.range_f32(-0.06, 0.06));
                        b.blade(
                            root + offset,
                            rng.range_f32(0.0, std::f32::consts::TAU),
                            height,
                            0.03,
                            lean,
                            base,
                            tip,
                        );
                        blades_total += 1;
                    }
                }
                z += SPACING;
            }
            if let Some(mesh) = b.into_mesh() {
                commands.spawn((
                    SceneEntity,
                    GrassChunk,
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(material.clone()),
                    NotShadowCaster,
                    VisibilityRange {
                        start_margin: 0.0..0.0,
                        end_margin: fade_from..fade_to,
                        use_aabb: true,
                    },
                ));
                chunks += 1;
            }
            cx += CHUNK;
        }
        cz += CHUNK;
    }
    info!(
        "Grew {blades_total} grass blades in {chunks} chunks in {:.1} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
}
