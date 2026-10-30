# 3 · Integration guide

[Guide index](README.md) · [中文](03-integration.zh-CN.md) · Specification sections: §10, §11, §15

## 3.1 Which implementation to choose

| Scenario | Recommended | Location |
|---|---|---|
| Web page, get running as fast as possible | JavaScript (ES modules, zero dependencies) | `src/core/` |
| Web page, receiver must save power and be fast | WebAssembly compiled from Rust (about 220 KB, no imports) | `dist/wasm/lumen.wasm` + `src/core/wasm-engine.js` |
| Rust program | `lumen-code` crate (zero dependencies, MSRV 1.73) | `src/rust/` |
| Native Android / iOS / desktop app | The C ABI of the same crate | `src/rust/include/lumen.h` |

All implementations generate **bit-identical** frames, so they can be mixed: send from JS and receive on Android, or send from Rust and receive in a browser.

## 3.2 Web: JavaScript

### Sending

```js
import { Sender, preparePayload } from './core/transfer.js';
import { Renderer } from './core/render.js';

const prepared = await preparePayload(fileBytes, 'photo.jpg'); // DEFLATE-compressed when that helps
const tx = new Sender(prepared, { mode: 1 /* C4 */, ecc: 1 /* 16% */, X: 90, Y: 160 });

const canvas = document.querySelector('canvas');
const ctx = canvas.getContext('2d');
let r, img;
setInterval(() => {
  const { cells, layout } = tx.nextFrame();
  if (!r) {
    r = new Renderer(layout, 1, 6); // 6 device pixels per cell, 2-cell quiet zone on all sides
    canvas.width = r.width;
    canvas.height = r.height;
    img = ctx.createImageData(r.width, r.height);
  }
  r.draw(cells, new Uint32Array(img.data.buffer));
  ctx.putImageData(img, 0, 0);
}, 1000 / 12); // 12 frames per second
```

Display the canvas at **device pixel** size (CSS size = pixel size / devicePixelRatio); otherwise browser scaling blurs the cell edges. §3.6 covers choosing X, Y and the cell size.

### Receiving

```js
import { Scanner } from './core/scan.js';
import { Receiver } from './core/transfer.js';

const video = document.querySelector('video');
video.srcObject = await navigator.mediaDevices.getUserMedia({
  video: { facingMode: 'environment', width: { ideal: 1920 }, height: { ideal: 1080 } },
});
await video.play();

const canvas = new OffscreenCanvas(video.videoWidth, video.videoHeight);
const g = canvas.getContext('2d', { willReadFrequently: true });
const scanner = new Scanner();
const rx = new Receiver(); // reference limits; new Receiver({ maxSize, maxKept, maxTransfers }) to change them

async function tick() {
  g.drawImage(video, 0, 0);
  const res = scanner.scan(g.getImageData(0, 0, canvas.width, canvas.height));
  if (res.ok) for (const cw of res.codewords) if (cw.ok) await rx.push(cw.bytes);
  const out = await rx.tryFinish(); // { name, bytes } | { error: 'checksum' } (the transfer restarts) | null
  if (out && out.bytes) return save(out.name, out.bytes);
  setTimeout(tick, 50);
}
tick();
```

This is the minimal version. In real use, run scanning in a Web Worker and schedule it as described in [guide 2 §2.4](02-scanner.md); `src/website/src/lib/recv/scan.worker.ts`, `sink.worker.ts` and `receiver.svelte.ts` are the complete reference.

The receiver implements the state machine of PROTOCOL §11:

- `rx.push(packet)` returns what it did with the packet: `'discard'`, `'keep'` (held until the file descriptor arrives), `'add'`, `'accept'` (a descriptor) or `'refuse'` (a descriptor of a transfer larger than the limit).
- `rx.progress()` returns `{ state, name, size, received, needed, done, T }` for the transfer of the newest packet. `state` is `'collecting'` (no descriptor yet), `'receiving'`, `'refused'` (tell the user the file is too large) or `'complete'`.
- `rx.tryFinish()` returns the file once. If the decoded file fails its check (size or CRC-32, very rare), it returns `{ error: 'checksum' }` and restarts the transfer itself; keep scanning.
- Limits (§11.4): by default 256 MiB per file, 4096 packets kept before the descriptor, 4 transfers at a time. The file name comes from the sender, so never use it as a path (§13).

