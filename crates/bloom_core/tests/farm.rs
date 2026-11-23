//! The shipped map: one bulb farm with straight beds of one colour each,
//! wildflowers on the verges and a few strays among the beds.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use bloom_core::data::GameData;
use bloom_core::world::{FlowerField, LandmarkKind, RegionLayout};
use bloom_core::{Game, GameState};
use glam::{Vec2, Vec3};

fn data() -> GameData {
    GameData::builtin().unwrap()
}

fn farm(d: &GameData) -> (RegionLayout, FlowerField) {
    let region = &d.regions[0];
    let layout = RegionLayout::generate(region, &d.world);
    let field = FlowerField::generate(region, &layout, d).unwrap();
    (layout, field)
}

#[test]
fn every_flower_is_planted_where_it_belongs() {
    let d = data();
    let region = &d.regions[0];
    let def = region.scenery.features.fields.as_ref().unwrap();
    let (layout, field) = farm(&d);
    let plan = layout.fields.as_ref().unwrap();
    assert_eq!(field.len() as u32, region.flower_total);

    let mut per_species: HashMap<&str, u32> = HashMap::new();
    for i in 0..field.len() as u32 {
        let id = d.species[field.species(i)].id.as_str();
        *per_species.entry(id).or_default() += 1;
        let p = field.position(i);
        let on_bed = plan.bed_at(p).is_some();
        if def.verge_species.iter().any(|s| s == id) {
            assert!(
                !plan.in_planted_area(p, 0.5),
                "wild {id} in the beds at {p}"
            );
        } else {
            assert!(on_bed, "{id} off its bed at {p}");
        }
        assert!(layout.in_bounds(p, 0.5), "{id} outside the fence");
        assert!(
            layout.distance_to_path(p) > layout.path_half_width,
            "{id} on the track"
        );
        assert!(!layout.near_landmark(p, 0.3), "{id} in a landmark");
    }
    for f in &region.flowers {
        assert_eq!(per_species[f.species.as_str()], f.count, "{}", f.species);
    }
}

#[test]
fn beds_are_dense_but_every_flower_stands_alone() {
    let d = data();
    let (_, field) = farm(&d);
    let mut tightest = (f32::MAX, 0, 0);
    for i in 0..field.len() as u32 {
        for j in (i + 1)..field.len() as u32 {
            let dist = field.position(i).distance(field.position(j));
            if dist < tightest.0 {
                tightest = (dist, i, j);
            }
        }
    }
    let (dist, i, j) = tightest;
    assert!(
        dist > 0.35,
        "flowers {i} ({}) and {j} ({}) only {dist} m apart at {}",
        d.species[field.species(i)].id,
        d.species[field.species(j)].id,
        field.position(i)
    );
}

#[test]
fn beds_are_blocks_of_one_colour_with_tall_ones_at_the_back() {
    let d = data();
    let def = d.regions[0].scenery.features.fields.clone().unwrap();
    let (layout, field) = farm(&d);
    let plan = layout.fields.as_ref().unwrap();
    // Most beds hold a single species (strays and the odd change of
    // species halfway along a bed aside).
    let mut single = 0;
    let mut planted = 0;
    for bed in &plan.beds {
        let species: BTreeSet<usize> = (0..field.len() as u32)
            .filter(|&i| {
                let p = field.position(i);
                (p.x - bed.x).abs() <= plan.bed_width / 2.0 && p.y >= bed.z0 && p.y <= bed.z1
            })
            .map(|i| field.species(i))
            .filter(|&s| !def.stray_species.contains(&d.species[s].id))
            .collect();
        if !species.is_empty() {
            planted += 1;
            if species.len() == 1 {
                single += 1;
            }
        }
    }
    assert!(
        planted >= plan.beds.len() * 9 / 10,
        "{planted} of {} beds planted",
        plan.beds.len()
    );
    assert!(
        single * 2 > planted,
        "only {single} of {planted} beds are one colour"
    );

    let mean_distance = |id: &str| {
        let s = d.species_index(id).unwrap();
        let xs: Vec<f32> = (0..field.len() as u32)
            .filter(|&i| field.species(i) == s)
            .map(|i| field.position(i).x.abs())
            .collect();
        xs.iter().sum::<f32>() / xs.len() as f32
    };
    assert!(mean_distance("bluebell") < mean_distance("tulip"));
    assert!(mean_distance("tulip") < mean_distance("foxglove"));
    assert!(mean_distance("foxglove") < mean_distance("sunflower"));
}

#[test]
fn the_farmyard_holds_the_workshop_and_the_track_runs_straight() {
    let d = data();
    let (layout, _) = farm(&d);
    let plan = layout.fields.as_ref().unwrap();
    let workshop = layout
        .landmarks
        .iter()
        .find(|l| l.kind == LandmarkKind::Workshop)
        .expect("the farm has a workshop");
    assert!(workshop.position.y - workshop.footprint > plan.yard_z);
    assert!(layout.spawn.y > plan.yard_z);
    assert!(
        layout.path.iter().all(|p| p.x == layout.spawn.x),
        "straight track"
    );
    assert!(!layout.trees.is_empty());
    assert!(layout.trees.iter().all(|t| t.position.y > plan.yard_z));
    // Lanterns line the track for the night.
    assert!(layout
        .landmarks
        .iter()
        .any(|l| l.kind == LandmarkKind::Lantern));
    // Beds are raised above their furrows.
    let bed = plan.beds[0];
    let crown = Vec2::new(bed.x, (bed.z0 + bed.z1) / 2.0);
    let furrow = crown + Vec2::new(bed.x.signum() * plan.pitch() / 2.0, 0.0);
    assert!(layout.terrain.height_at(crown) > layout.terrain.height_at(furrow) + 0.05);
}

#[test]
fn the_whole_farm_can_be_finished() {
    let d = Arc::new(data());
    let xp = d.progression.level_curve.total_xp_for_level(30);
    let mut g = Game::from_state(
        d.clone(),
        GameState {
            coins: 100_000,
            xp,
            ..Default::default()
        },
    )
    .unwrap();
    let n = g.region(0).field.len() as u32;
    for i in 0..n {
        let (c, _) = g.region(0).field.pick_sphere(i);
        g.tick(1.0);
        g.harvest(0, i, c + Vec3::Y).unwrap();
    }
    g.tick(1_000.0);
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
    assert!(g.completion().is_complete(), "{:?}", g.completion());
    for id in [
        "master_harvester",
        "harvest_home",
        "complete_collection",
        "all_tools",
        "all_required_bouquets",
        "bloom_and_gather",
    ] {
        assert!(
            g.is_achievement_unlocked(d.achievement_index(id).unwrap()),
            "{id}"
        );
    }
}

#[test]
fn a_save_from_the_five_region_valley_is_refused() {
    // Picked flowers in the old Sunny Meadow mean nothing on the farm: the
    // save is refused (the app sets it aside and starts a new game) rather
    // than loaded with the wrong flowers.
    let valley = Arc::new(GameData::from_ron_str(include_str!("fixtures/valley.ron")).unwrap());
    let mut old = Game::new(valley);
    let (c, _) = old.region(0).field.pick_sphere(0);
    old.tick(1.0);
    old.harvest(0, 0, c + Vec3::Y).unwrap();
    let state = old.state().clone();
    assert!(Game::from_state(Arc::new(data()), state).is_err());
}
