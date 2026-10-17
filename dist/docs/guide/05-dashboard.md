# 5 · Reading the dashboard

[Guide index](README.md) · [中文](05-dashboard.zh-CN.md)

The demo pages show all the key numbers of a transfer. This guide explains what each number means, its healthy range, and what to adjust when something looks wrong. For the theory behind the speed, see [guide 4](04-performance.md).

## 5.1 Receiver: while scanning

```
┌────────────────────────────────────────────────────────────────────────────────────┐
│ photo.jpg                                                                      63% │  ← file name, progress
│ ████████████████████░░░░░░░░░░░░                                                   │
│ Speed 54 KB/s  ETA 3 s  Cell 5.2px  Success 97%                                    │  ← four main metrics
│ Reading well                                                                       │  ← tip
│ WASM 7.7 ms · cam 30 · scan 19/s · new frames 11.6/s · sender 12/s · captured 97%  │  ← diagnostic line
└────────────────────────────────────────────────────────────────────────────────────┘
```

### Status and progress

| Display | Meaning |
|---|---|
| Status at top left: "Looking for a code…" / "Receiving" | Whether a code was detected in the last 0.7 seconds |
| File name | Shown once the file descriptor arrives (the descriptor appears every 4 frames, and in every frame for small files) |
| Progress % | Distinct symbols received ÷ total source symbols of the file. The fountain code needs two or three symbols more than the total, so progress stays at 99% for a moment, until decoding succeeds and it becomes 100% |

### Four main metrics

| Metric | How it is computed | Healthy | When low |
|---|---|---|---|
| **Speed** | New valid symbols in the last 3 seconds × bytes per symbol T. This is the payload speed on the wire (after compression), without duplicate packets | Close to the sender's "Raw rate" × 0.8 | Check the next three metrics and the diagnostic line to find which stage is the problem |
| **ETA** | Symbols still missing ÷ current symbol rate | — | — |
| **Cell** | Average number of pixels that one cell covers in the camera photo, over the last 2.5 seconds | ≥ 4.5 | Below 3.4 the code is unreadable: move closer, or lower the density on the sender. At 3.4–4.5 it is barely readable, and the success rate suffers |
| **Success** | For the frames first seen in the last 2.5 seconds, the share of their codewords that are decoded. A frame captured several times is counted once, with the results combined | ≥ 90% | See "Tips" below |

### Diagnostic line

The diagnostic line answers one question: **which stage is holding the speed back?**

| Field | Meaning | Healthy |
|---|---|---|
| `JS 11.0 ms` / `WASM 7.7 ms` | Average time that the current engine takes to scan one photo (in a Worker) | Phone-size code: WASM < 15 ms, JS < 25 ms |
| `cam 30` | Images per second that the camera actually delivers | Usually 30. Below 20 is common in poor light (auto-exposure makes the shutter time longer) |
| `scan 19/s` | Scans actually done per second | About 1.5–2 × sender FPS. This is **deliberate**: the same frame is not scanned again while it is still on screen, to save power |
| `new frames 11.6/s` | **Distinct** sender frames obtained per second (told apart by frame sequence number) | Close to the sender FPS |
| `sender 12/s` | Sender FPS, estimated from the gaps between frame sequence numbers | Equal to the FPS set on the sender |
| `captured 97%` | New frames ÷ sender frames: the share of the sender's frames that were captured and scanned | ≥ 90% |

How to use it:

- **Captured high, success high** → all normal. If the scan time is also low, you can raise the sender FPS (the receiver suggests it).
- **Captured low, engine time high** → the receiver does not have enough compute power, and scanning cannot keep up. Switch to the WASM engine, or lower the sender FPS.
- **Captured low, engine time normal** → the camera FPS is low (check "cam"), or the code often moves out of view.
- **Captured high, success low** → a photo quality problem: check the "Cell" pixel count, then the tip.

> About the capture rate: dropped frames do **not** need to be sent again. With a fountain code, a missed frame loses only the data of that frame; the other frames are still useful. So lowering the sender FPS is not a way to "fix dropped frames", unless the receiver really cannot keep up with the computation.

### Tips

The tips are checked in order, and the first one that holds is shown:

| Tip | Condition | Advice |
|---|---|---|
| Fit the whole code in view | No code detected within 0.7 seconds | Aim so that all four corners are in view (one missing corner is still readable) |
| The code is landscape — try rotating your phone | Cell < 3.4 pixels, and the code is landscape while the phone is held upright | Turn the phone (the receiver page turns with it) |
| Move closer, or lower the density on the sender | Cell < 3.4 pixels | Move closer, or set "Density" on the sender to "Low" |
| Hold steady and avoid glare | Success < 50%, and cell < 4.5 pixels | Hold the phone steadier; change the angle to avoid glare |
| Colours are hard to read: try 4 colours or lower density on the sender | Success < 50%, and the cells are large enough | Switch the sender to 4 colours. Check the screen brightness and colour mode (night mode and eye-comfort mode change the colours) |
| Decoding cannot keep up: try the WASM engine | JS engine, captured < 80%, Worker load > 80% | Switch to WASM |
| Spare capacity: you can raise the sender FPS to about N | Captured > 95%, success > 90%, load < 50%, camera FPS > 2.2 × sender FPS | Raise the FPS as suggested |
| Reading well | None of the above holds | — |
| The WASM engine could not load; switched back to JS | The WASM module failed to download or instantiate | Check that `wasm/lumen.wasm` is deployed, and that the server MIME type is `application/wasm` |
| Checksum mismatch, still receiving… | The decoded file size or CRC-32 is wrong (very rare) | No action needed. The receiver keeps collecting symbols |

