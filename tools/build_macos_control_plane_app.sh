#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_NAME="Literbike Control Plane"
EXECUTABLE_NAME="LiterbikeControlPlane"
MACOS_DIR="$REPO_ROOT/macos/LiterbikeControlPlane"
BUILD_ROOT="$REPO_ROOT/.artifacts/macos"
APP_BUNDLE="$BUILD_ROOT/$APP_NAME.app"
INSTALL_APP="/Applications/$APP_NAME.app"
INSTALL_APP_FLAG=0

if [[ "${1:-}" == "--install" ]]; then
    INSTALL_APP_FLAG=1
fi

rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources/ControlPlaneResources/configs"

cp "$MACOS_DIR/Info.plist" "$APP_BUNDLE/Contents/Info.plist"

swiftc \
    -O \
    -framework AppKit \
    -framework Network \
    -framework WebKit \
    "$MACOS_DIR/Sources/main.swift" \
    -o "$APP_BUNDLE/Contents/MacOS/$EXECUTABLE_NAME"

cp "$REPO_ROOT/index.html" "$APP_BUNDLE/Contents/Resources/ControlPlaneResources/index.html"
cp "$REPO_ROOT/index.css" "$APP_BUNDLE/Contents/Resources/ControlPlaneResources/index.css"
cp "$REPO_ROOT/bw_test_pattern.png" "$APP_BUNDLE/Contents/Resources/ControlPlaneResources/bw_test_pattern.png"
cp "$REPO_ROOT/literbike-vrod-icon.svg" "$APP_BUNDLE/Contents/Resources/ControlPlaneResources/literbike-vrod-icon.svg"
cp "$REPO_ROOT/configs/agent-host-free-lanes.dsel" "$APP_BUNDLE/Contents/Resources/ControlPlaneResources/configs/agent-host-free-lanes.dsel"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

APP_ICONSET="$tmpdir/AppIcon.iconset"
mkdir -p "$APP_ICONSET"
sips -s format png "$REPO_ROOT/literbike-vrod-icon.svg" --out "$tmpdir/app-icon-1024.png" >/dev/null

for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$tmpdir/app-icon-1024.png" --out "$APP_ICONSET/icon_${size}x${size}.png" >/dev/null
done

for size in 16 32 128 256 512; do
    retina_size=$((size * 2))
    sips -z "$retina_size" "$retina_size" "$tmpdir/app-icon-1024.png" --out "$APP_ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil -c icns "$APP_ICONSET" -o "$APP_BUNDLE/Contents/Resources/AppIcon.icns"
sips -s format png "$MACOS_DIR/Resources/status-template.svg" --out "$APP_BUNDLE/Contents/Resources/StatusIconTemplate.png" >/dev/null

chmod +x "$APP_BUNDLE/Contents/MacOS/$EXECUTABLE_NAME"

if [[ "$INSTALL_APP_FLAG" -eq 1 ]]; then
    ditto "$APP_BUNDLE" "$INSTALL_APP"
    echo "Installed app bundle at $INSTALL_APP"
fi

echo "Built app bundle at $APP_BUNDLE"
