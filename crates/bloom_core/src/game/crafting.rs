//! Bouquet crafting at the workbench, and selling bouquets.
//!
//! * A recipe is **discovered** once the player owns all its tools, has
//!   discovered every species it names, and has reached its level.
//! * Crafting is always an explicit player action: choose a recipe and a
//!   quantity (up to the workbench's batch capacity). The ingredients are
//!   planned automatically from the most plentiful flowers; the player may
//!   exclude species they want to keep. Flowers set aside for *other*
//!   required bouquets are never used.
//! * Validation happens first; on success the flowers are consumed at once
//!   and the batch binds over `craft_time_secs × count` of play time. The job
//!   is saved, so quitting mid-batch loses nothing. Bouquets (and their XP)
//!   arrive when the timer ends.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Game, GameError, GameEvent, Modifiers};
use crate::data::ToolEffect;
use crate::economy::Coins;

/// A batch being bound at the workbench (saved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CraftJob {
    pub recipe: String,
    pub count: u32,
    pub remaining_secs: f32,
    pub total_secs: f32,
    /// This is the recipe's first ever batch (earns the first-craft bonus).
    pub first: bool,
}

impl CraftJob {
    /// 0 → 1 as the batch binds.
    pub fn progress(&self) -> f32 {
        if self.total_secs <= 0.0 {
            1.0
        } else {
            (1.0 - self.remaining_secs / self.total_secs).clamp(0.0, 1.0)
        }
    }
}

