// Grid layout shared by encoder and decoder.
//
//  ┌────────┬──────────┬───────────────┬──────────┬────────┐
//  │ finder │ header A │  data …       │ header C │ finder │   finders: 7×7, light-dark-light(3)-dark-light
//  ├────────┘          ·  marker (3×3) ·          └────────┤   markers: lattice of 3×3 (light centre, dark ring)
//  │ header B   · · ·  ·   · · ·       ·    · · ·          │   header: 70 bits, 4 copies (A/B near TL, C near TR, D near BL)
//  │ …                 data, Hilbert order                 │   data: Hilbert-curve order so each codeword is a compact blob
//  │ header D                                              │
//  ├────────┐                                    ┌─────────┤
//  │ finder │                                    │ corner  │   corner: 7×7, light ring, 2-wide dark ring, light dot
//  └────────┴────────────────────────────────────┴─────────┘

import { crc16bits, mix32 } from './util.js';

export const VERSION = 1;
export const MIN_DIM = 40;
export const MAX_DIM = 511;
// Smallest packet: 7 bytes of packet header and CRC plus T >= 16 symbol bytes (§6.3, §8.1).
export const MIN_T = 16;
export const MIN_K = MIN_T + 7;

// cell kinds
export const K_DATA = 0;
export const K_FIXED = 1;
export const K_MARKER = 2;
export const K_HEADER = 3;

// fixed colours (indices into render table after the data palette)
export const INK = [10, 13, 20];
export const PAPER = [242, 244, 248];

export const MODES = [
  { id: 0, name: 'C2', bpc: 1, palette: [INK, PAPER] },
  {
    id: 1,
    name: 'C4',
    bpc: 2,
    palette: [INK, [0, 216, 255], [255, 40, 150], [255, 214, 0]],
  },
  {
    id: 2,
    name: 'C8',
    bpc: 3,
    // index bits = (R, G, B) so a one-channel confusion is a one-bit error
    palette: [
      INK,
      [44, 80, 255],
      [26, 222, 96],
      [0, 220, 255],
      [255, 52, 76],
      [250, 52, 220],
      [255, 214, 10],
      PAPER,
    ],
  },
];

// parity share per ECC level, in percent of the codeword length (encoder choice)
export const ECC_PERCENT = [10, 16, 22, 30];
export const ECC_LEVELS = ECC_PERCENT.map((p) => p / 100);

export const HEADER_BITS = 70;
const HEADER_W = 10;
const HEADER_H = 7;
// Each copy has its own mask: a transposed (mirrored) read of one copy lands on another
// copy's cells, and must fail rather than decode with swapped axes.
export const HEADER_MASKS = [0, 1, 2, 3].map((c) => {
  const m = new Uint8Array(HEADER_BITS);
  for (let i = 0; i < HEADER_BITS; i++) m[i] = mix32(0x4c554d45 + i + c * 977) & 1;
  return m;
});

// Marker lattice along one axis of D cells (§4.5; integer arithmetic, round half up).
export function latticeAxis(D) {
  let segs = Math.max(1, Math.floor((D - 7 + 9) / 18));
  while (segs > 1 && D - 7 < 16 * segs) segs--;
  const out = [];
  for (let i = 0; i <= segs; i++) out.push(3 + Math.floor((2 * i * (D - 7) + segs) / (2 * segs)));
  return out;
}

// Number of data cells of an X × Y grid without building the layout: all cells minus
// 4 × 49 pattern cells, 4 × 15 separator cells, 4 × 70 header cells and 9 cells for each
// of the nx · ny − 4 markers.
export function dataCellCount(X, Y) {
  return X * Y - 500 - 9 * latticeAxis(X).length * latticeAxis(Y).length;
}

function hilbertD2XY(order, d) {
  let x = 0;
  let y = 0;
  let t = d;
  for (let s = 1; s < order; s *= 2) {
    const rx = 1 & (t >>> 1);
    const ry = 1 & (t ^ rx);
    if (ry === 0) {
      if (rx === 1) {
        x = s - 1 - x;
        y = s - 1 - y;
      }
      const tmp = x;
      x = y;
      y = tmp;
    }
    x += s * rx;
    y += s * ry;
    t >>>= 2;
  }
  return [x, y];
}

// A few recent layouts (a large one is about 2 MB).
const layoutCache = new Map();
const LAYOUT_CACHE = 8;

