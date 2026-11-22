//! Deterministic region layout: terrain, footpath, spawn point, solid props
//! and their colliders.
//!
//! Everything here derives from the region definition and its seed, so the
//! same region always has the same shape on every machine. Flower placement
//! (Phase B) builds on this layout and avoids the path, props and spawn.
//!
//! Props are decoration that happens to be solid. They are never flowers and
//! never count towards collection or completion.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::collision::{BoxCollider, CircleCollider, ColliderSet};
use super::features::{
    self, Bridge, Gate, GateSide, Water, GATE_ARRIVAL_DEPTH, GATE_HALF_WIDTH, GATE_TRIGGER_DEPTH,
};
use super::grid::SpatialGrid;
use super::terrain::{FlattenArea, Terrain};
use crate::data::{RegionDef, WorldConfig};
use crate::rng::SplitMix64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeProp {
    pub position: Vec2,
    /// Ground height under the trunk.
    pub base: f32,
    pub height: f32,
    pub trunk_radius: f32,
    pub canopy_radius: f32,
    pub yaw: f32,
    pub variant: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RockProp {
    pub position: Vec2,
    pub base: f32,
    /// Horizontal radius.
    pub radius: f32,
    pub height: f32,
    pub yaw: f32,
    pub variant: u8,
}

/// A recognisable, walk-around point of interest (solid, never harvestable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LandmarkKind {
    /// A wooden bench beside the footpath, facing it.
    Bench,
    /// A stone well with a little roof, in an open spot.
    Well,
    /// A signpost where the path leaves the region.
    Signpost,
    /// The florist's cottage workshop (walk-in).
    Workshop,
    /// A lantern post beside the footpath.
    Lantern,
}

/// Things in the world the player can use with E.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractableKind {
    /// Craft bouquets.
    Workbench,
    /// Buy tools.
    ToolShelf,
    /// Open the basket (inventory).
    Storage,
    /// See and sell finished bouquets.
    BouquetDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interactable {
    pub kind: InteractableKind,
    /// Centre of the target sphere (world space).
    pub position: Vec3,
    pub radius: f32,
}

