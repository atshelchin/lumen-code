# Lumen Code

High-throughput file transfer from a screen to a camera. The sender plays a file as an animated grid of coloured cells; the receiver films it and rebuilds the file. No network, no pairing, no return channel.

[中文说明](README.zh-CN.md) · [Developer guides](docs/guide/README.md) · [Protocol (English, STE)](docs/PROTOCOL.md) · [协议（中文）](docs/PROTOCOL.zh-CN.md) · [Test vectors](test/vectors/test-vectors.json) · [Rust crate](src/rust/README.md)

## Why another animated code?

Animated QR codes are easy to make but slow: one QR version 25 frame holds about 1.2 KB, and the square shape leaves most of a phone or monitor screen unused. Lumen Code is used the same way, by pointing a camera at a screen, and carries more per frame:

| | Animated QR (v25, typical) | Lumen Code v1 |
|---|---|---|
| Bits per cell | 1 | 2 (C4, default) or 3 (C8) |
| Shape | square | fills the screen (adaptive width and height, aspect up to 2.4) |
| Cell size | fixed by version | chosen from the physical screen size |
| Data per frame, phone screen | ~1.2 KB | ~2.2 KB (C4) / ~3.4 KB (C8) |
| Data per frame, 13" laptop | ~1.2 KB | ~6.9 KB (C4) / ~10.3 KB (C8) |
| Lost frames | usually repeated cyclically | fountain code: every frame is new data |
| One corner hidden by glare | frame lost | decodes from the other three corners |

At the default 12 frames per second, a laptop screen sends roughly 80–125 KB/s of raw capacity. The rate a receiver actually gets depends on its camera, and the receiver page shows it live. [Performance](docs/guide/04-performance.md) breaks the speed down item by item.

## Try it

Online: <https://lumencode.link>. Open **Send** on the sending screen and choose a file, then open **Receive** on the receiving phone. Both pages work on phones and desktops. Nothing is installed, and the file never leaves the two devices.

> **Photosensitivity warning:** while sending, the code flashes and changes colour up to 60 times a second. People who are sensitive to flashing light should not look at it. The web pages and the native apps show this warning before a code plays.

- **Sender:** sends a file, or text typed or pasted in (File | Text tabs). Settings: density (low / auto / high / max), 4 or 8 colours, ECC level, frame rate, engine. Tapping the code hides or shows the controls.
- **Receiver:** text and images open in a full-screen reader (JSON is formatted and syntax-highlighted, links are clickable, text size is adjustable); files can be saved or opened. While scanning it shows progress, speed, cell size in camera pixels, success rate, and a diagnostics line (engine time, camera fps, scans/s, new frames/s, sender fps, capture rate) with hints. When the transfer completes, the camera turns off and the statistics stay available. Pinch on the image to zoom, if the camera supports it. [Reading the dashboard](docs/guide/05-dashboard.md) explains every number.
- **Engine switch:** both pages can run the JavaScript implementation or the Rust implementation compiled to WebAssembly (`JS | WASM`), which produce bit-identical frames. The receiver uses WASM by default; switching during a scan keeps the progress.
- **Screen fit:** the code adapts its width and height to the display in either orientation. To receive a landscape code, turn the phone sideways.

To run the web app yourself (SvelteKit, in [`src/website`](src/website/README.md)):

```sh
cd src/website && bun install
bun run dev            # http://localhost:5173 (the camera works on localhost)
bun run dev:https      # https://<lan-ip>:5173 for phones (self-signed certificate)
bun run build          # prerendered pages and a Cloudflare Worker in .svelte-kit/cloudflare
```

## Native apps (iOS, Android, desktop)

The native apps do the same transfer on one shared Rust core ([`src/app`](src/app/README.md): app logic, scan pipeline and the protocol crate). Each platform shell only draws the UI and calls the platform. The apps look and behave like the web pages, and add what native code allows: camera planes read in place, 60–240 fps capture where the camera allows it, the app's own exposure control on Android, 120 Hz playback, haptics, screen brightness, and less work when the phone gets hot.

| App | Shell | Build and run | Guide |
|---|---|---|---|
| iOS 17+ | SwiftUI | `src/ios/scripts/run.sh` | [src/ios](src/ios/README.md) |
| Android 10+ | Jetpack Compose | `cd src/android && ./gradlew :app:installDebug` | [src/android](src/android/README.md) |
| macOS, Windows, Linux | GPUI | `cd src/desktop && cargo run --release` | [src/desktop](src/desktop/README.md) |

