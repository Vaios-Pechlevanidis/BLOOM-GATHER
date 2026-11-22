//! Content for long-term growth: coin-priced **upgrades** (levelled, bought
//! at the workshop's tool shelf) and the **skill tree** (skill points).
//!
//! Both are lists of per-level [`Effect`]s. There is deliberately no effect
//! that changes the price of a flower, grants a tool, or unlocks a harvesting
//! mode or recipe: those stay tied to tools (see docs/game-design.md §7–8).

use serde::{Deserialize, Serialize};

use crate::economy::Coins;
use crate::progression::Xp;

/// Reductions of times (cooldowns, binding) never go below this factor.
pub const MIN_TIME_FACTOR: f32 = 0.25;
/// Price discounts never go below this factor of the base price.
pub const MIN_PRICE_FACTOR: f32 = 0.5;

/// What one level of an upgrade or skill does. Fractions are per level
/// (0.06 = 6 %) and add up across every source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// Shorter pause between single picks.
    PickCooldown(f32),
    /// Larger basket and rake circle.
    GatherRadius(f32),
    /// More flowers per basket, rake or cart action.
    GatherCapacity(f32),
    /// Larger cart rectangle.
    AreaSize(f32),
    /// Extra reach for picking and aiming, in metres.
    Reach(f32),
    /// Faster walking and sprinting.
    MoveSpeed(f32),
    /// Chance that a single pick also picks the nearest unpicked neighbour.
    DoubleHarvest(f32),
    /// More XP for discovering a species.
    DiscoveryXp(f32),
    /// More XP for binding bouquets.
    CraftXp(f32),
    /// Shorter binding time.
    CraftTime(f32),
    /// Bouquets the workbench binds at once, added on top of tools.
    BatchCapacity(u32),
    /// Every bouquet sells for more.
    BouquetPrice(f32),
    /// Bouquets whose recipe needs `tool` sell for more.
    BouquetPriceWithTool { tool: String, bonus: f32 },
    /// Chance per bound bouquet that one of its flowers is saved.
    FlowerSaver(f32),
    /// Tools cost less.
    ToolDiscount(f32),
    /// Upgrades cost less.
    UpgradeDiscount(f32),
    /// Regions need fewer harvested flowers to open.
    RegionThreshold(f32),
    /// Coins and XP whenever a region opens.
    RegionUnlockBonus { coins: Coins, xp: Xp },
    /// Flowers of species you haven't discovered yet sparkle.
    RevealUndiscovered,
    /// The region card counts the rare and very rare flowers still growing.
    RareFlowerCounter,
    /// +1 skill point for each region whose species are all discovered.
    CollectionSkillPoints,
}

impl Effect {
    /// The fraction this effect *reduces* something by, if it is a reduction
    /// (and so needs a floor), with that floor.
    pub fn reduction(&self) -> Option<(f32, f32)> {
        match *self {
            Effect::PickCooldown(x) | Effect::CraftTime(x) => Some((x, MIN_TIME_FACTOR)),
            Effect::ToolDiscount(x) | Effect::UpgradeDiscount(x) => Some((x, MIN_PRICE_FACTOR)),
            Effect::RegionThreshold(x) => Some((x, MIN_PRICE_FACTOR)),
            _ => None,
        }
    }

    /// Groups effects that stack against the same floor.
    pub fn kind(&self) -> &'static str {
        match self {
            Effect::PickCooldown(_) => "pick cooldown",
            Effect::GatherRadius(_) => "gather radius",
            Effect::GatherCapacity(_) => "gather capacity",
            Effect::AreaSize(_) => "area size",
            Effect::Reach(_) => "reach",
            Effect::MoveSpeed(_) => "move speed",
            Effect::DoubleHarvest(_) => "double harvest",
            Effect::DiscoveryXp(_) => "discovery xp",
            Effect::CraftXp(_) => "craft xp",
            Effect::CraftTime(_) => "craft time",
            Effect::BatchCapacity(_) => "batch capacity",
            Effect::BouquetPrice(_) => "bouquet price",
            Effect::BouquetPriceWithTool { .. } => "bouquet price with tool",
            Effect::FlowerSaver(_) => "flower saver",
            Effect::ToolDiscount(_) => "tool discount",
            Effect::UpgradeDiscount(_) => "upgrade discount",
            Effect::RegionThreshold(_) => "region threshold",
            Effect::RegionUnlockBonus { .. } => "region unlock bonus",
            Effect::RevealUndiscovered => "reveal undiscovered",
            Effect::RareFlowerCounter => "rare flower counter",
            Effect::CollectionSkillPoints => "collection skill points",
        }
    }

    /// The per-level fraction (or metres) carried by numeric effects.
    pub fn amount(&self) -> Option<f32> {
        match *self {
            Effect::PickCooldown(x)
            | Effect::GatherRadius(x)
            | Effect::GatherCapacity(x)
            | Effect::AreaSize(x)
            | Effect::Reach(x)
            | Effect::MoveSpeed(x)
            | Effect::DoubleHarvest(x)
            | Effect::DiscoveryXp(x)
            | Effect::CraftXp(x)
            | Effect::CraftTime(x)
            | Effect::BouquetPrice(x)
            | Effect::FlowerSaver(x)
            | Effect::ToolDiscount(x)
            | Effect::UpgradeDiscount(x)
            | Effect::RegionThreshold(x) => Some(x),
            Effect::BouquetPriceWithTool { bonus, .. } => Some(bonus),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradeCategory {
    Harvesting,
    Crafting,
    Economy,
    Exploration,
}

impl UpgradeCategory {
    pub fn label(self) -> &'static str {
        match self {
            UpgradeCategory::Harvesting => "Harvesting",
            UpgradeCategory::Crafting => "Crafting",
            UpgradeCategory::Economy => "Economy",
            UpgradeCategory::Exploration => "Exploration",
        }
    }
}

/// A levelled, coin-priced upgrade. Level `n` costs
/// `base_price × upgrade_cost_growth^n` (before discounts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpgradeDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: UpgradeCategory,
    pub base_price: Coins,
    pub max_level: u32,
    /// Only sold once this tool is owned (it would do nothing without it).
    #[serde(default)]
    pub requires_tool: Option<String>,
    /// Applied once per level.
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillBranch {
    Harvesting,
    Botany,
    Floristry,
    Efficiency,
    Exploration,
}

impl SkillBranch {
    pub const ALL: [SkillBranch; 5] = [
        SkillBranch::Harvesting,
        SkillBranch::Botany,
        SkillBranch::Floristry,
        SkillBranch::Efficiency,
        SkillBranch::Exploration,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SkillBranch::Harvesting => "Harvesting",
            SkillBranch::Botany => "Botany",
            SkillBranch::Floristry => "Floristry",
            SkillBranch::Efficiency => "Efficiency",
            SkillBranch::Exploration => "Exploration",
        }
    }
}

/// A skill on the tree, learned with skill points (permanent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub branch: SkillBranch,
    pub max_level: u32,
    /// Skill points per level.
    pub cost: u32,
    /// Must have at least one level of this skill first (same branch,
    /// listed earlier).
    #[serde(default)]
    pub requires_skill: Option<String>,
    /// Must own this tool first (the skill builds on what the tool does).
    #[serde(default)]
    pub requires_tool: Option<String>,
    /// Applied once per level.
    pub effects: Vec<Effect>,
}
