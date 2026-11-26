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

- ✅ Wind shader (`src/rendering/wind.wesl`, WGSL until G4): extended StandardMaterial whose vertex shader sways
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

## Phase F: Remaining regions ✅

- ✅ Data-driven region scenery (`data/scenery.rs`, `scenery:` in `game_data.ron`): entrance gate
  style, ambience, light and air (sun, ambient, sky zenith, fog, night) and features (flower rows,
  stream, pond, lanterns, mushrooms, glow plants, light shafts, petals, fireflies, blossom trees,
  tree heights, grass density); validation keeps water wadeable and values in range
- ✅ Layout (`world/features.rs`): gates where the path meets the north/south fence (trigger,
  arrival spot), stream carved into a level valley floor, pond, wooden bridges with walkable
  arched decks and railings wherever the path crosses water, lantern posts along the path;
  flowers stay out of water, lavender grows in rows
- ✅ Sunny Meadow unchanged: new features use their own random streams, a fingerprint test locks
  every meadow flower's position, so existing saves stay valid
- ✅ Core travel (`game/travel.rs`): region chain, gate destinations, arrival sides, unlock
  requirements, `enter_region` (refused while locked), `visited_regions` saved and checked
- ✅ Journeys (`travel.rs`): walk into an open gate or pick a region on the map; fade out, rebuild
  the region (scene, flowers, grass, sky, critters, scenery, ambience), arrive at the matching
  gate, fade in; movement frozen meanwhile; gate banner says where it leads or what it needs
- ✅ Gates per region (farm gate, stone arch with blossom, forest arch, lantern arch), barred
  while locked and opened the moment the region unlocks; fence gaps only where a gate leads on
- ✅ Region looks: golden light and tilled rows (lavender); stream, bridge, blossom trees, falling
  and floating petals (valley); tall trees, light shafts, toadstools, glowing ferns, fireflies,
  closer fog (forest); night sky with stars and moon, moonlight, lit lanterns, pond with lily
  pads, glow plants, fireflies, no butterflies (garden)
- ✅ Per-region ambience: meadow, valley stream, forest, night crickets (synthesised loops)
- ✅ Minimap (HUD corner) and world map (M): region cards with state and flowers left, a drawn map
  of every visited region with unpicked flowers, travel with Enter or the button; time paused
- ✅ HUD region name follows travel; welcome toast on a first visit; "Regions visited" in the
  journal; the save remembers the region you stand in
- ✅ Tests: 11 new (gates on the fence, walking every region gate to gate with the real controller,
  stream + bridge crossing on the deck, shallow water with walkable banks and no flowers, wading
  out of any water, lavender rows, lanterns, tall forest, gate chain, locked regions, first visit
  + save + tamper check) plus the meadow fingerprint; 158 tests in all
- ✅ Smoke test opens the map, travels to all four new regions (screenshots), checks the save
  records the region, walks back to the meadow through a gate, and verifies visited regions
  persist across a restart

## Phase Z: Completion & ending ✅

- ✅ Core: completion checked after every command that can complete the game; recorded once
  as a saved `JourneySummary` with a `GameCompleted` event; `finale_seen` and `finale_pending`
  (an interrupted ending plays again); older saves catch up quietly; forged endings refused
- ✅ Finale: celebration (golden petals, title), tour of all five regions gliding along each
  path while every picked flower blooms again in memory (glowing, see-through, with a light in
  its colour), back home, journey summary; Esc skips the tour; HUD, toasts and hands hidden;
  saves keep the home region while touring
- ✅ Journey summary: flowers (rare), bouquets (sold), coins (spent), time, distance, level,
  tools, regions, recipes, upgrade and skill levels, achievements, the full collection; Keep
  wandering / Collection / Title screen
- ✅ Free roam afterwards: HUD says so, the journal's Progress page links to the summary
- ✅ Herbarium (C): every species card (colours, rarity, picked of total, description, or where
  an undiscovered one grows) and the bouquets bound
- ✅ Tests: 7 new (an unpicked flower, a missing tool and an unbound bouquet each keep the game
  open and complete it when done; species and recipes counted on the way; the summary is a
  snapshot that free roam doesn't change and completion happens once; the ending survives
  saves and resumes if unseen; forged endings refused); 165 tests in all
- ✅ Smoke test run 2 finishes the game through the real rules and watches the ending, free
  roam, herbarium and journal, and checks the save records it

## Phase G1: Settings & feel ✅

- ✅ Settings model: master / effects / music / ambience volume, graphics quality (Low / Medium /
  High), fullscreen, VSync, interface size, minimap, key overrides; older settings files load
