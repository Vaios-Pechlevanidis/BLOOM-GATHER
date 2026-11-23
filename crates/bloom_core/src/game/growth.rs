//! Upgrades (coins), the skill tree (skill points), the modifier pipeline
//! that turns both into numbers the rest of the game uses, and the
//! progression guard that keeps optional spending from ever blocking the
//! tools needed to finish.
//!
//! * Effects add up per kind across every source; reductions are floored
//!   (times at 25 %, prices and thresholds at 50 % of base).
//! * Upgrade prices grow geometrically per level; discounts apply on top.
//! * Skill points are *derived* (levels, region rewards, collection
//!   milestones, the Collection Bonuses skill) minus what learned skills
//!   cost, so they can never drift out of sync with the save.

use super::{Game, GameError, GameEvent};
use crate::data::{Effect, MIN_PRICE_FACTOR, MIN_TIME_FACTOR};
use crate::economy::Coins;
use crate::progression::Xp;
use crate::rng::SplitMix64;

/// The combined effect of every upgrade level and skill level owned.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Modifiers {
    pub pick_cooldown: f32,
    pub gather_radius: f32,
    pub gather_capacity: f32,
    pub area_size: f32,
    pub reach: f32,
    pub move_speed: f32,
    pub double_harvest: f32,
    pub discovery_xp: f32,
    pub craft_xp: f32,
    pub craft_time: f32,
    pub batch_capacity: u32,
    pub carry_capacity: u32,
    pub bouquet_price: f32,
    /// `(tool id, bonus)`: bouquets whose recipe needs that tool.
    pub bouquet_price_with_tool: Vec<(String, f32)>,
    pub flower_saver: f32,
    pub tool_discount: f32,
    pub upgrade_discount: f32,
    pub region_threshold: f32,
    pub region_unlock_coins: Coins,
    pub region_unlock_xp: Xp,
    pub reveal_undiscovered: bool,
    pub rare_flower_counter: bool,
    pub collection_skill_points: bool,
}

impl Modifiers {
    fn add(&mut self, effect: &Effect, levels: u32) {
        let n = levels as f32;
        match effect {
            Effect::PickCooldown(x) => self.pick_cooldown += x * n,
            Effect::GatherRadius(x) => self.gather_radius += x * n,
            Effect::GatherCapacity(x) => self.gather_capacity += x * n,
            Effect::AreaSize(x) => self.area_size += x * n,
            Effect::Reach(x) => self.reach += x * n,
            Effect::MoveSpeed(x) => self.move_speed += x * n,
            Effect::DoubleHarvest(x) => self.double_harvest += x * n,
            Effect::DiscoveryXp(x) => self.discovery_xp += x * n,
            Effect::CraftXp(x) => self.craft_xp += x * n,
            Effect::CraftTime(x) => self.craft_time += x * n,
            Effect::BatchCapacity(c) => self.batch_capacity += c * levels,
            Effect::CarryCapacity(c) => self.carry_capacity += c * levels,
            Effect::BouquetPrice(x) => self.bouquet_price += x * n,
            Effect::BouquetPriceWithTool { tool, bonus } => {
                match self
                    .bouquet_price_with_tool
                    .iter_mut()
                    .find(|(t, _)| t == tool)
                {
                    Some((_, b)) => *b += bonus * n,
                    None => self.bouquet_price_with_tool.push((tool.clone(), bonus * n)),
                }
            }
            Effect::FlowerSaver(x) => self.flower_saver += x * n,
            Effect::ToolDiscount(x) => self.tool_discount += x * n,
            Effect::UpgradeDiscount(x) => self.upgrade_discount += x * n,
            Effect::RegionThreshold(x) => self.region_threshold += x * n,
            Effect::RegionUnlockBonus { coins, xp } => {
                self.region_unlock_coins += coins * levels as Coins;
                self.region_unlock_xp += xp * levels as Xp;
            }
            Effect::RevealUndiscovered => self.reveal_undiscovered |= levels > 0,
            Effect::RareFlowerCounter => self.rare_flower_counter |= levels > 0,
            Effect::CollectionSkillPoints => self.collection_skill_points |= levels > 0,
        }
    }

    /// Multiplier for a time after its reduction (floored).
    pub fn time_factor(reduction: f32) -> f32 {
        (1.0 - reduction).max(MIN_TIME_FACTOR)
    }

    /// Multiplier for a price or threshold after its reduction (floored).
    pub fn price_factor(reduction: f32) -> f32 {
        (1.0 - reduction).max(MIN_PRICE_FACTOR)
    }
}

