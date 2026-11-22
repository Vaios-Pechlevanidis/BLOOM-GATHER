//! Upgrades, the skill tree, the modifier pipeline and the progression guard.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use bloom_core::data::HarvestMode;
use bloom_core::game::{GameError, GameEvent, GameState, SkillLock, UpgradeLock};
use bloom_core::persistence::{decode, encode};
use bloom_core::{Game, GameData, Modifiers, SkillStatus, UpgradeStatus};
use glam::Vec3;

fn data() -> Arc<GameData> {
    Arc::new(GameData::builtin().unwrap())
}

fn levels(pairs: &[(&str, u32)]) -> BTreeMap<String, u32> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// A consistent game at `level` with `coins`, owning `tools`, with the given
/// upgrade and skill levels already in place.
fn game(
    d: Arc<GameData>,
    level: u32,
    coins: u64,
    tools: &[&str],
    upgrades: &[(&str, u32)],
    skills: &[(&str, u32)],
) -> Game {
    let xp = d.progression.level_curve.total_xp_for_level(level);
    Game::from_state(
        d,
        GameState {
            coins,
            xp,
            tools: tools.iter().map(|t| t.to_string()).collect(),
            upgrades: levels(upgrades),
            skills: levels(skills),
            ..Default::default()
        },
    )
    .unwrap()
}

fn upgrade(d: &GameData, id: &str) -> usize {
    d.upgrade_index(id).unwrap()
}

fn skill(d: &GameData, id: &str) -> usize {
    d.skill_index(id).unwrap()
}

/// Pick Sunny Meadow flowers by hand until `n` have been harvested in all
/// (in index order, skipping any a Double Harvest already took). Returns how
/// many picks were made.
fn pick(g: &mut Game, n: usize) -> usize {
    pick_in(g, 0, n)
}

fn pick_in(g: &mut Game, region: usize, n: usize) -> usize {
    let field = g.region(region).field.clone();
    let mut picks = 0;
    for i in 0..field.len() as u32 {
        if g.state().stats.flowers_harvested as usize >= n {
            break;
        }
        if g.is_harvested(region, i) {
            continue;
        }
        let (c, _) = field.pick_sphere(i);
        g.tick(1.0);
        g.harvest(region, i, c + Vec3::Y).unwrap();
        picks += 1;
    }
    picks
}

#[test]
fn upgrade_levels_cost_more_each_time_and_are_capped() {
    let d = data();
    let mut g = game(d.clone(), 1, 100_000, &[], &[], &[]);
    let u = upgrade(&d, "nimble_hands");
    let def = &d.upgrades[u];
    let mut paid = Vec::new();
    for level in 1..=def.max_level {
        let price = g.upgrade_price(u).unwrap();
        let before = g.state().coins;
        assert_eq!(g.buy_upgrade(u), Ok(level));
        assert_eq!(before - g.state().coins, price);
        assert!(g
            .drain_events()
            .contains(&GameEvent::UpgradeBought { upgrade: u, level }));
        paid.push(price);
    }
    assert_eq!(paid[0], def.base_price);
    assert!(paid.windows(2).all(|w| w[1] > w[0]), "{paid:?}");
    assert_eq!(g.upgrade_status(u), UpgradeStatus::Maxed);
    let coins = g.state().coins;
    assert_eq!(g.buy_upgrade(u), Err(GameError::MaxLevel));
    assert_eq!(g.state().coins, coins);
}

#[test]
fn upgrades_need_their_tool_and_the_coins() {
    let d = data();
    let u = upgrade(&d, "wider_reach");
    let mut g = game(d.clone(), 10, 100_000, &[], &[], &[]);
    assert_eq!(
        g.upgrade_status(u),
        UpgradeStatus::Locked(UpgradeLock::NeedsTool("harvesting_basket".into()))
    );
    assert!(matches!(g.buy_upgrade(u), Err(GameError::NeedsTool { .. })));
    assert_eq!(g.upgrade_level(u), 0);

    let mut poor = game(d.clone(), 10, 10, &["harvesting_basket"], &[], &[]);
    assert!(matches!(
        poor.buy_upgrade(u),
        Err(GameError::NotEnoughCoins { .. })
    ));
    assert_eq!(poor.state().coins, 10);
    assert_eq!(poor.upgrade_level(u), 0);
}

