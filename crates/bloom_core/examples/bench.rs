//! Core benchmark: how the rules scale with the number of flowers.
//!
//! `cargo run --release -p bloom_core --example bench [sizes...]`
//!
//! For each size it builds one synthetic region (see `bloom_core::bench`)
//! and times layout, flower placement, radius and area queries, look-at
//! picking, harvesting, and saving/loading a fully harvested world.

use std::sync::Arc;
use std::time::Instant;

use bloom_core::bench::{benchmark_data, SIZES};
use bloom_core::persistence::{decode, encode};
use bloom_core::rng::SplitMix64;
use bloom_core::world::{FlowerField, RegionLayout};
use bloom_core::{Game, GameState};
use glam::{Vec2, Vec3};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let sizes: Vec<u32> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let sizes = if sizes.is_empty() {
        SIZES.to_vec()
    } else {
        sizes
    };
    println!(
        "{:>8} | {:>9} {:>9} | {:>9} {:>9} {:>9} | {:>9} {:>9} | {:>9} {:>9} {:>9}",
        "flowers",
        "layout",
        "place",
        "radius*",
        "rect*",
        "pick*",
        "harv",
        "area",
        "encode",
        "decode",
        "restore"
    );
    println!(
        "{:>8} | {:>19} | {:>29} | {:>19} | {:>29}",
        "", "ms", "us per call", "ms", "ms"
    );
    for n in sizes {
        let data = Arc::new(benchmark_data(n).expect("benchmark data"));
        let region = &data.regions[0];

        let t = Instant::now();
        let layout = RegionLayout::generate(region, &data.world);
        let layout_ms = ms(t);
        let t = Instant::now();
        let field = FlowerField::generate(region, &layout, &data).expect("placement");
        let place_ms = ms(t);
        assert_eq!(field.len() as u32, n);

        // Queries at random spots (per call, microseconds).
        let none = bloom_core::bitset::BitSet::new(n);
        let mut rng = SplitMix64::new(1);
        let mut spot = || {
            Vec2::new(
                rng.range_f32(layout.bounds_min.x, layout.bounds_max.x),
                rng.range_f32(layout.bounds_min.y, layout.bounds_max.y),
            )
        };
        let calls = 20_000;
        let points: Vec<Vec2> = (0..calls).map(|_| spot()).collect();
        let t = Instant::now();
        let mut found = 0;
        for p in &points {
            found += field.query_radius(*p, 2.0, &none).len();
        }
        let radius_us = ms(t) * 1000.0 / calls as f64;
        let t = Instant::now();
        for p in &points {
            found += field.query_rect(*p, *p + Vec2::splat(8.0), &none).len();
        }
        let rect_us = ms(t) * 1000.0 / calls as f64;
        let t = Instant::now();
        for p in &points {
            let eye = Vec3::new(p.x, layout.terrain.height_at(*p) + 1.6, p.y);
            if field
                .pick(eye, Vec3::new(0.3, -0.5, -1.0), 3.0, &none)
                .is_some()
            {
                found += 1;
            }
        }
        let pick_us = ms(t) * 1000.0 / calls as f64;
        std::hint::black_box(found);

        // Harvesting through the game: 1,000 single picks, then a big area.
        let mut game = Game::new(data.clone());
        let shared = game.region(0).field.clone();
        let t = Instant::now();
        for i in 0..(n / 2).min(1_000) {
            let (c, _) = shared.pick_sphere(i);
            game.tick(1.0);
            game.harvest(0, i, c + Vec3::Y).unwrap();
        }
        let harvest_ms = ms(t);
        let state = GameState {
            tools: data.tools.iter().map(|t| t.id.clone()).collect(),
            xp: data.progression.level_curve.total_xp_for_level(30),
            ..game.state().clone()
        };
        let mut game = Game::from_state(data.clone(), state).unwrap();
        game.tick(10.0);
        // An 8 x 8 m patch around a flower that is still blooming.
        let center = shared.position(n - 1);
        let eye = Vec3::new(
            center.x,
            layout.terrain.height_at(center) + 1.6,
            center.y + 3.0,
        );
        let t = Instant::now();
        let area = game.harvest_area(0, center - Vec2::splat(4.0), center + Vec2::splat(4.0), eye);
        let area_ms = ms(t);
        let gathered = area
            .map(|o| o.flowers.len())
            .unwrap_or_else(|e| panic!("area: {e}"));
        assert!(gathered > 0, "the area gathered nothing");

        // Save and restore a world with every flower picked.
        let mut full = Game::new(data.clone());
        for i in 0..n {
            let (c, _) = shared.pick_sphere(i);
            full.tick(1.0);
            full.harvest(0, i, c + Vec3::Y).unwrap();
        }
        let t = Instant::now();
        let text = encode(full.state());
        let encode_ms = ms(t);
        let t = Instant::now();
        let state = decode(&text).unwrap();
        let decode_ms = ms(t);
        let t = Instant::now();
        Game::from_state(data.clone(), state).unwrap();
        let restore_ms = ms(t);

        println!(
            "{n:>8} | {layout_ms:>9.1} {place_ms:>9.1} | {radius_us:>9.2} {rect_us:>9.2} {pick_us:>9.2} | {harvest_ms:>9.1} {area_ms:>9.2} | {encode_ms:>9.2} {decode_ms:>9.2} {restore_ms:>9.1}"
        );
    }
}