- ✅ Settings screen (O in the pause menu, or the title screen): Controls, Video, Audio and Keys
  tabs, fully usable by keyboard (1-4, arrows, Enter, Esc) or mouse; changes apply at once
- ✅ Key rebinding for the twelve main actions; conflicts swap; Esc, Enter, F3, arrows and 1-4
  stay fixed; reset to defaults; the HUD hint, close buttons and toasts show the chosen keys
- ✅ Graphics quality drives shadows and cascades, MSAA, bloom, grass density and fade distance
  (grass rebuilds on change); HDR cameras with a thresholded bloom for lanterns and glows
- ✅ Music: two calm, seeded, seamless loops (day F major, night D minor) composed in the
  background at startup; music and ambience cross-fade when you travel
- ✅ Footsteps by surface (grass, gravel path, bridge wood, water), landing thump, travel whoosh
- ✅ Weather: the breeze rises and falls over minutes (grass, canopies, flowers, clouds)
- ✅ Tests: 6 new (rebinding swaps and reserved keys, key names and bad overrides, settings
  compatibility, volume channels and steps, music loops, footstep sounds, the breeze); 171
  tests in all
- ✅ Smoke test opens the settings by keyboard, changes the quality, rebinds the map key and
  verifies both were saved

## Phase G2: Performance ✅

- ✅ Benchmarks at 1k / 10k / 100k / 500k flowers: core (`--example bench`: placement, queries,
  harvesting, saves) and the real game (`BLOOM_BENCH`, a 12 s walk, FPS and p99 frame time);
  `scripts/bench.sh` runs both; results and analysis in `docs/performance.md`
- ✅ Spatial grids for trees, rocks and footpath segments: placement at 100k 751 → 75 ms, save
  restore 799 → 129 ms (answers unchanged; the meadow fingerprint test still holds)
- ✅ Flowers sway on the GPU (wind shader) and are one entity each (plus a glow child at night)
- ✅ Level of detail: flowers beyond 12 / 22 / 26 m (by quality) use a ~50-triangle stand-in
  without shadows (the full model is ~2,900 triangles); refreshed from the flower grid
- ✅ Flowers stream in and out around the camera (90 / 120 / 160 m); real regions fit entirely
- ✅ Result at High: 10k flowers 24 → 49 FPS, 100k 9.5 → 29, 500k 4.7 → 26 (Medium: 50 FPS at
  500k; Low: 60 at every size); real regions stay at the display's 60 FPS

## Phase G3: Packaging ✅

- ✅ App icon drawn by `scripts/make_icon.py`: `.icns`, `.ico`, PNGs and a raw RGBA window icon
  (set at runtime on Windows and Linux); committed, so packaging needs no Python
- ✅ `dist` build profile (release, thin LTO, one codegen unit, stripped)
- ✅ macOS: `scripts/package-macos.sh` builds `Bloom & Gather.app` (Info.plist, icon, ad-hoc
  signature, optional Developer ID) and a `.dmg` with an Applications link; Intel via `TARGET`
- ✅ Linux: `scripts/package-linux.sh` builds a tarball (binary, icon, desktop entry) and an
  AppImage when `appimagetool` is available
- ✅ Windows: `scripts/package-windows.ps1` builds a zip and an Inno Setup installer (per-user by
  default, Start menu and optional desktop shortcut)
- ✅ GitHub Actions: CI (fmt, clippy, tests on Linux, macOS, Windows) and a release workflow
  that builds every package and, for a `v*` tag, publishes a GitHub release with them
  (macOS jobs on `macos-latest`; `macos-14` is no longer offered)
- ✅ Install from GitHub with one line: `scripts/install.sh` (macOS, Linux) and
  `scripts/install.ps1` (Windows) download the latest release, install it and start the game;
  README "Download & install" section with the manual downloads and first-launch notes
- ⬜ First release: push a `v0.1.0` tag, check the workflow run, then test the install
  scripts against the real release
- ✅ Tested here: the macOS `.dmg` built, mounted, and the game in it ran from `/`; the Windows
  and Linux scripts and the workflows run on GitHub and have not run yet
- ⬜ Code signing and notarization need an Apple Developer account and a Windows certificate

## Phase G4: Bevy upgrade ✅

- ✅ Bevy 0.16 → 0.17 → 0.18 → 0.19 → 0.20, one version at a time, each built, tested and
  smoke-tested; `bloom_core` follows Bevy's glam (0.29 → 0.30 → 0.32 → 0.33), core unchanged
