//! Phase F: the five regions as places — gates between them, water,
//! bridges, lanterns, flower rows — and travelling from one to the next.
//!
//! Walking tests drive the real character controller with the shipped
//! tuning, so "you can always get through" is checked the way a player
//! would experience it.

use std::sync::Arc;

use bloom_core::game::{GameEvent, GameState};
use bloom_core::player::{CharacterBody, Environment, MoveInput};
use bloom_core::world::{FlowerField, GateSide, LandmarkKind, RegionLayout, WaterKind};
use bloom_core::{Game, GameData, GameError};
use glam::Vec2;

const DT: f32 = 1.0 / 64.0;

fn data() -> GameData {
    GameData::from_ron_str(include_str!("fixtures/valley.ron")).unwrap()
}

fn layouts(d: &GameData) -> Vec<RegionLayout> {
    d.regions
        .iter()
        .map(|r| RegionLayout::generate(r, &d.world))
        .collect()
}

fn index(d: &GameData, id: &str) -> usize {
    d.region_index(id).unwrap()
}

/// Walk along `route` with simple steering; true once `done` holds.
fn walk(
    d: &GameData,
    layout: &RegionLayout,
    start: Vec2,
    route: &[Vec2],
    done: impl Fn(Vec2) -> bool,
) -> Result<(), String> {
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let mut body = CharacterBody::standing_at(start, &env);
    let mut target = 0;
    let mut stuck = 0;
    let mut last = body.ground_position();
    for step in 0..64 * 240 {
        let at = body.ground_position();
        if done(at) {
            return Ok(());
        }
        while target + 1 < route.len() && at.distance(route[target]) < 1.5 {
            target += 1;
        }
        let wish = (route[target] - at).normalize_or_zero();
        body.step(
            &MoveInput {
                wish,
                ..Default::default()
            },
            DT,
            &d.player,
            &env,
        );
        if step % 64 == 63 {
            // Less than half a metre in a whole second: stuck.
            if body.ground_position().distance(last) < 0.5 {
                stuck += 1;
                if stuck >= 3 {
                    return Err(format!("stuck at {at:?} heading for {:?}", route[target]));
                }
            } else {
                stuck = 0;
            }
            last = body.ground_position();
        }
    }
    Err(format!(
        "never arrived (ended at {:?})",
        body.ground_position()
    ))
}

#[test]
fn gates_sit_where_the_path_meets_the_fence() {
    let d = data();
    for (layout, region) in layouts(&d).iter().zip(&d.regions) {
        let north = layout.gate(GateSide::North);
        let south = layout.gate(GateSide::South);
        assert_eq!(north.position.y, layout.bounds_min.y, "{}", region.id);
        assert_eq!(south.position.y, layout.bounds_max.y, "{}", region.id);
        assert!((north.position.x - layout.path.last().unwrap().x).abs() < 1e-4);
        assert!((south.position.x - layout.path[0].x).abs() < 1e-4);
        for side in [GateSide::North, GateSide::South] {
            let g = layout.gate(side);
            assert!(g.position.x > layout.bounds_min.x + 2.0);
            assert!(g.position.x < layout.bounds_max.x - 2.0);
            // The arrival spot is open ground, well clear of the trigger.
            let (at, _) = layout.arrival(side);
            assert!(layout.in_bounds(at, 1.0), "{} {side:?}", region.id);
            assert!(layout.gate_at(at).is_none(), "{} {side:?}", region.id);
            assert!(!layout.in_water(at, 0.5));
            let feet = layout.terrain.height_at(at);
            let (p, hit) = layout.colliders.resolve(
                at,
                d.player.radius,
                bloom_core::world::BodySlab {
                    feet,
                    step: d.player.step_height,
                    head: feet + d.player.height,
                },
            );
            assert!(
                hit.is_none(),
                "{} {side:?} arrival touches something",
                region.id
            );
            assert!(
                p.distance(at) < 1e-4,
                "{} {side:?} arrival is inside something",
                region.id
            );
        }
    }
}

#[test]
fn every_region_can_be_walked_from_gate_to_gate() {
    let d = data();
    let ls = layouts(&d);
    for (r, (layout, region)) in ls.iter().zip(&d.regions).enumerate() {
        // Spawn (the south arrival) → along the path → into the north gate.
        let mut route = layout.path.clone();
        route.push(layout.gate(GateSide::North).position);
        walk(&d, layout, layout.spawn, &route, |p| {
            layout.gate_at(p) == Some(GateSide::North)
        })
        .unwrap_or_else(|e| panic!("{} northwards: {e}", region.id));
        if r == 0 {
            continue; // nothing south of the meadow
        }
        // And back: from the north arrival down the path to the south gate.
        let (from, _) = layout.arrival(GateSide::North);
        let mut back: Vec<Vec2> = layout.path.iter().rev().copied().collect();
        back.push(layout.gate(GateSide::South).position);
        walk(&d, layout, from, &back, |p| {
            layout.gate_at(p) == Some(GateSide::South)
        })
        .unwrap_or_else(|e| panic!("{} southwards: {e}", region.id));
    }
}

