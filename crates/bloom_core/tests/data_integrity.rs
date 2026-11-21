//! Integration tests over the shipped content file.
//!
//! The first group pins the design targets from the specification; the second
//! group proves the validator actually catches broken content, so a passing
//! `validate()` means something.

use bloom_core::data::{GameData, HarvestMode, Rarity, BUILTIN_GAME_DATA};
use bloom_core::DataError;

fn data() -> GameData {
    GameData::builtin().unwrap_or_else(|e| panic!("{e}"))
}

/// Parse without validating, so tests can break content on purpose.
fn raw() -> GameData {
    ron::from_str(BUILTIN_GAME_DATA).expect("content parses")
}

fn invalid_messages(data: GameData) -> Vec<String> {
    match data.reindexed_and_validated() {
        Err(DataError::Invalid(errors)) => errors.into_iter().map(|e| e.0).collect(),
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("broken content passed validation"),
    }
}

fn assert_flags(data: GameData, needle: &str) {
    let messages = invalid_messages(data);
    assert!(
        messages.iter().any(|m| m.contains(needle)),
        "expected an error containing '{needle}', got: {messages:#?}"
    );
}

// ---------------------------------------------------------------------------
// Shipped content matches the design
// ---------------------------------------------------------------------------

#[test]
fn builtin_content_is_valid() {
    data();
}

#[test]
fn every_flower_has_the_same_base_value() {
    let d = data();
    assert_eq!(d.flower_value(), 10);
    // There is exactly one flower price in the whole content model; species
    // carry no price, so rarity cannot change it. Make that structural fact
    // visible: every species reports the same value.
    for s in &d.species {
        assert_eq!(d.flower_value(), d.economy.flower_base_value, "{}", s.id);
    }
}

#[test]
fn five_regions_with_spec_flower_counts() {
    let d = data();
    let counts: Vec<(&str, u32)> = d
        .regions
        .iter()
        .map(|r| (r.id.as_str(), r.counted_flowers()))
        .collect();
    assert_eq!(
        counts,
        vec![
            ("sunny_meadow", 100),
            ("lavender_fields", 200),
            ("blossom_valley", 350),
            ("enchanted_forest", 500),
            ("moonlight_garden", 750),
        ]
    );
    assert_eq!(d.world_flower_total(), 1_900);
}

#[test]
fn all_rarity_tiers_are_used() {
    let d = data();
    for rarity in Rarity::ALL {
        assert!(
            d.species.iter().any(|s| s.rarity == rarity),
            "{rarity:?} unused"
        );
    }
}

#[test]
fn spec_recipes_keep_their_target_prices() {
    let d = data();
    for (id, count, price) in [
        ("simple_bouquet", 3, 45),
        ("colorful_bouquet", 5, 90),
        ("romantic_bouquet", 7, 150),
        ("royal_bouquet", 15, 450),
    ] {
        let r = d.recipe(id).unwrap();
        assert_eq!((r.flower_count, r.sell_price), (count, price), "{id}");
    }
    assert!(d.recipe("premium_bouquet").unwrap().flower_count == 10);
}

#[test]
fn every_bouquet_beats_selling_its_flowers() {
    let d = data();
    for r in &d.recipes {
        let raw = d.economy.raw_flower_value(r.flower_count as u64);
        assert!(
            r.sell_price > raw,
            "{} sells for {} vs raw {raw}",
            r.id,
            r.sell_price
        );
    }
}

#[test]
fn bouquet_margins_rise_with_tier() {
    let d = data();
    let mut recipes: Vec<_> = d.recipes.iter().collect();
    recipes.sort_by_key(|r| r.tier);
    let margins: Vec<f64> = recipes
        .iter()
        .map(|r| r.sell_price as f64 / r.flower_count as f64)
        .collect();
    assert!(
        margins.windows(2).all(|w| w[0] < w[1]),
        "margins not increasing: {margins:?}"
    );
}

#[test]
fn required_tools_fit_in_guaranteed_income() {
    let d = data();
    assert_eq!(d.required_tool_cost(), 15_300);
    assert_eq!(d.guaranteed_income(), 20_900);
    assert!(d.required_tool_cost() <= d.guaranteed_income());
}

#[test]
fn harvesting_tools_unlock_every_mode_in_order() {
    let d = data();
    let modes: Vec<HarvestMode> = d
        .tools
        .iter()
        .flat_map(|t| t.effects.iter().filter_map(|e| e.unlocked_mode()))
        .collect();
    assert_eq!(
        modes,
        vec![HarvestMode::Radius, HarvestMode::Sweep, HarvestMode::Area]
    );
}

#[test]
fn every_species_is_obtainable() {
    let d = data();
    for s in &d.species {
        assert!(
            d.species_world_total(&s.id) > 0,
            "{} not in any region",
            s.id
        );
    }
}

// ---------------------------------------------------------------------------
// The validator catches broken content
// ---------------------------------------------------------------------------

#[test]
fn rejects_unprofitable_bouquet() {
    let mut d = raw();
    d.recipes[0].sell_price = 30; // 3 flowers × 10
    assert_flags(d, "worth 30 raw");
}

#[test]
fn rejects_region_count_mismatch() {
    let mut d = raw();
    d.regions[0].flower_total = 101;
    assert_flags(d, "flower_total is 101");
}

#[test]
fn rejects_unknown_references() {
    let mut d = raw();
    d.recipes[0].required_tools.push("magic_wand".into());
    d.regions[0].flowers[0].species = "plastic_rose".into();
    let messages = invalid_messages(d);
    assert!(messages
        .iter()
        .any(|m| m.contains("unknown tool 'magic_wand'")));
    assert!(messages
        .iter()
        .any(|m| m.contains("unknown species 'plastic_rose'")));
}

#[test]
fn rejects_unreachable_region_threshold() {
    let mut d = raw();
    d.regions[1].unlock.flowers_harvested = 101; // Sunny Meadow only has 100
    assert_flags(d, "earlier regions only hold 100");
}

#[test]
fn rejects_region_level_unreachable_by_harvesting() {
    let mut d = raw();
    d.regions[1].unlock.level = 10; // 60 flowers × 3 XP is nowhere near level 10
    assert_flags(d, "needs level 10");
}

#[test]
fn rejects_unaffordable_required_tools() {
    let mut d = raw();
    d.tools[0].price = 1_000_000;
    assert_flags(d, "guaranteed income");
}

#[test]
fn rejects_recipe_needing_more_than_world_supply() {
    let mut d = raw();
    let royal = d
        .recipes
        .iter_mut()
        .find(|r| r.id == "royal_bouquet")
        .unwrap();
    royal.required_species[2].count = 41; // only 40 Starpetals exist
    royal.flower_count = 60;
    royal.sell_price = 10_000;
    assert_flags(d, "only 40 exist");
}

#[test]
fn rejects_duplicate_ids() {
    let mut d = raw();
    let dup = d.species[0].clone();
    d.species.push(dup);
    assert_flags(d, "duplicate species id 'daisy'");
}

#[test]
fn rejects_tool_prerequisite_that_unlocks_later() {
    let mut d = raw();
    d.tools[0].requires_tools.push("pro_florist_set".into());
    assert_flags(d, "unlocks later");
}

#[test]
fn rejects_species_missing_from_world() {
    let mut d = raw();
    let mut ghost = d.species[0].clone();
    ghost.id = "ghost_orchid".into();
    d.species.push(ghost);
    assert_flags(d, "'ghost_orchid' does not appear in any region");
}
