#!/bin/sh
# Build the Rust implementation as a dependency-free WebAssembly module for the web demo.
#   rustup target add wasm32-unknown-unknown   (once)
set -e
cd "$(dirname "$0")/../rust"
cargo build --release --target wasm32-unknown-unknown
mkdir -p ../../dist/wasm
cp target/wasm32-unknown-unknown/release/lumen.wasm ../../dist/wasm/lumen.wasm
ls -l ../../dist/wasm/lumen.wasm
