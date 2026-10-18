#!/bin/sh
# Builds the Rust core for Android: app/src/main/jniLibs/<abi>/liblumen_ffi.so (cargo-ndk) and
# the generated Kotlin bindings in app/build/generated/uniffi. Gradle runs this before every
# build (task cargoBuild, up to date when nothing under src/app or src/rust changed).
#   one-time: rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
#             cargo install cargo-ndk
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$(cd "$HERE/../.." && pwd)"
ANDROID="$SRC/android"
LOCAL_SDK="$(sed -n 's/^sdk.dir=//p' "$ANDROID/local.properties" 2>/dev/null || true)"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-${LOCAL_SDK:-$HOME/Library/Android/sdk}}}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(ls -d "$SDK/ndk/"*/ 2>/dev/null | sort -V | tail -1)"
  export ANDROID_NDK_HOME
fi
ABIS="${LUMEN_ABIS:-arm64-v8a armeabi-v7a x86_64}"
TARGETS=""
for a in $ABIS; do TARGETS="$TARGETS -t $a"; done
# 16 KB pages (required by Google Play for new apps)
PAGE16K="-C link-arg=-Wl,-z,max-page-size=16384"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="${CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS:-} $PAGE16K"
export CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS="${CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS:-} $PAGE16K"

cd "$SRC/app"
# shellcheck disable=SC2086
cargo ndk $TARGETS --platform 29 -o "$ANDROID/app/src/main/jniLibs" build --release -p lumen-ffi
# the protocol crate's own C-ABI library comes along as a by-product; the app does not use it
rm -f "$ANDROID"/app/src/main/jniLibs/*/liblumen.so
# bindings from the host library
cargo build --release -p lumen-ffi
HOSTLIB="target/release/liblumen_ffi.dylib"
[ -f "$HOSTLIB" ] || HOSTLIB="target/release/liblumen_ffi.so"
OUT="$ANDROID/app/build/generated/uniffi"
rm -rf "$OUT"
cargo run --release -q -p lumen-bindgen --bin uniffi-bindgen -- generate --library "$HOSTLIB" --language kotlin --out-dir "$OUT" --no-format
echo "Android core libraries and Kotlin bindings updated"
