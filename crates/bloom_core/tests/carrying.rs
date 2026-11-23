//! The carry limit: the player starts able to carry a single flower, walks
//! into the hut to leave what they carry, sells only there, and carries more
//! with skills and carrying tools (sacks, baskets, the cart).

use std::collections::BTreeMap;
use std::sync::Arc;

use bloom_core::data::HarvestMode;
use bloom_core::game::{GameError, GameEvent, GameState};
use bloom_core::world::LandmarkKind;
use bloom_core::{Game, GameData};
use glam::{Vec2, Vec3};

fn data() -> Arc<GameData> {
    Arc::new(GameData::builtin().unwrap())
}

/// A consistent state with coins and XP but nothing harvested.
fn rich(level: u32, skills: &[(&str, u32)]) -> Game {
    let d = data();
    let xp = d.progression.level_curve.total_xp_for_level(level);
    Game::from_state(
        d,
        GameState {
            coins: 100_000,
            xp,
            skills: skills
                .iter()
                .map(|(id, n)| (id.to_string(), *n))
                .collect::<BTreeMap<_, _>>(),
            ..Default::default()
        },
    )
    .unwrap()
}

fn pick(g: &mut Game, i: u32) -> Result<(), GameError> {
    let (c, _) = g.region(0).field.pick_sphere(i);
    g.tick(5.0);
    g.harvest(0, i, c + Vec3::Y).map(|_| ())
}

fn tool(d: &GameData, id: &str) -> usize {
    d.tools.iter().position(|t| t.id == id).unwrap()
}

fn owned(g: &Game) -> u32 {
    g.state().inventory.values().sum()
}

#[test]
fn a_new_player_carries_a_single_flower() {
    let mut g = Game::new(data());
    assert_eq!(g.carry_capacity(), 1);
    assert_eq!(g.carry_space(), 1);
    pick(&mut g, 0).unwrap();
    assert_eq!(g.carried(), 1);
    assert_eq!(g.carry_space(), 0);
    let harvested = g.state().stats.flowers_harvested;
    assert_eq!(pick(&mut g, 1), Err(GameError::HandsFull { capacity: 1 }));
    assert!(!g.is_harvested(0, 1), "a refused pick changes nothing");
    assert_eq!(g.state().stats.flowers_harvested, harvested);
}

#[test]
fn the_hut_takes_the_flowers_off_your_hands() {
    let mut g = Game::new(data());
    pick(&mut g, 0).unwrap();
    g.drain_events();
    g.set_at_workshop(true);
    assert_eq!(g.carried(), 0);
    assert_eq!(owned(&g), 1, "the flower is still yours, kept at the hut");
    assert!(g
        .drain_events()
        .contains(&GameEvent::FlowersStored { count: 1 }));
    // Standing in the hut with empty hands announces nothing more.
    g.set_at_workshop(true);
    assert!(g.drain_events().is_empty());
    g.set_at_workshop(false);
    pick(&mut g, 1).unwrap();
    assert_eq!((g.carried(), owned(&g)), (1, 2));
}

#[test]
fn flowers_are_only_sold_at_the_hut() {
    let mut g = Game::new(data());
    pick(&mut g, 0).unwrap();
    let species = g.region(0).field.species(0);
    assert_eq!(g.sell(species, 1), Err(GameError::NotAtWorkshop));
    assert_eq!(g.sell_all(), (0, 0));
    assert_eq!(owned(&g), 1);
    g.set_at_workshop(true);
    assert_eq!(g.sell_all().0, 1);
    assert_eq!((g.carried(), owned(&g)), (0, 0));
}

#[test]
fn skills_and_carrying_tools_add_up() {
    let d = data();
    let base = d.harvesting.base_carry_capacity;
    let g = rich(30, &[("strong_arms", 2)]);
    assert_eq!(g.carry_capacity(), base + 4);

    let mut g = rich(30, &[]);
    let mut expected = base;
    for (id, extra) in [
        ("linen_sack", 2),
        ("burlap_sack", 6),
        ("harvesting_basket", 6),
        ("back_basket", 15),
    ] {
        g.buy_tool(tool(&d, id))
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        expected += extra;
        assert_eq!(g.carry_capacity(), expected, "after {id}");
    }
}

#[test]
fn a_basket_gathers_no_more_than_fits() {
    let d = data();
    let mut g = rich(30, &[]);
    g.buy_tool(tool(&d, "harvesting_basket")).unwrap();
    let field = g.region(0).field.clone();
    let layout = g.region(0).layout.clone();
    // The flower with the most neighbours in basket reach.
    let best = (0..field.len() as u32)
        .max_by_key(|&i| {
            (0..field.len() as u32)
                .filter(|&j| field.position(i).distance(field.position(j)) < 1.6)
                .count()
        })
        .unwrap();
    let p = field.position(best);
    let ground = Vec3::new(p.x, layout.terrain.height_at(p), p.y);
    let eye = ground + Vec3::new(0.0, 1.6, 2.0);
    let center = Vec2::new(p.x, p.y);

    // Fill the hands until only two flowers fit.
    let space = g.carry_space();
    let mut i = 0;
    while g.carry_space() > 2 {
        if field.position(i).distance(p) > 4.0 {
            pick(&mut g, i).unwrap();
        }
        i += 1;
    }
    assert_eq!(g.carried(), space - 2);
    assert_eq!(g.preview_circle(0, HarvestMode::Radius, center).len(), 2);
    g.tick(5.0);
    let out = g.harvest_radius(0, ground, eye).unwrap();
    assert_eq!(out.flowers.len(), 2);
    assert_eq!(g.carry_space(), 0);
    g.tick(5.0);
    assert!(matches!(
        g.harvest_radius(0, ground, eye),
        Err(GameError::HandsFull { .. })
    ));
}

#[test]
fn saves_keep_what_is_carried() {
    let mut g = Game::new(data());
    pick(&mut g, 0).unwrap();
    let restored = Game::from_state(data(), g.state().clone()).unwrap();
    assert_eq!(restored.carried(), 1);
    assert!(
        !restored.at_workshop(),
        "where the player stands is reported again after loading"
    );

    // Older saves have no carried count: nothing is in hand.
    let mut old = g.state().clone();
    old.carried = 0;
    assert_eq!(Game::from_state(data(), old).unwrap().carried(), 0);

    // A save claiming more in hand than is owned is trimmed.
    let mut odd = g.state().clone();
    odd.carried = 50;
    assert_eq!(Game::from_state(data(), odd).unwrap().carried(), 1);
}

#[test]
fn the_hut_interior_is_recognised() {
    let mut g = Game::new(data());
    let layout = g.region(0).layout.clone();
    let hut = layout
        .landmarks
        .iter()
        .find(|l| l.kind == LandmarkKind::Workshop)
        .expect("the farm has a hut");
    assert!(layout.in_workshop(hut.position));
    assert!(layout.in_workshop(hut.position + Vec2::new(1.0, 1.0)));
    assert!(!layout.in_workshop(hut.position + Vec2::new(12.0, 0.0)));
    assert!(!layout.in_workshop(hut.position + Vec2::new(0.0, -12.0)));
}
