//! The colour carpet of a bulb farm's beds.
//!
//! A real tulip bed holds dozens of blooms per metre; the game's beds hold a
//! couple of thousand pickable flowers in all. So every pickable bed flower
//! is drawn as a *clump*: the real, pickable model plus a handful of simple
//! decorative blooms of its species packed around it. Picking the flower
//! takes its whole clump with it, so what you see always matches what is
//! left to pick (strays stand alone).
//!
//! Each bed is one merged, wind-swayed mesh, rebuilt only when one of its
//! flowers is picked.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bloom_core::bitset::BitSet;
use bloom_core::data::FlowerShape;
use bloom_core::rng::SplitMix64;

use super::color;
use super::meshes::lin;
use super::scene::SceneEntity;
use super::wind::{wind_material, Wind, WindMaterial};
use crate::game::{Content, RegionScene, Session};

pub struct BedsPlugin;

impl Plugin for BedsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Carpet>().add_systems(
            Update,
            (
                keep_carpet.run_if(resource_exists::<RegionScene>),
                spawn_far_fields.run_if(resource_exists_and_changed::<RegionScene>),
            ),
        );
    }
}

/// Decorative blooms per metre of bed.
const BLOOMS_PER_METRE: f32 = 42.0;

#[derive(Clone, Copy)]
struct Bloom {
    ground: Vec3,
    /// The pickable flower this bloom belongs to.
    owner: u32,
    height: f32,
    yaw: f32,
    size: f32,
    lean: Vec2,
    tint: f32,
}

#[derive(Default)]
struct BedBlooms {
    blooms: Vec<Bloom>,
    owners: Vec<u32>,
    /// Unpicked owners when the mesh was last built.
    built_with: usize,
    entity: Option<Entity>,
}

#[derive(Resource, Default)]
struct Carpet {
    /// (session generation, region) the carpet was laid out for.
    key: Option<(u64, usize)>,
    beds: Vec<BedBlooms>,
    material: Option<Handle<WindMaterial>>,
    since: f32,
}

#[derive(Component)]
struct BedCarpet;

#[allow(clippy::too_many_arguments)]
fn keep_carpet(
    mut commands: Commands,
    time: Res<Time<Real>>,
    scene: Res<RegionScene>,
    session: Option<Res<Session>>,
    content: Res<Content>,
    mut carpet: ResMut<Carpet>,
    old: Query<Entity, With<BedCarpet>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
) {
    let Some(session) = session else { return };
    let key = (session.generation, scene.region);
    let carpet = &mut *carpet;
    if carpet.key != Some(key) || scene.is_changed() {
        for e in &old {
            commands.entity(e).despawn();
        }
        carpet.key = Some(key);
        carpet.beds = lay_out(&scene, &content);
        carpet.since = f32::MAX;
        if carpet.material.is_none() {
            carpet.material = Some(materials.add(wind_material(
                StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.72,
                    reflectance: 0.2,
                    double_sided: false,
                    cull_mode: None,
                    ..default()
                },
                Wind::new(0.06, 1.7, 0.25),
            )));
        }
    }
    carpet.since += time.delta_secs();
    if carpet.since < 0.2 {
        return;
    }
    carpet.since = 0.0;
    let material = carpet.material.clone().expect("made above");
    let region = scene.region;
    for bed in &mut carpet.beds {
        let unpicked = bed
            .owners
            .iter()
            .filter(|&&i| !session.is_harvested(region, i))
            .count();
        if bed.entity.is_some() && unpicked == bed.built_with {
            continue;
        }
        bed.built_with = unpicked;
        if let Some(e) = bed.entity.take() {
            commands.entity(e).despawn();
        }
        let mesh = build_mesh(bed, |i| !session.is_harvested(region, i), &scene, &content);
        if let Some(mesh) = mesh {
            bed.entity = Some(
                commands
                    .spawn((
                        SceneEntity,
                        BedCarpet,
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(material.clone()),
                        NotShadowCaster,
                    ))
                    .id(),
            );
        }
    }
}

