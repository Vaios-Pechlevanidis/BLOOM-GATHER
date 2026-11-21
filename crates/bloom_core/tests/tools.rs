//! Tools: buying them and using the harvesting modes they unlock.

use std::sync::Arc;

use bloom_core::data::HarvestMode;
use bloom_core::game::{GameError, GameEvent, GameState, ToolLock};
use bloom_core::{Game, GameData, ToolStatus};
use glam::{Vec2, Vec3};

fn data() -> Arc<GameData> {
    Arc::new(GameData::builtin().unwrap())
}

/// A game with coins and XP but nothing harvested (a consistent state).
fn rich(coins: u64, level: u32) -> Game {
    let d = data();
    let xp = d.progression.level_curve.total_xp_for_level(level);
    Game::from_state(
        d,
        GameState {
            coins,
            xp,
            ..Default::default()
        },
    )
    .unwrap()
}

fn tool(d: &GameData, id: &str) -> usize {
    d.tools.iter().position(|t| t.id == id).unwrap()
}

fn with_tools(ids: &[&str]) -> Game {
    let mut g = rich(100_000, 30);
    let d = g.data().clone();
    for id in ids {
        g.buy_tool(tool(&d, id))
            .unwrap_or_else(|e| panic!("{id}: {e}"));
    }
    g
}

/// A spot on the ground next to a flower cluster, and an eye looking at it.
fn near_flowers(g: &mut Game) -> (Vec3, Vec3) {
    let field = g.region(0).field.clone();
    let layout = g.region(0).layout.clone();
    // The flower with the most neighbours within 1.6 m.
    let best = (0..field.len() as u32)
        .max_by_key(|&i| {
            (0..field.len() as u32)
                .filter(|&j| field.position(i).distance(field.position(j)) < 1.6)
                .count()
        })
        .unwrap();
    let p = field.position(best);
    let ground = Vec3::new(p.x, layout.terrain.height_at(p), p.y);
    (ground, ground + Vec3::new(0.0, 1.6, 2.0))
}

#[test]
fn tools_need_level_coins_and_prerequisites() {
    let d = data();
    let ribbon = tool(&d, "binding_ribbon");
    let mut g = Game::new(d.clone());
    assert_eq!(
        g.tool_status(ribbon),
        ToolStatus::Locked(ToolLock::Level(2))
    );
    assert_eq!(
        g.buy_tool(ribbon),
        Err(GameError::LevelTooLow { needed: 2 })
    );

    let mut g = rich(50, 2);
    assert_eq!(
        g.buy_tool(ribbon),
        Err(GameError::NotEnoughCoins {
            price: 100,
            have: 50
        })
    );

    let mut g = rich(1_000, 12);
    assert_eq!(g.tool_status(ribbon), ToolStatus::Buyable);
    assert_eq!(g.buy_tool(ribbon), Ok(100));
    assert_eq!(g.state().coins, 900);
    assert!(g.owns_tool(ribbon));
    assert!(g
        .drain_events()
        .contains(&GameEvent::ToolBought { tool: ribbon }));
    assert_eq!(g.buy_tool(ribbon), Err(GameError::AlreadyOwned));
    assert!(g.is_dirty());

    let gardening = tool(&d, "pro_gardening_tools");
    assert_eq!(
        g.buy_tool(gardening),
        Err(GameError::NeedsTool {
            tool: "harvesting_basket".into()
        })
    );
    assert_eq!(g.state().coins, 900, "failed purchases cost nothing");
    assert_eq!(g.buy_tool(999), Err(GameError::UnknownTool));
}

#[test]
fn scissors_make_picking_faster() {
    let g = Game::new(data());
    let hand = g.harvest_cooldown_secs();
    let g = with_tools(&["garden_scissors"]);
    assert!((g.harvest_cooldown_secs() - hand * 0.6).abs() < 1e-6);
}

#[test]
fn modes_unlock_with_tools() {
    assert_eq!(
        Game::new(data()).available_modes(),
        vec![HarvestMode::Single]
    );
    let g = with_tools(&["harvesting_basket"]);
    assert_eq!(
        g.available_modes(),
        vec![HarvestMode::Single, HarvestMode::Radius]
    );
    let g = with_tools(&[
        "harvesting_basket",
        "pro_gardening_tools",
        "advanced_harvesting_equipment",
    ]);
    assert_eq!(
        g.available_modes(),
        vec![
            HarvestMode::Single,
            HarvestMode::Radius,
            HarvestMode::Sweep,
            HarvestMode::Area
        ]
    );
}

