//! Flower models made in a 3D tool: a species with a model in
//! `assets/flowers/<species id>.glb` is drawn with that model instead of its
//! procedural one or its picture (see `assets/flowers/README.md`;
//! `scripts/flower_model.py` prepares a model).
//!
//! Optional simpler versions of the same plant fill the beds
//! (`rendering::beds`) away from the player: `<species id>_mid.glb` at a
//! middle distance and `<species id>_low.glb` further away, which also stands in for
//! the model in the distance. So a field looks the same near and far.
//!
//! The models are embedded at build time (`build.rs`). Like the procedural
//! models they are vertex-coloured, so they share their material and batch
//! with them. Here they are measured so the plant stands with its stem at
//! the origin and reaches the species' height, whatever size and position it
//! was modelled at.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bloom_core::data::FlowerSpecies;

/// The model for `species`, if there is one: rooted at the origin,
/// `species.height` tall, facing +Z as modelled.
pub fn load(species: &FlowerSpecies) -> Option<Mesh> {
    load_plant(species).map(Plant::into_mesh)
}

/// The same model as `load`, as plain triangles.
pub fn load_plant(species: &FlowerSpecies) -> Option<Plant> {
    plant(species, &species.id)
}

/// A simpler version (`"mid"`, `"low"`) of the model for `species`, if there
/// is one, placed like `load`'s.
pub fn load_version(species: &FlowerSpecies, version: &str) -> Option<Plant> {
    plant(species, &format!("{}_{version}", species.id))
}

/// The tree (`assets/trees/tree.glb`), if there is one: rooted at the
/// origin, `height` tall, without the flat ground disc such models often
/// stand on (the terrain is the ground).
pub fn tree_mesh(height: f32) -> Option<Mesh> {
    let bytes = super::embedded::TREES
        .iter()
        .find(|(n, _)| *n == "tree")
        .map(|(_, b)| *b)?;
    let mut tree = match parse(bytes) {
        Ok(t) => t,
        Err(e) => {
            warn!("Tree model unreadable: {e}");
            return None;
        }
    };
    tree.drop_ground_disc();
    tree.stand(height);
    Some(tree.into_mesh())
}

/// The meadow lawn tile (`assets/lawn/lawn.glb`), if there is one.
pub fn lawn_model() -> Option<&'static [u8]> {
    super::embedded::LAWN
        .iter()
        .find(|(n, _)| *n == "lawn")
        .map(|(_, b)| *b)
}

/// A flower bed model from `assets/beds/<name>.glb`, if there is one.
pub fn bed_model(name: &str) -> Option<&'static [u8]> {
    super::embedded::BEDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)
}

/// A first-person hand model from `assets/hands/<name>.glb`, if there is one.
pub fn hand_model(name: &str) -> Option<&'static [u8]> {
    super::embedded::HANDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)
}

fn plant(species: &FlowerSpecies, name: &str) -> Option<Plant> {
    let bytes = super::embedded::MODELS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)?;
    match parse(bytes) {
        Ok(mut plant) => {
            plant.stand(species.height);
            Some(plant)
        }
        Err(e) => {
            warn!("Flower model {name} unreadable: {e}");
            None
        }
    }
}

