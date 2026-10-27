# 4 · Performance: where the speed comes from

[Guide index](README.md) · [中文](04-performance.zh-CN.md)

## 4.1 One formula

```
actual speed = FPS × codewords per frame × payload per codeword T
               └───────── set by the sender: raw rate ──────────┘
             × capture rate × codeword success rate
               └────── set by the receiver ───────┘
             ÷ (1 + fountain overhead)
               └──────── ≈ 0 ────────┘
```

- **Raw rate**: the "Raw rate" shown by the demo sender. It assumes every frame is received in full.
- **Capture rate**: the share of the sender's frames that the receiver captured and scanned ("captured" in the demo receiver's diagnostic line).
- **Codeword success rate**: the share of codewords decoded from the captured frames ("Success").
- **Fountain overhead**: recovering K source symbols takes on average K + 1.6 symbols (K + 2.4 for the largest blocks; specification Appendix C.2), which is negligible for large files.

Every speed-up technique raises one of these terms.

## 4.2 Comparison with animated QR codes

The baseline is a common animated QR setup: QR version 25 at ECC level L (1 273 bytes per frame), looped at 12 FPS.

| | Animated QR | Lumen Code (phone screen) | Lumen Code (laptop screen) |
|---|---|---|---|
| Grid | 117 × 117 = 13 689 | 83 × 147 = 12 201 | 143 × 254 = 36 322 |
| Bits per cell | 1 | 2 (C4) | 2 (C4) / 3 (C8) |
| Payload per frame | 1.24 KB | 2.2 KB | 6.9 KB / 10.3 KB |
| Raw rate (12 FPS) | 15 KB/s | 27 KB/s | 82 KB/s / 124 KB/s |
| Effective factor with 20% of frames dropped | ÷ 3.25 (loop, 46-frame file) | ÷ 1.26 | ÷ 1.26 |
| Half of a frame blurred | Whole frame lost | Codewords in the other half still usable | Same as left |

With 20% of frames dropped, which is common, Lumen Code (C4) on a laptop screen is about **14 times** faster than an animated QR code (82/1.26 ≈ 65 KB/s against 15/3.25 ≈ 4.6 KB/s), and on a phone screen about **4.6 times** faster.

## 4.3 Seven sources

### ① More bits per cell (× 2 or × 3)

A QR module is black or white: 1 bit. Lumen Code uses colour: 2 bits per cell with C4, 3 with C8. The three bits of C8 map directly to the R, G and B channels, so one misjudged channel costs only 1 bit.

C8 is not the default because the more colours there are, the closer adjacent colours look to the camera, and the more sensitive they are to blur, moiré and white balance. In the simulated "hand-held, not aimed" test, the codeword success rate is about 86–89% for C4 and about 60% for C8. C4 × 0.88 = 1.76 bits/cell is only slightly less than C8 × 0.6 = 1.8 bits/cell, and much more stable.

### ② More cells (shape follows the screen, size follows the physical dimensions)

- **Shape**: QR is square, but phone screens and camera images are rectangles, and a square covers at most 56% of a 16:9 area. A Lumen Code grid can be any X × Y. The reference sender fills the screen (aspect ratio ≤ 2.4); on an upright phone this gives a tall, narrow code with about 25% more cells than a 16:9 limit would allow.
- **Size**: the number of QR modules is set by the version, independent of the screen. Lumen Code chooses the cell size from the **physical size** of the screen, about 0.75 mm per cell, which is close to what a 1080p camera can resolve at a normal distance. A laptop screen therefore fits 36 000 cells, 2.65 times as many as QR v25.

### ③ Low structural overhead

In full-size frames, data cells make up 92.6% (phone) to 95.3% (laptop) of the grid. After subtracting the RS check bytes (16%), the 7-byte header and trailer of each packet, and filler, the **file payload is 75–77% of all cell bits**.

### ④ ECC designed for the screen-to-camera channel

- Each codeword has its own RS (n ≤ 255) ECC, at level 10% / 16% / 22% / 30%.
- The receiver uses the confidence of the colour classification to find the most suspect bytes and retries with them as erasures, which brings "soft information" into RS decoding.
- Every packet has a CRC-16, so aggressive erasure retries are safe: wrong data cannot get into the fountain decoder.

### ⑤ Independent codewords + Hilbert layout: half a frame is still useful

