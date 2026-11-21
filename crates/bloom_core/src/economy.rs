//! Economy configuration and money math.
//!
//! All prices live in `game_data.ron`; this module only defines their shape and
//! the arithmetic on them, so no price is ever hard-coded elsewhere.

use serde::{Deserialize, Serialize};

/// Whole coins. Unsigned: a balance can never go negative by construction.
pub type Coins = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomyConfig {
    /// The selling price of *every* individual flower, regardless of species
    /// or rarity. Species definitions intentionally have no price field.
    pub flower_base_value: Coins,
    pub starting_coins: Coins,
    /// How many bouquets one workshop action may craft before any tool or
    /// upgrade raises the limit.
    pub base_craft_batch_capacity: u32,
    /// Geometric growth factor for leveled upgrade prices (Phase 5).
    pub upgrade_cost_growth: f64,
}

impl EconomyConfig {
    /// Value of `count` flowers sold individually.
    pub fn raw_flower_value(&self, count: u64) -> Coins {
        self.flower_base_value.saturating_mul(count)
    }

    /// Price of the next level of a leveled upgrade whose first level costs
    /// `base` and which currently sits at `current_level`.
    pub fn leveled_cost(&self, base: Coins, current_level: u32) -> Coins {
        let cost = base as f64 * self.upgrade_cost_growth.powi(current_level as i32);
        cost.round() as Coins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> EconomyConfig {
        EconomyConfig {
            flower_base_value: 10,
            starting_coins: 0,
            base_craft_batch_capacity: 5,
            upgrade_cost_growth: 1.6,
        }
    }

    #[test]
    fn raw_value_is_linear_in_count() {
        let c = config();
        assert_eq!(c.raw_flower_value(0), 0);
        assert_eq!(c.raw_flower_value(1), 10);
        assert_eq!(c.raw_flower_value(1_900), 19_000);
    }

    #[test]
    fn raw_value_saturates_instead_of_overflowing() {
        let c = config();
        assert_eq!(c.raw_flower_value(u64::MAX), u64::MAX);
    }

    #[test]
    fn leveled_cost_grows_geometrically() {
        let c = config();
        assert_eq!(c.leveled_cost(100, 0), 100);
        assert_eq!(c.leveled_cost(100, 1), 160);
        assert_eq!(c.leveled_cost(100, 2), 256);
        let five_levels: Coins = (0..5).map(|l| c.leveled_cost(80, l)).sum();
        assert_eq!(five_levels, 1_265);
    }
}
