//! Bouquet crafting: discovery, planning, timed batches, keepsakes, sales,
//! saving mid-batch, and a full-world check that every required bouquet can
//! really be made.

use std::collections::BTreeSet;
use std::sync::Arc;

use bloom_core::bitset::BitSet;
use bloom_core::game::{GameError, GameEvent, GameState, RecipeLock};
use bloom_core::{Game, GameData, RecipeStatus};
use glam::Vec3;

fn data() -> Arc<GameData> {
    Arc::new(GameData::from_ron_str(include_str!("fixtures/valley.ron")).unwrap())
}

fn recipe(d: &GameData, id: &str) -> usize {
    d.recipe_index(id).unwrap()
}

fn none() -> BTreeSet<usize> {
    BTreeSet::new()
}

/// Lots of coins, a high level, the given tools.
fn rich(tools: &[&str]) -> Game {
    let d = data();
    let xp = d.progression.level_curve.total_xp_for_level(25);
    let mut g = Game::from_state(
        d.clone(),
        GameState {
            coins: 100_000,
            xp,
            ..Default::default()
        },
    )
    .unwrap();
    for t in tools {
        g.buy_tool(d.tool_index(t).unwrap())
            .unwrap_or_else(|e| panic!("{t}: {e}"));
    }
    g
}

/// Pick every flower of the first `regions` regions, in order.
fn harvest_regions(g: &mut Game, regions: usize) {
    for r in 0..regions {
        let field = g.region(r).field.clone();
        for i in 0..field.len() as u32 {
            let (c, _) = field.pick_sphere(i);
            g.tick(1.0);
            g.harvest(r, i, c + Vec3::Y)
                .unwrap_or_else(|e| panic!("region {r} flower {i}: {e}"));
        }
    }
    // Finish any bouquet still binding from earlier steps.
    g.tick(1_000.0);
}

fn basket(g: &Game) -> u32 {
    g.state().inventory.values().sum()
}

#[test]
fn recipes_are_discovered_with_tools_species_and_level() {
    let d = data();
    let simple = recipe(&d, "simple_bouquet");
    let colorful = recipe(&d, "colorful_bouquet");
    let romantic = recipe(&d, "romantic_bouquet");

    let mut g = rich(&[]);
    assert_eq!(
        g.recipe_status(simple),
        RecipeStatus::Locked(vec![RecipeLock::Tool("binding_ribbon".into())])
    );
    g.buy_tool(d.tool_index("binding_ribbon").unwrap()).unwrap();
    assert!(g.is_recipe_discovered(simple));
    assert!(g
        .drain_events()
        .contains(&GameEvent::RecipeDiscovered { recipe: simple }));
    assert!(!g.is_recipe_discovered(colorful));

    g.buy_tool(d.tool_index("florist_knife").unwrap()).unwrap();
    // Romantic also needs roses to have been found.
    assert_eq!(
        g.recipe_status(romantic),
        RecipeStatus::Locked(vec![RecipeLock::Species("rose".into())])
    );

    // Discovery survives a save.
    let restored = Game::from_state(d.clone(), g.state().clone()).unwrap();
    assert!(restored.is_recipe_discovered(simple));
}

#[test]
fn a_batch_consumes_exactly_its_flowers_then_binds_over_time() {
    let d = data();
    let simple = recipe(&d, "simple_bouquet");
    let mut g = rich(&["binding_ribbon"]);
    harvest_regions(&mut g, 1);
    let before = basket(&g);
    let xp_before = g.state().xp;

    let plan = g.craft(simple, 2, &none()).unwrap();
    assert_eq!(plan.ingredients.iter().map(|(_, n)| n).sum::<u32>(), 6);
    assert_eq!(basket(&g), before - 6);
    assert_eq!(g.crafted_count(simple), 2);
    assert_eq!(g.bouquet_count(simple), 0, "still binding");
    let job = g.craft_job().unwrap().clone();
    assert!((job.total_secs - d.recipes[simple].craft_time_secs * 2.0).abs() < 1e-5);
    assert!(job.first);

    g.tick(job.total_secs * 0.5);
    assert!((g.craft_job().unwrap().progress() - 0.5).abs() < 1e-3);
    g.tick(job.total_secs);
    assert!(g.craft_job().is_none());
    assert_eq!(g.bouquet_count(simple), 2);
    let tier_xp = d.progression.craft_xp(d.recipes[simple].tier) * 2;
    assert_eq!(
        g.state().xp - xp_before,
        tier_xp + d.progression.xp_first_craft
    );
    let events = g.drain_events();
    assert!(events.contains(&GameEvent::BouquetsReady {
        recipe: simple,
        count: 2,
        first: true
    }));

    // Second batch: no first-craft bonus.
    g.craft(simple, 1, &none()).unwrap();
    assert!(!g.craft_job().unwrap().first);
}

