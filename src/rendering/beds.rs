//! The colour carpet of a bulb farm's beds.
//!
//! A real tulip bed holds dozens of blooms per metre; the game's beds hold a
//! couple of thousand pickable flowers in all. So every pickable bed flower
//! is drawn as a *clump*: the real, pickable model plus a handful of simple
//! decorative blooms of its species packed around it. Picking the flower
//! takes its whole clump with it, so what you see always matches what is
//! left to pick (strays stand alone). A bed of a species with an imported
//! model (`flowers::imported`) is planted in neat rows of copies of it
//! instead, spaced so the plants don't grow into each other: the full model
//! near the player, simpler versions further away.
//!
//! Each bed is drawn in pieces, each one merged, wind-swayed mesh, rebuilt
//! only when one of its flowers is picked or the player comes near or
//! leaves.
//!
//! The beds' soil is built from a model of a stone-framed garden bed
//! (`assets/beds/soil.glb`, see `scripts/bed_soil.py`): its start piece, its
//! middle piece repeated as often as the bed is long, and its end piece,
//! laid over the terrain; finely modelled near the player, coarse further
//! away.

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
use crate::flowers::imported::{self, Plant};
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

/// Decorative blooms per metre of bed (species drawn the simple way).
const BLOOMS_PER_METRE: f32 = 42.0;

/// Beds are drawn in pieces this long (metres), each one mesh, so the
/// pieces near the player can use the finer models.
const CHUNK_LENGTH: f32 = 4.0;

/// Pieces of bed closer than these (metres, to their middle) use the full
/// model and the middle version; further away, the simple one. A piece
/// switches back to a coarser one only `SLACK` beyond.
const NEAR: f32 = 7.0;
const MIDDLE: f32 = 18.0;
const SLACK: f32 = 2.0;

/// Pieces of bed closer than this (metres) get the finely modelled soil;
/// they switch back beyond `SOIL_NEAR + SLACK`.
const SOIL_NEAR: f32 = 8.0;

/// The soil model's surface sits this far above the terrain (metres), which
/// stays underneath as backing.
const SOIL_LIFT: f32 = 0.012;

/// Plants of a modelled species stand this much of their width apart, so
/// each stands on its own with soil between the leaves.
const SPACING: f32 = 1.3;

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

/// One piece of a bed, drawn as one mesh.
#[derive(Default)]
struct Chunk {
    /// Middle of the piece (X, Z).
    centre: Vec2,
    blooms: Vec<Bloom>,
    owners: Vec<u32>,
    /// Unpicked owners and level of detail (0 finest) when the mesh was
    /// last built.
    built_with: usize,
    detail: usize,
    entity: Option<Entity>,
    /// Pieces of the soil model laid in this piece of bed.
    soil: Vec<SoilPlacement>,
    /// Whether the soil mesh is the fine one.
    soil_near: bool,
    soil_entity: Option<Entity>,
}

/// The pieces of the bed soil model: start, middle and end, each fine
/// ("near") and coarse ("far"), as made by `scripts/bed_soil.py`.
struct Soil {
    /// `[near, far][start, middle, end]`.
    pieces: [[Plant; 3]; 2],
    /// Length each piece covers along the bed (model units).
    lengths: [f32; 3],
    /// The model's width (model units).
    width: f32,
}

impl Soil {
    fn load() -> Option<Self> {
        let bytes = imported::bed_model("soil")?;
        let named = match imported::parse_named(bytes) {
            Ok(n) => n,
            Err(e) => {
                warn!("Bed soil model unreadable: {e}");
                return None;
            }
        };
        let mut pieces: [[Option<Plant>; 3]; 2] = Default::default();
        let (mut lengths, mut width) = ([0.0f32; 3], 0.0f32);
        for (name, plant) in named {
            // "<piece> <detail> length=<units> width=<units>"
            let words: Vec<&str> = name.split_whitespace().collect();
            let value = |key: &str| {
                words
                    .iter()
                    .find_map(|w| w.strip_prefix(key)?.parse::<f32>().ok())
            };
            let piece = ["start", "middle", "end"]
                .iter()
                .position(|p| words.first() == Some(p));
            let detail = ["near", "far"].iter().position(|d| words.get(1) == Some(d));
            let (Some(piece), Some(detail), Some(length), Some(w)) =
                (piece, detail, value("length="), value("width="))
            else {
                warn!("Bed soil model: unexpected piece {name:?}");
                continue;
            };
            lengths[piece] = length;
            width = w;
            pieces[detail][piece] = Some(plant);
        }
        let [near, far] = pieces;
        let all = |p: [Option<Plant>; 3]| -> Option<[Plant; 3]> {
            let [a, b, c] = p;
            Some([a?, b?, c?])
        };
        let pieces = [all(near)?, all(far)?];
        (width > 0.0 && lengths.iter().all(|&l| l > 0.0)).then_some(Self {
            pieces,
            lengths,
            width,
        })
    }
}

