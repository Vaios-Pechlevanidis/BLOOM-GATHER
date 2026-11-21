//! Character controller and region layout behaviour, using the shipped
//! player tuning so the tests describe what the player actually feels.

use bloom_core::data::PlayerConfig;
use bloom_core::player::{CharacterBody, Environment, MoveInput};
use bloom_core::world::{
    BoxCollider, CircleCollider, ColliderSet, LandmarkKind, RegionLayout, Terrain,
};
use bloom_core::GameData;
use glam::{Vec2, Vec3};

const DT: f32 = 1.0 / 64.0;

fn data() -> GameData {
    GameData::builtin().unwrap()
}

fn cfg() -> PlayerConfig {
    data().player
}

fn flat() -> Terrain {
    Terrain::flat(100.0, 100.0, 0.5, 0.0)
}

fn run(body: &mut CharacterBody, input: MoveInput, secs: f32, env: &Environment) {
    let cfg = cfg();
    let steps = (secs / DT).round() as usize;
    for _ in 0..steps {
        body.step(&input, DT, &cfg, env);
    }
}

fn forward() -> MoveInput {
    MoveInput {
        wish: Vec2::new(0.0, -1.0),
        ..Default::default()
    }
}

#[test]
fn falls_under_gravity_and_lands() {
    let terrain = flat();
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    body.position.y = 5.0;
    body.grounded = false;
    run(&mut body, MoveInput::default(), 2.0, &env);
    assert!(body.grounded);
    assert!(body.position.y.abs() < 1e-5);
    assert_eq!(body.velocity.y, 0.0);
}

#[test]
fn jump_reaches_configured_height_and_lands() {
    let terrain = flat();
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let cfg = cfg();
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    let events = body.step(
        &MoveInput {
            jump: true,
            ..Default::default()
        },
        DT,
        &cfg,
        &env,
    );
    assert!(events.jumped);
    let mut apex: f32 = 0.0;
    let mut landed = false;
    for _ in 0..200 {
        let e = body.step(&MoveInput::default(), DT, &cfg, &env);
        apex = apex.max(body.position.y);
        if e.landed {
            landed = true;
            break;
        }
    }
    assert!(landed, "never landed");
    let err = (apex - cfg.jump_height).abs() / cfg.jump_height;
    assert!(err < 0.05, "apex {apex} vs {}", cfg.jump_height);
}

#[test]
fn cannot_jump_in_mid_air() {
    let terrain = flat();
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let cfg = cfg();
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    body.step(
        &MoveInput {
            jump: true,
            ..Default::default()
        },
        DT,
        &cfg,
        &env,
    );
    run(&mut body, MoveInput::default(), 0.2, &env);
    let e = body.step(
        &MoveInput {
            jump: true,
            ..Default::default()
        },
        DT,
        &cfg,
        &env,
    );
    assert!(!e.jumped, "double jump");
}

#[test]
fn jump_pressed_just_before_landing_is_buffered() {
    let terrain = flat();
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let cfg = cfg();
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    body.position.y = 0.3;
    body.grounded = false;
    body.velocity.y = -6.0; // lands in ~3 steps
    body.step(
        &MoveInput {
            jump: true,
            ..Default::default()
        },
        DT,
        &cfg,
        &env,
    );
    let mut jumped = false;
    for _ in 0..8 {
        if body.step(&MoveInput::default(), DT, &cfg, &env).jumped {
            jumped = true;
        }
    }
    assert!(jumped, "buffered jump was lost");
}

#[test]
fn walks_and_sprints_at_configured_speeds() {
    let terrain = flat();
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let cfg = cfg();
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    run(&mut body, forward(), 0.5, &env);
    assert!((body.horizontal_speed() - cfg.walk_speed).abs() < 1e-3);
    assert!(body.velocity.z < 0.0, "forward is −Z");
    run(
        &mut body,
        MoveInput {
            sprint: true,
            ..forward()
        },
        0.5,
        &env,
    );
    assert!((body.horizontal_speed() - cfg.sprint_speed).abs() < 1e-3);
}

#[test]
fn stops_quickly_without_input() {
    let terrain = flat();
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    run(&mut body, forward(), 1.0, &env);
    run(&mut body, MoveInput::default(), 0.3, &env);
    assert_eq!(body.horizontal_speed(), 0.0);
}

#[test]
fn cannot_walk_through_a_tree() {
    let terrain = flat();
    let tree = CircleCollider {
        center: Vec2::new(0.0, -5.0),
        radius: 0.4,
        bottom: -1.0,
        top: 8.0,
    };
    let colliders = ColliderSet {
        circles: vec![tree],
        boxes: vec![],
    };
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let cfg = cfg();
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    for _ in 0..(4.0 / DT) as usize {
        body.step(&forward(), DT, &cfg, &env);
        let d = body.ground_position().distance(tree.center);
        assert!(d >= tree.radius + cfg.radius - 1e-3, "inside tree: {d}");
    }
}

