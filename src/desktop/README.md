# Lumen Code for the desktop

A [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) shell over the [shared Rust core](../app/README.md). The core makes all decisions; this crate lays out its view model, forwards input and performs the operations the core asks for (files, clipboard, webcam, keeping the screen on). It calls the core's runtime directly, with no FFI and no JSON.

Built and tested on macOS (Apple silicon). CI builds the Windows and Linux versions, but they have not yet been tried on real machines. The webcam goes through [nokhwa](https://github.com/l1npengtul/nokhwa): AVFoundation on macOS, Media Foundation on Windows, Video4Linux on Linux. On Windows and Linux the app uses the system title bar and Ctrl shortcuts (Ctrl+C, Ctrl+A, Ctrl+Q, F11 for full screen), and keeps the display on while a code plays.

## Build and run

```sh
cargo run --release          # first build fetches and compiles GPUI from the Zed repository (several minutes)
```

The GPUI revision is pinned in `Cargo.lock`. When the app is started from a terminal, macOS asks for camera access for the terminal app the first time the receiver opens the camera.

## Releases

Pushing a `release/vX.Y.Z` branch makes a release ([`.github/workflows/release.yml`](../../.github/workflows/release.yml)):

```sh
git switch -c release/v0.1.1 main      # bump version in Cargo.toml, cargo update -p lumen-desktop, commit
git push origin release/v0.1.1
```

CI checks that the branch name and `Cargo.toml` agree, builds Windows (x86_64) and Linux (x86_64, aarch64) archives and a macOS build check, then tags the branch's head commit and publishes a GitHub Release with the archives, `SHA256SUMS` and build provenance. The macOS build is signed on the maintainer's Mac, so no signing key is stored on GitHub:

```sh
git fetch --tags && git checkout v0.1.1
scripts/release-macos-local.sh v0.1.1 --upload   # Developer ID signature, notarization, three .dmg images
```

`scripts/build-macos-app.sh` can also be run by itself to make the app and disk image: ad-hoc signed for local use, or signed and notarized when `LUMEN_SIGN_IDENTITY` and `LUMEN_NOTARY_PROFILE` are set. The bundle (`packaging/macos/`) declares the camera entitlement for the hardened runtime; without it a notarized build gets no camera at all.

## Layout

```
src/main.rs        window, menus, assets (icons/)
src/app.rs         Root view: applies the core's updates, performs its operations, frame timer
src/home.rs, send.rs, recv.rs   the three screens (send.rs paints the code)
src/theme.rs       colours, buttons, segmented controls, the slider
src/text_area.rs   multi-line input with input methods (Chinese, Japanese)
src/camera.rs      webcam capture (nokhwa / AVFoundation) and preview
src/platform.rs    preferences, file locations, keeping the display awake
src/script.rs      test hooks (LUMEN_SCRIPT)
examples/camformats.rs   lists the formats and frame rates each camera offers
packaging/         macOS bundle template and entitlements, Linux desktop entry, release notes
scripts/           build-macos-app.sh, release-macos-local.sh
```

## Notes on performance

- **Sending.** The code's cells are painted inside `window.paint_layer`, which keeps GPUI from inserting every cell into its hit-test tree; that insertion was most of the cost. Frames are flipped by a timer rather than GPUI's next-frame callback, because GPUI slows that callback to 30 fps for windows that are not focused. The frame-rate slider responds to clicks, drags and the scroll wheel; the arrow keys change the rate by 1, or by 5 with Shift. At 60 fps sending uses about 10 % of one core on an M2.
- **Receiving.** Packed 4:2:2 (YUYV) and NV12 frames from the webcam go to the core without conversion. MJPEG frames (most USB webcams at 720p and up) are decoded with the `image` crate's pure-Rust decoder. The preview is made at half size. On macOS the app takes the camera's fastest mode (built-in FaceTime cameras: 1080p at 30 fps; an iPhone as Continuity Camera: 1920×1440 at up to 60). On Windows and Linux it asks for 1080p, then 720p, at the fastest rate each offers, because USB webcams reach their highest rates only at small sizes.

## Tests without a mouse

```sh
LUMEN_SCRIPT="nav:send;demo:3000000;wait:2;fps:60" cargo run --release
LUMEN_SCRIPT="nav:receive;camera;wait:8;diag" cargo run --release    # prints the receiver's dashboard line
```

Commands: `nav:<home|send|receive>`, `lang`, `tab:<file|text>`, `text:<s>`, `sendtext`, `json`, `demo:<bytes>`, `wait:<s>`, `tap`, `fps:<n>`, `settings`, `play`, `camera`, `close`, `diag`, `capture:<n>` / `feed` (decode the app's own frames), `bench:<frames>`. In script mode the sender logs its real frame rate every few seconds.
