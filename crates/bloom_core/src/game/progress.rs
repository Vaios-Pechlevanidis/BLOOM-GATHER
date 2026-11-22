//! Progress over the whole journey: achievements (optional goals with small
//! rewards), the completion check behind the ending, and statistics.
//!
//! Achievements are re-checked after every command that can move a
//! condition; each unlocks once, pays its reward and is announced once.
//! Completion never depends on achievements.

use serde::{Deserialize, Serialize};

use super::{Game, GameEvent};
use crate::data::{Condition, Rarity};
use crate::economy::Coins;

/// The journey as it stood the moment the game was completed. Saved, so
/// the summary stays the same however long the player roams afterwards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct JourneySummary {
    pub playtime_secs: f64,
    pub distance_walked_m: f64,
    pub flowers_harvested: u64,
    pub rare_flowers: u64,
    pub flowers_sold: u64,
    pub bouquets_crafted: u64,
    pub bouquets_sold: u64,
    pub coins_earned: Coins,
    pub coins_spent: Coins,
    pub level: u32,
    pub tools: u32,
    pub regions: u32,
    pub species: u32,
    pub recipes: u32,
    pub achievements: u32,
    pub upgrade_levels: u32,
    pub skill_levels: u32,
}

/// Where the player stands on each completion requirement (`done`, `total`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Completion {
    pub flowers: (u64, u64),
    pub regions: (u32, u32),
    pub tools: (u32, u32),
    pub species: (u32, u32),
    pub recipes_discovered: (u32, u32),
    pub required_bouquets: (u32, u32),
}

impl Completion {
    /// How many of the six requirements are met.
    pub fn met(&self) -> u32 {
        [
            self.flowers.0 >= self.flowers.1,
            self.regions.0 >= self.regions.1,
            self.tools.0 >= self.tools.1,
            self.species.0 >= self.species.1,
            self.recipes_discovered.0 >= self.recipes_discovered.1,
            self.required_bouquets.0 >= self.required_bouquets.1,
        ]
        .iter()
        .filter(|&&ok| ok)
        .count() as u32
    }

    pub const REQUIREMENTS: u32 = 6;

    pub fn is_complete(&self) -> bool {
        self.met() == Self::REQUIREMENTS
    }
}

impl Game {
    /// The ending's requirements: every flower harvested, every region
    /// complete, every required tool owned, every species discovered, every
    /// required recipe discovered and bound at least once.
    pub fn completion(&self) -> Completion {
        let d = &self.data;
        let s = &self.state;
        let required_tools: Vec<_> = d
            .tools
            .iter()
            .filter(|t| t.required_for_completion)
            .collect();
        let required_recipes: Vec<_> = d
            .recipes
            .iter()
            .filter(|r| r.required_for_completion)
            .collect();
        Completion {
            flowers: (s.stats.flowers_harvested, d.world_flower_total()),
            regions: (s.completed_regions.len() as u32, d.regions.len() as u32),
            tools: (
                required_tools
                    .iter()
                    .filter(|t| s.tools.contains(&t.id))
                    .count() as u32,
                required_tools.len() as u32,
            ),
            species: (s.discovered.len() as u32, d.species.len() as u32),
            recipes_discovered: (
                required_recipes
                    .iter()
                    .filter(|r| s.discovered_recipes.contains(&r.id))
                    .count() as u32,
                required_recipes.len() as u32,
            ),
            required_bouquets: (
                required_recipes
                    .iter()
                    .filter(|r| s.crafted.get(&r.id).copied().unwrap_or(0) > 0)
                    .count() as u32,
                required_recipes.len() as u32,
            ),
        }
    }

    /// Rare and very rare flowers picked so far.
    pub fn rare_flowers_harvested(&self) -> u64 {
        self.state
            .harvested_per_species
            .iter()
            .filter(|(id, _)| {
                self.data
                    .species(id)
                    .is_some_and(|s| s.rarity >= Rarity::Rare)
            })
            .map(|(_, &n)| n as u64)
            .sum()
    }

    pub fn is_achievement_unlocked(&self, achievement: usize) -> bool {
        self.state
            .achievements
            .contains(&self.data.achievements[achievement].id)
    }

