#!/usr/bin/env bash
# Linux packages: a tarball and, when appimagetool is available, an AppImage.
#   scripts/package-linux.sh
#   APPIMAGETOOL=/path/to/appimagetool scripts/package-linux.sh
# Needs the usual Bevy build packages (libasound2-dev, libudev-dev,
# libwayland-dev, libxkbcommon-dev). Players need ALSA/PulseAudio/PipeWire
# and Vulkan drivers (Mesa) as usual for games.
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)
ARCH=$(uname -m)
NAME="bloom-and-gather-$VERSION-linux-$ARCH"
OUT="dist"
mkdir -p "$OUT"

cargo build --profile dist
BIN="target/dist/bloom-and-gather"

# Tarball: the game, its icon and a desktop entry.
STAGE="$OUT/$NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "$BIN" "$STAGE/"
cp packaging/icon/icon-256.png "$STAGE/bloom-and-gather.png"
cp packaging/linux/bloom-and-gather.desktop "$STAGE/"
cp README.md "$STAGE/"
tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo "Built $OUT/$NAME.tar.gz"

# AppImage.
TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -z "$TOOL" ]; then
  echo "appimagetool not found; skipping the AppImage"
  exit 0
fi
APPDIR="$OUT/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/icons/hicolor/256x256/apps" "$APPDIR/usr/share/applications"
cp "$BIN" "$APPDIR/usr/bin/"
cp packaging/icon/icon-256.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/bloom-and-gather.png"
cp packaging/icon/icon-256.png "$APPDIR/bloom-and-gather.png"
cp packaging/linux/bloom-and-gather.desktop "$APPDIR/usr/share/applications/"
cp packaging/linux/bloom-and-gather.desktop "$APPDIR/"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/bloom-and-gather" "$@"
RUN
chmod +x "$APPDIR/AppRun"
ARCH="$ARCH" "$TOOL" "$APPDIR" "$OUT/Bloom-and-Gather-$VERSION-$ARCH.AppImage"
echo "Built $OUT/Bloom-and-Gather-$VERSION-$ARCH.AppImage"