/// Triangles in model space; colours in linear light.
#[derive(Debug, Default, Clone)]
pub struct Plant {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

/// Every triangle of the model's scene, in scene space.
fn parse(bytes: &[u8]) -> Result<Plant, String> {
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let blob = gltf.blob.as_deref();
    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .ok_or("no scene")?;
    let mut plant = Plant::default();
    let mut stack: Vec<(gltf::Node, Mat4)> = scene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
    while let Some((node, parent)) = stack.pop() {
        let at = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        stack.extend(node.children().map(|c| (c, at)));
        if let Some(mesh) = node.mesh() {
            read_mesh(&mesh, blob, at, &mut plant);
        }
    }
    finish(plant)
}

/// Every mesh of a model by name, each in its own space (node placement
/// ignored), for models made of named pieces.
pub fn parse_named(bytes: &[u8]) -> Result<Vec<(String, Plant)>, String> {
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let blob = gltf.blob.as_deref();
    gltf.meshes()
        .map(|mesh| {
            let mut plant = Plant::default();
            read_mesh(&mesh, blob, Mat4::IDENTITY, &mut plant);
            let name = mesh.name().unwrap_or_default().to_string();
            finish(plant)
                .map(|p| (name.clone(), p))
                .map_err(|e| format!("{name}: {e}"))
        })
        .collect()
}

/// Append the triangles of `mesh`, placed by `at`.
fn read_mesh(mesh: &gltf::Mesh, blob: Option<&[u8]>, at: Mat4, plant: &mut Plant) {
    for primitive in mesh.primitives() {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            continue;
        }
        let reader = primitive.reader(|buffer| match buffer.source() {
            gltf::buffer::Source::Bin => blob,
            gltf::buffer::Source::Uri(_) => None,
        });
        let Some(positions) = reader.read_positions() else {
            continue;
        };
        let base = plant.positions.len() as u32;
        plant
            .positions
            .extend(positions.map(|p| at.transform_point3(Vec3::from(p))));
        let count = plant.positions.len() - base as usize;
        let normal_matrix = Mat3::from_mat4(at).inverse().transpose();
        match reader.read_normals() {
            Some(n) => plant
                .normals
                .extend(n.map(|n| (normal_matrix * Vec3::from(n)).normalize_or(Vec3::Y))),
            None => plant.normals.extend(std::iter::repeat_n(Vec3::ZERO, count)),
        }
        let factor = primitive
            .material()
            .pbr_metallic_roughness()
            .base_color_factor();
        match reader.read_colors(0) {
            Some(c) => plant.colors.extend(
                c.into_rgba_f32()
                    .map(|c| std::array::from_fn(|k| c[k] * factor[k])),
            ),
            None => plant.colors.extend(std::iter::repeat_n(factor, count)),
        }
        let flip = at.determinant() < 0.0;
        match reader.read_indices() {
            Some(i) => {
                let i: Vec<u32> = i.into_u32().collect();
                for t in i.chunks_exact(3) {
                    let t = if flip {
                        [t[0], t[2], t[1]]
                    } else {
                        [t[0], t[1], t[2]]
                    };
                    plant.indices.extend(t.map(|i| base + i));
                }
            }
            None => {
                for t in (0..count as u32 / 3).map(|t| t * 3) {
                    let t = if flip {
                        [t, t + 2, t + 1]
                    } else {
                        [t, t + 1, t + 2]
                    };
                    plant.indices.extend(t.map(|i| base + i));
                }
            }
        }
    }
}

/// Check what was read and fill in missing normals.
fn finish(mut plant: Plant) -> Result<Plant, String> {
    if plant.indices.is_empty() {
        return Err("no triangles".into());
    }
    if plant
        .indices
        .iter()
        .any(|&i| i as usize >= plant.positions.len())
    {
        return Err("index out of range".into());
    }
    plant.fill_missing_normals();
    Ok(plant)
}

