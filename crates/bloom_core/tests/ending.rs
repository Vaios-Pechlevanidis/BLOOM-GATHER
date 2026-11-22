//! Phase Z: the completion checker (each requirement), the moment of
//! completion, the journey summary and free roam afterwards.

use std::collections::BTreeSet;
use std::sync::Arc;

use bloom_core::game::{GameEvent, GameState};
use bloom_core::{Game, GameData};
use glam::Vec3;

fn data() -> Arc<GameData> {
    Arc::new(GameData::builtin().unwrap())
}

fn rich(d: Arc<GameData>) -> Game {
    let xp = d.progression.level_curve.total_xp_for_level(30);
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

/// What the journey leaves undone.
#[derive(Clone, Copy, PartialEq)]
enum Skip {
    Nothing,
    /// One daisy in the meadow stays unpicked.
    OneFlower,
    /// The flower cart is never bought.
    OneTool,
    /// The Royal Bouquet is never bound.
    OneBouquet,
}

/// A meadow daisy that no required bouquet depends on.
fn spare_daisy(g: &mut Game) -> u32 {
    let d = g.data().clone();
    let daisy = d.species_index("daisy").unwrap();
    let field = g.region(0).field.clone();
    (0..field.len() as u32)
        .find(|&i| field.species(i) == daisy)
        .unwrap()
}

/// The whole game, minus `skip`.
fn journey(skip: Skip) -> Game {
    let d = data();
    let mut g = rich(d.clone());
    let spare = spare_daisy(&mut g);
    for r in 0..d.regions.len() {
        let n = g.region(r).field.len() as u32;
        for i in 0..n {
            if !(skip == Skip::OneFlower && r == 0 && i == spare) {
                pick(&mut g, r, i);
            }
        }
    }
    g.tick(1_000.0);
    for t in 0..d.tools.len() {
        let cart = d.tools[t].id == "advanced_harvesting_equipment";
        if !g.owns_tool(t) && !(skip == Skip::OneTool && cart) {
            g.buy_tool(t).unwrap();
        }
    }
    let mut order: Vec<usize> = (0..d.recipes.len())
        .filter(|&r| d.recipes[r].required_for_completion)
        .collect();
    order.sort_by_key(|&r| std::cmp::Reverse(d.recipes[r].tier));
    for r in order {
        if skip == Skip::OneBouquet && d.recipes[r].id == "royal_bouquet" {
            continue;
        }
        g.craft(r, 1, &BTreeSet::new())
            .unwrap_or_else(|e| panic!("{}: {e}", d.recipes[r].id));
        g.tick(100.0);
    }
    g
}

fn completed_events(g: &mut Game) -> usize {
    g.drain_events()
        .into_iter()
        .filter(|e| *e == GameEvent::GameCompleted)
        .count()
}

#[test]
fn one_unpicked_flower_keeps_the_game_open() {
    let mut g = journey(Skip::OneFlower);
    let c = g.completion();
    assert_eq!(c.flowers.0 + 1, c.flowers.1);
    assert_eq!(c.regions.0 + 1, c.regions.1, "the meadow isn't complete");
    assert_eq!((c.tools.0, c.species.0), (c.tools.1, c.species.1));
    assert_eq!(c.required_bouquets.0, c.required_bouquets.1);
    assert!(!c.is_complete());
    assert!(!g.is_game_complete());
    g.drain_events();

    let spare = spare_daisy(&mut g);
    pick(&mut g, 0, spare);
    assert!(g.completion().is_complete());
    assert!(g.is_game_complete());
    assert_eq!(completed_events(&mut g), 1);
}

#[test]
fn a_missing_tool_keeps_the_game_open() {
    let mut g = journey(Skip::OneTool);
    let c = g.completion();
    assert_eq!(c.tools.0 + 1, c.tools.1);
    assert_eq!(c.met(), 5);
    assert!(!g.is_game_complete());
    g.drain_events();
    let cart = g
        .data()
        .tool_index("advanced_harvesting_equipment")
        .unwrap();
    g.buy_tool(cart).unwrap();
    assert!(g.is_game_complete());
    assert_eq!(completed_events(&mut g), 1);
}

#[test]
fn an_unbound_required_bouquet_keeps_the_game_open() {
    let mut g = journey(Skip::OneBouquet);
    let c = g.completion();
    assert_eq!(c.required_bouquets.0 + 1, c.required_bouquets.1);
    assert_eq!(
        c.recipes_discovered.0, c.recipes_discovered.1,
        "known, just not bound"
    );
    assert_eq!(c.met(), 5);
    g.drain_events();
    let royal = g.data().recipe_index("royal_bouquet").unwrap();
    // A started batch always finishes (its timer is saved), so the bouquet
    // counts as bound from the moment it is started.
    g.craft(royal, 1, &BTreeSet::new()).unwrap();
    assert!(g.is_game_complete());
    assert_eq!(completed_events(&mut g), 1);
}

#[test]
fn species_and_recipes_count_on_the_way() {
    let d = data();
    let mut g = rich(d.clone());
    let c = g.completion();
    assert_eq!(c.species, (0, d.species.len() as u32));
    assert_eq!(c.flowers, (0, d.world_flower_total()));
    assert_eq!(c.tools.0, 0);
    assert_eq!(c.recipes_discovered.0, 0);
    assert_eq!(c.met(), 0);
    pick(&mut g, 0, 0);
    assert_eq!(g.completion().species.0, 1);
    // A recipe is discovered by its tools (and named species, level).
    let ribbon = d.tool_index("binding_ribbon").unwrap();
    g.buy_tool(ribbon).unwrap();
    assert_eq!(g.completion().recipes_discovered.0, 1);
    assert!(!g.is_game_complete());
}

#[test]
fn the_summary_is_a_snapshot_of_the_moment() {
    let mut g = journey(Skip::Nothing);
    assert!(g.is_game_complete());
    assert_eq!(completed_events(&mut g), 1);
    let s = g.journey_summary().unwrap().clone();
    let d = g.data().clone();
    assert_eq!(s.flowers_harvested, d.world_flower_total());
    assert_eq!(s.species, d.species.len() as u32);
    assert_eq!(s.tools, d.tools.len() as u32);
    assert_eq!(s.regions, d.regions.len() as u32);
    assert_eq!(s.level, g.level());
    assert!(s.bouquets_crafted >= 8);
    assert!(s.rare_flowers > 0);
    assert!(s.playtime_secs > 0.0);
    assert_eq!(s.achievements, g.state().achievements.len() as u32);
    // Free roam: time passes, things get sold, the summary stays.
    assert!(g.finale_pending());
    g.mark_finale_seen();
    assert!(!g.finale_pending());
    g.tick(500.0);
    g.sell_all();
    let held: Vec<usize> = (0..d.recipes.len())
        .filter(|&r| g.state().bouquets.contains_key(&d.recipes[r].id))
        .collect();
    for r in held {
        let n = g.state().bouquets[&d.recipes[r].id];
        g.sell_bouquets(r, n).unwrap();
    }
    assert_eq!(g.journey_summary(), Some(&s));
    assert!(g.is_game_complete());
    assert_eq!(completed_events(&mut g), 0, "completed only once");
}

#[test]
fn the_ending_survives_a_save_and_resumes_if_unseen() {
    let d = data();
    let g = journey(Skip::Nothing);
    // Saved before the finale was watched: it plays after loading.
    let restored = Game::from_state(d.clone(), g.state().clone()).unwrap();
    assert!(restored.finale_pending());
    assert_eq!(restored.journey_summary(), g.journey_summary());
    // A save from before Phase Z (no record yet) records it quietly.
    let mut older = g.state().clone();
    older.completed = None;
    let mut caught_up = Game::from_state(d.clone(), older).unwrap();
    assert!(caught_up.is_game_complete());
    assert!(caught_up.finale_pending());
    assert!(caught_up.drain_events().is_empty());
    // After watching it, free roam is remembered.
    let mut seen = Game::from_state(d.clone(), g.state().clone()).unwrap();
    seen.mark_finale_seen();
    let back = Game::from_state(d, seen.state().clone()).unwrap();
    assert!(back.is_game_complete() && !back.finale_pending());
}

#[test]
fn a_forged_ending_is_refused() {
    let d = data();
    let real = journey(Skip::Nothing);
    let summary = real.journey_summary().cloned();
    let forged = GameState {
        completed: summary,
        ..Default::default()
    };
    assert!(Game::from_state(d.clone(), forged).is_err());
    let seen_nothing = GameState {
        finale_seen: true,
        ..Default::default()
    };
    assert!(Game::from_state(d, seen_nothing).is_err());
}
