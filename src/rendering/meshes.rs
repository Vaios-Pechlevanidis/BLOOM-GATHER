//! Procedural meshes for the world: terrain chunks, trees, rocks and fences.
//!
//! These are *temporary art*, documented in TODO.md: real 3D geometry in the
//! cottagecore palette, built in code so the world is fully explorable now.
//! Painted glTF models replace them in the polish phase without touching
//! gameplay, because placement and collision come from `bloom_core`.
//!
//! Every prop is a single mesh with per-vertex colours, drawn with one shared
//! material, so all trees (and all rocks) batch into very few draw calls.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bloom_core::data::{FlowerRows, RegionPalette};
use bloom_core::world::RegionLayout;

use super::color;

/// Accumulates transformed, coloured primitives into one mesh.
#[derive(Default)]
pub struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    /// Append `mesh` transformed by `transform`, colouring each vertex with
    /// `paint(local_position)`.
    pub fn add(&mut self, mesh: &Mesh, transform: Transform, paint: impl Fn(Vec3) -> LinearRgba) {
        self.add_with(mesh, transform, |_, local| paint(local).to_f32_array());
    }

    /// Append `mesh` transformed by `transform`, keeping its own vertex
    /// colours (white where it has none).
    pub fn add_coloured(&mut self, mesh: &Mesh, transform: Transform) {
        let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
            Some(VertexAttributeValues::Float32x4(c)) => c.clone(),
            _ => Vec::new(),
        };
        self.add_with(mesh, transform, |i, _| {
            let c = colors.get(i).copied().unwrap_or([1.0; 4]);
            [c[0], c[1], c[2], 1.0]
        });
    }

    fn add_with(
        &mut self,
        mesh: &Mesh,
        transform: Transform,
        color: impl Fn(usize, Vec3) -> [f32; 4],
    ) {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return;
        };
        let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(n)) => n.clone(),
            _ => vec![[0.0, 1.0, 0.0]; positions.len()],
        };
        let base = self.positions.len() as u32;
        let inv_scale = transform.scale.recip();
        for (i, (p, n)) in positions.iter().zip(&normals).enumerate() {
            let local = Vec3::from(*p);
            self.positions.push(transform.transform_point(local).into());
            // Inverse-transpose for non-uniform scale: divide, rotate, renormalise.
            let normal = (transform.rotation * (Vec3::from(*n) * inv_scale)).normalize_or_zero();
            self.normals.push(normal.into());
            self.colors.push(color(i, local));
        }
        match mesh.indices() {
            Some(indices) => self.indices.extend(indices.iter().map(|i| base + i as u32)),
            None => self.indices.extend(base..base + positions.len() as u32),
        }
    }

    pub fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

pub fn lin(c: Color) -> LinearRgba {
    c.to_linear()
}

