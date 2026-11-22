//! Game rules: harvesting, inventory, discovery, XP, region completion,
//! selling with the keepsake reserve, and restoring saved state.

use std::sync::Arc;

use bloom_core::game::{GameError, GameEvent, GameState};
use bloom_core::{Game, GameData};
use glam::Vec3;

fn data() -> Arc<GameData> {
    Arc::new(GameData::builtin().unwrap())
}

/// An eye position right next to a flower's bloom.
fn eye_near(game: &mut Game, region: usize, flower: u32) -> Vec3 {
    let (center, _) = game.region(region).field.pick_sphere(flower);
    center + Vec3::new(0.8, 1.0, 0.0)
}

fn pick(
    game: &mut Game,
    region: usize,
    flower: u32,
) -> Result<bloom_core::game::HarvestOutcome, GameError> {
    let eye = eye_near(game, region, flower);
    game.tick(10.0); // let any cooldown expire
    game.harvest(region, flower, eye)
}

#[test]
fn harvesting_moves_a_flower_into_the_basket() {
    let mut game = Game::new(data());
    let species = game.region(0).field.species(0);
    let out = pick(&mut game, 0, 0).unwrap();
    assert_eq!(out.species, species);
    assert!(out.newly_discovered);
    assert_eq!(game.inventory_count(species), 1);
    assert!(game.is_harvested(0, 0));
    assert_eq!(game.remaining_in_region(0), 99);
    assert!(game.is_discovered(species));
    assert!(game.is_dirty());
    let events = game.drain_events();
    assert!(events.contains(&GameEvent::FlowerHarvested {
        region: 0,
        flower: 0,
        species
    }));
    assert!(events.contains(&GameEvent::SpeciesDiscovered { species }));
}

#[test]
fn a_flower_can_only_be_harvested_once() {
    let mut game = Game::new(data());
    pick(&mut game, 0, 5).unwrap();
    assert_eq!(pick(&mut game, 0, 5), Err(GameError::AlreadyHarvested));
    assert_eq!(game.state().stats.flowers_harvested, 1);
    let species = game.region(0).field.species(5);
    assert_eq!(game.inventory_count(species), 1);
}

#[test]
fn harvesting_has_a_cooldown_and_needs_reach() {
    let mut game = Game::new(data());
    pick(&mut game, 0, 1).unwrap();
    let eye = eye_near(&mut game, 0, 2);
    assert!(matches!(
        game.harvest(0, 2, eye),
        Err(GameError::CoolingDown { .. })
    ));
    game.tick(1.0);
    let far = eye + Vec3::new(20.0, 0.0, 0.0);
    assert!(matches!(
        game.harvest(0, 2, far),
        Err(GameError::OutOfReach { .. })
    ));
    assert!(game.harvest(0, 2, eye).is_ok());
    game.tick(1.0);
    assert_eq!(game.harvest(0, 10_000, eye), Err(GameError::UnknownFlower));
}

#[test]
fn locked_regions_cannot_be_harvested() {
    let mut game = Game::new(data());
    assert!(!game.is_region_unlocked(1));
    assert_eq!(pick(&mut game, 1, 0), Err(GameError::RegionLocked));
}

#[test]
fn xp_levels_and_discovery_bonus() {
    let d = data();
    let mut game = Game::new(d.clone());
    // First pick: harvest XP + discovery XP.
    let out = pick(&mut game, 0, 0).unwrap();
    assert_eq!(
        out.xp_gained,
        d.progression.xp_per_harvest + d.progression.xp_per_discovery
    );
    assert_eq!(game.state().xp, out.xp_gained);
    // 28 XP so far; level 2 needs 30.
    let mut leveled = false;
    for i in 1..10 {
        pick(&mut game, 0, i).unwrap();
        leveled |= game
            .drain_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelUp { level: 2 }));
    }
    assert!(leveled);
    assert!(game.level() >= 2);
}

#[test]
fn clearing_a_region_completes_it_once_and_pays_out() {
    let d = data();
    let mut game = Game::new(d.clone());
    let total = d.regions[0].flower_total;
    for i in 0..total {
        pick(&mut game, 0, i).unwrap();
    }
    assert_eq!(game.remaining_in_region(0), 0);
    assert!(game.state().completed_regions.contains("sunny_meadow"));
    // The completion reward, plus the coin rewards of the achievements
    // clearing a region earns on the way.
    let achievements: u64 = game
        .state()
        .achievements
        .iter()
        .map(|id| d.achievement(id).unwrap().reward.coins)
        .sum();
    assert!(achievements > 0);
    assert_eq!(
        game.state().coins,
        d.regions[0].completion_reward.coins + achievements
    );
    let events = game.drain_events();
    let completions = events
        .iter()
        .filter(|e| matches!(e, GameEvent::RegionCompleted { .. }))
        .count();
    assert_eq!(completions, 1);
    // Every species of the meadow is now discovered.
    for f in &d.regions[0].flowers {
        assert!(game.state().discovered.contains(&f.species));
    }
    // Lavender Fields' requirements (level 3, 60 harvested) are met.
    assert!(game.is_region_unlocked(1));
}

