#!/usr/bin/env bash
# Build "Bloom & Gather.app" and a .dmg for this Mac's architecture.
#   scripts/package-macos.sh            -> dist/Bloom-and-Gather-<version>-macos-<arch>.dmg
#   TARGET=x86_64-apple-darwin scripts/package-macos.sh   (Intel, if the target is installed)
#
# The app is ad-hoc signed (enough to run on Apple Silicon). It is not
# notarized: on first launch macOS asks; right-click > Open (or System
# Settings > Privacy & Security > Open Anyway). To sign and notarize, set
# SIGN_IDENTITY="Developer ID Application: ..." (and notarize the .dmg with
# `xcrun notarytool` afterwards).
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)
TARGET="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
ARCH="${TARGET%%-*}"
APP_NAME="Bloom & Gather"
OUT="dist"
APP="$OUT/$APP_NAME.app"

cargo build --profile dist --target "$TARGET"
BIN="target/$TARGET/dist/bloom-and-gather"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/bloom-and-gather"
cp packaging/icon/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

codesign --force --deep --options runtime --sign "${SIGN_IDENTITY:--}" "$APP"
codesign --verify --deep --strict "$APP"

DMG="$OUT/Bloom-and-Gather-$VERSION-macos-$ARCH.dmg"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
rm -rf "$STAGE"
echo "Built $APP and $DMG ($(du -h "$DMG" | cut -f1))"