Desktop builds are on the [Releases](https://github.com/atshelchin/lumen-code/releases) page: a signed and notarized macOS image, and Windows and Linux archives. The desktop app is tested on macOS. The Windows and Linux builds, webcam included, are new and have not yet been tried on real machines.

The iPhone and Android apps are coming to the App Store and Google Play at US$2.99, paid once (no subscription, no in-app purchases, no ads). The web version and the desktop builds stay free, and anyone can build every app from this repository for free.

## Use it in your own project

There is no package to install: take the code from this repository, pinned to a tag or a commit. A coding agent can follow the steps below as written.

| You build | Take | Then read |
|---|---|---|
| A web page or a Node program | the folder `src/core/` — ES modules, no build step, no dependencies. For the faster engine add `dist/wasm/lumen.wasm`; `src/core/wasm-engine.js` loads it | [Integration guide](docs/guide/03-integration.md) §3.2–§3.3 |
| A Rust program | `lumen-code = { git = "https://github.com/atshelchin/lumen-code" }` in `Cargo.toml`; the library is `lumen` (no dependencies) | [src/rust/README.md](src/rust/README.md) |
| An app in another language (Swift, Kotlin, C, …) | the C ABI of the same crate: `src/rust/include/lumen.h`, built with `--features ffi` | [Integration guide](docs/guide/03-integration.md) §3.5 |
| A new implementation | the [specification](docs/PROTOCOL.md) and [`test/vectors/test-vectors.json`](test/vectors/test-vectors.json) | [Integration guide](docs/guide/03-integration.md) §3.7 |

Every integration must follow these rules ([specification](docs/PROTOCOL.md) §10.4, §11, §13):

- Use the receiver unchanged. It enforces the limits and restarts a transfer whose file check fails.
- The file name and the content come from the sender. Never use the name as a path, and treat the file as one from an unknown source.
- Warn the user before a code plays, because it flashes many times per second.

## How it works (short)

1. The file is optionally DEFLATE-compressed and split into source blocks.
2. A Raptor-style fountain code turns each block into an endless stream of symbols. About K + 2 different symbols rebuild a block of K symbols (measured average: about 1.6 symbols more than K).
3. Each symbol becomes a packet with a CRC, then a Reed–Solomon codeword.
4. Codewords are laid along a Hilbert curve into the grid, so each codeword occupies a compact patch and a torn camera frame loses only a few codewords.
5. Three finders, one corner pattern, a lattice of alignment markers and four header copies let the receiver locate the grid under perspective, lens distortion, blur and moiré.

The [developer guides](docs/guide/README.md) explain this with figures and numbers. The full, normative description is [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Repository

```
docs/PROTOCOL.md, *.zh-CN.md     protocol specification (English STE / Chinese)
docs/guide/                      developer guides (how it works, scanner, integration, performance, dashboard)
src/core/                        JavaScript reference implementation (protocol library, ES modules)
src/website/                     the web app (SvelteKit): home, sender, receiver, in English and Chinese
src/rust/                        Rust reference implementation (zero dependencies, C ABI, WASM)
src/app/                         shared core of the native apps (app logic, scan pipeline, uniffi bindings)
src/ios/ src/android/ src/desktop/   native shells (SwiftUI, Jetpack Compose, GPUI)
src/web/, dist/                  the previous static site (still deployed until the switch to src/website); dist/wasm/lumen.wasm is the WASM engine
src/tools/                       WASM build, the previous site's build and dev server, figure and link-preview generators
test/                            Node tests, camera simulator, robustness suites
test/vectors/test-vectors.json   conformance vectors (generated by test/vectors.js)
```

## Tests

```sh
npm test                 # conformance vectors, fuzz/property tests, core, packets/blocks, end-to-end through a camera model, JS<->WASM
npm run test:fuzz        # the fuzz/property tests with 20 times more cases
npm run test:robustness  # perspective, rotation, mirroring, blur, bloom, moiré, motion, exposure
cd src/rust && cargo test --release   # the same vectors, differential tests against JS, fuzz/property tests, decoding of JS-simulated frames
cd src/app && cargo test --release    # native app core: app logic, camera pipeline, rolling-shutter simulation
node test/bench.js; cd src/rust && cargo run --release --example measure -- bench   # throughput of each stage
cd src/website && bun run test:e2e    # the web app in Chromium: a recorded transfer as its webcam, both engines, both languages
```

The camera simulator (`test/camera.js`) models perspective, barrel distortion, optical blur, colour crosstalk, white balance, vignetting, bloom, moiré, motion blur, 4:2:0 chroma subsampling and noise. In a randomised "hand-held, badly aimed" batch in 4-colour mode, about 27–28 of 30 frames and 86–89 % of all codewords decode. The frames that fail have about 3 camera pixels per cell plus heavy blur.

## Performance (Apple M2, 1080p camera frame)

| Scanner | Phone-size code | Laptop-size code |
|---|---|---|
| JavaScript (V8) | 14–16 ms | 41–45 ms |
| Rust → WebAssembly | 9–10 ms | 21 ms |
| Rust native | 7–9 ms | 16 ms |

The receiver also skips frames it has already decoded (it reads only the header, about 8 ms in JS), locks its scan rate to the sender's frame rate (about 1.6 scans per sender frame instead of one per camera frame), and has the GPU crop and scale only the region around the code to about 6 pixels per cell before a worker sees it. Panels over the video use no background blur, and the camera turns off when a transfer completes. Together these cut the work several times over and keep phones cool.

## Status

- Protocol version 1.0 is final and frozen (2026-10-20): its wire format will not change. All apps and the web version, 1.0.0, use it. The specification covers the wire format and the receiver: validity rules, a transfer state machine, limits, errors, security considerations and extension rules.
- Two independent implementations (JavaScript, Rust) pass the same conformance vectors: encoding, decoding of valid and invalid input, and packet streams through the receiver state machine. On pseudo-random corpora (20 000 headers, 20 000 descriptors, 5 000 damaged DEFLATE streams, 2 000 fountain symbol sets) they make identical decisions.
- Validation so far: simulation, Chrome with a fake camera stream, and early tests on real phones. Real-device tuning continues; please report camera models and conditions that do not work well.
- When you report a problem, quote the version. The web pages show it at the bottom of the home page and of the send and receive cards ("Version 9cc2078 · 2026-10-20", the commit of the sources); the apps show it at the bottom of their home screen and in the details of a received file.
- License: [MIT](LICENSE).
- The website loads a visit-statistics script (`tj.appsdata.org`, see the [privacy policy](https://lumencode.link/privacy)) from `src/website/src/app.html`. Remove it there, and from the CSP in `src/website/vite.config.ts`, if you host your own copy.
