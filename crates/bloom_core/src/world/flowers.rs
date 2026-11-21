//! The finite, harvestable flowers of a region.
//!
//! Placement is deterministic from the region seed and layout, so a flower's
//! index is its permanent id: the save file only stores *which* indices were
//! harvested (a [`BitSet`]), never positions. Data is stored as a struct of
//! arrays and indexed by a uniform grid for radius queries and look-at
//! picking.
//!
//! Decorative vegetation is never part of a `FlowerField` and can never be
//! harvested or counted.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::layout::RegionLayout;
use crate::bitset::BitSet;
use crate::data::{GameData, RegionDef};
use crate::rng::SplitMix64;

#[derive(Debug, Clone)]
pub struct FlowerField {
    positions: Vec<Vec2>,
    /// Ground height under each flower.
    bases: Vec<f32>,
    /// Index into `GameData::species`.
    species: Vec<u16>,
    yaws: Vec<f32>,
    scales: Vec<f32>,
    /// Bloom height (species height × scale) cached for picking.
    bloom_heights: Vec<f32>,
    grid: Grid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacementError {
    pub region: String,
    pub species: String,
    pub placed: u32,
    pub wanted: u32,
}

impl std::fmt::Display for PlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "could only place {} of {} '{}' in region '{}'",
            self.placed, self.wanted, self.species, self.region
        )
    }
}

/// Clearances used by placement (metres).
const PATH_CLEARANCE: f32 = 0.35;
const TREE_CLEARANCE: f32 = 0.9;
const ROCK_CLEARANCE: f32 = 0.4;
const SPAWN_CLEARANCE: f32 = 2.5;
const BOUNDS_INSET: f32 = 1.0;
/// Flowers don't grow on slopes steeper than ~32°.
const MIN_GROUND_NORMAL_Y: f32 = 0.85;
/// Average flowers per natural cluster.
const FLOWERS_PER_CLUSTER: u32 = 9;

impl FlowerField {
    pub fn generate(
        region: &RegionDef,
        layout: &RegionLayout,
        data: &GameData,
    ) -> Result<Self, PlacementError> {
        let spacing = data.world.min_flower_spacing;
        let mut rng = SplitMix64::new(region.seed ^ 0xF10_3E25);
        let mut field = Self {
            positions: Vec::new(),
            bases: Vec::new(),
            species: Vec::new(),
            yaws: Vec::new(),
            scales: Vec::new(),
            bloom_heights: Vec::new(),
            grid: Grid::new(layout.bounds_min, layout.bounds_max, spacing.max(2.0)),
        };

        for entry in &region.flowers {
            let species_idx = data
                .species_index(&entry.species)
                .expect("validated content references known species");
            let height = data.species[species_idx].height;
            let wanted = entry.count;

            // A few natural clusters per species, then fill around them.
            let clusters = wanted.div_ceil(FLOWERS_PER_CLUSTER).max(1);
            let cluster_radius = 2.5 + (wanted as f32 / clusters as f32).sqrt() * 1.3;
            let centers: Vec<Vec2> = (0..clusters)
                .map(|_| field.random_point(&mut rng, layout, BOUNDS_INSET + cluster_radius * 0.5))
                .collect();

            let mut placed = 0;
            let mut attempts: u64 = 0;
            let clustered_attempts = wanted as u64 * 400;
            let max_attempts = wanted as u64 * 3_000;
            while placed < wanted {
                attempts += 1;
                if attempts > max_attempts {
                    return Err(PlacementError {
                        region: region.id.clone(),
                        species: entry.species.clone(),
                        placed,
                        wanted,
                    });
                }
                let p = if attempts <= clustered_attempts {
                    let center = centers[rng.below(clusters) as usize];
                    let angle = rng.range_f32(0.0, TAU);
                    // Denser towards the cluster centre.
                    let r = cluster_radius * rng.next_f32().powf(0.8);
                    center + Vec2::new(angle.cos(), angle.sin()) * r
                } else {
                    field.random_point(&mut rng, layout, BOUNDS_INSET)
                };
                if !field.can_place(p, layout, spacing) {
                    continue;
                }
                let scale = rng.range_f32(0.85, 1.15) * data.world.flower_scale;
                field.positions.push(p);
                field.bases.push(layout.terrain.height_at(p));
                field.species.push(species_idx as u16);
                field.yaws.push(rng.range_f32(0.0, TAU));
                field.scales.push(scale);
                field.bloom_heights.push(height * scale);
                field.grid.insert(p, field.positions.len() as u32 - 1);
                placed += 1;
            }
        }
        Ok(field)
    }

    fn random_point(&self, rng: &mut SplitMix64, layout: &RegionLayout, inset: f32) -> Vec2 {
        let min = layout.bounds_min + Vec2::splat(inset);
        let max = layout.bounds_max - Vec2::splat(inset);
        Vec2::new(rng.range_f32(min.x, max.x), rng.range_f32(min.y, max.y))
    }