- ✅ Events → messages, `DespawnOnExit`, new module paths (camera, light, mesh, post_process),
  `Hdr` and `CursorOptions` components, `BorderColor::all`, `BorderRadius` as a `Node` field
  (39 places), `GlobalAmbientLight`, `FontSize`, `TextLayout::justify` / `no_wrap`,
  `shadow_maps_enabled`, `.or_else` run conditions
- ✅ Wind shader rewritten in WESL (`wind.wesl`: imports, `@if`, material bind group constant,
  compressed vertices)
- ✅ Buttons: headless `ui_widgets::Button` with `Hovered`/`Pressed`; clicks become `Clicked`
  messages for every screen (no deprecated `Interaction`); the smoke test now also clicks a
  real button ("Resume")
- ✅ Window icon via the `WINIT_WINDOWS` thread-local
- ✅ High quality keeps full flower models to 26 m (was 32): under 0.20 the 32 m setting pushed
  High just over the 60 Hz budget at 2560x1600 on an M2
- ✅ 176 tests, clippy clean, both smoke runs pass on 0.20; the macOS package rebuilt and run

## Phase M: One map, day and night ✅

- ✅ The five regions became one map, the Tulip Fields (52 x 88 m, 1,900 flowers, all 17
  species); tulips are the main crop; the older valley content lives on as a test fixture
  (`crates/bloom_core/tests/fixtures/valley.ron`) for the multi-region rules the engine keeps
- ✅ Bulb fields in data (`fields:` in scenery) and core (`world/fields.rs`): straight raised beds
  and furrows on both sides of a straight track, a farmyard with the workshop, bench and well,
  a headland halfway; flowers planted in beds by height (short by the track, tall at the back),
  wildflowers on the verges, rare strays scattered singly; the bed spacing adapts to the count
- ✅ Day cycle in data (`day_cycle:`) and `rendering/daylight.rs`: sun path east-south-north-west,
  sunrise/sunset colours, moonlight, ambient light, fog and sky follow the hour; 16 minutes per
  day, paused with the game; clock in the HUD
- ✅ Sky: moving sun and moon discs with halos, a sunset glow on the dome, stars fading in,
  clouds tinted by the time of day
- ✅ Lanterns light up at dusk; night flowers glow after dark; fireflies at night, butterflies and
  bees by day; night music and crickets after dark
- ✅ Look: sandy furrows and leafy bed crowns in the terrain, tulip leaves on the beds instead of
  grass, every bed flower drawn as a clump of blooms that goes with it when picked
  (`rendering/beds.rs`), neighbours' fields in colour bands to the horizon with tree lines and barns
- ✅ Skills and achievements without regions (Wide Rows replaces New Meadows / Region Unlock
  Bonuses; Harvest Home replaces the region achievements)
- ✅ Tests: farm placement, beds of one colour, tall at the back, farmyard, the whole farm to the
  ending, sun path, sunset, night, clock; smoke test checks the map, the save, sunset, night and
  passing time; `BLOOM_VIEWS` screenshot tool for the look
- ✅ Older saves from the five-region game no longer match the world: they are refused (tested), set
  aside as `save.unreadable-*`, and a new game starts

## Later phases ⬜

Art pass (painted models need real 3D assets; flower pictures can already replace a species'
model, see `assets/flowers/README.md`)

## Temporary assets (replace before release)

- Trees, rocks, fence: procedural low-poly meshes in `src/rendering/meshes.rs`
- Flowers: procedural models in `src/flowers/models.rs`; a painted picture in `assets/flowers/<id>.png`
  replaces one per species (a card turned to the player, `src/flowers/sprites.rs`; format in `assets/flowers/README.md`).
  Pictures so far: daisy
- Sound effects, footsteps, ambience and music: synthesised in `src/audio.rs`
- Landmarks, workshop cottage and furniture, bouquets, glints, butterflies, bees, clouds: procedural meshes
- Gates, bridges, lanterns, water surfaces, mushrooms, glow plants, light shafts, petals, fireflies,
  stars and moon: procedural meshes
- First-person tools (scissors, basket, rake tines, cart rake): procedural meshes in `src/viewmodel.rs`;
  the hands are models (`assets/hands/README.md`)
- Terrain: vertex colours only (no textures)
- Water: flat translucent surfaces (no waves or reflections)
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
- The smoke test locks the mouse cursor while it runs (~20 s per run) and ignores real input.
- Travelling rebuilds the whole region behind a short fade (30-90 ms per region measured in the
  smoke test); no preloading of the neighbour yet.
- The workshop exists only in the Sunny Meadow; the world map is the quick way back to it.
- macOS: transitive dependency `block v0.1.6` emits a future-incompatibility warning.