/// Workshop dimensions (metres, local space; the door is in the +Z wall).
pub mod workshop {
    use glam::Vec2;
    pub const HALF: Vec2 = Vec2::new(3.5, 3.0);
    pub const WALL: f32 = 0.25;
    pub const WALL_HEIGHT: f32 = 2.7;
    pub const DOOR_HALF_WIDTH: f32 = 0.85;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landmark {
    pub kind: LandmarkKind,
    pub position: Vec2,
    /// Ground height (the ground is flattened under landmarks).
    pub base: f32,
    /// Rotation about +Y; the model's front (+Z) faces this way.
    pub yaw: f32,
    /// Radius kept clear of flowers, grass and props.
    pub footprint: f32,
}

#[derive(Debug, Clone)]
pub struct RegionLayout {
    pub terrain: Terrain,
    /// Half the region size (the region spans `-half_size..half_size`).
    pub half_size: Vec2,
    /// The walkable rectangle; invisible walls sit just outside it.
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
    pub spawn: Vec2,
    /// Facing at spawn (radians about +Y; 0 looks towards −Z).
    pub spawn_yaw: f32,
    /// Footpath centre line from the spawn across the region.
    pub path: Vec<Vec2>,
    pub path_half_width: f32,
    pub landmarks: Vec<Landmark>,
    pub interactables: Vec<Interactable>,
    pub trees: Vec<TreeProp>,
    pub rocks: Vec<RockProp>,
    /// Scenery trees outside the fence. Unreachable, so no colliders.
    pub backdrop_trees: Vec<TreeProp>,
    /// Gates where the footpath meets the north and south fence. Which of
    /// them lead anywhere depends on the region's neighbours.
    pub gates: [Gate; 2],
    pub water: Vec<Water>,
    pub bridges: Vec<Bridge>,
    pub colliders: ColliderSet,
    /// Footpath segments by cell (for `distance_to_path`).
    path_index: SpatialGrid,
    trees_index: SpatialGrid,
    rocks_index: SpatialGrid,
}

/// `distance_to_path` is exact below this distance; beyond it, it may
/// report just this value (every caller asks about the path's surroundings).
pub const PATH_NEAR: f32 = 8.0;
const INDEX_CELL: f32 = 8.0;

/// How far the terrain (and scenery) extends beyond the region edge.
pub const OUTSKIRTS: f32 = 30.0;
/// Trunk radius and canopy radius as a fraction of tree height, so the
/// renderer can scale one model uniformly and still match the collider.
pub const TRUNK_RADIUS_PER_HEIGHT: f32 = 0.05;
pub const CANOPY_RADIUS_PER_HEIGHT: f32 = 0.36;

const SPAWN_CLEARANCE: f32 = 12.0;
const TREE_PATH_CLEARANCE: f32 = 3.5;
const TREE_SPACING: f32 = 7.0;
const ROCK_PATH_CLEARANCE: f32 = 1.8;
const ROCK_SPACING: f32 = 2.5;

impl RegionLayout {
    pub fn generate(region: &RegionDef, world: &WorldConfig) -> Self {
        let half_size = Vec2::new(region.width, region.depth) / 2.0;
        let margin = world.boundary_margin;
        let bounds_min = -half_size + Vec2::splat(margin);
        let bounds_max = half_size - Vec2::splat(margin);
        let mut rng = SplitMix64::new(region.seed ^ 0x1A70_0075);

        let features = &region.scenery.features;

        // Spawn near the south edge (+Z), looking north into the region.
        let spawn = Vec2::new(0.0, bounds_max.y - 6.0);
        let path = footpath(&mut rng, spawn, bounds_min, bounds_max);

        // A stream is laid out first so landmarks can keep their feet dry.
        let mut water: Vec<Water> = features
            .stream
            .iter()
            .map(|s| features::stream_line(s, half_size, region.seed))
            .collect();
        let mut landmarks = place_landmarks(
            &mut rng,
            &path,
            spawn,
            bounds_min,
            bounds_max,
            region.workshop,
            &|p| wet(&water, p, 4.0),
        );
        let mut flatten = vec![FlattenArea {
            center: spawn,
            inner_radius: 5.0,
            outer_radius: 11.0,
        }];
        flatten.extend(landmarks.iter().map(|l| FlattenArea {
            center: l.position,
            inner_radius: l.footprint,
            outer_radius: l.footprint + 2.5,
        }));
        let mut terrain = Terrain::generate(
            half_size,
            OUTSKIRTS,
            world.terrain_cell,
            &region.terrain,
            region.seed,
            &flatten,
        );
        for w in &mut water {
            features::carve(&mut terrain, w, bounds_min, bounds_max);
        }
        if let Some(pond) = &features.pond {
            let mut prng = SplitMix64::new(region.seed ^ 0x90DD_0E5E);
            let clear = pond.radius + 2.0;
            for _ in 0..400 {
                let q = Vec2::new(
                    prng.range_f32(bounds_min.x + clear + 6.0, bounds_max.x - clear - 6.0),
                    prng.range_f32(bounds_min.y + clear + 8.0, bounds_max.y - clear - 8.0),
                );
                let dry_path = path
                    .windows(2)
                    .all(|w| distance_to_segment(q, w[0], w[1]) > clear + 4.0);
                if dry_path
                    && q.distance(spawn) > clear + 12.0
                    && landmarks
                        .iter()
                        .all(|l| q.distance(l.position) > clear + l.footprint + 4.0)
                    && !wet(&water, q, clear + 6.0)
                {
                    let mut w = features::pond_at(pond, q);
                    features::carve(&mut terrain, &mut w, bounds_min, bounds_max);
                    water.push(w);
                    break;
                }
            }
        }
        for l in &mut landmarks {
            l.base = terrain.height_at(l.position);
        }
        let gate = |side: GateSide| {
            let (x, z, yaw) = match side {
                GateSide::North => (path[path.len() - 1].x, bounds_min.y, 0.0),
                GateSide::South => (path[0].x, bounds_max.y, std::f32::consts::PI),
            };
            let position = Vec2::new(x, z);
            Gate {
                side,
                position,
                base: terrain.height_at(position),
                yaw,
            }
        };
        let gates = [gate(GateSide::North), gate(GateSide::South)];

        let mut layout = Self {
            terrain,
            half_size,
            bounds_min,
            bounds_max,
            spawn,
            spawn_yaw: 0.0,
            path,
            path_half_width: 1.1,
            landmarks,
            interactables: Vec::new(),
            trees: Vec::new(),
            rocks: Vec::new(),
            backdrop_trees: Vec::new(),
            gates,
            water,
            bridges: Vec::new(),
            colliders: ColliderSet::default(),
            path_index: SpatialGrid::new(Vec2::ZERO, Vec2::ONE, 1.0),
            trees_index: SpatialGrid::new(Vec2::ZERO, Vec2::ONE, 1.0),
            rocks_index: SpatialGrid::new(Vec2::ZERO, Vec2::ONE, 1.0),
        };
        layout.build_indexes();
        layout.bridges = layout
            .water
            .iter()
            .flat_map(|w| features::bridges_over(w, &layout.path, &layout.terrain))
            .collect();
        let (min_h, max_h) = features.tree_height.unwrap_or((4.5, 7.5));
        layout.place_interactables();
        layout.place_trees(&mut rng, region.props.trees, min_h, max_h);
        layout.place_rocks(&mut rng, region.props.rocks);
        layout.place_backdrop(&mut rng);
        layout.place_lanterns(features.lanterns);
        layout.build_colliders();
        layout
    }