## 5.2 Receiver: after completion

When text or an image is received, the result is shown in a full-screen reader view. Other files are shown as a card. The "Details" button expands a summary of the transfer:

| Item | Meaning |
|---|---|
| Time | From the first valid packet received until the file is restored |
| Average speed | **Original file** size ÷ time. Text is compressed, so this number is usually higher than the receive rate |
| Receive rate | Symbols received × T ÷ time, that is, the payload speed on the wire |
| New frames / scans | Distinct sender frames obtained / scans actually done. The closer the ratio is to 1, the less computation is spent |
| Success | Over the whole transfer, the share of decoded codewords in all frames seen |
| Engine | The scan engine used, and its average time |
| Code | Grid size, number of colours, ECC ratio (nsym / n), average pixels per cell |

The camera turns off by itself after completion (to save power); "Receive another" turns it on again.

Reader view features: JSON is formatted and highlighted automatically (you can switch to "Raw"); line numbers; a line-wrap toggle; font size control; links in the text are clickable. In the "Formatted" view, "Copy" copies the formatted JSON. "Save as file" always saves the original bytes.

## 5.3 Sender

First row of the control bar:

| Display | Meaning |
|---|---|
| Raw rate | Payload per frame × FPS. The speed limit if the receiver gets every frame |
| Per frame | Codewords per frame × payload per codeword T |
| One pass | Time needed to send every source symbol once. The actual completion time is about this value divided by the capture rate, then divided by the success rate |
| Grid | X × Y cells, bits per cell |
| JS / WASM x ms | Time to generate one frame (encode + draw the cells), usually far below 1 ms |

Settings:

| Setting | Effect | Recommendation |
|---|---|---|
| Density | Multiplies the base of "about 0.75 mm per cell" by 0.72 / 1 / 1.3 / 1.65 | Auto; Low when the receiver's "Cell" is < 4.5 |
| Colours | 4 colours: 2 bits per cell; 8 colours: 3 bits per cell | 4 colours; try 8 colours when conditions are very good |
| ECC | Low 10% / Mid 16% / High 22% of the bytes are used for RS check bytes | Mid; High when there is a lot of glare or shaking |
| FPS | Frames shown per second (4–30) | 12; raise it when the receiver reports spare capacity |
| Engine | Generate frames with JS or with Rust/WASM | Both give bit-identical output; either one is fine |

The code adapts its width and height to fill the screen (aspect ratio up to 2.4) and refills it when the screen is turned. While the control dock is shown it covers the code's two bottom corners; it hides itself after 2.6 s of playing, or with the hide button or a tap on the code.

Changing the density, colours or ECC, or a change of screen size, **does not interrupt** the transfer: the symbol size T stays the same, and the receiver continues seamlessly.

## 5.4 Receiver controls

| Action | How |
|---|---|
| Zoom the camera | Pinch on the image; on a computer, Ctrl + scroll wheel or a trackpad pinch. The badge in the middle shows the zoom level; tap it to reset to 1×. Shown only when the camera supports zoom |
| Switch engine | `JS \| WASM` in the top bar, WASM by default. Switching during a scan **keeps the progress** |
| Switch camera | The button on the right (shown when there are several cameras) |
| Home / go to Send | "‹" and the logo at the top left go home; the link at the bottom of the start screen goes to the send page |

## 5.5 Troubleshooting "why is it slow"

```
Speed far below the Raw rate?
├─ Cell < 4.5px ────────────────────► move closer / lower the density
├─ Success < 90%
│   ├─ cell large enough ───────────► 4 colours, higher ECC, avoid glare, turn off eye-comfort/night mode
│   └─ cell on the small side ──────► move closer / lower the density
├─ captured < 90%
│   ├─ high engine time (JS) ───────► switch to WASM
│   ├─ cam < 25 ────────────────────► more ambient light
│   └─ other ───────────────────────► lower the sender FPS
└─ all normal ──────────────────────► raise the FPS as the tip suggests; when conditions are very good, try 8 colours or a higher density
```

### Two example readings

**Example 1**: `JS 38.0 ms · cam 30 · scan 9/s · new frames 6.8/s · sender 12/s · captured 57%`, Success 95%, Cell 5.0px.
The photo quality is very good (high success rate, large cells). But each scan takes 38 ms, so only 9 scans run per second, and half of the frames are missed. Switching to WASM fixes it, and the receiver suggests this automatically.

**Example 2**: `WASM 8.1 ms · cam 30 · scan 20/s · new frames 11.8/s · sender 12/s · captured 98%`, Success 62%, Cell 3.6px.
There is enough compute power, and almost every frame is captured. But the cells are too small, so only about 60% of the codewords decode. Move a little closer, or lower the density on the sender by one step.