#[test]
fn the_guard_keeps_coins_needed_for_required_tools() {
    // Tight content: required tools cost exactly the guaranteed income, so
    // every coin earned by selling flowers is already spoken for.
    let mut tight = GameData::builtin().unwrap();
    let total = tight.required_tool_cost();
    let slack = tight.guaranteed_income() - total;
    let last = tight.tools.len() - 1;
    tight.tools[last].price += slack;
    let tight = Arc::new(tight.reindexed_and_validated().unwrap());

    let mut g = Game::new(tight.clone());
    pick(&mut g, 12);
    let (_, coins) = g.sell_all();
    assert!(coins >= 80);
    let u = upgrade(&tight, "nimble_hands");
    assert_eq!(g.optional_spending_limit(), 0);
    assert_eq!(
        g.upgrade_status(u),
        UpgradeStatus::Locked(UpgradeLock::SavingForTools {
            price: 80,
            can_spend: 0
        })
    );
    assert_eq!(
        g.buy_upgrade(u),
        Err(GameError::SavingForTools { can_spend: 0 })
    );
    assert_eq!(g.state().coins, coins);
    // Required tools themselves are never held back.
    let ribbon = tight.tool_index("binding_ribbon").unwrap();
    g.tick(1.0);
    let xp = tight.progression.level_curve.total_xp_for_level(2);
    assert!(g.state().xp >= xp, "level 2 from 12 picks");
    assert!(g.buy_tool(ribbon).is_ok());

    // With the shipped content the same player may buy it.
    let d = data();
    let mut g = Game::new(d.clone());
    pick(&mut g, 12);
    g.sell_all();
    assert!(g.optional_spending_limit() >= 80);
    assert_eq!(g.buy_upgrade(upgrade(&d, "nimble_hands")), Ok(1));
}

#[test]
fn the_guard_counts_bouquets_and_region_rewards_as_income() {
    let d = data();
    let g = Game::new(d.clone());
    let start = g.guaranteed_income();
    assert_eq!(start, d.guaranteed_income());
    assert_eq!(g.required_tools_left_cost(), d.required_tool_cost());
    // Owning bouquets adds their (bonus-inclusive) value.
    let mut state = g.state().clone();
    state.bouquets.insert("simple_bouquet".into(), 2);
    let g = Game::from_state(d.clone(), state).unwrap();
    assert_eq!(g.guaranteed_income(), start + 2 * g.bouquet_price(0));
}

#[test]
fn skill_points_come_from_levels_regions_and_milestones() {
    let d = data();
    let g = Game::new(d.clone());
    assert_eq!(g.skill_points(), 0);

    let g = game(d.clone(), 6, 0, &[], &[], &[]);
    assert_eq!(g.skill_points_earned(), 5);

    // Eight discovered species is a collection milestone.
    let mut state = g.state().clone();
    state.discovered = d.species.iter().take(8).map(|s| s.id.clone()).collect();
    let g = Game::from_state(d.clone(), state).unwrap();
    assert_eq!(g.skill_points_earned(), 6);
}