A QR frame either decodes completely or is lost. A Lumen Code frame has a dozen to several dozen independent codewords, each occupying a compact area along the Hilbert curve. A torn frame (the camera captures half of the old frame and half of the new one), a patch of glare or an out-of-focus corner loses only the codewords in that area (see the codeword map in [guide 1](01-how-it-works.md)). Such partly usable frames are a large share of real captures.

### ⑥ Fountain code: every frame is new data

With looped playback the receiver must collect every fragment, and each missed one costs a whole pass. With a frame loss rate p and N frames per pass, the average number of passes needed (equal to the time factor) is:

| File frames N | 10% lost | 20% lost | 30% lost |
|---|---|---|---|
| 10 | 1.76 | 2.32 | 2.93 |
| 46 | 2.41 | 3.25 | 4.17 |
| 150 | 2.93 | 3.97 | 5.14 |

A fountain code only needs enough distinct symbols, so its time factor is about 1 / (1 − p): 1.25 at 20% loss, whatever the file size. The advantage grows with file size and loss rate; across the table above it is 1.5 to 3.6 times.

The receiver can also start scanning at any time, and moving away and aiming again loses no data already received. The file descriptor repeats every 4 frames, so the file name is available at any time.

### ⑦ Compression

Text, JSON, logs and source code are compressed with DEFLATE before sending, usually to 1/3–1/5 of their size, which multiplies the speed by 3–5. The "Average speed" that the demo receiver shows on completion is computed from the original file size, so for text it is clearly higher than the "Receive rate".

## 4.4 Compute side: helping the receiver keep up

The protocol sets the upper limit on speed; the receiver's compute speed decides how much of it is actually received. If scanning a frame takes 45 ms while the camera delivers a frame every 33 ms and the sender changes frames every 83 ms, the receiver misses frames and heats up. The reference implementation does the following:

| Technique | Effect |
|---|---|
| Scanning follows the sender FPS (the same frame is not scanned twice) | Scans drop from about 30 per second to about 1.6 × sender FPS (about 16–20 at 12 FPS) |
| A frame that is already decoded: read only the header | About 8 ms, against 14–45 ms for a full scan |
| Second capture of the same frame: decode only the remaining codewords | Partly decoded frames are faster the second time |
| ROI + adaptive resolution (scaled to about 6 pixels per cell) | About 1/3 of the pixels, scanning about 2 times faster, same decode rate |
| ROI crop and scaling done by the GPU (the crop and resize options of `createImageBitmap`) | No copy and read-back of the full 1080p frame; preparing a scan takes about 3 ms |
| Camera off after completion; fewer scans with no code in view; camera at 30 FPS unless the sender plays fast | Almost no power used while reading the result or with the phone put down |
| No background blur over the video | Saves one full-screen GPU blur per camera frame |
| Rust → WebAssembly | 1.5–2.1 times faster than JS (native Rust 1.6–2.6 times) |
| Parallel Workers | 2 on phones, up to 3 on computers |
| GPU compositing on the sender (a canvas with 1 pixel per cell + a static mask, scaled up by the GPU) | The sender's CPU cost per frame is close to zero |

On an Apple M2, a full scan of a 1080p photo takes 14–16 ms in JS and 9–10 ms in WASM for a phone-size code, and 41–45 ms in JS and 21 ms in WASM for a laptop-size code.

## 4.5 What actually limits the speed

In order of impact:

1. **Pixels per cell in the camera image.** ≥ 4.5 pixels is very stable, 3.4–4.5 is marginal, and < 3.4 is mostly unreadable. It depends on the cell size on the screen, the capture distance and the camera resolution. A higher "Density" puts more data in each frame, but the cells get smaller and the success rate drops.
2. **Camera**: most phone web pages get only 30 FPS; the rolling shutter causes tearing; auto-exposure may overexpose in front of a bright screen; motion blur.
3. **Display**: on a 60 Hz screen, the FPS should be a divisor of 60 (10, 12, 15, 20, 30); otherwise the time each frame stays on screen varies.
4. **Receiver compute power**: if "captured" in the diagnostic line is below 80% and the engine time is high, compute power is the limit. Switch to WASM or lower the scan resolution.