    fn can_place(&self, p: Vec2, layout: &RegionLayout, spacing: f32) -> bool {
        layout.in_bounds(p, BOUNDS_INSET)
            && p.distance(layout.spawn) >= SPAWN_CLEARANCE
            && layout.distance_to_path(p) >= layout.path_half_width + PATH_CLEARANCE
            && layout.terrain.normal_at(p).y >= MIN_GROUND_NORMAL_Y
            && layout
                .trees
                .iter()
                .all(|t| t.position.distance(p) >= t.trunk_radius + TREE_CLEARANCE)
            && layout
                .rocks
                .iter()
                .all(|r| r.position.distance(p) >= r.radius + ROCK_CLEARANCE)
            && !self.grid.any_within(p, spacing, &self.positions)
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn position(&self, i: u32) -> Vec2 {
        self.positions[i as usize]
    }

    pub fn base(&self, i: u32) -> f32 {
        self.bases[i as usize]
    }

    pub fn species(&self, i: u32) -> usize {
        self.species[i as usize] as usize
    }

    pub fn yaw(&self, i: u32) -> f32 {
        self.yaws[i as usize]
    }

    pub fn scale(&self, i: u32) -> f32 {
        self.scales[i as usize]
    }

    /// World position of the flower's base (on the ground).
    pub fn ground_point(&self, i: u32) -> Vec3 {
        let p = self.position(i);
        Vec3::new(p.x, self.base(i), p.y)
    }

    /// Centre and radius of the sphere used for look-at picking: around the
    /// bloom, generous enough to be easy to target.
    pub fn pick_sphere(&self, i: u32) -> (Vec3, f32) {
        let h = self.bloom_heights[i as usize];
        let radius = (0.12 + h * 0.28).clamp(0.22, 0.6);
        let center = self.ground_point(i) + Vec3::Y * (h - radius * 0.5).max(radius * 0.6);
        (center, radius)
    }

    /// Unharvested flowers within `radius` of `center`, in index order.
    pub fn query_radius(&self, center: Vec2, radius: f32, harvested: &BitSet) -> Vec<u32> {
        let mut out = Vec::new();
        self.grid.for_each_in_rect(
            center - Vec2::splat(radius),
            center + Vec2::splat(radius),
            |i| {
                if !harvested.get(i)
                    && self.positions[i as usize].distance_squared(center) <= radius * radius
                {
                    out.push(i);
                }
            },
        );
        out.sort_unstable();
        out
    }

    /// The nearest unharvested flower whose pick sphere the ray hits within
    /// `max_distance`. Returns `(index, distance along the ray)`.
    pub fn pick(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_distance: f32,
        harvested: &BitSet,
    ) -> Option<(u32, f32)> {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        let end = origin + dir * max_distance;
        let a = Vec2::new(origin.x, origin.z);
        let b = Vec2::new(end.x, end.z);
        let pad = Vec2::splat(0.6);
        let mut best: Option<(u32, f32)> = None;
        self.grid
            .for_each_in_rect(a.min(b) - pad, a.max(b) + pad, |i| {
                if harvested.get(i) {
                    return;
                }
                let (center, radius) = self.pick_sphere(i);
                if let Some(t) = ray_sphere(origin, dir, center, radius) {
                    if t <= max_distance && best.is_none_or(|(_, bt)| t < bt) {
                        best = Some((i, t));
                    }
                }
            });
        best
    }
}

/// Distance along a normalised ray to the first hit with a sphere (0 if the
/// origin is inside it).
fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let disc = b * b - c;
    if disc < 0.0 || b > 0.0 {
        return None;
    }
    Some(-b - disc.sqrt())
}

/// Uniform grid of flower indices.
#[derive(Debug, Clone)]
struct Grid {
    origin: Vec2,
    cell: f32,
    nx: usize,
    nz: usize,
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new(min: Vec2, max: Vec2, cell: f32) -> Self {
        let size = max - min;
        let nx = (size.x / cell).ceil().max(1.0) as usize;
        let nz = (size.y / cell).ceil().max(1.0) as usize;
        Self {
            origin: min,
            cell,
            nx,
            nz,
            cells: vec![Vec::new(); nx * nz],
        }
    }

    fn cell_of(&self, p: Vec2) -> (usize, usize) {
        let g = (p - self.origin) / self.cell;
        (
            (g.x.floor().max(0.0) as usize).min(self.nx - 1),
            (g.y.floor().max(0.0) as usize).min(self.nz - 1),
        )
    }

    fn insert(&mut self, p: Vec2, i: u32) {
        let (x, z) = self.cell_of(p);
        self.cells[z * self.nx + x].push(i);
    }

    fn for_each_in_rect(&self, min: Vec2, max: Vec2, mut f: impl FnMut(u32)) {
        let (x0, z0) = self.cell_of(min);
        let (x1, z1) = self.cell_of(max);
        for z in z0..=z1 {
            for x in x0..=x1 {
                for &i in &self.cells[z * self.nx + x] {
                    f(i);
                }
            }
        }
    }

    fn any_within(&self, p: Vec2, r: f32, positions: &[Vec2]) -> bool {
        let mut found = false;
        self.for_each_in_rect(p - Vec2::splat(r), p + Vec2::splat(r), |i| {
            found |= positions[i as usize].distance_squared(p) < r * r;
        });
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_sphere_hits_front_surface() {
        let t = ray_sphere(Vec3::ZERO, Vec3::Z, Vec3::new(0.0, 0.0, 5.0), 1.0).unwrap();
        assert!((t - 4.0).abs() < 1e-5);
        assert_eq!(
            ray_sphere(Vec3::ZERO, Vec3::Z, Vec3::new(3.0, 0.0, 5.0), 1.0),
            None
        );
        assert_eq!(
            ray_sphere(Vec3::ZERO, -Vec3::Z, Vec3::new(0.0, 0.0, 5.0), 1.0),
            None
        );
        assert_eq!(ray_sphere(Vec3::ZERO, Vec3::Z, Vec3::ZERO, 1.0), Some(0.0));
    }
}
