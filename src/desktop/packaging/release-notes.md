## Desktop downloads

| System | File |
|---|---|
| macOS (Apple silicon and Intel) | `LumenCode-<version>-macos-universal.dmg` — signed and notarized; `-arm64` / `-x86_64` images are smaller |
| Windows 10/11 (x86_64) | `LumenCode-<version>-windows-x86_64.zip` — unzip and run `Lumen Code.exe` |
| Linux (x86_64, aarch64) | `LumenCode-<version>-linux-<arch>.tar.gz` — see `README.txt` inside (Vulkan, Wayland or X11) |

The Windows build is not code-signed yet: SmartScreen asks once ("More info" > "Run anyway"). The webcam receiver works on macOS; on Windows and Linux the desktop app sends, and decodes codes from images.

Phones: build the iOS and Android apps from `src/ios` and `src/android`, or use the web pages (`send.html`, `recv.html`) in any browser.

Checksums: `SHA256SUMS` (Windows, Linux) and `SHA256SUMS-macos` (macOS images, attached by hand after signing).
