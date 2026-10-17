# lumen-code (Rust)

Rust reference implementation of [Lumen Code v1](../../PROTOCOL.md): high-throughput screen-to-camera data transfer. Zero dependencies, `std` only.

- `transfer::Sender` — file → endless stream of frames (cell grids)
- `render::Renderer` — frame → RGBA pixels (or a 1-pixel-per-cell image plus a static mask)
- `scan::Scanner` — camera image (RGBA) → packets
- `transfer::Receiver` — packets → file (fountain decoding, DEFLATE, CRC-32 check)
- `ffi` — plain C ABI for WebAssembly and native hosts (`include/lumen.h`)

The crate reproduces [`test/vectors/test-vectors.json`](../../test/vectors/test-vectors.json) bit for bit, and its frames are identical to those of the JavaScript implementation.

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
let mut rx = Receiver::new();
let res = scanner.scan(&Image { width, height, rgba: &camera_rgba }, None);
for cw in res.codewords.iter().filter(|c| c.ok) {
    rx.push(cw.bytes.as_ref().unwrap());
}
if let Some(Finish::File { name, bytes }) = rx.try_finish() { /* done */ }
```

## Platforms

| Target | Command | Output |
|---|---|---|
| Web (WASM, no imports) | `../tools/build-wasm.sh` | `../../dist/wasm/lumen.wasm` (~220 KB) |
| Android | `cargo ndk -t arm64-v8a build --release --features ffi` | `liblumen.so` |
| iOS | `cargo build --release --features ffi --target aarch64-apple-ios` | `liblumen.a` |
| Desktop | `cargo build --release --features ffi` | `liblumen.{so,dylib,dll}` / `.a` |

Native hosts call the C ABI in [`include/lumen.h`](include/lumen.h) (JNI on Android, a bridging header on iOS). The JavaScript glue for the web is [`../core/wasm-engine.js`](../core/wasm-engine.js); it shows how to call every function.

## Tests

```sh
cargo test --release
```

- `tests/vectors.rs` — every value in the test vectors, including two complete frames
- `tests/roundtrip.rs` — Sender → Renderer → simulated camera → Scanner → Receiver, and decoding of camera frames simulated by the JavaScript test suite (run `node test/make-raw.js` in the repository root)

Minimum Rust version: 1.73. License: MIT.
