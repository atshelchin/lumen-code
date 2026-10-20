// Rasterise a frame (cell colour indices) into RGBA pixels at an integer cell size.
// Static parts (finders, corner, markers) are drawn once with rounded shapes;
// data/header cells are drawn per frame as rounded "LED" squares on ink.

import { MODES, INK, PAPER, K_DATA, K_HEADER } from './layout.js?v=bd0b2be23b';
import { IDX_INK, IDX_PAPER } from './frame.js?v=bd0b2be23b';

function pack(rgb) {
  // little-endian RGBA
  return (255 << 24) | (rgb[2] << 16) | (rgb[1] << 8) | rgb[0];
}

function rrInside(px, py, cx, cy, half, r) {
  const dx = Math.max(Math.abs(px - cx) - (half - r), 0);
  const dy = Math.max(Math.abs(py - cy) - (half - r), 0);
  return dx * dx + dy * dy <= r * r;
}

export function cellStyle(s) {
  if (s <= 4) return { gap: 0, radius: 0 };
  if (s <= 7) return { gap: 1, radius: 1 };
  return { gap: Math.max(1, Math.round(s * 0.11)), radius: Math.round(s * 0.24) };
}

export class Renderer {
  constructor(layout, mode, s, quiet = 2) {
    this.layout = layout;
    this.mode = mode;
    this.s = s;
    this.q = quiet;
    const { X, Y } = layout;
    this.width = (X + 2 * quiet) * s;
    this.height = (Y + 2 * quiet) * s;
    const table = new Uint32Array(10);
    const pal = MODES[mode].palette;
    for (let i = 0; i < 8; i++) table[i] = pack(pal[i % pal.length]);
    table[IDX_INK] = pack(INK);
    table[IDX_PAPER] = pack(PAPER);
    this.table = table;
    this.ink = pack(INK);
    this.paper = pack(PAPER);

    // per-cell pixel spans for a rounded cell
    const { gap, radius } = cellStyle(s);
    const inner = s - gap;
    const spans = [];
    for (let y = 0; y < s; y++) {
      if (y >= inner) {
        spans.push(null);
        continue;
      }
      let x0 = 0;
      let x1 = inner;
      if (radius > 0) {
        const cy = y + 0.5;
        const dyc = cy < radius ? radius - cy : cy > inner - radius ? cy - (inner - radius) : 0;
        if (dyc > 0) {
          const dx = radius - Math.sqrt(Math.max(0, radius * radius - dyc * dyc));
          x0 = Math.round(dx);
          x1 = inner - x0;
        }
      }
      spans.push([x0, x1]);
    }
    this.spans = spans;
    this.staticLayer = this._buildStatic();
  }

  _buildStatic() {
    const { X, Y } = this.layout;
    const { s, q, width, height } = this;
    const px = new Uint32Array(width * height);
    px.fill(this.ink);
    // supersampled shape painter
    const paintShape = (ox, oy, size, inside) => {
      // ox, oy, size in cells (grid coords); inside(gx, gy) -> 0 ink | 1 paper, gx,gy in cell units relative to origin
      const x0 = (ox + q) * s;
      const y0 = (oy + q) * s;
      const w = size * s;
      for (let y = 0; y < w; y++)
        for (let x = 0; x < w; x++) {
          let acc = 0;
          for (let sy = 0; sy < 2; sy++)
            for (let sx = 0; sx < 2; sx++) acc += inside((x + 0.25 + sx * 0.5) / s, (y + 0.25 + sy * 0.5) / s);
          const a = acc / 4;
          const r = Math.round(INK[0] + (PAPER[0] - INK[0]) * a);
          const g = Math.round(INK[1] + (PAPER[1] - INK[1]) * a);
          const b = Math.round(INK[2] + (PAPER[2] - INK[2]) * a);
          px[(y0 + y) * width + x0 + x] = pack([r, g, b]);
        }
    };
    const finder = (gx, gy) => {
      if (rrInside(gx, gy, 3.5, 3.5, 1.5, 0.55)) return 1;
      if (rrInside(gx, gy, 3.5, 3.5, 2.5, 0.95)) return 0;
      if (rrInside(gx, gy, 3.5, 3.5, 3.5, 1.5)) return 1;
      return 0;
    };
    const corner = (gx, gy) => {
      if (rrInside(gx, gy, 3.5, 3.5, 0.5, 0.5)) return 1;
      if (rrInside(gx, gy, 3.5, 3.5, 2.5, 0.95)) return 0;
      if (rrInside(gx, gy, 3.5, 3.5, 3.5, 1.5)) return 1;
      return 0;
    };
    const marker = (gx, gy) => (rrInside(gx, gy, 1.5, 1.5, 0.5, 0.32) ? 1 : 0);
    paintShape(0, 0, 7, finder);
    paintShape(X - 7, 0, 7, finder);
    paintShape(0, Y - 7, 7, finder);
    paintShape(X - 7, Y - 7, 7, corner);
    const { lx, ly } = this.layout;
    for (let j = 0; j < ly.length; j++)
      for (let i = 0; i < lx.length; i++) {
        const isCorner = (i === 0 || i === lx.length - 1) && (j === 0 || j === ly.length - 1);
        if (!isCorner) paintShape(lx[i] - 1, ly[j] - 1, 3, marker);
      }
    return px;
  }

  // Static mask at full resolution: everything opaque except the coloured interiors of
  // data/header cells, which are transparent. Draw the 1-pixel-per-cell image scaled by
  // `s` (nearest neighbour) underneath it to compose a frame on the GPU.
  maskLayer() {
    const { X, Y, kind } = this.layout;
    const { s, q, width, spans } = this;
    const px = this.staticLayer.slice();
    for (let cy = 0; cy < Y; cy++)
      for (let cx = 0; cx < X; cx++) {
        const k = kind[cy * X + cx];
        if (k !== K_DATA && k !== K_HEADER) continue;
        const base = (cy + q) * s * width + (cx + q) * s;
        for (let y = 0; y < s; y++) {
          const sp = spans[y];
          if (sp) px.fill(0, base + y * width + sp[0], base + y * width + sp[1]);
        }
      }
    return px;
  }

  // One pixel per cell (grid plus quiet zone): (X + 2q) × (Y + 2q).
  cellPixels(cells, out) {
    const { X, Y } = this.layout;
    const { q, table } = this;
    const W = X + 2 * q;
    for (let cy = 0; cy < Y; cy++) {
      const row = (cy + q) * W + q;
      for (let cx = 0; cx < X; cx++) out[row + cx] = table[cells[cy * X + cx]];
    }
  }

  // out: Uint32Array view over RGBA buffer of width*height (full-resolution CPU path)
  draw(cells, out) {
    const { X, Y, kind } = this.layout;
    const { s, q, width, table, spans } = this;
    out.set(this.staticLayer);
    for (let cy = 0; cy < Y; cy++) {
      const rowBase = (cy + q) * s * width;
      for (let cx = 0; cx < X; cx++) {
        const i = cy * X + cx;
        const k = kind[i];
        if (k !== K_DATA && k !== K_HEADER) continue;
        const color = table[cells[i]];
        if (color === this.ink) continue;
        const base = rowBase + (cx + q) * s;
        for (let y = 0; y < s; y++) {
          const sp = spans[y];
          if (!sp) continue;
          const o = base + y * width;
          out.fill(color, o + sp[0], o + sp[1]);
        }
      }
    }
  }
}
