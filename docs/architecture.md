# Bloom & Gather — Architecture

Status: Phase G3 (packaging) done. This document describes the
target architecture and marks what exists today.

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
│       │   ├── world/         terrain, region layout, gates, water & bridges, collision, flower field
│       │   ├── game.rs        authoritative game state + commands (harvest, sell…)
│       │   ├── persistence.rs versioned, checksummed, atomic save files
│       │   ├── bitset.rs      harvested-flower bitsets (set-only)
│       │   ├── player.rs      first-person character controller
│       │   ├── economy.rs     economy configuration and money math
│       │   ├── progression.rs XP and level curve
│       │   └── rng.rs         deterministic SplitMix64
│       └── tests/             content, movement, flowers, game rules, saves
├── src/                       Bevy application (presentation layer)
│   ├── app.rs                 plugin wiring, window
│   ├── game/                  states, settings, session (load/autosave/new game), content
│   ├── input/                 action bindings
│   ├── player/                input → controller (fixed step), mouse look, camera
│   ├── interaction.rs         look-at raycast, E to pick, event feedback
│   ├── travel.rs              gates, journeys between regions (fade, rebuild, arrival)
│   ├── tools.rs               tool selection, ground aiming, basket/rake/cart actions
│   ├── viewmodel.rs           first-person hands & tools (second camera, layer 1)
│   ├── flowers/               3D flower models, spawning, wind, highlight, pick FX
│   ├── audio.rs               synthesised sound effects, per-region ambience
│   ├── rendering/             camera, region scene, wind shader, grass, sky,
│   │                          landmarks, butterflies & bees, region scenery
│   │                          (gates, water, bridges, mushrooms, glow plants,
│   │                          light shafts, petals, fireflies), procedural meshes
│   ├── ui/                    theme tokens, widgets, menus, HUD, debug overlay
│   └── dev.rs                 debug-only automated smoke test (scripts/smoke.sh)
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

* **bloom_core** is plain Rust. `Game` owns the saved `GameState` and exposes
  commands (`harvest`, `sell`, later `craft`, `buy`) that validate first and
  either apply everything or return a `GameError` without changing anything.
  Commands queue `GameEvent`s; the app re-sends them as Bevy events
  (`GameEventMsg`) for sounds, toasts and animations.
* **One layout, two consumers.** `RegionLayout::generate(region, world)` is
  deterministic from the region seed. Rendering builds meshes from it and the
  controller collides with it, so visuals and collision always agree.
* **UI reads, never writes, game state** — every gameplay mutation will go
  through a `bloom_core` command so validation exists in exactly one place.

## 4. Game states

```
Boot ──(data validated)──► MainMenu ──Begin/Enter──► InGame{Playing ⇄ Paused | Inventory | Workshop | Skills | Journal | Map | Collection | Finale}
  │                           ▲                           │
  └──(invalid data)──► log + exit      └──Main Menu (pause)──┘
```

* `Inventory` (basket / Tool Shop / Upgrades), `Skills`, `Journal`, `Map`, `Collection` and `Paused` stop game time;
  `Finale` stops the game clock but keeps the world animating; `Workshop`
  (the workbench screen) keeps it running so a batch binds while you watch.
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
* **Wind:** `WindMaterial = ExtendedMaterial<StandardMaterial, Wind>` with a
  custom vertex shader (`rendering/wind.wgsl`, embedded in the binary). The
  sway weight of each vertex is stored in its vertex-colour alpha (unused by
  opaque materials): 0 at roots/trunks, 1 at blade tips and canopy tops. The
  phase comes from world position, so gusts travel across the field. Used by
  grass and tree canopies; flowers sway per entity on the CPU (≤ 750).
* **Grass:** decorative only. Tapered blades merged into 16 m chunk meshes
  (~135k blades in the Sunny Meadow, built in ~19 ms), no shadows, faded out
  beyond ~45 m with visibility ranges, leaving room around every flower.
* **Sky:** an unlit gradient dome following the camera, horizon colour equal
  to the fog colour, plus drifting unlit cloud clusters.
* **Landmarks:** bench, well and signpost placed by the core layout (with
  colliders and flattened ground), drawn by `rendering/landmarks.rs`.
