//! Achievements, the completion check and statistics.

use std::collections::BTreeSet;
use std::sync::Arc;

use bloom_core::data::{Condition, Rarity};
use bloom_core::game::{GameEvent, GameState};
use bloom_core::{Game, GameData};
use glam::Vec3;

fn data() -> Arc<GameData> {
    Arc::new(GameData::builtin().unwrap())
}

fn achievement(d: &GameData, id: &str) -> usize {
    d.achievement_index(id).unwrap()
}

fn unlocked_events(g: &mut Game) -> Vec<usize> {
    g.drain_events()
        .into_iter()
        .filter_map(|e| match e {
            GameEvent::AchievementUnlocked { achievement } => Some(achievement),
            _ => None,
        })
        .collect()
}

fn rich(d: Arc<GameData>, level: u32) -> Game {
    let xp = d.progression.level_curve.total_xp_for_level(level);
    Game::from_state(
        d,
        GameState {
            coins: 100_000,
            xp,
            ..Default::default()
        },
    )
    .unwrap()
}

fn pick(g: &mut Game, region: usize, i: u32) {
    let (c, _) = g.region(region).field.pick_sphere(i);
    g.tick(1.0);
    g.harvest(region, i, c + Vec3::Y).unwrap();
}

/// Pick every flower of every region, in order.
fn harvest_world(g: &mut Game) {
    for r in 0..g.data().regions.len() {
        let n = g.region(r).field.len() as u32;
        for i in 0..n {
            pick(g, r, i);
        }
    }
    g.tick(1_000.0);
}

#[test]
fn first_steps_unlock_once_and_pay_their_reward() {
    let d = data();
    let mut g = Game::new(d.clone());
    pick(&mut g, 0, 0);
    let first_flower = achievement(&d, "first_flower");
    let first_discovery = achievement(&d, "first_discovery");
    let got = unlocked_events(&mut g);
    assert!(got.contains(&first_flower) && got.contains(&first_discovery));
    let reward =
        d.achievements[first_flower].reward.coins + d.achievements[first_discovery].reward.coins;
    assert_eq!(g.state().coins, reward);
    assert_eq!(g.state().stats.coins_earned, reward);

    // Another pick announces nothing new for them.
    pick(&mut g, 0, 1);
    let again = unlocked_events(&mut g);
    assert!(!again.contains(&first_flower));
    assert!(g.is_achievement_unlocked(first_flower));
}

#[test]
fn progress_counts_up_and_is_capped() {
    let d = data();
    let mut g = Game::new(d.clone());
    let hundred = achievement(&d, "hundred_flowers");
    assert_eq!(g.achievement_progress(hundred), (0, 100));
    for i in 0..10 {
        pick(&mut g, 0, i);
    }
    assert_eq!(g.achievement_progress(hundred), (10, 100));
    for i in 10..100 {
        pick(&mut g, 0, i);
    }
    assert_eq!(g.achievement_progress(hundred), (100, 100));
    assert!(g.is_achievement_unlocked(hundred));
    assert!(g.is_achievement_unlocked(achievement(&d, "first_region")));
}

#[test]
fn a_rare_find_counts_rare_and_very_rare_flowers() {
    // Make the meadow's first flower's species rare for this test.
    let mut content = GameData::builtin().unwrap();
    let mut probe = Game::new(Arc::new(content.clone()));
    let species = probe.region(0).field.species(0);
    content.species[species].rarity = Rarity::Rare;
    let d = Arc::new(content.reindexed_and_validated().unwrap());

    let mut g = Game::new(d.clone());
    assert_eq!(g.rare_flowers_harvested(), 0);
    pick(&mut g, 0, 0);
    assert_eq!(g.rare_flowers_harvested(), 1);
    assert!(g.is_achievement_unlocked(achievement(&d, "first_rare")));
}

#[test]
fn binding_bouquets_unlocks_crafting_achievements() {
    let d = data();
    let mut g = rich(d.clone(), 10);
    g.buy_tool(d.tool_index("binding_ribbon").unwrap()).unwrap();
    for i in 0..30 {
        pick(&mut g, 0, i);
    }
    g.drain_events();
    g.craft(0, 5, &BTreeSet::new()).unwrap();
    let got = unlocked_events(&mut g);
    assert!(got.contains(&achievement(&d, "first_bouquet")));
    g.tick(100.0);
    g.craft(0, 5, &BTreeSet::new()).unwrap();
    assert!(unlocked_events(&mut g).contains(&achievement(&d, "ten_bouquets")));
}

