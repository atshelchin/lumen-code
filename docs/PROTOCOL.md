# Lumen Code v1 — Protocol Specification

| Item | Value |
|---|---|
| Protocol | Lumen Code, version 1 |
| Status | Draft 2 for public review |
| Date | 2026-10-20 |
| Language | ASD-STE100 Simplified Technical English |
| Chinese version | [PROTOCOL.zh-CN.md](PROTOCOL.zh-CN.md) |
| Test vectors | [test/vectors/test-vectors.json](../test/vectors/test-vectors.json) |
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
- the requirements for senders (§10) and receivers (§11),
- the errors (§12), the security properties (§13), the extension rules (§14) and conformance (§15).

A receiver can use any algorithm to find and sample the grid in a camera image. Section 11.6 describes the algorithm of the reference receivers. It is informative.

The protocol does not give confidentiality or authentication (§13).

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
| discard | Remove from memory and do not use. A discarded item has no other effect. |

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

A receiver does these steps in the opposite direction. One complete packet is enough to use the data in it. The receiver can reconstruct a source block when it has slightly more than K different packets of that block (Appendix C.2).

Errors are found at four levels. Each level discards what it cannot use:

| Level | Check | Section |
|---|---|---|
| frame header | four copies, CRC-16, validity rules | §6.3 |
| codeword | Reed–Solomon | §7 |
| packet | CRC-16, validity rules | §8 |
| file | descriptor validity, size and CRC-32 of the result | §8.4, §11.3 |

## 4 Grid

### 4.1 Dimensions and coordinates

The grid has X columns and Y rows. X and Y MUST be in the range 40 to 511.

The cell `(x, y)` is in column x and row y. The cell `(0, 0)` is at the top-left corner. The cell index is `i = y · X + x`.

Each cell has one kind: fixed, header, marker or data. Sections 4.2 to 4.5 assign the kinds in this sequence. Cells that do not get a kind are data cells. For all X and Y in the range, the assignments do not overlap.

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

> Informative: the Hilbert curve keeps each codeword in one compact area. A torn frame (the camera reads half of the old frame and half of the new frame) then loses only the codewords on the tear line. Slot 0 is at the top-left corner.

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
- Mode 3 is reserved (§14).
- In mode C8, the bits of the index are (R, G, B). One wrong colour channel then gives one wrong bit.
- A sender SHOULD use the nominal colours. A sender MAY change the colours a small quantity for its display.
- A receiver MUST NOT need the exact nominal colours. It SHOULD measure the colours in each frame (§11.6).

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
| 8 | n | codeword length in bytes |
| 7 | nsym / 2 | half the number of parity bytes |
| 8 | seq | frame sequence number, 0 … 255 |
| 7 | reserved | 0 |
| 16 | CRC | CRC-16/CCITT-FALSE of the first 54 bits |

Calculate the CRC over the first 54 bits as a bit sequence (Appendix A.3). Then append the 16 CRC bits, most significant bit first.

These values follow from the fields:

```
bpc = bits per cell of the mode (§5)
k   = n − nsym                   packet length in bytes (§8)
T   = k − 7                      symbol length in bytes (§8)
cpc = ceil(8 · n / bpc)          cells per codeword (§7.2)
m   = floor(D / cpc)             codewords per frame; D from the grid X × Y (§4.6)
```

### 6.2 Masks

Each copy `c` (0 to 3) has a different mask. Before the sender writes copy `c`, it does an XOR of header bit `i` with:

```
m[c][i] = mix32(0x4C554D45 + i + 977 · c) AND 1        (32-bit arithmetic)
```

> Informative: copy B is copy A transposed. If a receiver reads a mirrored image with its axes swapped, it reads copy B where it expects copy A. The different masks make the CRC fail in that condition. The receiver then tries the correct orientation.

### 6.3 Validity

A header is valid only if all of these conditions are true:

- the CRC is correct,
- version is 1,
- mode is 0, 1 or 2,
- X ≥ 40 and Y ≥ 40,
- nsym ≥ 2,
- T ≥ 16, that is, `n − nsym ≥ 23`,
- m ≥ 1.

A sender MUST make only valid headers. A receiver MUST discard a frame that has no valid header, except as §11.1 permits. A sender MUST set the reserved bits to 0. A receiver MUST ignore the reserved bits.

> Informative: from the rules, n is in 25 … 255 and nsym is an even number in 2 … 232.

### 6.4 Frame sequence number

