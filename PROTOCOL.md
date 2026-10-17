# Lumen Code v1 — Protocol Specification

| Item | Value |
|---|---|
| Protocol | Lumen Code, version 1 |
| Status | Draft for public review |
| Date | 2026-10-17 |
| Language | ASD-STE100 Simplified Technical English |
| Chinese version | [PROTOCOL.zh-CN.md](PROTOCOL.zh-CN.md) |
| Test vectors | [test/vectors/test-vectors.json](test/vectors/test-vectors.json) |
| Reference implementations | JavaScript (`src/core/`), Rust (`src/rust/`) |
| License | MIT (this document and the reference implementations) |

---

## 1 Scope

Lumen Code moves a file from a screen to a camera. The sender shows a sequence of frames. Each frame is a grid of coloured cells. The receiver records the frames with a camera and decodes them.

The protocol uses no network, no pairing and no return channel. The receiver can start at any time. The receiver can lose frames. Each frame carries new data, so the receiver does not need a specific frame.

This document specifies:

- the frame format (grid, patterns, header, codewords, colours),
- the packet format and the file descriptor,
- the fountain code that makes the packet stream,
- the sender procedure.

This document also gives recommendations for receivers. A receiver can use any detection algorithm.

## 2 Conventions

### 2.1 Requirement words

The words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY have the meanings of RFC 2119. Sections and notes that use the word "informative" do not contain requirements.

### 2.2 Numbers and operators

- All integers are unsigned, unless the text says differently.
- `floor(a / b)` is integer division. `ceil(a / b)` is `floor((a + b − 1) / b)`.
- `a mod b` is the remainder. It is always in `[0, b)`.
- `XOR`, `AND`, `>>` and `<<` are bit operators.
- "32-bit arithmetic" means that each result is reduced modulo 2^32.
- `bitlen(k)` is the number of bits in `k`. Example: `bitlen(1) = 1`, `bitlen(4) = 3`.
- A prime is an integer ≥ 2 that has no divisors other than 1 and itself.

### 2.3 Byte and bit order

- Multi-byte fields in packets and descriptors are big-endian.
- A bit sequence made from bytes starts with the most significant bit of the first byte.
- An integer made from a bit sequence uses the first bit as the most significant bit.

### 2.4 Terminology

| Term | Meaning |
|---|---|
| cell | One square of the grid. The cell is the unit of all frame dimensions. |
| grid | The X × Y cells of one frame. |
| frame | One image that the sender shows. |
| ink, paper | The dark and the light reference colours. |
| data cell | A cell that carries codeword bits. |
| slot | The position of a data cell in the data order (§4.6). |
| codeword | One Reed–Solomon codeword of n bytes. |
| packet | The message part of one codeword (k = n − nsym bytes). |
| symbol | One fountain-code encoding symbol of T bytes. |
| transfer | All frames that carry one file. A transfer has one transfer ID. |
| source block | A part of the payload that the fountain code encodes separately. |

## 3 Overview (informative)

```
file ──(optional DEFLATE)──► payload ──split──► source blocks (≤ 16384 symbols of T bytes)
                                                    │ fountain code (§9)
                                                    ▼
             file descriptor ──► packets: [transfer ID | M,SBN,ESI | T bytes | CRC-16]
                                                    │ Reed–Solomon (§7)
                                                    ▼
                                    codewords ──bits──► data cells (Hilbert order, whitened)
                                                    │ + finders, corner, markers, header
                                                    ▼
                                         frame (grid of coloured cells) ──► screen ──► camera
```

A receiver does these steps in the opposite direction. One complete packet is enough to use the data in it. The receiver can reconstruct a source block when it has approximately K + 2 different packets of that block.

## 4 Grid

### 4.1 Dimensions and coordinates

The grid has X columns and Y rows. X and Y MUST be in the range 40 to 511.

The cell `(x, y)` is in column x and row y. The cell `(0, 0)` is at the top-left corner. The cell index is `i = y · X + x`.

Each cell has one kind: fixed, header, marker or data. Sections 4.2 to 4.5 assign the kinds in this sequence. Cells that do not get a kind are data cells. The assignments do not overlap.

### 4.2 Finders and the corner pattern

The grid has three finders and one corner pattern. Each one is a square of 7 × 7 fixed cells.

| Pattern | Top-left cell |
|---|---|
| Finder TL | (0, 0) |
| Finder TR | (X − 7, 0) |
| Finder BL | (0, Y − 7) |
| Corner pattern BR | (X − 7, Y − 7) |