#[test]
fn failed_crafts_change_nothing() {
    let d = data();
    let simple = recipe(&d, "simple_bouquet");
    let colorful = recipe(&d, "colorful_bouquet");

    let mut g = rich(&["binding_ribbon"]);
    assert!(matches!(
        g.craft(simple, 1, &none()),
        Err(GameError::MissingFlowers { .. })
    ));
    harvest_regions(&mut g, 1);
    let state = g.state().clone();
    assert_eq!(
        g.craft(simple, 6, &none()),
        Err(GameError::BatchTooLarge { max: 5 })
    );
    assert_eq!(
        g.craft(simple, 0, &none()),
        Err(GameError::BatchTooLarge { max: 5 })
    );
    assert!(matches!(
        g.craft(colorful, 1, &none()),
        Err(GameError::RecipeLocked(_))
    ));
    assert_eq!(g.craft(99, 1, &none()), Err(GameError::UnknownRecipe));
    assert_eq!(g.state(), &state, "nothing consumed by failed crafts");

    g.craft(simple, 1, &none()).unwrap();
    let busy = g.state().clone();
    assert_eq!(g.craft(simple, 1, &none()), Err(GameError::Busy));
    assert_eq!(g.state(), &busy);
}

#[test]
fn colorful_uses_three_species_and_respects_exclusions() {
    let d = data();
    let colorful = recipe(&d, "colorful_bouquet");
    let mut g = rich(&["binding_ribbon", "garden_scissors"]);
    harvest_regions(&mut g, 1);
    let plan = g.plan_craft(colorful, 1, &none()).unwrap();
    assert!(plan.ingredients.len() >= 3, "{plan:?}");
    assert_eq!(plan.ingredients.iter().map(|(_, n)| n).sum::<u32>(), 5);

    let keep: BTreeSet<usize> = [plan.ingredients[0].0].into();
    let plan2 = g.plan_craft(colorful, 1, &keep).unwrap();
    assert!(plan2.ingredients.iter().all(|(s, _)| !keep.contains(s)));

    // Keep all but two species: three distinct species are impossible.
    let present: Vec<usize> = (0..d.species.len())
        .filter(|&s| g.inventory_count(s) > 0)
        .collect();
    let keep_most: BTreeSet<usize> = present[2..].iter().copied().collect();
    assert!(matches!(
        g.plan_craft(colorful, 1, &keep_most),
        Err(GameError::MissingFlowers { .. })
    ));
}

#[test]
fn named_species_are_used_and_the_workbench_raises_capacity() {
    let d = data();
    let romantic = recipe(&d, "romantic_bouquet");
    let rose = d.species_index("rose").unwrap();
    let mut g = rich(&["binding_ribbon", "florist_knife"]);
    harvest_regions(&mut g, 3);
    assert!(g.is_recipe_discovered(romantic));
    let plan = g.craft(romantic, 3, &none()).unwrap();
    let roses = plan
        .ingredients
        .iter()
        .find(|(s, _)| *s == rose)
        .map_or(0, |(_, n)| *n);
    assert!(roses >= 6, "{plan:?}");

    assert_eq!(g.batch_capacity(), 5);
    g.buy_tool(d.tool_index("florist_workbench").unwrap())
        .unwrap();
    assert_eq!(g.batch_capacity(), 25);
    g.buy_tool(d.tool_index("pro_florist_set").unwrap())
        .unwrap();
    assert_eq!(g.batch_capacity(), 50);
}

#[test]
fn keepsakes_for_required_bouquets_are_never_bound_into_others() {
    // Only three Starpetals left anywhere, all in the basket; the Royal
    // Bouquet (needs one) hasn't been made. A Simple Bouquet would need all
    // three, so it must be refused and nothing consumed.
    let d = data();
    let starpetal = d.species_index("starpetal").unwrap();
    let world = d.species_world_total("starpetal") as u32;
    let mut probe = Game::new(d.clone());
    let garden = d
        .regions
        .iter()
        .position(|r| r.id == "moonlight_garden")
        .unwrap();
    let field = probe.region(garden).field.clone();
    let mut bits = BitSet::new(d.regions[garden].flower_total);
    for i in 0..field.len() as u32 {
        if field.species(i) == starpetal {
            bits.test_and_set(i);
        }
    }
    let mut state = GameState {
        coins: 1_000,
        xp: 10_000,
        ..Default::default()
    };
    state.harvested.insert("moonlight_garden".into(), bits);
    state
        .harvested_per_species
        .insert("starpetal".into(), world);
    state.inventory.insert("starpetal".into(), 3);
    state.discovered.insert("starpetal".into());
    state.stats.flowers_harvested = world as u64;
    let mut g = Game::from_state(d.clone(), state).unwrap();
    g.buy_tool(d.tool_index("binding_ribbon").unwrap()).unwrap();
    let before = g.state().clone();
    let simple = recipe(&d, "simple_bouquet");
    assert!(matches!(
        g.craft(simple, 1, &none()),
        Err(GameError::MissingFlowers { .. })
    ));
    assert_eq!(g.state(), &before);
    assert_eq!(g.inventory_count(starpetal), 3);
}