The sender MUST increase `seq` by 1 (mod 256) for each new frame. The first value is not specified. A sender SHOULD use a random first value.

> Informative: seq identifies a frame only for a short time. It repeats every 256 frames, which is 4.3 s at 60 frames per second. Section 11.5 gives the rule for receivers.

## 7 Codewords

### 7.1 Reed–Solomon code

Each codeword has n bytes. It has `k = n − nsym` message bytes and nsym parity bytes.

- The field is GF(2^8) with the primitive polynomial x^8 + x^4 + x^3 + x^2 + 1 (0x11D). The primitive element α is 2.
- The generator polynomial is `g(x) = (x − α^0)(x − α^1) … (x − α^(nsym−1))`.
- The code is systematic. The codeword is the message followed by the remainder of `message(x) · x^nsym` divided by `g(x)`.
- Byte `i` of the codeword is the coefficient of `x^(n−1−i)`.

### 7.2 Codewords per frame

```
cpc = ceil(8 · n / bpc)        cells per codeword
m   = floor(D / cpc)           codewords per frame
```

Each frame carries exactly m codewords.

### 7.3 From codeword to cells

Codeword `j` (0 ≤ j < m) uses the slots `j · cpc` to `(j + 1) · cpc − 1`.

1. Make the bit sequence of the n codeword bytes (§2.3). Append zero bits to get `cpc · bpc` bits.
2. For `t = 0 … cpc − 1`, read bits `t · bpc` to `t · bpc + bpc − 1` as the integer `v`.
3. Put the value `v XOR (w[s] AND (2^bpc − 1))` in slot `s = j · cpc + t`.

The value of a data cell is a palette index (§5). The sender shows the palette colour of that index.

### 7.4 Filler

The slots from `m · cpc` to `D − 1` are filler. The sender MUST calculate their values as follows (32-bit arithmetic):

```
r = mix32(seq + 0x01234567)
for s = m · cpc … D − 1:
    r = mix32(r + s)
    value[s] = r AND (2^bpc − 1)
```

A receiver MUST ignore filler slots.

> Informative: the filler makes the frame a function of its inputs only, so that test vectors can check frames bit by bit. Random filler also keeps the colour statistics of the frame uniform.

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

- T is at least 16 (§6.3).
- T MUST NOT change during a transfer. A receiver identifies a transfer by the pair (TID, T).
- A receiver MUST discard a packet with a wrong CRC.

> Informative: Reed–Solomon decoding with erasures can accept a wrong codeword. The packet CRC stops almost all of these (Appendix C.1). The file check (§11.3) stops the rest.

### 8.2 Data packets

If M is 0, the packet is a data packet. The payload is fountain symbol ESI of source block SBN (§9). A sender MUST use only `SBN < Z` (§9.2).

### 8.3 Metadata packets

If M is 1, the packet is a metadata packet. ESI is the metadata index. The sender MUST set SBN to 0. A receiver MUST discard a metadata packet with SBN ≠ 0.

- Index 0 is the file descriptor (§8.4).
- Other indexes are reserved (§14). A receiver MUST discard metadata packets with other indexes.

### 8.4 File descriptor

The file descriptor fills the T payload bytes of metadata packet 0:

| Bytes | Field |
|---|---|
| 0 | descriptor version = 1 |
| 1 | flags; bit 0 = payload is raw DEFLATE; bits 1–7 = 0 |
| 2–5 | payload length L |
| 6–9 | file size F |
| 10–13 | CRC-32 of the file (Appendix A.3) |
| 14 | name length N |
| 15 … 14 + N | file name, UTF-8 |
| after the name | list of TLV records |
| remainder | 0 |

**The name.** The name is for display and for a suggested file name. N can be 0. N MUST NOT be more than `min(255, T − 15)`. If the name is longer, the sender MUST cut it at a UTF-8 character boundary.

**The TLV list.** A TLV record has a type byte, a length byte `ℓ` and `ℓ` value bytes. Parse the list from the byte after the name:

```
p = 15 + N
while p < T:
    type = byte[p]
    if type = 0: stop                          end of the list
    if p + 2 > T: the descriptor is malformed
    ℓ = byte[p + 1]
    if p + 2 + ℓ > T: the descriptor is malformed
    the record is (type, bytes p + 2 … p + 1 + ℓ)
    p = p + 2 + ℓ
```

Version 1 defines no TLV types. A receiver MUST ignore records with unknown types. The sender MUST fill the bytes after the list with 0. A receiver MUST ignore the bytes after the end of the list.

