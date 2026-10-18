# 2 · How the receiver reads a frame

[Guide index](README.md) · [中文](02-scanner.zh-CN.md) · Specification section: §11 (informative) · Code: `src/core/scan.js`, `src/rust/src/scan.rs`

The specification defines only "what a frame looks like", not "how to read it". This guide describes what the reference scanner does, and why. The JS and Rust scanners use the same algorithm. The Rust version is 1.5–2.6 times faster.

## 2.1 How bad real photos are

The sender draws a perfect grid. The camera captures something else:

| Effect | Cause | Impact on scanning |
|---|---|---|
| Perspective, rotation | The phone is not held square to the screen | The grid is no longer a rectangle |
| Barrel / pincushion distortion | Wide-angle phone lens | Even straight lines bend; the four corners alone cannot place the middle accurately |
| Blur | Bad focus, hand shake, motion | Black rings get thinner; cell colours bleed into each other |
| Bloom | Bright screen, high exposure | Bright areas get "fatter" all round |
| Moiré | Beat between screen pixels and camera pixels | A whole area becomes brighter or shifts in colour |
| Colour crosstalk, white balance | Camera colour processing, screen gamut | Cyan may look green; yellow may look white |
| Torn frame | Rolling shutter + screen frame change | The top and bottom halves come from different frames |
| 4:2:0 chroma subsampling | Video encoding | Colour resolution is only half the luma resolution |
| Glare, occlusion | Ambient light, fingers | A corner or an area is not visible at all |

Each step of the reference scanner deals with one or two of these. The camera simulator used for tests (`test/camera.js`) stacks all of them together.

## 2.2 Ten steps

### ① Preprocessing (luma and local threshold)

Convert RGB to luma. Split the image into 16×16-pixel blocks (32×32 for large images), and record the minimum and maximum luma of each block. The threshold for each pixel is the **midpoint** of the range over the surrounding 5×5 blocks.

- Why the midpoint of min and max, and not the mean? Near the edge of the code, most of a block is black quiet zone. The mean then shifts toward ink, and white rings are classified as black.
- Flat areas (background, sensor noise) are binarised only if their local range reaches 30% of the full-image range (and at least 40 levels). Otherwise a noisy photo produces thousands of "finder patterns".

### ② Find the finder patterns

Scan the binary image row by row for runs of "paper-ink-paper-ink-paper = 1:1:3:1:1", then cross-check in the vertical direction. The corner pattern works the same way, with ratio 1:2:1:2:1.

- The tolerance is deliberately loose: blur makes black rings thinner, and bloom makes bright parts fatter. It is better to find a few false ones. Later steps (module fitting, header CRC, alignment marker check) remove them.
- Each module is at least 2 pixels, so the scan can sample every other pixel. This doubles the speed.

### ③ Choose three corners

**Any three** of the four corners (three finder patterns + the corner pattern) are enough: three finder patterns, or two finder patterns plus the corner pattern. Candidates are sorted by how much they look like three corners of a grid (distance ratios, angles, size consistency) and tried one by one. The mirrored orientation is tried too (for a screen filmed in a mirror or from the back).

This is why a code still decodes when glare hides one corner.

### ④ Measure the cell size

At each finder pattern, find the positions of the six ring edges along two axes. An edge is taken at the **extremum of the luma gradient**, not at the threshold crossing, because blur shifts threshold crossings systematically. Then fit `t = c0 + m·k + δ·e`: m is the module size, and δ absorbs symmetric biases such as "black rings get thinner / bright parts get fatter".

### ⑤ Read the header

The header copies are right next to the finder patterns. Sample their 70 cells with the module size and orientation just measured.

- The module size always has some error, so try 17 scale hypotheses (±10%).
- For each hypothesis, find a threshold with an iterative "two-means" method. Try it once, and the global threshold once.
- A correct CRC means success.
- If every copy fails, take the soft values (−1 to +1) of each copy's "sharpest hypothesis", remove the masks, add them up, and take a **soft majority vote**. A copy that disagrees with the other copies on more than 1/4 of the bits is assumed to be read from the wrong position and is left out. A common case: each copy has one or two wrong bits, but in different positions. The sum is then correct.
- If that still fails, **reuse the last header that was read correctly**: within one transfer the grid size, colour mode, codeword length and number of parity bytes do not change, and decoding data does not need the frame number. This step runs only after every corner set (mirrored ones included) has been tried. A wrong reuse is stopped by the size check, the marker-lattice check and the packet CRCs; after 3 attempts in a row that yield nothing, the cached header is dropped. In the "hand-held, badly aimed" test it raised C4 frames decoded from 81/100 to 88/100, with no wrong data accepted. Frames decoded this way have no frame number, so they take no part in frame skipping or the frame-rate statistics.

### ⑥ Build the grid geometry

This step decides whether thousands of cells can be read accurately.

