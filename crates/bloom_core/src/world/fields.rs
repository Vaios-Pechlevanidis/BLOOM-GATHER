//! Bulb-field beds: long, straight, raised strips of one colour each with
//! bare furrows between them, running north-south on both sides of the farm
//! track (see [`BulbFields`]).
//!
//! The plan is pure geometry derived from the region bounds, so layout
//! (terrain ridges), flower placement and the renderer (soil colours, leaf
//! foliage) all agree on where every bed is.

use glam::Vec2;

use crate::data::BulbFields;

/// One bed: a strip `x - half_width..x + half_width` by `z0..z1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bed {
    /// Centre line (X).
    pub x: f32,
    /// South-most and north-most ends; `z0 < z1`.
    pub z0: f32,
    pub z1: f32,
    /// How many beds lie between this one and the track (0 = next to it).
    pub ring: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldPlan {
    pub beds: Vec<Bed>,
    pub bed_width: f32,
    pub furrow_width: f32,
    pub lines: u32,
    /// Beds end here; the farmyard lies south of it (towards +Z).
    pub yard_z: f32,
    /// The whole planted rectangle (beds and furrows), both sides of the track.
    pub area_min: Vec2,
    pub area_max: Vec2,
    /// Inner edge of the planted strip on each side of the track (|x|).
    pub inner_x: f32,
}

/// Height of a bed's crown above the furrows (metres).
pub const RIDGE_HEIGHT: f32 = 0.14;

impl FieldPlan {
    /// Beds between `track_half_width` (+ a grass strip for lanterns) and the
    /// verge, and between the verge in the north and the farmyard in the
    /// south, split in two blocks by the headland.
    pub fn new(
        def: &BulbFields,
        bounds_min: Vec2,
        bounds_max: Vec2,
        track_half_width: f32,
    ) -> Self {
        let pitch = def.bed_width + def.furrow_width;
        let inner_x = track_half_width + 1.6;
        let outer_x = bounds_max.x.min(-bounds_min.x) - def.verge;
        let yard_z = bounds_max.y - def.yard_depth;
        let north = bounds_min.y + def.verge;
        let rings = (((outer_x - inner_x + def.furrow_width) / pitch).floor() as i32).max(0) as u32;
        let mid = (north + yard_z) / 2.0;
        let blocks: Vec<(f32, f32)> = if def.headland > 0.0 {
            vec![
                (mid + def.headland / 2.0, yard_z),
                (north, mid - def.headland / 2.0),
            ]
        } else {
            vec![(north, yard_z)]
        };
        let mut beds = Vec::new();
        for ring in 0..rings {
            let offset = inner_x + ring as f32 * pitch + def.bed_width / 2.0;
            for &(z0, z1) in &blocks {
                for side in [1.0, -1.0] {
                    if z1 - z0 > 2.0 {
                        beds.push(Bed {
                            x: side * offset,
                            z0,
                            z1,
                            ring,
                        });
                    }
                }
            }
        }
        let reach = inner_x + rings as f32 * pitch - def.furrow_width;
        Self {
            beds,
            bed_width: def.bed_width,
            furrow_width: def.furrow_width,
            lines: def.lines.max(1),
            yard_z,
            area_min: Vec2::new(-reach, north),
            area_max: Vec2::new(reach, yard_z),
            inner_x,
        }
    }

    pub fn pitch(&self) -> f32 {
        self.bed_width + self.furrow_width
    }

    /// Inside the planted rectangle on either side of the track (beds or
    /// furrows; the headland and track excluded), with `margin` around it.
    pub fn in_planted_area(&self, p: Vec2, margin: f32) -> bool {
        let ax = p.x.abs();
        ax >= self.inner_x - margin
            && ax <= self.area_max.x + margin
            && p.y >= self.area_min.y - margin
            && p.y <= self.area_max.y + margin
            && self.beds.iter().any(|b| {
                p.y >= b.z0 - margin && p.y <= b.z1 + margin && (b.x.signum() == p.x.signum())
            })
    }

