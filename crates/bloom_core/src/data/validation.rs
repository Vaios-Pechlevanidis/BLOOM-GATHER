//! Content validation: reference integrity plus the economic and progression
//! invariants from `docs/economy-balancing.md` §2.
//!
//! Validation collects *every* problem instead of stopping at the first, so a
//! designer editing the RON file sees the full picture in one run.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::{Effect, GameData, ToolEffect};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError(pub String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

struct Report(Vec<ValidationError>);

impl Report {
    fn check(&mut self, ok: bool, msg: impl FnOnce() -> String) {
        if !ok {
            self.0.push(ValidationError(msg()));
        }
    }
}

impl GameData {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut r = Report(Vec::new());
        self.validate_config(&mut r);
        self.validate_ids(&mut r);
        self.validate_species(&mut r);
        self.validate_regions(&mut r);
        self.validate_tools(&mut r);
        self.validate_recipes(&mut r);
        self.validate_global_balance(&mut r);
        self.validate_growth(&mut r);
        self.validate_achievements(&mut r);
        if r.0.is_empty() {
            Ok(())
        } else {
            Err(r.0)
        }
    }

    fn validate_config(&self, r: &mut Report) {
        let e = &self.economy;
        r.check(e.flower_base_value > 0, || {
            "economy.flower_base_value must be > 0".into()
        });
        r.check(e.base_craft_batch_capacity > 0, || {
            "economy.base_craft_batch_capacity must be > 0".into()
        });
        r.check(e.upgrade_cost_growth > 1.0, || {
            "economy.upgrade_cost_growth must be > 1.0".into()
        });

        let p = &self.progression;
        r.check(p.xp_per_harvest > 0, || {
            "progression.xp_per_harvest must be > 0 (levels must be reachable by harvesting)".into()
        });
        r.check(p.level_curve.max_level >= 2, || {
            "level_curve.max_level must be >= 2".into()
        });
        r.check(p.level_curve.base > 0, || {
            "level_curve.base must be > 0".into()
        });

        let h = &self.harvesting;
        r.check(
            h.hand_cooldown_secs > 0.0
                && h.radius_cooldown_secs > 0.0
                && h.sweep_interval_secs > 0.0
                && h.area_cooldown_secs > 0.0
                && h.ground_reach_bonus >= 0.0,
            || "harvesting cooldowns must be > 0 and ground_reach_bonus >= 0".into(),
        );
        r.check(self.world.chunk_size > 0.0, || {
            "world.chunk_size must be > 0".into()
        });
        r.check(self.world.min_flower_spacing > 0.0, || {
            "world.min_flower_spacing must be > 0".into()
        });
        r.check(self.world.terrain_cell > 0.0, || {
            "world.terrain_cell must be > 0".into()
        });
        r.check((0.5..=5.0).contains(&self.world.flower_scale), || {
            "world.flower_scale must be within 0.5..=5".into()
        });
        r.check(self.world.boundary_margin >= 0.0, || {
            "world.boundary_margin must be >= 0".into()
        });

        let pl = &self.player;
        r.check(pl.radius > 0.0 && pl.height > 2.0 * pl.radius, || {
            "player.height must exceed its diameter".into()
        });
        r.check(pl.eye_height > 0.0 && pl.eye_height <= pl.height, || {
            "player.eye_height must be within the body".into()
        });
        r.check(
            pl.walk_speed > 0.0 && pl.sprint_speed >= pl.walk_speed,
            || "player.sprint_speed must be >= walk_speed > 0".into(),
        );
        r.check(
            pl.ground_accel > 0.0 && pl.ground_decel > 0.0 && pl.air_accel >= 0.0,
            || "player accelerations must be positive".into(),
        );
        r.check(pl.gravity > 0.0 && pl.jump_height >= 0.0, || {
            "player.gravity must be > 0 and jump_height >= 0".into()
        });
        r.check(pl.max_slope_deg > 0.0 && pl.max_slope_deg < 90.0, || {
            "player.max_slope_deg must be in (0, 90)".into()
        });
        r.check(pl.step_height >= 0.0 && pl.step_height < pl.height, || {
            "player.step_height must be below the body height".into()
        });
        r.check(pl.interact_range > 0.0, || {
            "player.interact_range must be > 0".into()
        });
    }

    fn validate_ids(&self, r: &mut Report) {
        fn unique<'a>(r: &mut Report, kind: &str, ids: impl Iterator<Item = &'a str>) {
            let mut seen = HashSet::new();
            for id in ids {
                r.check(!id.is_empty(), || format!("{kind} has an empty id"));
                r.check(seen.insert(id), || format!("duplicate {kind} id '{id}'"));
            }
        }
        unique(r, "species", self.species.iter().map(|s| s.id.as_str()));
        unique(r, "tool", self.tools.iter().map(|t| t.id.as_str()));
        unique(r, "recipe", self.recipes.iter().map(|x| x.id.as_str()));
        unique(r, "region", self.regions.iter().map(|x| x.id.as_str()));
        unique(r, "upgrade", self.upgrades.iter().map(|x| x.id.as_str()));
        unique(r, "skill", self.skills.iter().map(|x| x.id.as_str()));
        unique(
            r,
            "achievement",
            self.achievements.iter().map(|x| x.id.as_str()),
        );
    }

    fn validate_species(&self, r: &mut Report) {
        r.check(!self.species.is_empty(), || {
            "no flower species defined".into()
        });
        for s in &self.species {
            r.check(!s.name.trim().is_empty(), || {
                format!("species '{}' has no name", s.id)
            });
            r.check(s.height > 0.05 && s.height < 3.0, || {
                format!(
                    "species '{}' has an implausible height {} m",
                    s.id, s.height
                )
            });
            r.check((0.0..=1.0).contains(&s.glow), || {
                format!("species '{}' glow must be within 0..=1", s.id)
            });
            // A species nobody can ever find would make the collection
            // impossible to complete.
            r.check(self.species_world_total(&s.id) > 0, || {
                format!("species '{}' does not appear in any region", s.id)
            });
        }
    }

    fn validate_regions(&self, r: &mut Report) {
        r.check(!self.regions.is_empty(), || "no regions defined".into());
        let curve = &self.progression.level_curve;
        let mut flowers_before: u64 = 0;

        for (i, region) in self.regions.iter().enumerate() {
            let id = &region.id;
            let counted = region.counted_flowers();
            r.check(counted == region.flower_total, || {
                format!(
                    "region '{id}': species counts sum to {counted}, but flower_total is {}",
                    region.flower_total
                )
            });
            r.check(region.width > 0.0 && region.depth > 0.0, || {
                format!("region '{id}' must have a positive size")
            });
            let margin = self.world.boundary_margin;
            r.check(
                region.width > 4.0 * margin && region.depth > 4.0 * margin,
                || format!("region '{id}' is too small for its {margin} m boundary margin"),
            );
            let t = region.terrain;
            r.check(
                t.amplitude >= 0.0
                    && t.feature_size > 0.0
                    && t.octaves >= 1
                    && t.border_width > 0.0,
                || format!("region '{id}' has invalid terrain parameters"),
            );

            let mut seen = HashSet::new();
            for f in &region.flowers {
                r.check(self.species(&f.species).is_some(), || {
                    format!("region '{id}' references unknown species '{}'", f.species)
                });
                r.check(f.count > 0, || {
                    format!("region '{id}' lists '{}' with count 0", f.species)
                });
                r.check(seen.insert(f.species.as_str()), || {
                    format!("region '{id}' lists species '{}' twice", f.species)
                });
            }

            // Generation must be able to place every flower with the minimum
            // spacing. Each flower needs roughly a spacing² cell; demand 2×
            // headroom so jittered placement never runs out of room.
            let spacing = self.world.min_flower_spacing;
            let needed_area = counted as f32 * spacing * spacing * 2.0;
            r.check(region.width * region.depth >= needed_area, || {
                format!(
                    "region '{id}' is too small to place {counted} flowers {spacing} units apart"
                )
            });

            // Reachability without spending anything: the harvest threshold
            // must be satisfiable from earlier regions, and harvest XP alone
            // from that many flowers must reach the level requirement.
            let unlock = region.unlock;
            if i == 0 {
                r.check(unlock.level <= 1 && unlock.flowers_harvested == 0, || {
                    format!("first region '{id}' must be unlocked from the start")
                });
            } else {
                r.check(unlock.flowers_harvested as u64 <= flowers_before, || {
                    format!(
                        "region '{id}' needs {} harvested flowers but earlier regions only hold {flowers_before}",
                        unlock.flowers_harvested
                    )
                });
                let xp = unlock.flowers_harvested as u64 * self.progression.xp_per_harvest;
                r.check(curve.level_for_xp(xp) >= unlock.level, || {
                    format!(
                        "region '{id}' needs level {} but harvesting {} flowers only gives {xp} XP (level {})",
                        unlock.level,
                        unlock.flowers_harvested,
                        curve.level_for_xp(xp)
                    )
                });
            }
            flowers_before += counted as u64;
        }
    }

    fn validate_tools(&self, r: &mut Report) {
        let curve = &self.progression.level_curve;
        let harvest_only_level =
            curve.level_for_xp(self.world_flower_total() * self.progression.xp_per_harvest);

        for tool in &self.tools {
            let id = &tool.id;
            r.check(tool.price > 0, || {
                format!("tool '{id}' must have a price > 0")
            });
            r.check(tool.unlock_level <= harvest_only_level, || {
                format!(
                    "tool '{id}' needs level {} but harvesting the whole world only reaches level {harvest_only_level}",
                    tool.unlock_level
                )
            });
            for req in &tool.requires_tools {
                match self.tool(req) {
                    None => r.check(false, || {
                        format!("tool '{id}' requires unknown tool '{req}'")
                    }),
                    Some(dep) => {
                        r.check(dep.id != *id, || format!("tool '{id}' requires itself"));
                        // Ordering by level rules out cycles and makes sure the
                        // prerequisite can be bought first.
                        r.check(dep.unlock_level <= tool.unlock_level, || {
                            format!(
                                "tool '{id}' (level {}) requires '{req}' which unlocks later (level {})",
                                tool.unlock_level, dep.unlock_level
                            )
                        });
                        r.check(
                            !tool.required_for_completion || dep.required_for_completion,
                            || format!("required tool '{id}' depends on optional tool '{req}'"),
                        );
                    }
                }
            }
            for effect in &tool.effects {
                let ok = match *effect {
                    ToolEffect::HarvestCooldownMultiplier(m) => m > 0.0 && m <= 1.0,
                    ToolEffect::RadiusMode {
                        radius,
                        max_flowers,
                    } => (0.3..=5.0).contains(&radius) && max_flowers > 0,
                    ToolEffect::SweepMode {
                        radius,
                        max_flowers_per_tick,
                    } => (0.3..=5.0).contains(&radius) && max_flowers_per_tick > 0,
                    ToolEffect::AreaMode {
                        max_width,
                        max_height,
                        max_flowers,
                    } => {
                        (1.0..=20.0).contains(&max_width)
                            && (1.0..=20.0).contains(&max_height)
                            && max_flowers > 0
                    }
                    ToolEffect::CraftBatchCapacity(c) => c > 0,
                };
                r.check(ok, || {
                    format!("tool '{id}' has an out-of-range effect {effect:?}")
                });
            }
        }
    }

    fn validate_recipes(&self, r: &mut Report) {
        let base = self.economy.flower_base_value;
        let species_in_world = self
            .species
            .iter()
            .filter(|s| self.species_world_total(&s.id) > 0)
            .count();

        for recipe in &self.recipes {
            let id = &recipe.id;
            r.check(recipe.tier >= 1, || {
                format!("recipe '{id}' tier must be >= 1")
            });
            r.check(recipe.flower_count > 0, || {
                format!("recipe '{id}' needs at least one flower")
            });
            r.check(recipe.craft_time_secs >= 0.0, || {
                format!("recipe '{id}' has a negative craft time")
            });
            r.check(!recipe.required_tools.is_empty(), || {
                format!("recipe '{id}' must require at least one tool")
            });

            // Invariant 2: a bouquet is always worth more than its flowers.
            let raw = self.economy.raw_flower_value(recipe.flower_count as u64);
            r.check(recipe.sell_price > raw, || {
                format!(
                    "recipe '{id}' sells for {} but its {} flowers are worth {raw} raw (base value {base})",
                    recipe.sell_price, recipe.flower_count
                )
            });

            let named: u32 = recipe.required_species.iter().map(|s| s.count).sum();
            r.check(named <= recipe.flower_count, || {
                format!(
                    "recipe '{id}' names {named} specific flowers but only uses {}",
                    recipe.flower_count
                )
            });
            r.check(recipe.min_distinct_species <= recipe.flower_count, || {
                format!("recipe '{id}' wants more distinct species than flowers")
            });
            r.check(
                recipe.min_distinct_species as usize <= species_in_world,
                || {
                    format!(
                        "recipe '{id}' wants {} distinct species but only {species_in_world} exist",
                        recipe.min_distinct_species
                    )
                },
            );

            let mut seen = HashSet::new();
            for req in &recipe.required_species {
                r.check(seen.insert(req.species.as_str()), || {
                    format!("recipe '{id}' names species '{}' twice", req.species)
                });
                r.check(req.count > 0, || {
                    format!("recipe '{id}' requires 0 of '{}'", req.species)
                });
                match self.species(&req.species) {
                    None => r.check(false, || {
                        format!("recipe '{id}' requires unknown species '{}'", req.species)
                    }),
                    Some(_) => {
                        let supply = self.species_world_total(&req.species);
                        r.check(supply >= req.count as u64, || {
                            format!(
                                "recipe '{id}' needs {} '{}' but only {supply} exist",
                                req.count, req.species
                            )
                        });
                    }
                }
            }

            for tool_id in &recipe.required_tools {
                match self.tool(tool_id) {
                    None => r.check(false, || {
                        format!("recipe '{id}' requires unknown tool '{tool_id}'")
                    }),
                    Some(tool) => r.check(
                        !recipe.required_for_completion || tool.required_for_completion,
                        || format!("required recipe '{id}' depends on optional tool '{tool_id}'"),
                    ),
                }
            }
        }
    }

    fn validate_global_balance(&self, r: &mut Report) {
        // Invariant 3: every required tool is affordable from guaranteed income.
        let required = self.required_tool_cost();
        let guaranteed = self.guaranteed_income();
        r.check(required <= guaranteed, || {
            format!("required tools cost {required} but guaranteed income is only {guaranteed}")
        });

        // Invariant 4 (joint form): all required recipes can be crafted once
        // *together* from the finite world supply.
        let mut named_demand: HashMap<&str, u64> = HashMap::new();
        let mut total_demand: u64 = 0;
        for recipe in self.recipes.iter().filter(|x| x.required_for_completion) {
            total_demand += recipe.flower_count as u64;
            for req in &recipe.required_species {
                *named_demand.entry(req.species.as_str()).or_default() += req.count as u64;
            }
        }
        r.check(total_demand <= self.world_flower_total(), || {
            format!(
                "required recipes need {total_demand} flowers but the world only has {}",
                self.world_flower_total()
            )
        });
        for (species, demand) in named_demand {
            let supply = self.species_world_total(species);
            r.check(demand <= supply, || {
                format!(
                    "required recipes jointly need {demand} '{species}' but only {supply} exist"
                )
            });
        }
    }

    /// Upgrades and the skill tree: references, sane per-level values, a
    /// chain-shaped tree without cycles, and no level that would be wasted
    /// against a floor.
    fn validate_growth(&self, r: &mut Report) {
        let effect_ok = |r: &mut Report, owner: &str, e: &Effect| {
            if let Some(x) = e.amount() {
                let max = if matches!(e, Effect::Reach(_)) {
                    2.0
                } else {
                    1.0
                };
                r.check(x > 0.0 && x <= max, || {
                    format!("{owner}: {} must be in (0, {max}] per level", e.kind())
                });
            }
            match e {
                Effect::BatchCapacity(n) => {
                    r.check(*n > 0, || format!("{owner}: batch capacity must be > 0"))
                }
                Effect::BouquetPriceWithTool { tool, .. } => r
                    .check(self.tool(tool).is_some(), || {
                        format!("{owner}: unknown tool '{tool}'")
                    }),
                Effect::RegionUnlockBonus { coins, xp } => r.check(*coins > 0 || *xp > 0, || {
                    format!("{owner}: region unlock bonus gives nothing")
                }),
                _ => {}
            }
        };

        for u in &self.upgrades {
            let id = format!("upgrade '{}'", u.id);
            r.check(!u.name.trim().is_empty(), || format!("{id} has no name"));
            r.check(u.base_price > 0, || format!("{id} must cost something"));
            r.check(u.max_level >= 1, || {
                format!("{id} needs at least one level")
            });
            r.check(!u.effects.is_empty(), || format!("{id} does nothing"));
            if let Some(t) = &u.requires_tool {
                r.check(self.tool(t).is_some(), || {
                    format!("{id} requires unknown tool '{t}'")
                });
            }
            for e in &u.effects {
                effect_ok(r, &id, e);
            }
        }

        for (i, s) in self.skills.iter().enumerate() {
            let id = format!("skill '{}'", s.id);
            r.check(!s.name.trim().is_empty(), || format!("{id} has no name"));
            r.check(s.max_level >= 1, || {
                format!("{id} needs at least one level")
            });
            r.check(s.cost >= 1, || {
                format!("{id} must cost at least one skill point")
            });
            r.check(!s.effects.is_empty(), || format!("{id} does nothing"));
            if let Some(t) = &s.requires_tool {
                r.check(self.tool(t).is_some(), || {
                    format!("{id} requires unknown tool '{t}'")
                });
            }
            if let Some(req) = &s.requires_skill {
                // Listed earlier in the same branch: no cycles, a readable tree.
                match self.skills.iter().position(|x| &x.id == req) {
                    None => r.check(false, || format!("{id} requires unknown skill '{req}'")),
                    Some(j) => {
                        r.check(j < i, || format!("{id} must come after '{req}'"));
                        r.check(self.skills[j].branch == s.branch, || {
                            format!("{id} requires '{req}' from another branch")
                        });
                    }
                }
            }
            for e in &s.effects {
                effect_ok(r, &id, e);
            }
        }

        // Every level of a reduction must still do something: the combined
        // maximum from all sources stays within its floor.
        let mut totals: HashMap<&str, (f32, f32)> = HashMap::new();
        let all = self
            .upgrades
            .iter()
            .map(|u| (u.max_level, &u.effects))
            .chain(self.skills.iter().map(|s| (s.max_level, &s.effects)));
        for (levels, effects) in all {
            for e in effects {
                if let Some((x, floor)) = e.reduction() {
                    let t = totals.entry(e.kind()).or_insert((0.0, floor));
                    t.0 += x * levels as f32;
                }
            }
        }
        for (kind, (total, floor)) in totals {
            r.check(total <= 1.0 - floor + 1e-4, || {
                format!(
                    "{kind} reductions add up to {:.0} % at most levels, beyond the {:.0} % floor",
                    total * 100.0,
                    (1.0 - floor) * 100.0
                )
            });
        }

        let m = &self.progression.collection_milestones;
        r.check(m.windows(2).all(|w| w[0] < w[1]), || {
            "collection_milestones must be increasing".into()
        });
        r.check(
            m.iter()
                .all(|&n| n >= 1 && n as usize <= self.species.len()),
            || "collection_milestones must be between 1 and the number of species".into(),
        );
    }

    /// Achievements: every target can actually be reached in this world.
    fn validate_achievements(&self, r: &mut Report) {
        use super::Condition as C;
        let world = self.world_flower_total();
        // The most bouquets the finite world could ever make.
        let smallest = self
            .recipes
            .iter()
            .map(|x| x.flower_count)
            .min()
            .unwrap_or(1)
            .max(1);
        let max_bouquets = world / smallest as u64;
        for a in &self.achievements {
            let id = format!("achievement '{}'", a.id);
            r.check(!a.name.trim().is_empty(), || format!("{id} has no name"));
            let (ok, why) = match a.condition {
                C::FlowersHarvested(n) => (n >= 1 && n <= world, format!("1..={world} flowers")),
                C::BouquetsCrafted(n) => (
                    n >= 1 && n <= max_bouquets,
                    format!("1..={max_bouquets} bouquets"),
                ),
                C::SpeciesDiscovered(n) => (
                    n >= 1 && n as usize <= self.species.len(),
                    format!("1..={} species", self.species.len()),
                ),
                C::RegionsCompleted(n) => (
                    n >= 1 && n as usize <= self.regions.len(),
                    format!("1..={} regions", self.regions.len()),
                ),
                _ => (true, String::new()),
            };
            r.check(ok, || format!("{id}: target must be {why}"));
        }
    }
}