1. Starting from the three corners, find the alignment markers **step by step**. The position of each marker is predicted by a weighted local affine fit of the neighbours already found. Then a small area is searched for the pattern "bright centre, all eight neighbours dark". This way, the scanner follows perspective and distortion little by little.
2. Fit a least-squares homography to all markers found (with virtual points along the known edges as constraints). Then smoothly interpolate the **residuals** of nearby markers to correct lens distortion.
3. **Alignment marker check**: if almost no alignment markers are found at the lattice positions that a set of corners predicts (fewer than 60%), one of the "corners" is false (for example, a patch in the data area that happens to look like a finder pattern). Drop that set of corners and try the next one.
4. Every four adjacent alignment markers enclose a small quadrilateral, and each quadrilateral has its own homography matrix. Each cell is located with the matrix of the quadrilateral that contains it.

### ⑦ Sample colours

For each data cell, take the centre point (weight 2) and four points offset by 0.18 cell up, down, left and right, and average them. Only the cells of codewords that are **not yet decoded** are sampled (when the same frame is captured a second time, codewords already decoded are not computed again).

Colours are normalised against local references. The centre of each alignment marker is paper and its surround is ink, so each small quadrilateral has its own "white" and "black". The cell colour is interpolated by position between the references at the four corners and mapped to 0–1. This removes vignetting, uneven brightness and most white-balance error.

### ⑧ Classify

Run k-means clustering with the nominal palette as the starting point. This gives each cell a colour index and a **confidence** (the difference between the distances to the nearest and the second-nearest cluster centre).

Then apply **local gain correction** once. Split the grid into blocks of 4×4 cells. Use the classification results of the cells in each block to estimate the gain of each channel for that block (sparse blocks are pulled toward 1). Interpolate bilinearly, then classify again. This step is the main defence against moiré and local colour shifts.

### ⑨ Reed–Solomon decoding

Pack the cell bits back into bytes, and RS-decode each codeword:

1. First decode directly (this corrects up to nsym/2 wrong bytes).
2. If that fails, mark the **lowest-confidence** bytes as erasures and retry: first nsym/4 of them, then nsym/2. An erasure uses only half as much correction capacity as an error, so more errors can be corrected.
3. Use at most nsym/2 erasures, so that at least nsym/2 bytes of redundancy always remain. With too many erasures, RS "corrects" random data into some valid codeword.
4. Finally check the packet CRC-16 as one more filter.

### ⑩ Deduplication and frame skipping

The receiver identifies a frame by (seq, X, Y, mode, n, nsym). Cameras usually deliver 30 frames per second, but the sender changes only 10–12 frames per second. So the same frame is captured several times:

- If all codewords of this frame are already decoded, the next scan reads only the header and returns (about 8 ms in JS, against 14–45 ms for a full decode).
- If only some are decoded, the next scan samples only the remaining codewords.

## 2.3 Failure reasons

When scanning fails, the scanner returns a reason. This helps to locate the problem:

| reason | Meaning | Common causes |
|---|---|---|
| `finders` | Fewer than 3 finder pattern candidates | Code not in view, too small, too blurred |
| `geometry` | No set of three that looks like a grid | Only part of the code is captured; other interfering patterns are in view |
| `header` | No set gives a readable header | Too blurred; cells too small (< 3 pixels) |
| `lattice` | Alignment markers not found (internal use; the scanner goes on to the next set of corners) | False finder pattern |

## 2.4 Scheduling in the demo app (the key to saving power)

Apart from the scanner itself, the web demo does the following to cut computation and power (that is, phone heat):

1. **Follow the sender FPS**: estimate the sender FPS from the frame sequence numbers. Right after a new frame is obtained, do not scan again while the same frame is still on screen (within about 0.75 of a sender period). If the frame obtained was already decoded, wait about 1/3 of a period before trying again. This cuts the scans from about 30 per second ("scan every camera frame") to about 1.6 times the sender FPS.
2. **Region of interest (ROI) + adaptive resolution, done on the GPU**: once the code is found, the main thread takes only the area around it with `createImageBitmap(video, x, y, w, h, { resizeWidth, resizeHeight })`, and the GPU scales it to "about 6 pixels per cell". The worker receives only this small image. The copy and read-back of the full 1080p frame are gone, and preparing a scan takes about 3 ms. When the code is lost, or the phone is turned and the frame size changes, the full image at the original resolution is used again.
3. **Parallel Workers**: scanning runs in Web Workers (2 on phones, up to 3 on computers). The Rust/WASM engine is the default (about 35% faster than JS), with an automatic fallback to JS if it cannot load. Switching engines during a transfer keeps the progress: the packets received so far are handed to the new engine.
4. **No wasted work**: the camera runs at 30 FPS, and at 60 only while the sender plays 16 FPS or more (scans stay locked to the sender, so a faster camera adds captures, not scans); with no code in view, scanning drops to 5 per second, and to 2 per second after 10 s; the camera is turned off when the transfer completes and on again for "Receive another"; panels over the video use no background blur (`backdrop-filter` makes the GPU redo the blur for every camera frame).

## 2.5 Want to improve the scanner?

- First run `npm run test:robustness`. It reports the success rate under various combinations of distortions, which serves as a baseline.
- `test/batch.js` simulates random "hand-held, not aimed" scenes. It is the metric closest to a real phone.
- Keep the two scanners in sync: if you change JS, change Rust too (`src/rust/src/scan.rs`), then run `npm run build:wasm`.
- The scanning algorithm is not part of the protocol. You can improve it freely without affecting compatibility.
