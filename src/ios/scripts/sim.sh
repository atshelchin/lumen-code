#!/bin/sh
# Build, install and launch on a simulator; optional LUMEN_SCRIPT in $1 (debug builds).
#   scripts/sim.sh "nav:send;tab:text;text:hello;sendtext"
set -eu
cd "$(dirname "$0")/.."
SIM="${LUMEN_SIM:-$(xcrun simctl list devices booted | grep -Eo '[0-9A-F-]{36}' | head -1)}"
[ -n "$SIM" ] || { echo "boot a simulator first"; exit 1; }
xcodegen generate -q
xcodebuild -project Lumen.xcodeproj -scheme Lumen -configuration Debug -destination "id=$SIM" -derivedDataPath build build -quiet
xcrun simctl install "$SIM" build/Build/Products/Debug-iphonesimulator/Lumen.app
SIMCTL_CHILD_LUMEN_SCRIPT="${1:-}" xcrun simctl launch --terminate-running-process "$SIM" ltd.mondaylabs.lumencode