#[test]
fn blossom_valley_has_a_stream_with_a_bridge_on_the_path() {
    let d = data();
    let ls = layouts(&d);
    let valley = &ls[index(&d, "blossom_valley")];
    let stream: Vec<_> = valley
        .water
        .iter()
        .filter(|w| w.kind == WaterKind::Stream)
        .collect();
    assert_eq!(stream.len(), 1);
    assert!(
        !valley.bridges.is_empty(),
        "the path crosses the stream on a bridge"
    );
    for b in &valley.bridges {
        // Both ends stand on dry ground, and the deck spans the water.
        assert!(!valley.in_water(b.point(0.0), 0.0));
        assert!(!valley.in_water(b.point(1.0), 0.0));
        assert!(valley.distance_to_path(b.center) < 1.0);
        // Standing in the middle you are on the deck, above the water.
        let env = Environment {
            terrain: &valley.terrain,
            colliders: &valley.colliders,
        };
        let mut body = CharacterBody::standing_at(b.point(0.0) - b.dir * 1.5, &env);
        let mut lowest_over_water = f32::INFINITY;
        for _ in 0..64 * 6 {
            body.step(
                &MoveInput {
                    wish: b.dir,
                    ..Default::default()
                },
                DT,
                &d.player,
                &env,
            );
            let at = body.ground_position();
            if valley.water_level_at(at).is_some() {
                lowest_over_water = lowest_over_water.min(body.position.y - stream[0].level);
            }
        }
        let past = (body.ground_position() - b.center).dot(b.dir);
        assert!(past > b.half_length, "crossed the bridge ({past} m along)");
        assert!(
            lowest_over_water > 0.3,
            "walked on the deck, {lowest_over_water} above the water"
        );
        // The deck climbs in steps far below the step height.
        let mut prev = b.deck_height(0.0);
        for k in 1..=40 {
            let h = b.deck_height(k as f32 / 40.0);
            assert!((h - prev).abs() < d.player.step_height * 0.5);
            prev = h;
        }
    }
    // Only the valley has a stream; only the garden a pond.
    for (layout, region) in ls.iter().zip(&d.regions) {
        let has = |k| layout.water.iter().any(|w| w.kind == k);
        assert_eq!(
            has(WaterKind::Stream),
            region.scenery.features.stream.is_some()
        );
        assert_eq!(
            has(WaterKind::Pond),
            region.scenery.features.pond.is_some(),
            "{}",
            region.id
        );
    }
}

#[test]
fn water_is_shallow_with_walkable_banks_and_no_flowers() {
    let d = data();
    let max_slope = d.player.max_slope_deg.to_radians().cos();
    for (layout, region) in layouts(&d).iter().zip(&d.regions) {
        let field = FlowerField::generate(region, layout, &d).unwrap();
        for w in &layout.water {
            assert!(w.depth <= d.player.height * 0.6, "{}", region.id);
            for i in 0..field.len() as u32 {
                assert!(
                    !layout.in_water(field.position(i), 0.0),
                    "{}: flower in water",
                    region.id
                );
            }
            // Sample the carved area: every slope is walkable.
            let reach = w.outer_radius() + 15.0;
            let centres: Vec<Vec2> = match w.kind {
                WaterKind::Pond => w.points.clone(),
                WaterKind::Stream => w.points.iter().step_by(3).copied().collect(),
            };
            for c in centres {
                if !layout.in_bounds(c, 0.0) {
                    continue;
                }
                for k in 0..24 {
                    let a = k as f32 / 24.0 * std::f32::consts::TAU;
                    let mut r = 0.0;
                    while r < reach {
                        let p = c + Vec2::new(a.cos(), a.sin()) * r;
                        if layout.in_bounds(p, 0.0) {
                            let n = layout.terrain.normal_at(p);
                            assert!(n.y >= max_slope, "{}: steep bank at {p:?}", region.id);
                        }
                        r += 0.5;
                    }
                }
            }
        }
    }
}

#[test]
fn you_can_wade_out_of_any_water() {
    let d = data();
    for (layout, region) in layouts(&d).iter().zip(&d.regions) {
        for w in &layout.water {
            // Deepest inside the region: the stream's middle at x = 0, or the
            // pond's centre.
            let middle = match w.kind {
                WaterKind::Pond => w.points[0],
                WaterKind::Stream => *w
                    .points
                    .iter()
                    .min_by(|a, b| a.x.abs().total_cmp(&b.x.abs()))
                    .unwrap(),
            };
            assert!(layout.water_level_at(middle).is_some(), "{}", region.id);
            for dir in [Vec2::Y, Vec2::NEG_Y, Vec2::X, Vec2::NEG_X] {
                let goal = middle + dir * (w.outer_radius() + 3.0);
                if !layout.in_bounds(goal, 1.0) || layout.in_water(goal, 0.0) {
                    continue;
                }
                walk(&d, layout, middle, &[goal], |p| !layout.in_water(p, 0.0))
                    .unwrap_or_else(|e| panic!("{} wading {dir:?}: {e}", region.id));
            }
        }
    }
}

