## Lumen Code <version>

The first stable release. Protocol 1.0 is final and frozen: every 1.0 receiver reads every 1.0 sender.

- Simpler send and receive screens: a permanent control bar that never covers the code; on the receive screen, progress, speed and one hint, with the details folded away.
- A photosensitivity confirmation before the first code plays.
- A calmer result screen: the file, Save and Open, and the transfer details on request.
- The app identifier is now `ltd.mondaylabs.lumencode`. macOS treats this version as a new app: allow the camera again on first use.

## Desktop downloads

| System | File |
|---|---|
| macOS (Apple silicon and Intel) | `LumenCode-<version>-macos-universal.dmg` — signed and notarized; `-arm64` / `-x86_64` images are smaller |
| Windows 10/11 (x86_64) | `LumenCode-<version>-windows-x86_64.zip` — unzip and run `Lumen Code.exe` |
| Linux (x86_64, aarch64) | `LumenCode-<version>-linux-<arch>.tar.gz` — see `README.txt` inside (Vulkan, Wayland or X11) |

The Windows build is not code-signed yet, so SmartScreen warns on first launch: choose "More info" > "Run anyway". The webcam receiver is tested on macOS. The Windows (Media Foundation) and Linux (Video4Linux) camera code is new and has not been tested on real machines; reports are welcome.

Phones: the iPhone and Android apps are coming to the App Store and Google Play; the web version at https://lumencode.link works in any browser with a camera, and every version sends to every other.

Checksums: `SHA256SUMS` (Windows, Linux) and `SHA256SUMS-macos` (macOS images, attached by hand after signing).
