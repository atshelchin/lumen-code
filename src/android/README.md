# Lumen Code for Android

A Jetpack Compose shell over the [shared Rust core](../app/README.md). It shows the core's view model, performs the operations the core asks for and passes it camera frames. Everything else (what to show, when to scan, every string) is decided in Rust and is the same on iOS and the desktop.

## Requirements

- JDK 17 or later (Android Studio's bundled JBR works), Android SDK 36 with an NDK
- Rust with the Android targets and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk):
  ```sh
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  cargo install cargo-ndk
  ```
- A device with Android 10 (API 29) or later

Gradle builds the Rust core: the `cargoBuild` task runs `scripts/build-core.sh` and is up to date when nothing under `src/app` or `src/rust` has changed. Pass `-PlumenSkipRust` to skip it when only Kotlin changed.

## Build and run

```sh
./gradlew :app:installDebug      # debug build
./gradlew :app:assembleBench     # release code + test hooks: what to measure performance with
./gradlew :app:assembleRelease   # release
```

| Build type | Use |
|---|---|
| `debug` | Development. The build is debuggable, so ART interprets much of the UI and checks every JNI call; the interface and every call into Rust run several times slower than in release. Do not judge performance with it. |
| `bench` | Release code (compiled ahead of time, no JNI checks), profileable with simpleperf, with the test hooks below. |
| `release` | The app. It is still signed with the debug key; set up a release signing config before publishing. |

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

- **Sending.** The code is a one-pixel-per-cell bitmap scaled with nearest-neighbour filtering under a static mask. Frames are built on a background thread into a ring of three bitmaps, so the render thread never draws a bitmap that is still being written, and each frame is shown on the vsync closest to its scheduled time. While a code plays, the screen stays on at full brightness and requests the panel's fastest refresh rate.
- **Receiving.** CameraX `ImageAnalysis` delivers YUV_420_888 at 1080p, and the core reads the planes in place through their addresses. The app sets exposure itself: a phone screen is a light source showing a new picture every few milliseconds, and auto exposure (measured at 1/50 s) clips its colours and blends consecutive frames. Exposure time stays under the core's limit (a sixth of the sender's frame period) and the gain follows the brightest pixels of the code. Autofocus is triggered on the code when it appears. The sensor's rolling-shutter readout is reported to the core, which then scans every capture. The core sets the frame rate: 30 fps, or 60 fps when the camera offers 60 outside video recording.
- **Heat.** The most cautious of three sources wins: the thermal status, the thermal headroom forecast and the battery temperature (44 / 47 / 50 °C), since some vendors report neither of the first two. The core scans less as the level rises.
- **Interface cost.** While scanning, the on-screen numbers refresh at most 8 times a second and the code's outline is redrawn only when it moves. Both matter on slow or throttled phones.

## Device tests without touching the phone

Debug and bench builds run a script passed as an intent extra. Results go to logcat and to the app's `files/` directory (readable with `run-as` in debug builds):

```sh
adb shell am start -n ltd.mondaylabs.lumencode/com.shelchin.lumen.MainActivity \
  --es lumen.script "'nav:receive;camera;wait:1;watch:20'"
adb logcat -s lumen-watch lumen-bench lumen-script
```

| Command | Effect |
|---|---|
| `nav`, `lang`, `tab`, `text`, `sendtext`, `demo:<bytes>[:<name>]`, `json`, `image`, `tap`, `settings`, `play`, `camera`, `close`, `event:<json>` | drive the app as a person would |
| `watch:<s>` | each second: exposure, ISO, rolling-shutter skew, focus, heat, view updates, the receiver's dashboard line (tag `lumen-watch`) |
| `grab:<n>` | the next n camera frames to `files/frame-N.nv12`, for the [analysis tools](../app/README.md#tools) |
| `replay:<dir>` | replays the frames in `files/<dir>` through the engine inside the app (tag `lumen-bench`) |
| `bench:<frames>` | scanner self-test on synthetic frames |
| `flips:<s>` | the sender's real frame rate and the display's refresh rate |
| `cammode:<fps>`, `formats`, `dump` | force a camera rate, list the camera's frame-rate ranges, write the view model |

A script also runs in an app that is already open (the activity is `singleTask`), so `watch` and `grab` work in the middle of a transfer.

To profile, install the bench build and record with simpleperf. The library in `src/app/target/aarch64-linux-android/release/` keeps the symbols that the APK strips:

```sh
adb shell simpleperf record --app ltd.mondaylabs.lumencode --duration 5 -o /data/local/tmp/perf.data
adb shell simpleperf report -i /data/local/tmp/perf.data --sort comm,symbol
```