For the cell `(ox + i, oy + j)` with `i, j` in `[0, 7)`, calculate `d = max(|i − 3|, |j − 3|)`.

- In a finder, the cell is paper if `d = 3` or `d ≤ 1`. Other cells are ink.
- In the corner pattern, the cell is paper if `d = 3` or `d = 0`. Other cells are ink.

A line through the centre of a finder crosses paper, ink, paper, ink and paper in the proportions 1 : 1 : 3 : 1 : 1. A line through the centre of the corner pattern crosses them in the proportions 1 : 2 : 1 : 2 : 1.

### 4.3 Separators

Each pattern has a separator of ink cells on its inner sides. For each `i` from 0 to 7 (both included), set these cells to fixed ink:

```
(7, i)  (i, 7)  (X−8, i)  (X−1−i, 7)  (7, Y−1−i)  (i, Y−8)  (X−8, Y−1−i)  (X−1−i, Y−8)
```

### 4.4 Header copies

The grid has four copies of the frame header (§6). Each copy has 70 header cells. Bit `b` of a copy is in the cell given below, with `r = floor(b / 10)` and `c = b mod 10`:

| Copy | Near | Cell of bit b |
|---|---|---|
| 0 (A) | Finder TL, right side | (8 + c, r) |
| 1 (B) | Finder TL, lower side | (r, 8 + c) |
| 2 (C) | Finder TR, left side | (X − 18 + c, r) |
| 3 (D) | Finder BL, upper side | (r, Y − 18 + c) |

A header cell with bit value 1 is paper. A header cell with bit value 0 is ink.

### 4.5 Alignment markers

Alignment markers make a lattice. Calculate the lattice positions along one axis of D cells (D is X or Y) as follows:

```
span = D − 7
segs = max(1, floor((span + 9) / 18))
while segs > 1 and span < 16 · segs:
    segs = segs − 1
for i = 0 … segs:
    L[i] = 3 + floor((2 · i · span + segs) / (2 · segs))
```

The lattice points are `(LX[i], LY[j])` for all `i` and `j`. The four corner points of the lattice are the centres of the finders and of the corner pattern. Do not put markers at these four points.

At each other lattice point, put a marker of 3 × 3 marker cells. The centre cell is paper. The eight other cells are ink.

### 4.6 Data cells and data order

All other cells are data cells. Their sequence is the data order. Calculate it as follows:

1. Find the smallest power of two `N` with `N ≥ max(X, Y)`.
2. For `d = 0 … N·N − 1`, calculate `(x, y) = hilbert(N, d)` (Appendix A.4).
3. If `x < X`, `y < Y` and the cell `(x, y)` is a data cell, append it to the data order.

The position of a cell in the data order is its slot. `D` is the number of data cells.

> Informative: the Hilbert curve keeps each codeword in one compact area. A torn frame (the camera reads half of the old frame and half of the new frame) then loses only the codewords on the tear line.

### 4.7 Whitening

Each slot `s` has a whitening value:

```
w[s] = mix32(0x9E3779B9 XOR ((s + 1) · 0x85EBCA6B)) AND 7        (32-bit arithmetic)
```

`mix32` is in Appendix A.1.

## 5 Colour modes

The colour mode sets the bits per cell (bpc) and the palette. The values are nominal sRGB colours.

| Mode | Name | bpc | Palette index → colour |
|---|---|---|---|
| 0 | C2 | 1 | 0 ink, 1 paper |
| 1 | C4 | 2 | 0 ink, 1 (0, 216, 255), 2 (255, 40, 150), 3 (255, 214, 0) |
| 2 | C8 | 3 | 0 ink, 1 (44, 80, 255), 2 (26, 222, 96), 3 (0, 220, 255), 4 (255, 52, 76), 5 (250, 52, 220), 6 (255, 214, 10), 7 paper |

- Ink is (10, 13, 20). Paper is (242, 244, 248).
- Mode 3 is reserved. A receiver MUST reject a header with mode 3.
- In mode C8, the bits of the index are (R, G, B). One wrong colour channel then gives one wrong bit.
- A sender SHOULD use the nominal colours. A sender MAY change the colours a small quantity for its display.
- A receiver MUST NOT need the exact nominal colours. It SHOULD measure the colours in each frame (§11).