impl Plant {
    /// Stand the plant up: its lowest point on the ground, the middle of its
    /// lowest part (the stem base) at the origin, and `height` tall.
    fn stand(&mut self, height: f32) {
        let (low, high) = self
            .positions
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p.y), h.max(p.y)));
        let tall = (high - low).max(1e-6);
        // The stem base: what is in the lowest 3% of the plant.
        let foot: Vec<Vec3> = self
            .positions
            .iter()
            .copied()
            .filter(|p| p.y <= low + tall * 0.03)
            .collect();
        let stem = foot.iter().sum::<Vec3>() / foot.len().max(1) as f32;
        let scale = height / tall;
        for p in &mut self.positions {
            *p = Vec3::new(p.x - stem.x, p.y - low, p.z - stem.z) * scale;
        }
    }

    /// Remove flat, upward-facing triangles at the very bottom (a ground
    /// disc), and the vertices only they used.
    fn drop_ground_disc(&mut self) {
        let (low, high) = self
            .positions
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p.y), h.max(p.y)));
        let flat = (high - low) * 0.012;
        let keep: Vec<u32> = self
            .indices
            .chunks_exact(3)
            .filter(|t| {
                let at_bottom = t.iter().all(|&i| self.positions[i as usize].y < low + flat);
                let level = t.iter().all(|&i| self.normals[i as usize].y.abs() > 0.8);
                !(at_bottom && level)
            })
            .flatten()
            .copied()
            .collect();
        let mut map = vec![u32::MAX; self.positions.len()];
        let mut plant = Plant::default();
        for &i in &keep {
            if map[i as usize] == u32::MAX {
                map[i as usize] = plant.positions.len() as u32;
                plant.positions.push(self.positions[i as usize]);
                plant.normals.push(self.normals[i as usize]);
                plant.colors.push(self.colors[i as usize]);
            }
            plant.indices.push(map[i as usize]);
        }
        *self = plant;
    }

    /// Area-weighted smooth normals for vertices the model gave none.
    fn fill_missing_normals(&mut self) {
        if self.normals.iter().all(|n| *n != Vec3::ZERO) {
            return;
        }
        let mut sums = vec![Vec3::ZERO; self.positions.len()];
        for t in self.indices.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| self.positions[t[k] as usize]);
            let face = (b - a).cross(c - a);
            for &i in t {
                sums[i as usize] += face;
            }
        }
        for (n, sum) in self.normals.iter_mut().zip(sums) {
            if *n == Vec3::ZERO {
                *n = sum.normalize_or(Vec3::Y);
            }
        }
    }

    pub fn into_mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            self.positions
                .iter()
                .map(|p| p.to_array())
                .collect::<Vec<_>>(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            self.normals
                .iter()
                .map(|n| n.to_array())
                .collect::<Vec<_>>(),
        )
        // The alpha is overwritten with the sway weight.
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plants_stand_on_the_ground_at_their_height() {
        // A thin post from (5, -2, 3) up to (5, 4, 3) with a wide top.
        let mut plant = Plant {
            positions: vec![
                Vec3::new(4.9, -2.0, 3.0),
                Vec3::new(5.1, -2.0, 3.0),
                Vec3::new(5.0, -2.0, 3.1),
                Vec3::new(3.0, 4.0, 3.0),
                Vec3::new(8.0, 4.0, 3.0),
                Vec3::new(5.0, 4.0, 6.0),
            ],
            ..default()
        };
        plant.stand(0.3);
        let low = plant.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let high = plant.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(low.abs() < 1e-6);
        assert!((high - 0.3).abs() < 1e-6);
        // The stem base (not the middle of the wide top) is at the origin.
        let foot = plant.positions[..3].iter().sum::<Vec3>() / 3.0;
        assert!(foot.x.abs() < 1e-6 && foot.z.abs() < 1e-6, "{foot}");
    }

    #[test]
    fn the_tree_stands_on_the_ground_at_its_height() {
        let mesh = tree_mesh(6.0).expect("assets/trees/tree.glb");
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let low = p.iter().map(|v| v[1]).fold(f32::MAX, f32::min);
        let high = p.iter().map(|v| v[1]).fold(f32::MIN, f32::max);
        assert!(low.abs() < 1e-4 && (high - 6.0).abs() < 1e-3);
        // No ground disc: nothing wide lies flat at the very bottom.
        let wide = p
            .iter()
            .filter(|v| v[1] < 0.05)
            .map(|v| v[0].hypot(v[2]))
            .fold(0.0f32, f32::max);
        assert!(wide < 1.5, "{wide}");
    }

    #[test]
    fn the_shipped_models_load() {
        for (name, bytes) in super::super::embedded::MODELS {
            let plant = parse(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(plant.positions.len(), plant.normals.len(), "{name}");
            assert_eq!(plant.positions.len(), plant.colors.len(), "{name}");
            assert_eq!(plant.indices.len() % 3, 0, "{name}");
            // A meadow of them must stay affordable.
            assert!(
                plant.indices.len() / 3 <= 6000,
                "{name}: too many triangles"
            );
            assert!(
                plant
                    .normals
                    .iter()
                    .all(|n| (n.length() - 1.0).abs() < 1e-3),
                "{name}: normals"
            );
        }
    }
}
