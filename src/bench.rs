//! In-game benchmark (`scripts/bench.sh`): with `BLOOM_BENCH=<flowers>` the
//! game loads a synthetic field of that many flowers (see
//! `bloom_core::bench`), enters it, and walks a loop through it for a fixed
//! time while timing every frame. It then prints one result line and quits:
//!
//! `BENCH flowers=… spawn_ms=… avg_fps=… p99_ms=… max_ms=… entities=…`
//!
//! The save folder must be a throwaway one (`BLOOM_SAVE_DIR`, the script
//! sets it). Never active otherwise.

use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin};
use bevy::prelude::*;
use bloom_core::player::{CharacterBody, Environment};

use crate::flowers::SpawnedFlowers;
use crate::game::{AppState, RegionScene, Settings};
use crate::player::Player;

/// Flowers for the benchmark field, if benchmarking.
pub fn requested() -> Option<u32> {
    std::env::var("BLOOM_BENCH").ok()?.parse().ok()
}

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        let Some(flowers) = requested() else { return };
        let mut settings = Settings {
            pause_on_focus_loss: false,
            master_volume: 0.0,
            vsync: false,
            ..default()
        };
        if let Ok(q) = std::env::var("BLOOM_BENCH_QUALITY") {
            settings.quality = match q.as_str() {
                "low" => crate::game::Quality::Low,
                "medium" => crate::game::Quality::Medium,
                _ => crate::game::Quality::High,
            };
        }
        app.insert_resource(settings)
            .insert_resource(Bench {
                flowers,
                phase: Phase::Boot,
                t: 0.0,
                entered: None,
                spawn_ms: 0.0,
                frames: Vec::new(),
                spawned: 0,
            })
            .add_systems(Update, run);
    }
}

#[derive(PartialEq)]
enum Phase {
    Boot,
    Spawning,
    Warmup,
    Measure,
    Done,
}

#[derive(Resource)]
struct Bench {
    flowers: u32,
    phase: Phase,
    t: f32,
    entered: Option<std::time::Instant>,
    spawn_ms: f64,
    frames: Vec<f32>,
    /// Flower entities at the end (the ones around the camera).
    spawned: usize,
}

const WARMUP_SECS: f32 = 3.0;
const MEASURE_SECS: f32 = 12.0;
/// Walking pace along the loop (metres per second).
const PACE: f32 = 5.0;

#[allow(clippy::too_many_arguments)]
fn run(
    time: Res<Time<Real>>,
    mut bench: ResMut<Bench>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    spawned: Res<SpawnedFlowers>,
    scene: Option<Res<RegionScene>>,
    player: Option<ResMut<Player>>,
    diagnostics: Res<DiagnosticsStore>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    bench.t += dt;
    match bench.phase {
        Phase::Boot => {
            if *state.get() == AppState::MainMenu && bench.t > 1.0 {
                next.set(AppState::InGame);
                bench.entered = Some(std::time::Instant::now());
                bench.phase = Phase::Spawning;
            }
        }
        Phase::Spawning => {
            // Flowers stream in around the camera; the first fill is the
            // spawn time.
            if !spawned.map.is_empty() {
                bench.spawn_ms = bench
                    .entered
                    .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
                bench.phase = Phase::Warmup;
                bench.t = 0.0;
            } else if bench.t > 120.0 {
                error!("BENCH flowers never finished spawning");
                exit.write(AppExit::error());
                bench.phase = Phase::Done;
            }
        }
        Phase::Warmup | Phase::Measure => {
            // Walk a loop through the middle of the field, looking ahead.
            if let (Some(scene), Some(mut player)) = (scene, player) {
                let layout = &scene.layout;
                let radius = layout.half_size.min_element() * 0.45;
                let a = bench.t * PACE / radius;
                let at = Vec2::new(a.cos(), a.sin()) * radius;
                let env = Environment {
                    terrain: &layout.terrain,
                    colliders: &layout.colliders,
                };
                let body = CharacterBody::standing_at(at, &env);
                let ahead = Vec2::new(-a.sin(), a.cos());
                player.teleport(body, (-ahead.x).atan2(-ahead.y));
                player.pitch = -0.25;
            }
            if bench.phase == Phase::Warmup {
                if bench.t > WARMUP_SECS {
                    bench.phase = Phase::Measure;
                }
            } else {
                bench.frames.push(dt);
                if bench.t > WARMUP_SECS + MEASURE_SECS {
                    bench.spawned = spawned.map.len();
                    report(&bench, &diagnostics);
                    exit.write(AppExit::Success);
                    bench.phase = Phase::Done;
                }
            }
        }
        Phase::Done => {}
    }
}

fn report(bench: &Bench, diagnostics: &DiagnosticsStore) {
    let mut frames: Vec<f32> = bench.frames.iter().map(|f| f * 1000.0).collect();
    frames.sort_by(f32::total_cmp);
    let total: f32 = frames.iter().sum();
    let avg_fps = frames.len() as f32 / (total / 1000.0);
    let p99 = frames[(frames.len() as f32 * 0.99) as usize..]
        .first()
        .copied()
        .unwrap_or(0.0);
    let max = frames.last().copied().unwrap_or(0.0);
    let entities = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    println!(
        "BENCH flowers={} spawn_ms={:.0} avg_fps={:.1} p99_ms={:.1} max_ms={:.1} entities={:.0} spawned={}",
        bench.flowers, bench.spawn_ms, avg_fps, p99, max, entities, bench.spawned
    );
}
