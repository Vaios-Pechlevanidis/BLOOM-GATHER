# Bloom & Gather — Performance

The real game holds at most 750 flowers per region (1,900 in all). The
benchmarks push far beyond that, to 500,000 flowers in one field, to find
what grows with the number of flowers and fix it before it matters.

Run them with:

```bash
scripts/bench.sh                     # core + game, 1k / 10k / 100k / 500k
scripts/bench.sh 1000 100000         # chosen sizes
BLOOM_BENCH_QUALITY=low scripts/bench.sh
```

* **Core** (`cargo run --release -p bloom_core --example bench`): no
  window. Layout and flower placement, radius / rectangle / look-at queries,
  harvesting, the cart's area harvest, and saving / loading a world with
  every flower picked.
* **Game** (`BLOOM_BENCH=<flowers>`, release build): the real renderer and
  systems on a synthetic field (`bloom_core::bench::benchmark_data`: every
  species, grass thinned to the largest real region's amount). The camera
  walks a loop through the field for 12 s after 3 s of warm-up; the result
  is average FPS, the slowest 1 % of frames (p99) and entity counts. It
  uses a throwaway save folder.

Numbers below: Apple M2, macOS, 2560×1600, release build, best of three
runs (a background sync service was loading the machine, so single runs
varied by up to 30 %). 60 FPS is the display's limit.

## Core (release)

| Flowers | Layout | Placement | Radius query | Rect query | Look-at pick | Area harvest | Save + load | Restore (validate) |
|---|---|---|---|---|---|---|---|---|
| 1,000 | 7 ms | 1.4 ms | 0.2 µs | 0.5 µs | 0.1 µs | 0.01 ms | 0.07 ms | 4 ms |
| 10,000 | 10 ms | 10 ms | 0.15 µs | 0.5 µs | 0.1 µs | 0.01 ms | 0.08 ms | 19 ms |
| 100,000 | 50 ms | 75 ms | 0.16 µs | 0.5 µs | 0.1 µs | 0.01 ms | 0.4 ms | 129 ms |
| 500,000 | 50 ms | 536 ms | 0.28 µs | 0.7 µs | 0.24 µs | 0.02 ms | 1.9 ms | 518 ms |

Before the optimisation, placement took 751 ms at 100,000 flowers (linear
in trees, rocks and path points per attempt, all of which grow with the
field), and restoring a save took 799 ms.

## Game (release, High quality)

| Flowers | Before (FPS / p99) | After (FPS / p99) | Flower entities after |
|---|---|---|---|
| 1,000 | 63 / 19 ms | 60 / 18 ms (display limit) | 1,000 |
| 10,000 | 24 / 66 ms | 49 / 34 ms | 9,870 |
| 100,000 | 9.5 / 120 ms | 29 / 51 ms | 17,900 |
| 500,000 | 4.7 / 234 ms | 26 / 52 ms | 17,300 |

Medium quality: 60 FPS at 1,000, 50 FPS at 100,000 and 500,000. Low: 60 FPS
at every size.

## What was found, and fixed

1. **Flower placement and the layout scaled with the field** (every
   placement attempt checked every tree, rock and path point). A spatial
   grid now indexes trees, rocks and footpath segments
   (`world::grid::SpatialGrid`); `distance_to_path` is exact within 8 m
   and a lower bound beyond, which is all any caller asks. Answers are
   unchanged (a test pins the Sunny Meadow's flowers to the centimetre).
   Placement at 100k: 751 → 75 ms.
2. **Triangles, not entities, were the main cost.** A flower is about 2,900
   triangles, drawn again in each of three shadow cascades. With shadows
   off the 10k field ran at 123 FPS instead of 24. Flowers beyond a
   distance (12 / 22 / 26 m for Low / Medium / High) now use a ~50-triangle
   stand-in built from the full model (stem, bloom ball in the petal
   colour, centre dot) and cast no shadow. The near set is refreshed a few
   times a second from the flower grid, so it costs nothing per flower.
   10k: 24 → 54 FPS (49 with the near distance later raised to 32 m).
3. **Wind sway moved to the GPU.** Flowers used to be rotated on the CPU
   every frame (a transform change per flower per frame). They now use the
   same wind shader as grass and trees, with sway weights in vertex-colour
   alpha. Each flower is one entity (night flowers keep one child for their
   glow), down from two or three.
4. **Flowers stream in around the camera.** Only flowers within 90 / 120 /
   160 m (Low / Medium / High) are entities; they are spawned and despawned
   (with a 15 m margin) as you move, a few times a second, from the grid.
   Every real region fits in that radius, so nothing changes there; a
   500,000-flower field keeps about 17,000 flower entities.

## After the Bevy 0.20 upgrade (Phase G4)

Same machine, release build, High quality, flower pictures for the daisy
(Phase "flower pictures"). The machine was warm from many runs before, so
single runs varied by up to 40 %:

| Flowers | FPS (best of three) |
|---|---|
| 1,000 | 52 |
| 10,000 | 33 |
| 100,000 | 33 |
| 500,000 | 34 |

Large fields got faster (streaming and culling in newer Bevy), small ones
sit just over the 60 Hz frame budget at 2560×1600: a frame that misses
16.7 ms waits for the next refresh, so a little extra work halves the rate
for those frames (the slowest 1 % are all near 33–40 ms). Bringing the
High near distance for full flower models from 32 m back to 26 m gave
about 5 FPS back in A/B runs. Medium and Low reach the 60 FPS limit at
1,000 flowers.

## What is left

* At 100,000+ flowers the field itself is huge (860 × 610 m and more): the
  terrain chunks, trees and their shadows now cost more than the flowers.
  Not a concern for the game's regions (at most 230 × 160 m).
* Loading a save regenerates the region to check it: about half a second
  at 500,000 flowers, 4 ms for a real region.
* Flower Recognition's glints are rebuilt when the set of spawned flowers
  changes; with the skill learned, walking a huge field rebuilds them a few
  times a second (fine at real sizes).
