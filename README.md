# Bloom & Gather

A cozy, finite, first-person 3D flower-picking and bouquet-crafting game for
the desktop (Windows, macOS, Linux). Walk one big bulb farm, the Tulip Fields,
pick every flower, buy better tools, craft increasingly valuable bouquets, and
finish the game with a real ending. It runs fully offline as a native app,
with no browser and no account.

**The map:** a flat polder farm in the style of the Dutch bulb fields. Long,
raised beds of one colour each (tulips, hyacinth-blue bluebells, daffodils,
roses, lavender, foxgloves and more, shortest by the track, tallest at the
back) run north on both sides of a straight farm track lined with lanterns;
bare furrows lie between them. Wildflowers grow on the verges and around the
farmyard with the workshop, rare strays hide among the beds, and beyond the
fence the neighbours' fields stripe the land in colour up to tree lines on the
horizon. Every pickable flower stands in a clump of blooms that goes with it
when you pick it.

**Day and night:** the sun rises in the east, crosses the south and sets in
the north-west over the beds; a whole day lasts 16 minutes. Sunsets colour the
sky orange and purple, at night the moon and stars come out, the lanterns
light up, night flowers glow, fireflies replace the butterflies and bees, and
the music and ambience turn to night. The HUD shows the time; time stands
still while the game is paused.

**Status:** the game can be played from the first daisy to its ending: pick
the farm's 1,900 flowers by looking at them and pressing E (or with the
basket, garden rake and flower cart), discover species, level up, buy tools at
the workshop's tool shelf, bind bouquets at the workbench and sell them, buy
upgrades and learn skills (K), follow achievements, statistics and the road to
the ending in the journal (J), see the farm on the map (M), and quit and
continue later. Gather everything and the finale plays, then your journey
summary; afterwards you can keep wandering (free roam). Picked flowers never
come back. See [TODO.md](TODO.md) and
[docs/development-roadmap.md](docs/development-roadmap.md).

## Quick start

**Just play** (installs the latest release and starts it; run again to update):

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/Vaios-Pechlevanidis/BLOOM-GATHER/main/scripts/install.sh | bash
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/Vaios-Pechlevanidis/BLOOM-GATHER/main/scripts/install.ps1 | iex
```

**From source** (needs Rust via `rustup`):

```bash
git clone https://github.com/Vaios-Pechlevanidis/BLOOM-GATHER.git && cd BLOOM-GATHER && cargo run --release
```

In game: Enter to begin, WASD + mouse to walk and look, E to pick, Tab for
the basket, Esc to pause. Full controls are under
[Running from source](#running-from-source).

## Download & install

Ready-made packages for macOS, Windows and Linux are on the
[latest release](https://github.com/Vaios-Pechlevanidis/BLOOM-GATHER/releases/latest).
The quickest way is one line in a terminal, which downloads the latest
release, installs it and starts the game (run it again to update):

**macOS** (Apple Silicon and Intel) and **Linux** (x86_64):

```bash
curl -fsSL https://raw.githubusercontent.com/Vaios-Pechlevanidis/BLOOM-GATHER/main/scripts/install.sh | bash
```

**Windows** (PowerShell):

```powershell
irm https://raw.githubusercontent.com/Vaios-Pechlevanidis/BLOOM-GATHER/main/scripts/install.ps1 | iex
```

Where it goes: macOS puts `Bloom & Gather.app` in `/Applications` (or
`~/Applications`); Linux puts the AppImage at `~/.local/bin/bloom-and-gather`
with a menu entry; Windows installs for the current user (no administrator
rights) with a Start menu entry, uninstalled via Settings > Apps.

Or download by hand from the release page:

| System | File |
|---|---|
| macOS, Apple Silicon | `Bloom-and-Gather-<version>-macos-aarch64.dmg` |
| macOS, Intel | `Bloom-and-Gather-<version>-macos-x86_64.dmg` |
| Windows | `Bloom-and-Gather-<version>-windows-x86_64-setup.exe` (or the `.zip`) |
| Linux | `Bloom-and-Gather-<version>-x86_64.AppImage` (or the `.tar.gz`) |

The packages are not signed with a developer certificate yet, so a
download from the browser asks once on first launch:

* **macOS:** open the `.dmg`, drag the app to Applications, then
  right-click it > Open (or System Settings > Privacy & Security > Open
  Anyway). The install script avoids this.
* **Windows:** if SmartScreen says "Windows protected your PC", choose
  More info > Run anyway.
* **Linux:** make the AppImage executable (`chmod +x`). It needs FUSE 2
  (`libfuse2` on Ubuntu/Debian); without it, use the `.tar.gz`.

Saves and settings stay in your user data folder, so updating or
reinstalling keeps your progress.

## Running from source

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
| Basket (inventory, selling) | Tab |
| Use a workshop station (workbench, tool shelf, storage, bouquet display) | E while looking at it |
| Workbench | Up/Down recipe, Left/Right quantity, Enter bind, 1 / 2 tabs, Esc close |
| Tool shelf (Tool Shop, Upgrades) | 2 / 3 tabs, arrows choose, Enter buys |
| Skill tree | K (arrows choose, Enter learns, K / Esc close) |
| Journal (achievements, statistics, progress) | J (1 / 2 / 3 pages, J / Esc close) |
| Map of the farm | M |
| Herbarium (every species, bouquets bound) | C |
| Skip the ending's tour | Esc |
| Settings (from the pause menu or the title screen) | O while paused; 1-4 tabs, arrows, Enter, Esc |
| Look around | Mouse |
| Start from title screen | Enter or click **Begin** |
| Pause / settings | Esc (sensitivity, field of view, sound volume, head movement, invert look) |
| Performance overlay | F3 |

Every main action key can be rebound in Settings > Keys (conflicting keys
swap; Esc, Enter, F3, the arrow keys and 1-4 stay fixed so menus always
work). Hints everywhere show the keys you chose.

Saves live in `~/Library/Application Support/Bloom and Gather` (macOS),
`%APPDATA%\Bloom and Gather` (Windows) or `~/.local/share/bloom-and-gather`
(Linux). The game saves automatically, and when pausing or quitting.

## Testing

```bash
cargo test                      # all unit + integration tests
cargo test -p bloom_core        # gameplay/content tests only (no Bevy, fast)
cargo clippy --all-targets
scripts/package-macos.sh       # macOS .app + .dmg (Linux/Windows scripts and CI: docs/release.md)
scripts/bench.sh               # performance: core + game at 1k-500k flowers (docs/performance.md)
scripts/smoke.sh               # play the real game twice (pick, tools, workshop, upgrades, skills, journal, map, sunset and night, settings, key rebinding, save, restart, verify, ending, herbarium); screenshots
BLOOM_VIEWS=/tmp/views cargo run   # debug builds: screenshots of fixed spots from morning to night, then quit
```

## Project layout

| Path | Purpose |
|---|---|
| `crates/bloom_core/` | Engine-independent, authoritative game rules and content model |
| `assets/data/game_data.ron` | All content and balancing (species, tools, recipes, the farm map, economy) |
| `src/` | Bevy app: windowing, rendering, UI, input |
| `docs/` | Architecture, game design, economy balancing, roadmap |

## Documentation

* [Architecture](docs/architecture.md)
* [Game design](docs/game-design.md)
* [Economy & balancing](docs/economy-balancing.md)
* [Development roadmap](docs/development-roadmap.md)
