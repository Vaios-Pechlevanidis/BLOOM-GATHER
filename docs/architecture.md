# Bloom & Gather — Architecture

Status: Phase A (first-person 3D foundation) complete. This document describes
the target architecture and marks what exists today.

## 1. Goals that shape the architecture

| Goal | Consequence |
|------|-------------|
| Native, offline desktop game (Windows/macOS/Linux) | Rust + Bevy, no browser/web stack, all data and assets ship with the binary. |
| First-person 3D exploration | Bevy 3D renderer, a kinematic first-person controller, terrain + props built from deterministic layouts. |
| Gameplay logic is authoritative and testable | All rules — including movement and collision — live in an engine-independent crate (`bloom_core`) with no Bevy dependency. Tests run in seconds without a GPU. |
| Finite world, never-respawning flowers | One authoritative flower store with a harvested **bitset**; the save file persists that bitset. Nothing in the code path can clear a bit except loading a save. |
| Data-driven content | Flowers, tools, recipes, regions (size, terrain, props), economy, progression and player movement live in `assets/data/game_data.ron`, validated at startup and in tests. |
| Scales to large fields | Struct-of-arrays flower storage, a uniform spatial grid, chunked terrain and flower rendering with frustum culling and instancing. |

## 2. Workspace layout

```
bloom-and-gather/
├── Cargo.toml                 workspace + the game binary package
├── assets/
│   └── data/game_data.ron     all content & balancing numbers (metres, seconds, coins)
├── crates/
│   └── bloom_core/            engine-independent simulation (lib)
│       ├── src/
│       │   ├── data/          content definitions, loading, validation
│       │   ├── world/         terrain, region layout, static collision
│       │   ├── player.rs      first-person character controller
│       │   ├── economy.rs     economy configuration and money math
│       │   ├── progression.rs XP and level curve
│       │   └── rng.rs         deterministic SplitMix64
│       └── tests/             content integrity, movement & layout tests
├── src/                       Bevy application (presentation layer)
│   ├── app.rs                 plugin wiring, window
│   ├── game/                  app states, settings, content & region-scene resources
│   ├── input/                 action bindings
│   ├── player/                input → controller (fixed step), mouse look, camera
│   ├── rendering/             3D camera, region scene, procedural meshes
│   ├── ui/                    theme tokens, widgets, menus, HUD, debug overlay
│   └── dev.rs                 debug-only automated smoke test (BLOOM_SMOKE_TEST)
└── docs/
```

The spec suggested a single crate with `flowers/`, `harvesting/`, `economy/`…
modules under `src/`. Those modules still exist, but the *simulation* modules
live in `bloom_core` and the *presentation* modules in the binary. Reasons:

1. Hard separation: the compiler stops gameplay rules from touching rendering.
2. `cargo test -p bloom_core` doesn't compile Bevy, so logic tests are fast.
3. Benchmarks of harvesting/crafting/movement measure the logic alone.
4. Save files serialize `bloom_core` types only.

`bloom_core` uses `glam` 0.29, the same version Bevy 0.16 uses, so `Vec2` /
`Vec3` cross the boundary with no conversion.

Modules are only created when the phase that needs them begins. The spec's
`core/` module is named `game/` in the binary: a module called `core` shadows
Rust's built-in `core` crate and breaks macro expansions.

## 3. Layers and data flow

```
             ┌──────────────────────── Bevy app (src/) ─────────────────────────┐
 keyboard ──►│ input/ Bindings ──► player/ gather_input ──► PendingInput         │
 mouse    ──►│                     player/ mouse_look ──► yaw/pitch              │
             │                                                                  │
             │ FixedUpdate 64 Hz: player/step_player                             │
             │     bloom_core::player::CharacterBody::step(input, terrain,       │
             │                                              colliders)          │
             │                                                                  │
             │ Update: camera = lerp(prev, current, overstep) + eye + bob        │
             │                                                                  │
             │ RegionScene { layout: Arc<RegionLayout> } ◄── bloom_core::world   │
             │     ├─► rendering/scene (terrain chunks, trees, rocks, fence)     │
             │     └─► player controller (same terrain + colliders)              │
             │                                                                  │
             │ ui/ reads state; later phases: gameplay commands → bloom_core     │
             └──────────────────────────────────────────────────────────────────┘
```

* **bloom_core** is plain Rust. Later phases add a `Game` struct with command
  methods (`harvest`, `craft`, `buy`) returning typed results plus a queue of
  `GameEvent`s. It never panics on player input.
* **One layout, two consumers.** `RegionLayout::generate(region, world)` is
  deterministic from the region seed. Rendering builds meshes from it and the
  controller collides with it, so visuals and collision always agree.
* **UI reads, never writes, game state** — every gameplay mutation will go
  through a `bloom_core` command so validation exists in exactly one place.

## 4. Game states

```
Boot ──(data validated)──► MainMenu ──Begin/Enter──► InGame{Playing ⇄ Paused}
  │                           ▲                           │
  └──(invalid data)──► log + exit      └──Main Menu (pause)──┘
```

* State-scoped entities despawn on exit. The 3D region scene is *not*
  state-scoped: it is built once per region and shown behind the title screen
  (slow orbit camera) and in game.
* Entering `Playing` locks and hides the cursor; leaving it releases it.
  Losing window focus pauses (configurable).
* Entering the game blends the camera from the menu orbit into the player's
  eyes over one second.