#[test]
fn slides_along_a_wall() {
    let terrain = flat();
    let wall = BoxCollider {
        center: Vec2::new(0.0, -3.0),
        half_extents: Vec2::new(20.0, 0.25),
        angle: 0.0,
        bottom: -1.0,
        top: 3.0,
    };
    let colliders = ColliderSet {
        circles: vec![],
        boxes: vec![wall],
    };
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    let diagonal = MoveInput {
        wish: Vec2::new(1.0, -1.0).normalize(),
        ..Default::default()
    };
    run(&mut body, diagonal, 3.0, &env);
    assert!(
        body.position.z > -3.0 + 0.25,
        "went through wall: {}",
        body.position
    );
    assert!(body.position.x > 5.0, "did not slide: {}", body.position);
}

#[test]
fn steps_onto_low_rocks_but_not_boulders() {
    let terrain = flat();
    let pebble = CircleCollider {
        center: Vec2::new(0.0, -3.0),
        radius: 0.8,
        bottom: -1.0,
        top: 0.25,
    };
    let boulder = CircleCollider {
        center: Vec2::new(10.0, -3.0),
        radius: 0.8,
        bottom: -1.0,
        top: 1.0,
    };
    let colliders = ColliderSet {
        circles: vec![pebble, boulder],
        boxes: vec![],
    };
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };

    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    run(&mut body, forward(), 0.7, &env); // ~3 m: on top of the pebble
    assert!(
        (body.position.y - 0.25).abs() < 1e-4,
        "not on pebble: {}",
        body.position
    );
    assert!(body.grounded);

    let mut body = CharacterBody::standing_at(Vec2::new(10.0, 0.0), &env);
    run(&mut body, forward(), 2.0, &env);
    assert_eq!(body.position.y, 0.0);
    assert!(body.ground_position().distance(boulder.center) >= 0.8 + cfg().radius - 1e-3);
}

#[test]
fn can_jump_onto_a_boulder() {
    let terrain = flat();
    let boulder = CircleCollider {
        center: Vec2::new(0.0, -1.6),
        radius: 1.0,
        bottom: -1.0,
        top: 0.8,
    };
    let colliders = ColliderSet {
        circles: vec![boulder],
        boxes: vec![],
    };
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let cfg = cfg();
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    body.step(
        &MoveInput {
            jump: true,
            ..forward()
        },
        DT,
        &cfg,
        &env,
    );
    run(&mut body, forward(), 0.4, &env);
    run(&mut body, MoveInput::default(), 1.0, &env);
    assert!(body.grounded);
    assert!(
        (body.position.y - 0.8).abs() < 1e-4,
        "not on boulder: {}",
        body.position
    );
}

#[test]
fn follows_ground_downhill_without_bouncing() {
    // A gentle 10% slope falling away towards −Z (the walking direction).
    let terrain = slope(Terrain::flat(100.0, 100.0, 0.5, 0.0), -0.1);
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let cfg = cfg();
    let mut body = CharacterBody::standing_at(Vec2::new(0.0, 0.0), &env);
    for _ in 0..(2.0 / DT) as usize {
        body.step(
            &MoveInput {
                sprint: true,
                ..forward()
            },
            DT,
            &cfg,
            &env,
        );
        assert!(body.grounded, "lost ground at {}", body.position);
    }
}

#[test]
fn refuses_to_climb_a_cliff() {
    let terrain = slope(Terrain::flat(100.0, 100.0, 0.5, 0.0), 2.0); // ~63° uphill
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let mut body = CharacterBody::standing_at(Vec2::ZERO, &env);
    let start = body.position;
    run(&mut body, forward(), 2.0, &env);
    assert!(
        body.position.y - start.y < 0.5,
        "climbed cliff to {}",
        body.position
    );
}

/// Same grid as `terrain`, with heights rising by `gradient` per metre
/// towards −Z (negative = falling away).
fn slope(terrain: Terrain, gradient: f32) -> Terrain {
    let (nx, nz) = terrain.samples();
    let cell = terrain.cell();
    let mut heights = Vec::with_capacity(nx * nz);
    for iz in 0..nz {
        for ix in 0..nx {
            let p = terrain.sample_pos(ix, iz);
            heights.push(-p.y * gradient);
        }
    }
    Terrain::from_heights(terrain.sample_pos(0, 0), cell, nx, nz, heights)
}

// ---------------------------------------------------------------------------
// Region layouts
// ---------------------------------------------------------------------------