#[test]
fn lavender_grows_in_rows() {
    let d = data();
    let r = index(&d, "lavender_fields");
    let region = &d.regions[r];
    let rows = region.scenery.features.rows.unwrap();
    let layout = RegionLayout::generate(region, &d.world);
    let field = FlowerField::generate(region, &layout, &d).unwrap();
    let a = rows.angle_deg.to_radians();
    let across = Vec2::new(-a.sin(), a.cos());
    for i in 0..field.len() as u32 {
        let v = field.position(i).dot(across) / rows.spacing;
        assert!(
            (v - v.round()).abs() * rows.spacing < 0.2,
            "flower {i} off its row"
        );
    }
}

#[test]
fn lanterns_line_the_moonlit_path() {
    let d = data();
    let ls = layouts(&d);
    for (layout, region) in ls.iter().zip(&d.regions) {
        let lanterns: Vec<_> = layout
            .landmarks
            .iter()
            .filter(|l| l.kind == LandmarkKind::Lantern)
            .collect();
        let wanted = region.scenery.features.lanterns as usize;
        assert!(lanterns.len() <= wanted);
        assert!(
            lanterns.len() * 3 >= wanted * 2,
            "{}: only {} lanterns",
            region.id,
            lanterns.len()
        );
        for l in lanterns {
            let off = layout.distance_to_path(l.position);
            assert!(off > layout.path_half_width && off < layout.path_half_width + 2.0);
            assert!(!layout.in_water(l.position, 0.0));
        }
    }
}

#[test]
fn forest_trees_are_tall() {
    let d = data();
    let r = index(&d, "enchanted_forest");
    let layout = RegionLayout::generate(&d.regions[r], &d.world);
    let (lo, hi) = d.regions[r].scenery.features.tree_height.unwrap();
    assert!(layout.trees.len() >= 100);
    assert!(layout.trees.iter().all(|t| (lo..=hi).contains(&t.height)));
}

#[test]
fn gates_lead_along_the_chain() {
    let d = Arc::new(data());
    let g = Game::new(d.clone());
    let last = d.regions.len() - 1;
    assert_eq!(g.gate_destination(0, GateSide::South), None);
    assert_eq!(g.gate_destination(0, GateSide::North), Some(1));
    assert_eq!(g.gate_destination(2, GateSide::South), Some(1));
    assert_eq!(g.gate_destination(last, GateSide::North), None);
    assert_eq!(g.arrival_side(0, 1), Some(GateSide::South));
    assert_eq!(g.arrival_side(2, 1), Some(GateSide::North));
    assert_eq!(g.arrival_side(0, 3), None);
}

#[test]
fn locked_regions_cannot_be_entered() {
    let d = Arc::new(data());
    let mut g = Game::new(d.clone());
    let lavender = index(&d, "lavender_fields");
    assert_eq!(g.enter_region(lavender), Err(GameError::RegionLocked));
    assert_eq!(g.enter_region(99), Err(GameError::UnknownRegion));
    let req = g.region_requirements(lavender);
    assert!(!req.met());
    assert_eq!(req.level, (d.regions[lavender].unlock.level, 1));
    assert_eq!(req.flowers.1, 0);
    assert!(!g.has_visited(lavender));
    assert!(g.has_visited(0));
}

#[test]
fn entering_an_open_region_is_announced_once_and_saved() {
    let d = Arc::new(data());
    let lavender = index(&d, "lavender_fields");
    let mut g = Game::from_state(
        d.clone(),
        GameState {
            xp: d.progression.level_curve.total_xp_for_level(20),
            ..Default::default()
        },
    )
    .unwrap();
    // Unlocking also needs harvested flowers.
    assert!(!g.is_region_unlocked(lavender));
    let meadow = g.region(0).field.clone();
    for i in 0..meadow.len() as u32 {
        let (c, _) = meadow.pick_sphere(i);
        g.tick(1.0);
        g.harvest(0, i, c + glam::Vec3::Y).unwrap();
    }
    assert!(g.is_region_unlocked(lavender));
    assert!(g.region_requirements(lavender).met());
    g.drain_events();
    g.enter_region(lavender).unwrap();
    assert_eq!(
        g.drain_events(),
        vec![GameEvent::RegionEntered {
            region: lavender,
            first_visit: true
        }]
    );
    assert!(g.has_visited(lavender));
    assert!(g.is_dirty());
    g.enter_region(lavender).unwrap();
    assert_eq!(
        g.drain_events(),
        vec![GameEvent::RegionEntered {
            region: lavender,
            first_visit: false
        }]
    );
    // Survives a save; a save claiming a visit to a locked region doesn't.
    let restored = Game::from_state(d.clone(), g.state().clone()).unwrap();
    assert!(restored.has_visited(lavender));
    let mut forged = GameState::default();
    forged.visited_regions.insert("moonlight_garden".into());
    assert!(Game::from_state(d, forged).is_err());
}