    /// `(current, target)` towards an achievement (current is capped).
    pub fn achievement_progress(&self, achievement: usize) -> (u64, u64) {
        let d = &self.data;
        let s = &self.state;
        let (now, target) = match d.achievements[achievement].condition {
            Condition::FlowersHarvested(n) => (s.stats.flowers_harvested, n),
            Condition::AllFlowersHarvested => (s.stats.flowers_harvested, d.world_flower_total()),
            Condition::BouquetsCrafted(n) => (s.stats.bouquets_crafted, n),
            Condition::SpeciesDiscovered(n) => (s.discovered.len() as u64, n as u64),
            Condition::AllSpeciesDiscovered => (s.discovered.len() as u64, d.species.len() as u64),
            Condition::RareFlowerHarvested => (self.rare_flowers_harvested().min(1), 1),
            Condition::RegionsCompleted(n) => (s.completed_regions.len() as u64, n as u64),
            Condition::AllRegionsCompleted => {
                (s.completed_regions.len() as u64, d.regions.len() as u64)
            }
            Condition::AllRegionsUnlocked => (
                (0..d.regions.len())
                    .filter(|&r| self.is_region_unlocked(r))
                    .count() as u64,
                d.regions.len() as u64,
            ),
            Condition::AllToolsOwned => (s.tools.len() as u64, d.tools.len() as u64),
            Condition::AllRecipesDiscovered => {
                (s.discovered_recipes.len() as u64, d.recipes.len() as u64)
            }
            Condition::AllRequiredBouquetsCrafted => {
                let (done, total) = self.completion().required_bouquets;
                (done as u64, total as u64)
            }
            Condition::GameCompleted => (
                self.completion().met() as u64,
                Completion::REQUIREMENTS as u64,
            ),
        };
        (now.min(target), target)
    }

    /// Unlock every achievement whose condition now holds, paying rewards.
    /// `announce` raises an event per unlock (off when catching up a loaded
    /// save from an older build).
    pub(super) fn refresh_achievements(&mut self, announce: bool) {
        for a in 0..self.data.achievements.len() {
            if self.is_achievement_unlocked(a) {
                continue;
            }
            let (now, target) = self.achievement_progress(a);
            if now < target {
                continue;
            }
            let def = self.data.achievements[a].clone();
            self.state.achievements.insert(def.id);
            self.earn(def.reward.coins);
            self.dirty = true;
            if announce {
                self.events
                    .push(GameEvent::AchievementUnlocked { achievement: a });
            }
        }
        self.check_completion(announce);
    }

    /// The moment every requirement is met, record the journey summary and
    /// (when `announce`) raise `GameCompleted`. Happens once per game.
    fn check_completion(&mut self, announce: bool) {
        if self.state.completed.is_some() || !self.completion().is_complete() {
            return;
        }
        let s = &self.state;
        let summary = JourneySummary {
            playtime_secs: s.stats.playtime_secs,
            distance_walked_m: s.stats.distance_walked_m,
            flowers_harvested: s.stats.flowers_harvested,
            rare_flowers: self.rare_flowers_harvested(),
            flowers_sold: s.stats.flowers_sold,
            bouquets_crafted: s.stats.bouquets_crafted,
            bouquets_sold: s.stats.bouquets_sold,
            coins_earned: s.stats.coins_earned,
            coins_spent: s.stats.coins_spent,
            level: self.level(),
            tools: s.tools.len() as u32,
            regions: s.completed_regions.len() as u32,
            species: s.discovered.len() as u32,
            recipes: s.discovered_recipes.len() as u32,
            achievements: s.achievements.len() as u32,
            upgrade_levels: s.upgrades.values().sum(),
            skill_levels: s.skills.values().sum(),
        };
        self.state.completed = Some(summary);
        self.dirty = true;
        if announce {
            self.events.push(GameEvent::GameCompleted);
        }
    }

    /// Every requirement met (and recorded).
    pub fn is_game_complete(&self) -> bool {
        self.state.completed.is_some()
    }

    pub fn journey_summary(&self) -> Option<&JourneySummary> {
        self.state.completed.as_ref()
    }

    /// The finale should play: the game is complete but the player hasn't
    /// seen its ending through yet (also after a crash mid-finale).
    pub fn finale_pending(&self) -> bool {
        self.state.completed.is_some() && !self.state.finale_seen
    }

    /// The ending has been watched; from now on the world is free roam.
    pub fn mark_finale_seen(&mut self) {
        if self.state.completed.is_some() && !self.state.finale_seen {
            self.state.finale_seen = true;
            self.dirty = true;
        }
    }

    /// Skill points from unlocked achievements.
    pub(super) fn achievement_skill_points(&self) -> u32 {
        self.state
            .achievements
            .iter()
            .filter_map(|id| self.data.achievement(id))
            .map(|a| a.reward.skill_points)
            .sum()
    }

    /// Record distance walked (statistics only; saved with the next save).
    pub fn add_distance(&mut self, metres: f32) {
        if metres.is_finite() && metres > 0.0 {
            self.state.stats.distance_walked_m += metres as f64;
        }
    }
}