/// Spread decorative blooms over every bed and give each to its nearest
/// pickable (non-stray) flower.
fn lay_out(scene: &RegionScene, content: &Content) -> Vec<BedBlooms> {
    let (Some(plan), Some(def)) = (
        scene.layout.fields.as_ref(),
        content.regions[scene.region]
            .scenery
            .features
            .fields
            .as_ref(),
    ) else {
        return Vec::new();
    };
    let field = &scene.field;
    let none = BitSet::new(field.len() as u32);
    let stray = |i: u32| {
        def.stray_species
            .iter()
            .any(|s| *s == content.species[field.species(i)].id)
    };
    let mut rng = SplitMix64::new(content.regions[scene.region].seed ^ 0xBED5);
    let usable = plan.bed_width * 0.86;
    let mut out = Vec::new();
    for bed in &plan.beds {
        let mut b = BedBlooms::default();
        let length = bed.z1 - bed.z0 - 0.3;
        let count = (length * BLOOMS_PER_METRE) as usize;
        for _ in 0..count {
            let p = Vec2::new(
                bed.x + rng.range_f32(-usable / 2.0, usable / 2.0),
                rng.range_f32(bed.z0 + 0.15, bed.z1 - 0.15),
            );
            let owner = field
                .query_radius(p, 1.3, &none)
                .into_iter()
                .filter(|&i| !stray(i))
                .min_by(|&a, &c| {
                    field
                        .position(a)
                        .distance_squared(p)
                        .total_cmp(&field.position(c).distance_squared(p))
                });
            let Some(owner) = owner else { continue };
            // Not right on top of the real flower.
            if field.position(owner).distance(p) < 0.12 {
                continue;
            }
            let species = &content.species[field.species(owner)];
            let height = species.height * field.scale(owner) * rng.range_f32(0.72, 1.0);
            b.blooms.push(Bloom {
                ground: Vec3::new(p.x, scene.layout.terrain.height_at(p), p.y),
                owner,
                height,
                yaw: rng.range_f32(0.0, std::f32::consts::TAU),
                size: rng.range_f32(0.8, 1.15),
                lean: Vec2::new(rng.range_f32(-0.12, 0.12), rng.range_f32(-0.12, 0.12)),
                tint: rng.range_f32(-1.0, 1.0),
            });
            if !b.owners.contains(&owner) {
                b.owners.push(owner);
            }
        }
        out.push(b);
    }
    out
}

#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    /// A vertex; `sway` (0 at the ground, 1 at the bloom) rides in alpha.
    fn v(&mut self, p: Vec3, c: LinearRgba, sway: f32) -> u32 {
        self.positions.push(p.to_array());
        self.normals.push([0.0, 1.0, 0.0]);
        self.colors.push([c.red, c.green, c.blue, sway]);
        self.positions.len() as u32 - 1
    }

    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }

    fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.indices.extend([a, b, c, a, c, d]);
    }
}

fn shade(c: LinearRgba, k: f32) -> LinearRgba {
    LinearRgba::rgb(c.red * k, c.green * k, c.blue * k)
}

