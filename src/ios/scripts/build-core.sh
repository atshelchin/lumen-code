#!/bin/sh
# Builds the Rust core for iOS: LumenCoreKit/Artifacts/LumenFFI.xcframework (device + simulator,
# arm64) and the generated Swift bindings in LumenCoreKit/Sources/LumenCore/.
# Run it after every change under src/app or src/rust; Xcode never rebuilds the Rust.
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$(cd "$HERE/../.." && pwd)"
APP="$SRC/app"
KIT="$SRC/ios/LumenCoreKit"
export IPHONEOS_DEPLOYMENT_TARGET=17.0

cd "$APP"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim >/dev/null 2>&1 || true

# host library: the binding generator reads the interface from it
cargo build --release -p lumen-ffi
cargo run --release -q -p lumen-bindgen --bin uniffi-bindgen -- generate \
  --library target/release/liblumen_ffi.dylib --language swift --out-dir target/bindings/swift

SIM_ONLY="${LUMEN_IOS_SIM_ONLY:-0}"
if [ "$SIM_ONLY" != 1 ]; then
  cargo build --release -p lumen-ffi --target aarch64-apple-ios
fi
cargo build --release -p lumen-ffi --target aarch64-apple-ios-sim

HDRS="target/xcframework-headers"
rm -rf "$HDRS" "$KIT/Artifacts/LumenFFI.xcframework"
ARGS=""
for SLICE in ios ios-sim; do
  mkdir -p "$HDRS/$SLICE"
  cp target/bindings/swift/lumen_ffiFFI.h "$HDRS/$SLICE/"
  cp target/bindings/swift/lumen_ffiFFI.modulemap "$HDRS/$SLICE/module.modulemap"
done
if [ "$SIM_ONLY" != 1 ]; then
  ARGS="-library target/aarch64-apple-ios/release/liblumen_ffi.a -headers $HDRS/ios"
fi
mkdir -p "$KIT/Artifacts"
# shellcheck disable=SC2086
xcodebuild -create-xcframework $ARGS \
  -library target/aarch64-apple-ios-sim/release/liblumen_ffi.a -headers "$HDRS/ios-sim" \
  -output "$KIT/Artifacts/LumenFFI.xcframework" >/dev/null
cp target/bindings/swift/lumen_ffi.swift "$KIT/Sources/LumenCore/lumen_ffi.swift"
echo "LumenFFI.xcframework and lumen_ffi.swift updated"