pub fn mix(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    let t = t.clamp(0.0, 1.0);
    LinearRgba::new(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        1.0,
    )
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Cheap, organic-looking colour variation (not used for gameplay).
fn swirl(x: f32, z: f32) -> f32 {
    ((x * 0.13 + (z * 0.071).sin() * 2.0).sin() + (z * 0.11 + x * 0.047).sin()) * 0.25 + 0.5
}

/// Deterministic per-vertex jitter in [-1, 1].
pub fn hash3(p: Vec3) -> f32 {
    let h = (p.x * 127.1 + p.y * 311.7 + p.z * 74.7).sin() * 43_758.547;
    h.fract() * 2.0 - 1.0
}

// ---------------------------------------------------------------------------
// Terrain
// ---------------------------------------------------------------------------

/// Terrain split into square chunks so the renderer can frustum-cull them.
/// Mesh positions are in world space; Bevy computes each chunk's bounds.
pub fn terrain_chunks(
    layout: &RegionLayout,
    palette: &RegionPalette,
    rows: Option<FlowerRows>,
    chunk_size: f32,
) -> Vec<Mesh> {
    let terrain = &layout.terrain;
    let (nx, nz) = terrain.samples();
    let cells_per_chunk = ((chunk_size / terrain.cell()).round() as usize).max(4);

    let dark = lin(color(palette.ground_dark));
    let mid = lin(color(palette.ground));
    let light = lin(color(palette.ground_light));
    let path = lin(color(palette.path));
    let path_edge = mix(path, dark, 0.35);
    let half_w = layout.path_half_width;
    // Tilled soil between flower rows, and a muddy stream or pond bed.
    let soil = mix(path, lin(Color::srgb_u8(120, 96, 76)), 0.6);
    let bed = mix(dark, lin(Color::srgb_u8(96, 86, 66)), 0.55);
    let furrow = lin(Color::srgb_u8(170, 140, 102));
    let furrow_light = lin(Color::srgb_u8(204, 176, 132));
    // The beds' crowns: the brown of the bed soil model laid over them
    // (`beds::soil_mesh`), so nothing green shows through its seams.
    let bed_soil = lin(Color::srgb_u8(120, 62, 32));
    let bed_soil_light = lin(Color::srgb_u8(150, 82, 44));
    let across = rows.map(|r| {
        let a = r.angle_deg.to_radians();
        (Vec2::new(-a.sin(), a.cos()), r.spacing)
    });

    // Colour every sample once, then cut chunks out of the shared grid.
    let mut colors = Vec::with_capacity(nx * nz);
    let mut normals = Vec::with_capacity(nx * nz);
    for iz in 0..nz {
        for ix in 0..nx {
            let p = terrain.sample_pos(ix, iz);
            let h = terrain.sample_height(ix, iz);
            let n = terrain.normal_at(p);
            let v = swirl(p.x, p.y);
            // Hilltops lighter, hollows and slopes darker.
            let tone = (0.45 + h * 0.07 + (v - 0.5) * 0.5 - (1.0 - n.y) * 2.0).clamp(0.0, 1.0);
            let mut c = if tone < 0.5 {
                mix(dark, mid, tone * 2.0)
            } else {
                mix(mid, light, (tone - 0.5) * 2.0)
            };
            if let Some((dir, spacing)) = across {
                if layout.in_bounds(p, 0.5) {
                    let v = p.dot(dir) / spacing;
                    let f = (v - v.round()).abs(); // 0 on a row, 0.5 between
                    c = mix(c, soil, smoothstep(0.22, 0.42, f) * 0.5);
                }
            }
            for w in &layout.water {
                let d = w.centre_distance(p);
                if d < w.outer_radius() {
                    let wet = 1.0 - smoothstep(w.half_width * 0.5, w.outer_radius(), d);
                    c = mix(c, bed, wet);
                }
            }
            // Bulb fields: sandy furrows between raised beds of brown soil;
            // the rest stays meadow.
            if let Some(plan) = &layout.fields {
                if plan.in_planted_area(p, 0.35) {
                    let w = plan.bed_weight(p);
                    let grain = hash3(Vec3::new(p.x * 3.1, 0.0, p.y * 3.1)) * 0.5 + 0.5;
                    let sand = mix(furrow, furrow_light, grain * 0.6 + (v - 0.5) * 0.4);
                    let crown = mix(bed_soil, bed_soil_light, grain * 0.5);
                    // Brown only right under the soil model, so it doesn't
                    // bleed into the grass beyond a bed's end.
                    c = mix(sand, crown, smoothstep(0.9, 1.0, w));
                }
            }
            let d = layout.distance_to_path(p);
            if d < half_w + 0.8 {
                let worn = 1.0 - smoothstep(half_w - 0.35, half_w + 0.6, d + (v - 0.5) * 0.5);
                let edge = mix(c, path_edge, smoothstep(half_w + 0.8, half_w - 0.1, d));
                c = mix(edge, path, worn);
            }
            colors.push(c.to_f32_array());
            normals.push(n.to_array());
        }
    }

    let mut chunks = Vec::new();
    let mut z0 = 0;
    while z0 < nz - 1 {
        let z1 = (z0 + cells_per_chunk).min(nz - 1);
        let mut x0 = 0;
        while x0 < nx - 1 {
            let x1 = (x0 + cells_per_chunk).min(nx - 1);
            chunks.push(terrain_chunk(layout, &colors, &normals, (x0, x1), (z0, z1)));
            x0 = x1;
        }
        z0 = z1;
    }
    chunks
}

fn terrain_chunk(
    layout: &RegionLayout,
    colors: &[[f32; 4]],
    normals: &[[f32; 3]],
    (x0, x1): (usize, usize),
    (z0, z1): (usize, usize),
) -> Mesh {
    let terrain = &layout.terrain;
    let (nx, _) = terrain.samples();
    let w = x1 - x0 + 1;
    let mut positions = Vec::new();
    let mut chunk_normals = Vec::new();
    let mut chunk_colors = Vec::new();
    for iz in z0..=z1 {
        for ix in x0..=x1 {
            let p = terrain.sample_pos(ix, iz);
            positions.push([p.x, terrain.sample_height(ix, iz), p.y]);
            chunk_normals.push(normals[iz * nx + ix]);
            chunk_colors.push(colors[iz * nx + ix]);
        }
    }
    let mut indices = Vec::new();
    for z in 0..(z1 - z0) {
        for x in 0..(x1 - x0) {
            let a = (z * w + x) as u32;
            let b = a + 1;
            let c = a + w as u32;
            let d = c + 1;
            // Counter-clockwise seen from above → normals face up.
            indices.extend([a, c, b, b, c, d]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, chunk_normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, chunk_colors)
    .with_inserted_indices(Indices::U32(indices))
}

// ---------------------------------------------------------------------------
// Trees — modelled 6 m tall; instances scale uniformly by height / 6.
// ---------------------------------------------------------------------------

pub const TREE_MODEL_HEIGHT: f32 = 6.0;

/// Four tree silhouettes: round oak, tall poplar, a green tree dotted with
/// blooms, and a tree in full blossom (canopy in the accent colour). If the
/// game ships a tree model, all four are that model.
pub fn tree_variants(palette: &RegionPalette) -> [Mesh; 4] {
    // A modelled tree (`assets/trees/tree.glb`) replaces all of them.
    if let Some(tree) = crate::flowers::imported::tree_mesh(TREE_MODEL_HEIGHT) {
        return [tree.clone(), tree.clone(), tree.clone(), tree];
    }
    let bark_dark = lin(Color::srgb_u8(104, 80, 62));
    let bark_light = lin(Color::srgb_u8(146, 114, 86));
    let leaf_dark = lin(color(palette.ground_dark)) * 0.72;
    let leaf_mid = lin(color(palette.ground_dark));
    let leaf_light = lin(color(palette.ground_light));
    let blossom = lin(color(palette.accent));
    let blossom_light = mix(blossom, LinearRgba::WHITE, 0.45);

    // Trunk radius 0.3 at 6 m = TRUNK_RADIUS_PER_HEIGHT (matches the collider).
    let trunk = |height: f32| {
        let mesh = Mesh::from(Cylinder::new(0.3, height).mesh().resolution(10));
        (mesh, Transform::from_xyz(0.0, height / 2.0, 0.0))
    };
    let bark = move |p: Vec3| mix(bark_dark, bark_light, 0.5 + p.x * 0.8 + hash3(p) * 0.15);
    let blob = |r: f32| Sphere::new(r).mesh().ico(2).expect("valid icosphere");
    // Canopy shading: darker underneath, sunlit on top, with leafy noise.
    let leaves = move |p: Vec3, r: f32| {
        let t = (p.y / r) * 0.5 + 0.5 + hash3(p * 3.0) * 0.12;
        if t < 0.5 {
            mix(leaf_dark, leaf_mid, t * 2.0)
        } else {
            mix(leaf_mid, leaf_light, (t - 0.5) * 1.4)
        }
    };

    let mut oak = MeshBuilder::default();
    let (m, t) = trunk(3.4);
    oak.add(&m, t, bark);
    for (pos, r) in [
        (Vec3::new(0.0, 3.9, 0.0), 1.55),
        (Vec3::new(0.95, 3.45, 0.35), 1.15),
        (Vec3::new(-0.85, 3.55, -0.4), 1.1),
        (Vec3::new(0.2, 4.75, -0.15), 1.05),
        (Vec3::new(-0.3, 3.4, 0.9), 0.95),
    ] {
        oak.add(&blob(r), Transform::from_translation(pos), move |p| {
            leaves(p, r)
        });
    }

    let mut poplar = MeshBuilder::default();
    let (m, t) = trunk(2.4);
    poplar.add(&m, t, bark);
    for (pos, scale) in [
        (Vec3::new(0.0, 3.5, 0.0), Vec3::new(1.15, 2.3, 1.15)),
        (Vec3::new(0.15, 5.2, 0.05), Vec3::new(0.75, 1.0, 0.75)),
    ] {
        let t = Transform::from_translation(pos).with_scale(scale);
        poplar.add(&blob(1.0), t, move |p| leaves(p, 1.0));
    }

    let mut blossom_tree = MeshBuilder::default();
    let (m, t) = trunk(2.8);
    blossom_tree.add(&m, t, bark);
    let crowns = [
        (Vec3::new(0.0, 3.6, 0.0), 1.45),
        (Vec3::new(1.0, 3.2, -0.2), 1.0),
        (Vec3::new(-0.9, 3.3, 0.3), 1.0),
        (Vec3::new(0.1, 4.4, 0.5), 0.9),
    ];
    for (pos, r) in crowns {
        blossom_tree.add(&blob(r), Transform::from_translation(pos), move |p| {
            leaves(p, r)
        });
    }
    // Little blooms dotted over the canopy surface.
    let bloom = Sphere::new(0.16).mesh().ico(1).expect("valid icosphere");
    for i in 0..26 {
        let (pos, r) = crowns[i % crowns.len()];
        let a = i as f32 * 2.399; // golden angle spiral
        let y = 1.0 - (i as f32 + 0.5) / 26.0 * 1.4;
        let ring = (1.0 - y * y).max(0.0).sqrt();
        let dir = Vec3::new(a.cos() * ring, y, a.sin() * ring).normalize();
        if dir.y < -0.3 {
            continue;
        }
        let tint = if i % 3 == 0 { blossom_light } else { blossom };
        blossom_tree.add(
            &bloom,
            Transform::from_translation(pos + dir * r * 0.98),
            move |_| tint,
        );
    }

    // Full blossom: the round crown, all in soft pinks.
    let petal_dark = mix(blossom, lin(Color::srgb_u8(170, 110, 130)), 0.35);
    let mut cherry = MeshBuilder::default();
    let (m, t) = trunk(2.9);
    cherry.add(&m, t, bark);
    for (pos, r) in [
        (Vec3::new(0.0, 3.7, 0.0), 1.6),
        (Vec3::new(1.1, 3.3, 0.3), 1.1),
        (Vec3::new(-1.0, 3.4, -0.3), 1.1),
        (Vec3::new(0.2, 4.6, -0.2), 1.0),
        (Vec3::new(-0.2, 3.3, 1.0), 0.9),
    ] {
        cherry.add(&blob(r), Transform::from_translation(pos), move |p| {
            let t = (p.y / r) * 0.5 + 0.5 + hash3(p * 3.0) * 0.15;
            if t < 0.5 {
                mix(petal_dark, blossom, t * 2.0)
            } else {
                mix(blossom, blossom_light, (t - 0.5) * 1.6)
            }
        });
    }

    [
        oak.build(),
        poplar.build(),
        blossom_tree.build(),
        cherry.build(),
    ]
}

// ---------------------------------------------------------------------------
// Rocks — unit size; instances scale by (radius, height, radius).
// ---------------------------------------------------------------------------

pub fn rock_variants(palette: &RegionPalette) -> [Mesh; 3] {
    let base = lin(color(palette.path));
    let dark = mix(base, LinearRgba::new(0.05, 0.05, 0.06, 1.0), 0.55);
    let light = mix(base, LinearRgba::WHITE, 0.2);
    let moss = lin(color(palette.ground_dark));

    std::array::from_fn(|variant| {
        let mut mesh = Sphere::new(1.0).mesh().ico(1).expect("valid icosphere");
        // Knock the sphere into an irregular stone, flatter on the bottom.
        if let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for p in positions.iter_mut() {
                let v = Vec3::from(*p);
                let j = hash3(v * (3.0 + variant as f32));
                let mut out = v * (1.0 + j * 0.2);
                if out.y < -0.3 {
                    out.y = -0.3 + (out.y + 0.3) * 0.4;
                }
                *p = out.into();
            }
        }
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
        let mut builder = MeshBuilder::default();
        builder.add(&mesh, Transform::IDENTITY, move |p| {
            let shade = mix(dark, light, p.y * 0.5 + 0.5 + hash3(p * 5.0) * 0.15);
            // A touch of moss on the top of some stones.
            if variant == 2 && p.y > 0.55 {
                mix(shade, moss, 0.6)
            } else {
                shade
            }
        });
        builder.build()
    })
}

// ---------------------------------------------------------------------------
// Fence along the walkable boundary
// ---------------------------------------------------------------------------

/// The boundary fence, open at `gaps` (gate centres on the fence line).
pub fn fence(layout: &RegionLayout, gaps: &[Vec2]) -> Mesh {
    let wood_dark = lin(Color::srgb_u8(132, 102, 76));
    let wood_light = lin(Color::srgb_u8(176, 142, 108));
    let post = Mesh::from(Cuboid::new(0.14, 1.15, 0.14));
    let rail = Mesh::from(Cuboid::new(1.0, 0.09, 0.06));
    let spacing = 2.6;
    // Fence sits just outside the walkable rectangle, on the invisible wall.
    let min = layout.bounds_min - Vec2::splat(0.3);
    let max = layout.bounds_max + Vec2::splat(0.3);
    let corners = [
        Vec2::new(min.x, min.y),
        Vec2::new(max.x, min.y),
        Vec2::new(max.x, max.y),
        Vec2::new(min.x, max.y),
    ];
    let ground = |p: Vec2| layout.terrain.height_at(p);
    let paint = move |p: Vec3| mix(wood_dark, wood_light, 0.5 + hash3(p * 7.0) * 0.4);

    let mut builder = MeshBuilder::default();
    let half_gap = bloom_core::world::features::GATE_HALF_WIDTH + 0.1;
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let len = (b - a).length();
        let dir = (b - a) / len;
        // Cut the edge into runs around any gate opening on it.
        let mut cuts: Vec<(f32, f32)> = gaps
            .iter()
            .filter_map(|g| {
                let along = (*g - a).dot(dir);
                let off = (*g - a).perp_dot(dir).abs();
                (off < 1.0 && along > 0.0 && along < len)
                    .then_some((along - half_gap, along + half_gap))
            })
            .collect();
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut runs = Vec::new();
        let mut start = 0.0;
        for (lo, hi) in cuts {
            runs.push((start, lo.max(start)));
            start = hi;
        }
        runs.push((start, len));
        for (r0, r1) in runs {
            if r1 - r0 < 0.3 {
                continue;
            }
            let steps = ((r1 - r0) / spacing).ceil().max(1.0) as usize;
            let mut prev: Option<Vec3> = None;
            for s in 0..=steps {
                let p = a + dir * (r0 + (r1 - r0) * s as f32 / steps as f32);
                let lean = Quat::from_rotation_z(hash3(p.extend(1.0)) * 0.04);
                let top = Vec3::new(p.x, ground(p) + 0.55, p.y);
                builder.add(
                    &post,
                    Transform::from_translation(top).with_rotation(lean),
                    paint,
                );
                if let Some(prev_top) = prev {
                    for h in [0.15, -0.25] {
                        let from = prev_top + Vec3::Y * h;
                        let to = top + Vec3::Y * h;
                        let mid = (from + to) / 2.0;
                        let d = to - from;
                        let rot = Quat::from_rotation_arc(Vec3::X, d.normalize());
                        let t = Transform::from_translation(mid)
                            .with_rotation(rot)
                            .with_scale(Vec3::new(d.length(), 1.0, 1.0));
                        builder.add(&rail, t, paint);
                    }
                }
                prev = Some(top);
            }
        }
    }
    builder.build()
}
