//! Region features that shape the walkable world: the gates between
//! regions, water (a stream or a pond carved into the terrain), bridges
//! where the footpath crosses water, and lantern posts along the path.
//!
//! All of it is deterministic from the region definition. Regions without
//! these features generate exactly as before (separate random streams), so
//! flower placement in older regions never moves.

use std::f32::consts::{PI, TAU};

use glam::Vec2;

use super::collision::{BoxCollider, CircleCollider};
use super::layout::{distance_to_segment, OUTSKIRTS};
use super::terrain::{smoothstep, Terrain};
use crate::data::{PondDef, StreamDef};
use crate::rng::SplitMix64;

/// Which end of a region a gate is at. Every region's footpath runs from
/// its south edge (+Z, the spawn) to its north edge (−Z): the north gate
/// leads on to the next region, the south gate back to the previous one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GateSide {
    North,
    South,
}

/// A gap in the fence where the footpath meets the region edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gate {
    pub side: GateSide,
    /// Centre of the opening, on the fence line.
    pub position: Vec2,
    /// Ground height at the opening.
    pub base: f32,
    /// Rotation about +Y; the gate's front faces into the region.
    pub yaw: f32,
}

/// Half the width of a gate's opening (posts stand just outside it).
pub const GATE_HALF_WIDTH: f32 = 1.5;
/// Walking this close to the fence inside a gate's opening goes through it.
pub const GATE_TRIGGER_DEPTH: f32 = 1.1;
/// Where you stand after coming through a gate (metres in from the fence).
pub const GATE_ARRIVAL_DEPTH: f32 = 4.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    Stream,
    Pond,
}

/// A body of water. The terrain is carved below `level` inside it; the
/// water surface is a flat plane at `level`.
#[derive(Debug, Clone, PartialEq)]
pub struct Water {
    pub kind: WaterKind,
    /// Stream: centre line (extends into the outskirts). Pond: one point,
    /// the centre.
    pub points: Vec<Vec2>,
    /// Stream: half the water width. Pond: radius.
    pub half_width: f32,
    /// Width of the sloping bank beyond `half_width`.
    pub bank: f32,
    /// Height of the water surface.
    pub level: f32,
    pub depth: f32,
}

impl Water {
    /// Distance from `p` to the centre line (stream) or centre (pond).
    pub fn centre_distance(&self, p: Vec2) -> f32 {
        match self.kind {
            WaterKind::Pond => p.distance(self.points[0]),
            WaterKind::Stream => self
                .points
                .windows(2)
                .map(|w| distance_to_segment(p, w[0], w[1]))
                .fold(f32::INFINITY, f32::min),
        }
    }

    /// Everything carved by the water, banks included.
    pub fn outer_radius(&self) -> f32 {
        self.half_width + self.bank
    }

    /// Bed height relative to the surrounding floor at distance `d` from the
    /// centre: the full depth in the middle, rising smoothly over the banks.
    fn carve(&self, d: f32) -> f32 {
        let t = (d / self.outer_radius()).clamp(0.0, 1.0);
        (WATER_DROP + self.depth) * (1.0 - smoothstep(t))
    }
}

/// The water surface sits this far below the surrounding floor.
const WATER_DROP: f32 = 0.18;

/// A wooden footbridge carrying the path over water. The deck rises in a
/// gentle arch from one bank to the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bridge {
    pub center: Vec2,
    /// Unit direction along the deck.
    pub dir: Vec2,
    pub half_length: f32,
    pub half_width: f32,
    /// Deck height at the start (`-dir` end) and the end (`+dir` end).
    pub ends: (f32, f32),
    pub arch: f32,
}

impl Bridge {
    /// Deck surface height at `u` in 0..=1 along the bridge.
    pub fn deck_height(&self, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        self.ends.0 + (self.ends.1 - self.ends.0) * u + self.arch * (PI * u).sin()
    }

    /// World point at `u` along the centre line.
    pub fn point(&self, u: f32) -> Vec2 {
        self.center + self.dir * (u * 2.0 - 1.0) * self.half_length
    }

    /// Rotation about +Y that maps the model's +X (length) onto `dir`.
    pub fn yaw(&self) -> f32 {
        (-self.dir.y).atan2(self.dir.x)
    }

    /// Deck slabs (walkable, in steps far below the step height) and a
    /// railing along each side.
    pub fn colliders(&self) -> Vec<BoxCollider> {
        let segments = ((self.half_length * 2.0) / 0.5).ceil().max(2.0) as usize;
        let seg_half = self.half_length / segments as f32;
        let angle = self.dir.y.atan2(self.dir.x);
        let mut out = Vec::with_capacity(segments + 2);
        for k in 0..segments {
            let u = (k as f32 + 0.5) / segments as f32;
            let top = self.deck_height(u);
            out.push(BoxCollider {
                center: self.point(u),
                half_extents: Vec2::new(seg_half + 0.02, self.half_width),
                angle,
                bottom: top - 0.45,
                top,
            });
        }
        let normal = Vec2::new(-self.dir.y, self.dir.x);
        let low = self.ends.0.min(self.ends.1);
        let high = self.deck_height(0.5).max(self.ends.0).max(self.ends.1);
        for side in [-1.0, 1.0] {
            out.push(BoxCollider {
                center: self.center + normal * side * (self.half_width + 0.05),
                // Railings stop short of the ends so stepping on is easy.
                half_extents: Vec2::new(self.half_length - 0.6, 0.06),
                angle,
                bottom: low - 1.0,
                top: high + 0.95,
            });
        }
        out
    }
}