/// What still stands between the player and a recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeLock {
    Tool(String),
    Species(String),
    Level(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeStatus {
    Discovered,
    Locked(Vec<RecipeLock>),
}

/// The flowers a craft will use, in total for the whole batch.
#[derive(Debug, Clone, PartialEq)]
pub struct CraftPlan {
    pub recipe: usize,
    pub count: u32,
    /// `(species index, flowers)`, most used first.
    pub ingredients: Vec<(usize, u32)>,
    pub secs: f32,
}

impl Game {
    pub fn recipe_locks(&self, recipe: usize) -> Vec<RecipeLock> {
        let def = &self.data.recipes[recipe];
        let mut locks = Vec::new();
        for tool in &def.required_tools {
            if !self.state.tools.contains(tool) {
                locks.push(RecipeLock::Tool(tool.clone()));
            }
        }
        for req in &def.required_species {
            if !self.state.discovered.contains(&req.species) {
                locks.push(RecipeLock::Species(req.species.clone()));
            }
        }
        if self.level() < def.unlock_level {
            locks.push(RecipeLock::Level(def.unlock_level));
        }
        locks
    }

    pub fn recipe_status(&self, recipe: usize) -> RecipeStatus {
        if self.is_recipe_discovered(recipe) {
            RecipeStatus::Discovered
        } else {
            RecipeStatus::Locked(self.recipe_locks(recipe))
        }
    }

    pub fn is_recipe_discovered(&self, recipe: usize) -> bool {
        self.state
            .discovered_recipes
            .contains(&self.data.recipes[recipe].id)
    }

    /// Record (and announce) recipes the player newly qualifies for.
    pub(super) fn refresh_recipes(&mut self) {
        for recipe in 0..self.data.recipes.len() {
            if !self.is_recipe_discovered(recipe) && self.recipe_locks(recipe).is_empty() {
                self.state
                    .discovered_recipes
                    .insert(self.data.recipes[recipe].id.clone());
                self.events.push(GameEvent::RecipeDiscovered { recipe });
                self.dirty = true;
            }
        }
    }

    /// How many bouquets one workbench action may bind.
    pub fn batch_capacity(&self) -> u32 {
        let mut capacity = self.data.economy.base_craft_batch_capacity;
        for tool in self.state.tools.iter().filter_map(|t| self.data.tool(t)) {
            for effect in &tool.effects {
                if let ToolEffect::CraftBatchCapacity(c) = *effect {
                    capacity = capacity.max(c);
                }
            }
        }
        capacity + self.modifiers().batch_capacity
    }

    pub fn bouquet_count(&self, recipe: usize) -> u32 {
        self.state
            .bouquets
            .get(&self.data.recipes[recipe].id)
            .copied()
            .unwrap_or(0)
    }

    pub fn crafted_count(&self, recipe: usize) -> u32 {
        self.state
            .crafted
            .get(&self.data.recipes[recipe].id)
            .copied()
            .unwrap_or(0)
    }

    pub fn craft_job(&self) -> Option<&CraftJob> {
        self.state.craft_job.as_ref()
    }

    /// Choose the flowers for `count` bouquets of `recipe` without changing
    /// anything. `exclude` lists species the player wants to keep (named
    /// ingredients can't be excluded).
    pub fn plan_craft(
        &self,
        recipe: usize,
        count: u32,
        exclude: &BTreeSet<usize>,
    ) -> Result<CraftPlan, GameError> {
        let def = self
            .data
            .recipes
            .get(recipe)
            .ok_or(GameError::UnknownRecipe)?;
        let n = self.data.species.len();
        let named: Vec<(usize, u32)> = def
            .required_species
            .iter()
            .map(|r| {
                (
                    self.data.species_index(&r.species).expect("validated"),
                    r.count,
                )
            })
            .collect();
        let named_set: BTreeSet<usize> = named.iter().map(|(s, _)| *s).collect();

        // What may be used: the basket, minus keepsakes for other required
        // bouquets, minus species the player chose to keep.
        let mut usable: Vec<u32> = (0..n)
            .map(|s| {
                if (exclude.contains(&s) && !named_set.contains(&s)) || self.inventory_count(s) == 0
                {
                    0
                } else {
                    self.free_amount(s, Some(recipe))
                }
            })
            .collect();
        let name = |s: usize| self.data.species[s].name.clone();
        let mut totals = vec![0u32; n];
        for _ in 0..count {
            for &(s, c) in &named {
                if usable[s] < c {
                    return Err(GameError::MissingFlowers {
                        detail: format!("each {} needs {c} {}", def.name, name(s)),
                    });
                }
                usable[s] -= c;
                totals[s] += c;
            }
            let extra = (def.min_distinct_species as usize).saturating_sub(named.len());
            let mut free = def.free_slots();
            let mut others: Vec<usize> = (0..n)
                .filter(|s| !named_set.contains(s) && usable[*s] > 0)
                .collect();
            if others.len() < extra {
                return Err(GameError::MissingFlowers {
                    detail: format!(
                        "each {} needs {} different species",
                        def.name, def.min_distinct_species
                    ),
                });
            }
            others.sort_by_key(|&s| std::cmp::Reverse(usable[s]));
            for &s in &others[..extra] {
                usable[s] -= 1;
                totals[s] += 1;
                free -= 1;
            }
            while free > 0 {
                let Some(s) = (0..n).filter(|&s| usable[s] > 0).max_by_key(|&s| usable[s]) else {
                    return Err(GameError::MissingFlowers {
                        detail: format!("each {} needs {} flowers", def.name, def.flower_count),
                    });
                };
                let take = free.min(usable[s]);
                usable[s] -= take;
                totals[s] += take;
                free -= take;
            }
        }

        // The batch as a whole must still leave every other required
        // bouquet possible.
        let mut pool = self.supply_pool();
        for (p, t) in pool.iter_mut().zip(&totals) {
            *p -= *t as u64;
        }
        if !self.reserve_feasible(&mut pool, Some(recipe)) {
            return Err(GameError::KeepsakesNeeded);
        }

        let mut ingredients: Vec<(usize, u32)> = totals
            .iter()
            .enumerate()
            .filter(|(_, &t)| t > 0)
            .map(|(s, &t)| (s, t))
            .collect();
        ingredients.sort_by_key(|&(s, t)| (std::cmp::Reverse(t), s));
        Ok(CraftPlan {
            recipe,
            count,
            ingredients,
            secs: def.craft_time_secs
                * count as f32
                * Modifiers::time_factor(self.modifiers().craft_time),
        })
    }

    /// Start binding `count` bouquets. Nothing changes unless it succeeds.
    pub fn craft(
        &mut self,
        recipe: usize,
        count: u32,
        exclude: &BTreeSet<usize>,
    ) -> Result<CraftPlan, GameError> {
        if recipe >= self.data.recipes.len() {
            return Err(GameError::UnknownRecipe);
        }
        if !self.is_recipe_discovered(recipe) {
            return Err(GameError::RecipeLocked(self.recipe_locks(recipe)));
        }
        if self.state.craft_job.is_some() {
            return Err(GameError::Busy);
        }
        let max = self.batch_capacity();
        if count == 0 || count > max {
            return Err(GameError::BatchTooLarge { max });
        }
        let plan = self.plan_craft(recipe, count, exclude)?;

        // --- validated: apply ---
        let data = self.data.clone();
        for &(s, used) in &plan.ingredients {
            let id = &data.species[s].id;
            let left = self.inventory_count(s) - used;
            if left == 0 {
                self.state.inventory.remove(id);
            } else {
                self.state.inventory.insert(id.clone(), left);
            }
        }
        let id = data.recipes[recipe].id.clone();
        let before = self.crafted_count(recipe);
        *self.state.crafted.entry(id.clone()).or_default() += count;
        self.state.stats.bouquets_crafted += count as u64;
        self.state.craft_job = Some(CraftJob {
            recipe: id,
            count,
            remaining_secs: plan.secs,
            total_secs: plan.secs,
            first: before == 0,
        });
        self.events.push(GameEvent::CraftStarted { recipe, count });
        // Efficient Production: each bouquet may save one of its flowers
        // (only ever flowers this craft used, so the world stays finite).
        let chance = self.modifiers().flower_saver;
        let saved = (0..count).filter(|_| self.roll(chance)).count() as u32;
        if saved > 0 {
            let (species, used) = plan.ingredients[0];
            let saved = saved.min(used);
            *self
                .state
                .inventory
                .entry(data.species[species].id.clone())
                .or_default() += saved;
            self.events.push(GameEvent::FlowersSaved {
                species,
                count: saved,
            });
        }
        self.dirty = true;
        if plan.secs <= 0.0 {
            self.progress_craft(0.0);
        }
        Ok(plan)
    }

    /// Advance the workbench; deliver the batch when it's done.
    pub(super) fn progress_craft(&mut self, dt: f32) {
        let Some(job) = self.state.craft_job.as_mut() else {
            return;
        };
        job.remaining_secs = (job.remaining_secs - dt).max(0.0);
        if job.remaining_secs > 0.0 {
            return;
        }
        let job = self.state.craft_job.take().expect("checked above");
        let recipe = self.data.recipe_index(&job.recipe).expect("validated job");
        *self.state.bouquets.entry(job.recipe.clone()).or_default() += job.count;
        let progression = &self.data.progression;
        let mut xp = progression.craft_xp(self.data.recipes[recipe].tier) * job.count as u64;
        xp = (xp as f64 * (1.0 + self.modifiers().craft_xp as f64)).round() as u64;
        if job.first {
            xp += progression.xp_first_craft;
        }
        self.events.push(GameEvent::BouquetsReady {
            recipe,
            count: job.count,
            first: job.first,
        });
        let level_before = self.level();
        self.award_xp(level_before, xp);
    }

    pub fn sell_bouquets(&mut self, recipe: usize, count: u32) -> Result<Coins, GameError> {
        let def = self
            .data
            .recipes
            .get(recipe)
            .ok_or(GameError::UnknownRecipe)?
            .clone();
        let have = self.bouquet_count(recipe);
        if count == 0 {
            return Err(GameError::NothingToSell);
        }
        if count > have {
            return Err(GameError::NoBouquets { have });
        }
        if have == count {
            self.state.bouquets.remove(&def.id);
        } else {
            self.state.bouquets.insert(def.id.clone(), have - count);
        }
        let coins = self.bouquet_price(recipe) * count as u64;
        self.earn(coins);
        self.state.stats.bouquets_sold += count as u64;
        self.events.push(GameEvent::BouquetsSold {
            recipe,
            count,
            coins,
        });
        self.dirty = true;
        Ok(coins)
    }
}