**Validity.** A descriptor is valid only if all of these conditions are true:

- the descriptor version is 1,
- flags bits 1–7 are 0,
- `N ≤ T − 15`,
- the name is valid UTF-8,
- the TLV list is not malformed,
- if flags bit 0 is 0, then `L = F`,
- the number of source blocks Z (§9.2) is not more than 128.

A receiver MUST discard a descriptor that is not valid. The descriptor MUST NOT change during a transfer.

## 9 Payload, source blocks and fountain code

### 9.1 Payload

If flags bit 0 is 1, the payload is the file compressed with raw DEFLATE (RFC 1951). Otherwise the payload is the file. A sender MAY compress. A receiver MUST be able to decompress.

A compressed payload is exactly one raw DEFLATE stream. It ends with the block that has BFINAL = 1, and no byte follows that block. The unused bits of the last byte can have any value.

### 9.2 Source symbols and source blocks

```
Kt = max(1, ceil(L / T))           total source symbols
Z  = ceil(Kt / 16384)              number of source blocks
KS = floor(Kt / Z)
ZL = Kt − KS · Z
K[b]     = KS + 1 if b < ZL, else KS
start[b] = K[0] + … + K[b − 1]
```

Global source symbol `i` is payload bytes `i · T` to `i · T + T − 1`. Use zero bytes after the end of the payload. Block `b` has the source symbols `start[b]` to `start[b] + K[b] − 1`.

Z MUST NOT be more than 128. Thus L is at most `2^21 · T` bytes.

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

### 9.7 Decoding

Each received symbol ESI `x` of a block gives one linear equation over GF(2) in the intermediate symbols: the ⊕ of the symbols in LT and PI of `x` is the received symbol. The S rows of step 3 and the P rows of step 2 in §9.5 give `S + P` more equations with the value zero. The block is decodable when this set of equations determines `C[0] … C[K − 1]`. A receiver MAY use any algorithm that solves the set.

> Informative: whether a set of ESIs makes a block decodable depends only on the set, not on the algorithm. The test vectors use this property. The code is not systematic. The reference decoders use peeling with inactivation and then Gauss–Jordan elimination. Appendix C.2 gives the measured number of extra symbols.

## 10 Sender

### 10.1 Requirements

A sender MUST:

1. Make the payload (§9.1), the file CRC-32 and a valid descriptor (§8.4).
2. Select a new TID for each transfer. A sender SHOULD select it at random with uniform distribution.
3. Make only valid headers (§6.3). Keep T and the descriptor the same during the transfer.
4. Put exactly m packets in each frame (§7.2). Each packet is the descriptor or a data packet of the transfer, with a correct CRC (§8).
5. Send the descriptor again and again during the transfer. A receiver can start at any time and needs the descriptor.
6. Increase seq for each frame (§6.4). Make the fixed cells, the markers, the four header copies, the codewords and the filler as §4 to §7 specify.

A sender MAY change X, Y, the mode, n and nsym between frames, but not T (§8.1). To change T, start a new transfer with a new TID.

The sequence of the packets, the positions of the descriptor and the selection of the parameters are the decision of the sender. A receiver MUST accept any sequence of valid packets.

### 10.2 Reference packet schedule (informative)

The reference sender uses this schedule. It is the schedule of the `referenceSender` test vectors.

1. Put the descriptor in a frame if one of these conditions is true:
   - `Kt ≤ 2 · m`,
   - the frame counter (0 for the first frame) is a multiple of 4.
2. The descriptor goes in codeword `mix32(c) mod m`, where `c` is the number of descriptors sent before (0 for the first).
3. Fill all other codewords with data packets. Use a global packet counter `g` that starts at 0. Packet `g` is in block `b = g mod Z` and has `ESI = floor(g / Z) mod 65536`. Increase `g` by 1 for each data packet.

> Informative: the position of the descriptor changes because a codeword is always in the same area of the frame. A fixed position puts the descriptor in a corner (slot 0 is at the top-left corner). If glare or the edge of the camera image hides that corner, the receiver gets data but never the descriptor.

### 10.3 Selection of n and nsym (informative)

The reference sender uses this procedure:

```
m0 = max(1, ceil(D / ceil(8 · 255 / bpc)))
n  = 255; while n > 24 and m0 · ceil(8 · n / bpc) > D: n = n − 1
nsym = max(4, 2 · floor((n · pct + 100) / 200))
```