## 3.3 Web: WebAssembly

`wasm-engine.js` wraps the WASM module in classes with the **same shape** as the JS ones, so switching takes one changed import line:

```js
import { loadWasm, WasmScanner, WasmSender, WasmReceiver } from './core/wasm-engine.js';

await loadWasm('./wasm/lumen.wasm'); // an ArrayBuffer also works (for example in Node)
const scanner = new WasmScanner();   // scan() returns the same result as Scanner
const rx = new WasmReceiver();       // push() / progress() / tryFinish() same as Receiver; free() when done
const tx = new WasmSender(prepared, { mode: 1, ecc: 1, X: 90, Y: 160 });
// when no longer needed: scanner.free(), tx.free() release the WASM memory
```

WASM scanning is 1.5–2 times faster than JS and noticeably reduces heat on phones. Build the module with `npm run build:wasm` (this needs the Rust toolchain and the `wasm32-unknown-unknown` target).

## 3.4 Rust

Add the crate from this repository (it has no dependencies; pin a tag or a commit with `tag = "…"` or `rev = "…"`):

```toml
[dependencies]
lumen-code = { git = "https://github.com/mondaylabsltd/lumen-code" }
```

The library name is `lumen`:

```rust
use lumen::scan::{Image, Scanner};
use lumen::transfer::{prepare, Finish, Receiver, Sender};
use lumen::Renderer;

// Send
let prepared = prepare(&file_bytes, "photo.jpg", true);
let mut tx = Sender::new(&prepared, 1 /* C4 */, 1 /* ecc */, 90, 160, 0x1234 /* tid */, 0 /* seq */).unwrap();
let frame = tx.next_frame();
let r = Renderer::new(&tx.layout, tx.mode, 6 /* px/cell */, 2 /* quiet zone */);
let mut pixels = vec![0u32; r.width * r.height];
r.draw(&frame.cells, &mut pixels);

// Receive
let mut scanner = Scanner::new();
let mut rx = Receiver::new();
let res = scanner.scan(&Image { width, height, rgba: &camera_rgba }, None);
for cw in res.codewords.iter().filter(|c| c.ok) {
    rx.push(cw.bytes.as_ref().unwrap());
}
if let Some(Finish::File { name, bytes }) = rx.try_finish() { /* done */ }
```

The TID should be random (different for each transfer); a random initial seq is also recommended.

## 3.5 Native apps: C ABI

```sh
cd src/rust
cargo build --release --features ffi                                   # desktop: liblumen.{so,dylib,dll}
cargo build --release --features ffi --target aarch64-apple-ios        # iOS: liblumen.a
cargo ndk -t arm64-v8a build --release --features ffi                  # Android: liblumen.so
```

Android calls the library through JNI, iOS through a bridging header. All handles are opaque pointers. Memory returned by the library stays valid until the next call on the same handle.

### Sending

```c
#include "lumen.h"

WSender *tx = lm_sender_new_file(file, file_len, (const uint8_t *)"photo.jpg", 9,
                                 1 /* compress */, 1 /* C4 */, 1 /* ecc */, 90, 160,
                                 random_u16(), random_u32());
const uint32_t *info = lm_sender_info(tx); // n, nsym, T, perFrame, K, X, Y, mode, ecc, blocks
for (;;) {                                  // once every 1/12 s
    const uint8_t *cells = lm_sender_next_frame(tx); // X*Y cell values
    draw_cells(cells, info[5], info[6]);
}
lm_sender_free(tx);
```

Cell values: 0–7 are indices into the palette of the current colour mode, 8 is ink (10, 13, 20), and 9 is paper (242, 244, 248). The palettes are in specification §5. Draw the cells following §3.6; `src/core/render.js` is a reference renderer in a rounded "LED" style.

### Receiving

