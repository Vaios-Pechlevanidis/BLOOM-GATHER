//! First-person character controller (kinematic "collide and slide").
//!
//! The body is a vertical cylinder of `radius` and `height` whose `position`
//! is at the feet. Each fixed step:
//!
//! 1. ages the jump-buffer and coyote timers,
//! 2. accelerates horizontal velocity towards the input (walk/sprint),
//! 3. starts a jump or applies gravity,
//! 4. moves horizontally, refusing too-steep terrain and sliding along
//!    colliders,
//! 5. resolves the vertical position against terrain and walkable collider
//!    tops (landing, stepping up, staying glued to the ground going downhill).
//!
//! It runs at a fixed rate in the app, so behaviour is independent of frame
//! rate and the same inputs always produce the same motion.

use glam::{Vec2, Vec3};

use crate::data::PlayerConfig;
use crate::world::{BodySlab, ColliderSet, Terrain};

/// What the player wants to do this step. `wish` is the desired horizontal
/// direction in world space (x, z); its length is clamped to 1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MoveInput {
    pub wish: Vec2,
    pub sprint: bool,
    /// The jump key was pressed since the previous step.
    pub jump: bool,
}

/// The static world the body moves through.
#[derive(Debug, Clone, Copy)]
pub struct Environment<'a> {
    pub terrain: &'a Terrain,
    pub colliders: &'a ColliderSet,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterBody {
    /// Feet position.
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    coyote: f32,
    jump_buffer: f32,
}

/// Things that happened during a step (for camera effects and sounds).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StepEvents {
    pub jumped: bool,
    /// Landed this step; `landing_speed` is the downward speed at impact.
    pub landed: bool,
    pub landing_speed: f32,
    /// Bumped into a collider or refused a slope.
    pub blocked: bool,
}

impl CharacterBody {
    /// Place a body standing on the ground at `at`.
    pub fn standing_at(at: Vec2, env: &Environment) -> Self {
        Self {
            position: Vec3::new(at.x, env.terrain.height_at(at), at.y),
            velocity: Vec3::ZERO,
            grounded: true,
            coyote: 0.0,
            jump_buffer: 0.0,
        }
    }

    pub fn ground_position(&self) -> Vec2 {
        Vec2::new(self.position.x, self.position.z)
    }

    pub fn horizontal_speed(&self) -> f32 {
        Vec2::new(self.velocity.x, self.velocity.z).length()
    }

    pub fn eye(&self, cfg: &PlayerConfig) -> Vec3 {
        self.position + Vec3::Y * cfg.eye_height
    }

    fn slab(&self, cfg: &PlayerConfig) -> BodySlab {
        BodySlab {
            feet: self.position.y,
            step: cfg.step_height,
            head: self.position.y + cfg.height,
        }
    }

    /// Ground height under `p`: terrain or a walkable collider top.
    fn ground_height(&self, p: Vec2, cfg: &PlayerConfig, env: &Environment) -> f32 {
        let terrain = env.terrain.height_at(p);
        match env.colliders.support_height(p, self.slab(cfg)) {
            Some(top) => terrain.max(top),
            None => terrain,
        }
    }

