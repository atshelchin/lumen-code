# Lumen Code for the desktop

A [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) shell over the [shared Rust core](../app/README.md). The core decides everything; this crate lays out its view model, forwards input, and performs the operations it asks for (files, clipboard, webcam, keeping the screen on). It drives the core's runtime directly: no FFI and no JSON.

Built and tested on macOS (Apple silicon); CI builds the Windows and Linux versions. The webcam is only implemented for macOS: elsewhere the receiver reports that no camera was found, and decoding an image still works.

## Build and run

```sh
cargo run --release          # first build fetches and compiles GPUI from the Zed repository (several minutes)
```

The GPUI revision is pinned in `Cargo.lock`. Started from a terminal, macOS asks for camera access for the terminal app the first time the receiver opens the camera.

## Releases

Pushing a `release/vX.Y.Z` branch is the release ([`.github/workflows/release.yml`](../../.github/workflows/release.yml)):

```sh
git switch -c release/v0.1.1 main      # bump version in Cargo.toml, cargo update -p lumen-desktop, commit
git push origin release/v0.1.1
```

CI checks that the branch and `Cargo.toml` agree, builds Windows (x86_64) and Linux (x86_64, aarch64) archives and a macOS build check, then tags the branch's head commit and publishes a GitHub Release with the archives, `SHA256SUMS` and build provenance. macOS is signed on the maintainer's Mac, so no signing key is stored on GitHub:

```sh
git fetch --tags && git checkout v0.1.1
scripts/release-macos-local.sh v0.1.1 --upload   # Developer ID signature, notarization, three .dmg images
```

`scripts/build-macos-app.sh` makes the app and image on its own: ad-hoc signed for local use, or with `LUMEN_SIGN_IDENTITY` and `LUMEN_NOTARY_PROFILE` signed and notarized. The bundle (`packaging/macos/`) declares the camera for the hardened runtime; without that entitlement a notarized build gets no camera at all.

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

- **Sending.** The code's cells are painted inside `window.paint_layer`, which keeps GPUI from inserting every cell into its hit-test tree (that insertion was most of the cost). Frames flip from a timer, not from GPUI's next-frame callback, because GPUI slows that callback to 30 fps for windows that are not focused. The frame-rate slider takes clicks, drags and the scroll wheel; the arrow keys change the rate by 1, with Shift by 5. 60 fps uses about 10 % of one core on an M2.
- **Receiving.** The webcam's packed 4:2:2 (YUYV) or NV12 frames go to the core without conversion; the preview is made at half size. Built-in FaceTime cameras give 1080p at 30 fps; an iPhone used as a Continuity Camera gives 1920×1440 at up to 60 fps.

## Tests without a mouse

```sh
LUMEN_SCRIPT="nav:send;demo:3000000;wait:2;fps:60" cargo run --release
LUMEN_SCRIPT="nav:receive;camera;wait:8;diag" cargo run --release    # prints the receiver's dashboard line
```

Commands: `nav:<home|send|receive>`, `lang`, `tab:<file|text>`, `text:<s>`, `sendtext`, `json`, `demo:<bytes>`, `wait:<s>`, `tap`, `fps:<n>`, `settings`, `play`, `camera`, `close`, `diag`, `capture:<n>` / `feed` (decode the app's own frames), `bench:<frames>`. In script mode the sender logs its real frame rate every few seconds.