```c
WScanner *sc = lm_scanner_new();
WReceiver *rx = lm_receiver_new();

// For each camera frame (RGBA, width w, height h):
const uint8_t *rec = lm_scan(sc, rgba, w, h, NULL, 0);
// Parse the result record (little-endian):
//   u32 ok, skipped, reason, candidates
//   u32 mode, X, Y, n, nsym, seq
//   f64 cellPx, corners[8], markers
//   u32 cornerFound, cwOk, cwTotal, cwCount, corrected
//   u32 nPackets, packetLen
//   nPackets × { u32 codeword index, packet of packetLen bytes }
for (each packet p) lm_receiver_push(rx, p, packetLen);  // returns 0 discard, 1 keep, 2 add, 3 accept, 4 refuse

// lm_receiver_progress(rx): u32[10] valid, tid, received, needed, blocks, done, T, fileSize,
//                           state (0 collecting, 1 receiving, 2 refused, 3 complete), payloadLength
if (lm_receiver_finish(rx) == 1) {          // 0 not finished, 1 finished, 2 check failed: the transfer restarts
    const uint8_t *name = lm_receiver_name(rx);  uint32_t nl = lm_receiver_name_len(rx);
    const uint8_t *data = lm_receiver_file(rx);  uint32_t dl = lm_receiver_file_len(rx);
}
```

A seq of 0xFFFFFFFF means the frame number is unknown: no header copy was readable and the scanner reused the last correct header (the data is valid but cannot be used for frame skipping). `WasmScanner.scan()` in `src/core/wasm-engine.js` is the complete reference for parsing this record, since WASM and native code use the same C ABI. The `skip` parameter skips frames that are already decoded; its format is in `lumen.h`.

## 3.6 Choosing parameters

| Parameter | Recommendation | Notes |
|---|---|---|
| Grid shape | Fill the available screen area, aspect ratio ≤ 2.4 | More cells mean more data per frame; for a landscape code the receiver turns the phone |
| Physical cell size | About 0.75 mm | 52–150 cells on the short side. The "Density" setting of the demo page multiplies this by 0.72 / 1 / 1.3 / 1.65 |
| Pixels per cell | A whole number of device pixels, ≥ 3 | Fractional sizes make cells uneven in width |
| Quiet zone | ≥ 2 cells of ink on all sides | Separates the finder patterns from the background |
| Colour mode | C4 | C8 carries 50% more data, but needs good focus and little moiré |
| ECC | 16% (Mid) | Use 22% in bad conditions; 10% suits only a very stable fixed setup |
| FPS | 12 FPS | With the camera at 30 FPS, each sender frame is captured about 2.5 times. Raise it only when the receiver reports spare capacity |

The grid size, colour mode and ECC level can change at any time during a transfer (for example, when the window is resized). As long as T (bytes per symbol) stays the same, the receiver carries on without interruption. Changing T requires a new transfer with a new TID.

## 3.7 Building a new compatible implementation

1. Pass all vectors in `test/vectors/test-vectors.json` (PROTOCOL §15). `test/test-vectors.js` and `src/rust/tests/vectors.rs` show how each group is used:
   - **encoding**: CRCs, mix32, Rng, Reed–Solomon codewords, grid layouts, header bits, frames made from given packets, packets, descriptors, fountain parameters and symbols, block plans;
   - **decoding**: headers, packets and descriptors that must be accepted or discarded (each discarded one has exactly one named defect), damaged Reed–Solomon codewords inside the correction bound, sets of fountain symbols that are or are not decodable;
   - **streams**: packet sequences fed to a fresh receiver, with the action of §11.2 for every packet and the result of every decode attempt — the state machine, the file check and its restart, the limits, DEFLATE edge cases.
2. Generate frames with your sender and decode them with the reference receiver (or the other way around).
3. Run `node test/make-raw.js` to generate frames that pass through the camera simulation, and let your scanner decode them.
4. Optionally, reproduce the pseudo-random corpora of `test/corpus.js` and compare the digest of your decisions with the `differential` section of the vectors. This checks thousands of edge cases against the reference decoders.

The packet schedule, parameter choice, scanning algorithm and display method are up to you (§10.2–§10.4, §11.6) and do not affect compatibility. If you want frames identical to the reference sender, the `referenceSender` vectors show the reference choices.