`pct` is 10, 16, 22 or 30 for the ECC levels low, medium, high and very high. The default is 16.

### 10.4 Display (informative)

- Use an integer number of device pixels for each cell. Use 3 pixels or more.
- Put a quiet zone of at least 2 cells of ink around the grid.
- Rounded corners of the patterns and of the cells are permitted. A gap between cells is permitted. The gap MUST be ink. The colour of a cell MUST fill at least the centre half of the cell in each direction.
- Use a frame rate that the receiver can capture. The reference sender uses 12 frames per second by default and permits up to 60.
- Select the cell size from the physical size of the screen. The reference sender uses approximately 0.75 mm for each cell. It also limits the short side to 52 … 150 cells. The grid fills the display area, and the aspect ratio is not more than 2.4.

## 11 Receiver

### 11.1 Frames and packets

For each camera image, a receiver finds the grid, reads a header copy, samples the data cells and decodes the codewords (§11.6 gives one method). Then:

1. If no header copy is valid, the receiver MAY decode the frame with the last valid header that it read, if the grid that it found agrees with X and Y of that header. It MUST NOT use that header for the frame key of §11.5.
2. The receiver uses only codewords that Reed–Solomon decoding accepts, and only packets with a correct CRC (§8.1).
3. The receiver gives each packet to the transfer state machine of §11.2, in any sequence.

> Informative: X, Y, the mode, n and nsym usually do not change during a transfer, and seq is not necessary to decode the data. A wrong reused header does not cause wrong data: the alignment markers, the codewords and the packet CRCs find it.

### 11.2 Transfer state machine

A receiver keeps a state for each transfer key (TID, T). It identifies a descriptor by all its T bytes. Two descriptors are "different" if one of their bytes is different.

The states are:

| State | Meaning |
|---|---|
| NEW | The receiver has no state for this key. |
| COLLECTING | No descriptor is accepted. The receiver keeps data packets for later. |
| RECEIVING | A descriptor is accepted. Each source block has a decoder. |
| REFUSED | A descriptor is accepted, but the transfer is more than a limit of the receiver (§11.4). |
| COMPLETE | The file is delivered. |

The receiver MUST do these actions. "Accept D" is: store D; calculate the source blocks (§9.2); if the transfer is more than a limit of the receiver, go to REFUSED; otherwise make one decoder for each block, give it the kept data packets of its block, discard the kept data packets with `SBN ≥ Z`, and go to RECEIVING. "Reset" is: discard all state of the key.

| State | Data packet | Valid descriptor, same as stored | Valid descriptor, different or none stored |
|---|---|---|---|
| NEW | go to COLLECTING, then as COLLECTING | — | accept D |
| COLLECTING | keep it, unless a packet with the same (SBN, ESI) is kept or the limit of §11.4 is reached | — | accept D |
| RECEIVING | if `SBN < Z`, the block is not decoded and the ESI is new for the block: give it to the decoder of the block; otherwise discard it | discard it | reset, then accept D |
| REFUSED | discard it | discard it | reset, then accept D |
| COMPLETE | discard it | discard it | reset, then accept D |

These rules apply in all states:

- A receiver MUST discard a descriptor that is not valid (§8.4), and any other metadata packet (§8.3). They do not change the state.
- In RECEIVING, the receiver tries to decode a block when it has at least K different symbols of the block (§9.7). If the set is not decodable, it continues to collect symbols.
- When all blocks are decoded, the receiver does the file check (§11.3).

> Informative: a "different descriptor" for a known key occurs when two transfers have the same TID and T (probability 2^−16 for random TIDs), or when a wrong descriptor passed the CRC. In both cases the newest descriptor is the best guess of what the screen shows now.

### 11.3 File check

When all blocks are decoded:

1. Join the blocks and keep the first L bytes. This is the payload.
2. If flags bit 0 is 1, decompress the payload. Stop with a failure if the output becomes longer than F bytes, or if the payload is not exactly one correct raw DEFLATE stream (§9.1).
3. The check is a success if the result has F bytes and its CRC-32 is the CRC-32 of the descriptor.

If the check is a success, the receiver delivers the file and goes to COMPLETE. It MUST NOT deliver a file that does not pass the check.

If the check fails, the receiver MUST reset the key (§11.2): it discards the descriptor, the decoders and the decoded blocks. It SHOULD tell the user, and it continues to receive. The transfer then starts again from COLLECTING.