fn build_mesh(
    bed: &BedBlooms,
    unpicked: impl Fn(u32) -> bool,
    scene: &RegionScene,
    content: &Content,
) -> Option<Mesh> {
    let field = &scene.field;
    let stem = lin(Color::srgb_u8(86, 128, 70));
    let scale = content.world.flower_scale;
    let mut m = Builder::default();
    for bloom in bed.blooms.iter().filter(|b| unpicked(b.owner)) {
        let species = &content.species[field.species(bloom.owner)];
        let petal = shade(lin(color(species.petal_color)), 1.0 + bloom.tint * 0.08);
        let centre = lin(color(species.center_color));
        let top = bloom.ground
            + Vec3::new(
                bloom.lean.x * bloom.height,
                bloom.height,
                bloom.lean.y * bloom.height,
            );
        let (s, c) = bloom.yaw.sin_cos();
        let side = Vec3::new(c, 0.0, -s);
        let fwd = Vec3::new(s, 0.0, c);

        // Stem: a thin ribbon from the ground to the bloom.
        let w = 0.012 * scale;
        let a = m.v(bloom.ground - side * w, stem, 0.0);
        let b = m.v(bloom.ground + side * w, stem, 0.0);
        let cc = m.v(top + side * w * 0.6, stem, 1.0);
        let d = m.v(top - side * w * 0.6, stem, 1.0);
        m.quad(a, b, cc, d);

        let r = 0.034 * scale * bloom.size;
        let ring = |k: usize, n: usize, radius: f32, y: f32| {
            let t = (k as f32 / n as f32) * std::f32::consts::TAU + bloom.yaw;
            top + Vec3::new(t.cos() * radius, y, t.sin() * radius)
        };
        match species.shape {
            // An upright spike of florets.
            FlowerShape::Spike | FlowerShape::Foxglove | FlowerShape::Bells => {
                let h = r * 3.0;
                for axis in [side, fwd] {
                    let a = m.v(top - axis * r * 0.45, shade(petal, 0.75), 1.0);
                    let b = m.v(top + axis * r * 0.45, shade(petal, 0.75), 1.0);
                    let cc = m.v(top + axis * r * 0.12 + Vec3::Y * h, shade(petal, 1.05), 1.0);
                    let d = m.v(top - axis * r * 0.12 + Vec3::Y * h, shade(petal, 1.05), 1.0);
                    m.quad(a, b, cc, d);
                }
            }
            // A round head.
            FlowerShape::Pompom
            | FlowerShape::Mophead
            | FlowerShape::Ruffle
            | FlowerShape::Rose
            | FlowerShape::StarCluster => {
                let r = r * 1.15;
                let mid = top + Vec3::Y * r;
                let up = m.v(mid + Vec3::Y * r, shade(petal, 1.08), 1.0);
                let down = m.v(mid - Vec3::Y * r * 0.9, shade(petal, 0.7), 1.0);
                let around: Vec<u32> = (0..5)
                    .map(|k| {
                        let p = ring(k, 5, r, r);
                        m.v(p, petal, 1.0)
                    })
                    .collect();
                for k in 0..5 {
                    let (p, q) = (around[k], around[(k + 1) % 5]);
                    m.tri(up, p, q);
                    m.tri(down, q, p);
                }
            }
            // An open, flat face.
            FlowerShape::Daisy
            | FlowerShape::Sunflower
            | FlowerShape::Star
            | FlowerShape::Moonbloom
            | FlowerShape::Orchid => {
                let mid = m.v(top + Vec3::Y * 0.01, centre, 1.0);
                let around: Vec<u32> = (0..6)
                    .map(|k| {
                        let p = ring(k, 6, r * 1.3, -r * 0.15);
                        m.v(p, petal, 1.0)
                    })
                    .collect();
                for k in 0..6 {
                    m.tri(mid, around[k], around[(k + 1) % 6]);
                }
            }
            // A tulip-like cup.
            _ => {
                let low: Vec<u32> = (0..5)
                    .map(|k| {
                        let p = ring(k, 5, r * 0.45, 0.0);
                        m.v(p, shade(petal, 0.72), 1.0)
                    })
                    .collect();
                let high: Vec<u32> = (0..5)
                    .map(|k| {
                        let p = ring(k, 5, r, r * 1.5);
                        m.v(p, shade(petal, 1.06), 1.0)
                    })
                    .collect();
                for k in 0..5 {
                    let j = (k + 1) % 5;
                    m.quad(low[k], low[j], high[j], high[k]);
                }
                let base = m.v(top - Vec3::Y * r * 0.2, shade(petal, 0.6), 1.0);
                for k in 0..5 {
                    m.tri(base, low[(k + 1) % 5], low[k]);
                }
            }
        }
    }
    if m.indices.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, m.colors)
        .with_inserted_indices(Indices::U32(m.indices)),
    )
}

/// Colours of the neighbours' fields (sRGB): tulips and hyacinths in bloom.
const FAR_COLOURS: [(u8, u8, u8); 11] = [
    (214, 44, 132),  // magenta
    (240, 112, 170), // pink
    (210, 34, 44),   // red
    (244, 122, 34),  // orange
    (250, 206, 46),  // yellow
    (118, 52, 168),  // purple
    (246, 240, 232), // white
    (78, 92, 206),   // hyacinth blue
    (172, 118, 214), // lilac
    (232, 60, 70),   // scarlet
    (255, 170, 60),  // apricot
];

