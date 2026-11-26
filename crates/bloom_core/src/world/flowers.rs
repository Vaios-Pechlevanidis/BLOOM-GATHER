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

use super::grid::SpatialGrid as Grid;
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
const LANDMARK_CLEARANCE: f32 = 0.6;
const SPAWN_CLEARANCE: f32 = 2.5;
const WATER_CLEARANCE: f32 = 0.6;
const BOUNDS_INSET: f32 = 1.0;
/// Flowers don't grow on slopes steeper than ~32°.
const MIN_GROUND_NORMAL_Y: f32 = 0.85;
/// Average flowers per natural cluster.
const FLOWERS_PER_CLUSTER: u32 = 9;
/// Closest spacing along a bed's planting lines (metres).
const MIN_BED_STEP: f32 = 0.45;

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

        if let (Some(def), Some(plan)) = (&region.scenery.features.fields, &layout.fields) {
            field.plant_fields(region, layout, data, def, plan, &mut rng)?;
            return Ok(field);
        }

        let rows = region.scenery.features.rows.map(|r| {
            let a = r.angle_deg.to_radians();
            (Vec2::new(-a.sin(), a.cos()), r.spacing)
        });
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
                // Row fields: snap across the rows so flowers line up in
                // long parallel bands (a little jitter keeps them natural).
                let p = match rows {
                    Some((across, row_spacing)) => {
                        let v = p.dot(across);
                        let snapped =
                            (v / row_spacing).round() * row_spacing + rng.range_f32(-0.15, 0.15);
                        p + across * (snapped - v)
                    }
                    None => p,
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

    fn push(
        &mut self,
        p: Vec2,
        species_idx: usize,
        layout: &RegionLayout,
        data: &GameData,
        rng: &mut SplitMix64,
    ) {
        let height = data.species[species_idx].height;
        let scale = rng.range_f32(0.88, 1.12) * data.world.flower_scale;
        self.positions.push(p);
        self.bases.push(layout.terrain.height_at(p));
        self.species.push(species_idx as u16);
        self.yaws.push(rng.range_f32(0.0, TAU));
        self.scales.push(scale);
        self.bloom_heights.push(height * scale);
        self.grid.insert(p, self.positions.len() as u32 - 1);
    }

    /// A bulb farm: wild species in clusters on the verges and in the yard,
    /// every other species in its own beds (shortest next to the track,
    /// tallest at the back), strays scattered singly among the beds.
    fn plant_fields(
        &mut self,
        region: &RegionDef,
        layout: &RegionLayout,
        data: &GameData,
        def: &crate::data::BulbFields,
        plan: &super::fields::FieldPlan,
        rng: &mut SplitMix64,
    ) -> Result<(), PlacementError> {
        let spacing = data.world.min_flower_spacing;
        let index = |id: &str| {
            data.species_index(id)
                .expect("validated content references known species")
        };
        let fail = |species: &str, placed: u32, wanted: u32| PlacementError {
            region: region.id.clone(),
            species: species.to_string(),
            placed,
            wanted,
        };

        // Wildflowers: natural clusters off the beds.
        for entry in region
            .flowers
            .iter()
            .filter(|f| def.verge_species.contains(&f.species))
        {
            let species_idx = index(&entry.species);
            let wanted = entry.count;
            let clusters = wanted.div_ceil(FLOWERS_PER_CLUSTER).max(1);
            let off_beds = |p: Vec2| !plan.in_planted_area(p, 0.8);
            let mut centers = Vec::new();
            for _ in 0..clusters as usize * 200 {
                if centers.len() == clusters as usize {
                    break;
                }
                let c = self.random_point(rng, layout, BOUNDS_INSET + 1.0);
                if off_beds(c) && self.can_place(c, layout, spacing) {
                    centers.push(c);
                }
            }
            if centers.is_empty() {
                return Err(fail(&entry.species, 0, wanted));
            }
            let mut placed = 0;
            let mut attempts: u64 = 0;
            while placed < wanted {
                attempts += 1;
                if attempts > wanted as u64 * 3_000 {
                    return Err(fail(&entry.species, placed, wanted));
                }
                let p = if attempts <= wanted as u64 * 400 {
                    let center = centers[rng.below(centers.len() as u32) as usize];
                    let angle = rng.range_f32(0.0, TAU);
                    center + Vec2::new(angle.cos(), angle.sin()) * 4.0 * rng.next_f32().powf(0.8)
                } else {
                    self.random_point(rng, layout, BOUNDS_INSET)
                };
                if !off_beds(p) || !self.can_place(p, layout, spacing) {
                    continue;
                }
                self.push(p, species_idx, layout, data, rng);
                placed += 1;
            }
        }

        // Beds: every free spot along the planting lines, then as many of
        // them as there are bed flowers, evenly spread.
        let mut beds: Vec<(usize, u32)> = region
            .flowers
            .iter()
            .filter(|f| !def.verge_species.contains(&f.species))
            .map(|f| (index(&f.species), f.count))
            .collect();
        let total: u32 = beds.iter().map(|&(_, n)| n).sum();
        let mut order: Vec<usize> = (0..plan.beds.len()).collect();
        // Next to the track first; within a ring, alternate sides and blocks.
        order.sort_by_key(|&i| plan.beds[i].ring);
        // Start from the step that would just fit everything and tighten it
        // until the free spots (some are lost to lanterns and the like)
        // suffice; the few spare ones are skipped evenly.
        let length: f32 = plan.beds.iter().map(|b| b.z1 - b.z0 - 0.6).sum();
        let mut step = length * plan.lines as f32 / total.max(1) as f32;
        let slots = loop {
            let mut slots = Vec::new();
            for &i in &order {
                for c in plan.candidates(&plan.beds[i], step) {
                    if self.can_plant(c, layout) {
                        slots.push(c);
                    }
                }
            }
            if slots.len() as u32 >= total {
                break slots;
            }
            step *= 0.98;
            if step < MIN_BED_STEP {
                return Err(fail("bed flowers", slots.len() as u32, total));
            }
        };
        let chosen: Vec<Vec2> = (0..total as usize)
            .map(|k| slots[((k as f64 + 0.5) * slots.len() as f64 / total as f64) as usize])
            .collect();

        // Strays take random spots; the rest are filled bed by bed, shortest
        // species first.
        let mut species_at: Vec<Option<usize>> = vec![None; chosen.len()];
        for (species_idx, count) in beds
            .iter()
            .filter(|&&(s, _)| def.stray_species.contains(&data.species[s].id))
        {
            let mut left = *count;
            while left > 0 {
                let k = rng.below(chosen.len() as u32) as usize;
                if species_at[k].is_none() {
                    species_at[k] = Some(*species_idx);
                    left -= 1;
                }
            }
        }
        beds.retain(|&(s, _)| !def.stray_species.contains(&data.species[s].id));
        beds.sort_by(|a, b| {
            data.species[a.0]
                .height
                .total_cmp(&data.species[b.0].height)
        });
        let mut queue = beds
            .into_iter()
            .flat_map(|(s, n)| std::iter::repeat_n(s, n as usize));
        for slot in &mut species_at {
            if slot.is_none() {
                *slot = queue.next();
            }
        }
        for (p, s) in chosen.into_iter().zip(species_at) {
            let s = s.expect("every chosen spot gets a species");
            // A touch of jitter so the lines don't look machine-planted.
            let p = p + Vec2::new(rng.range_f32(-0.06, 0.06), rng.range_f32(-0.1, 0.1));
            self.push(p, s, layout, data, rng);
        }
        Ok(())
    }

    /// Bed spots only need to keep clear of solid things, not of each other.
    fn can_plant(&self, p: Vec2, layout: &RegionLayout) -> bool {
        layout.in_bounds(p, BOUNDS_INSET)
            && p.distance(layout.spawn) >= SPAWN_CLEARANCE
            && layout.distance_to_path(p) >= layout.path_half_width + PATH_CLEARANCE
            && !layout.any_tree_near(p, 2.5, |t| {
                t.position.distance(p) < t.trunk_radius + TREE_CLEARANCE
            })
            && !layout.any_rock_near(p, 2.0, |r| {
                r.position.distance(p) < r.radius + ROCK_CLEARANCE
            })
            && !layout.near_landmark(p, LANDMARK_CLEARANCE)
            && !layout.in_water(p, WATER_CLEARANCE)
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
            && !layout.any_tree_near(p, 2.5, |t| {
                t.position.distance(p) < t.trunk_radius + TREE_CLEARANCE
            })
            && !layout.any_rock_near(p, 2.0, |r| {
                r.position.distance(p) < r.radius + ROCK_CLEARANCE
            })
            && !layout.near_landmark(p, LANDMARK_CLEARANCE)
            && !layout.in_water(p, WATER_CLEARANCE)
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
        let radius = pick_radius(h);
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

    /// Unharvested flowers inside the axis-aligned rectangle, in index order.
    pub fn query_rect(&self, min: Vec2, max: Vec2, harvested: &BitSet) -> Vec<u32> {
        let mut out = Vec::new();
        self.grid.for_each_in_rect(min, max, |i| {
            let p = self.positions[i as usize];
            if !harvested.get(i) && p.cmpge(min).all() && p.cmple(max).all() {
                out.push(i);
            }
        });
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
/// Radius of a flower's look-at sphere (its "hitbox") for a bloom
/// `bloom_height` metres above the ground.
pub fn pick_radius(bloom_height: f32) -> f32 {
    (0.12 + bloom_height * 0.28).clamp(0.22, 0.6)
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
