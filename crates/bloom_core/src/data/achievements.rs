//! Achievements: optional goals with small rewards. None of them is ever
//! required for completing the game; they mark the journey.

use serde::{Deserialize, Serialize};

use crate::economy::Coins;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AchievementCategory {
    Harvesting,
    Crafting,
    Exploration,
    Collection,
    Completion,
}

impl AchievementCategory {
    pub const ALL: [AchievementCategory; 5] = [
        AchievementCategory::Harvesting,
        AchievementCategory::Crafting,
        AchievementCategory::Exploration,
        AchievementCategory::Collection,
        AchievementCategory::Completion,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AchievementCategory::Harvesting => "Harvesting",
            AchievementCategory::Crafting => "Crafting",
            AchievementCategory::Exploration => "Exploration",
            AchievementCategory::Collection => "Collection",
            AchievementCategory::Completion => "Completion",
        }
    }
}

/// What has to be true for an achievement to unlock. Counts are lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    FlowersHarvested(u64),
    /// Every flower in the world.
    AllFlowersHarvested,
    BouquetsCrafted(u64),
    SpeciesDiscovered(u32),
    /// Every species.
    AllSpeciesDiscovered,
    /// At least one rare or very rare flower picked.
    RareFlowerHarvested,
    RegionsCompleted(u32),
    AllRegionsCompleted,
    AllRegionsUnlocked,
    AllToolsOwned,
    AllRecipesDiscovered,
    /// Every recipe required for completion bound at least once.
    AllRequiredBouquetsCrafted,
    /// Every completion condition met (see `Game::completion`).
    GameCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AchievementReward {
    #[serde(default)]
    pub coins: Coins,
    #[serde(default)]
    pub skill_points: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AchievementDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: AchievementCategory,
    pub condition: Condition,
    #[serde(default)]
    pub reward: AchievementReward,
}
