//! Static world colliders for the character controller.
//!
//! The world's solid objects are trees, rocks, fences and building walls —
//! all static and well approximated by vertical cylinders and oriented boxes
//! with a vertical extent. Against these a "collide and slide" kinematic
//! controller is simple, exact and deterministic, which is why the game uses
//! this instead of a general rigid-body physics engine (see architecture.md).
//!
//! Collider counts per region are in the tens to low hundreds, so queries are
//! a linear scan. If profiling shows otherwise, bucket them in a uniform grid.

use glam::Vec2;

/// A vertical cylinder (tree trunk, rock, post).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircleCollider {
    pub center: Vec2,
    pub radius: f32,
    pub bottom: f32,
    pub top: f32,
}

/// A box rotated about the vertical axis (wall, fence rail, table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxCollider {
    pub center: Vec2,
    pub half_extents: Vec2,
    /// Rotation about +Y in radians (counter-clockwise seen from above).
    pub angle: f32,
    pub bottom: f32,
    pub top: f32,
}

impl BoxCollider {
    fn to_local(self, p: Vec2) -> Vec2 {
        let (s, c) = self.angle.sin_cos();
        let d = p - self.center;
        Vec2::new(d.x * c + d.y * s, -d.x * s + d.y * c)
    }

    fn to_world_dir(self, v: Vec2) -> Vec2 {
        let (s, c) = self.angle.sin_cos();
        Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    fn contains(self, p: Vec2) -> bool {
        let l = self.to_local(p);
        l.x.abs() <= self.half_extents.x && l.y.abs() <= self.half_extents.y
    }
}

#[derive(Debug, Clone, Default)]
pub struct ColliderSet {
    pub circles: Vec<CircleCollider>,
    pub boxes: Vec<BoxCollider>,
}

/// Vertical slab occupied by the character: `[feet + step, head]`. Anything
/// whose top is at or below `feet + step` is a step, not a wall.
#[derive(Debug, Clone, Copy)]
pub struct BodySlab {
    pub feet: f32,
    pub step: f32,
    pub head: f32,
}

impl BodySlab {
    fn blocked_by(&self, bottom: f32, top: f32) -> bool {
        top > self.feet + self.step && bottom < self.head
    }
}

impl ColliderSet {
    pub fn len(&self) -> usize {
        self.circles.len() + self.boxes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Highest collider top under `p` that the body can stand on (at most
    /// `step` above the feet). `None` if there is none.
    pub fn support_height(&self, p: Vec2, slab: BodySlab) -> Option<f32> {
        let limit = slab.feet + slab.step;
        let circles = self
            .circles
            .iter()
            .filter(|c| c.top <= limit && p.distance_squared(c.center) <= c.radius * c.radius)
            .map(|c| c.top);
        let boxes = self
            .boxes
            .iter()
            .filter(|b| b.top <= limit && b.contains(p))
            .map(|b| b.top);
        circles.chain(boxes).reduce(f32::max)
    }

    /// Push a circle of `radius` at `p` out of every blocking collider.
    /// Returns the corrected position and the last contact normal (for
    /// sliding), or `None` if nothing was touched.
    pub fn resolve(&self, mut p: Vec2, radius: f32, slab: BodySlab) -> (Vec2, Option<Vec2>) {
        let mut contact = None;
        // A few passes settle corners where two colliders meet.
        for _ in 0..4 {
            let mut moved = false;
            for c in self
                .circles
                .iter()
                .filter(|c| slab.blocked_by(c.bottom, c.top))
            {
                let d = p - c.center;
                let min = c.radius + radius;
                let dist_sq = d.length_squared();
                if dist_sq < min * min {
                    let dist = dist_sq.sqrt();
                    let n = if dist > 1e-6 { d / dist } else { Vec2::X };
                    p = c.center + n * min;
                    contact = Some(n);
                    moved = true;
                }
            }
            for b in self
                .boxes
                .iter()
                .filter(|b| slab.blocked_by(b.bottom, b.top))
            {
                let local = b.to_local(p);
                let closest = local.clamp(-b.half_extents, b.half_extents);
                let d = local - closest;
                let dist_sq = d.length_squared();
                if dist_sq >= radius * radius {
                    continue;
                }
                let local_n = if dist_sq > 1e-12 {
                    d / dist_sq.sqrt()
                } else {
                    // Centre inside the box: leave through the nearest face.
                    let pen = b.half_extents - local.abs();
                    if pen.x < pen.y {
                        Vec2::new(local.x.signum(), 0.0)
                    } else {
                        Vec2::new(0.0, local.y.signum())
                    }
                };
                let surface = if dist_sq > 1e-12 {
                    closest
                } else if local_n.x != 0.0 {
                    Vec2::new(b.half_extents.x * local_n.x, local.y)
                } else {
                    Vec2::new(local.x, b.half_extents.y * local_n.y)
                };
                let local_p = surface + local_n * radius;
                let n = b.to_world_dir(local_n);
                p = b.center + b.to_world_dir(local_p);
                contact = Some(n);
                moved = true;
            }
            if !moved {
                break;
            }
        }
        (p, contact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLAB: BodySlab = BodySlab {
        feet: 0.0,
        step: 0.35,
        head: 1.75,
    };

    fn tree() -> CircleCollider {
        CircleCollider {
            center: Vec2::ZERO,
            radius: 0.5,
            bottom: -1.0,
            top: 10.0,
        }
    }

    #[test]
    fn pushes_out_of_circle() {
        let set = ColliderSet {
            circles: vec![tree()],
            boxes: vec![],
        };
        let (p, n) = set.resolve(Vec2::new(0.6, 0.0), 0.35, SLAB);
        assert!((p.x - 0.85).abs() < 1e-5);
        assert_eq!(n, Some(Vec2::X));
    }

    #[test]
    fn low_collider_is_a_step_not_a_wall() {
        let rock = CircleCollider {
            center: Vec2::ZERO,
            radius: 0.6,
            bottom: -1.0,
            top: 0.3,
        };
        let set = ColliderSet {
            circles: vec![rock],
            boxes: vec![],
        };
        let (p, n) = set.resolve(Vec2::new(0.2, 0.0), 0.35, SLAB);
        assert_eq!(n, None);
        assert_eq!(p, Vec2::new(0.2, 0.0));
        assert_eq!(set.support_height(Vec2::new(0.2, 0.0), SLAB), Some(0.3));
        assert_eq!(set.support_height(Vec2::new(2.0, 0.0), SLAB), None);
    }

    #[test]
    fn collider_above_head_does_not_block() {
        let beam = BoxCollider {
            center: Vec2::ZERO,
            half_extents: Vec2::new(2.0, 0.2),
            angle: 0.0,
            bottom: 2.5,
            top: 3.0,
        };
        let set = ColliderSet {
            circles: vec![],
            boxes: vec![beam],
        };
        assert_eq!(set.resolve(Vec2::ZERO, 0.35, SLAB).1, None);
    }

    #[test]
    fn pushes_out_of_rotated_box() {
        let wall = BoxCollider {
            center: Vec2::ZERO,
            half_extents: Vec2::new(5.0, 0.25),
            angle: std::f32::consts::FRAC_PI_2, // runs along Z
            bottom: 0.0,
            top: 3.0,
        };
        let set = ColliderSet {
            circles: vec![],
            boxes: vec![wall],
        };
        let (p, n) = set.resolve(Vec2::new(0.4, 1.0), 0.35, SLAB);
        assert!((p.x - 0.6).abs() < 1e-4, "{p}");
        assert!((p.y - 1.0).abs() < 1e-4);
        let n = n.unwrap();
        assert!((n - Vec2::X).length() < 1e-4, "{n}");
    }

    #[test]
    fn centre_inside_box_exits_nearest_face() {
        let wall = BoxCollider {
            center: Vec2::ZERO,
            half_extents: Vec2::new(5.0, 0.5),
            angle: 0.0,
            bottom: 0.0,
            top: 3.0,
        };
        let set = ColliderSet {
            circles: vec![],
            boxes: vec![wall],
        };
        let (p, _) = set.resolve(Vec2::new(1.0, 0.1), 0.35, SLAB);
        assert!((p.y - 0.85).abs() < 1e-4, "{p}");
    }
}