> Informative: a check failure means that one wrong packet passed all checks (Appendix C.1), or that two transfers with the same key were mixed. The receiver cannot find the wrong packet, because the fountain decoder mixes all symbols. Thus it starts again. The descriptor is also discarded, because the wrong packet can be the descriptor.

### 11.4 Limits

A sender can be malicious (§13). A receiver MUST limit the memory and time that one sender can make it use:

- **Size.** A receiver MUST have a maximum size. If L or F of an accepted descriptor is more than this maximum, the receiver goes to REFUSED and SHOULD tell the user. It MUST NOT allocate memory in proportion to L or F before this check.
- **Kept packets.** A receiver MUST limit the number of data packets that it keeps in COLLECTING. When it reaches the limit, it discards new data packets.
- **Transfers.** A receiver MUST limit the number of keys that it keeps. To make space, it resets the key that received a packet least recently.
- **Decompression.** The output of DEFLATE MUST NOT be more than F bytes (§11.3).
- **Decoding.** A receiver SHOULD NOT try to decode a block again until it has more symbols of that block than at the last failed try.

The reference receivers use these values: maximum size 256 MiB, 4096 kept packets for each key, 4 keys.

> Informative: a receiver MAY reset a key at any time, for example when no packet has arrived for some time, or to free memory. The fountain code makes this safe: the sender never needs to know.

### 11.5 Frame keys

A receiver MAY skip the codewords of a frame that it decoded before. It identifies the frame by the frame key (seq, X, Y, mode, n, nsym).

Because seq repeats every 256 frames, a receiver MUST NOT use a frame key more than 1 s after it first recorded it. This rule is safe for senders up to 255 frames per second.

### 11.6 Reference scan procedure (informative)

A receiver can use any algorithm. The reference receivers use this procedure:

1. Find candidates for finders (1:1:3:1:1) and for the corner pattern (1:2:1:2:1) with a run-length scan.
2. Use any three of the four corners. Measure the cell size at each finder. Read a header copy. If the header is not valid, try the next copy, a different cell size, or the mirrored orientation.
3. Find the alignment markers step by step from the corners. Use a least-squares homography of all known points and correct small lens distortion with the residuals of near points.
4. Sample each data cell. Normalise its colour with the local ink and paper of the near markers.
5. Classify the colours with k-means. Start from the nominal palette. Correct local gain (moiré, uneven light) and classify again.
6. Decode each codeword. If decoding fails, try again with the least reliable bytes as erasures. Keep at least nsym / 2 bytes of redundancy.
7. If no header copy is valid but the grid is found, use the last valid header for 3 more tries (§11.1).

## 12 Errors

The channel is one way. No error goes back to the sender. Each error has a local effect only:

| Error | Action of the receiver | Recovery |
|---|---|---|
| No valid header in a frame | Discard the frame (or use the last header, §11.1). | Next frame. |
| Reed–Solomon decoding fails | Discard the codeword. | Fountain code: other packets replace it. |
| Packet CRC is wrong | Discard the packet. | As above. |
| Metadata packet with SBN ≠ 0, or index ≠ 0 | Discard the packet. | Not necessary. |
| Descriptor not valid | Discard the descriptor. | The sender sends it again (§10.1). |
| Data packet with `SBN ≥ Z` | Discard the packet. | Not necessary. |
| Block not decodable yet | Continue to collect. | More symbols. |
| Transfer more than a limit | Go to REFUSED. Tell the user. | None for this receiver. |
| File check fails | Reset the key. Tell the user. | Automatic: the transfer starts again. |

## 13 Security considerations

**Model.** Any person who can put an image in front of the camera can send frames to the receiver. Any person who can see the screen, or a recording of it, can read the file.

**No confidentiality.** The frames are not encrypted. Do not show secret data where other persons or cameras can see the screen. An application that needs confidentiality MUST encrypt the file before it gives it to the sender.

**No authentication.** The CRCs find accidental errors only. They do not show who made a frame, and an attacker can calculate them. An application that needs to know the origin of a file MUST sign or authenticate the file itself, outside this protocol.

**Interference.** A second screen in front of the camera can send packets with the same key. Section 11.2 then resets the transfer. This can stop a transfer, but it cannot make the receiver deliver a file that does not pass the check of §11.3. An attacker that can show frames can also show its own complete file. The receiver cannot know which screen the user wants.

**Resource exhaustion.** A malicious descriptor can ask for 2^21 · T bytes, and a small DEFLATE payload can expand by a factor of more than 1000. The limits of §11.4 are mandatory for this reason.