* **Ambience:** butterflies and bees steer between still-blooming flowers;
  a synthesised looping breeze + birdsong bed plays under the effects.
* **Art status:** procedural low-poly meshes are *temporary art* (documented
  in TODO.md); painted glTF models replace them later without code changes to
  gameplay, because placement and collision come from `bloom_core`. Flowers
  can already be painted pictures: `assets/flowers/<species id>.png`, embedded
  by `build.rs` and drawn as a card turned to the player (`src/flowers/sprites.rs`).

## 7. Flower world (implemented in Phase B)

* **Storage (SoA):** `FlowerField { positions, bases, species, yaws, scales,
  bloom_heights, grid }`. A flower's id is its index, stable forever because
  generation is deterministic from `(region seed, layout)`. The harvested
  state is *not* in the field: it is a set-only `BitSet` per region inside the
  saved `GameState`.
* **Placement:** exact per-species counts from data, natural clusters, off the
  path and props, minimum spacing; decorative grass/vegetation is separate and
  never counts as a flower.
* **Interaction:** each frame a ray from the camera is tested against flower
  pick spheres via the spatial grid within `player.interact_range`; E calls
  `Game::harvest`, which re-validates reach, cooldown and harvested state.
* **Rendering:** a procedural 3D model per species (17 silhouette families,
  stems, leaves, petals, glowing blooms on a separate emissive mesh). One
  entity per unharvested flower sharing meshes/materials (automatic
  instancing); wind sway per entity. Flowers are drawn and picked at
  `world.flower_scale` (2.2×). At ≤ 750 flowers per region all are spawned;
  chunk streaming is reserved for when benchmarks ask for it.
* **Double-harvest prevention:** the only write path is
  `BitSet::test_and_set`; there is no API to clear a bit. On load, harvested
  indices are skipped when spawning, so picked flowers never reappear.

## 7b. Tools and first-person view (Phase D)

* **Core:** `Game::buy_tool` validates level, prerequisite tools and coins and
  applies atomically. Batch modes (`harvest_radius`, `harvest_sweep`,
  `harvest_area`) validate the tool, cooldown and ground reach, gather the
  flowers `preview_*` would show (nearest first, capped), and run them
  through the same `apply_one`/`finish_batch` path as single picks, so
  events, XP, discovery and region completion behave identically.
* **Aiming:** batch tools raycast the terrain (`Terrain::raycast`) within
  ground reach. Aim and pick systems read the camera's `Transform` (already
  updated this frame) rather than `GlobalTransform`, which would lag a frame.
* **Viewmodel:** a second `Camera3d` (order 1, no clear, own depth) parented
  to the main camera renders render layer 1 only, so hands and tools never
  clip into the world. The sun is on both layers. Models and poses are
  procedural; animations are driven by real game events (a pick happened),
  not by key presses.

## 7c. Workshop and crafting (Phase E)

* **Placement:** regions with `workshop: true` get a cottage landmark from
  `RegionLayout::place_landmarks` (next to the spawn, away from the path,
  door facing the spawn). `layout::workshop` holds its dimensions; walls (with
  a door gap) and furniture are box colliders, and the four stations are
  `Interactable` spheres picked by `pick_interactable` (eye ray). Rendering
  places the mesh, the lamp and the vases from the same numbers
  (`layout::to_world` converts workshop-local positions).
* **Interaction:** `LookTarget.station` names the station in view; E sets
  `OpenScreen` and enters `PlayState::Workshop` (workbench, display) or
  `Inventory` (tool shelf → Tool Shop tab, storage → basket).
* **Core (`game/crafting.rs`):** `refresh_recipes` records discoveries
  whenever tools, species or levels change. `plan_craft(recipe, count,
  keep)` is a pure preview that picks named species first, then distinct
  species, then the most plentiful flowers, never touching flowers reserved
  for *other* required bouquets, and checks the reserve for the batch as a
  whole. `craft` validates (discovered, workbench free, batch ≤ capacity,
  plan) and only then consumes flowers and starts a saved `CraftJob`;
  `tick` advances it and delivers bouquets and XP. Selling bouquets earns
  coins like flowers.
