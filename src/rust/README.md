# lumen-code (Rust)

Rust reference implementation of [Lumen Code v1](../../docs/PROTOCOL.md), a protocol for high-throughput screen-to-camera data transfer. Zero dependencies, `std` only.

- `transfer::Sender` — file → endless stream of frames (cell grids)
- `render::Renderer` — frame → RGBA pixels (or a 1-pixel-per-cell image plus a static mask)
- `scan::Scanner` — camera image (RGBA) → packets
- `transfer::Receiver` — packets → file: the receiver state machine of PROTOCOL §11 (fountain decoding, strict and bounded DEFLATE, file check that restarts a failed transfer, limits)
- `ffi` — plain C ABI for WebAssembly and native hosts (`include/lumen.h`)

The crate passes every group of [`test/vectors/test-vectors.json`](../../test/vectors/test-vectors.json) (encoding, decoding of valid and invalid input, packet streams through the receiver). Its frames are identical to those of the JavaScript implementation, and on the shared pseudo-random corpora it makes the same decisions as the JavaScript decoders.

## Example

```rust
use lumen::scan::{Image, Scanner};
use lumen::transfer::{prepare, Finish, Receiver, Sender};
use lumen::Renderer;

// sender
let prepared = prepare(&file_bytes, "photo.jpg", true);
let mut tx = Sender::new(&prepared, /*mode C4*/ 1, /*ecc*/ 1, /*X*/ 90, /*Y*/ 150, /*tid*/ 0x1234, /*seq*/ 0).unwrap();
let frame = tx.next_frame();
let r = Renderer::new(&tx.layout, tx.mode, /*px per cell*/ 6, /*quiet*/ 2);
let mut pixels = vec![0u32; r.width * r.height];
r.draw(&frame.cells, &mut pixels);

// receiver
let mut scanner = Scanner::new();
let mut rx = Receiver::new(); // reference limits; Receiver::with_limits(Limits { .. }) to change them
let res = scanner.scan(&Image { width, height, rgba: &camera_rgba }, None);
for cw in res.codewords.iter().filter(|c| c.ok) {
    rx.push(cw.bytes.as_ref().unwrap()); // returns the Action of §11.2
}
match rx.try_finish() {
    Some(Finish::File { name, bytes }) => { /* done; `name` comes from the sender: not a path */ }
    Some(Finish::ChecksumError) => { /* the file check failed; the transfer restarts by itself */ }
    None => {}
}
// rx.progress(): state (Collecting, Receiving, Refused, Complete), name, size, received, needed
```

## Platforms

| Target | Command | Output |
|---|---|---|
| Web (WASM, no imports) | `../tools/build-wasm.sh` | `../../dist/wasm/lumen.wasm` (~220 KB) |
| Android | `cargo ndk -t arm64-v8a build --release --features ffi` | `liblumen.so` |
| iOS | `cargo build --release --features ffi --target aarch64-apple-ios` | `liblumen.a` |
| Desktop | `cargo build --release --features ffi` | `liblumen.{so,dylib,dll}` / `.a` |

Native hosts call the C ABI in [`include/lumen.h`](include/lumen.h) (JNI on Android, a bridging header on iOS). `lm_sender_new_file` starts from the file itself (CRC-32 and DEFLATE are done inside), so a native host needs no zlib. The [integration guide](../../docs/guide/03-integration.md) walks through it step by step. The JavaScript glue for the web, [`../core/wasm-engine.js`](../core/wasm-engine.js), shows how to call every function.

## Tests

```sh
cargo test --release
```

- `tests/vectors.rs` — every group of the test vectors: encoding, decoding, Reed–Solomon, fountain decodability, packet streams through the receiver state machine
- `tests/fuzz.rs` — differential tests against the JavaScript decoders (same corpora, same decisions) and property tests: parsers, Reed–Solomon, inflate, fountain decoder, hostile packet streams. `FUZZ=20 cargo test --release --test fuzz` runs 20 times more cases
- `tests/roundtrip.rs` — Sender → Renderer → simulated camera → Scanner → Receiver, and decoding of camera frames simulated by the JavaScript test suite (run `node test/make-raw.js` in the repository root)
- `examples/measure.rs` — the measurements of PROTOCOL Appendix C and the stage benchmarks: `cargo run --release --example measure -- residual | fountain | tail K n | rank K n | bench`

Minimum Rust version: 1.73. License: MIT.
