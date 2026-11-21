# TODO / Development status

Legend: ✅ done · 🔨 in progress · ⬜ not started. Phases are described in
[docs/development-roadmap.md](docs/development-roadmap.md).

## Phase 1: Technical foundation ✅

- ✅ Cargo workspace: game binary + engine-independent `bloom_core` crate
- ✅ App states (Boot → MainMenu → InGame{Playing, Paused}), state-scoped entities
- ✅ Content model + validation (references, counts, profitability, affordability, reachability)
- ✅ Level curve, economy config, deterministic RNG
- ✅ Design tokens, buttons, main menu, pause menu, F3 overlay
- ✅ Docs: README, architecture, game design, economy balancing, roadmap

## Phase A: First-person 3D foundation ✅

- ✅ World units switched to metres; per-region terrain + prop settings and player tuning in data
- ✅ `bloom_core::world::terrain`: seeded fBm heightfield, edge berm, flattened spawn, outskirts
- ✅ `bloom_core::world::layout`: spawn, footpath, trees, rocks, scenery trees, invisible walls
- ✅ `bloom_core::world::collision`: cylinder + rotated-box colliders, steps, support heights
- ✅ `bloom_core::player`: kinematic controller (accel/decel, sprint, jump buffer, coyote time,
  exact jump arc, slope limit, collide-and-slide, step-up, ground snap)
- ✅ 3D scene: chunked vertex-coloured terrain with footpath, 3 tree and 3 rock variants
  (merged meshes, one shared material), fence, scenery beyond the fence
- ✅ Sun with cascaded shadows, ambient light, distance fog, pastel sky
- ✅ First-person camera: fixed-step physics + interpolation, mouse look, pitch clamp,
  head bob and landing dip (toggleable), blend from menu camera
- ✅ Cursor locked while playing; released in menus; auto-pause on focus loss
- ✅ Pause menu settings: mouse sensitivity, FOV, head movement, invert look
- ✅ HUD: region card, crosshair, controls hint; F3 shows player position/speed/state
- ✅ Title screen over the 3D meadow with a slow orbit
- ✅ Tests: 62 core tests (17 movement/layout) + 1 app test + real-game smoke test (18 checks)

## Phase B: First 3D flower field ✅

- ✅ Species roster: daffodil, bluebell, hydrangea and meadow clover (wildflower) added in place of
  buttercup, cornflower and camellia; counts and balance invariants unchanged; 17 species
- ✅ Data-driven flower visuals: `shape` (17 silhouette families), botanical `height`, `glow`;
  `world.flower_scale` (2.2×) shared by rendering and picking
- ✅ `BitSet` (set-only) and `FlowerField` (SoA, deterministic clustered placement avoiding path,
  props, spawn and steep ground; spatial grid; radius queries; ray picking)
- ✅ `Game`: harvest (reach, cooldown, once only), inventory, discovery, XP/levels, region
  completion reward, selling at the single flower price, keepsake reserve for required bouquets,
  tamper-checked restore from a saved state
- ✅ Save system: versioned + checksummed file, atomic writes, good-backup rotation, fallback to
  backup, quarantine of unreadable saves, migration hook; autosave + save on pause/title/quit
- ✅ Settings persist (`settings.ron`); new Sound volume setting
- ✅ Title screen: Continue / New Game (confirmation) / Quit; save notices
- ✅ 17 procedural 3D flower models; wind sway; look-at highlight; pick animation + petal burst
- ✅ E to pick with name/rarity prompt; Tab basket (filter, sort, sell 1 / sell all / sell
  everything, "kept" flowers shown); HUD: remaining flowers, collection, coins, level, XP bar
- ✅ Toasts (discoveries, level-ups, region complete, refusals); synthesised sound effects
- ✅ Tests: 93 core + 4 app tests; two-run smoke test (`scripts/smoke.sh`) verifying picking and
  persistence across a real restart

## Phase C: Sunny Meadow complete ✅

- ✅ Wind shader (`src/rendering/wind.wgsl`): extended StandardMaterial whose vertex shader sways
  vertices by their vertex-colour alpha; world-space phase so gusts roll across the field
- ✅ Decorative grass: ~135k tapered blades in 16 m chunks, swaying, distance-faded
  (visibility ranges), no shadows; keeps clear of path, props, landmarks and every flower
- ✅ Tree canopies sway (trunks stay rooted)
- ✅ Sky dome with a soft gradient (horizon = fog colour) and 16 drifting clouds
- ✅ Butterflies (4 colourways) and bees visiting still-blooming flowers; freeze on pause
- ✅ Landmarks with collision and flattened ground: bench by the path, stone well, signpost at the
  path's end; trees, rocks, flowers and grass keep clear (core layout + tests)
