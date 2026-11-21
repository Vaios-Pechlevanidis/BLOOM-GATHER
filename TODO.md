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

## Phase B: First 3D flower field ⬜ (next)

- ⬜ Revisit species roster to include bluebell, daffodil, hydrangea, wildflowers (keep counts/balance tests green)
- ⬜ `FlowerField` SoA + harvested bitset; deterministic clustered placement avoiding path/props/spawn
- ⬜ Spatial grid; ray picking within interact range (tests vs brute force)
- ⬜ 3D flower models per species (distinct silhouettes, stems, leaves), size/rotation variation
- ⬜ Look-at highlight, name + "Press E to harvest" prompt; add E binding
- ⬜ Harvest animation, particles, sound; duplicate-harvest prevention
- ⬜ Inventory (Tab), species discovery, coins & selling, XP/levels core
- ⬜ Save system (atomic, versioned, backup, autosave) incl. settings; Continue/New Game
- ⬜ Extend smoke test: harvest a flower, restart, verify it stays harvested

## Later phases ⬜

C Sunny Meadow complete · D Tools & harvesting modes · E Workshop, crafting & economy ·
U Upgrades & skill tree · P Progression & achievements · F Remaining regions ·
Z Completion & ending · G Polish & release

## Temporary assets (replace before release)

- Trees, rocks, fence: procedural low-poly meshes in `src/rendering/meshes.rs`
- Terrain: vertex colours only (no textures, no grass yet)
- Sky: flat clear colour + fog (no sky gradient or clouds yet)
- UI font: Bevy's built-in monospace (lacks glyphs such as `·` and `°`; keep UI strings ASCII)

## Known issues / decisions to revisit

- Shutdown hung once (1 of 6 smoke runs, first run after a rebuild; possibly
  waiting on shader compilation). Not reproduced since. The smoke test force-exits
  after 10 s to stay reliable; investigate with the Quit button during Phase B.
- Bevy pinned to 0.16 (latest 0.20). Upgrade confined to `src/`; plan before polish.
- Settings don't persist yet (arrive with the save system in Phase B).
- Key rebinding UI arrives with the full settings screen (Phase G); bindings are
  already data in `Bindings`.
- The smoke test locks the mouse cursor while it runs (~20 s), like normal play.
- macOS: transitive dependency `block v0.1.6` emits a future-incompatibility warning.
