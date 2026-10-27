# Lumen Code app core

Everything in the iOS, Android and desktop apps that is not platform code: all decisions, every string on screen, and the real-time sending and receiving. The apps ([iOS](../ios/README.md), [Android](../android/README.md), [desktop](../desktop/README.md)) are thin shells that show the view model this core produces and perform the operations it asks for.

The wire protocol is the zero-dependency [`lumen-code` crate](../rust/README.md); this workspace builds the app on top of it.

## Layout

```
core/      lumen-app: the app (Crux), the runtime that drives it, the real-time engine
  src/app/        Crux App: events, model, view model, shell operations (+ tests.rs)
  src/runtime.rs  runs the core and the work it asks for; the Shell trait the platforms implement
  src/engine/     send.rs (frame stream), recv.rs (scan pacing, region of interest, statistics),
                  image.rs (camera frame → scanner input), sink.rs (fountain decoder thread),
                  bench.rs (self-test, rolling-shutter simulation, frame replay)
  src/geometry.rs, content.rs, format.rs, i18n.rs   rules and strings the shells share
  examples/       profiling and analysis tools (see below)
ffi/       lumen-ffi: the uniffi interface (LumenCore object, camera frames, send frames)
bindgen/   the uniffi binding generator, run by the iOS and Android build scripts
```

## How a shell talks to the core

The core is a [Crux](https://github.com/redbadger/crux) app driven by `runtime::Runtime`:

1. The shell sends **events** as JSON (`{"type":"open_camera"}`, `{"type":"set_fps","value":30}` …), from any thread.
2. The core answers with **updates**: `{"view": {…}, "requests": [{"id": 7, "op": {"type": "camera_start", "device": 0}}]}`. The shell shows the view model and performs each operation: keep the screen on, haptics, camera start / mode / exposure, file pickers, save / share, clipboard. Operations with an `id` other than 0 expect an answer (`resolve(id, json)`).
3. Camera frames go to `camera_frame` as raw plane addresses (zero copy): the core reads them during the call and decides itself whether to scan.

Static labels are fetched once per language with `lumen_labels(lang)`; text that changes with state arrives already translated in the view model. Views are coalesced to 20 per second (10 when the device is hot), so a shell can apply every update it receives.

## Receiving

```
camera thread                       scanner pool (2–3 threads)          decoder thread
plan: scan this frame? ──► crop + scale the region around the code ──► scan ──► packets ──► fountain decoder
       (pacing, heat)       (prepare_yuv, on the camera thread)        finish: statistics, pacing, region
```

- **Pacing.** Normally scans are locked to the sender's frame rate: the capture right after each new sender frame is scanned. When captures are torn, because the sensor reads its rows over a large part of a sender frame (rolling shutter, reported by Android) or the camera is barely faster than the sender, every capture is scanned, since each one holds data no other capture has.
- **Searching.** With no code in view the core scans 15 times a second for the first two seconds, then 5 times, and 2 times after ten seconds. Searches run at half resolution, except every third one.
- **Tracking.** The last code position narrows the finder search to four windows (10 cells around each corner) and the frame to a region of interest at about 6 camera pixels per cell.
- **Camera mode.** The core asks for 60/120/240 fps when the sender plays fast enough and the camera offers it (`camera_mode`), 4K only for cells under about 4 pixels, and an exposure limit of a sixth of the sender's frame period (`camera_exposure`) so one capture holds one frame.
- **Heat.** `Thermal { level }` (0–3) from the shell sets a time budget for the scanners: 1.5, 0.8 or 0.4 seconds of scanning per second. This barely slows a phone that scans a frame in 12 ms but does slow one that needs 60 ms. Hot phones also give up the camera modes above 60 fps, and the dashboard shows when the budget turns frames away.

## Build and test

```sh
cargo test --release                          # 21 tests: app logic, camera pipeline, rolling shutter
cargo run --release --example bench           # self-test: synthetic camera frames through the engine
```

The shells build the `lumen-ffi` library themselves (`src/ios/scripts/build-core.sh`, `src/android/scripts/build-core.sh`, `cargo` for the desktop). Run `npm test` in the repository root after any change to the protocol crate: the JavaScript and WebAssembly scanners must stay bit-identical.

## Tools

The shells' debug builds can save raw camera frames with `grab`, in the `frame-N.nv12` format: a 16-byte header with width, height and strides, followed by NV12 data. These examples work on them:

| Example | What it does |
|---|---|
| `realframes -- <dir> [rounds]` | replays the frames through the receive engine: scans, time per scan, packets |
| `scanprof -- <frame>` | times the scanner on one frame at full resolution, without and with a tracking hint |
| `failmap -- <dir>` | where on the code the codewords fail (tilt, glare, a torn frame) |
| `rolling -- <sender fps> <camera fps> <skew ms>` | simulates a rolling-shutter camera watching a sender |
| `synthframe -- <file>` | writes the self-test's synthetic frame in the same format |
| `bench -- profile` | a tight scan loop for a profiler |

On a phone, `lumen_replay(dir, rounds)` (the Android `replay` script command) runs the same replay inside the app, on the threads and cores live scanning uses.