#[test]
fn skills_need_points_their_prerequisite_and_tools() {
    let d = data();
    let mut g = game(d.clone(), 3, 0, &[], &[], &[]);
    assert_eq!(g.skill_points(), 2);

    let quick = skill(&d, "quick_fingers");
    let double = skill(&d, "double_harvest");
    assert_eq!(
        g.skill_status(double),
        SkillStatus::Locked(SkillLock::NeedsSkill("quick_fingers".into()))
    );
    assert!(matches!(
        g.learn_skill(double),
        Err(GameError::NeedsSkill { .. })
    ));
    assert_eq!(g.learn_skill(quick), Ok(1));
    assert_eq!(g.learn_skill(double), Ok(1));
    assert_eq!(g.skill_points(), 0);
    assert_eq!(
        g.learn_skill(quick),
        Err(GameError::NotEnoughSkillPoints { needed: 1, have: 0 })
    );
    assert_eq!(g.skill_level(quick), 1);

    // Improved Radius builds on the basket.
    let mut g = game(
        d.clone(),
        10,
        0,
        &[],
        &[],
        &[("quick_fingers", 1), ("double_harvest", 1)],
    );
    let radius = skill(&d, "improved_radius");
    assert_eq!(
        g.skill_status(radius),
        SkillStatus::Locked(SkillLock::NeedsTool("harvesting_basket".into()))
    );
    assert!(matches!(
        g.learn_skill(radius),
        Err(GameError::NeedsTool { .. })
    ));

    // Maxed skills stay maxed.
    let mut g = game(d.clone(), 10, 0, &[], &[], &[("new_meadows", 1)]);
    assert_eq!(
        g.learn_skill(skill(&d, "new_meadows")),
        Err(GameError::MaxLevel)
    );
}

#[test]
fn collection_bonuses_pay_for_fully_discovered_regions() {
    let d = data();
    let meadow: BTreeSet<String> = d.regions[0]
        .flowers
        .iter()
        .map(|f| f.species.clone())
        .collect();
    let skills = [
        ("botanical_knowledge", 1),
        ("flower_recognition", 1),
        ("rare_flower_detection", 1),
    ];
    let mut g = game(d.clone(), 10, 0, &[], &[], &skills);
    let mut state = g.state().clone();
    state.discovered = meadow;
    g = Game::from_state(d.clone(), state).unwrap();
    let before = g.skill_points_earned();
    g.learn_skill(skill(&d, "collection_bonuses")).unwrap();
    assert_eq!(g.fully_discovered_regions(), 1);
    assert_eq!(g.skill_points_earned(), before + 1);
}

#[test]
fn modifiers_add_up_and_respect_their_floors() {
    let d = data();
    let plain = game(d.clone(), 20, 0, &["harvesting_basket"], &[], &[]);
    let g = game(
        d.clone(),
        20,
        0,
        &["harvesting_basket"],
        &[
            ("nimble_hands", 2),
            ("wider_reach", 3),
            ("deep_basket", 5),
            ("garden_boots", 2),
        ],
        &[("quick_fingers", 1)],
    );
    let ratio = g.harvest_cooldown_secs() / plain.harvest_cooldown_secs();
    assert!((ratio - (1.0 - 0.12 - 0.05)).abs() < 1e-5, "{ratio}");
    let (a, b) = (
        plain.mode_params(HarvestMode::Radius).unwrap(),
        g.mode_params(HarvestMode::Radius).unwrap(),
    );
    assert!((b.radius / a.radius - 1.3).abs() < 1e-5);
    assert_eq!(b.max_flowers, a.max_flowers * 2);
    assert!((g.move_speed_multiplier() - 1.08).abs() < 1e-6);
    assert_eq!(Modifiers::time_factor(0.9), 0.25);
    assert_eq!(Modifiers::price_factor(0.9), 0.5);

    // Long Reach extends picking reach.
    let far = game(d.clone(), 20, 0, &[], &[], &[("long_reach", 2)]);
    assert!((far.reach() - plain.reach() - 0.6).abs() < 1e-5);
}