    pub fn step(
        &mut self,
        input: &MoveInput,
        dt: f32,
        cfg: &PlayerConfig,
        env: &Environment,
    ) -> StepEvents {
        let mut events = StepEvents::default();
        if dt <= 0.0 {
            return events;
        }
        let was_grounded = self.grounded;

        // 1. Timers.
        self.jump_buffer = if input.jump {
            cfg.jump_buffer_secs
        } else {
            (self.jump_buffer - dt).max(0.0)
        };
        self.coyote = if was_grounded {
            cfg.coyote_secs
        } else {
            (self.coyote - dt).max(0.0)
        };

        // 2. Horizontal velocity.
        let top_speed = if input.sprint {
            cfg.sprint_speed
        } else {
            cfg.walk_speed
        };
        let wish = input.wish.clamp_length_max(1.0) * top_speed;
        let rate = if !was_grounded {
            cfg.air_accel
        } else if wish != Vec2::ZERO {
            cfg.ground_accel
        } else {
            cfg.ground_decel
        };
        let mut vh = move_towards(Vec2::new(self.velocity.x, self.velocity.z), wish, rate * dt);

        // 3. Jump or gravity. Vertical motion uses the average velocity over
        //    the step (exact for constant gravity), so jump arcs don't depend
        //    on the step size.
        let mut vy = self.velocity.y;
        let mut dy = 0.0;
        if self.jump_buffer > 0.0 && (was_grounded || self.coyote > 0.0) {
            vy = cfg.jump_speed();
            self.grounded = false;
            self.coyote = 0.0;
            self.jump_buffer = 0.0;
            events.jumped = true;
        } else if was_grounded {
            vy = 0.0;
        }
        if !self.grounded || events.jumped {
            let v0 = vy;
            vy -= cfg.gravity * dt;
            dy = (v0 + vy) * 0.5 * dt;
        }

        // 4. Horizontal move with slope limit and collisions.
        let old = self.ground_position();
        let mut p = old + vh * dt;
        if was_grounded && !events.jumped {
            let n = env.terrain.normal_at(p);
            let too_steep = n.y < cfg.max_slope_deg.to_radians().cos();
            if too_steep && env.terrain.height_at(p) > self.position.y + 1e-3 {
                // Remove the uphill component of the move; keep the rest so
                // the player slides along the slope instead of sticking.
                let downhill = Vec2::new(n.x, n.z).normalize_or_zero();
                let delta = p - old;
                let into = delta.dot(downhill);
                if into < 0.0 {
                    p -= downhill * into;
                    vh -= downhill * vh.dot(downhill).min(0.0);
                    events.blocked = true;
                }
            }
        }
        let (resolved, contact) = env.colliders.resolve(p, cfg.radius, self.slab(cfg));
        if let Some(n) = contact {
            let into = vh.dot(n);
            if into < 0.0 {
                vh -= n * into;
            }
            events.blocked = true;
        }
        p = resolved;

        // 5. Vertical resolution.
        let mut y = self.position.y + dy;
        let ground = self.ground_height(p, cfg, env);
        if y <= ground {
            if !was_grounded {
                events.landed = true;
                events.landing_speed = -vy;
            }
            y = ground;
            vy = vy.max(0.0);
            self.grounded = true;
        } else if was_grounded && !events.jumped && y - ground <= cfg.ground_snap {
            // Walking downhill or off a small step: stay on the ground.
            y = ground;
            vy = 0.0;
            self.grounded = true;
        } else {
            self.grounded = false;
        }

        self.position = Vec3::new(p.x, y, p.y);
        self.velocity = Vec3::new(vh.x, vy, vh.y);
        events
    }
}

fn move_towards(current: Vec2, target: Vec2, max_delta: f32) -> Vec2 {
    let d = target - current;
    let len = d.length();
    if len <= max_delta || len < 1e-6 {
        target
    } else {
        current + d / len * max_delta
    }
}

/// Convert yaw (radians about +Y, 0 = facing −Z) and a local input vector
/// (x = right, y = forward) into a world-space horizontal direction (x, z).
pub fn wish_direction(yaw: f32, local: Vec2) -> Vec2 {
    let (s, c) = yaw.sin_cos();
    let forward = Vec2::new(-s, -c);
    let right = Vec2::new(c, -s);
    (right * local.x + forward * local.y).clamp_length_max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wish_direction_follows_yaw() {
        let f = wish_direction(0.0, Vec2::Y);
        assert!((f - Vec2::new(0.0, -1.0)).length() < 1e-6);
        let r = wish_direction(0.0, Vec2::X);
        assert!((r - Vec2::new(1.0, 0.0)).length() < 1e-6);
        // Turning left 90° (positive yaw) makes forward point to −X.
        let f = wish_direction(std::f32::consts::FRAC_PI_2, Vec2::Y);
        assert!((f - Vec2::new(-1.0, 0.0)).length() < 1e-5);
        // Diagonals are not faster.
        assert!((wish_direction(0.3, Vec2::new(1.0, 1.0)).length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn move_towards_reaches_target() {
        assert_eq!(move_towards(Vec2::ZERO, Vec2::X, 2.0), Vec2::X);
        assert_eq!(move_towards(Vec2::ZERO, Vec2::X * 4.0, 1.0), Vec2::X);
    }
}