#[test]
fn spending_is_counted() {
    let d = data();
    let mut g = rich(d.clone(), 10);
    let paid = g.buy_tool(d.tool_index("binding_ribbon").unwrap()).unwrap();
    g.buy_upgrade(0).unwrap();
    let upgrade = d.upgrades[0].base_price;
    assert_eq!(g.state().stats.coins_spent, paid + upgrade);
    g.add_distance(12.5);
    g.add_distance(-3.0);
    g.add_distance(f32::NAN);
    assert!((g.state().stats.distance_walked_m - 12.5).abs() < 1e-9);
}

#[test]
fn the_whole_journey_reaches_completion() {
    let d = data();
    let mut g = rich(d.clone(), 30);
    let sp_before = g.skill_points_earned();
    harvest_world(&mut g);

    let c = g.completion();
    assert_eq!(c.flowers.0, c.flowers.1);
    assert_eq!(c.regions.0, c.regions.1);
    assert_eq!(c.species.0, c.species.1);
    assert!(!c.is_complete(), "tools and bouquets are still missing");
    for id in [
        "master_harvester",
        "thousand_flowers",
        "world_explorer",
        "all_regions_unlocked",
        "complete_collection",
    ] {
        assert!(g.is_achievement_unlocked(achievement(&d, id)), "{id}");
    }
    // Field Hand, World Explorer and Complete Collection give skill points
    // (alongside the five region rewards and two collection milestones).
    assert_eq!(g.skill_points_earned(), sp_before + 5 + 2 + 3);
    let done = achievement(&d, "bloom_and_gather");
    assert_eq!(g.achievement_progress(done), (3, 6));

    // Every tool, then every required bouquet once (rarest recipes first).
    for t in 0..d.tools.len() {
        if !g.owns_tool(t) {
            g.buy_tool(t).unwrap();
        }
    }
    let mut order: Vec<usize> = (0..d.recipes.len())
        .filter(|&r| d.recipes[r].required_for_completion)
        .collect();
    order.sort_by_key(|&r| std::cmp::Reverse(d.recipes[r].tier));
    for r in order {
        g.craft(r, 1, &BTreeSet::new())
            .unwrap_or_else(|e| panic!("{}: {e}", d.recipes[r].id));
        g.tick(100.0);
    }
    let c = g.completion();
    assert!(c.is_complete(), "{c:?}");
    for id in [
        "all_tools",
        "all_recipes",
        "all_required_bouquets",
        "bloom_and_gather",
    ] {
        assert!(g.is_achievement_unlocked(achievement(&d, id)), "{id}");
    }
}

#[test]
fn older_saves_catch_up_quietly() {
    let d = data();
    let mut g = Game::new(d.clone());
    for i in 0..3 {
        pick(&mut g, 0, i);
    }
    // A save from before achievements existed.
    let mut state = g.state().clone();
    state.achievements.clear();
    let coins = state.coins;
    let mut loaded = Game::from_state(d.clone(), state).unwrap();
    assert!(loaded.is_achievement_unlocked(achievement(&d, "first_flower")));
    assert!(loaded.state().coins > coins, "rewards are paid");
    assert!(unlocked_events(&mut loaded).is_empty(), "but not announced");

    let bad = GameState {
        achievements: ["climb_everest".to_string()].into(),
        ..Default::default()
    };
    assert!(Game::from_state(d, bad).is_err());
}

#[test]
fn achievements_are_reachable_and_optional() {
    let d = data();
    assert_eq!(d.achievements.len(), 19);
    // Each category from the design is represented.
    for c in bloom_core::data::AchievementCategory::ALL {
        assert!(d.achievements.iter().any(|a| a.category == c), "{c:?}");
    }
    // An impossible target is rejected.
    let mut broken = GameData::builtin().unwrap();
    broken.achievements[0].condition = Condition::BouquetsCrafted(100_000);
    assert!(broken.reindexed_and_validated().is_err());
    // Skill points from achievements keep the tree a choice until late game.
    let sp: u32 = d.achievements.iter().map(|a| a.reward.skill_points).sum();
    assert!(sp <= 4, "{sp}");
}