> Informative: C4 is the recommended default. C8 carries 50 % more bits per cell, but it needs good focus and low moiré.

## 6 Frame header

### 6.1 Fields

The frame header has 70 bits. The fields are in this sequence:

| Bits | Field | Value |
|---|---|---|
| 4 | version | 1 |
| 2 | mode | 0, 1 or 2 (§5) |
| 9 | X | 40 … 511 |
| 9 | Y | 40 … 511 |
| 8 | n | codeword length, 24 … 255 |
| 7 | nsym / 2 | half the number of parity bytes |
| 8 | seq | frame sequence number, 0 … 255 |
| 7 | reserved | 0 |
| 16 | CRC | CRC-16/CCITT-FALSE of the first 54 bits |

Calculate the CRC over the first 54 bits as a bit sequence (Appendix A.3). Then append the 16 CRC bits, most significant bit first.

### 6.2 Masks

Each copy `c` (0 to 3) has a different mask. Before the sender writes copy `c`, it does an XOR of header bit `i` with:

```
m[c][i] = mix32(0x4C554D45 + i + 977 · c) AND 1        (32-bit arithmetic)
```

> Informative: copy B is copy A transposed. If a receiver reads a mirrored image with its axes swapped, it reads copy B where it expects copy A. The different masks make the CRC fail in that condition. The receiver then tries the correct orientation.

### 6.3 Validity

A receiver MUST reject a header if one of these conditions is true:

- the CRC is not correct,
- version is not 1,
- mode is 3,
- X < 40 or Y < 40,
- n < 24, or nsym < 2, or nsym ≥ n.

A sender MUST set the reserved bits to 0. A receiver MUST ignore the reserved bits.

### 6.4 Frame sequence number

The sender MUST increase `seq` by 1 (mod 256) for each new frame. The first value is not specified. A sender SHOULD use a random first value.

## 7 Codewords

### 7.1 Reed–Solomon code

Each codeword has n bytes. It has `k = n − nsym` message bytes and nsym parity bytes. nsym MUST be even.

- The field is GF(2^8) with the primitive polynomial x^8 + x^4 + x^3 + x^2 + 1 (0x11D). The primitive element α is 2.
- The generator polynomial is `g(x) = (x − α^0)(x − α^1) … (x − α^(nsym−1))`.
- The code is systematic. The codeword is the message followed by the remainder of `message(x) · x^nsym` divided by `g(x)`.
- Byte `i` of the codeword is the coefficient of `x^(n−1−i)`.

### 7.2 Codewords per frame

```
cpc = ceil(8 · n / bpc)        cells per codeword
m   = floor(D / cpc)           codewords per frame
```

A sender MUST select n so that `m ≥ 1`.

### 7.3 From codeword to cells

Codeword `j` (0 ≤ j < m) uses the slots `j · cpc` to `(j + 1) · cpc − 1`.

1. Make the bit sequence of the n codeword bytes (§2.3). Append zero bits to get `cpc · bpc` bits.
2. For `t = 0 … cpc − 1`, read bits `t · bpc` to `t · bpc + bpc − 1` as the integer `v`.
3. Put the value `v XOR (w[s] AND (2^bpc − 1))` in slot `s = j · cpc + t`.

The value of a data cell is a palette index (§5). The sender shows the palette colour of that index.

### 7.4 Filler

The slots from `m · cpc` to `D − 1` are filler. Calculate their values as follows (32-bit arithmetic):

```
r = mix32(seq + 0x01234567)
for s = m · cpc … D − 1:
    r = mix32(r + s)
    value[s] = r AND (2^bpc − 1)
```

A receiver MUST ignore filler slots.

## 8 Packets

### 8.1 Format

The message of each codeword is one packet of `k = n − nsym` bytes:

| Bytes | Field |
|---|---|
| 0–1 | transfer ID (TID) |
| 2 | bit 7: M (metadata flag); bits 6–0: SBN (source block number) |
| 3–4 | ESI (encoding symbol ID) |
| 5 … k − 3 | payload, `T = k − 7` bytes |
| k − 2 … k − 1 | CRC-16/CCITT-FALSE of bytes 0 … k − 3 |

- A sender MUST select n and nsym so that `T ≥ 16`.
- T MUST NOT change during a transfer.
- A receiver MUST discard a packet with a wrong CRC.

> Informative: Reed–Solomon decoding with erasures can accept a wrong codeword. The packet CRC prevents a wrong packet from damaging the fountain decoder.

