//! Turning upgrade and skill effects into concrete numbers for the shop and
//! the skill tree ("Single pick: 0.21 s -> 0.19 s").
//!
//! Values are read from the game itself (and from a preview of the game with
//! one more level), so what the screen promises is exactly what buying does.

use bloom_core::data::{Effect, HarvestMode};
use bloom_core::{Game, Modifiers};

/// `(label, current value)` for what an effect changes.
pub fn stat(game: &Game, effect: &Effect) -> (String, String) {
    let data = game.data();
    let m = game.modifiers();
    let pct = |x: f32| format!("{:.0} %", x * 100.0);
    match effect {
        Effect::PickCooldown(_) => (
            "Single pick every".into(),
            format!("{:.2} s", game.harvest_cooldown_secs()),
        ),
        Effect::GatherRadius(_) => {
            let r = |mode| game.mode_params(mode).map(|p| format!("{:.1} m", p.radius));
            let value = match (r(HarvestMode::Radius), r(HarvestMode::Sweep)) {
                (Some(a), Some(b)) => format!("{a} / rake {b}"),
                (Some(a), None) => a,
                _ => "needs the basket".into(),
            };
            ("Basket circle".into(), value)
        }
        Effect::GatherCapacity(_) => {
            let parts: Vec<String> = [
                (HarvestMode::Radius, "basket"),
                (HarvestMode::Sweep, "rake"),
                (HarvestMode::Area, "cart"),
            ]
            .iter()
            .filter_map(|&(mode, name)| {
                game.mode_params(mode)
                    .map(|p| format!("{name} {}", p.max_flowers))
            })
            .collect();
            let value = if parts.is_empty() {
                "needs the basket".into()
            } else {
                parts.join(", ")
            };
            ("Flowers per gather".into(), value)
        }
        Effect::AreaSize(_) => (
            "Cart area up to".into(),
            game.mode_params(HarvestMode::Area)
                .map_or("needs the cart".into(), |p| {
                    format!("{:.1} x {:.1} m", p.size.x, p.size.y)
                }),
        ),
        Effect::Reach(_) => ("Reach".into(), format!("{:.1} m", game.reach())),
        Effect::MoveSpeed(_) => (
            "Walking speed".into(),
            format!(
                "{:.1} m/s",
                data.player.walk_speed * game.move_speed_multiplier()
            ),
        ),
        Effect::DoubleHarvest(_) => ("Chance of a second flower".into(), pct(m.double_harvest)),
        Effect::DiscoveryXp(_) => (
            "XP per new species".into(),
            format!(
                "{:.0}",
                data.progression.xp_per_discovery as f32 * (1.0 + m.discovery_xp)
            ),
        ),
        Effect::CraftXp(_) => ("Bonus crafting XP".into(), pct(m.craft_xp)),
        Effect::CraftTime(_) => (
            "Binding time".into(),
            format!(
                "{:.0} % of normal",
                Modifiers::time_factor(m.craft_time) * 100.0
            ),
        ),
        Effect::BatchCapacity(_) => (
            "Bouquets per batch".into(),
            game.batch_capacity().to_string(),
        ),
        Effect::BouquetPrice(_) => (
            format!("{} sells for", data.recipes[0].name),
            format!("{} coins", game.bouquet_price(0)),
        ),
        Effect::BouquetPriceWithTool { tool, .. } => {
            match data
                .recipes
                .iter()
                .position(|r| r.required_tools.contains(tool))
            {
                Some(r) => (
                    format!("{} sells for", data.recipes[r].name),
                    format!("{} coins", game.bouquet_price(r)),
                ),
                None => ("Wrapped bouquets".into(), "-".into()),
            }
        }
        Effect::FlowerSaver(_) => ("Chance to save a flower".into(), pct(m.flower_saver)),
        Effect::ToolDiscount(_) => (
            "Tool prices".into(),
            format!(
                "{:.0} % of list",
                Modifiers::price_factor(m.tool_discount) * 100.0
            ),
        ),
        Effect::UpgradeDiscount(_) => (
            "Upgrade prices".into(),
            format!(
                "{:.0} % of list",
                Modifiers::price_factor(m.upgrade_discount) * 100.0
            ),
        ),
        Effect::RegionThreshold(_) => {
            match (0..data.regions.len()).find(|&r| !game.is_region_unlocked(r)) {
                Some(r) => (
                    format!("{} opens after", data.regions[r].name),
                    format!("{} flowers", game.region_harvest_threshold(r)),
                ),
                None => (
                    "Regions open after".into(),
                    format!(
                        "{:.0} % of the flowers",
                        Modifiers::price_factor(m.region_threshold) * 100.0
                    ),
                ),
            }
        }
        Effect::RegionUnlockBonus { .. } => (
            "When a region opens".into(),
            if m.region_unlock_coins > 0 || m.region_unlock_xp > 0 {
                format!(
                    "+{} coins, +{} XP",
                    m.region_unlock_coins, m.region_unlock_xp
                )
            } else {
                "nothing extra".into()
            },
        ),
        Effect::RevealUndiscovered => (
            "New species".into(),
            if m.reveal_undiscovered {
                "sparkle"
            } else {
                "hidden"
            }
            .into(),
        ),
        Effect::RareFlowerCounter => (
            "Rare flowers left".into(),
            if m.rare_flower_counter {
                "on the region card"
            } else {
                "unknown"
            }
            .into(),
        ),
        Effect::CollectionSkillPoints => (
            "Skill points from collections".into(),
            if m.collection_skill_points {
                format!("+{}", game.fully_discovered_regions())
            } else {
                "none".into()
            },
        ),
    }
}

/// One line per effect: "Label: now -> next" (or just "Label: now" when there
/// is no next level).
pub fn change_lines(now: &Game, next: Option<&Game>, effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .map(|e| {
            let (label, before) = stat(now, e);
            match next {
                Some(next) => {
                    let (_, after) = stat(next, e);
                    format!("{label}: {before} -> {after}")
                }
                None => format!("{label}: {before}"),
            }
        })
        .collect()
}