- ✅ Ambient meadow loop (breeze + birdsong, synthesised, seamless), follows volume setting
- ✅ Flower stems drawn sturdier; highlight toned down
- ✅ Quit watchdog: after the final save, a stalled shutdown is forced to finish (keeps exit code)
- ✅ Smoke test measures FPS (debug build: 56-62 FPS on an Apple M2 with everything on)

## Phase D: Tools & harvesting modes ✅

- ✅ Tool effects converted to metres (basket 1.6 m / 6, rake 2.2 m / 4 per 0.25 s, cart 8 x 8 m / 40)
  plus per-mode cooldowns and ground reach in data
- ✅ Core: `buy_tool` (level, prerequisites, coins; atomic), `tool_status`, `available_modes`,
  `mode_params`, `preview_circle` / `preview_area`, `harvest_radius` / `harvest_sweep` /
  `harvest_area` sharing one apply path with single picks; `Terrain::raycast`,
  `FlowerField::query_rect`; 9 new tests
- ✅ Tool selection: keys 1-4 and mouse wheel; HUD tool bar of owned tools
- ✅ Ground aiming with terrain-following marker ring / rectangle; every flower a tool would take
  is highlighted; per-tool prompts ("3 flowers in reach", "Hold to sweep", "Release to gather")
- ✅ First-person viewmodel on its own render layer (never clips into the world): hand, scissors
  (snip), basket (scoop), hand rake (sweeps while held), cart's long rake (pull); walk bob and
  mouse sway
- ✅ Tool Shop tab in the basket screen (Buy / Owned / what's missing)
- ✅ Fixed one-frame aiming lag (aim/pick read the camera's current Transform)
- ✅ Toasts capped at three even when many arrive at once
- ✅ Smoke test buys tools, opens the shop, uses every mode through real input and verifies tools
  and picks persist across a restart; injected held keys now behave like real held keys

## Phase E: Workshop, crafting & economy ⬜ (next)

- ⬜ A cottage workshop building near the spawn (walk-in, collision, landmark placement in core)
- ⬜ Interactables: workbench → crafting, storage → basket, tool display → tool shop (moves the
  shop out of the basket screen), bouquet display
- ⬜ Core crafting: recipe discovery, ingredient selection (auto-fill + manual), validation, atomic
  batch crafting, craft timer, keepsake reserve applies to other recipes, bouquet inventory
- ⬜ Selling bouquets; bouquet 3D models on the display
- ⬜ Tests: tool requirements, ingredient consumption, nothing consumed on failure, reserve

## Later phases ⬜

U Upgrades & skill tree · P Progression & achievements · F Remaining regions ·
Z Completion & ending · G Polish & release

## Temporary assets (replace before release)

- Trees, rocks, fence: procedural low-poly meshes in `src/rendering/meshes.rs`
- Flowers: procedural models in `src/flowers/models.rs` (replace per species via `FlowerSpecies::model`)
- Sound effects and ambience: synthesised in `src/audio.rs`
- Landmarks, butterflies, bees, clouds: procedural meshes
- First-person hand and tools: procedural meshes in `src/viewmodel.rs`
- Terrain: vertex colours only (no textures, no grass yet)
- Sky: flat clear colour + fog (no sky gradient or clouds yet)
- UI font: Bevy's built-in monospace (lacks glyphs such as `·` and `°`; keep UI strings ASCII)

## Known issues / decisions to revisit

- Shutdown occasionally stalls, so far only on the first launch after a rebuild (likely
  pipeline compilation still running). Mitigated: after the final save the game forces the
  exit after 5 s, keeping the exit code. Root cause still to find (Bevy upgrade may fix it).
- Adding landmarks in Phase C moved where flowers are placed, so saves made with the Phase B
  build no longer match the world; the game detects this, keeps the old file as
  `save.unreadable-*` and starts fresh. Before 1.0, layout changes can do this again; after
  1.0, seeds and placement are frozen (content version + migration).
- Grass and tree shadows: shadows use the un-swayed shape (prepass shader not overridden).
- Bevy pinned to 0.16 (latest 0.20). Upgrade confined to `src/`; plan before polish.
- Key rebinding UI arrives with the full settings screen (Phase G); bindings are
  already data in `Bindings`.
- The smoke test locks the mouse cursor while it runs (~20 s per run) and ignores real input.
- Flowers are spawned for the whole region (≤ 750); chunk streaming waits for benchmarks.
- macOS: transitive dependency `block v0.1.6` emits a future-incompatibility warning.
