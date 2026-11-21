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

## Phase C: Sunny Meadow complete ⬜ (next)

- ⬜ Instanced, animated grass (decorative, never harvestable) so the meadow feels lush
- ⬜ Wind on trees; butterflies and bees; drifting pollen
- ⬜ Sky gradient and clouds; soft time-of-day lighting
- ⬜ Ambient meadow audio loop (birdsong, breeze)
- ⬜ A landmark or two (well, bench, signpost to the next region)
- ⬜ Flower model pass after seeing them among grass (thicker stems, leaf shapes)

## Later phases ⬜

D Tools & harvesting modes · E Workshop, crafting & economy ·
U Upgrades & skill tree · P Progression & achievements · F Remaining regions ·
Z Completion & ending · G Polish & release

## Temporary assets (replace before release)

- Trees, rocks, fence: procedural low-poly meshes in `src/rendering/meshes.rs`
- Flowers: procedural models in `src/flowers/models.rs` (replace per species via `FlowerSpecies::model`)
- Sound effects: synthesised in `src/audio.rs`
- Terrain: vertex colours only (no textures, no grass yet)
- Sky: flat clear colour + fog (no sky gradient or clouds yet)
- UI font: Bevy's built-in monospace (lacks glyphs such as `·` and `°`; keep UI strings ASCII)

## Known issues / decisions to revisit

- Shutdown hung once (1 of 6 smoke runs, first run after a rebuild; possibly
  waiting on shader compilation). Not reproduced since. The smoke test force-exits
  after 10 s to stay reliable; investigate with the Quit button during Phase B.
- Bevy pinned to 0.16 (latest 0.20). Upgrade confined to `src/`; plan before polish.
- Key rebinding UI arrives with the full settings screen (Phase G); bindings are
  already data in `Bindings`.
- The smoke test locks the mouse cursor while it runs (~20 s per run) and ignores real input.
- Flowers are spawned for the whole region (≤ 750); chunk streaming waits for benchmarks.
- macOS: transitive dependency `block v0.1.6` emits a future-incompatibility warning.