Tuning order: first secure the success rate (≥ 4.5 pixels per cell, success rate ≥ 90%), then raise the FPS, and only then consider a higher density or C8. When there is headroom, the receiver shows "Spare capacity: you can raise the sender FPS to about N".

## 4.6 Measured cost of each stage

Method: `node test/bench.js` (JavaScript, Node 22) and `cargo run --release --example measure -- bench` in `src/rust` (Rust, native). Each value is the mean of many repetitions after one warm-up run. Machine: Apple M2 Pro, macOS 26.5. T = 200 bytes; RS(255, 215) is the default codeword of a large grid.

| Stage | JavaScript | Rust |
|---|---|---|
| RS encode, one codeword | 12.8 µs | 8.0 µs |
| RS decode, clean codeword | 33 µs | 33 µs |
| RS decode, 20 byte errors | 87 µs | 75 µs |
| Fountain: one encoding symbol | 1.2 µs | 0.12 µs |
| Fountain: decode K = 1000 from K + 10 symbols | 8.4 ms (24 MB/s) | 0.8 ms (250 MB/s) |
| Fountain: decode K = 16 384 from K + 10 symbols | 105 ms (31 MB/s) | 20 ms (164 MB/s) |
| Frame cells, 150 × 240 C4 (34 codewords) | 0.76 ms | 0.76 ms |
| Inflate (zlib stream of 2.2 MB of text) | 147 MB/s | — |
| Receiver, 2 MB transfer from packets (10 064 packets) | 168 ms (12 MB/s) | 25 ms (79 MB/s) |

The receiver's packet handling and decoding cost far less than scanning (§4.4): on a 100 KB/s camera link, everything after the scanner uses about 1 % of one core.

## 4.7 Can it go faster? Roadmap

The improvements are grouped by whether the protocol must change. Those that do not can be made at any time and stay compatible with all existing implementations.

### A. Receiver only (no protocol change)

| Improvement | How it works | Expected gain | Difficulty |
|---|---|---|---|
| **Cross-frame soft combining** | The same frame is usually captured 2–3 times, and today each capture is decoded independently. Instead, add up the colour likelihoods of the same cell across captures before classifying. A codeword that neither capture decodes alone may decode when they are combined | A clear success-rate gain in marginal conditions (3.4–4.5 pixels per cell), roughly worth one more density step | Medium |
| **Deblurring / crosstalk equalisation** | Blur mixes the colours of neighbours into each cell. Apply one equalisation pass with the measured blur kernel (subtract the weighted contribution of the neighbours from each cell) | Smaller cells become usable: density +20–30% | Medium |
| **60 FPS capture** | On devices that support it, request 60 FPS and use `requestVideoFrameCallback` to grab frames precisely | The sender can go up to 20–24 FPS | Low (limited by the device) |
| **WebGPU scanning** | Sampling and classification are highly parallel and suit the GPU | Scan time drops several times further; less heat | High |

### B. Sender only (no protocol change)

| Improvement | How it works | Expected gain |
|---|---|---|
| FPS aligned to the refresh rate | On a 120 Hz screen, use 20, 24 or 30 FPS so that each frame stays on screen for a constant time | Less tearing; higher FPS becomes usable |
| Tuning by the receiver's tips | There is no return channel, but a person can read the receiver's tips and adjust FPS and density | Already implemented (the tip "you can raise the sender FPS to about N") |

### C. Protocol changes (not in 1.0; left for a later version)

| Improvement | How it works | Cost | Recommendation |
|---|---|---|---|
| Header ECC | A BCH or short RS code on the header | 21–35 more cells per copy; the frame format changes | **Not adopted**: measured, when a header cannot be read its best copy usually has 15–30 wrong bits (it was sampled in the wrong place), so an ECC would recover only about 1/4 of them. Instead the receiver reuses the last correct header (implemented, no protocol change), which in the same test captured the full gain of a perfect header ECC (+3.7 points of C4 codeword success) |
| LDPC / soft-decision codes instead of RS | Use the colour confidence of each cell directly; coding gain about 1.5–2 dB | High decoding complexity; hard to implement | Leave for v2 |
| More colours (C16) | 4 bits per cell | Cameras cannot tell the colours apart well enough; the success rate would drop sharply | Not recommended |

The best next step is cross-frame soft combining from group A: it needs no protocol change and raises the success rate directly. The header format stays as it is; it is frozen in 1.0.
