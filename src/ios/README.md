# Lumen Code for iOS

A SwiftUI shell over the [shared Rust core](../app/README.md). It shows the core's view model, performs the operations the core asks for and passes it camera frames. Everything else (what to show, when to scan, every string) is decided in Rust and is the same on Android and the desktop.

## Requirements

- A current Xcode (tested with Xcode 26.3), iOS 17 or later, an arm64 device or simulator
- [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`): the Xcode project is generated from `project.yml` and not committed
- Rust with the iOS targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`

Signing uses the team set in `project.yml` (`DEVELOPMENT_TEAM`); change it to yours.

## Build and run

```sh
scripts/build-core.sh       # Rust core → LumenCoreKit (xcframework + Swift bindings); after any change in src/app or src/rust
scripts/run.sh              # core, project, build, install and launch on the first connected iPhone
LUMEN_SKIP_CORE=1 LUMEN_DEVICE=<udid> scripts/run.sh    # reuse the last core build, pick a device
scripts/sim.sh              # build, install and launch on the booted simulator (no camera there)
```

Or run `scripts/build-core.sh` and `xcodegen generate`, then open `Lumen.xcodeproj`. Xcode does not rebuild the Rust core, so run `build-core.sh` again after changing it.

## Layout

```
project.yml                  XcodeGen project (bundle id, team, Info.plist keys)
LumenCoreKit/                Swift package around the Rust core (Artifacts/ and the bindings are build output)
Lumen/App/                   LumenApp, AppModel (the shell side of the core), Wire (view model types),
                             Platform (haptics, brightness, files), DebugScript
Lumen/Send/                  SendScreen, CodeView (the animated code)
Lumen/Receive/               ReceiveScreen, Camera, CameraPreview, DoneScreen (reader, save, share)
Lumen/Home/, Lumen/Design/   home screen, colours and type of the web demo
scripts/                     build-core.sh, run.sh, sim.sh
```

## What the shell does

- **Sending.** The code is a `CALayer` holding a one-pixel-per-cell image, scaled with nearest-neighbour filtering under a static mask, so a frame change touches only a few thousand pixels. A display link running at up to 120 Hz (`CADisableMinimumFrameDurationOnPhone`) shows each frame at its scheduled time, and the next frame is built on a background queue. While a code plays, the screen stays on at full brightness.
- **Receiving.** `AVCaptureVideoDataOutput` delivers 4:2:0 full-range Y'CbCr at 1080p, and the core reads both planes in place. Exposure bias is half a stop negative because a screen is a light source; stabilisation is off and continuous autofocus is restricted to near range. When the core asks, the camera switches to 60/120/240 fps (unbinned formats only) or 4K. `AVCaptureDevice.RotationCoordinator` keeps the preview upright.
- **Heat.** `ProcessInfo.thermalState` is passed to the core, which scans less and drops the fast camera modes as the phone warms up.
- **Feedback.** Haptics mark lock-on, each quarter of progress, completion and errors. The received file opens in a reader, Quick Look, the share sheet or Files.

## Device tests without touching the phone

Debug builds read a script from the `LUMEN_SCRIPT` launch variable, which `scripts/run.sh "<script>"` sets:

```sh
scripts/run.sh "nav:send;demo:40000;wait:2;snap:send;tap;wait:0.5;snap:dock"
scripts/run.sh "bench:300"            # scanner self-test; result in Documents/bench.txt
```

Commands include `nav`, `lang`, `tab`, `text`, `sendtext`, `demo:<bytes>`, `wait:<s>`, `tap`, `settings`, `play`, `accept` (the photosensitivity warning), `camera`, `close`, `json`, `image`, `capture`/`feed` (decode the app's own frames), `snap:<name>` (screenshot to Documents), `grab:<n>` (raw camera frames to Documents, for the [analysis tools](../app/README.md#tools)), `dump`, `formats`, `bench`, `event:<json>`. Copy results off the phone with `xcrun devicectl device copy from --device <id> --domain-type appDataContainer --domain-identifier ltd.mondaylabs.lumencode --source Documents/<file> --destination <path>`.