* **Presentation:** `ui::workshop` renders the plan the core computed, so
  the screen can't disagree with what binding will do; `rendering::bouquets`
  rebuilds the vase bouquets when the owned bouquets change.

## 7d. Upgrades, skills and modifiers (Phase U)

* **Content:** `GameData::upgrades` and `GameData::skills`
  (`data/growth.rs`), each a list of per-level `Effect`s. Validation checks
  references, value ranges, that a skill's prerequisite is listed earlier in
  the same branch (no cycles), and that stacked reductions can't pass their
  floors.
* **Modifier pipeline:** `Game::modifiers()` folds every owned upgrade and
  skill level into one `Modifiers` value. The rest of the core asks it at
  the point of use: pick cooldown, `mode_params` (radius, capacity, area),
  `reach`, `batch_capacity`, `plan_craft` time, crafting and discovery XP,
  `tool_price`, `upgrade_price`, `bouquet_price`, region thresholds. There
  is no cached copy to go stale.
* **Skill points** are derived, never stored: earned (levels, region
  rewards, collection milestones, Collection Bonuses) minus the cost of
  learned skills.
* **Guard:** `optional_spending_limit()` implements economy doc §5.2;
  `upgrade_status` reports `SavingForTools` when it bites.
* **Chance:** Double Harvest and Efficient Production roll a SplitMix64
  whose state is saved in `GameState::rng`.
* **Region unlocks** are recorded in `GameState::unlocked_regions` and
  announced once (`RegionUnlocked`), paying the unlock bonus skill if
  learned. New and loaded games record already-met unlocks silently.
* **Presentation:** `ui::effects` renders "now -> next" lines by asking a
  `preview_upgrade`/`preview_skill` copy of the game, so the shop can't
  promise something the rules don't do. `ui::skills` is the K screen; the
  Upgrades tab lives in `ui::inventory`; `flowers::sparkles` draws Flower
  Recognition's glints; `player` scales walk/sprint speed.

## 7e. Achievements, completion and statistics (Phase P)

* **Content:** `GameData::achievements` (`data/achievements.rs`): a
  `Condition` plus a reward. Validation rejects targets the finite world
  can't reach.
* **Core (`game/progress.rs`):** `refresh_achievements` runs after XP
  awards (which cover harvests, finished batches and skills), tool
  purchases and craft starts. Each achievement unlocks once, pays once and
  raises `AchievementUnlocked`. `from_state` catches up older saves without
  announcing anything. `Game::completion()` reports the six ending
  requirements; the `GameCompleted` condition and the Phase Z ending both
  use it.
* **Statistics:** `Stats` gained `coins_spent` and `distance_walked_m`. The
  player system feeds distance through `Game::add_distance` each fixed step.
* **Presentation:** `ui::journal` (J). Toasts move to the bottom edge and show
  titles only while any screen is open.

## 7f. Regions and travel (Phase F)

* **Content:** each `RegionDef` has a `scenery` block (`data/scenery.rs`):
  the entrance gate style, ambience, light (`Atmosphere`: sun colour,
  strength and height, ambient light, sky zenith, fog, night) and features
  (flower rows, stream, pond, lanterns, mushrooms, glow plants, light
  shafts, petals, fireflies, blossom share, tree heights, grass density).
  Everything defaults to "nothing special"; validation keeps water shallow
  enough to wade (never a trap) and values in range.
* **Layout (`world/features.rs`):** gates where the footpath meets the north
  and south fence (trigger zone, arrival spot), a stream laid out before
  landmarks (so they keep dry) and carved into a level valley floor, a pond
  chosen clear of the path and landmarks, wooden bridges wherever the path
  crosses water (deck slabs as box colliders in steps far below the step
  height, railings), lantern posts along the path. Every new feature uses
  its own random stream and only applies where content asks for it, so the
  Sunny Meadow's flowers keep their exact positions (fingerprint test) and
  older saves stay valid. Flowers keep clear of water; lavender snaps to
  rows.
