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

## Phase E: Workshop, crafting & economy ✅

- ✅ Walk-in florist's cottage near the spawn (Sunny Meadow, `workshop: true` in data): placed by
  core layout, flattened ground, walls with a door gap, furniture colliders, warm interior lamp
- ✅ Interactables (core `pick_interactable`): workbench and bouquet display → workbench screen,
  tool shelf → Tool Shop, storage chest → basket; HUD prompts with live details
- ✅ Tool Shop moved out of the basket: the shop tab only exists when opened at the tool shelf
- ✅ Core crafting (`game/crafting.rs`): recipe discovery (tools, named species, level) with
  "still needed" reasons, `plan_craft` auto-fill from the most plentiful flowers, keep-out
  species, batch capacity from tools, keepsake reserve for *other* required bouquets checked for
  the whole batch, atomic consume, saved craft timer, bouquets + XP on completion, selling
- ✅ Workbench screen: recipe list, requirements, price vs. raw flower value, exact ingredient
  plan, quantity (-/+/Max, arrows), keep-out chips, Bind (Enter), progress bar; Bouquets tab
  with Sell 1 / Sell all / Sell every bouquet; keys 1/2, Esc closes; crafting keeps running
- ✅ Procedural bouquets (recipe colours, stems, leaves, ribbon) in the display table's vases
- ✅ Tests: 10 crafting tests (incl. crafting every required bouquet from the whole world in
  rarest-first and careless cheapest-first order) + 3 workshop layout tests; 121 tests in all
- ✅ Smoke test walks into the workshop, opens the workbench with E, binds 3 Simple Bouquets,
  checks the Bouquets tab and the display, and verifies bouquets and basket persist across a
  restart

## Phase U: Upgrades & skill tree ✅

- ✅ Content: 10 levelled upgrades and a 21-skill, five-branch tree in `game_data.ron`, built from
  per-level effects (`data/growth.rs`); collection milestones (8, 16 species) in progression
- ✅ Validation: references, value ranges, chain-shaped tree without cycles, stacked reductions
  stay within their floors (no wasted levels), increasing milestones
- ✅ Core modifier pipeline (`game/growth.rs`) applied to pick cooldown, gather radius/capacity,
  cart area, reach, move speed, discovery/crafting XP, binding time, batch capacity, tool,
  upgrade and bouquet prices, region thresholds
- ✅ Progression guard (economy doc §5.2): upgrades can't eat coins needed for missing tools
- ✅ Derived skill points (levels, regions, milestones, Collection Bonuses); learn with
  prerequisites and tool requirements; skills never grant tools, modes or recipes
- ✅ Chance skills (Double Harvest, Efficient Production) on a saved deterministic RNG
- ✅ Region unlocks recorded and announced once; New Meadows and Region Unlock Bonuses
- ✅ Upgrades tab at the tool shelf (before -> after values, guard explanation); shelf screen
  keyboard buying (arrows + Enter)
- ✅ Skill tree screen (K): branches, connectors, details with now -> next, Learn (Enter)
- ✅ HUD: unspent skill points, rare flowers left (skill); toasts for upgrades, skills, region
  unlocks, saved flowers, collection milestones; bonus bouquet prices everywhere
- ✅ Flower Recognition: golden glints above flowers of undiscovered species
- ✅ Tests: 17 new (prices, caps, tool needs, guard, SP sources, prerequisites, stacking and
  floors, bouquet bonuses, discounts, crafting effects, deterministic Double Harvest, Efficient
  Production accounting, early region unlock + bonus, save round trip, tamper checks, content
  rules); 138 tests in all
- ✅ Smoke test buys an upgrade and learns three skills with the keyboard, screenshots the glints,
  and verifies upgrades and skills persist across a restart

## Phase P: Progression & achievements ✅

- ✅ 19 data-driven achievements in five categories (`data/achievements.rs`, `game_data.ron`)
  with conditions, small coin or skill-point rewards; never required for the ending
- ✅ Validation: unique ids, every target reachable in the finite world (flowers, bouquets the
  world could make, species, regions)
- ✅ Core (`game/progress.rs`): achievements re-checked after every command that moves a
  condition, unlocked once, rewards paid once, `AchievementUnlocked` event; older saves catch
  up quietly; skill points from achievements join the derived total
- ✅ Completion check (`Game::completion`) with the six ending requirements (for Phase Z)
- ✅ Statistics: coins spent and distance walked added; rare flowers picked derived
- ✅ Journal screen (J): achievements with progress bars and rewards, statistics, and the road
  to the ending (completion checklist + next level, region, tool, collection milestone and
  closest achievement)
- ✅ Level-up toast says what the level brings (skill point, tools now at the shelf, regions
  that now only wait for flowers); achievement toasts with rewards
- ✅ Toasts move to the bottom edge, title only, while a screen is open, so they never cover a
  heading or button
- ✅ Tests: 8 new (first steps, capped progress, rare finds, crafting achievements, spending and
  distance, the whole journey to completion, quiet catch-up and tamper check, content rules);
  146 tests in all
- ✅ Smoke test opens the journal on all three pages and verifies achievements and distance
  persist across a restart

## Later phases ⬜

F Remaining regions (next) · Z Completion & ending · G Polish & release

## Temporary assets (replace before release)

- Trees, rocks, fence: procedural low-poly meshes in `src/rendering/meshes.rs`
- Flowers: procedural models in `src/flowers/models.rs` (replace per species via `FlowerSpecies::model`)
- Sound effects and ambience: synthesised in `src/audio.rs`
- Landmarks, workshop cottage and furniture, bouquets, glints, butterflies, bees, clouds: procedural meshes
- First-person hand and tools: procedural meshes in `src/viewmodel.rs`
- Terrain: vertex colours only (no textures, no grass yet)
- Sky: flat clear colour + fog (no sky gradient or clouds yet)
- UI font: Bevy's built-in monospace (lacks glyphs such as `·` and `°`; keep UI strings ASCII)

## Known issues / decisions to revisit

- Shutdown occasionally stalls, so far only on the first launch after a rebuild (likely
  pipeline compilation still running). Mitigated: after the final save the game forces the
  exit after 5 s, keeping the exit code. Root cause still to find (Bevy upgrade may fix it).
- Adding landmarks in Phase C (and the workshop in Phase E) moved where flowers are placed, so saves made with the Phase B
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
