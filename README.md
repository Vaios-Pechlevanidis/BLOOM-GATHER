# Bloom & Gather

A cozy, finite, first-person 3D flower-picking and bouquet-crafting game for
the desktop (Windows, macOS, Linux). Walk through five hand-seeded regions,
pick every flower, buy better tools, craft increasingly valuable bouquets, and finish
the game with a real ending. It runs fully offline as a native app, with no
browser and no account.

**Status:** Phase 1 (foundation) and Phase A (first-person 3D foundation) are
complete: you can walk, sprint, jump and look around the Sunny Meadow. Flowers
arrive in Phase B. See [TODO.md](TODO.md) and
[docs/development-roadmap.md](docs/development-roadmap.md).

## Running

Requirements: a recent stable Rust toolchain (`rustup`). On Linux, Bevy also
needs the usual system packages (`libasound2-dev`, `libudev-dev`, and X11 or
Wayland dev headers).

```bash
cargo run              # debug build (dependencies are optimised, so it is smooth)
cargo run --release    # release build
cargo run --features dev   # faster incremental rebuilds (dynamic linking)
```

Controls:

| Action | Input |
|---|---|
| Move | W A S D (or arrow keys) |
| Sprint | Shift |
| Jump | Space |
| Look around | Mouse |
| Start from title screen | Enter or click **Begin** |
| Pause / settings | Esc (sensitivity, field of view, head movement, invert look) |
| Performance overlay | F3 |

E (interact), Tab (inventory) and M (map) are added with the features they open.

## Testing

```bash
cargo test                      # all unit + integration tests
cargo test -p bloom_core        # gameplay/content tests only (no Bevy, fast)
cargo clippy --all-targets
BLOOM_SMOKE_TEST=/tmp/bloom-smoke cargo run   # drive the real game, save screenshots
```

## Project layout

| Path | Purpose |
|---|---|
| `crates/bloom_core/` | Engine-independent, authoritative game rules and content model |
| `assets/data/game_data.ron` | All content and balancing (species, tools, recipes, regions, economy) |
| `src/` | Bevy app: windowing, rendering, UI, input |
| `docs/` | Architecture, game design, economy balancing, roadmap |

## Documentation

* [Architecture](docs/architecture.md)
* [Game design](docs/game-design.md)
* [Economy & balancing](docs/economy-balancing.md)
* [Development roadmap](docs/development-roadmap.md)
