//! Deterministic heightfield terrain.
//!
//! The ground plane uses `Vec2(x, z)`; heights are world Y. A region is
//! centred on the origin. Heights are sampled on a regular grid and
//! bilinearly interpolated, so physics (`height_at`) and the rendered mesh
//! (built from the same samples) always agree.

use glam::{Vec2, Vec3};

use crate::data::TerrainParams;

#[derive(Debug, Clone)]
pub struct Terrain {
    /// World position of grid sample (0, 0) — the region's min corner.
    origin: Vec2,
    /// Distance between samples in metres.
    cell: f32,
    /// Samples along X and Z.
    nx: usize,
    nz: usize,
    heights: Vec<f32>,
}

/// A circular area flattened to a fixed height (spawn point, building pads).
#[derive(Debug, Clone, Copy)]
pub struct FlattenArea {
    pub center: Vec2,
    /// Fully flat inside this radius.
    pub inner_radius: f32,
    /// Blends back to natural terrain by this radius.
    pub outer_radius: f32,
}

impl Terrain {
    /// A perfectly flat terrain (tests, tools).
    pub fn flat(width: f32, depth: f32, cell: f32, height: f32) -> Self {
        let (nx, nz) = Self::sample_counts(width, depth, cell);
        Self {
            origin: Vec2::new(-width / 2.0, -depth / 2.0),
            cell,
            nx,
            nz,
            heights: vec![height; nx * nz],
        }
    }

    /// A terrain from explicit samples (`heights[iz * nx + ix]`).
    pub fn from_heights(origin: Vec2, cell: f32, nx: usize, nz: usize, heights: Vec<f32>) -> Self {
        assert!(
            nx >= 2 && nz >= 2 && heights.len() == nx * nz,
            "bad terrain grid"
        );
        Self {
            origin,
            cell,
            nx,
            nz,
            heights,
        }
    }

    /// Rolling hills from seeded fractal value noise, a raised berm along the
    /// edges and optional flattened areas.
    ///
    /// The region spans `-region_half..region_half`; the terrain extends a
    /// further `outskirts` metres on every side (scenery beyond the fence)
    /// where the berm stays at full height.
    pub fn generate(
        region_half: Vec2,
        outskirts: f32,
        cell: f32,
        params: &TerrainParams,
        seed: u64,
        flatten: &[FlattenArea],
    ) -> Self {
        let size = (region_half + Vec2::splat(outskirts)) * 2.0;
        let mut terrain = Self::flat(size.x, size.y, cell, 0.0);
        let half = region_half;

        // Natural height at each flatten centre, so flat areas sit at the
        // level the hills would have had there (no cliffs).
        let natural = |p: Vec2| natural_height(p, half, params, seed);
        let flat_levels: Vec<f32> = flatten.iter().map(|f| natural(f.center)).collect();

        for iz in 0..terrain.nz {
            for ix in 0..terrain.nx {
                let p = terrain.sample_pos(ix, iz);
                let mut h = natural(p);
                for (area, level) in flatten.iter().zip(&flat_levels) {
                    let d = p.distance(area.center);
                    if d < area.outer_radius {
                        let span = (area.outer_radius - area.inner_radius).max(f32::EPSILON);
                        let t = ((d - area.inner_radius) / span).clamp(0.0, 1.0);
                        let w = smoothstep(t); // 0 = fully flat, 1 = natural
                        h = level + (h - level) * w;
                    }
                }
                terrain.heights[iz * terrain.nx + ix] = h;
            }
        }
        terrain
    }

    fn sample_counts(width: f32, depth: f32, cell: f32) -> (usize, usize) {
        let nx = (width / cell).ceil() as usize + 1;
        let nz = (depth / cell).ceil() as usize + 1;
        (nx.max(2), nz.max(2))
    }

    pub fn sample_pos(&self, ix: usize, iz: usize) -> Vec2 {
        self.origin + Vec2::new(ix as f32, iz as f32) * self.cell
    }

    pub fn sample_height(&self, ix: usize, iz: usize) -> f32 {
        self.heights[iz * self.nx + ix]
    }

    pub fn samples(&self) -> (usize, usize) {
        (self.nx, self.nz)
    }

    pub fn cell(&self) -> f32 {
        self.cell
    }

    /// Bilinearly interpolated height. Points outside the grid are clamped
    /// to the edge.
    pub fn height_at(&self, p: Vec2) -> f32 {
        let local = (p - self.origin) / self.cell;
        let max_x = (self.nx - 1) as f32;
        let max_z = (self.nz - 1) as f32;
        let gx = local.x.clamp(0.0, max_x);
        let gz = local.y.clamp(0.0, max_z);
        let ix = (gx.floor() as usize).min(self.nx - 2);
        let iz = (gz.floor() as usize).min(self.nz - 2);
        let fx = gx - ix as f32;
        let fz = gz - iz as f32;
        let h00 = self.sample_height(ix, iz);
        let h10 = self.sample_height(ix + 1, iz);
        let h01 = self.sample_height(ix, iz + 1);
        let h11 = self.sample_height(ix + 1, iz + 1);
        let a = h00 + (h10 - h00) * fx;
        let b = h01 + (h11 - h01) * fx;
        a + (b - a) * fz
    }

    /// Upward unit normal from central differences.
    pub fn normal_at(&self, p: Vec2) -> Vec3 {
        let e = self.cell;
        let dx = self.height_at(p + Vec2::X * e) - self.height_at(p - Vec2::X * e);
        let dz = self.height_at(p + Vec2::Y * e) - self.height_at(p - Vec2::Y * e);
        Vec3::new(-dx, 2.0 * e, -dz).normalize()
    }
}

