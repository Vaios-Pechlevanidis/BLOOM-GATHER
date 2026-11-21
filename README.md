# Bloom & Gather

A cozy, finite, first-person 3D flower-picking and bouquet-crafting game for
the desktop (Windows, macOS, Linux). Walk through five hand-seeded regions,
pick every flower, buy better tools, craft increasingly valuable bouquets, and finish
the game with a real ending. It runs fully offline as a native app, with no
browser and no account.

**Status:** Phases 1, A, B, C and D are complete: walk a lush, breezy Sunny
Meadow in first person, buy tools (scissors, basket, garden rake, flower cart), pick its 100 flowers by looking at them and pressing E, discover
species, level up, sell from your basket, and quit and continue later; picked
flowers never come back. See [TODO.md](TODO.md) and
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
| Pick the flower you're looking at / use the tool | E (hold for the rake; hold and drag for the cart) |
| Switch tool | 1-4 or mouse wheel |
| Basket (inventory, selling) and Tool Shop | Tab (then 1 / 2 to switch tabs) |
| Look around | Mouse |
| Start from title screen | Enter or click **Begin** |
| Pause / settings | Esc (sensitivity, field of view, sound volume, head movement, invert look) |
| Performance overlay | F3 |

M (map) arrives with the world map.

Saves live in `~/Library/Application Support/Bloom and Gather` (macOS),
`%APPDATA%\Bloom and Gather` (Windows) or `~/.local/share/bloom-and-gather`
(Linux). The game saves automatically, and when pausing or quitting.

## Testing

```bash
cargo test                      # all unit + integration tests
cargo test -p bloom_core        # gameplay/content tests only (no Bevy, fast)
cargo clippy --all-targets
scripts/smoke.sh               # play the real game twice (pick, save, restart, verify); screenshots
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