#[test]
fn layouts_are_deterministic_and_complete() {
    let d = data();
    for region in &d.regions {
        let a = RegionLayout::generate(region, &d.world);
        let b = RegionLayout::generate(region, &d.world);
        assert_eq!(a.trees, b.trees, "{}", region.id);
        assert_eq!(a.rocks, b.rocks, "{}", region.id);
        assert_eq!(a.backdrop_trees, b.backdrop_trees, "{}", region.id);
        assert!(!a.backdrop_trees.is_empty(), "{} has no scenery", region.id);
        for t in &a.backdrop_trees {
            assert!(
                !a.in_bounds(t.position, -2.0),
                "{} scenery tree inside the fence",
                region.id
            );
        }
        assert_eq!(
            a.trees.len(),
            region.props.trees as usize,
            "{} trees",
            region.id
        );
        assert_eq!(
            a.rocks.len(),
            region.props.rocks as usize,
            "{} rocks",
            region.id
        );
    }
}

#[test]
fn spawn_is_clear_flat_and_inside_bounds() {
    let d = data();
    let cfg = d.player.clone();
    for region in &d.regions {
        let layout = RegionLayout::generate(region, &d.world);
        let env = Environment {
            terrain: &layout.terrain,
            colliders: &layout.colliders,
        };
        let mut body = CharacterBody::standing_at(layout.spawn, &env);
        assert!(layout.in_bounds(layout.spawn, cfg.radius), "{}", region.id);
        let before = body.position;
        run(&mut body, MoveInput::default(), 0.5, &env);
        assert!(
            (body.position - before).length() < 1e-4,
            "{} spawn not stable",
            region.id
        );
        assert!(
            layout.terrain.normal_at(layout.spawn).y > 0.99,
            "{} spawn not flat",
            region.id
        );
    }
}

#[test]
fn props_stay_off_the_path_and_inside_bounds() {
    let d = data();
    for region in &d.regions {
        let layout = RegionLayout::generate(region, &d.world);
        for t in &layout.trees {
            assert!(
                layout.distance_to_path(t.position) >= 3.5,
                "{} tree on path",
                region.id
            );
            assert!(layout.in_bounds(t.position, 0.0));
        }
        for r in &layout.rocks {
            assert!(
                layout.distance_to_path(r.position) >= 1.8,
                "{} rock on path",
                region.id
            );
            assert!(layout.in_bounds(r.position, 0.0));
        }
    }
}

#[test]
fn player_cannot_leave_the_region() {
    let d = data();
    let region = &d.regions[0];
    let layout = RegionLayout::generate(region, &d.world);
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let cfg = d.player.clone();
    for dir in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y, Vec2::ONE.normalize()] {
        let mut body = CharacterBody::standing_at(layout.spawn, &env);
        for _ in 0..(60.0 / DT) as usize {
            body.step(
                &MoveInput {
                    wish: dir,
                    sprint: true,
                    jump: true,
                },
                DT,
                &cfg,
                &env,
            );
        }
        assert!(
            layout.in_bounds(body.ground_position(), cfg.radius - 1e-3),
            "escaped towards {dir}: {}",
            body.position
        );
        assert!(body.position.y > -1.0 && body.position.y.is_finite());
    }
}

#[test]
fn eye_is_above_feet() {
    let cfg = cfg();
    let terrain = flat();
    let colliders = ColliderSet::default();
    let env = Environment {
        terrain: &terrain,
        colliders: &colliders,
    };
    let body = CharacterBody::standing_at(Vec2::ZERO, &env);
    assert_eq!(body.eye(&cfg), Vec3::new(0.0, cfg.eye_height, 0.0));
}

