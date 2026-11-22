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

## Phase U — Upgrades & skill tree ✅ (current)
Modifier pipeline, upgrade shop with before/after values, progression guard;
five-branch skill tree UI, SP accounting, prerequisites, tool requirements.

## Phase P — Progression & achievements
Level-up rewards & UI, milestones, achievements and toasts, statistics.

## Phase F — Remaining regions
Lavender Fields, Blossom Valley (streams, bridges, petals), Enchanted Forest
(canopy light, mushrooms, glow plants), Moonlight Garden (night, lanterns,
water). Walkable transitions (gates, bridges, forest entrance), minimap and
world map (M), region unlocks and completion.

## Phase Z — Completion & ending
Completion checker (each condition tested), finale sequence, journey summary,
collection screen, free-roam.

## Phase G — Polish & release
Painted glTF art replacing procedural meshes, day/night and weather,
animation and lighting pass, sound design and music, full settings
(key rebinding, graphics, audio), benchmarks at 1k–500k flowers and
optimisation driven by them, Bevy upgrade, packaging (macOS .app/.dmg,
Windows installer, Linux AppImage/tarball).
