#!/usr/bin/env bash
# Install (or update) Bloom & Gather from the latest GitHub release and start
# it. macOS (Apple Silicon and Intel) and Linux (x86_64):
#
#   curl -fsSL https://raw.githubusercontent.com/Vaios-Pechlevanidis/BLOOM-GATHER/main/scripts/install.sh | bash
#
# macOS: copies "Bloom & Gather.app" to /Applications (or ~/Applications if
#        that isn't writable). Downloaded this way the app carries no
#        quarantine flag, so it opens without the Gatekeeper warning.
# Linux: puts the AppImage at ~/.local/bin/bloom-and-gather and adds a menu
#        entry with the icon.
#
# Options (environment variables):
#   BLOOM_NO_LAUNCH=1       install only, don't start the game
#   BLOOM_INSTALL_DIR=...   where the app goes (macOS) / the binary (Linux)
#   BLOOM_PACKAGE=file      install this .dmg / .AppImage instead of downloading
#   BLOOM_REPO=owner/name   another fork's releases
set -euo pipefail

REPO="${BLOOM_REPO:-Vaios-Pechlevanidis/BLOOM-GATHER}"
APP_NAME="Bloom & Gather"

say() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

os="$(uname -s)"
arch="$(uname -m)"
case "$arch" in
  arm64 | aarch64) arch_mac="aarch64"; arch_linux="aarch64" ;;
  x86_64 | amd64) arch_mac="x86_64"; arch_linux="x86_64" ;;
  *) die "unsupported processor: $arch" ;;
esac
case "$os" in
  Darwin) pattern="-macos-$arch_mac\\.dmg" ;;
  Linux) pattern="-$arch_linux\\.AppImage" ;;
  *) die "unsupported system: $os (on Windows use scripts/install.ps1)" ;;
esac

tmp="$(mktemp -d)"
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT

package="${BLOOM_PACKAGE:-}"
if [ -z "$package" ]; then
  command -v curl >/dev/null || die "curl is needed"
  say "Looking up the latest release of $REPO"
  api="https://api.github.com/repos/$REPO/releases/latest"
  url="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$api" \
    | grep -o '"browser_download_url": *"[^"]*"' \
    | sed 's/.*"\(https[^"]*\)"/\1/' \
    | grep -E "$pattern\$" | head -n 1 || true)"
  [ -n "$url" ] || die "no package for $os/$arch in the latest release (see https://github.com/$REPO/releases)"
  package="$tmp/$(basename "$url")"
  say "Downloading $(basename "$url")"
  curl -fL --progress-bar -o "$package" "$url"
fi
[ -f "$package" ] || die "package not found: $package"

if [ "$os" = "Darwin" ]; then
  dest="${BLOOM_INSTALL_DIR:-/Applications}"
  if ! [ -w "$dest" ]; then
    dest="$HOME/Applications"
    mkdir -p "$dest"
  fi
  mnt="$tmp/mnt"
  mkdir -p "$mnt"
  hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$package" >/dev/null 2>&1 \
    || die "could not open the disk image"
  rm -rf "$dest/$APP_NAME.app"
  cp -R "$mnt/$APP_NAME.app" "$dest/"
  hdiutil detach "$mnt" >/dev/null 2>&1 || true
  # Only set by browsers, but clear it in case the .dmg came from one.
  xattr -dr com.apple.quarantine "$dest/$APP_NAME.app" 2>/dev/null || true
  say "Installed $dest/$APP_NAME.app"
  if [ -z "${BLOOM_NO_LAUNCH:-}" ]; then
    say "Starting $APP_NAME"
    open "$dest/$APP_NAME.app"
  fi
else
  bindir="${BLOOM_INSTALL_DIR:-$HOME/.local/bin}"
  mkdir -p "$bindir"
  install -m 755 "$package" "$bindir/bloom-and-gather"
  # Menu entry and icon (the icon is inside the AppImage).
  apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"
  mkdir -p "$apps" "$icons"
  (cd "$tmp" && "$bindir/bloom-and-gather" --appimage-extract bloom-and-gather.png >/dev/null 2>&1 \
    && cp squashfs-root/bloom-and-gather.png "$icons/") || true
  cat > "$apps/bloom-and-gather.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=A cozy first-person flower-picking and bouquet-crafting game
Exec=$bindir/bloom-and-gather
Icon=bloom-and-gather
Categories=Game;Simulation;
Terminal=false
EOF
  say "Installed $bindir/bloom-and-gather (and a menu entry)"
  case ":$PATH:" in *":$bindir:"*) ;; *) say "Tip: add $bindir to your PATH" ;; esac
  if [ -z "${BLOOM_NO_LAUNCH:-}" ]; then
    say "Starting $APP_NAME"
    nohup "$bindir/bloom-and-gather" >/dev/null 2>&1 &
  fi
fi
