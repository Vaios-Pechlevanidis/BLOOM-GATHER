# Bloom & Gather — Development Roadmap

Each phase ends with: `cargo check`, `cargo clippy`, `cargo test`, the
real-game smoke test, updating `TODO.md` and the docs, and a written phase
report. A phase does not start until the previous one is implemented, tested
and working.

The game is a **first-person 3D** game. The original systems roadmap
(inventory, economy, crafting, upgrades, skills, progression, regions,
ending) is merged into the 3D phases below; no gameplay system was dropped.

## Dependency graph

```
P1 Foundation ─► A 3D foundation ─► B Flower field + inventory + saves ─► C Sunny Meadow complete
                                                                              │
             E Workshop, crafting, economy ◄── D Tools & harvesting modes ◄───┘
                     │
                     ▼
             U Upgrades & skill tree ─► P Progression & achievements ─► F Remaining regions
                                                                              │
                                              G Polish & release ◄── Z Completion & ending
```

## Phase 1 — Technical foundation ✅
Rust workspace, Bevy app, states, content model + validation, docs.

## Phase A — 3D foundation ✅
3D rendering, lighting, fog, chunked terrain, trees/rocks/fence with
collision, first-person controller (walk, sprint, jump, gravity, slopes,
steps), mouse look with cursor lock, comfort settings (sensitivity, FOV, head
movement, invert), camera blend from menu, smoke test with real input.

## Phase B — First 3D flower field ✅
`FlowerField` (SoA + bitset) and deterministic clustered placement; 3D models
per species (distinct silhouettes; species roster revisited to include
bluebell, daffodil, hydrangea and wildflower varieties); look-at raycast,
highlight, name + "E" prompt; harvest animation and particle; inventory
(per species, sort/filter, Tab screen); species discovery; coins & selling;
XP/levels core; save system (atomic, versioned, backup, autosave; settings
persist). Tests: placement counts/determinism, ray picking, duplicate-harvest
prevention, save round-trip (harvested flowers never return).

## Phase C — Sunny Meadow complete ✅
Instanced animated grass, wind on flowers and trees, butterflies/bees,
clouds and sky gradient, ambient audio, harvest sounds, region landmarks.

## Phase D — Tools & harvesting modes ✅
First-person hand/scissors/basket/tool models and animations; tool selection;
Radius, Sweep and Area harvesting in 3D with batched harvesting.

## Phase E — Workshop, crafting & economy ✅
Physical 3D workshop (workbench → crafting, storage → inventory, tool display
→ tool shop, bouquet display); recipes, validation, batch crafting with
craft timer, keepsake reserve, bouquet models and selling.

## Phase U — Upgrades & skill tree ✅
Modifier pipeline, upgrade shop with before/after values, progression guard;
five-branch skill tree UI, SP accounting, prerequisites, tool requirements.

## Phase P — Progression & achievements ✅
Level-up rewards & UI, milestones, achievements and toasts, statistics.

## Phase F — Remaining regions ✅
Lavender Fields (flower rows, golden light), Blossom Valley (stream, bridge,
blossom trees, falling petals), Enchanted Forest (tall trees, light shafts,
mushrooms, glow plants, fireflies), Moonlight Garden (night sky, lanterns,
pond, fireflies). Walkable gates between neighbouring regions (farm gate,
stone arch, forest arch, lantern arch; barred while locked), minimap and
world map (M) with travel, per-region light and ambience, region the player
stands in saved.

## Phase Z — Completion & ending ✅
Completion checker (each requirement tested; recorded once with a journey
summary), finale (celebration, tour of all five regions with the picked
flowers blooming in memory, skippable), journey summary, herbarium (C),
free roam, summary reachable from the journal, ending resumes after a
crash.

## Phase G — Polish & release (in parts)

**G1 Settings & feel ✅:** settings screen (controls, video,
audio, keys) with key rebinding, graphics quality levels, fullscreen,
VSync, interface size, separate volumes; calm synthesised music (day and
night), footsteps by surface, landing and travel sounds; a breeze that
rises and falls; soft bloom on bright things.

**G2 Performance ✅:** core and in-game benchmarks at 1k–500k
flowers (`scripts/bench.sh`, results in `docs/performance.md`); spatial
grids for layout and placement, GPU wind sway for flowers, a distant
stand-in model without shadows, flowers streamed around the camera.

**G3 Packaging ✅:** app icon in every format, `dist` profile,
macOS `.app` + `.dmg` (ad-hoc signed), Linux tarball + AppImage, Windows
zip + Inno Setup installer, window icon on Windows/Linux, CI (fmt, clippy,
tests on three systems) and a release workflow building every package
(`docs/release.md`).

**G4 Bevy upgrade ✅ (current):** 0.16 → 0.20 step by step (messages,
despawn-on-exit, UI and rendering API changes, WESL wind shader, headless
buttons with click messages), every step tested and smoke-tested.

Still to come: painted art replacing the procedural models (needs real
assets; a species can already be drawn from a picture, see
`assets/flowers/README.md`), and code signing / notarization (needs an
Apple Developer account and a Windows certificate).