#[test]
fn bouquets_sell_for_their_price() {
    let d = data();
    let simple = recipe(&d, "simple_bouquet");
    let mut g = rich(&["binding_ribbon"]);
    harvest_regions(&mut g, 1);
    g.craft(simple, 3, &none()).unwrap();
    g.tick(100.0);
    let coins = g.state().coins;
    assert_eq!(g.sell_bouquets(simple, 2), Err(GameError::NotAtWorkshop));
    g.set_at_workshop(true);
    assert_eq!(
        g.sell_bouquets(simple, 2),
        Ok(2 * d.recipes[simple].sell_price)
    );
    assert_eq!(g.state().coins, coins + 2 * d.recipes[simple].sell_price);
    assert_eq!(g.bouquet_count(simple), 1);
    assert_eq!(
        g.sell_bouquets(simple, 5),
        Err(GameError::NoBouquets { have: 1 })
    );
    assert_eq!(g.sell_bouquets(simple, 0), Err(GameError::NothingToSell));
    assert_eq!(g.state().stats.bouquets_sold, 2);
    // A bouquet always beats selling its flowers one by one.
    assert!(
        d.recipes[simple].sell_price
            > d.economy
                .raw_flower_value(d.recipes[simple].flower_count as u64)
    );
}

#[test]
fn a_batch_survives_saving_mid_way() {
    let d = data();
    let simple = recipe(&d, "simple_bouquet");
    let mut g = rich(&["binding_ribbon"]);
    harvest_regions(&mut g, 1);
    g.craft(simple, 4, &none()).unwrap();
    g.tick(0.3);
    let text = bloom_core::persistence::encode(g.state());
    let state = bloom_core::persistence::decode(&text).unwrap();
    let mut restored = Game::from_state(d.clone(), state).unwrap();
    assert!(restored.craft_job().is_some());
    restored.tick(100.0);
    assert_eq!(restored.bouquet_count(simple), 4);
}

#[test]
fn every_required_bouquet_can_be_made_from_the_whole_world() {
    let d = data();
    let tools: Vec<&str> = d.tools.iter().map(|t| t.id.as_str()).collect();
    let mut g = rich(&tools);
    harvest_regions(&mut g, d.regions.len());
    assert_eq!(g.state().stats.flowers_harvested, d.world_flower_total());
    // Rarest first, so plentiful recipes can't eat the rare species.
    let mut order: Vec<usize> = (0..d.recipes.len())
        .filter(|&r| d.recipes[r].required_for_completion)
        .collect();
    order.sort_by_key(|&r| std::cmp::Reverse(d.recipes[r].tier));
    for r in order {
        assert!(
            g.is_recipe_discovered(r),
            "{} not discovered",
            d.recipes[r].id
        );
        g.craft(r, 1, &none())
            .unwrap_or_else(|e| panic!("{}: {e}", d.recipes[r].id));
        g.tick(100.0);
        assert_eq!(g.bouquet_count(r), 1);
    }
}

#[test]
fn careless_crafting_order_still_leaves_every_required_bouquet_possible() {
    // Bind as many cheap bouquets as possible first (a careless player),
    // then the rest. The keepsake reserve must keep every required bouquet
    // possible.
    let d = data();
    let tools: Vec<&str> = d.tools.iter().map(|t| t.id.as_str()).collect();
    let mut g = rich(&tools);
    harvest_regions(&mut g, d.regions.len());
    let simple = recipe(&d, "simple_bouquet");
    while g
        .craft(simple, g.batch_capacity(), &none())
        .or_else(|_| g.craft(simple, 1, &none()))
        .is_ok()
    {
        g.tick(10_000.0);
    }
    let mut order: Vec<usize> = (0..d.recipes.len())
        .filter(|&r| d.recipes[r].required_for_completion)
        .collect();
    order.sort_by_key(|&r| d.recipes[r].tier);
    for r in order {
        if g.crafted_count(r) == 0 {
            g.craft(r, 1, &none())
                .unwrap_or_else(|e| panic!("{} after careless crafting: {e}", d.recipes[r].id));
            g.tick(100.0);
        }
        assert!(g.crafted_count(r) >= 1);
    }
}
