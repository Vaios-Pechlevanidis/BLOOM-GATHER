//! Synthetic content for benchmarks: the shipped game data with its five
//! regions replaced by one large field of `n` flowers (every species, in
//! the moonlit garden's proportions), sized like the real regions are
//! (about 2.6 × the minimum area per flower). Used by the core benchmark
//! (`cargo run --release -p bloom_core --example bench`) and the in-game
//! benchmark (`scripts/bench.sh`). Never used by the game itself.

use crate::data::{DataError, GameData, RegionDef, SpeciesCount};

/// Flower counts the benchmarks measure.
pub const SIZES: [u32; 4] = [1_000, 10_000, 100_000, 500_000];

/// The shipped content with a single region of `n` flowers.
pub fn benchmark_data(n: u32) -> Result<GameData, DataError> {
    let mut data = GameData::builtin()?;
    let template: RegionDef = data.regions[0].clone();
    let species: Vec<String> = data.species.iter().map(|s| s.id.clone()).collect();
    // Spread `n` over all species (the remainder goes to the first ones).
    let per = n / species.len() as u32;
    let extra = n % species.len() as u32;
    let flowers: Vec<SpeciesCount> = species
        .iter()
        .enumerate()
        .map(|(i, id)| SpeciesCount {
            species: id.clone(),
            count: per + (i < extra as usize) as u32,
        })
        .filter(|f| f.count > 0)
        .collect();
    let spacing = data.world.min_flower_spacing;
    let area = n as f32 * spacing * spacing * 2.6;
    let depth = (area / 1.4).sqrt().max(80.0);
    let width = (depth * 1.4).max(110.0);
    // Coarser terrain for very large fields keeps the heightfield to a few
    // million samples.
    data.world.terrain_cell = (width / 1_500.0).max(data.world.terrain_cell);
    let region = RegionDef {
        id: "benchmark_field".into(),
        name: "Benchmark Field".into(),
        description: format!("{n} flowers for measuring performance."),
        flower_total: n,
        width,
        depth,
        seed: 9_001,
        flowers,
        unlock: template.unlock,
        // Small fields can't pay for every tool by themselves; the
        // benchmark only measures speed, so completing it pays enough.
        completion_reward: crate::data::RegionReward {
            coins: 20_000,
            ..template.completion_reward
        },
        palette: template.palette,
        terrain: template.terrain,
        props: crate::data::RegionProps {
            trees: (width * depth / 1_000.0) as u32,
            rocks: (width * depth / 600.0) as u32,
        },
        workshop: true,
        // Grass as in the largest real region, spread thinner over a
        // bigger field (the benchmark measures flowers).
        scenery: crate::data::Scenery {
            features: crate::data::RegionFeatures {
                grass_density: Some((230.0 * 160.0 / (width * depth)).min(1.0)),
                ..Default::default()
            },
            ..Default::default()
        },
    };
    data.regions = vec![region];
    // Achievements and the ending speak of the real world; the benchmark
    // world only needs to load.
    data.achievements.clear();
    data.reindexed_and_validated()
}