/// A stream from beyond the west fence to beyond the east fence, in the
/// middle third of the region. Uses its own random stream.
pub fn stream_line(def: &StreamDef, half: Vec2, seed: u64) -> Water {
    let mut rng = SplitMix64::new(seed ^ 0x57EA_4D00);
    let z0 = rng.range_f32(-0.12, 0.08) * half.y * 2.0;
    let amp = half.y * rng.range_f32(0.08, 0.13);
    let freq = TAU / (half.x * 2.0) * rng.range_f32(1.0, 1.6);
    let phase = rng.range_f32(0.0, TAU);
    let wobble = rng.range_f32(0.0, TAU);
    let mut points = Vec::new();
    let end = half.x + OUTSKIRTS;
    let mut x = -end;
    while x <= end + 0.01 {
        let z = z0 + amp * (x * freq + phase).sin() + amp * 0.3 * (x * freq * 2.7 + wobble).sin();
        points.push(Vec2::new(x, z));
        x += 2.0;
    }
    Water {
        kind: WaterKind::Stream,
        points,
        half_width: def.width / 2.0,
        bank: 1.6,
        level: 0.0,
        depth: def.depth,
    }
}

/// A pond centred on `center`.
pub fn pond_at(def: &PondDef, center: Vec2) -> Water {
    Water {
        kind: WaterKind::Pond,
        points: vec![center],
        half_width: def.radius,
        bank: 1.8,
        level: 0.0,
        depth: def.depth,
    }
}

/// Carve `water` into the terrain: a level floor around it (a valley for a
/// stream, a hollow for a pond) blending back into the hills, and the
/// water bed below that. Sets `water.level`.
pub fn carve(terrain: &mut Terrain, water: &mut Water, bounds_min: Vec2, bounds_max: Vec2) {
    // Floor level: the average natural height along the water inside the
    // region, so the valley sits where the hills would have it.
    let inside: Vec<Vec2> = match water.kind {
        WaterKind::Pond => water.points.clone(),
        WaterKind::Stream => water
            .points
            .iter()
            .copied()
            .filter(|p| p.cmpge(bounds_min).all() && p.cmple(bounds_max).all())
            .collect(),
    };
    let floor = if inside.is_empty() {
        0.0
    } else {
        inside.iter().map(|&p| terrain.height_at(p)).sum::<f32>() / inside.len() as f32
    };
    let flat = water.outer_radius() + 2.5;
    let blend = 12.0;
    let w = water.clone();
    terrain.reshape(|p, h| {
        let d = w.centre_distance(p);
        if d >= flat + blend {
            return h;
        }
        let t = smoothstep((d - flat) / blend);
        floor + (h - floor) * t - w.carve(d)
    });
    water.level = floor - WATER_DROP;
}

/// Bridges wherever the footpath crosses `water`.
pub fn bridges_over(water: &Water, path: &[Vec2], terrain: &Terrain) -> Vec<Bridge> {
    if water.kind != WaterKind::Stream {
        return Vec::new();
    }
    let mut out: Vec<Bridge> = Vec::new();
    for seg in path.windows(2) {
        for w in water.points.windows(2) {
            let Some(c) = intersect(seg[0], seg[1], w[0], w[1]) else {
                continue;
            };
            if out.iter().any(|b| b.center.distance(c) < 4.0) {
                continue;
            }
            let dir = (seg[1] - seg[0]).normalize_or(Vec2::NEG_Y);
            // Walk out from the crossing until both ends stand on dry floor.
            let clear = water.outer_radius() + 0.4;
            let reach = |sign: f32| {
                let mut t = 0.0;
                while t < 20.0 && water.centre_distance(c + dir * sign * t) < clear {
                    t += 0.1;
                }
                t
            };
            let (back, front) = (reach(-1.0), reach(1.0));
            let half_length = (back + front) / 2.0 + 0.6;
            let center = c + dir * (front - back) / 2.0;
            let start = center - dir * half_length;
            let end = center + dir * half_length;
            out.push(Bridge {
                center,
                dir,
                half_length,
                half_width: 1.1,
                ends: (
                    terrain.height_at(start) + 0.08,
                    terrain.height_at(end) + 0.08,
                ),
                arch: 0.45,
            });
        }
    }
    out
}

/// Thin lantern post collider.
pub fn lantern_collider(p: Vec2, base: f32) -> CircleCollider {
    CircleCollider {
        center: p,
        radius: 0.14,
        bottom: base - 1.0,
        top: base + 2.5,
    }
}

/// Segment intersection point, if the segments cross.
fn intersect(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = b - a;
    let s = d - c;
    let denom = r.perp_dot(s);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (c - a).perp_dot(s) / denom;
    let u = (c - a).perp_dot(r) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a + r * t)
}