* **Core (`game/travel.rs`):** the regions form a chain; `gate_destination`,
  `arrival_side`, `region_requirements`, `enter_region` (refused while
  locked; first visit announced through `RegionEntered`) and the saved
  `visited_regions` (checked against unlocked regions on load).
* **Presentation:** `travel.rs` runs journeys: fade out → `enter_region` →
  switch `ActiveRegion` (the scene, flowers, grass, sky, critters, scenery and
  ambience rebuild on the change) → place the player at the gate they came
  through → fade in; movement is frozen meanwhile. A banner near a gate says
  where it leads or what it still needs. `ui::map` draws each region into a
  small image (ground, path, water, trees, gates, workshop, unpicked flowers),
  shown as the HUD minimap and in the world map (M), which travels to any
  open region. The save stores the region you stand in.

## 7g. Completion and the ending (Phase Z)

* **Core (`game/progress.rs`):** after every achievement check (which runs
  after harvests, finished batches, tool purchases and craft starts),
  `check_completion` looks at `Game::completion()`. The first time all six
  requirements hold it stores a `JourneySummary` (playtime, distance,
  flowers, rare flowers, bouquets, coins, level, tools, regions, species,
  recipes, achievements, upgrade and skill levels) in
  `GameState::completed` and raises `GameCompleted` once. A started batch
  counts as bound (its timer always finishes). `finale_seen` records that
  the ending was watched; `finale_pending()` is "complete but not seen", so
  a game closed mid-finale plays it again. `from_state` records older saves'
  completion quietly and refuses a claimed completion that isn't real, or
  an ending seen without one.
* **Finale (`ui/finale.rs`, `PlayState::Finale`):** starts 1.5 s after
  completion (from play, the workbench or the basket). A director drives the
  camera (the player camera is paused in this state): celebration with
  golden petals and the title, then for each region a fade, a region switch
  (`ActiveRegion`; the scene rebuilds as when travelling) and a glide along
  the footpath while `flowers::MemoryBloom` shows every picked flower as a
  glowing, see-through bloom with a small light in its colour, then back
  home and the journey summary. Esc skips the tour. While touring, the
  `Touring` resource makes saves record the player's home region. HUD roots
  (`ui::HudRoot`) and toasts are hidden; the hands are hidden.
* **Free roam:** leaving the summary marks the finale seen. The HUD says
  "Every flower gathered. Free roam"; the journal's Progress page links to
  the summary (the finale state opened at its last step).
* **Herbarium (`ui/collection.rs`, C):** every species card (colours,
  rarity, picked of total, description; undiscovered ones only say where
  they grow) and the bouquets bound.

## 7h. Settings, sound and feel (Phase G1)

* **Settings (`game/settings.rs`):** controls and comfort, graphics
  `Quality` (Low / Medium / High: shadows and cascades, MSAA, bloom, grass
  density and fade distance), fullscreen, VSync, interface size (`UiScale`),
  minimap, master / effects / music / ambience volume and key overrides. Old
  `settings.ron` files load with defaults for new fields; values are
  clamped. `apply_settings` keeps `Bindings`, the window and `UiScale` in
  step; the camera, the sun's shadows and the grass react to quality
  changes (the grass rebuilds alone).
* **Key bindings (`input`):** `Action` lists the rebindable actions;
  `rebind` sets an action's primary key and swaps with any action that
  already had it; menu keys are reserved. Every on-screen hint reads its
  key from `Bindings`.
* **Settings screen (`ui/settings.rs`):** an overlay over the pause menu or
  the title screen with four tabs, keyboard and mouse; other menus ignore
  their keys while it is open.
* **Sound (`audio.rs`):** effects at the effects volume; footsteps chosen by
  surface (grass, gravel path, bridge wood, water) about every 0.8 m, a
  landing thump and a travel whoosh. Two music loops (day, night) are
  composed by a seeded generator on a background thread (pad on a
  four-chord progression, bass, a sparse kalimba melody on the pentatonic
  scale, echo; seamless loops) and, like the ambience, cross-fade when the
  region changes.
* **Weather (`rendering/wind.rs`):** `Weather::wind` rises and falls over
  minutes; the wind shader, flower sway and clouds follow it.