/// Beyond the fence the farmland goes on to the horizon: blocks of beds in
/// bloom (a few green or bare), split by grass strips, with lines of trees
/// and the odd barn far away. Pure scenery, out of reach.
fn spawn_far_fields(
    mut commands: Commands,
    scene: Res<RegionScene>,
    content: Res<Content>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let region = &content.regions[scene.region];
    if scene.layout.fields.is_none() {
        return;
    }
    let layout = &scene.layout;
    let terrain = &layout.terrain;
    let started = std::time::Instant::now();
    let mut rng = SplitMix64::new(region.seed ^ 0xFA2F_1E1D);
    let painted = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        reflectance: 0.1,
        cull_mode: None,
        ..default()
    });
    let grass = lin(color(region.palette.ground));
    let soil = lin(Color::srgb_u8(170, 140, 102));
    let leaf = lin(Color::srgb_u8(70, 108, 56));
    let fence = layout.half_size + Vec2::splat(2.0);
    let edge = layout.half_size + Vec2::splat(bloom_core::world::layout::OUTSKIRTS);
    const REACH: f32 = 700.0;

    // Ground beyond the terrain: a ring of grass at the terrain's edge height.
    let mut ground = Builder::default();
    let cell = 50.0;
    let mut z = -REACH;
    while z < REACH {
        let mut x = -REACH;
        while x < REACH {
            let (lo, hi) = (Vec2::new(x, z), Vec2::new(x + cell, z + cell));
            let inside = lo.x > -edge.x && hi.x < edge.x && lo.y > -edge.y && hi.y < edge.y;
            if !inside {
                let corner = |p: Vec2| {
                    // Tuck under the terrain where they overlap.
                    let inner = p.x.abs() < edge.x && p.y.abs() < edge.y;
                    Vec3::new(
                        p.x,
                        terrain.height_at(p) - if inner { 0.3 } else { 0.02 },
                        p.y,
                    )
                };
                let a = ground.v(corner(lo), grass, 0.0);
                let b = ground.v(corner(Vec2::new(hi.x, lo.y)), grass, 0.0);
                let c = ground.v(corner(hi), grass, 0.0);
                let d = ground.v(corner(Vec2::new(lo.x, hi.y)), grass, 0.0);
                ground.quad(a, d, c, b);
            }
            x += cell;
        }
        z += cell;
    }

    // Blocks of beds: 64 m wide, 120 m long, 8 m of grass between.
    let mut beds = Builder::default();
    let (bw, bl, gap) = (64.0, 120.0, 8.0);
    let pitch = 1.9;
    let mut bz = -REACH;
    while bz < REACH {
        let mut bx = -REACH + rng.range_f32(-20.0, 0.0);
        while bx < REACH {
            let lo = Vec2::new(bx, bz);
            let hi = lo + Vec2::new(bw, bl);
            bx += bw + gap;
            // Clear of the farm itself (and its windbreak in the south).
            let clear =
                hi.x < -fence.x || lo.x > fence.x || hi.y < -fence.y || lo.y > fence.y + 26.0;
            if !clear {
                // Trim the block to whatever lies outside the fence.
                let parts = [
                    (lo, Vec2::new(hi.x.min(-fence.x), hi.y)),
                    (Vec2::new(lo.x.max(fence.x), lo.y), hi),
                    (
                        Vec2::new(lo.x.max(-fence.x), lo.y),
                        Vec2::new(hi.x.min(fence.x), hi.y.min(-fence.y)),
                    ),
                ];
                for (a, b) in parts {
                    if b.x - a.x > 6.0 && b.y - a.y > 6.0 {
                        far_block(&mut beds, &mut rng, a, b, pitch, terrain, leaf, soil);
                    }
                }
                continue;
            }
            far_block(&mut beds, &mut rng, lo, hi, pitch, terrain, leaf, soil);
        }
        bz += bl + gap;
    }
    for b in [ground, beds] {
        if b.indices.is_empty() {
            continue;
        }
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, b.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, b.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, b.colors)
        .with_inserted_indices(Indices::U32(b.indices));
        commands.spawn((
            SceneEntity,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(painted.clone()),
            NotShadowCaster,
        ));
    }

    // Windbreaks: rows of trees along a few block edges, and some barns.
    let trees = super::meshes::tree_variants(&region.palette).map(|m| meshes.add(m));
    let mut rows: Vec<(Vec2, Vec2)> = Vec::new();
    for _ in 0..9 {
        let horizontal = rng.next_f32() < 0.6;
        let at = rng.range_f32(140.0, 520.0) * if rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
        let from = rng.range_f32(-500.0, 100.0);
        let len = rng.range_f32(120.0, 380.0);
        rows.push(if horizontal {
            (Vec2::new(from, at), Vec2::new(from + len, at))
        } else {
            (Vec2::new(at, from), Vec2::new(at, from + len))
        });
    }
    // Always one behind the far end of the farm, where the player looks.
    rows.push((Vec2::new(-260.0, -330.0), Vec2::new(240.0, -330.0)));
    let mut count = 0;
    for (a, b) in rows {
        let n = (a.distance(b) / 9.0) as usize;
        for k in 0..n {
            let p = a.lerp(b, k as f32 / n as f32)
                + Vec2::new(rng.range_f32(-2.0, 2.0), rng.range_f32(-2.0, 2.0));
            if p.x.abs() < fence.x + 40.0 && p.y.abs() < fence.y + 40.0 {
                continue;
            }
            let h = rng.range_f32(9.0, 15.0);
            commands.spawn((
                SceneEntity,
                Mesh3d(trees[rng.below(3) as usize].clone()),
                MeshMaterial3d(painted.clone()),
                Transform::from_xyz(p.x, terrain.height_at(p) - 0.2, p.y)
                    .with_rotation(Quat::from_rotation_y(
                        rng.range_f32(0.0, std::f32::consts::TAU),
                    ))
                    .with_scale(Vec3::splat(h / super::meshes::TREE_MODEL_HEIGHT)),
                NotShadowCaster,
            ));
            count += 1;
        }
    }
    let barn = meshes.add(barn_mesh());
    for spot in [
        Vec2::new(-190.0, -300.0),
        Vec2::new(230.0, -180.0),
        Vec2::new(-320.0, 120.0),
    ] {
        commands.spawn((
            SceneEntity,
            Mesh3d(barn.clone()),
            MeshMaterial3d(painted.clone()),
            Transform::from_xyz(spot.x, terrain.height_at(spot), spot.y)
                .with_rotation(Quat::from_rotation_y(rng.range_f32(-0.3, 0.3))),
            NotShadowCaster,
        ));
    }
    info!(
        "Laid out the far fields in {:.1} ms ({count} trees)",
        started.elapsed().as_secs_f64() * 1000.0
    );
}