    /// The bed `p` stands on, if any.
    pub fn bed_at(&self, p: Vec2) -> Option<&Bed> {
        let half = self.bed_width / 2.0;
        self.beds
            .iter()
            .find(|b| (p.x - b.x).abs() <= half && p.y >= b.z0 && p.y <= b.z1)
    }

    /// 1 on the crown of a bed, falling to 0 in the furrows (and 0 outside
    /// the planted area). Soft edges for terrain shape and colour.
    pub fn bed_weight(&self, p: Vec2) -> f32 {
        if !self.in_planted_area(p, 0.0) {
            return 0.0;
        }
        let half = self.bed_width / 2.0;
        self.beds
            .iter()
            .filter(|b| p.y >= b.z0 - 0.6 && p.y <= b.z1 + 0.6)
            .map(|b| {
                let across = 1.0 - smooth(half - 0.25, half + 0.2, (p.x - b.x).abs());
                let end =
                    (1.0 - smooth(-0.1, 0.6, b.z0 - p.y)) * (1.0 - smooth(-0.1, 0.6, p.y - b.z1));
                across * end
            })
            .fold(0.0, f32::max)
    }

    /// Positions along a bed's planting lines: candidates every `step`
    /// metres, neighbouring lines staggered by half a step. Ordered from the
    /// yard end northwards, line by line at each step.
    pub fn candidates(&self, bed: &Bed, step: f32) -> Vec<Vec2> {
        let lines = self.lines;
        let usable = self.bed_width * 0.72;
        let mut out = Vec::new();
        let len = bed.z1 - bed.z0 - 0.6;
        let n = (len / step).floor().max(0.0) as u32;
        for j in 0..n {
            for k in 0..lines {
                let x = if lines == 1 {
                    bed.x
                } else {
                    bed.x - usable / 2.0 + usable * k as f32 / (lines - 1) as f32
                };
                let stagger = if k % 2 == 1 { step * 0.5 } else { 0.0 };
                let z = bed.z1 - 0.3 - (j as f32 + 0.5) * step - stagger;
                if z > bed.z0 + 0.3 {
                    out.push(Vec2::new(x, z));
                }
            }
        }
        out
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> FieldPlan {
        let def = BulbFields::default();
        FieldPlan::new(&def, Vec2::new(-30.0, -40.0), Vec2::new(30.0, 40.0), 1.1)
    }

    #[test]
    fn beds_stay_between_track_verge_and_yard() {
        let p = plan();
        assert!(!p.beds.is_empty());
        for b in &p.beds {
            assert!(b.x.abs() - p.bed_width / 2.0 >= p.inner_x - 1e-4);
            assert!(b.x.abs() + p.bed_width / 2.0 <= 30.0 - 3.0 + 1e-4);
            assert!(b.z0 >= -40.0 + 3.0 - 1e-4 && b.z1 <= p.yard_z + 1e-4);
        }
    }

    #[test]
    fn crowns_are_beds_and_furrows_are_not() {
        let p = plan();
        let b = p.beds[0];
        let mid = Vec2::new(b.x, (b.z0 + b.z1) / 2.0);
        assert!(p.bed_weight(mid) > 0.99);
        assert!(p.bed_at(mid).is_some());
        let furrow = mid
            + Vec2::new(
                b.x.signum() * (p.bed_width / 2.0 + p.furrow_width / 2.0),
                0.0,
            );
        assert!(p.bed_weight(furrow) < 0.2);
        assert!(p.bed_at(furrow).is_none());
        assert_eq!(
            p.bed_weight(Vec2::new(0.0, 0.0)),
            0.0,
            "the track is no bed"
        );
    }

    #[test]
    fn candidates_lie_on_their_bed() {
        let p = plan();
        for b in &p.beds {
            for c in p.candidates(b, 0.5) {
                assert!((c.x - b.x).abs() <= p.bed_width / 2.0);
                assert!(c.y > b.z0 && c.y < b.z1);
            }
        }
    }
}