## 5. First-person controller (`bloom_core::player`)

**Decision: a custom kinematic controller, not a physics engine.** The solid
world is a heightfield plus static trees, rocks, fences and (later) building
walls — all well represented by vertical cylinders and yaw-rotated boxes with
a vertical extent. Against those shapes a collide-and-slide controller is
exact, deterministic, cheap and testable without Bevy. A rigid-body engine
(Avian, Rapier) would add a large dependency and a second source of truth for
the world while solving problems the game doesn't have (dynamic bodies,
stacking). If dynamic objects are ever needed, revisit.

Per fixed step (64 Hz):
1. jump-buffer (0.12 s) and coyote-time (0.10 s) timers,
2. horizontal velocity eases towards the input direction (separate ground
   accel/decel and weaker air control),
3. jump (v = √(2gh)) or gravity, integrated with the average velocity over
   the step (exact parabola regardless of step size),
4. horizontal move: steep terrain (> 42°) is refused uphill but slid along;
   colliders push the body out and remove the into-wall velocity (sliding),
5. vertical: land on terrain or on collider tops ≤ step height (0.35 m)
   above the feet; stay glued to the ground going downhill (ground snap).

Colliders: `CircleCollider` (trees, rocks) and `BoxCollider` (invisible region
walls; later fences and buildings). Anything whose top is within step height
is a step, not a wall, so small stones can be walked over and boulders jumped
onto. Queries are linear (≤ ~200 colliders per region); add a grid if
profiling ever shows a need.

## 6. World & rendering

* **Units:** metres. Regions are 110×80 m (Sunny Meadow) up to 230×160 m
  (Moonlight Garden).
* **Terrain:** seeded fractal value noise + a berm along the region edge +
  flattened spawn area, sampled every 0.5 m. It extends 30 m beyond the fence
  ("outskirts") where scenery trees stand, so the world never ends at a
  visible edge. Rendered as 32 m chunks (frustum-culled) with vertex colours
  (hill/hollow tones and the worn footpath).
* **Props:** trees (3 silhouettes), rocks (3 shapes), fence. Each variant is
  one merged mesh with vertex colours, all sharing one matte material, so
  hundreds of props batch into a handful of draw calls.
* **Lighting:** warm directional sun with 3 shadow cascades (110 m), soft
  ambient, linear distance fog in the sky colour.
* **Art status:** procedural low-poly meshes are *temporary art* (documented
  in TODO.md); painted glTF models replace them later without code changes to
  gameplay, because placement and collision come from `bloom_core`.

## 7. Flower world (Phase B design)

* **Storage (SoA):** `FlowerField { pos: Vec<Vec2>, species: Vec<u16>,
  variant: Vec<u8>, harvested: BitSet }`. A flower's id is its index, stable
  forever because generation is deterministic from `(region seed, layout)`.
* **Placement:** exact per-species counts from data, natural clusters, off the
  path and props, minimum spacing; decorative grass/vegetation is separate and
  never counts as a flower.
* **Interaction:** ray from the camera, tested against flower bounds via the
  spatial grid within `player.interact_range`; E harvests.
* **Rendering:** a real 3D model per species; one entity per flower in
  visible chunks, sharing meshes/materials (automatic instancing); wind sway
  in a vertex shader or per-entity transform.
* **Double-harvest prevention:** `harvest(id)` is `if !bitset.test_and_set(id)`.

## 8. Persistence (design)

* One save slot file per profile in the OS data dir.
* Format: RON in a versioned envelope `{ version, checksum, payload }`;
  harvested flowers as a compact bitset per region.
* Safe writes: `save.tmp` → fsync → rename; previous file rotated to
  `save.bak`; load falls back to the backup.
* Dirty tracking with debounced autosave plus immediate saves after
  purchases, crafting, unlocks, level-ups and on quit.
* Settings (sensitivity, FOV, …) persist with the save system.

## 9. Testing strategy

* `cargo test` — unit tests in `bloom_core` modules plus integration tests:
  content integrity, movement (gravity, exact jump height, buffered jumps,
  speeds, stopping, trees, wall sliding, steps, boulders, slopes, bounds) and
  layouts (determinism, clear spawn, props off the path).
* **Smoke test of the real game** (debug builds):
  `BLOOM_SMOKE_TEST=<dir> cargo run` boots the real window and renderer, plays
  with injected keyboard and mouse input, asserts movement, look, jump, pause
  and cursor locking, saves screenshots and exits non-zero on failure.

## 10. Performance monitoring

* F3 overlay: FPS, frame time, entity count, player position/speed/state.
  Later: visible/total flowers, harvest/craft/save timings, draw calls.
* Region build time is logged (Sunny Meadow: ~19 ms).
* Benchmarks (polish phase): flower generation, ray/area queries, batch
  harvest, batch craft, save/load at 1k–500k flowers.

## 11. Engine version

Bevy **0.16** is pinned (latest is 0.20): stable, well-known APIs while the
foundation is built. All engine-touching code is in `src/`, so an upgrade is
contained there (tracked in TODO.md).

## 12. Dependencies

| Crate | Why |
|-------|-----|
| `bevy` 0.16 | engine: windowing, ECS, 3D rendering, UI, audio, input |
| `glam` 0.29 | maths in `bloom_core` (Bevy's own maths types) |
| `serde`, `ron` | content files and save files |

Nothing else until a phase needs it.
