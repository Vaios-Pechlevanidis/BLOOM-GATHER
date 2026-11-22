//! The authoritative game: persistent [`GameState`] plus the commands that
//! change it.
//!
//! Every gameplay mutation goes through a method here that validates first
//! and either applies the whole change or returns a [`GameError`] without
//! touching anything. Commands queue [`GameEvent`]s for presentation (sounds,
//! toasts, animations) and mark the game dirty for autosave.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::bitset::BitSet;
use crate::data::{GameData, HarvestMode, ToolEffect};
use crate::economy::Coins;
use crate::progression::Xp;
use crate::world::{FlowerField, RegionLayout};

mod crafting;
mod growth;
mod progress;
mod travel;

pub use crafting::{CraftJob, CraftPlan, RecipeLock, RecipeStatus};
pub use growth::{Modifiers, SkillLock, SkillStatus, UpgradeLock, UpgradeStatus};
pub use progress::Completion;
pub use travel::RegionRequirements;

/// Everything that is saved. Keys are content ids (not indices) so saves
/// survive content being reordered. Missing fields (saves from older
/// builds) take their defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GameState {
    pub coins: Coins,
    /// Lifetime XP; the level is derived from it.
    pub xp: Xp,
    /// Flowers in the basket, by species id.
    pub inventory: BTreeMap<String, u32>,
    /// Species harvested at least once.
    pub discovered: BTreeSet<String>,
    /// Harvested flowers per region id. Bits are never cleared.
    pub harvested: BTreeMap<String, BitSet>,
    /// Lifetime harvested count per species (to know what the world still holds).
    pub harvested_per_species: BTreeMap<String, u32>,
    pub completed_regions: BTreeSet<String>,
    /// Owned tools.
    pub tools: BTreeSet<String>,
    /// Bouquets ever bound, per recipe id.
    pub crafted: BTreeMap<String, u32>,
    /// Recipes the player has discovered.
    pub discovered_recipes: BTreeSet<String>,
    /// Finished bouquets waiting to be sold, per recipe id.
    pub bouquets: BTreeMap<String, u32>,
    /// The batch being bound at the workbench, if any.
    pub craft_job: Option<CraftJob>,
    /// Upgrade levels bought, per upgrade id.
    pub upgrades: BTreeMap<String, u32>,
    /// Skill levels learned, per skill id.
    pub skills: BTreeMap<String, u32>,
    /// Regions whose unlock conditions have been met (announced once).
    pub unlocked_regions: BTreeSet<String>,
    /// Achievements unlocked (each pays its reward once).
    pub achievements: BTreeSet<String>,
    /// Regions the player has walked into at least once.
    pub visited_regions: BTreeSet<String>,
    /// State of the gameplay RNG (chance-based skills), saved so reloading
    /// can't re-roll a result.
    pub rng: u64,
    pub stats: Stats,
    /// Where the player stood when the game was saved.
    pub player: Option<PlayerPose>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Stats {
    pub flowers_harvested: u64,
    pub flowers_sold: u64,
    pub bouquets_crafted: u64,
    pub bouquets_sold: u64,
    pub coins_earned: Coins,
    /// Coins paid for tools and upgrades.
    pub coins_spent: Coins,
    pub playtime_secs: f64,
    /// Metres walked (and run) in the world.
    pub distance_walked_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerPose {
    pub region: String,
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

/// Something that happened, for presentation.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    FlowerHarvested {
        region: usize,
        flower: u32,
        species: usize,
    },
    SpeciesDiscovered {
        species: usize,
    },
    LevelUp {
        level: u32,
    },
    RegionCompleted {
        region: usize,
        coins: Coins,
    },
    RecipeDiscovered {
        recipe: usize,
    },
    CraftStarted {
        recipe: usize,
        count: u32,
    },
    /// A batch finished binding. `first` = the recipe's first ever batch.
    BouquetsReady {
        recipe: usize,
        count: u32,
        first: bool,
    },
    BouquetsSold {
        recipe: usize,
        count: u32,
        coins: Coins,
    },
    ToolBought {
        tool: usize,
    },
    FlowersSold {
        species: usize,
        count: u32,
        coins: Coins,
    },
    UpgradeBought {
        upgrade: usize,
        level: u32,
    },
    SkillLearned {
        skill: usize,
        level: u32,
    },
    /// A region's unlock conditions are met; `coins`/`xp` = unlock bonus paid.
    RegionUnlocked {
        region: usize,
        coins: Coins,
        xp: Xp,
    },
    /// Efficient Production put flowers back in the basket.
    FlowersSaved {
        species: usize,
        count: u32,
    },
    /// Discovering species reached a milestone (+1 skill point).
    CollectionMilestone {
        species_found: u32,
    },
    AchievementUnlocked {
        achievement: usize,
    },
    /// The player stepped into a region (through a gate or from the map).
    RegionEntered {
        region: usize,
        first_visit: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameError {
    UnknownRegion,
    UnknownFlower,
    UnknownSpecies,
    RegionLocked,
    AlreadyHarvested,
    OutOfReach {
        distance: f32,
    },
    CoolingDown {
        remaining: f32,
    },
    NothingToSell,
    NotEnoughFlowers {
        have: u32,
    },
    UnknownTool,
    AlreadyOwned,
    LevelTooLow {
        needed: u32,
    },
    NeedsTool {
        tool: String,
    },
    NotEnoughCoins {
        price: Coins,
        have: Coins,
    },
    /// No owned tool provides that harvesting mode.
    ModeLocked,
    /// The basket/sweep/area would gather nothing.
    NothingInReach,
    UnknownRecipe,
    /// The recipe isn't discovered yet (missing tools, species or level).
    RecipeLocked(Vec<RecipeLock>),
    /// The workbench is still binding the previous batch.
    Busy,
    BatchTooLarge {
        max: u32,
    },
    /// Not enough suitable flowers in the basket.
    MissingFlowers {
        detail: String,
    },
    /// Doing this would use flowers needed for a required bouquet.
    KeepsakesNeeded,
    NoBouquets {
        have: u32,
    },
    /// Some of these flowers are set aside for required bouquets.
    Reserved {
        sellable: u32,
    },
    UnknownUpgrade,
    UnknownSkill,
    MaxLevel,
    NeedsSkill {
        skill: String,
    },
    NotEnoughSkillPoints {
        needed: u32,
        have: u32,
    },
    /// An optional purchase would put a required tool out of reach.
    SavingForTools {
        can_spend: Coins,
    },
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::UnknownRegion => write!(f, "unknown region"),
            GameError::UnknownFlower => write!(f, "unknown flower"),
            GameError::UnknownSpecies => write!(f, "unknown species"),
            GameError::RegionLocked => write!(f, "that region is still locked"),
            GameError::AlreadyHarvested => write!(f, "that flower has already been picked"),
            GameError::OutOfReach { distance } => write!(f, "too far away ({distance:.1} m)"),
            GameError::CoolingDown { remaining } => write!(f, "hands busy for {remaining:.2} s"),
            GameError::NothingToSell => write!(f, "nothing to sell"),
            GameError::UnknownTool => write!(f, "unknown tool"),
            GameError::AlreadyOwned => write!(f, "you already own that tool"),
            GameError::LevelTooLow { needed } => write!(f, "reach level {needed} first"),
            GameError::NeedsTool { tool } => {
                write!(f, "you need the {} first", tool.replace('_', " "))
            }
            GameError::NotEnoughCoins { price, have } => {
                write!(f, "costs {price} coins; you have {have}")
            }
            GameError::ModeLocked => write!(f, "you don't have a tool for that yet"),
            GameError::NothingInReach => write!(f, "no flowers there to gather"),
            GameError::UnknownRecipe => write!(f, "unknown recipe"),
            GameError::RecipeLocked(_) => write!(f, "you haven't discovered that recipe yet"),
            GameError::Busy => write!(f, "the workbench is still busy"),
            GameError::BatchTooLarge { max } => write!(f, "the workbench can bind {max} at a time"),
            GameError::MissingFlowers { detail } => write!(f, "{detail}"),
            GameError::KeepsakesNeeded => {
                write!(f, "that would use flowers set aside for a required bouquet")
            }
            GameError::NoBouquets { have } => write!(f, "only {have} of those bouquets left"),
            GameError::NotEnoughFlowers { have } => write!(f, "only {have} in the basket"),
            GameError::UnknownUpgrade => write!(f, "unknown upgrade"),
            GameError::UnknownSkill => write!(f, "unknown skill"),
            GameError::MaxLevel => write!(f, "already at its highest level"),
            GameError::NeedsSkill { skill } => {
                write!(f, "learn {} first", skill.replace('_', " "))
            }
            GameError::NotEnoughSkillPoints { needed, have } => {
                write!(f, "needs {needed} skill points; you have {have}")
            }
            GameError::SavingForTools { can_spend } => write!(
                f,
                "keep your coins for the tools you still need (you can spend {can_spend} on extras)"
            ),
            GameError::Reserved { sellable } => {
                write!(
                    f,
                    "only {sellable} can be sold; the rest are set aside for required bouquets"
                )
            }
        }
    }
}