#[test]
fn every_region_has_landmarks_off_the_path_and_solid() {
    let d = data();
    let cfg = d.player.clone();
    for region in &d.regions {
        let layout = RegionLayout::generate(region, &d.world);
        let kinds: Vec<_> = layout.landmarks.iter().map(|l| l.kind).collect();
        for kind in [
            LandmarkKind::Bench,
            LandmarkKind::Well,
            LandmarkKind::Signpost,
        ] {
            assert!(kinds.contains(&kind), "{} lacks {kind:?}", region.id);
        }
        let env = Environment {
            terrain: &layout.terrain,
            colliders: &layout.colliders,
        };
        for l in &layout.landmarks {
            assert!(
                layout.in_bounds(l.position, 0.5),
                "{} {:?} outside",
                region.id,
                l.kind
            );
            assert!(
                layout.distance_to_path(l.position) > 1.3,
                "{} {:?} on path",
                region.id,
                l.kind
            );
            assert!((layout.terrain.height_at(l.position) - l.base).abs() < 1e-3);
            for t in &layout.trees {
                assert!(
                    t.position.distance(l.position) > l.footprint + 2.9,
                    "{} tree in {:?}",
                    region.id,
                    l.kind
                );
            }
            // Walk straight at it from 4 m: it must block.
            let start = l.position + Vec2::new(0.0, 4.0);
            if !layout.in_bounds(start, 0.5) {
                continue;
            }
            let mut body = CharacterBody::standing_at(start, &env);
            for _ in 0..(3.0 / DT) as usize {
                body.step(
                    &MoveInput {
                        wish: Vec2::new(0.0, -1.0),
                        ..Default::default()
                    },
                    DT,
                    &cfg,
                    &env,
                );
            }
            assert!(
                body.ground_position().distance(l.position) > 0.3,
                "{} walked into the {:?}",
                region.id,
                l.kind
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The workshop
// ---------------------------------------------------------------------------

use bloom_core::world::layout::{to_world, workshop};
use bloom_core::world::InteractableKind;

/// World (x, z) → the workshop's local frame.
fn to_local(origin: Vec2, yaw: f32, p: Vec2) -> Vec2 {
    let d = p - origin;
    let (s, c) = yaw.sin_cos();
    Vec2::new(d.x * c - d.y * s, d.x * s + d.y * c)
}

fn meadow() -> (GameData, RegionLayout) {
    let d = data();
    let layout = RegionLayout::generate(&d.regions[0], &d.world);
    (d, layout)
}

#[test]
fn only_the_meadow_has_the_workshop_with_four_stations() {
    let d = data();
    for (i, region) in d.regions.iter().enumerate() {
        let layout = RegionLayout::generate(region, &d.world);
        let has = layout
            .landmarks
            .iter()
            .any(|l| l.kind == LandmarkKind::Workshop);
        assert_eq!(has, i == 0, "{}", region.id);
        assert_eq!(layout.interactables.len(), if i == 0 { 4 } else { 0 });
    }
    let (_, layout) = meadow();
    let w = layout
        .landmarks
        .iter()
        .find(|l| l.kind == LandmarkKind::Workshop)
        .unwrap();
    assert!(
        layout.distance_to_path(w.position) > 7.0,
        "workshop on the path"
    );
    assert!(layout.in_bounds(w.position, 5.0));
    // Its door faces the spawn.
    let door = to_world(w.position, w.yaw, Vec2::new(0.0, workshop::HALF.y));
    assert!(door.distance(layout.spawn) < w.position.distance(layout.spawn));
}

#[test]
fn the_door_lets_you_in_and_the_walls_do_not() {
    let (d, layout) = meadow();
    let cfg = d.player.clone();
    let env = Environment {
        terrain: &layout.terrain,
        colliders: &layout.colliders,
    };
    let w = *layout
        .landmarks
        .iter()
        .find(|l| l.kind == LandmarkKind::Workshop)
        .unwrap();
    let walk = |from_local: Vec2, to_local_pt: Vec2, secs: f32| {
        let from = to_world(w.position, w.yaw, from_local);
        let to = to_world(w.position, w.yaw, to_local_pt);
        let mut body = CharacterBody::standing_at(from, &env);
        for _ in 0..(secs / DT) as usize {
            let wish = (to - body.ground_position()).normalize_or_zero();
            body.step(
                &MoveInput {
                    wish,
                    ..Default::default()
                },
                DT,
                &cfg,
                &env,
            );
        }
        to_local(w.position, w.yaw, body.ground_position())
    };
    // Through the door to the middle of the room.
    let inside = walk(Vec2::new(0.0, 6.5), Vec2::new(0.0, 0.0), 4.0);
    assert!(inside.length() < 0.5, "didn't get in: {inside}");
    // Straight at the front wall beside the door: stopped outside.
    let blocked = walk(Vec2::new(2.3, 6.5), Vec2::new(2.3, 0.0), 4.0);
    assert!(
        blocked.y > workshop::HALF.y,
        "walked through the wall: {blocked}"
    );
    // And from behind.
    let back = walk(Vec2::new(0.0, -6.5), Vec2::new(0.0, 0.0), 4.0);
    assert!(
        back.y < -workshop::HALF.y,
        "walked through the back wall: {back}"
    );
}

#[test]
fn looking_at_the_workbench_targets_it() {
    let (d, layout) = meadow();
    let bench = layout
        .interactables
        .iter()
        .find(|i| i.kind == InteractableKind::Workbench)
        .unwrap();
    let w = *layout
        .landmarks
        .iter()
        .find(|l| l.kind == LandmarkKind::Workshop)
        .unwrap();
    let stand = to_world(w.position, w.yaw, Vec2::new(0.0, -0.6));
    let eye = Vec3::new(stand.x, w.base + d.player.eye_height, stand.y);
    let hit = layout.pick_interactable(eye, bench.position - eye, d.player.interact_range);
    assert_eq!(hit.map(|h| h.0), Some(InteractableKind::Workbench));
    // Looking away: nothing.
    assert_eq!(
        layout.pick_interactable(eye, eye - bench.position, d.player.interact_range),
        None
    );
}