#[test]
fn basket_gathers_a_capped_circle_once() {
    let mut g = with_tools(&["harvesting_basket"]);
    let params = g.mode_params(HarvestMode::Radius).unwrap();
    let (ground, eye) = near_flowers(&mut g);
    let center = Vec2::new(ground.x, ground.z);
    let preview = g.preview_circle(0, HarvestMode::Radius, center);
    assert!(!preview.is_empty() && preview.len() <= params.max_flowers as usize);
    let field = g.region(0).field.clone();
    for &i in &preview {
        assert!(field.position(i).distance(center) <= params.radius + 1e-4);
    }
    let out = g.harvest_radius(0, ground, eye).unwrap();
    assert_eq!(out.flowers, preview);
    for &i in &preview {
        assert!(g.is_harvested(0, i));
    }
    assert_eq!(g.state().stats.flowers_harvested as usize, preview.len());
    // Basket cooldown, then the same spot can't pick the same flowers again.
    assert!(matches!(
        g.harvest_radius(0, ground, eye),
        Err(GameError::CoolingDown { .. })
    ));
    g.tick(5.0);
    if let Ok(again) = g.harvest_radius(0, ground, eye) {
        assert!(again.flowers.iter().all(|f| !preview.contains(f)));
    }
}

#[test]
fn basket_checks_tool_reach_and_emptiness() {
    let mut g = Game::new(data());
    let (ground, eye) = near_flowers(&mut g);
    assert_eq!(g.harvest_radius(0, ground, eye), Err(GameError::ModeLocked));

    let mut g = with_tools(&["harvesting_basket"]);
    let far_eye = ground + Vec3::new(0.0, 1.6, 20.0);
    assert!(matches!(
        g.harvest_radius(0, ground, far_eye),
        Err(GameError::OutOfReach { .. })
    ));
    // Empty ground (the spawn point is kept clear of flowers).
    let layout = g.region(0).layout.clone();
    let spawn = Vec3::new(
        layout.spawn.x,
        layout.terrain.height_at(layout.spawn),
        layout.spawn.y,
    );
    assert_eq!(
        g.harvest_radius(0, spawn, spawn + Vec3::Y * 1.6),
        Err(GameError::NothingInReach)
    );
    // A miss doesn't start the cooldown.
    assert!(g.harvest_radius(0, ground, eye).is_ok());
}

#[test]
fn sweeping_gathers_in_small_steps() {
    let mut g = with_tools(&["harvesting_basket", "pro_gardening_tools"]);
    let cap = g.mode_params(HarvestMode::Sweep).unwrap().max_flowers as usize;
    let interval = g.data().harvesting.sweep_interval_secs;
    let (ground, eye) = near_flowers(&mut g);
    let first = g.harvest_sweep(0, ground, eye).unwrap();
    assert!(first.flowers.len() <= cap);
    assert!(matches!(
        g.harvest_sweep(0, ground, eye),
        Err(GameError::CoolingDown { .. })
    ));
    g.tick(interval);
    let _ = g.harvest_sweep(0, ground, eye);
}

#[test]
fn area_tool_clamps_size_and_needs_reach() {
    let mut g = with_tools(&[
        "harvesting_basket",
        "pro_gardening_tools",
        "advanced_harvesting_equipment",
    ]);
    let params = g.mode_params(HarvestMode::Area).unwrap();
    let (ground, eye) = near_flowers(&mut g);
    let start = Vec2::new(ground.x, ground.z) - Vec2::splat(2.0);
    let (min, max, hits) = g.preview_area(0, start, start + Vec2::splat(50.0)).unwrap();
    assert!((max - min).x <= params.size.x + 1e-4 && (max - min).y <= params.size.y + 1e-4);
    assert!(hits.len() <= params.max_flowers as usize);
    let field = g.region(0).field.clone();
    for &i in &hits {
        let p = field.position(i);
        assert!(p.cmpge(min).all() && p.cmple(max).all());
    }
    let far = eye + Vec3::new(40.0, 0.0, 40.0);
    assert!(matches!(
        g.harvest_area(0, start, start + Vec2::splat(50.0), far),
        Err(GameError::OutOfReach { .. })
    ));
    let out = g
        .harvest_area(0, start, start + Vec2::splat(50.0), eye)
        .unwrap();
    assert_eq!(out.flowers, hits);
}

#[test]
fn mixed_modes_keep_the_state_consistent() {
    let mut g = with_tools(&[
        "harvesting_basket",
        "pro_gardening_tools",
        "advanced_harvesting_equipment",
    ]);
    let (ground, eye) = near_flowers(&mut g);
    let _ = g.harvest_radius(0, ground, eye);
    g.tick(5.0);
    let _ = g.harvest_sweep(0, ground + Vec3::X, eye);
    g.tick(5.0);
    let start = Vec2::new(ground.x, ground.z) + Vec2::new(-3.0, -3.0);
    let _ = g.harvest_area(0, start, start + Vec2::splat(6.0), eye);
    // Inventory adds up and a save of this state restores cleanly.
    let held: u32 = g.state().inventory.values().sum();
    assert_eq!(held as u64, g.state().stats.flowers_harvested);
    assert!(Game::from_state(g.data().clone(), g.state().clone()).is_ok());
    // Every harvested flower produced exactly one event.
    let picked = g
        .drain_events()
        .iter()
        .filter(|e| matches!(e, GameEvent::FlowerHarvested { .. }))
        .count();
    assert_eq!(picked as u64, g.state().stats.flowers_harvested);
}