#[test]
fn bouquet_bonuses_never_touch_flower_prices() {
    let d = data();
    let simple = d.recipe_index("simple_bouquet").unwrap();
    let wrapped = d.recipe_index("wrapped_garden_bouquet").unwrap();
    let g = game(
        d.clone(),
        20,
        0,
        &[],
        &[("market_stall", 5), ("pretty_packaging", 3)],
        &[],
    );
    assert_eq!(g.bouquet_price(simple), (45.0f64 * 1.2).round() as u64);
    assert_eq!(g.bouquet_price(wrapped), (190.0f64 * 1.38).round() as u64);
    assert_eq!(g.data().flower_value(), 10);
    for r in 0..d.recipes.len() {
        assert!(g.bouquet_price(r) >= d.recipes[r].sell_price);
    }
}

#[test]
fn discounts_lower_what_tools_and_upgrades_cost() {
    let d = data();
    let ribbon = d.tool_index("binding_ribbon").unwrap();
    let mut g = game(
        d.clone(),
        5,
        1_000,
        &[],
        &[("haggler", 5)],
        &[("better_deals", 2), ("thrifty_upgrades", 3)],
    );
    let price = g.tool_price(ribbon);
    assert_eq!(price, (100.0f64 * (1.0 - 0.15 - 0.08)).round() as u64);
    assert_eq!(g.buy_tool(ribbon), Ok(price));
    assert_eq!(g.state().coins, 1_000 - price);
    let nimble = upgrade(&d, "nimble_hands");
    assert_eq!(
        g.upgrade_price(nimble),
        Some((80.0f64 * 0.88).round() as u64)
    );
}

#[test]
fn crafting_upgrades_change_capacity_time_and_xp() {
    let d = data();
    let plain = game(d.clone(), 20, 0, &["binding_ribbon"], &[], &[]);
    let g = game(
        d.clone(),
        20,
        0,
        &["binding_ribbon"],
        &[("bulk_orders", 2), ("quick_binding", 5)],
        &[("basic_florist", 1), ("efficient_binding", 2)],
    );
    assert_eq!(g.batch_capacity(), plain.batch_capacity() + 10);
    let mut g = g;
    pick(&mut g, 30);
    let plan = g.plan_craft(0, 4, &BTreeSet::new()).unwrap();
    let base = d.recipes[0].craft_time_secs * 4.0;
    assert!((plan.secs - base * (1.0 - 0.4 - 0.2)).abs() < 1e-5);
}

#[test]
fn double_harvest_is_deterministic_and_counts_every_flower() {
    let d = data();
    let skills = [("quick_fingers", 1), ("double_harvest", 3)];
    let mut g = game(d.clone(), 10, 0, &[], &[], &skills);
    let snapshot = g.clone();
    let picks = pick(&mut g, 60);
    let harvested = g.state().stats.flowers_harvested as usize;
    assert!(harvested > picks, "some picks took a neighbour too");
    let basket: u32 = g.state().inventory.values().sum();
    assert_eq!(basket as usize, harvested);

    // The RNG is part of the save: replaying gives the same result.
    let mut again = snapshot;
    pick(&mut again, 60);
    assert_eq!(again.state().harvested, g.state().harvested);
}

#[test]
fn efficient_production_only_returns_flowers_it_used() {
    let d = data();
    let skills = [
        ("better_deals", 1),
        ("thrifty_upgrades", 1),
        ("improved_profits", 1),
        ("efficient_production", 2),
    ];
    let mut g = game(d.clone(), 20, 0, &["binding_ribbon"], &[], &skills);
    pick_in(&mut g, 0, 100);
    pick_in(&mut g, 1, 300);
    let held = |g: &Game| g.state().inventory.values().sum::<u32>();
    let start = held(&g);
    assert_eq!(start, 300);
    let (mut saved, mut used, mut bouquets) = (0, 0, 0);
    // Bind Simple Bouquets until the basket runs low (or only keepsakes are left).
    while let Ok(plan) = g.craft(0, 5, &BTreeSet::new()) {
        used += plan.ingredients.iter().map(|(_, n)| n).sum::<u32>();
        bouquets += 5;
        for e in g.drain_events() {
            if let GameEvent::FlowersSaved { species, count } = e {
                assert!(count <= 5);
                assert!(plan.ingredients.iter().any(|&(s, _)| s == species));
                saved += count;
            }
        }
        g.tick(100.0);
    }
    assert!(bouquets >= 80, "{bouquets}");
    assert!(saved > 0);
    assert_eq!(held(&g), start - used + saved);
}

