//! The authoritative game: persistent [`GameState`] plus the commands that
//! change it.
//!
//! Every gameplay mutation goes through a method here that validates first
//! and either applies the whole change or returns a [`GameError`] without
//! touching anything. Commands queue [`GameEvent`]s for presentation (sounds,
//! toasts, animations) and mark the game dirty for autosave.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::bitset::BitSet;
use crate::data::{GameData, ToolEffect};
use crate::economy::Coins;
use crate::progression::Xp;
use crate::world::{FlowerField, RegionLayout};

/// Everything that is saved. Keys are content ids (not indices) so saves
/// survive content being reordered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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
    /// Owned tools (Phase D/E).
    pub tools: BTreeSet<String>,
    /// Bouquets crafted per recipe id (Phase E).
    pub crafted: BTreeMap<String, u32>,
    pub stats: Stats,
    /// Where the player stood when the game was saved.
    pub player: Option<PlayerPose>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Stats {
    pub flowers_harvested: u64,
    pub flowers_sold: u64,
    pub coins_earned: Coins,
    pub playtime_secs: f64,
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
    FlowersSold {
        species: usize,
        count: u32,
        coins: Coins,
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
    /// Some of these flowers are set aside for required bouquets.
    Reserved {
        sellable: u32,
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
            GameError::NotEnoughFlowers { have } => write!(f, "only {have} in the basket"),
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

#[derive(Debug, Clone, PartialEq)]
pub struct HarvestOutcome {
    pub species: usize,
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

/// Distance slack when validating reach: the player targets the edge of a
/// flower's pick sphere, not its centre.
const REACH_SLACK: f32 = 0.6;

impl Game {
    pub fn new(data: Arc<GameData>) -> Self {
        let state = GameState {
            coins: data.economy.starting_coins,
            ..Default::default()
        };
        Self::with_state(data, state)
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
        let unlock = self.data.regions[region].unlock;
        self.level() >= unlock.level
            && self.state.stats.flowers_harvested >= unlock.flowers_harvested as u64
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
        secs
    }

    pub fn harvest_cooldown_remaining(&self) -> f32 {
        self.harvest_cooldown
    }

    /// Advance game time (only while actually playing).
    pub fn tick(&mut self, dt: f32) {
        self.harvest_cooldown = (self.harvest_cooldown - dt).max(0.0);
        self.state.stats.playtime_secs += dt as f64;
    }

    /// The flower the player is looking at, if any is within reach.
    pub fn look_at(&mut self, region: usize, eye: Vec3, dir: Vec3) -> Option<u32> {
        let range = self.data.player.interact_range;
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
        let range = self.data.player.interact_range;
        let field = self.region(region).field.clone();
        if flower as usize >= field.len() {
            return Err(GameError::UnknownFlower);
        }
        if self.is_harvested(region, flower) {
            return Err(GameError::AlreadyHarvested);
        }
        let (center, radius) = field.pick_sphere(flower);
        let distance = eye.distance(center) - radius;
        if distance > range + REACH_SLACK {
            return Err(GameError::OutOfReach { distance });
        }

        // --- validated: apply ---
        let data = self.data.clone();
        let region_def = &data.regions[region];
        let bits = self
            .state
            .harvested
            .entry(region_def.id.clone())
            .or_insert_with(|| BitSet::new(region_def.flower_total));
        let changed = bits.test_and_set(flower);
        debug_assert!(changed, "checked above");

        let species = field.species(flower);
        let species_id = data.species[species].id.clone();
        let level_before = self.level();
        *self.state.inventory.entry(species_id.clone()).or_default() += 1;
        *self
            .state
            .harvested_per_species
            .entry(species_id.clone())
            .or_default() += 1;
        self.state.stats.flowers_harvested += 1;
        let mut xp = data.progression.xp_per_harvest;
        let newly_discovered = self.state.discovered.insert(species_id);
        if newly_discovered {
            xp += data.progression.xp_per_discovery;
        }
        self.events.push(GameEvent::FlowerHarvested {
            region,
            flower,
            species,
        });
        if newly_discovered {
            self.events.push(GameEvent::SpeciesDiscovered { species });
        }

        let mut region_completed = false;
        if self.remaining_in_region(region) == 0
            && self.state.completed_regions.insert(region_def.id.clone())
        {
            region_completed = true;
            let coins = region_def.completion_reward.coins;
            xp += data.progression.xp_region_complete;
            self.earn(coins);
            self.events
                .push(GameEvent::RegionCompleted { region, coins });
        }

        self.state.xp += xp;
        for level in (level_before + 1)..=self.level() {
            self.events.push(GameEvent::LevelUp { level });
        }
        self.harvest_cooldown = self.harvest_cooldown_secs();
        self.dirty = true;
        Ok(HarvestOutcome {
            species,
            newly_discovered,
            xp_gained: xp,
            region_completed,
        })
    }

    fn earn(&mut self, coins: Coins) {
        self.state.coins += coins;
        self.state.stats.coins_earned += coins;
    }

    // ---------------------------------------------------------------------
    // Selling and the keepsake reserve
    // ---------------------------------------------------------------------

    /// How many of this species can be sold now without making a required,
    /// not-yet-crafted bouquet impossible to ever complete.
    pub fn sellable(&self, species: usize) -> u32 {
        let have = self.inventory_count(species);
        if have == 0 {
            return 0;
        }
        let mut pool = self.supply_pool();
        pool[species] -= have as u64;
        let feasible_after = |sold: u32, pool: &[u64]| {
            let mut p = pool.to_vec();
            p[species] += (have - sold) as u64;
            self.reserve_feasible(&mut p)
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
    fn reserve_feasible(&self, pool: &mut [u64]) -> bool {
        let pending: Vec<_> = self
            .data
            .recipes
            .iter()
            .filter(|r| r.required_for_completion && !self.state.crafted.contains_key(&r.id))
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