    /// Within `margin` of water (banks included).
    pub fn in_water(&self, p: Vec2, margin: f32) -> bool {
        wet(&self.water, p, margin)
    }

    /// Height of the water surface at `p`, if `p` is over water.
    pub fn water_level_at(&self, p: Vec2) -> Option<f32> {
        self.water
            .iter()
            .find(|w| {
                self.terrain.height_at(p) < w.level && w.centre_distance(p) < w.outer_radius()
            })
            .map(|w| w.level)
    }

    pub fn gate(&self, side: GateSide) -> &Gate {
        &self.gates[match side {
            GateSide::North => 0,
            GateSide::South => 1,
        }]
    }

    /// The gate whose opening `p` stands in (walking into it goes through).
    pub fn gate_at(&self, p: Vec2) -> Option<GateSide> {
        self.gates
            .iter()
            .find(|g| {
                (p.x - g.position.x).abs() < GATE_HALF_WIDTH - 0.2
                    && (p.y - g.position.y).abs() < GATE_TRIGGER_DEPTH
            })
            .map(|g| g.side)
    }

    /// Distance from `p` to the nearest gate opening's centre.
    pub fn gate_distance(&self, side: GateSide, p: Vec2) -> f32 {
        self.gate(side).position.distance(p)
    }

    /// Where the player stands, and faces, after coming through a gate.
    pub fn arrival(&self, side: GateSide) -> (Vec2, f32) {
        match side {
            GateSide::South => (self.spawn, self.spawn_yaw),
            GateSide::North => {
                let g = self.gate(GateSide::North);
                (
                    Vec2::new(g.position.x, g.position.y + GATE_ARRIVAL_DEPTH),
                    std::f32::consts::PI,
                )
            }
        }
    }

