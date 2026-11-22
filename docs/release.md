# Bloom & Gather — Packaging and release

The game is a single self-contained executable: content, shaders, sounds
and music are compiled in or generated at startup, and saves and settings
live in the OS data folder. A package is that executable plus an icon and
platform metadata. Packages use the `dist` profile (release, thin LTO, one
codegen unit, stripped).

## Building packages

| Platform | Command | Output (in `dist/`) |
|---|---|---|
| macOS | `scripts/package-macos.sh` | `Bloom & Gather.app`, `Bloom-and-Gather-<version>-macos-<arch>.dmg` |
| macOS (Intel, from Apple Silicon) | `rustup target add x86_64-apple-darwin` then `TARGET=x86_64-apple-darwin scripts/package-macos.sh` | `…-macos-x86_64.dmg` |
| Linux | `scripts/package-linux.sh` (AppImage when `appimagetool` is on `PATH` or in `APPIMAGETOOL`) | `bloom-and-gather-<version>-linux-<arch>.tar.gz`, `Bloom-and-Gather-<version>-<arch>.AppImage` |
| Windows | `pwsh scripts/package-windows.ps1` (installer needs Inno Setup 6) | `Bloom-and-Gather-<version>-windows-x86_64.zip`, `…-setup.exe` |

The version comes from `Cargo.toml` (`[workspace.package] version`).

The icon (`packaging/icon/`) is drawn by `scripts/make_icon.py` (Pillow)
in every size the platforms need: `.icns` for macOS, `.ico` for Windows,
PNGs for Linux, and a raw 64 × 64 RGBA image that the game sets as its
window icon on Windows and Linux. The generated files are committed, so
packaging needs no Python.

## Continuous integration (`.github/workflows/`)

* **CI** (`ci.yml`), on every push and pull request, on Linux, macOS and
  Windows: `cargo fmt --check`, `cargo clippy`, `cargo test`. The smoke test
  needs a real window and GPU, so it stays a local check
  (`scripts/smoke.sh`).
* **Release** (`release.yml`), by hand (Actions > Release > Run workflow)
  or on a version tag (`git tag v0.2.0 && git push origin v0.2.0`): builds
  the macOS (Apple Silicon and Intel), Windows and Linux packages and keeps
  them as workflow artifacts. For a tag it also **publishes** a GitHub
  release with the packages attached and marks it as the latest, so the
  download links and the install scripts pick it up at once.

## Installing from GitHub

Players install with one line (see the README's "Download & install"):

* `scripts/install.sh` (macOS, Linux): asks the GitHub API for the latest
  release, picks the `.dmg` for the Mac's processor or the `.AppImage`,
  downloads it with curl and installs it. macOS: copies the app to
  `/Applications` (`~/Applications` if that isn't writable) and clears the
  quarantine flag; a curl download carries none anyway, so Gatekeeper does
  not ask. Linux: `~/.local/bin/bloom-and-gather`, the icon and a desktop
  entry. Then it starts the game.
* `scripts/install.ps1` (Windows): downloads the `-setup.exe` and runs it
  silently for the current user, then starts the game.

Options: `BLOOM_NO_LAUNCH=1` (install only), `BLOOM_INSTALL_DIR`,
`BLOOM_REPO=owner/name` (a fork's releases), and for `install.sh`
`BLOOM_PACKAGE=<file>` to install a local `.dmg` / `.AppImage` (used for
testing before a release exists).

## Signing

Nothing is signed with a developer certificate yet, so first launches ask
the player once:

* **macOS:** the app is ad-hoc signed (required on Apple Silicon) but not
  notarized. First launch: right-click the app > Open (or System Settings >
  Privacy & Security > Open Anyway). To sign and notarize: build with
  `SIGN_IDENTITY="Developer ID Application: <name> (<team>)"
  scripts/package-macos.sh`, then `xcrun notarytool submit <dmg> --wait`
  and `xcrun stapler staple <dmg>`. This needs an Apple Developer account.
* **Windows:** SmartScreen may show "Windows protected your PC" for an
  unsigned installer: More info > Run anyway. Signing needs a code-signing
  certificate (`signtool sign` on the `.exe` and the installer).
* **Linux:** no signing is expected.

## What was tested

macOS (Apple Silicon): the `.dmg` was built locally, mounted, and the game
inside it started from `/` as Finder would start it, and ran the in-game
benchmark to the end. `scripts/install.sh` installed that `.dmg` into a
test folder (`BLOOM_PACKAGE`): the app's signature verified, it carried no
quarantine flag, and the installed game ran the benchmark. The Windows and
Linux packaging and install scripts and the release workflow run on
GitHub's machines and other systems; they have not run yet.
