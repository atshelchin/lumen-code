#!/bin/sh
# Build the Rust core, generate the project, build, install and launch on a connected iPhone.
#   scripts/run.sh                 # first connected device
#   LUMEN_DEVICE=<udid> scripts/run.sh "nav:send;demo:40000"   # optional debug script
# Set LUMEN_SKIP_CORE=1 to reuse the last Rust build.
set -eu
cd "$(dirname "$0")/.."
[ "${LUMEN_SKIP_CORE:-0}" = 1 ] || ./scripts/build-core.sh
xcodegen generate -q
UDID="${LUMEN_DEVICE:-$(xcrun xctrace list devices 2>/dev/null | sed -n '/== Devices ==/,/== Devices Offline ==/p' | grep -Eo '\(([0-9A-F]{8}-[0-9A-F]{16})\)' | head -1 | tr -d '()')}"
[ -n "$UDID" ] || { echo "no iPhone connected"; exit 1; }
xcodebuild -project Lumen.xcodeproj -scheme Lumen -configuration "${LUMEN_CONFIG:-Debug}" -destination "id=$UDID" \
  -derivedDataPath build -allowProvisioningUpdates build -quiet
APP="build/Build/Products/${LUMEN_CONFIG:-Debug}-iphoneos/Lumen.app"
DEV="$UDID" # devicectl accepts the hardware UDID too
xcrun devicectl device install app --device "$DEV" "$APP" >/dev/null
xcrun devicectl device process launch --terminate-existing --device "$DEV" \
  ${1:+-e "{\"LUMEN_SCRIPT\":\"$1\"}"} ltd.mondaylabs.lumencode