/// One block of beds running north-south (or east-west), mostly in bands of
/// a single colour.
#[allow(clippy::too_many_arguments)]
fn far_block(
    b: &mut Builder,
    rng: &mut SplitMix64,
    lo: Vec2,
    hi: Vec2,
    pitch: f32,
    terrain: &bloom_core::world::Terrain,
    leaf: LinearRgba,
    soil: LinearRgba,
) {
    let roll = rng.next_f32();
    let along_z = rng.next_f32() < 0.75;
    let (across_len, along_lo, along_hi) = if along_z {
        (hi.x - lo.x, lo.y, hi.y)
    } else {
        (hi.y - lo.y, lo.x, hi.x)
    };
    let n = (across_len / pitch) as usize;
    let pick = |rng: &mut SplitMix64| {
        let c = FAR_COLOURS[rng.below(FAR_COLOURS.len() as u32) as usize];
        lin(Color::srgb_u8(c.0, c.1, c.2))
    };
    let mut colour = pick(rng);
    let mut band_left = 4 + rng.below(14) as usize;
    let near = lo.length().min(hi.length()) < 140.0;
    let segments = if near {
        ((along_hi - along_lo) / 8.0).ceil() as usize
    } else {
        1
    };
    for k in 0..n {
        if band_left == 0 {
            colour = pick(rng);
            band_left = 4 + rng.below(14) as usize;
        }
        band_left -= 1;
        // A few blocks only in leaf, or freshly turned.
        let top = if roll < 0.12 {
            leaf
        } else if roll < 0.16 {
            soil
        } else {
            shade(colour, 0.92 + rng.next_f32() * 0.14)
        };
        let off = (if along_z { lo.x } else { lo.y }) + k as f32 * pitch + 0.35;
        let width = 1.2;
        for s in 0..segments {
            let t0 = along_lo + (along_hi - along_lo) * s as f32 / segments as f32;
            let t1 = along_lo + (along_hi - along_lo) * (s + 1) as f32 / segments as f32;
            let pt = |across: f32, along: f32| {
                if along_z {
                    Vec2::new(across, along)
                } else {
                    Vec2::new(along, across)
                }
            };
            let lift = 0.42;
            let v3 = |p: Vec2, up: f32| Vec3::new(p.x, terrain.height_at(p) + up, p.y);
            let (a0, a1) = (pt(off, t0), pt(off + width, t0));
            let (b0, b1) = (pt(off, t1), pt(off + width, t1));
            // The flowering top and the leafy sides.
            let i = b.v(v3(a0, lift), top, 0.0);
            let j = b.v(v3(a1, lift), top, 0.0);
            let kk = b.v(v3(b1, lift), top, 0.0);
            let l = b.v(v3(b0, lift), top, 0.0);
            b.quad(i, l, kk, j);
            for (p, q) in [(a0, b0), (a1, b1)] {
                let i = b.v(v3(p, 0.0), shade(leaf, 0.8), 0.0);
                let j = b.v(v3(q, 0.0), shade(leaf, 0.8), 0.0);
                let kk = b.v(v3(q, lift * 0.85), leaf, 0.0);
                let l = b.v(v3(p, lift * 0.85), leaf, 0.0);
                b.quad(i, j, kk, l);
            }
        }
    }
    // Bare soil under the block.
    let y = |p: Vec2| terrain.height_at(p) + 0.03;
    let i = b.v(Vec3::new(lo.x, y(lo), lo.y), soil, 0.0);
    let j = b.v(Vec3::new(hi.x, y(Vec2::new(hi.x, lo.y)), lo.y), soil, 0.0);
    let kk = b.v(Vec3::new(hi.x, y(hi), hi.y), soil, 0.0);
    let l = b.v(Vec3::new(lo.x, y(Vec2::new(lo.x, hi.y)), hi.y), soil, 0.0);
    b.quad(i, l, kk, j);
}