impl std::error::Error for GameError {}

/// Reach and limits of a batch harvesting mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeParams {
    /// Circle radius (basket, sweep).
    pub radius: f32,
    /// Largest rectangle (area tool).
    pub size: Vec2,
    pub max_flowers: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchOutcome {
    pub flowers: Vec<u32>,
    pub discovered: Vec<usize>,
    pub xp_gained: Xp,
    pub region_completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStatus {
    Owned,
    Buyable,
    Locked(ToolLock),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolLock {
    Level(u32),
    NeedsTool(String),
    Coins(Coins),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HarvestOutcome {
    pub species: usize,
    /// A neighbour picked as well (Double Harvest skill).
    pub bonus_flower: Option<u32>,
    pub newly_discovered: bool,
    pub xp_gained: Xp,
    pub region_completed: bool,
}

/// Why a saved state was rejected.
#[derive(Debug, Clone, PartialEq)]
pub struct InvalidState(pub String);

impl std::fmt::Display for InvalidState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid saved game: {}", self.0)
    }
}

impl std::error::Error for InvalidState {}

/// Generated, read-only data of a region (shared with the renderer).
#[derive(Debug, Clone)]
pub struct RegionRuntime {
    pub layout: Arc<RegionLayout>,
    pub field: Arc<FlowerField>,
}

#[derive(Debug, Clone)]
pub struct Game {
    data: Arc<GameData>,
    state: GameState,
    regions: Vec<Option<RegionRuntime>>,
    harvest_cooldown: f32,
    events: Vec<GameEvent>,
    dirty: bool,
}

/// How far Double Harvest looks for a neighbour (metres).
const DOUBLE_HARVEST_RADIUS: f32 = 1.6;

/// Distance slack when validating reach: the player targets the edge of a
/// flower's pick sphere, not its centre.
const REACH_SLACK: f32 = 0.6;

impl Game {
    pub fn new(data: Arc<GameData>) -> Self {
        let state = GameState {
            coins: data.economy.starting_coins,
            rng: growth::RNG_SEED,
            ..Default::default()
        };
        let mut game = Self::with_state(data, state);
        let home = game.data.regions[0].id.clone();
        game.state.visited_regions.insert(home);
        game.refresh_regions(false);
        game.dirty = false;
        game
    }

    fn with_state(data: Arc<GameData>, state: GameState) -> Self {
        let regions = vec![None; data.regions.len()];
        Self {
            data,
            state,
            regions,
            harvest_cooldown: 0.0,
            events: Vec::new(),
            dirty: false,
        }
    }

    /// Restore a saved state after checking it is consistent with the
    /// content: known ids, well-formed bitsets of the right length, and
    /// per-species counts that match the flowers actually marked harvested.
    pub fn from_state(data: Arc<GameData>, state: GameState) -> Result<Self, InvalidState> {
        let bad = |msg: String| Err(InvalidState(msg));
        for id in state
            .inventory
            .keys()
            .chain(&state.discovered)
            .chain(state.harvested_per_species.keys())
        {
            if data.species(id).is_none() {
                return bad(format!("unknown species '{id}'"));
            }
        }
        for id in state.completed_regions.iter().chain(state.harvested.keys()) {
            if data.region(id).is_none() {
                return bad(format!("unknown region '{id}'"));
            }
        }
        for id in &state.tools {
            if data.tool(id).is_none() {
                return bad(format!("unknown tool '{id}'"));
            }
        }
        for id in state.crafted.keys() {
            if data.recipe(id).is_none() {
                return bad(format!("unknown recipe '{id}'"));
            }
        }
        for (id, &level) in &state.upgrades {
            match data.upgrade(id) {
                None => return bad(format!("unknown upgrade '{id}'")),
                Some(u) if level > u.max_level => {
                    return bad(format!("upgrade '{id}' above its highest level"))
                }
                _ => {}
            }
        }
        for (id, &level) in &state.skills {
            match data.skill(id) {
                None => return bad(format!("unknown skill '{id}'")),
                Some(s) if level > s.max_level => {
                    return bad(format!("skill '{id}' above its highest level"))
                }
                _ => {}
            }
        }
        for id in state.unlocked_regions.iter().chain(&state.visited_regions) {
            if data.region(id).is_none() {
                return bad(format!("unknown region '{id}'"));
            }
        }
        for id in &state.achievements {
            if data.achievement(id).is_none() {
                return bad(format!("unknown achievement '{id}'"));
            }
        }

        let mut game = Self::with_state(data.clone(), state);
        let mut counted: BTreeMap<String, u32> = BTreeMap::new();
        let mut total = 0u64;
        for (region_idx, region) in data.regions.iter().enumerate() {
            let Some(bits) = game.state.harvested.get(&region.id) else {
                continue;
            };
            if !bits.is_well_formed() || bits.len() != region.flower_total {
                return bad(format!(
                    "harvest record for '{}' doesn't match the region",
                    region.id
                ));
            }
            if bits.count_ones() == 0 {
                continue;
            }
            let field = game.region(region_idx).field.clone();
            for i in game.state.harvested[&region.id].iter_ones() {
                *counted
                    .entry(data.species[field.species(i)].id.clone())
                    .or_default() += 1;
                total += 1;
            }
        }
        let recorded: BTreeMap<String, u32> = game
            .state
            .harvested_per_species
            .iter()
            .filter(|(_, &n)| n > 0)
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        if counted != recorded {
            return bad("harvest totals per species don't match the harvested flowers".into());
        }
        if total != game.state.stats.flowers_harvested {
            return bad("lifetime harvest count doesn't match the harvested flowers".into());
        }
        for (id, &n) in &game.state.inventory {
            if n > counted.get(id).copied().unwrap_or(0) {
                return bad(format!("basket holds more '{id}' than were ever picked"));
            }
        }
        let recipe_ids = game
            .state
            .discovered_recipes
            .iter()
            .chain(game.state.bouquets.keys())
            .chain(game.state.crafted.keys())
            .chain(game.state.craft_job.iter().map(|j| &j.recipe));
        for id in recipe_ids {
            if data.recipe(id).is_none() {
                return bad(format!("unknown recipe '{id}'"));
            }
        }
        if let Some(job) = &game.state.craft_job {
            let ok = |v: f32| v.is_finite() && v >= 0.0;
            if job.count == 0 || !ok(job.remaining_secs) || !ok(job.total_secs) {
                return bad("the workbench job is malformed".into());
            }
        }
        // Regions and recipes the player now qualifies for (e.g. older
        // saves, content updates), without announcing them.
        game.refresh_regions(false);
        for id in &game.state.visited_regions {
            let open = data
                .region_index(id)
                .is_some_and(|r| game.is_region_unlocked(r));
            if !open {
                return bad(format!("visited region '{id}' isn't open yet"));
            }
        }
        game.refresh_recipes();
        // Older saves: catch up achievements already earned (rewards paid,
        // not announced).
        game.refresh_achievements(false);
        game.events.clear();
        Ok(game)
    }

    pub fn data(&self) -> &Arc<GameData> {
        &self.data
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Generated layout and flowers of a region (built on first use).
    pub fn region(&mut self, idx: usize) -> &RegionRuntime {
        let data = &self.data;
        self.regions[idx].get_or_insert_with(|| {
            let def = &data.regions[idx];
            let layout = RegionLayout::generate(def, &data.world);
            let field = FlowerField::generate(def, &layout, data)
                .unwrap_or_else(|e| panic!("validated content failed to place flowers: {e}"));
            RegionRuntime {
                layout: Arc::new(layout),
                field: Arc::new(field),
            }
        })
    }

    /// Harvested bits for a region (all clear if nothing was picked yet).
    pub fn harvested(&self, region: usize) -> BitSet {
        let def = &self.data.regions[region];
        self.state
            .harvested
            .get(&def.id)
            .cloned()
            .unwrap_or_else(|| BitSet::new(def.flower_total))
    }

    pub fn is_harvested(&self, region: usize, flower: u32) -> bool {
        let id = &self.data.regions[region].id;
        self.state.harvested.get(id).is_some_and(|b| b.get(flower))
    }

    pub fn remaining_in_region(&self, region: usize) -> u32 {
        let def = &self.data.regions[region];
        def.flower_total
            - self
                .state
                .harvested
                .get(&def.id)
                .map_or(0, |b| b.count_ones())
    }

    pub fn level(&self) -> u32 {
        self.data
            .progression
            .level_curve
            .level_for_xp(self.state.xp)
    }

    /// `(xp into the current level, xp needed for the next)`; `None` at max level.
    pub fn level_progress(&self) -> Option<(Xp, Xp)> {
        let curve = &self.data.progression.level_curve;
        let level = self.level();
        let needed = curve.xp_to_next(level)?;
        Some((self.state.xp - curve.total_xp_for_level(level), needed))
    }

    pub fn is_region_unlocked(&self, region: usize) -> bool {
        self.state
            .unlocked_regions
            .contains(&self.data.regions[region].id)
            || self.meets_region_unlock(region)
    }

    pub fn inventory_count(&self, species: usize) -> u32 {
        self.state
            .inventory
            .get(&self.data.species[species].id)
            .copied()
            .unwrap_or(0)
    }

    pub fn is_discovered(&self, species: usize) -> bool {
        self.state
            .discovered
            .contains(&self.data.species[species].id)
    }

    /// Seconds between single picks with the tools currently owned.
    pub fn harvest_cooldown_secs(&self) -> f32 {
        let mut secs = self.data.harvesting.hand_cooldown_secs;
        for tool in self.state.tools.iter().filter_map(|t| self.data.tool(t)) {
            for effect in &tool.effects {
                if let ToolEffect::HarvestCooldownMultiplier(m) = effect {
                    secs *= m;
                }
            }
        }
        secs * Modifiers::time_factor(self.modifiers().pick_cooldown)
    }

    pub fn harvest_cooldown_remaining(&self) -> f32 {
        self.harvest_cooldown
    }

    /// Advance game time (only while actually playing).
    pub fn tick(&mut self, dt: f32) {
        self.harvest_cooldown = (self.harvest_cooldown - dt).max(0.0);
        self.state.stats.playtime_secs += dt as f64;
        self.progress_craft(dt);
    }

    /// The flower the player is looking at, if any is within reach.
    pub fn look_at(&mut self, region: usize, eye: Vec3, dir: Vec3) -> Option<u32> {
        let range = self.reach();
        let bits = self.harvested(region);
        self.region(region)
            .field
            .pick(eye, dir, range, &bits)
            .map(|(i, _)| i)
    }

    /// Pick one flower. `eye` is where the player is looking from; the
    /// flower must be within reach.
    pub fn harvest(
        &mut self,
        region: usize,
        flower: u32,
        eye: Vec3,
    ) -> Result<HarvestOutcome, GameError> {
        let field = self.begin_harvest(region)?;
        if flower as usize >= field.len() {
            return Err(GameError::UnknownFlower);
        }
        if self.is_harvested(region, flower) {
            return Err(GameError::AlreadyHarvested);
        }
        let range = self.reach();
        let (center, radius) = field.pick_sphere(flower);
        let distance = eye.distance(center) - radius;
        if distance > range + REACH_SLACK {
            return Err(GameError::OutOfReach { distance });
        }

        // --- validated: apply ---
        let level_before = self.level();
        let (species, newly_discovered, mut xp) = self.apply_one(region, &field, flower);
        // Double Harvest: sometimes the nearest neighbour comes along too.
        let mut bonus_flower = None;
        if self.roll(self.modifiers().double_harvest) {
            let bits = self.harvested(region);
            let here = field.position(flower);
            let neighbour = field
                .query_radius(here, DOUBLE_HARVEST_RADIUS, &bits)
                .into_iter()
                .min_by(|&a, &b| {
                    field
                        .position(a)
                        .distance(here)
                        .total_cmp(&field.position(b).distance(here))
                });
            if let Some(n) = neighbour {
                let (_, _, gained) = self.apply_one(region, &field, n);
                xp += gained;
                bonus_flower = Some(n);
            }
        }
        let region_completed = self.finish_batch(region, level_before, &mut xp);
        self.harvest_cooldown = self.harvest_cooldown_secs();
        Ok(HarvestOutcome {
            species,
            bonus_flower,
            newly_discovered,
            xp_gained: xp,
            region_completed,
        })
    }

    // ---------------------------------------------------------------------
    // Harvesting modes (basket, sweep, area)
    // ---------------------------------------------------------------------

    /// Modes the player can use with the tools they own, in tool order.
    pub fn available_modes(&self) -> Vec<HarvestMode> {
        let mut modes = vec![HarvestMode::Single];
        for mode in [HarvestMode::Radius, HarvestMode::Sweep, HarvestMode::Area] {
            if self.mode_params(mode).is_some() {
                modes.push(mode);
            }
        }
        modes
    }

    /// Reach and limits of a batch mode from the best owned tool, or `None`
    /// if no owned tool provides it. (`Single` is not a batch mode.)
    pub fn mode_params(&self, mode: HarvestMode) -> Option<ModeParams> {
        let m = self.modifiers();
        self.tool_mode_params(mode).map(|p| ModeParams {
            radius: p.radius * (1.0 + m.gather_radius),
            size: p.size * (1.0 + m.area_size),
            max_flowers: (p.max_flowers as f32 * (1.0 + m.gather_capacity)).round() as u32,
        })
    }

    /// Mode parameters from tools alone (before upgrades and skills).
    fn tool_mode_params(&self, mode: HarvestMode) -> Option<ModeParams> {
        let mut best: Option<ModeParams> = None;
        for tool in self.state.tools.iter().filter_map(|t| self.data.tool(t)) {
            for effect in &tool.effects {
                let p = match (effect, mode) {
                    (
                        &ToolEffect::RadiusMode {
                            radius,
                            max_flowers,
                        },
                        HarvestMode::Radius,
                    ) => ModeParams {
                        radius,
                        size: Vec2::ZERO,
                        max_flowers,
                    },
                    (
                        &ToolEffect::SweepMode {
                            radius,
                            max_flowers_per_tick,
                        },
                        HarvestMode::Sweep,
                    ) => ModeParams {
                        radius,
                        size: Vec2::ZERO,
                        max_flowers: max_flowers_per_tick,
                    },
                    (
                        &ToolEffect::AreaMode {
                            max_width,
                            max_height,
                            max_flowers,
                        },
                        HarvestMode::Area,
                    ) => ModeParams {
                        radius: 0.0,
                        size: Vec2::new(max_width, max_height),
                        max_flowers,
                    },
                    _ => continue,
                };
                if best.is_none_or(|b| p.max_flowers > b.max_flowers || p.radius > b.radius) {
                    best = Some(p);
                }
            }
        }
        best
    }

    /// How far away the ground can be targeted (basket, sweep, area).
    pub fn ground_reach(&self) -> f32 {
        self.reach() + self.data.harvesting.ground_reach_bonus
    }

    /// Unpicked flowers a basket or sweep centred on `center` would gather,
    /// nearest first, capped by the tool.
    pub fn preview_circle(&mut self, region: usize, mode: HarvestMode, center: Vec2) -> Vec<u32> {
        let Some(params) = self.mode_params(mode).filter(|_| mode != HarvestMode::Area) else {
            return Vec::new();
        };
        let bits = self.harvested(region);
        let field = self.region(region).field.clone();
        let mut hits = field.query_radius(center, params.radius, &bits);
        hits.sort_by(|&a, &b| {
            field
                .position(a)
                .distance(center)
                .total_cmp(&field.position(b).distance(center))
        });
        hits.truncate(params.max_flowers as usize);
        hits
    }

    /// The area an area tool marks from `start` towards `end` (clamped to the
    /// tool's size) and the unpicked flowers inside it.
    pub fn preview_area(
        &mut self,
        region: usize,
        start: Vec2,
        end: Vec2,
    ) -> Option<(Vec2, Vec2, Vec<u32>)> {
        let params = self.mode_params(HarvestMode::Area)?;
        let half = params.size;
        let end = start + (end - start).clamp(-half, half);
        let (min, max) = (start.min(end), start.max(end));
        let bits = self.harvested(region);
        let field = self.region(region).field.clone();
        let mut hits = field.query_rect(min, max, &bits);
        hits.truncate(params.max_flowers as usize);
        Some((min, max, hits))
    }

    /// Basket: gather every flower in a small circle on the ground in reach.
    pub fn harvest_radius(
        &mut self,
        region: usize,
        center: Vec3,
        eye: Vec3,
    ) -> Result<BatchOutcome, GameError> {
        self.harvest_circle(region, HarvestMode::Radius, center, eye)
    }

    /// Sweep: called repeatedly while the button is held.
    pub fn harvest_sweep(
        &mut self,
        region: usize,
        center: Vec3,
        eye: Vec3,
    ) -> Result<BatchOutcome, GameError> {
        self.harvest_circle(region, HarvestMode::Sweep, center, eye)
    }

    fn harvest_circle(
        &mut self,
        region: usize,
        mode: HarvestMode,
        center: Vec3,
        eye: Vec3,
    ) -> Result<BatchOutcome, GameError> {
        if self.mode_params(mode).is_none() {
            return Err(GameError::ModeLocked);
        }
        let field = self.begin_harvest(region)?;
        let distance = eye.distance(center);
        if distance > self.ground_reach() + REACH_SLACK {
            return Err(GameError::OutOfReach { distance });
        }
        let targets = self.preview_circle(region, mode, Vec2::new(center.x, center.z));
        if targets.is_empty() {
            return Err(GameError::NothingInReach);
        }
        let cooldown = match mode {
            HarvestMode::Sweep => self.data.harvesting.sweep_interval_secs,
            _ => self.data.harvesting.radius_cooldown_secs,
        };
        Ok(self.apply_batch(region, &field, &targets, cooldown))
    }

    /// Area tool: gather everything in the marked rectangle. The player must
    /// stand within ground reach of it.
    pub fn harvest_area(
        &mut self,
        region: usize,
        start: Vec2,
        end: Vec2,
        eye: Vec3,
    ) -> Result<BatchOutcome, GameError> {
        if self.mode_params(HarvestMode::Area).is_none() {
            return Err(GameError::ModeLocked);
        }
        let field = self.begin_harvest(region)?;
        let (min, max, targets) = self
            .preview_area(region, start, end)
            .ok_or(GameError::ModeLocked)?;
        let here = Vec2::new(eye.x, eye.z);
        let distance = here.clamp(min, max).distance(here);
        if distance > self.ground_reach() + REACH_SLACK {
            return Err(GameError::OutOfReach { distance });
        }
        if targets.is_empty() {
            return Err(GameError::NothingInReach);
        }
        let cooldown = self.data.harvesting.area_cooldown_secs;
        Ok(self.apply_batch(region, &field, &targets, cooldown))
    }

    fn apply_batch(
        &mut self,
        region: usize,
        field: &FlowerField,
        targets: &[u32],
        cooldown: f32,
    ) -> BatchOutcome {
        let level_before = self.level();
        let mut xp = 0;
        let mut discovered = Vec::new();
        for &flower in targets {
            let (species, newly, gained) = self.apply_one(region, field, flower);
            xp += gained;
            if newly {
                discovered.push(species);
            }
        }
        let region_completed = self.finish_batch(region, level_before, &mut xp);
        self.harvest_cooldown = cooldown;
        BatchOutcome {
            flowers: targets.to_vec(),
            discovered,
            xp_gained: xp,
            region_completed,
        }
    }

    // ---------------------------------------------------------------------
    // Harvest internals shared by every mode
    // ---------------------------------------------------------------------

    /// Checks common to every harvest; returns the region's flower field.
    fn begin_harvest(&mut self, region: usize) -> Result<Arc<FlowerField>, GameError> {
        if region >= self.data.regions.len() {
            return Err(GameError::UnknownRegion);
        }
        if !self.is_region_unlocked(region) {
            return Err(GameError::RegionLocked);
        }
        if self.harvest_cooldown > 0.0 {
            return Err(GameError::CoolingDown {
                remaining: self.harvest_cooldown,
            });
        }
        Ok(self.region(region).field.clone())
    }

    /// Mark one (validated, unpicked) flower harvested and put it in the
    /// basket. Returns `(species, newly discovered, xp gained)`.
    fn apply_one(&mut self, region: usize, field: &FlowerField, flower: u32) -> (usize, bool, Xp) {
        let data = self.data.clone();
        let region_def = &data.regions[region];
        let bits = self
            .state
            .harvested
            .entry(region_def.id.clone())
            .or_insert_with(|| BitSet::new(region_def.flower_total));
        let changed = bits.test_and_set(flower);
        debug_assert!(changed, "callers only pass unpicked flowers");

        let species = field.species(flower);
        let species_id = data.species[species].id.clone();
        *self.state.inventory.entry(species_id.clone()).or_default() += 1;
        *self
            .state
            .harvested_per_species
            .entry(species_id.clone())
            .or_default() += 1;
        self.state.stats.flowers_harvested += 1;
        let mut xp = data.progression.xp_per_harvest;
        let newly = self.state.discovered.insert(species_id);
        if newly {
            let bonus = 1.0 + self.modifiers().discovery_xp as f64;
            xp += (data.progression.xp_per_discovery as f64 * bonus).round() as Xp;
        }
        self.events.push(GameEvent::FlowerHarvested {
            region,
            flower,
            species,
        });
        if newly {
            self.events.push(GameEvent::SpeciesDiscovered { species });
            let found = self.state.discovered.len() as u32;
            if data.progression.collection_milestones.contains(&found) {
                self.events.push(GameEvent::CollectionMilestone {
                    species_found: found,
                });
            }
        }
        (species, newly, xp)
    }

    /// Region completion, XP and level-up events after one or more picks.
    fn finish_batch(&mut self, region: usize, level_before: u32, xp: &mut Xp) -> bool {
        let data = self.data.clone();
        let region_def = &data.regions[region];
        let mut completed = false;
        if self.remaining_in_region(region) == 0
            && self.state.completed_regions.insert(region_def.id.clone())
        {
            completed = true;
            let coins = region_def.completion_reward.coins;
            *xp += data.progression.xp_region_complete;
            self.earn(coins);
            self.events
                .push(GameEvent::RegionCompleted { region, coins });
        }
        self.award_xp(level_before, *xp);
        completed
    }

    /// Add XP, announce level-ups and newly qualified recipes.
    fn award_xp(&mut self, level_before: u32, xp: Xp) {
        self.state.xp += xp;
        // May pay an unlock bonus (coins and XP) before levels are counted.
        self.refresh_regions(true);
        for level in (level_before + 1)..=self.level() {
            self.events.push(GameEvent::LevelUp { level });
        }
        self.refresh_recipes();
        self.refresh_achievements(true);
        self.dirty = true;
    }

    // ---------------------------------------------------------------------
    // Tools
    // ---------------------------------------------------------------------

    pub fn owns_tool(&self, tool: usize) -> bool {
        self.state.tools.contains(&self.data.tools[tool].id)
    }

    /// Whether a tool is owned, can be bought now, or why not.
    pub fn tool_status(&self, tool: usize) -> ToolStatus {
        let def = &self.data.tools[tool];
        if self.owns_tool(tool) {
            return ToolStatus::Owned;
        }
        if self.level() < def.unlock_level {
            return ToolStatus::Locked(ToolLock::Level(def.unlock_level));
        }
        if let Some(missing) = def
            .requires_tools
            .iter()
            .find(|t| !self.state.tools.contains(*t))
        {
            return ToolStatus::Locked(ToolLock::NeedsTool(missing.clone()));
        }
        let price = self.tool_price(tool);
        if self.state.coins < price {
            return ToolStatus::Locked(ToolLock::Coins(price));
        }
        ToolStatus::Buyable
    }

    /// Buy a tool. Tools are permanent and required for completion, so the
    /// optional-purchase guard never applies to them.
    pub fn buy_tool(&mut self, tool: usize) -> Result<Coins, GameError> {
        let Some(def) = self.data.tools.get(tool).cloned() else {
            return Err(GameError::UnknownTool);
        };
        match self.tool_status(tool) {
            ToolStatus::Owned => Err(GameError::AlreadyOwned),
            ToolStatus::Locked(ToolLock::Level(needed)) => Err(GameError::LevelTooLow { needed }),
            ToolStatus::Locked(ToolLock::NeedsTool(tool)) => Err(GameError::NeedsTool { tool }),
            ToolStatus::Locked(ToolLock::Coins(price)) => Err(GameError::NotEnoughCoins {
                price,
                have: self.state.coins,
            }),
            ToolStatus::Buyable => {
                let price = self.tool_price(tool);
                self.state.coins -= price;
                self.state.stats.coins_spent += price;
                self.state.tools.insert(def.id.clone());
                self.events.push(GameEvent::ToolBought { tool });
                self.refresh_recipes();
                self.refresh_achievements(true);
                self.dirty = true;
                Ok(price)
            }
        }
    }

    pub(super) fn earn(&mut self, coins: Coins) {
        self.state.coins += coins;
        self.state.stats.coins_earned += coins;
    }

    // ---------------------------------------------------------------------
    // Selling and the keepsake reserve
    // ---------------------------------------------------------------------

    /// How many of this species can be sold now without making a required,
    /// not-yet-crafted bouquet impossible to ever complete.
    pub fn sellable(&self, species: usize) -> u32 {
        self.free_amount(species, None)
    }

    /// How many of this species can leave the basket (sold, or bound into
    /// `skip`) without endangering another required bouquet.
    fn free_amount(&self, species: usize, skip: Option<usize>) -> u32 {
        let have = self.inventory_count(species);
        if have == 0 {
            return 0;
        }
        let mut pool = self.supply_pool();
        pool[species] -= have as u64;
        let feasible_after = |sold: u32, pool: &[u64]| {
            let mut p = pool.to_vec();
            p[species] += (have - sold) as u64;
            self.reserve_feasible(&mut p, skip)
        };
        // Feasibility only gets harder the more is sold: binary search.
        let (mut lo, mut hi) = (0u32, have);
        if feasible_after(have, &pool) {
            return have;
        }
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if feasible_after(mid, &pool) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }

    /// Flowers of this species held back for required bouquets.
    pub fn reserved(&self, species: usize) -> u32 {
        self.inventory_count(species) - self.sellable(species)
    }

    pub fn sell(&mut self, species: usize, count: u32) -> Result<Coins, GameError> {
        if species >= self.data.species.len() {
            return Err(GameError::UnknownSpecies);
        }
        if count == 0 {
            return Err(GameError::NothingToSell);
        }
        let have = self.inventory_count(species);
        if count > have {
            return Err(GameError::NotEnoughFlowers { have });
        }
        let sellable = self.sellable(species);
        if count > sellable {
            return Err(GameError::Reserved { sellable });
        }
        let id = self.data.species[species].id.clone();
        let left = have - count;
        if left == 0 {
            self.state.inventory.remove(&id);
        } else {
            self.state.inventory.insert(id, left);
        }
        let coins = self.data.economy.raw_flower_value(count as u64);
        self.earn(coins);
        self.state.stats.flowers_sold += count as u64;
        self.events.push(GameEvent::FlowersSold {
            species,
            count,
            coins,
        });
        self.dirty = true;
        Ok(coins)
    }

    /// Sell every flower that isn't reserved. Returns `(flowers, coins)`.
    pub fn sell_all(&mut self) -> (u32, Coins) {
        let mut flowers = 0;
        let mut coins = 0;
        for species in 0..self.data.species.len() {
            let n = self.sellable(species);
            if n > 0 {
                if let Ok(c) = self.sell(species, n) {
                    flowers += n;
                    coins += c;
                }
            }
        }
        (flowers, coins)
    }

    /// Per species: basket + flowers still growing anywhere in the world.
    fn supply_pool(&self) -> Vec<u64> {
        self.data
            .species
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let world = self.data.species_world_total(&s.id);
                let picked = self
                    .state
                    .harvested_per_species
                    .get(&s.id)
                    .copied()
                    .unwrap_or(0) as u64;
                self.inventory_count(i) as u64 + world.saturating_sub(picked)
            })
            .collect()
    }

    /// Can every required recipe that hasn't been crafted yet still be made
    /// once from `pool`? Greedy (named species, then distinct species from
    /// the most plentiful, then any) — never more permissive than an exact
    /// solver, so it is a safe guard.
    /// `skip`: a recipe about to be crafted (its own needs no longer count).
    fn reserve_feasible(&self, pool: &mut [u64], skip: Option<usize>) -> bool {
        let pending: Vec<_> = self
            .data
            .recipes
            .iter()
            .enumerate()
            .filter(|(i, r)| {
                Some(*i) != skip
                    && r.required_for_completion
                    && !self.state.crafted.contains_key(&r.id)
            })
            .map(|(_, r)| r)
            .collect();
        for recipe in &pending {
            for req in &recipe.required_species {
                let s = self.data.species_index(&req.species).expect("validated");
                if pool[s] < req.count as u64 {
                    return false;
                }
                pool[s] -= req.count as u64;
            }
        }
        for recipe in &pending {
            let named: BTreeSet<usize> = recipe
                .required_species
                .iter()
                .map(|r| self.data.species_index(&r.species).expect("validated"))
                .collect();
            let extra = (recipe.min_distinct_species as usize).saturating_sub(named.len());
            let mut free = recipe.free_slots() as u64;
            // One flower each from `extra` further species, most plentiful first.
            let mut others: Vec<usize> = (0..pool.len())
                .filter(|s| !named.contains(s) && pool[*s] > 0)
                .collect();
            if others.len() < extra {
                return false;
            }
            others.sort_by_key(|&s| std::cmp::Reverse(pool[s]));
            for &s in &others[..extra] {
                pool[s] -= 1;
                free = free.saturating_sub(1);
            }
            while free > 0 {
                let Some(s) = (0..pool.len())
                    .max_by_key(|&s| pool[s])
                    .filter(|&s| pool[s] > 0)
                else {
                    return false;
                };
                let take = free.min(pool[s]);
                pool[s] -= take;
                free -= take;
            }
        }
        true
    }

    // ---------------------------------------------------------------------
    // Persistence helpers
    // ---------------------------------------------------------------------

    pub fn set_player_pose(&mut self, pose: PlayerPose) {
        self.state.player = Some(pose);
    }

    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }
}
