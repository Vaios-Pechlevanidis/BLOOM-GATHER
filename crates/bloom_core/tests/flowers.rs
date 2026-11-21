//! Flower placement, spatial queries and look-at picking on the shipped
//! content.

use bloom_core::bitset::BitSet;
use bloom_core::world::{FlowerField, RegionLayout};
use bloom_core::GameData;
use glam::{Vec2, Vec3};

fn data() -> GameData {
    GameData::builtin().unwrap()
}

fn region_field(d: &GameData, i: usize) -> (RegionLayout, FlowerField) {
    let region = &d.regions[i];
    let layout = RegionLayout::generate(region, &d.world);
    let field = FlowerField::generate(region, &layout, d).unwrap_or_else(|e| panic!("{e}"));
    (layout, field)
}

#[test]
fn every_region_places_exactly_its_flowers() {
    let d = data();
    for (i, region) in d.regions.iter().enumerate() {
        let (_, field) = region_field(&d, i);
        assert_eq!(field.len() as u32, region.flower_total, "{}", region.id);
        for entry in &region.flowers {
            let s = d.species_index(&entry.species).unwrap();
            let n = (0..field.len() as u32)
                .filter(|&f| field.species(f) == s)
                .count() as u32;
            assert_eq!(n, entry.count, "{} {}", region.id, entry.species);
        }
    }
}

#[test]
fn placement_is_deterministic() {
    let d = data();
    for i in 0..d.regions.len() {
        let (_, a) = region_field(&d, i);
        let (_, b) = region_field(&d, i);
        for f in 0..a.len() as u32 {
            assert_eq!(a.position(f), b.position(f));
            assert_eq!(a.species(f), b.species(f));
            assert_eq!(a.scale(f), b.scale(f));
        }
    }
}

#[test]
fn flowers_respect_spacing_path_props_and_bounds() {
    let d = data();
    let spacing = d.world.min_flower_spacing;
    for i in 0..d.regions.len() {
        let (layout, field) = region_field(&d, i);
        let id = &d.regions[i].id;
        for a in 0..field.len() as u32 {
            let p = field.position(a);
            assert!(layout.in_bounds(p, 0.99), "{id} flower {a} outside");
            assert!(
                layout.distance_to_path(p) >= layout.path_half_width,
                "{id} flower {a} on path"
            );
            assert!(p.distance(layout.spawn) >= 2.4, "{id} flower {a} on spawn");
            for t in &layout.trees {
                assert!(
                    t.position.distance(p) > t.trunk_radius + 0.5,
                    "{id} flower {a} in tree"
                );
            }
            assert!(
                !layout.near_landmark(p, 0.5),
                "{id} flower {a} in a landmark"
            );
            for r in &layout.rocks {
                assert!(r.position.distance(p) > r.radius, "{id} flower {a} in rock");
            }
            assert!((field.base(a) - layout.terrain.height_at(p)).abs() < 1e-4);
            for b in (a + 1)..field.len() as u32 {
                assert!(
                    p.distance(field.position(b)) >= spacing - 1e-4,
                    "{id} flowers {a}/{b} overlap"
                );
            }
        }
    }
}

#[test]
fn flowers_form_clusters() {
    // Clustered placement: the average nearest same-species neighbour is
    // much closer than for a uniform scatter of the same density.
    let d = data();
    let (layout, field) = region_field(&d, 0);
    let area = (layout.bounds_max - layout.bounds_min).element_product();
    let mut total = 0.0;
    for a in 0..field.len() as u32 {
        let nearest = (0..field.len() as u32)
            .filter(|&b| b != a && field.species(b) == field.species(a))
            .map(|b| field.position(a).distance(field.position(b)))
            .fold(f32::INFINITY, f32::min);
        total += nearest;
    }
    let mean = total / field.len() as f32;
    let uniform = 0.5 * (area / 20.0).sqrt(); // ~20 flowers per species
    assert!(
        mean < uniform * 0.6,
        "mean nearest {mean} vs uniform {uniform}"
    );
}

#[test]
fn radius_query_matches_brute_force() {
    let d = data();
    let (layout, field) = region_field(&d, 2);
    let mut harvested = BitSet::new(field.len() as u32);
    for i in (0..field.len() as u32).step_by(7) {
        harvested.test_and_set(i);
    }
    for (center, r) in [
        (layout.spawn, 6.0),
        (Vec2::new(10.0, -20.0), 9.5),
        (Vec2::ZERO, 30.0),
    ] {
        let fast = field.query_radius(center, r, &harvested);
        let slow: Vec<u32> = (0..field.len() as u32)
            .filter(|&i| !harvested.get(i) && field.position(i).distance(center) <= r)
            .collect();
        assert_eq!(fast, slow, "query at {center} r={r}");
    }
}

#[test]
fn picking_finds_the_flower_looked_at() {
    let d = data();
    let (_, field) = region_field(&d, 0);
    let mut harvested = BitSet::new(field.len() as u32);
    let target = 17;
    let (center, _) = field.pick_sphere(target);
    // Stand 2 m away at eye height and look straight at the bloom.
    let eye = center + Vec3::new(2.0, 1.2, 0.0);
    let dir = (center - eye).normalize();
    let (hit, dist) = field.pick(eye, dir, 3.0, &harvested).expect("should hit");
    // Something may be in front of the target, but nothing farther.
    assert!(dist <= eye.distance(center));
    if hit != target {
        let (c, _) = field.pick_sphere(hit);
        assert!(eye.distance(c) <= eye.distance(center) + 0.5);
    }
    // Out of range: nothing.
    assert!(field.pick(eye + -dir * 5.0, dir, 3.0, &harvested).is_none());
    // Harvested flowers can't be picked.
    harvested.test_and_set(hit);
    if let Some((again, _)) = field.pick(eye, dir, 3.0, &harvested) {
        assert_ne!(again, hit);
    }
}

#[test]
fn looking_at_the_sky_picks_nothing() {
    let d = data();
    let (layout, field) = region_field(&d, 0);
    let harvested = BitSet::new(field.len() as u32);
    let eye = Vec3::new(layout.spawn.x, 2.0, layout.spawn.y);
    assert!(field.pick(eye, Vec3::Y, 3.0, &harvested).is_none());
}