fn natural_height(p: Vec2, half: Vec2, params: &TerrainParams, seed: u64) -> f32 {
    let hills = fbm(p / params.feature_size, params.octaves, seed) * params.amplitude;
    // Distance to the nearest edge → a soft berm that frames the region.
    let edge = (half.x - p.x.abs()).min(half.y - p.y.abs()).max(0.0);
    let berm = if edge < params.border_width {
        smoothstep(1.0 - edge / params.border_width) * params.border_rise
    } else {
        0.0
    };
    hills + berm
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Lattice hash → [-1, 1]. Integer-only, so identical on every platform.
fn lattice(seed: u64, x: i32, z: i32) -> f32 {
    let mut h = seed ^ 0x9E37_79B9_7F4A_7C15;
    h ^= (x as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h = h.rotate_left(31).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= (z as u32 as u64).wrapping_mul(0x27D4_EB2F_1656_67C5);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

fn value_noise(p: Vec2, seed: u64) -> f32 {
    let x0 = p.x.floor();
    let z0 = p.y.floor();
    let (ix, iz) = (x0 as i32, z0 as i32);
    let fx = smoothstep(p.x - x0);
    let fz = smoothstep(p.y - z0);
    let a = lattice(seed, ix, iz);
    let b = lattice(seed, ix + 1, iz);
    let c = lattice(seed, ix, iz + 1);
    let d = lattice(seed, ix + 1, iz + 1);
    let top = a + (b - a) * fx;
    let bottom = c + (d - c) * fx;
    top + (bottom - top) * fz
}

/// Fractal Brownian motion, normalised to roughly [-1, 1].
fn fbm(p: Vec2, octaves: u32, seed: u64) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += value_noise(p * freq, seed.wrapping_add(o as u64 * 0x1F3D)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARAMS: TerrainParams = TerrainParams {
        amplitude: 2.5,
        feature_size: 30.0,
        octaves: 4,
        border_rise: 3.0,
        border_width: 10.0,
    };

    #[test]
    fn generation_is_deterministic() {
        let a = Terrain::generate(Vec2::new(50.0, 40.0), 0.0, 0.5, &PARAMS, 42, &[]);
        let b = Terrain::generate(Vec2::new(50.0, 40.0), 0.0, 0.5, &PARAMS, 42, &[]);
        assert_eq!(a.heights, b.heights);
        let c = Terrain::generate(Vec2::new(50.0, 40.0), 0.0, 0.5, &PARAMS, 43, &[]);
        assert_ne!(a.heights, c.heights);
    }

    #[test]
    fn heights_are_continuous_and_bounded() {
        let t = Terrain::generate(Vec2::new(50.0, 40.0), 0.0, 0.5, &PARAMS, 7, &[]);
        let max = PARAMS.amplitude + PARAMS.border_rise + 0.01;
        let mut prev = t.height_at(Vec2::new(-49.0, 3.3));
        let mut x = -49.0;
        while x < 49.0 {
            let h = t.height_at(Vec2::new(x, 3.3));
            assert!(h.abs() <= max, "height {h} out of range");
            assert!((h - prev).abs() < 0.2, "jump in height at x={x}");
            prev = h;
            x += 0.05;
        }
    }

    #[test]
    fn interpolation_hits_samples_exactly() {
        let t = Terrain::generate(Vec2::new(10.0, 10.0), 0.0, 0.5, &PARAMS, 3, &[]);
        for (ix, iz) in [(0, 0), (5, 7), (40, 40), (13, 2)] {
            let p = t.sample_pos(ix, iz);
            assert!((t.height_at(p) - t.sample_height(ix, iz)).abs() < 1e-4);
        }
    }

    #[test]
    fn flatten_area_is_flat() {
        let area = FlattenArea {
            center: Vec2::new(10.0, -5.0),
            inner_radius: 6.0,
            outer_radius: 12.0,
        };
        let t = Terrain::generate(Vec2::new(50.0, 40.0), 0.0, 0.5, &PARAMS, 9, &[area]);
        let h0 = t.height_at(area.center);
        for offset in [
            Vec2::new(3.0, 0.0),
            Vec2::new(-4.0, 2.0),
            Vec2::new(0.0, -5.5),
        ] {
            assert!((t.height_at(area.center + offset) - h0).abs() < 1e-3);
        }
        assert!(t.normal_at(area.center).y > 0.999);
    }

    #[test]
    fn edges_rise_into_a_berm() {
        let t = Terrain::generate(Vec2::new(50.0, 40.0), 0.0, 0.5, &PARAMS, 11, &[]);
        let edge = t.height_at(Vec2::new(0.0, 39.9));
        let inside = t.height_at(Vec2::new(0.0, 25.0));
        assert!(edge > inside + 0.5, "edge {edge} vs inside {inside}");
    }

    #[test]
    fn outskirts_extend_the_grid_beyond_the_region() {
        let t = Terrain::generate(Vec2::new(50.0, 40.0), 20.0, 0.5, &PARAMS, 5, &[]);
        assert_eq!(t.samples(), (281, 241));
        assert_eq!(t.sample_pos(0, 0), Vec2::new(-70.0, -60.0));
    }

    #[test]
    fn flat_terrain_normal_points_up() {
        let t = Terrain::flat(10.0, 10.0, 1.0, 2.0);
        assert_eq!(t.height_at(Vec2::new(1.3, -2.2)), 2.0);
        assert!((t.normal_at(Vec2::ZERO) - Vec3::Y).length() < 1e-6);
    }
}