/// One piece of the soil model laid on a bed.
#[derive(Clone, Copy)]
struct SoilPlacement {
    /// 0 start, 1 middle, 2 end.
    piece: usize,
    /// Where the piece begins: the bed's centre line and the piece's south
    /// end (metres).
    x: f32,
    z: f32,
    /// Model units to metres, and the extra stretch along the bed.
    scale: f32,
    stretch: f32,
}

/// The models a species' decorative blooms copy, finest first: the full
/// model near the player, then the middle and the simple version.
struct Clump {
    models: [Plant; 3],
    /// Distance between neighbouring plants (metres, as drawn).
    spacing: f32,
}

impl Clump {
    /// `scale`: how much larger than life flowers are drawn.
    fn load(species: &bloom_core::data::FlowerSpecies, scale: f32) -> Option<Self> {
        let low = imported::load_version(species, "low")?;
        let mid = imported::load_version(species, "mid").unwrap_or_else(|| low.clone());
        let full = imported::load_plant(species).unwrap_or_else(|| mid.clone());
        let spacing = (full.reach() * scale * 2.0 * SPACING).clamp(0.1, 1.5);
        Some(Self {
            models: [full, mid, low],
            spacing,
        })
    }
}

#[derive(Resource, Default)]
struct Carpet {
    /// (session generation, region) the carpet was laid out for.
    key: Option<(u64, usize)>,
    chunks: Vec<Chunk>,
    material: Option<Handle<WindMaterial>>,
    /// Per species: the models its decorative blooms copy, if any.
    clumps: Option<Vec<Option<Clump>>>,
    /// The bed soil model (outer `None`: not loaded yet).
    soil: Option<Option<Soil>>,
    soil_material: Option<Handle<StandardMaterial>>,
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
    cameras: Query<&GlobalTransform, With<super::MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
    mut plain_materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(session) = session else { return };
    let key = (session.generation, scene.region);
    let carpet = &mut *carpet;
    let clumps = carpet.clumps.get_or_insert_with(|| {
        let scale = content.world.flower_scale;
        content
            .species
            .iter()
            .map(|s| Clump::load(s, scale))
            .collect()
    });
    let soil = carpet.soil.get_or_insert_with(Soil::load).as_ref();
    if carpet.key != Some(key) || scene.is_changed() {
        for e in &old {
            commands.entity(e).despawn();
        }
        carpet.key = Some(key);
        carpet.chunks = lay_out(&scene, &content, clumps, soil);
        carpet.since = f32::MAX;
        let blooms = carpet.chunks.iter().flat_map(|c| &c.blooms);
        let (count, modelled) = blooms.fold((0, 0), |(n, m), b| {
            let clump = clumps[scene.field.species(b.owner)].as_ref();
            (n + 1, m + usize::from(clump.is_some()))
        });
        info!(
            "Laid out {count} decorative blooms ({modelled} modelled) in {} pieces of bed",
            carpet.chunks.len()
        );
        if carpet.soil_material.is_none() {
            carpet.soil_material = Some(plain_materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.92,
                reflectance: 0.12,
                cull_mode: None,
                ..default()
            }));
        }
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
    let soil_material = carpet.soil_material.clone().expect("made above");
    let eye = cameras
        .single()
        .ok()
        .map(|c| Vec2::new(c.translation().x, c.translation().z));
    let region = scene.region;
    for chunk in &mut carpet.chunks {
        if let Some(soil) = soil.filter(|_| !chunk.soil.is_empty()) {
            let distance = eye.map_or(f32::MAX, |e| e.distance(chunk.centre));
            let near = if chunk.soil_near {
                distance < SOIL_NEAR + SLACK
            } else {
                distance < SOIL_NEAR
            };
            if chunk.soil_entity.is_none() || near != chunk.soil_near {
                chunk.soil_near = near;
                if let Some(e) = chunk.soil_entity.take() {
                    commands.entity(e).despawn();
                }
                let mesh = soil_mesh(&chunk.soil, soil, near, &scene);
                chunk.soil_entity = Some(
                    commands
                        .spawn((
                            SceneEntity,
                            BedCarpet,
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(soil_material.clone()),
                            NotShadowCaster,
                        ))
                        .id(),
                );
            }
        }
        if chunk.blooms.is_empty() {
            continue;
        }
        let unpicked = chunk
            .owners
            .iter()
            .filter(|&&i| !session.is_harvested(region, i))
            .count();
        let distance = eye.map_or(f32::MAX, |e| e.distance(chunk.centre));
        let mut detail = [NEAR, MIDDLE]
            .iter()
            .position(|&d| distance < d)
            .unwrap_or(2);
        // Only coarser once clearly past the edge (no flicker on it).
        if detail > chunk.detail && chunk.entity.is_some() {
            let edge = [NEAR, MIDDLE][chunk.detail];
            if distance < edge + SLACK {
                detail = chunk.detail;
            }
        }
        if chunk.entity.is_some() && unpicked == chunk.built_with && detail == chunk.detail {
            continue;
        }
        chunk.built_with = unpicked;
        chunk.detail = detail;
        if let Some(e) = chunk.entity.take() {
            commands.entity(e).despawn();
        }
        let mesh = build_mesh(
            &chunk.blooms,
            |i| !session.is_harvested(region, i),
            &scene,
            &content,
            clumps,
            detail,
        );
        if let Some(mesh) = mesh {
            chunk.entity = Some(
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

/// Fill every bed with decorative blooms and give each to its nearest
/// pickable (non-stray) flower. A bed of a modelled species is planted in
/// neat staggered rows around its pickable flowers, the plants just far
/// enough apart not to grow into each other; other beds get a dense random
/// carpet of simple blooms.
fn lay_out(
    scene: &RegionScene,
    content: &Content,
    clumps: &[Option<Clump>],
    soil: Option<&Soil>,
) -> Vec<Chunk> {
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
    let owner_near = |p: Vec2| {
        field
            .query_radius(p, 1.3, &none)
            .into_iter()
            .filter(|&i| !stray(i))
            .min_by(|&a, &c| {
                field
                    .position(a)
                    .distance_squared(p)
                    .total_cmp(&field.position(c).distance_squared(p))
            })
    };
    let mut rng = SplitMix64::new(content.regions[scene.region].seed ^ 0xBED5);
    let mut out = Vec::new();
    for bed in &plan.beds {
        let length = bed.z1 - bed.z0;
        let pieces = (length / CHUNK_LENGTH).ceil().max(1.0) as usize;
        let mut chunks: Vec<Chunk> = (0..pieces)
            .map(|k| Chunk {
                centre: Vec2::new(bed.x, bed.z0 + (k as f32 + 0.5) * length / pieces as f32),
                ..default()
            })
            .collect();
        if let Some(soil) = soil {
            for place in lay_soil(soil, bed.x, bed.z0, bed.z1, plan.bed_width) {
                let length = soil.lengths[place.piece] * place.scale * place.stretch;
                let k = (((place.z + length / 2.0 - bed.z0) / length) * pieces as f32) as usize;
                chunks[k.min(pieces - 1)].soil.push(place);
            }
        }
        let mut add = |p: Vec2, bloom: Bloom| {
            let k = (((p.y - bed.z0) / length) * pieces as f32) as usize;
            let chunk = &mut chunks[k.min(pieces - 1)];
            chunk.blooms.push(bloom);
            if !chunk.owners.contains(&bloom.owner) {
                chunk.owners.push(bloom.owner);
            }
        };
        let ground = |p: Vec2| Vec3::new(p.x, scene.layout.terrain.height_at(p), p.y);
        // What grows here: the commonest species among the bed's flowers.
        let middle = Vec2::new(bed.x, (bed.z0 + bed.z1) / 2.0);
        let mut counts = vec![0u32; content.species.len()];
        for i in field.query_radius(middle, length / 2.0 + plan.bed_width, &none) {
            if !stray(i) && plan.bed_at(field.position(i)).is_some_and(|b| b == bed) {
                counts[field.species(i)] += 1;
            }
        }
        let main = (0..counts.len()).max_by_key(|&s| counts[s]).unwrap_or(0);
        if let Some(clump) = clumps.get(main).and_then(Option::as_ref) {
            // Rows across the bed, the outer two on the pickable flowers'
            // planting lines; neighbouring rows staggered.
            let step = clump.spacing;
            let half = plan.bed_width * 0.36;
            let rows = ((2.0 * half / (step * 0.87)).round() as usize + 1).max(2);
            let z_count = ((length - 0.6) / step).floor().max(0.0) as usize;
            for r in 0..rows {
                let x = bed.x - half + 2.0 * half * r as f32 / (rows - 1) as f32;
                let shift = if r % 2 == 1 { step * 0.5 } else { 0.0 };
                for j in 0..z_count {
                    let z = bed.z1 - 0.3 - (j as f32 + 0.5) * step - shift;
                    if z < bed.z0 + 0.3 {
                        continue;
                    }
                    let p = Vec2::new(x, z)
                        + Vec2::new(rng.range_f32(-1.0, 1.0), rng.range_f32(-1.0, 1.0))
                            * (step * 0.06);
                    // Leave room for the pickable flowers.
                    if !field.query_radius(p, step * 0.75, &none).is_empty() {
                        continue;
                    }
                    let Some(owner) = owner_near(p) else { continue };
                    let species = &content.species[field.species(owner)];
                    add(
                        p,
                        Bloom {
                            ground: ground(p),
                            owner,
                            height: species.height * field.scale(owner) * rng.range_f32(0.88, 1.0),
                            yaw: rng.range_f32(0.0, std::f32::consts::TAU),
                            size: 1.0,
                            lean: Vec2::new(rng.range_f32(-0.03, 0.03), rng.range_f32(-0.03, 0.03)),
                            tint: rng.range_f32(-1.0, 1.0),
                        },
                    );
                }
            }
        } else {
            let usable = plan.bed_width * 0.86;
            let count = ((length - 0.3) * BLOOMS_PER_METRE) as usize;
            for _ in 0..count {
                let p = Vec2::new(
                    bed.x + rng.range_f32(-usable / 2.0, usable / 2.0),
                    rng.range_f32(bed.z0 + 0.15, bed.z1 - 0.15),
                );
                let Some(owner) = owner_near(p) else { continue };
                // Not right on top of the real flower.
                if field.position(owner).distance(p) < 0.12 {
                    continue;
                }
                let species = &content.species[field.species(owner)];
                let height = species.height * field.scale(owner) * rng.range_f32(0.72, 1.0);
                add(
                    p,
                    Bloom {
                        ground: ground(p),
                        owner,
                        height,
                        yaw: rng.range_f32(0.0, std::f32::consts::TAU),
                        size: rng.range_f32(0.8, 1.15),
                        lean: Vec2::new(rng.range_f32(-0.12, 0.12), rng.range_f32(-0.12, 0.12)),
                        tint: rng.range_f32(-1.0, 1.0),
                    },
                );
            }
        }
        out.extend(
            chunks
                .into_iter()
                .filter(|c| !c.blooms.is_empty() || !c.soil.is_empty()),
        );
    }
    out
}

/// Cover a bed (`x`: centre line, `z0..z1`, `width` across) with the soil
/// model: the start piece, the middle piece as often as fits, the end
/// piece, the middle ones stretched a little to fill the bed exactly.
fn lay_soil(soil: &Soil, x: f32, z0: f32, z1: f32, width: f32) -> Vec<SoilPlacement> {
    let scale = width / soil.width;
    let [start, middle, end] = soil.lengths.map(|l| l * scale);
    let length = z1 - z0;
    let room = length - start - end;
    let count = (room / middle).round().max(0.0) as usize;
    let (cap_stretch, middle_stretch) = if count == 0 {
        (length / (start + end), 1.0)
    } else {
        (1.0, room / (count as f32 * middle))
    };
    let mut out = Vec::with_capacity(count + 2);
    let mut z = z0;
    let mut put = |piece: usize, stretch: f32, z: &mut f32| {
        out.push(SoilPlacement {
            piece,
            x,
            z: *z,
            scale,
            stretch,
        });
        *z += soil.lengths[piece] * scale * stretch;
    };
    put(0, cap_stretch, &mut z);
    for _ in 0..count {
        put(1, middle_stretch, &mut z);
    }
    put(2, cap_stretch, &mut z);
    out
}

/// The soil of one piece of bed: its placed model pieces laid over the
/// terrain (each vertex follows the ground beneath it).
fn soil_mesh(placed: &[SoilPlacement], soil: &Soil, near: bool, scene: &RegionScene) -> Mesh {
    let terrain = &scene.layout.terrain;
    let detail = usize::from(!near);
    let mut m = Builder::default();
    for place in placed {
        let model = &soil.pieces[detail][place.piece];
        let base = m.positions.len() as u32;
        for ((p, n), c) in model
            .positions
            .iter()
            .zip(&model.normals)
            .zip(&model.colors)
        {
            let at = Vec2::new(
                place.x + p.x * place.scale,
                place.z + p.z * place.scale * place.stretch,
            );
            let y = terrain.height_at(at) + p.y * place.scale + SOIL_LIFT;
            m.positions.push([at.x, y, at.y]);
            m.normals.push(n.to_array());
            m.colors.push([c[0], c[1], c[2], 1.0]);
        }
        m.indices.extend(model.indices.iter().map(|i| base + i));
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, m.colors)
    .with_inserted_indices(Indices::U32(m.indices))
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
    blooms: &[Bloom],
    unpicked: impl Fn(u32) -> bool,
    scene: &RegionScene,
    content: &Content,
    clumps: &[Option<Clump>],
    detail: usize,
) -> Option<Mesh> {
    let field = &scene.field;
    let stem = lin(Color::srgb_u8(86, 128, 70));
    let scale = content.world.flower_scale;
    let mut m = Builder::default();
    for bloom in blooms.iter().filter(|b| unpicked(b.owner)) {
        let index = field.species(bloom.owner);
        let species = &content.species[index];
        if let Some(Some(clump)) = clumps.get(index) {
            add_model(&mut m, &clump.models[detail], bloom, species.height);
            continue;
        }
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

/// A small copy of a species' simple model for `bloom`: its height, turned
/// and leaning like the bloom, the colours tinted a little.
fn add_model(m: &mut Builder, model: &Plant, bloom: &Bloom, height: f32) {
    let scale = bloom.height / height.max(1e-3);
    let turn = Quat::from_rotation_y(bloom.yaw);
    let tint = 1.0 + bloom.tint * 0.08;
    let base = m.positions.len() as u32;
    for ((p, n), c) in model
        .positions
        .iter()
        .zip(&model.normals)
        .zip(&model.colors)
    {
        let local = turn * (*p * scale);
        let lean = Vec3::new(bloom.lean.x, 0.0, bloom.lean.y) * local.y;
        m.positions.push((bloom.ground + local + lean).to_array());
        m.normals.push((turn * *n).to_array());
        // Rooted at the ground, freest at the bloom (as `sway_by_height`).
        let t = (p.y / height.max(1e-3)).clamp(0.0, 1.0);
        m.colors.push([
            c[0] * tint,
            c[1] * tint,
            c[2] * tint,
            t * t * (3.0 - 2.0 * t),
        ]);
    }
    m.indices.extend(model.indices.iter().map(|i| base + i));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bed_soil_model_loads() {
        let soil = Soil::load().expect("assets/beds/soil.glb with all six pieces");
        assert!(soil.width > 0.0);
        for detail in &soil.pieces {
            for piece in detail {
                assert!(!piece.indices.is_empty());
            }
        }
        // The fine pieces are the finer ones.
        let triangles = |d: usize| soil.pieces[d][1].indices.len();
        assert!(triangles(0) > triangles(1));
    }

    #[test]
    fn soil_covers_a_bed_exactly_whatever_its_length() {
        let soil = Soil::load().expect("soil model");
        for length in [0.4, 1.0, 1.7, 9.3, 22.6] {
            let placed = lay_soil(&soil, 3.0, 10.0, 10.0 + length, 1.2);
            assert_eq!(
                placed.first().map(|p| p.piece),
                Some(0),
                "starts with the start"
            );
            assert_eq!(placed.last().map(|p| p.piece), Some(2), "ends with the end");
            let mut z = 10.0;
            for p in &placed {
                assert!((p.z - z).abs() < 1e-4, "pieces follow on without gaps");
                assert!((p.x - 3.0).abs() < 1e-6);
                z += soil.lengths[p.piece] * p.scale * p.stretch;
            }
            assert!((z - (10.0 + length)).abs() < 1e-3, "{length}: ends at {z}");
            // The middle pieces are hardly stretched on long beds.
            if length > 5.0 {
                assert!(placed.iter().all(|p| (p.stretch - 1.0).abs() < 0.1));
            }
        }
    }
}