/// Whether an upgrade can be bought now, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeStatus {
    Maxed,
    Buyable { price: Coins },
    Locked(UpgradeLock),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeLock {
    NeedsTool(String),
    Coins {
        price: Coins,
    },
    /// Buying it could leave too little for the tools still needed.
    SavingForTools {
        price: Coins,
        can_spend: Coins,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillStatus {
    Maxed,
    Learnable,
    Locked(SkillLock),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillLock {
    NeedsSkill(String),
    NeedsTool(String),
    Points { needed: u32, have: u32 },
}

/// Seed of the gameplay RNG in a new game (chance-based skills).
pub(super) const RNG_SEED: u64 = 0xB100_6A7E_5EED_0001;

impl Game {
    // ---------------------------------------------------------------------
    // Modifiers
    // ---------------------------------------------------------------------

    pub fn modifiers(&self) -> Modifiers {
        let mut m = Modifiers::default();
        for (id, &level) in &self.state.upgrades {
            if let Some(def) = self.data.upgrade(id) {
                for e in &def.effects {
                    m.add(e, level);
                }
            }
        }
        for (id, &level) in &self.state.skills {
            if let Some(def) = self.data.skill(id) {
                for e in &def.effects {
                    m.add(e, level);
                }
            }
        }
        m
    }

    /// How far flowers can be picked from (metres).
    pub fn reach(&self) -> f32 {
        self.data.player.interact_range + self.modifiers().reach
    }

    /// Multiplier on walking and sprinting speed.
    pub fn move_speed_multiplier(&self) -> f32 {
        1.0 + self.modifiers().move_speed
    }

    /// A tool's price after discounts.
    pub fn tool_price(&self, tool: usize) -> Coins {
        let factor = Modifiers::price_factor(self.modifiers().tool_discount);
        (self.data.tools[tool].price as f64 * factor as f64).round() as Coins
    }

    /// What one bouquet of a recipe sells for, with every bonus.
    pub fn bouquet_price(&self, recipe: usize) -> Coins {
        let def = &self.data.recipes[recipe];
        let m = self.modifiers();
        let mut bonus = m.bouquet_price;
        for (tool, b) in &m.bouquet_price_with_tool {
            if def.required_tools.contains(tool) {
                bonus += b;
            }
        }
        (def.sell_price as f64 * (1.0 + bonus as f64)).round() as Coins
    }

    /// Roll the saved gameplay RNG: `true` with probability `chance`.
    pub(super) fn roll(&mut self, chance: f32) -> bool {
        if chance <= 0.0 {
            return false;
        }
        let mut rng = SplitMix64::new(self.state.rng);
        let hit = rng.next_f32() < chance;
        self.state.rng = rng.next_u64();
        hit
    }

    // ---------------------------------------------------------------------
    // Upgrades
    // ---------------------------------------------------------------------

    pub fn upgrade_level(&self, upgrade: usize) -> u32 {
        self.state
            .upgrades
            .get(&self.data.upgrades[upgrade].id)
            .copied()
            .unwrap_or(0)
    }

    /// Price of the next level, or `None` when maxed.
    pub fn upgrade_price(&self, upgrade: usize) -> Option<Coins> {
        let def = &self.data.upgrades[upgrade];
        let level = self.upgrade_level(upgrade);
        if level >= def.max_level {
            return None;
        }
        let base = self.data.economy.leveled_cost(def.base_price, level);
        let factor = Modifiers::price_factor(self.modifiers().upgrade_discount);
        Some(((base as f64 * factor as f64).round() as Coins).max(1))
    }

    /// Coins the player can always still earn: every flower left in the
    /// world and the basket sold raw, bouquets owned or being bound, and
    /// unclaimed region rewards.
    pub fn guaranteed_income(&self) -> Coins {
        let data = &self.data;
        let in_world = data
            .world_flower_total()
            .saturating_sub(self.state.stats.flowers_harvested);
        let in_basket: u64 = self.state.inventory.values().map(|&n| n as u64).sum();
        let mut income = data.economy.raw_flower_value(in_world + in_basket);
        for (r, def) in data.recipes.iter().enumerate() {
            let mut n = self.bouquet_count(r) as u64;
            if let Some(job) = self.craft_job().filter(|j| j.recipe == def.id) {
                n += job.count as u64;
            }
            income += self.bouquet_price(r) * n;
        }
        for region in &data.regions {
            if !self.state.completed_regions.contains(&region.id) {
                income += region.completion_reward.coins;
            }
        }
        income
    }

    /// What the required tools not yet owned cost now.
    pub fn required_tools_left_cost(&self) -> Coins {
        (0..self.data.tools.len())
            .filter(|&t| self.data.tools[t].required_for_completion && !self.owns_tool(t))
            .map(|t| self.tool_price(t))
            .sum()
    }

    /// The most that can go on optional purchases right now without ever
    /// making a required tool unaffordable (economy doc §5.2).
    pub fn optional_spending_limit(&self) -> Coins {
        let budget = self.state.coins + self.guaranteed_income();
        budget
            .saturating_sub(self.required_tools_left_cost())
            .min(self.state.coins)
    }

    pub fn upgrade_status(&self, upgrade: usize) -> UpgradeStatus {
        let def = &self.data.upgrades[upgrade];
        let Some(price) = self.upgrade_price(upgrade) else {
            return UpgradeStatus::Maxed;
        };
        if let Some(tool) = def
            .requires_tool
            .as_ref()
            .filter(|t| !self.state.tools.contains(*t))
        {
            return UpgradeStatus::Locked(UpgradeLock::NeedsTool(tool.clone()));
        }
        if self.state.coins < price {
            return UpgradeStatus::Locked(UpgradeLock::Coins { price });
        }
        let can_spend = self.optional_spending_limit();
        if price > can_spend {
            return UpgradeStatus::Locked(UpgradeLock::SavingForTools { price, can_spend });
        }
        UpgradeStatus::Buyable { price }
    }

    /// Buy the next level of an upgrade. Returns the new level.
    pub fn buy_upgrade(&mut self, upgrade: usize) -> Result<u32, GameError> {
        if upgrade >= self.data.upgrades.len() {
            return Err(GameError::UnknownUpgrade);
        }
        let price = match self.upgrade_status(upgrade) {
            UpgradeStatus::Maxed => return Err(GameError::MaxLevel),
            UpgradeStatus::Locked(UpgradeLock::NeedsTool(tool)) => {
                return Err(GameError::NeedsTool { tool })
            }
            UpgradeStatus::Locked(UpgradeLock::Coins { price }) => {
                return Err(GameError::NotEnoughCoins {
                    price,
                    have: self.state.coins,
                })
            }
            UpgradeStatus::Locked(UpgradeLock::SavingForTools { can_spend, .. }) => {
                return Err(GameError::SavingForTools { can_spend })
            }
            UpgradeStatus::Buyable { price } => price,
        };
        self.state.coins -= price;
        self.state.stats.coins_spent += price;
        let id = self.data.upgrades[upgrade].id.clone();
        let level = {
            let l = self.state.upgrades.entry(id).or_default();
            *l += 1;
            *l
        };
        self.events
            .push(GameEvent::UpgradeBought { upgrade, level });
        self.dirty = true;
        Ok(level)
    }

    // ---------------------------------------------------------------------
    // Skills
    // ---------------------------------------------------------------------

    pub fn skill_level(&self, skill: usize) -> u32 {
        self.state
            .skills
            .get(&self.data.skills[skill].id)
            .copied()
            .unwrap_or(0)
    }

    /// Regions whose every species has been discovered.
    pub fn fully_discovered_regions(&self) -> u32 {
        self.data
            .regions
            .iter()
            .filter(|r| {
                r.flowers
                    .iter()
                    .all(|f| self.state.discovered.contains(&f.species))
            })
            .count() as u32
    }

    /// Every skill point earned so far.
    pub fn skill_points_earned(&self) -> u32 {
        let p = &self.data.progression;
        let mut points = p.skill_points_per_level * (self.level() - 1);
        for region in &self.data.regions {
            if self.state.completed_regions.contains(&region.id) {
                points += region.completion_reward.skill_points;
            }
        }
        let found = self.state.discovered.len() as u32;
        points += p
            .collection_milestones
            .iter()
            .filter(|&&m| found >= m)
            .count() as u32;
        if self.modifiers().collection_skill_points {
            points += self.fully_discovered_regions();
        }
        points + self.achievement_skill_points()
    }

    pub fn skill_points_spent(&self) -> u32 {
        self.state
            .skills
            .iter()
            .filter_map(|(id, &level)| self.data.skill(id).map(|s| s.cost * level))
            .sum()
    }

    /// Skill points available to spend.
    pub fn skill_points(&self) -> u32 {
        self.skill_points_earned()
            .saturating_sub(self.skill_points_spent())
    }

    pub fn skill_status(&self, skill: usize) -> SkillStatus {
        let def = &self.data.skills[skill];
        if self.skill_level(skill) >= def.max_level {
            return SkillStatus::Maxed;
        }
        if let Some(req) = &def.requires_skill {
            if self.state.skills.get(req).copied().unwrap_or(0) == 0 {
                return SkillStatus::Locked(SkillLock::NeedsSkill(req.clone()));
            }
        }
        if let Some(tool) = def
            .requires_tool
            .as_ref()
            .filter(|t| !self.state.tools.contains(*t))
        {
            return SkillStatus::Locked(SkillLock::NeedsTool(tool.clone()));
        }
        let have = self.skill_points();
        if have < def.cost {
            return SkillStatus::Locked(SkillLock::Points {
                needed: def.cost,
                have,
            });
        }
        SkillStatus::Learnable
    }

    /// Learn the next level of a skill (permanent). Returns the new level.
    pub fn learn_skill(&mut self, skill: usize) -> Result<u32, GameError> {
        if skill >= self.data.skills.len() {
            return Err(GameError::UnknownSkill);
        }
        match self.skill_status(skill) {
            SkillStatus::Maxed => return Err(GameError::MaxLevel),
            SkillStatus::Locked(SkillLock::NeedsSkill(skill)) => {
                return Err(GameError::NeedsSkill { skill })
            }
            SkillStatus::Locked(SkillLock::NeedsTool(tool)) => {
                return Err(GameError::NeedsTool { tool })
            }
            SkillStatus::Locked(SkillLock::Points { needed, have }) => {
                return Err(GameError::NotEnoughSkillPoints { needed, have })
            }
            SkillStatus::Learnable => {}
        }
        let id = self.data.skills[skill].id.clone();
        let level = {
            let l = self.state.skills.entry(id).or_default();
            *l += 1;
            *l
        };
        self.events.push(GameEvent::SkillLearned { skill, level });
        // A lower region threshold may open a region at once.
        let level_before = self.level();
        self.award_xp(level_before, 0);
        Ok(level)
    }

    /// A copy of the game with one more level of an upgrade, to show what
    /// buying it would change. Nothing is charged or announced.
    pub fn preview_upgrade(&self, upgrade: usize) -> Game {
        let mut g = self.clone();
        *g.state
            .upgrades
            .entry(self.data.upgrades[upgrade].id.clone())
            .or_default() += 1;
        g.events.clear();
        g
    }

    /// A copy of the game with one more level of a skill (see
    /// [`Game::preview_upgrade`]).
    pub fn preview_skill(&self, skill: usize) -> Game {
        let mut g = self.clone();
        *g.state
            .skills
            .entry(self.data.skills[skill].id.clone())
            .or_default() += 1;
        g.events.clear();
        g
    }

    // ---------------------------------------------------------------------
    // Region unlocks
    // ---------------------------------------------------------------------

    /// Record regions that have just become unlocked. When `announce` is
    /// set, each one raises an event and pays the unlock bonus (if any).
    pub(super) fn refresh_regions(&mut self, announce: bool) {
        loop {
            let Some(region) = (0..self.data.regions.len()).find(|&r| {
                !self
                    .state
                    .unlocked_regions
                    .contains(&self.data.regions[r].id)
                    && self.meets_region_unlock(r)
            }) else {
                return;
            };
            self.state
                .unlocked_regions
                .insert(self.data.regions[region].id.clone());
            self.dirty = true;
            if !announce {
                continue;
            }
            let m = self.modifiers();
            let (coins, xp) = (m.region_unlock_coins, m.region_unlock_xp);
            self.earn(coins);
            self.state.xp += xp;
            self.events
                .push(GameEvent::RegionUnlocked { region, coins, xp });
        }
    }

    /// The level and harvest thresholds (after skills) are met.
    pub(super) fn meets_region_unlock(&self, region: usize) -> bool {
        let unlock = self.data.regions[region].unlock;
        let factor = Modifiers::price_factor(self.modifiers().region_threshold);
        let needed = (unlock.flowers_harvested as f64 * factor as f64).ceil() as u64;
        self.level() >= unlock.level && self.state.stats.flowers_harvested >= needed
    }

    /// Harvested flowers needed to open a region, after skills.
    pub fn region_harvest_threshold(&self, region: usize) -> u32 {
        let unlock = self.data.regions[region].unlock;
        let factor = Modifiers::price_factor(self.modifiers().region_threshold);
        (unlock.flowers_harvested as f64 * factor as f64).ceil() as u32
    }
}