**The file name.** The name comes from the sender and is not trusted. A receiver MUST NOT use it as a path. A receiver that stores the file MUST remove path separators and the names `.` and `..`, and SHOULD remove control characters and characters that the file system does not accept. A receiver SHOULD show the name without control characters and bidirectional-override characters.

**The file content.** The content comes from the sender and is not trusted. A receiver MUST treat it as a file from an unknown source. For example, a text viewer must escape markup, and links must not run scripts.

**Residual errors.** Appendix C.1 gives the probability that a wrong packet passes the checks. A wrong file passes the file check only if, in addition, its CRC-32 and size are correct.

## 14 Extensibility and versions

The channel is one way, so there is no negotiation. The sender selects the formats. A receiver that does not know a format discards it. These are the only extension points:

| Point | Rule for a receiver of this version | Use |
|---|---|---|
| Header version (4 bits) | Version ≠ 1: discard the header. | A new frame format. |
| Header reserved bits (7) | Ignore them. | Frame options that receivers of this version can ignore. |
| Mode 3 | Discard the header. | A new palette. |
| Metadata indexes 1 … 65535 | Discard the packet. | More information about the transfer. |
| Descriptor version | Version ≠ 1: discard the descriptor. | A new descriptor format. |
| Descriptor flags bits 1–7 | Bit set: discard the descriptor. | Changes to the meaning of the payload, for example encryption. |
| TLV types 1 … 255 | Unknown type: ignore the record. | Optional information about the file. |

A flag bit is "critical": a receiver that does not know it cannot use the transfer. A TLV record is "not critical": a receiver that does not know it can still use the transfer.

## 15 Conformance

**Encoder.** For the same inputs, a conforming encoder MUST make the same bytes and cells as the test vectors:

- frame cells from (X, Y, mode, n, nsym, seq, packets),
- packets from (TID, M, SBN, ESI, payload),
- descriptors from (flags, L, F, CRC-32, name, T),
- fountain symbols from (block data, T, TID, SBN, ESI),
- the checksums, the hash, the generator, Reed–Solomon codewords, grid layouts and block plans.

**Sender.** A conforming sender MUST obey §10.1. The schedule and the parameter selection are free. The `referenceSender` vectors show the reference choices (§10.2, §10.3); they are informative.

**Receiver.** A conforming receiver MUST:

- accept every valid header, packet and descriptor of the vectors, and discard every one that the vectors mark as not valid (§6.3, §8). Each vector that is not valid has a correct CRC and one defect, which the vector names,
- correct the Reed–Solomon vectors that have `2 · errors + erasures ≤ nsym`,
- decode the fountain vectors that are marked decodable, and only those (§9.7),
- give the results of the `streams` vectors, which are packet sequences with the results that §11.2 and §11.3 specify,
- obey §11.4, §11.5 and §13.

[test/vectors/test-vectors.json](../test/vectors/test-vectors.json) contains all vectors. The reference implementations check it in `npm test` (JavaScript) and `cargo test` (Rust).

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
- **Strict parsing.** Receivers discard input that is not valid, instead of a guess. Extensions use only the points of §14, each with a clear rule.
- **Restart instead of repair.** A failed file check resets the transfer (§11.3). This is simple, it is always correct, and the event is very rare.

## Appendix C — Residual errors and fountain overhead (informative)

### C.1 Wrong packets

To be completed with measured values.

### C.2 Fountain overhead

To be completed with measured values.

## Appendix D — Changes from draft 1 (informative)

Draft 1 was dated 2026-10-17. Draft 2 changes no bit on the wire: frames, packets, descriptors and fountain symbols are the same. Frames of a draft-1 sender are valid for a draft-2 receiver.

- §6.3: a header is valid only if T ≥ 16 and m ≥ 1. Draft 1 required these from senders only.
- §8.3, §8.4: receivers discard metadata packets with SBN ≠ 0. The descriptor has complete validity rules: flags bits 1–7 are critical, the name must fit and be UTF-8, the TLV list must be correct, and `L = F` without compression.
- §9.7: decodability is defined as a property of the set of symbols.
- §10: sender requirements are separate from the reference schedule. The reference sender moves the descriptor between codewords (§10.2).
- §11: new normative receiver rules: the transfer state machine, the file check with reset on failure, limits, and the lifetime of frame keys.
- §12–§14: errors, security considerations and extension rules are new.
- §15: an encoder must match the vectors for the same packets. The packet schedule is not part of conformance.