#[test]
fn selling_pays_the_same_for_every_species() {
    let d = data();
    let mut game = Game::new(d.clone());
    for i in 0..20 {
        pick(&mut game, 0, i).unwrap();
    }
    let mut before = game.state().coins;
    for species in 0..d.species.len() {
        let n = game.inventory_count(species);
        if n == 0 {
            continue;
        }
        let coins = game.sell(species, n).unwrap();
        assert_eq!(coins, n as u64 * d.economy.flower_base_value);
        assert_eq!(game.state().coins, before + coins);
        assert_eq!(game.inventory_count(species), 0);
        before = game.state().coins;
    }
    assert_eq!(game.state().stats.flowers_sold, 20);
}

#[test]
fn selling_validates_quantities() {
    let mut game = Game::new(data());
    pick(&mut game, 0, 0).unwrap();
    let coins = game.state().coins; // first-flower achievement rewards
    let species = game.region(0).field.species(0);
    assert_eq!(game.sell(species, 0), Err(GameError::NothingToSell));
    assert_eq!(
        game.sell(species, 2),
        Err(GameError::NotEnoughFlowers { have: 1 })
    );
    assert_eq!(game.sell(999, 1), Err(GameError::UnknownSpecies));
    assert_eq!(game.state().coins, coins, "failed sales change nothing");
}

#[test]
fn reserve_protects_flowers_needed_for_required_bouquets() {
    // Build a late-game situation directly: the world has only a few
    // Starpetals left and the player holds the last ones. The Royal Bouquet
    // (needs 1 Starpetal) hasn't been crafted, so one must be kept.
    let d = data();
    let starpetal = d.species_index("starpetal").unwrap();
    let world = d.species_world_total("starpetal") as u32;
    let mut state = GameState::default();
    state.inventory.insert("starpetal".into(), 3);
    // Pretend every Starpetal in the world has been picked: mark them in the
    // Moonlight Garden bitset so the state stays consistent.
    let mut game = Game::new(d.clone());
    let garden = d
        .regions
        .iter()
        .position(|r| r.id == "moonlight_garden")
        .unwrap();
    let field = game.region(garden).field.clone();
    let mut bits = bloom_core::bitset::BitSet::new(d.regions[garden].flower_total);
    for i in 0..field.len() as u32 {
        if field.species(i) == starpetal {
            bits.test_and_set(i);
        }
    }
    state.harvested.insert("moonlight_garden".into(), bits);
    // The other regions' Starpetals: none exist outside the garden.
    state
        .harvested_per_species
        .insert("starpetal".into(), world);
    state.stats.flowers_harvested = world as u64;
    let mut game = Game::from_state(d.clone(), state).expect("consistent state");

    assert_eq!(game.sellable(starpetal), 2);
    assert_eq!(game.reserved(starpetal), 1);
    assert_eq!(
        game.sell(starpetal, 3),
        Err(GameError::Reserved { sellable: 2 })
    );
    assert_eq!(game.sell(starpetal, 2), Ok(2 * d.economy.flower_base_value));
    assert_eq!(game.sellable(starpetal), 0);
    let (sold, _) = game.sell_all();
    assert_eq!(sold, 0);
    assert_eq!(game.inventory_count(starpetal), 1, "the keepsake stays");
}

#[test]
fn early_game_flowers_are_never_reserved() {
    let mut game = Game::new(data());
    for i in 0..30 {
        pick(&mut game, 0, i).unwrap();
    }
    let held: u32 = (0..game.data().species.len())
        .map(|s| game.inventory_count(s))
        .sum();
    let (sold, coins) = game.sell_all();
    assert_eq!(sold, held);
    assert_eq!(coins, held as u64 * 10);
}

#[test]
fn saved_state_restores_exactly() {
    let d = data();
    let mut game = Game::new(d.clone());
    for i in [0, 3, 9, 42, 77] {
        pick(&mut game, 0, i).unwrap();
    }
    let restored = Game::from_state(d.clone(), game.state().clone()).unwrap();
    assert_eq!(restored.state(), game.state());
    for i in [0, 3, 9, 42, 77] {
        assert!(restored.is_harvested(0, i));
    }
    assert_eq!(restored.remaining_in_region(0), 95);
}

#[test]
fn tampered_state_is_rejected() {
    let d = data();
    let mut game = Game::new(d.clone());
    pick(&mut game, 0, 0).unwrap();
    let species = d.species[game.region(0).field.species(0)].id.clone();

    let mut more_in_basket = game.state().clone();
    *more_in_basket.inventory.get_mut(&species).unwrap() += 5;
    assert!(Game::from_state(d.clone(), more_in_basket).is_err());

    let mut unknown = game.state().clone();
    unknown.inventory.insert("plastic_rose".into(), 1);
    assert!(Game::from_state(d.clone(), unknown).is_err());

    let mut miscounted = game.state().clone();
    miscounted.stats.flowers_harvested = 7;
    assert!(Game::from_state(d.clone(), miscounted).is_err());

    let mut wrong_species = game.state().clone();
    wrong_species.harvested_per_species.clear();
    wrong_species
        .harvested_per_species
        .insert("starpetal".into(), 1);
    assert!(Game::from_state(d.clone(), wrong_species).is_err());
}