export function makeLayout(X, Y) {
  const key = X * 1024 + Y;
  const hit = layoutCache.get(key);
  if (hit) {
    layoutCache.delete(key); // most recently used last
    layoutCache.set(key, hit);
    return hit;
  }
  if (X < MIN_DIM || Y < MIN_DIM || X > MAX_DIM || Y > MAX_DIM) throw new Error('bad dims');
  const N = X * Y;
  const kind = new Uint8Array(N);
  const fixed = new Uint8Array(N); // 0 ink, 1 paper (for non-data cells)
  const set = (x, y, k, light) => {
    kind[y * X + x] = k;
    fixed[y * X + x] = light ? 1 : 0;
  };

  // finders + separators
  const finder = (ox, oy) => {
    for (let y = 0; y < 7; y++)
      for (let x = 0; x < 7; x++) {
        const d = Math.max(Math.abs(x - 3), Math.abs(y - 3));
        set(ox + x, oy + y, K_FIXED, d === 3 || d <= 1);
      }
  };
  const corner = (ox, oy) => {
    for (let y = 0; y < 7; y++)
      for (let x = 0; x < 7; x++) {
        const d = Math.max(Math.abs(x - 3), Math.abs(y - 3));
        set(ox + x, oy + y, K_FIXED, d === 3 || d === 0);
      }
  };
  finder(0, 0);
  finder(X - 7, 0);
  finder(0, Y - 7);
  corner(X - 7, Y - 7);
  for (let i = 0; i <= 7; i++) {
    set(7, i, K_FIXED, false);
    set(i, 7, K_FIXED, false);
    set(X - 8, i, K_FIXED, false);
    set(X - 1 - i, 7, K_FIXED, false);
    set(7, Y - 1 - i, K_FIXED, false);
    set(i, Y - 8, K_FIXED, false);
    set(X - 8, Y - 1 - i, K_FIXED, false);
    set(X - 1 - i, Y - 8, K_FIXED, false);
  }

  // header copies: list of cell indices in bit order
  const headers = [[], [], [], []];
  for (let r = 0; r < HEADER_H; r++)
    for (let c = 0; c < HEADER_W; c++) {
      headers[0].push(r * X + (8 + c));
      headers[1].push((8 + c) * X + r);
      headers[2].push(r * X + (X - 18 + c));
      headers[3].push((Y - 18 + c) * X + r);
    }
  for (const h of headers) for (const i of h) kind[i] = K_HEADER;

  // alignment lattice
  const lx = latticeAxis(X);
  const ly = latticeAxis(Y);
  const nx = lx.length;
  const ny = ly.length;
  for (let j = 0; j < ny; j++)
    for (let i = 0; i < nx; i++) {
      const isCorner = (i === 0 || i === nx - 1) && (j === 0 || j === ny - 1);
      if (isCorner) continue;
      for (let dy = -1; dy <= 1; dy++)
        for (let dx = -1; dx <= 1; dx++) set(lx[i] + dx, ly[j] + dy, K_MARKER, dx === 0 && dy === 0);
    }

  // data order along a Hilbert curve
  let order = 1;
  while (order < Math.max(X, Y)) order *= 2;
  const data = [];
  for (let d = 0; d < order * order; d++) {
    const [x, y] = hilbertD2XY(order, d);
    if (x < X && y < Y && kind[y * X + x] === K_DATA) data.push(y * X + x);
  }
  const dataOrder = Int32Array.from(data);

  // whitening mask per data slot
  const whiten = new Uint8Array(dataOrder.length);
  for (let i = 0; i < whiten.length; i++) whiten[i] = mix32(0x9e3779b9 ^ Math.imul(i + 1, 0x85ebca6b)) & 7;

  const L = { X, Y, kind, fixed, headers, lx, ly, dataOrder, whiten, dataCells: dataOrder.length };
  layoutCache.set(key, L);
  if (layoutCache.size > LAYOUT_CACHE) layoutCache.delete(layoutCache.keys().next().value);
  return L;
}

export function cellsPerCodeword(n, bpc) {
  return Math.ceil((n * 8) / bpc);
}

// Pick codeword length n (<=255) so that codewords tile the data area with little waste.
export function chooseCodeword(layout, mode, eccLevel) {
  const bpc = MODES[mode].bpc;
  const D = layout.dataCells;
  const m = Math.max(1, Math.ceil(D / cellsPerCodeword(255, bpc)));
  let n = 255;
  while (n > 24 && m * cellsPerCodeword(n, bpc) > D) n--;
  const nsym = Math.max(4, 2 * Math.floor((n * ECC_PERCENT[eccLevel] + 100) / 200));
  return { n, nsym };
}

export function codewordsPerFrame(layout, mode, n) {
  return Math.floor(layout.dataCells / cellsPerCodeword(n, MODES[mode].bpc));
}

// ---- header (70 bits): ver4 mode2 X9 Y9 n8 nsym/2:7 seq8 rsv7 crc16
export function packHeader(h, copy) {
  const bits = new Uint8Array(HEADER_BITS);
  let p = 0;
  const put = (v, w) => {
    for (let i = w - 1; i >= 0; i--) bits[p++] = (v >>> i) & 1;
  };
  put(VERSION, 4);
  put(h.mode, 2);
  put(h.X, 9);
  put(h.Y, 9);
  put(h.n, 8);
  put(h.nsym >>> 1, 7);
  put(h.seq & 255, 8);
  put(0, 7);
  put(crc16bits(bits, 54), 16);
  for (let i = 0; i < HEADER_BITS; i++) bits[i] ^= HEADER_MASKS[copy][i];
  return bits;
}

// Header validity (§6.3). Returns { header } or { error } with the first failed rule:
// 'crc', 'version', 'mode', 'size' (X or Y < 40), 'nsym' (< 2), 'T' (T < 16), 'm' (no codeword fits).
export function decodeHeader(raw, copy) {
  const bits = new Uint8Array(HEADER_BITS);
  for (let i = 0; i < HEADER_BITS; i++) bits[i] = raw[i] ^ HEADER_MASKS[copy][i];
  let p = 0;
  const get = (w) => {
    let v = 0;
    for (let i = 0; i < w; i++) v = (v << 1) | bits[p++];
    return v;
  };
  const ver = get(4);
  const mode = get(2);
  const X = get(9);
  const Y = get(9);
  const n = get(8);
  const nsym = get(7) * 2;
  const seq = get(8);
  get(7);
  const crc = get(16);
  if (crc !== crc16bits(bits, 54)) return { error: 'crc' };
  if (ver !== VERSION) return { error: 'version' };
  if (mode > 2) return { error: 'mode' };
  if (X < MIN_DIM || Y < MIN_DIM) return { error: 'size' };
  if (nsym < 2) return { error: 'nsym' };
  if (n - nsym < MIN_K) return { error: 'T' };
  if (Math.floor(dataCellCount(X, Y) / cellsPerCodeword(n, MODES[mode].bpc)) < 1) return { error: 'm' };
  return { header: { mode, X, Y, n, nsym, seq } };
}

export function unpackHeader(raw, copy) {
  return decodeHeader(raw, copy).header || null;
}
