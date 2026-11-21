//! XP sources and the player level curve.

use serde::{Deserialize, Serialize};

pub type Xp = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressionConfig {
    pub xp_per_harvest: Xp,
    pub xp_per_discovery: Xp,
    pub xp_per_craft_base: Xp,
    pub xp_per_craft_tier: Xp,
    pub xp_first_craft: Xp,
    pub xp_region_complete: Xp,
    pub skill_points_per_level: u32,
    pub level_curve: LevelCurve,
}

impl ProgressionConfig {
    /// XP for crafting one bouquet of the given recipe tier (excluding the
    /// one-off first-craft bonus).
    pub fn craft_xp(&self, tier: u8) -> Xp {
        self.xp_per_craft_base + self.xp_per_craft_tier * tier as Xp
    }
}

/// Linear-step level curve: going from level `L` to `L + 1` costs
/// `base + step × (L − 1)` XP. Levels start at 1 and stop at `max_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelCurve {
    pub base: Xp,
    pub step: Xp,
    pub max_level: u32,
}

impl LevelCurve {
    /// XP needed to advance from `level` to `level + 1`.
    /// Returns `None` at (or beyond) the level cap.
    pub fn xp_to_next(&self, level: u32) -> Option<Xp> {
        if level == 0 || level >= self.max_level {
            return None;
        }
        Some(self.base + self.step * (level as Xp - 1))
    }

    /// Total XP required to *reach* `level` from level 1 (level 1 = 0 XP).
    /// Levels above the cap are clamped to the cap.
    pub fn total_xp_for_level(&self, level: u32) -> Xp {
        // n = number of level-ups; Σ_{k=0}^{n-1} (base + step·k) = n·base + step·n(n−1)/2
        let n = level.clamp(1, self.max_level) as Xp - 1;
        n * self.base + self.step * n * n.saturating_sub(1) / 2
    }

    /// The level a player with `total_xp` lifetime XP has reached.
    pub fn level_for_xp(&self, total_xp: Xp) -> u32 {
        // At most `max_level` iterations (30 by default) — cheaper and clearer
        // than solving the quadratic with floating point.
        let mut level = 1;
        while level < self.max_level && self.total_xp_for_level(level + 1) <= total_xp {
            level += 1;
        }
        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURVE: LevelCurve = LevelCurve {
        base: 30,
        step: 20,
        max_level: 30,
    };

    #[test]
    fn xp_to_next_follows_linear_step() {
        assert_eq!(CURVE.xp_to_next(1), Some(30));
        assert_eq!(CURVE.xp_to_next(2), Some(50));
        assert_eq!(CURVE.xp_to_next(10), Some(210));
        assert_eq!(CURVE.xp_to_next(29), Some(590));
        assert_eq!(CURVE.xp_to_next(30), None);
        assert_eq!(CURVE.xp_to_next(0), None);
    }

    #[test]
    fn total_matches_sum_of_steps() {
        let mut running = 0;
        for level in 1..=CURVE.max_level {
            assert_eq!(CURVE.total_xp_for_level(level), running, "level {level}");
            running += CURVE.xp_to_next(level).unwrap_or(0);
        }
    }

    #[test]
    fn documented_milestones() {
        // Values quoted in docs/economy-balancing.md.
        assert_eq!(CURVE.total_xp_for_level(3), 80);
        assert_eq!(CURVE.total_xp_for_level(7), 480);
        assert_eq!(CURVE.total_xp_for_level(11), 1_200);
        assert_eq!(CURVE.total_xp_for_level(15), 2_240);
        assert_eq!(CURVE.level_for_xp(5_700), 23);
    }

    #[test]
    fn level_for_xp_is_inverse_of_total() {
        for level in 1..=CURVE.max_level {
            let at = CURVE.total_xp_for_level(level);
            assert_eq!(CURVE.level_for_xp(at), level);
            if at > 0 {
                assert_eq!(CURVE.level_for_xp(at - 1), level - 1);
            }
        }
        assert_eq!(CURVE.level_for_xp(u64::MAX / 4), CURVE.max_level);
    }

    #[test]
    fn levels_above_cap_are_clamped() {
        assert_eq!(CURVE.total_xp_for_level(99), CURVE.total_xp_for_level(30));
    }
}