### 8.2 Data packets

If M is 0, the packet is a data packet. The payload is fountain symbol ESI of source block SBN (§9).

### 8.3 Metadata packets

If M is 1, the packet is a metadata packet. SBN MUST be 0. ESI is the metadata index.

- Index 0 is the file descriptor (§8.4).
- Other indexes are reserved. A receiver MUST ignore them.

### 8.4 File descriptor

The file descriptor fills the T payload bytes of metadata packet 0:

| Bytes | Field |
|---|---|
| 0 | descriptor version = 1 |
| 1 | flags; bit 0 = payload is raw DEFLATE; other bits 0 |
| 2–5 | payload length L |
| 6–9 | file size |
| 10–13 | CRC-32 of the file (Appendix A.3) |
| 14 | name length N |
| 15 … 14 + N | file name, UTF-8 |
| after the name | TLV records (optional) |
| remainder | 0 |

- N MUST NOT be more than `min(255, T − 15)`. If the name is longer, the sender MUST cut it at a UTF-8 character boundary.
- A TLV record has a type byte, a length byte and `length` value bytes. Type 0 ends the list.
- Version 1 defines no TLV types. A receiver MUST ignore TLV records with unknown types.
- A receiver MUST ignore a descriptor with a version other than 1.

## 9 Payload, source blocks and fountain code

### 9.1 Payload

If flags bit 0 is 1, the payload is the file compressed with raw DEFLATE (RFC 1951). Otherwise the payload is the file. A sender MAY compress. A receiver MUST be able to decompress.

### 9.2 Source symbols and source blocks

```
Kt = max(1, ceil(L / T))           total source symbols
Z  = ceil(Kt / 16384)              number of source blocks; Z MUST be ≤ 128
KS = floor(Kt / Z)
ZL = Kt − KS · Z
K[b]     = KS + 1 if b < ZL, else KS
start[b] = K[0] + … + K[b − 1]
```

Global source symbol `i` is payload bytes `i · T` to `i · T + T − 1`. Use zero bytes after the end of the payload. Block `b` has the source symbols `start[b]` to `start[b] + K[b] − 1`.

### 9.3 Seed

Each source block has its own seed (32-bit arithmetic):

```
seed[b] = mix32(TID · 65537 + 0x51ED + b · 0x9E3779B9)
```

### 9.4 Parameters

For a block with K source symbols:

```
X  = the smallest integer ≥ 2 with X · (X − 1) ≥ 2 · K
S  = max(3, ceil(K / 100) + X); then increase S by 1 until K + S is a prime
W  = K + S
P  = 12 + bitlen(K)
P1 = the smallest prime > P
L  = W + P
```

### 9.5 Intermediate symbols

The encoder makes L intermediate symbols `C[0] … C[L − 1]` of T bytes each. `⊕` is the bytewise XOR.

1. For `i = 0 … K − 1`: `C[i]` = source symbol `i` of the block.
2. For `h = 0 … P − 1`: `C[W + h]` = ⊕ of `C[i]` for all `i` in `PI(h)`.
3. For `s = 0 … S − 1`: `C[K + s]` = ⊕ of `C[i]` for all `i` in `LDPC(s)`, ⊕ `C[W + (s mod P)]` ⊕ `C[W + ((s + 1) mod P)]`.

Do step 2 before step 3.

`PI(h)` is a pseudo-random half of the source symbols:

```
rng = Rng(mix32(seed XOR ((h + 1) · 0x5BD1E995)))
for i = 0, 32, 64, … while i < K:
    bits = rng.next()
    for t = 0 … min(32, K − i) − 1:
        if (bits >> t) AND 1 = 1: put i + t in PI(h)
```

`LDPC(s)` is the set of source symbols that this procedure gives to row s:

```
for i = 0 … K − 1:
    a = 1 + (floor(i / S) mod (S − 1))
    if (2 · a) mod S = 0: a = 1
    b = i mod S
    put i in LDPC(b), LDPC((b + a) mod S) and LDPC((b + 2 · a) mod S)
```

### 9.6 Encoding symbols

Encoding symbol ESI `x` is the ⊕ of `C[j]` for each `j` in the list LT, and of `C[W + p]` for each `p` in the list PI. Calculate the two lists as follows:

```
rng = Rng(mix32(seed XOR ((x + 1) · 0x9E3779B1)))
v = rng.next() AND 0xFFFFF
d = the smallest d ≥ 1 with v < DEG[d]
d = max(1, min(d, W − 2))
a = 1 + (rng.next() mod (W − 1))
b = rng.next() mod W
LT = [b]; then d − 1 times: b = (b + a) mod W; append b to LT
if d < 4: d1 = 2 + (rng.next() AND 1)   else: d1 = 2
a1 = 1 + (rng.next() mod (P1 − 1))
b1 = rng.next() mod P1
PI = []; d1 times:
    while b1 ≥ P: b1 = (b1 + a1) mod P1
    append b1 to PI
    b1 = (b1 + a1) mod P1
```

Do the `rng.next()` calls exactly in this sequence. The call for d1 occurs only if `d < 4`.

`DEG` is this table (index 0 to 30):

```
0, 5243, 529531, 704294, 791675, 844104, 879057, 904023, 922747, 937311, 948962,
958494, 966438, 973160, 978921, 983914, 988283, 992138, 995565, 998631, 1001391,
1003887, 1006157, 1008229, 1010129, 1011876, 1013490, 1014983, 1016370, 1017662, 1048576
```

> Informative: the code is non-systematic. A receiver can solve for the source block from any set of approximately K + 2 different symbols. The reference decoders use peeling with inactivation and then Gauss–Jordan elimination over GF(2).

## 10 Sender procedure

### 10.1 Start of a transfer

1. Make the payload (§9.1). Calculate the file CRC-32.
2. Select X, Y, the mode, n and nsym. Make sure that `m ≥ 1` and `T ≥ 16`.
3. Select a random 16-bit TID.
4. Calculate the source blocks (§9.2) and the intermediate symbols of each block (§9.5).
5. Make the file descriptor (§8.4).

### 10.2 Each frame

1. Make m packets.
2. Put the descriptor packet (M = 1, SBN = 0, ESI = 0) first if one of these conditions is true:
   - `Kt ≤ 2 · m`,
   - the frame counter is a multiple of 4.
3. Fill the other packets with data packets. Use a global packet counter `g` that starts at 0. Packet `g` is in block `b = g mod Z` and has `ESI = floor(g / Z) mod 65536`. Increase `g` by 1 for each data packet.
4. Add the packet CRC (§8.1). Encode each packet as a codeword (§7.1).
5. Make the cells: fixed cells, markers, the four header copies with the current seq (§6), the codewords (§7.3) and the filler (§7.4).
6. Show the frame. Increase seq and the frame counter.

The descriptor rule and the packet sequence in steps 2 and 3 are recommendations. A receiver MUST accept any sequence of packets.

### 10.3 Changes during a transfer

A sender MAY change X, Y, the mode, n and nsym between frames. T MUST stay the same, because a receiver identifies a transfer by TID and T. To change T, start a new transfer with a new TID.

### 10.4 Selection of n and nsym (informative)

The reference sender uses this procedure:

```
m0 = max(1, ceil(D / ceil(8 · 255 / bpc)))
n  = 255; while n > 24 and m0 · ceil(8 · n / bpc) > D: n = n − 1
nsym = max(4, 2 · floor((n · pct + 100) / 200))
```

`pct` is 10, 16, 22 or 30 for the ECC levels low, medium, high and very high. The default is 16.

### 10.5 Display (informative)

- Use an integer number of device pixels for each cell. Use 3 pixels or more.
- Put a quiet zone of at least 2 cells of ink around the grid.
- Rounded corners of the patterns and of the cells are permitted. A gap between cells is permitted. The gap MUST be ink. The colour of a cell MUST fill at least the centre half of the cell in each direction.
- Use a frame rate that the receiver can capture. The reference sender uses 12 frames per second by default.
- Select the cell size from the physical size of the screen. The reference sender uses approximately 0.75 mm for each cell. It also limits the short side to 52 … 150 cells and the aspect ratio to 1.78.

## 11 Receiver (informative)

A receiver can use any algorithm. The reference receivers use this procedure:

1. Find candidates for finders (1:1:3:1:1) and for the corner pattern (1:2:1:2:1) with a run-length scan.
2. Use any three of the four corners. Measure the cell size at each finder. Read a header copy. If the CRC is not correct, try the next copy, a different cell size, or the mirrored orientation.
3. Find the alignment markers step by step from the corners. Use a least-squares homography of all known points and correct small lens distortion with the residuals of near points.
4. Sample each data cell. Normalise its colour with the local ink and paper of the near markers.
5. Classify the colours with k-means. Start from the nominal palette. Correct local gain (moiré, uneven light) and classify again.
6. Decode each codeword. If decoding fails, try again with the least reliable bytes as erasures. Keep at least nsym / 2 bytes of redundancy.
7. Keep only packets with a correct CRC.

