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

use glam::Vec2;

use super::collision::{BoxCollider, CircleCollider, ColliderSet};
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
    pub trees: Vec<TreeProp>,
    pub rocks: Vec<RockProp>,
    /// Scenery trees outside the fence. Unreachable, so no colliders.
    pub backdrop_trees: Vec<TreeProp>,
    pub colliders: ColliderSet,
}

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

        // Spawn near the south edge (+Z), looking north into the region.
        let spawn = Vec2::new(0.0, bounds_max.y - 6.0);
        let path = footpath(&mut rng, spawn, bounds_min, bounds_max);

        let mut landmarks = place_landmarks(&mut rng, &path, spawn, bounds_min, bounds_max);
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
        let terrain = Terrain::generate(
            half_size,
            OUTSKIRTS,
            world.terrain_cell,
            &region.terrain,
            region.seed,
            &flatten,
        );
        for l in &mut landmarks {
            l.base = terrain.height_at(l.position);
        }

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
            trees: Vec::new(),
            rocks: Vec::new(),
            backdrop_trees: Vec::new(),
            colliders: ColliderSet::default(),
        };
        layout.place_trees(&mut rng, region.props.trees);
        layout.place_rocks(&mut rng, region.props.rocks);
        layout.place_backdrop(&mut rng);
        layout.build_colliders();
        layout
    }

    /// Distance from `p` to the footpath centre line.
    pub fn distance_to_path(&self, p: Vec2) -> f32 {
        self.path
            .windows(2)
            .map(|w| distance_to_segment(p, w[0], w[1]))
            .fold(f32::INFINITY, f32::min)
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

    /// Within `margin` of a landmark's footprint.
    pub fn near_landmark(&self, p: Vec2, margin: f32) -> bool {
        self.landmarks
            .iter()
            .any(|l| l.position.distance(p) < l.footprint + margin)
    }

    fn place_trees(&mut self, rng: &mut SplitMix64, count: u32) {
        let mut attempts = count * 60;
        while self.trees.len() < count as usize && attempts > 0 {
            attempts -= 1;
            let p = self.random_point(rng, 3.0);
            if p.distance(self.spawn) < SPAWN_CLEARANCE
                || self.near_landmark(p, 3.0)
                || self.distance_to_path(p) < TREE_PATH_CLEARANCE
                || self
                    .trees
                    .iter()
                    .any(|t| t.position.distance(p) < TREE_SPACING)
            {
                continue;
            }
            let tree = self.make_tree(rng, p, 4.5, 7.5);
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
            if self
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
                || self.trees.iter().any(|t| t.position.distance(p) < 3.0)
                || self
                    .rocks
                    .iter()
                    .any(|r| r.position.distance(p) < ROCK_SPACING)
            {
                continue;
            }
            // Mostly small stones, occasionally a boulder.
            let u = rng.next_f32();
            let radius = 0.3 + 1.0 * u * u;
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
                LandmarkKind::Signpost => set.circles.push(CircleCollider {
                    center: l.position,
                    radius: 0.14,
                    bottom: l.base - 1.0,
                    top: l.base + 2.3,
                }),
            }
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
fn place_landmarks(
    rng: &mut SplitMix64,
    path: &[Vec2],
    spawn: Vec2,
    min: Vec2,
    max: Vec2,
) -> Vec<Landmark> {
    let along = |i: usize| {
        let i = i.clamp(1, path.len() - 2);
        let tangent = (path[i + 1] - path[i - 1]).normalize_or(Vec2::NEG_Y);
        (path[i], Vec2::new(-tangent.y, tangent.x))
    };
    let side = if rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
    let (p, normal) = along(path.len() / 3);
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
    let mut well = None;
    for _ in 0..200 {
        let q = Vec2::new(
            rng.range_f32(min.x + 8.0, max.x - 8.0),
            rng.range_f32(min.y + 8.0, max.y - 8.0),
        );
        if path_distance(q) > 6.0 && q.distance(spawn) > 14.0 && q.distance(bench.position) > 10.0 {
            well = Some(q);
            break;
        }
    }
    let mut out = vec![bench, sign];
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

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}
