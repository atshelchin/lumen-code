# Lumen Code for Android

A Jetpack Compose shell over the [shared Rust core](../app/README.md): it shows the core's view model, performs the operations the core asks for, and feeds it camera frames. Everything else (what to show, when to scan, every string) is decided in Rust and is the same on iOS and the desktop.

## Requirements

- JDK 17 or later (Android Studio's bundled JBR works), Android SDK 36 with an NDK
- Rust with the Android targets and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk):
  ```sh
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  cargo install cargo-ndk
  ```
- A device with Android 10 (API 29) or later

Gradle builds the Rust core itself (task `cargoBuild` runs `scripts/build-core.sh`, up to date when nothing under `src/app` or `src/rust` changed). Pass `-PlumenSkipRust` to skip it when only Kotlin changed.

## Build and run

```sh
./gradlew :app:installDebug      # debug build
./gradlew :app:assembleBench     # release code + test hooks: what to measure performance with
./gradlew :app:assembleRelease   # release
```

| Build type | Use |
|---|---|
| `debug` | development. Debuggable: ART interprets much of the UI and checks every JNI call, so the interface and every call into Rust are several times slower than in release. Do not judge performance with it. |
| `bench` | release code (compiled ahead of time, no JNI checks), profileable with simpleperf, with the test hooks below. |
| `release` | the app. Still signed with the debug key: set up a release signing config before publishing. |

## Layout

```
app/src/main/java/com/shelchin/lumen/
  MainActivity.kt          window, permissions, pickers, "Send with Lumen" share target
  core/AppModel.kt         the shell side of the core: events in, view model and operations out
  core/Wire.kt             view model types (kotlinx.serialization)
  platform/Camera.kt       CameraX capture, exposure, focus, frame hand-off
  platform/Platform.kt     haptics, screen on / brightness / refresh rate, files
  ui/                      Home, Send (the animated code), Receive, Done (reader), theme, icons
  DebugScript.kt           test hooks (debug and bench builds)
scripts/build-core.sh      Rust core → jniLibs + generated Kotlin bindings (uniffi, via JNA)
```

## What the shell does

- **Sending.** The code is a one-pixel-per-cell bitmap scaled with nearest-neighbour filtering under a static mask. Frames are built on a background thread into a ring of three bitmaps, so the render thread never draws one being written, and flip on the vsync closest to the schedule. While a code plays the screen stays on at full brightness and asks for the panel's fastest refresh rate.
- **Receiving.** CameraX `ImageAnalysis` in YUV_420_888 at 1080p; the core reads the planes in place through their addresses. The app runs its own exposure: a phone screen is a light source showing a new picture every few milliseconds, and auto exposure (1/50 s was measured) clips its colours and blends consecutive frames. The exposure stays under the core's limit (a sixth of the sender's frame period) and the gain follows the brightest pixels of the code. Autofocus is triggered on the code when it appears. The sensor's rolling-shutter readout goes to the core, which then scans every capture. The camera's frame rate follows the core (30 or 60 fps, when the camera offers 60 outside video recording).
- **Heat.** Three sources, the most cautious wins: the thermal status, the thermal headroom forecast, and the battery temperature (44 / 47 / 50 °C), because some vendors report neither of the first two. The core scans less as the level rises.
- **Interface cost.** While scanning, the screen refreshes its numbers at most 8 times a second and the code's outline redraws only when it moves; both matter on slow or throttled phones.

## Device tests without touching the phone

Debug and bench builds run a script passed as an intent extra; results go to logcat and the app's `files/` directory (readable with `run-as` in debug builds):

```sh
adb shell am start -n com.shelchin.lumencode/com.shelchin.lumen.MainActivity \
  --es lumen.script "'nav:receive;camera;wait:1;watch:20'"
adb logcat -s lumen-watch lumen-bench lumen-script
```

| Command | Effect |
|---|---|
| `nav`, `lang`, `tab`, `text`, `sendtext`, `demo:<bytes>`, `json`, `image`, `tap`, `settings`, `play`, `camera`, `close`, `event:<json>` | drive the app as a person would |
| `watch:<s>` | each second: exposure, ISO, rolling-shutter skew, focus, heat, view updates, the receiver's dashboard line (tag `lumen-watch`) |
| `grab:<n>` | the next n camera frames to `files/frame-N.nv12`, for the [analysis tools](../app/README.md#tools) |
| `replay:<dir>` | replays the frames in `files/<dir>` through the engine inside the app (tag `lumen-bench`) |
| `bench:<frames>` | scanner self-test on synthetic frames |
| `flips:<s>` | the sender's real frame rate and the display's refresh rate |
| `cammode:<fps>`, `formats`, `dump` | force a camera rate, list the camera's frame-rate ranges, write the view model |

The script runs in the running app as well (the activity is `singleTask`), so `watch` and `grab` work in the middle of a transfer.

To profile, install the bench build and record with simpleperf; the library in `src/app/target/aarch64-linux-android/release/` keeps the symbols the APK strips:

```sh
adb shell simpleperf record --app com.shelchin.lumencode --duration 5 -o /data/local/tmp/perf.data
adb shell simpleperf report -i /data/local/tmp/perf.data --sort comm,symbol
```