These rules are recommended for all receivers:

- Identify a transfer by (TID, T).
- Ignore duplicate packets. The key is (TID, T, SBN, ESI).
- Keep data packets that arrive before the descriptor.
- Decode each source block separately.
- Make sure that the size and the CRC-32 of the result are correct before you give the file to the user.
- Do not decode a frame again if all its codewords are known. The key is (seq, X, Y, mode, n, nsym).

## 12 Extensibility

- The version field in the header permits a new frame format.
- The reserved header bits permit new frame options in version 1. Receivers ignore them.
- Reserved metadata indexes and TLV types permit new file information.
- Descriptor flags bits 1 to 7 are reserved.
- A new palette needs a new mode value (mode 3 is reserved).

## 13 Conformance

- A conforming sender makes frames that agree bit by bit with the test vectors for the same inputs. The inputs are the file, the TID, the first seq, X, Y, the mode and the ECC level.
- A conforming receiver accepts every frame and packet sequence that a conforming sender can make.
- [test/vectors/test-vectors.json](test/vectors/test-vectors.json) contains values for: CRC-32, CRC-16, mix32, Rng, Reed–Solomon, grid layouts, header bits, codeword selection, fountain parameters, seeds, symbol lists, symbols, block plans, a packet, a descriptor and two complete frames.

---

## Appendix A — Algorithms

### A.1 mix32

```
mix32(x):                                    32-bit arithmetic
    x = (x XOR (x >> 16)) · 0x7FEB352D
    x = (x XOR (x >> 15)) · 0x846CA68B
    return x XOR (x >> 16)
```

Check values: `mix32(1) = 1753845952`, `mix32(0x51ED) = 1153879395`.

### A.2 Rng

```
Rng(seed): state = seed
next():    state = state + 0x9E3779B9        32-bit arithmetic
           return mix32(state)
```

### A.3 Checksums

| Name | Polynomial | Init | Reflect | XOR out | Check ("123456789") |
|---|---|---|---|---|---|
| CRC-32 | 0x04C11DB7 | 0xFFFFFFFF | yes | 0xFFFFFFFF | 0xCBF43926 |
| CRC-16/CCITT-FALSE | 0x1021 | 0xFFFF | no | 0x0000 | 0x29B1 |

For the header CRC, process one bit at a time:

```
crc = 0xFFFF
for each bit:
    top = ((crc >> 15) AND 1) XOR bit
    crc = (crc << 1) AND 0xFFFF
    if top = 1: crc = crc XOR 0x1021
```

### A.4 Hilbert curve

```
hilbert(N, d):                    N is a power of two
    x = 0; y = 0; t = d; s = 1
    while s < N:
        rx = 1 AND (t >> 1)
        ry = 1 AND (t XOR rx)
        if ry = 0:
            if rx = 1: x = s − 1 − x; y = s − 1 − y
            swap x and y
        x = x + s · rx
        y = y + s · ry
        t = t >> 2
        s = s · 2
    return (x, y)
```

## Appendix B — Design notes (informative)

- **Solid colour cells.** Each cell has one colour. This keeps decoding simple and tolerant of blur. The palettes keep a large distance between colours in camera RGB.
- **Light patterns on ink.** The frame is mostly dark. This is comfortable to look at and keeps camera exposure stable.
- **Adaptive grid.** The sender selects the grid from the physical screen size. A phone screen gets fewer, larger cells, so that a camera at its focus distance can resolve them.
- **Any three of four corners.** Glare, blur or the edge of the camera image can hide one corner. The receiver continues with the other three.
- **Alignment lattice.** Markers every 16 to 18 cells correct perspective and lens distortion locally.
- **Compact codewords.** The Hilbert order puts each codeword in one area. Rolling-shutter tears and local defects then remove only some codewords.
- **Fountain code.** Every packet is new information. The receiver needs no specific frame and no return channel.
- **Packet CRC.** It stops wrong packets that Reed–Solomon erasure decoding can accept.
- **Source blocks.** Each block has at most 16384 symbols. This limits decoder memory and time on small devices.