#[test]
fn new_meadows_and_unlock_bonuses_open_regions_early() {
    let d = data();
    let needed = d.regions[1].unlock.flowers_harvested;
    let chain = [
        ("new_meadows", 1),
        ("trailblazer", 1),
        ("long_reach", 1),
        ("region_unlock_bonuses", 1),
    ];
    let mut g = game(d.clone(), 8, 0, &[], &[], &chain);
    let early = g.region_harvest_threshold(1);
    assert_eq!(early, (needed as f64 * 0.9).ceil() as u32);
    assert!(!g.is_region_unlocked(1));
    pick(&mut g, early as usize - 1);
    g.drain_events();
    assert!(!g.is_region_unlocked(1));
    let coins = g.state().coins;
    pick(&mut g, early as usize);
    assert!(g.is_region_unlocked(1));
    assert!(g.drain_events().contains(&GameEvent::RegionUnlocked {
        region: 1,
        coins: 50,
        xp: 50
    }));
    assert_eq!(g.state().coins, coins + 50);
    assert!(g.state().unlocked_regions.contains(&d.regions[1].id));

    // Without the skill, the same harvest isn't enough.
    let mut plain = game(d.clone(), 8, 0, &[], &[], &[]);
    pick(&mut plain, early as usize);
    assert!(!plain.is_region_unlocked(1));
    // The starting region is open from the start, with no announcement.
    let fresh = Game::new(d.clone());
    assert!(fresh.state().unlocked_regions.contains(&d.regions[0].id));
}

#[test]
fn upgrades_skills_and_rng_survive_a_save() {
    let d = data();
    let mut g = game(d.clone(), 6, 5_000, &[], &[], &[]);
    g.buy_upgrade(upgrade(&d, "garden_boots")).unwrap();
    g.learn_skill(skill(&d, "quick_fingers")).unwrap();
    g.learn_skill(skill(&d, "double_harvest")).unwrap();
    pick(&mut g, 10);
    let text = encode(g.state());
    let restored = Game::from_state(d.clone(), decode(&text).unwrap()).unwrap();
    assert_eq!(restored.state(), g.state());
    assert_eq!(restored.skill_points(), g.skill_points());
    assert_eq!(restored.modifiers(), g.modifiers());
}

#[test]
fn bad_upgrade_or_skill_records_are_rejected() {
    let d = data();
    let unknown = GameState {
        upgrades: levels(&[("golden_shovel", 1)]),
        ..Default::default()
    };
    assert!(Game::from_state(d.clone(), unknown).is_err());
    let too_high = GameState {
        skills: levels(&[("new_meadows", 2)]),
        ..Default::default()
    };
    assert!(Game::from_state(d.clone(), too_high).is_err());
}

#[test]
fn content_rules_for_growth_hold() {
    let d = data();
    assert_eq!(d.upgrades.len(), 10);
    assert_eq!(d.skills.len(), 21);
    // The tree costs more than a full playthrough earns: choices matter.
    let tree: u32 = d.skills.iter().map(|s| s.cost * s.max_level).sum();
    assert!(tree > 40, "{tree}");
    // A broken prerequisite is caught.
    let mut broken = GameData::builtin().unwrap();
    broken.skills[1].requires_skill = Some("no_such_skill".into());
    assert!(broken.reindexed_and_validated().is_err());
    // So is a reduction that could never take effect.
    let mut wasteful = GameData::builtin().unwrap();
    wasteful.upgrades[0].max_level = 20;
    assert!(wasteful.reindexed_and_validated().is_err());
}