/// A simple Dutch barn: brick walls and a steep dark roof.
fn barn_mesh() -> Mesh {
    use super::meshes::MeshBuilder;
    let mut m = MeshBuilder::default();
    let wall = lin(Color::srgb_u8(150, 72, 56));
    let roof = lin(Color::srgb_u8(58, 54, 56));
    let (w, d, h) = (12.0, 22.0, 4.0);
    m.add(
        &Mesh::from(Cuboid::new(w, h, d)),
        Transform::from_xyz(0.0, h / 2.0, 0.0),
        move |_| wall,
    );
    let slope = (w / 2.0f32).hypot(5.0);
    let angle = (5.0f32).atan2(w / 2.0);
    for side in [1.0f32, -1.0] {
        m.add(
            &Mesh::from(Cuboid::new(slope + 0.6, 0.3, d + 0.8)),
            Transform::from_xyz(side * w / 4.0, h + 2.5, 0.0)
                .with_rotation(Quat::from_rotation_z(-side * angle)),
            move |_| roof,
        );
    }
    // Gable ends.
    m.add(
        &Mesh::from(Cuboid::new(w * 0.55, 3.0, d - 0.2)),
        Transform::from_xyz(0.0, h + 1.3, 0.0),
        move |_| wall,
    );
    m.build()
}