* **Bloom:** HDR cameras; on Medium and High a thresholded bloom lets only
  bright things glow (lanterns, fireflies, glowing flowers).

## 7i. Performance (Phase G2)

See `docs/performance.md` for the numbers. In short: trees, rocks and path
segments are in spatial grids (`world::grid`), flowers sway on the GPU and
are one entity each, flowers beyond a short distance use a cheap stand-in
without shadows (`flowers::level_of_detail`), and flowers are streamed in
and out around the camera (`flowers::sync_flowers`). `bloom_core::bench`
builds the synthetic fields for `scripts/bench.sh`; `src/bench.rs` is the
in-game benchmark (only active with `BLOOM_BENCH`).

## 7j. Packaging (Phase G3)

The executable is self-contained (content, shaders and sounds are inside
it), so each package is the `dist`-profile binary plus an icon and platform
metadata: `scripts/package-{macos,linux,windows}` and `packaging/` (icon,
Info.plist, desktop entry, Inno Setup script). `src/icon.rs` sets the
window icon on Windows and Linux (the `winit` dependency is the version
Bevy uses). GitHub Actions run fmt, clippy and tests on three systems and
build all packages; see `docs/release.md`.

## 8. Persistence (implemented in Phase B)

* One save slot in the OS data folder (`~/Library/Application Support/Bloom
  and Gather`, `%APPDATA%\Bloom and Gather`, `~/.local/share/bloom-and-gather`;
  override with `BLOOM_SAVE_DIR`).
* Format: a header line `BLOOM-AND-GATHER-SAVE v1 <fnv1a64>` + `GameState` as
  RON. Content ids (not indices) as keys; harvested flowers as bitsets.
* Safe writes: `save.tmp` → fsync → rename. The previous save becomes
  `save.bak` only if it still decodes, so a damaged file never pushes out the
  last good one. Load falls back to the backup; an unreadable save is moved
  aside (`save.unreadable-<time>`) instead of being overwritten.
* `Game::from_state` validates a loaded state against content (known ids,
  bitset sizes, per-species counts match the actual harvested flowers).
* Migrations: `persistence::migrate(version, payload)`; newer-format saves
  are refused, never overwritten.
* Dirty tracking: autosave 2 s after the last change; immediate save on
  pause, on returning to the title and on quit. Saves are ~2 KB, < 1 ms.
* Settings persist separately in `settings.ron` whenever they change.

## 9. Testing strategy

* `cargo test` — unit tests in `bloom_core` modules plus integration tests:
  content integrity, movement (gravity, exact jump height, buffered jumps,
  speeds, stopping, trees, wall sliding, steps, boulders, slopes, bounds) and
  layouts (determinism, clear spawn, props off the path), flowers (exact
  counts, determinism, spacing, clusters, grid vs brute force, picking), game
  rules (harvest once, reach, cooldown, XP/levels, region completion, selling,
  keepsake reserve, tamper detection) and saves (round trip, damage
  detection, backup fallback, quarantine).
* **Smoke test of the real game** (debug builds): `scripts/smoke.sh` boots
  the real window twice with a private save folder. Run 1 walks, sprints,
  looks, jumps, picks five flowers by aiming and pressing E, opens the
  basket, pauses (saves). Run 2 restarts, continues, and verifies the picked
  flowers are still picked, absent from the world and in the basket, and the
  player resumes where they stood. Later phases extend it: tools and every
  harvesting mode (Phase D), walking into the workshop, binding bouquets at
  the workbench and checking they persist (Phase E), buying an upgrade with
the keyboard at the tool shelf and learning skills in the tree (Phase U),
and opening every journal page, checking achievements and distance (Phase P),
then travelling with the world map to every region, checking the save records
the region, and walking back to the meadow through a gate (Phase F).
Run 2 then finishes the game through the real rules, watches the ending
(celebration, two tour stops, Esc to the summary), continues into free roam,
opens the herbarium and the journal, and checks the save records the ending
(Phase Z). Run 1 also opens the settings with O, lowers the quality and
rebinds the map to N by keyboard; run 2 checks both were saved (Phase G1).
Injected input is authoritative (real
  device input is discarded) so the test is deterministic.

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