    /// Lantern posts at even steps along the path, alternating sides.
    fn place_lanterns(&mut self, count: u32) {
        if count == 0 {
            return;
        }
        let mut lengths = vec![0.0];
        for w in self.path.windows(2) {
            lengths.push(lengths.last().unwrap() + w[0].distance(w[1]));
        }
        let total = *lengths.last().unwrap();
        let (start, end) = (8.0, total - 6.0);
        let step = (end - start) / count.max(1) as f32;
        for k in 0..count {
            let s = start + step * (k as f32 + 0.5);
            let i = lengths
                .partition_point(|&l| l < s)
                .clamp(1, self.path.len() - 1);
            let (a, b) = (self.path[i - 1], self.path[i]);
            let t =
                ((s - lengths[i - 1]) / (lengths[i] - lengths[i - 1]).max(1e-4)).clamp(0.0, 1.0);
            let on = a.lerp(b, t);
            let tangent = (b - a).normalize_or(Vec2::NEG_Y);
            let normal = Vec2::new(-tangent.y, tangent.x);
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let p = on + normal * side * (self.path_half_width + 0.85);
            if p.distance(self.spawn) < 5.0
                || !self.in_bounds(p, 1.0)
                || self.near_landmark(p, 1.0)
                || self.in_water(p, 0.5)
                || self
                    .bridges
                    .iter()
                    .any(|b| b.center.distance(p) < b.half_length + 1.5)
            {
                continue;
            }
            let facing = (on - p).normalize();
            self.landmarks.push(Landmark {
                kind: LandmarkKind::Lantern,
                position: p,
                base: self.terrain.height_at(p),
                yaw: facing.x.atan2(facing.y),
                footprint: 0.45,
            });
        }
    }

    /// Distance from `p` to the footpath centre line: exact up to
    /// [`PATH_NEAR`] metres, at least `PATH_NEAR` beyond that.
    pub fn distance_to_path(&self, p: Vec2) -> f32 {
        self.path_index
            .at(p)
            .iter()
            .map(|&k| distance_to_segment(p, self.path[k as usize], self.path[k as usize + 1]))
            .fold(PATH_NEAR, f32::min)
    }

    /// Spatial indexes over the whole terrain (region and outskirts). The
    /// path's are filled now; trees and rocks as they are placed.
    fn build_indexes(&mut self) {
        let extent = self.half_size + Vec2::splat(OUTSKIRTS);
        let grid = || SpatialGrid::new(-extent, extent, INDEX_CELL);
        self.path_index = grid();
        self.trees_index = grid();
        self.rocks_index = grid();
        for (k, w) in self.path.windows(2).enumerate() {
            let pad = Vec2::splat(PATH_NEAR);
            self.path_index
                .insert_rect(w[0].min(w[1]) - pad, w[0].max(w[1]) + pad, k as u32);
        }
    }

    /// Whether `test` holds for any tree within `r` metres of `p` (by trunk
    /// position).
    pub fn any_tree_near(&self, p: Vec2, r: f32, mut test: impl FnMut(&TreeProp) -> bool) -> bool {
        self.trees_index
            .any_near(p, r, |i| test(&self.trees[i as usize]))
    }

    /// Whether `test` holds for any rock within `r` metres of `p`.
    pub fn any_rock_near(&self, p: Vec2, r: f32, mut test: impl FnMut(&RockProp) -> bool) -> bool {
        self.rocks_index
            .any_near(p, r, |i| test(&self.rocks[i as usize]))
    }

    pub fn in_bounds(&self, p: Vec2, inset: f32) -> bool {
        p.cmpge(self.bounds_min + Vec2::splat(inset)).all()
            && p.cmple(self.bounds_max - Vec2::splat(inset)).all()
    }

    fn random_point(&self, rng: &mut SplitMix64, inset: f32) -> Vec2 {
        let min = self.bounds_min + Vec2::splat(inset);
        let max = self.bounds_max - Vec2::splat(inset);
        Vec2::new(rng.range_f32(min.x, max.x), rng.range_f32(min.y, max.y))
    }

    fn place_interactables(&mut self) {
        let Some(w) = self
            .landmarks
            .iter()
            .find(|l| l.kind == LandmarkKind::Workshop)
            .copied()
        else {
            return;
        };
        for (kind, local, height) in [
            (InteractableKind::Workbench, Vec2::new(0.0, -2.35), 1.0),
            (InteractableKind::ToolShelf, Vec2::new(-3.0, -0.6), 1.2),
            (InteractableKind::Storage, Vec2::new(2.85, -1.2), 0.6),
            (InteractableKind::BouquetDisplay, Vec2::new(2.5, 1.5), 1.0),
        ] {
            let p = to_world(w.position, w.yaw, local);
            self.interactables.push(Interactable {
                kind,
                position: Vec3::new(p.x, w.base + height, p.y),
                radius: 0.75,
            });
        }
    }

    /// The interactable whose sphere the ray hits first within `range`.
    pub fn pick_interactable(
        &self,
        origin: Vec3,
        dir: Vec3,
        range: f32,
    ) -> Option<(InteractableKind, f32)> {
        let dir = dir.normalize_or_zero();
        self.interactables
            .iter()
            .filter_map(|i| {
                let oc = origin - i.position;
                let b = oc.dot(dir);
                let c = oc.length_squared() - i.radius * i.radius;
                let disc = b * b - c;
                if c <= 0.0 {
                    return Some((i.kind, 0.0f32));
                }
                if disc < 0.0 || b > 0.0 {
                    return None;
                }
                let t = -b - disc.sqrt();
                (t <= range).then_some((i.kind, t))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Within `margin` of a landmark's footprint.
    pub fn near_landmark(&self, p: Vec2, margin: f32) -> bool {
        self.landmarks
            .iter()
            .any(|l| l.position.distance(p) < l.footprint + margin)
    }

    fn place_trees(&mut self, rng: &mut SplitMix64, count: u32, min_h: f32, max_h: f32) {
        let mut attempts = count * 60;
        while self.trees.len() < count as usize && attempts > 0 {
            attempts -= 1;
            let p = self.random_point(rng, 3.0);
            if p.distance(self.spawn) < SPAWN_CLEARANCE
                || self.near_landmark(p, 3.0)
                || self.distance_to_path(p) < TREE_PATH_CLEARANCE
                || self.in_water(p, 1.5)
                || self.any_tree_near(p, TREE_SPACING, |t| t.position.distance(p) < TREE_SPACING)
            {
                continue;
            }
            let tree = self.make_tree(rng, p, min_h, max_h);
            self.trees_index.insert(p, self.trees.len() as u32);
            self.trees.push(tree);
        }
    }

    fn make_tree(&self, rng: &mut SplitMix64, p: Vec2, min_h: f32, max_h: f32) -> TreeProp {
        let height = rng.range_f32(min_h, max_h);
        TreeProp {
            position: p,
            base: self.terrain.height_at(p),
            height,
            trunk_radius: height * TRUNK_RADIUS_PER_HEIGHT,
            canopy_radius: height * CANOPY_RADIUS_PER_HEIGHT,
            yaw: rng.range_f32(0.0, TAU),
            variant: rng.below(3) as u8,
        }
    }

    /// A loose ring of trees outside the fence so the world doesn't end at a
    /// visible edge. Density thins out with distance.
    fn place_backdrop(&mut self, rng: &mut SplitMix64) {
        let inner = self.half_size + Vec2::splat(2.5);
        let outer = self.half_size + Vec2::splat(OUTSKIRTS - 4.0);
        let band_area = (outer.x * outer.y - inner.x * inner.y) * 4.0;
        let target = (band_area / 40.0) as usize;
        let mut attempts = target * 20;
        while self.backdrop_trees.len() < target && attempts > 0 {
            attempts -= 1;
            let p = Vec2::new(
                rng.range_f32(-outer.x, outer.x),
                rng.range_f32(-outer.y, outer.y),
            );
            if p.x.abs() < inner.x && p.y.abs() < inner.y {
                continue;
            }
            let depth = (p.x.abs() - inner.x).max(p.y.abs() - inner.y);
            if rng.next_f32() < depth / (OUTSKIRTS * 1.6) {
                continue; // thin out further away
            }
            if self.in_water(p, 1.5)
                || self
                    .backdrop_trees
                    .iter()
                    .any(|t| t.position.distance(p) < 4.0)
            {
                continue;
            }
            let tree = self.make_tree(rng, p, 5.0, 9.0);
            self.backdrop_trees.push(tree);
        }
    }

    fn place_rocks(&mut self, rng: &mut SplitMix64, count: u32) {
        let mut attempts = count * 60;
        while self.rocks.len() < count as usize && attempts > 0 {
            attempts -= 1;
            let p = self.random_point(rng, 1.5);
            if p.distance(self.spawn) < 6.0
                || self.near_landmark(p, 1.2)
                || self.distance_to_path(p) < ROCK_PATH_CLEARANCE
                || self.in_water(p, 0.3)
                || self.any_tree_near(p, 3.0, |t| t.position.distance(p) < 3.0)
                || self.any_rock_near(p, ROCK_SPACING, |r| r.position.distance(p) < ROCK_SPACING)
            {
                continue;
            }
            // Mostly small stones, occasionally a boulder.
            let u = rng.next_f32();
            let radius = 0.3 + 1.0 * u * u;
            self.rocks_index.insert(p, self.rocks.len() as u32);
            self.rocks.push(RockProp {
                position: p,
                base: self.terrain.height_at(p),
                radius,
                height: radius * rng.range_f32(0.55, 0.95),
                yaw: rng.range_f32(0.0, TAU),
                variant: rng.below(3) as u8,
            });
        }
    }

    fn build_colliders(&mut self) {
        let mut set = ColliderSet::default();
        for t in &self.trees {
            set.circles.push(CircleCollider {
                center: t.position,
                radius: t.trunk_radius + 0.05,
                bottom: t.base - 1.0,
                top: t.base + t.height,
            });
        }
        for r in &self.rocks {
            // Rocks sit partly sunk into the ground; the collider matches the
            // visible part.
            set.circles.push(CircleCollider {
                center: r.position,
                radius: r.radius * 0.85,
                bottom: r.base - 1.0,
                top: r.base + r.height * 0.8,
            });
        }
        for l in &self.landmarks {
            match l.kind {
                LandmarkKind::Bench => set.boxes.push(BoxCollider {
                    center: l.position,
                    half_extents: Vec2::new(0.85, 0.32),
                    // Model X axis (bench length) maps to (cos yaw, -sin yaw).
                    angle: -l.yaw,
                    bottom: l.base - 1.0,
                    top: l.base + 0.95,
                }),
                LandmarkKind::Well => set.circles.push(CircleCollider {
                    center: l.position,
                    radius: 1.0,
                    bottom: l.base - 1.0,
                    top: l.base + 2.6,
                }),
                LandmarkKind::Workshop => set.boxes.extend(workshop_colliders(l)),
                LandmarkKind::Signpost => set.circles.push(CircleCollider {
                    center: l.position,
                    radius: 0.14,
                    bottom: l.base - 1.0,
                    top: l.base + 2.3,
                }),
                LandmarkKind::Lantern => set
                    .circles
                    .push(features::lantern_collider(l.position, l.base)),
            }
        }
        for b in &self.bridges {
            set.boxes.extend(b.colliders());
        }
        // Invisible walls just outside the walkable rectangle. The visible
        // fence follows the same line.
        let center = (self.bounds_min + self.bounds_max) / 2.0;
        let size = self.bounds_max - self.bounds_min;
        let t = 0.5;
        let wall = |c: Vec2, h: Vec2| BoxCollider {
            center: c,
            half_extents: h,
            angle: 0.0,
            bottom: -1000.0,
            top: 1000.0,
        };
        set.boxes.extend([
            wall(
                Vec2::new(center.x, self.bounds_min.y - t),
                Vec2::new(size.x / 2.0 + 2.0 * t, t),
            ),
            wall(
                Vec2::new(center.x, self.bounds_max.y + t),
                Vec2::new(size.x / 2.0 + 2.0 * t, t),
            ),
            wall(
                Vec2::new(self.bounds_min.x - t, center.y),
                Vec2::new(t, size.y / 2.0 + 2.0 * t),
            ),
            wall(
                Vec2::new(self.bounds_max.x + t, center.y),
                Vec2::new(t, size.y / 2.0 + 2.0 * t),
            ),
        ]);
        self.colliders = set;
    }
}

/// Bench beside the path a third of the way along, signpost where the path
/// leaves the region, well in an open spot away from both.
/// Local (x, z) in a landmark's frame → world (x, z). Matches Bevy's
/// `Quat::from_rotation_y(yaw)`: local +X → (cos, −sin), +Z → (sin, cos).
pub fn to_world(origin: Vec2, yaw: f32, local: Vec2) -> Vec2 {
    let (s, c) = yaw.sin_cos();
    origin + Vec2::new(local.x * c + local.y * s, -local.x * s + local.y * c)
}

/// Walls (with a door gap in the front) and solid furniture.
fn workshop_colliders(l: &Landmark) -> Vec<BoxCollider> {
    use workshop::*;
    let bx = |center: Vec2, half: Vec2, top: f32| BoxCollider {
        center: to_world(l.position, l.yaw, center),
        half_extents: half,
        angle: -l.yaw,
        bottom: l.base - 1.0,
        top: l.base + top,
    };
    let t = WALL / 2.0;
    let side = (HALF.x - DOOR_HALF_WIDTH) / 2.0;
    vec![
        bx(Vec2::new(0.0, -HALF.y), Vec2::new(HALF.x, t), WALL_HEIGHT),
        bx(Vec2::new(-HALF.x, 0.0), Vec2::new(t, HALF.y), WALL_HEIGHT),
        bx(Vec2::new(HALF.x, 0.0), Vec2::new(t, HALF.y), WALL_HEIGHT),
        bx(
            Vec2::new(-(DOOR_HALF_WIDTH + side), HALF.y),
            Vec2::new(side, t),
            WALL_HEIGHT,
        ),
        bx(
            Vec2::new(DOOR_HALF_WIDTH + side, HALF.y),
            Vec2::new(side, t),
            WALL_HEIGHT,
        ),
        // Furniture: workbench, tool shelf, storage chest, display table.
        bx(Vec2::new(0.0, -2.35), Vec2::new(1.25, 0.42), 0.95),
        bx(Vec2::new(-3.0, -0.6), Vec2::new(0.32, 1.2), 2.0),
        bx(Vec2::new(2.85, -1.2), Vec2::new(0.42, 0.34), 0.7),
        bx(Vec2::new(2.5, 1.5), Vec2::new(0.62, 0.42), 0.85),
    ]
}

fn place_landmarks(
    rng: &mut SplitMix64,
    path: &[Vec2],
    spawn: Vec2,
    min: Vec2,
    max: Vec2,
    with_workshop: bool,
    wet: &dyn Fn(Vec2) -> bool,
) -> Vec<Landmark> {
    let along = |i: usize| {
        let i = i.clamp(1, path.len() - 2);
        let tangent = (path[i + 1] - path[i - 1]).normalize_or(Vec2::NEG_Y);
        (path[i], Vec2::new(-tangent.y, tangent.x))
    };
    let side = if rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
    // A third of the way along, or the nearest dry spot to it.
    let third = path.len() / 3;
    let (p, normal) = (0..path.len())
        .flat_map(|k| [third + k, third.saturating_sub(k)])
        .map(along)
        .find(|&(p, n)| !wet(p + n * side * 2.4))
        .unwrap_or_else(|| along(third));
    let bench_pos = p + normal * side * 2.4;
    let facing = (p - bench_pos).normalize();
    let bench = Landmark {
        kind: LandmarkKind::Bench,
        position: bench_pos,
        base: 0.0,
        yaw: facing.x.atan2(facing.y),
        footprint: 1.2,
    };
    let (end, normal) = along(path.len() - 2);
    let sign = Landmark {
        kind: LandmarkKind::Signpost,
        position: end + normal * -side * 1.6,
        base: 0.0,
        yaw: 0.0, // faces south, towards a player walking up the path
        footprint: 0.5,
    };

    let path_distance = |q: Vec2| {
        path.windows(2)
            .map(|w| distance_to_segment(q, w[0], w[1]))
            .fold(f32::INFINITY, f32::min)
    };
    // The workshop sits beside the start of the path, door facing the spawn.
    let workshop = if with_workshop {
        [
            (-13.0, -6.0),
            (13.0, -6.0),
            (-16.0, -11.0),
            (16.0, -11.0),
            (-12.0, -16.0),
            (12.0, -16.0),
        ]
        .into_iter()
        .map(|(dx, dz)| spawn + Vec2::new(dx, dz))
        .find(|&q| {
            path_distance(q) > 8.0
                && q.x > min.x + 7.0
                && q.x < max.x - 7.0
                && q.y > min.y + 7.0
                && q.y < max.y - 6.0
                && q.distance(bench.position) > 9.0
        })
        .map(|q| {
            let facing = (spawn - q).normalize();
            Landmark {
                kind: LandmarkKind::Workshop,
                position: q,
                base: 0.0,
                yaw: facing.x.atan2(facing.y),
                footprint: 5.0,
            }
        })
    } else {
        None
    };
    let mut well = None;
    for _ in 0..200 {
        let q = Vec2::new(
            rng.range_f32(min.x + 8.0, max.x - 8.0),
            rng.range_f32(min.y + 8.0, max.y - 8.0),
        );
        let clear_of_workshop = workshop.is_none_or(|w| q.distance(w.position) > 10.0);
        if path_distance(q) > 6.0
            && !wet(q)
            && q.distance(spawn) > 14.0
            && q.distance(bench.position) > 10.0
            && clear_of_workshop
        {
            well = Some(q);
            break;
        }
    }
    let mut out = vec![bench, sign];
    out.extend(workshop);
    if let Some(q) = well {
        out.push(Landmark {
            kind: LandmarkKind::Well,
            position: q,
            base: 0.0,
            yaw: rng.range_f32(0.0, TAU),
            footprint: 1.6,
        });
    }
    out
}

/// A meandering path from the spawn northwards to the far edge.
fn footpath(rng: &mut SplitMix64, spawn: Vec2, min: Vec2, max: Vec2) -> Vec<Vec2> {
    let width = max.x - min.x;
    let amp = width * rng.range_f32(0.12, 0.2);
    let freq = TAU / (max.y - min.y) * rng.range_f32(0.7, 1.2);
    let phase = rng.range_f32(0.0, TAU);
    let offset = (spawn.y * freq + phase).sin();
    let mut points = Vec::new();
    let mut z = max.y;
    while z >= min.y {
        let x = amp * ((z * freq + phase).sin() - offset);
        points.push(Vec2::new(x.clamp(min.x + 2.0, max.x - 2.0), z));
        z -= 2.0;
    }
    points
}

fn wet(water: &[Water], p: Vec2, margin: f32) -> bool {
    water
        .iter()
        .any(|w| w.centre_distance(p) < w.outer_radius() + margin)
}

pub(crate) fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}
