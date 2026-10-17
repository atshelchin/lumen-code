// Camera frame -> codewords. Pure JS, no OpenCV.
//
// 1. luma + block thresholds
// 2. finder candidates by 1:1:3:1:1 run scanning (light-on-dark finders), cross-checked
// 3. for each plausible finder triple: refine centres + module sizes, read the header
// 4. corner pattern -> global homography -> alignment lattice found progressively
// 5. per-quad homographies; each cell sampled and normalised by local white/ink references
// 6. colour classification (k-means seeded by the palette), confidences -> RS (with erasures)

import { makeLayout, unpackHeader, MODES, INK, PAPER, HEADER_BITS, HEADER_MASKS, codewordsPerFrame, cellsPerCodeword } from './layout.js?v=37123c580f';
import { decodeCodewords } from './frame.js?v=37123c580f';

// ---------- linear algebra
export function homography(src, dst) {
  // src/dst: [[x,y] x4]; returns 9-element array mapping src -> dst
  const A = [];
  for (let i = 0; i < 4; i++) {
    const [x, y] = src[i];
    const [u, v] = dst[i];
    A.push([x, y, 1, 0, 0, 0, -u * x, -u * y, u]);
    A.push([0, 0, 0, x, y, 1, -v * x, -v * y, v]);
  }
  for (let c = 0; c < 8; c++) {
    let p = c;
    for (let r = c + 1; r < 8; r++) if (Math.abs(A[r][c]) > Math.abs(A[p][c])) p = r;
    if (Math.abs(A[p][c]) < 1e-12) return null;
    [A[c], A[p]] = [A[p], A[c]];
    for (let r = 0; r < 8; r++) {
      if (r === c) continue;
      const f = A[r][c] / A[c][c];
      if (f === 0) continue;
      for (let k = c; k < 9; k++) A[r][k] -= f * A[c][k];
    }
  }
  const h = new Float64Array(9);
  for (let i = 0; i < 8; i++) h[i] = A[i][8] / A[i][i];
  h[8] = 1;
  return h;
}

export function applyH(H, x, y, out) {
  const w = H[6] * x + H[7] * y + H[8];
  out[0] = (H[0] * x + H[1] * y + H[2]) / w;
  out[1] = (H[3] * x + H[4] * y + H[5]) / w;
  return out;
}

// Least-squares homography (DLT, h8 = 1) from >= 4 correspondences, with normalisation.
export function fitHomography(src, dst, wts) {
  const n = src.length;
  if (n < 4) return null;
  let sx = 0, sy = 0, dx = 0, dy = 0;
  for (let i = 0; i < n; i++) {
    sx += src[i][0];
    sy += src[i][1];
    dx += dst[i][0];
    dy += dst[i][1];
  }
  sx /= n; sy /= n; dx /= n; dy /= n;
  let ss = 0, ds = 0;
  for (let i = 0; i < n; i++) {
    ss += Math.hypot(src[i][0] - sx, src[i][1] - sy);
    ds += Math.hypot(dst[i][0] - dx, dst[i][1] - dy);
  }
  ss = ss / n || 1;
  ds = ds / n || 1;
  const N = new Float64Array(64);
  const B = new Float64Array(8);
  const row = new Float64Array(8);
  const acc = (r, rhs, w) => {
    for (let a = 0; a < 8; a++) {
      if (!r[a]) continue;
      B[a] += w * r[a] * rhs;
      for (let b = 0; b < 8; b++) N[a * 8 + b] += w * r[a] * r[b];
    }
  };
  for (let i = 0; i < n; i++) {
    const x = (src[i][0] - sx) / ss;
    const y = (src[i][1] - sy) / ss;
    const u = (dst[i][0] - dx) / ds;
    const v = (dst[i][1] - dy) / ds;
    const w = wts ? wts[i] : 1;
    row.set([x, y, 1, 0, 0, 0, -u * x, -u * y]);
    acc(row, u, w);
    row.set([0, 0, 0, x, y, 1, -v * x, -v * y]);
    acc(row, v, w);
  }
  // solve 8x8
  const A = [];
  for (let a = 0; a < 8; a++) A.push([...N.subarray(a * 8, a * 8 + 8), B[a]]);
  for (let c = 0; c < 8; c++) {
    let p = c;
    for (let r = c + 1; r < 8; r++) if (Math.abs(A[r][c]) > Math.abs(A[p][c])) p = r;
    if (Math.abs(A[p][c]) < 1e-12) return null;
    [A[c], A[p]] = [A[p], A[c]];
    for (let r = 0; r < 8; r++) {
      if (r === c) continue;
      const f = A[r][c] / A[c][c];
      if (f) for (let k = c; k < 9; k++) A[r][k] -= f * A[c][k];
    }
  }
  const h = [];
  for (let i = 0; i < 8; i++) h.push(A[i][8] / A[i][i]);
  h.push(1);
  // denormalise: H = Td^-1 * Hn * Ts
  const Ts = [1 / ss, 0, -sx / ss, 0, 1 / ss, -sy / ss, 0, 0, 1];
  const Tdi = [ds, 0, dx, 0, ds, dy, 0, 0, 1];
  const mul = (P, Q) => {
    const R = new Float64Array(9);
    for (let i = 0; i < 3; i++)
      for (let j = 0; j < 3; j++) R[i * 3 + j] = P[i * 3] * Q[j] + P[i * 3 + 1] * Q[3 + j] + P[i * 3 + 2] * Q[6 + j];
    return R;
  };
  const H = mul(Tdi, mul(h, Ts));
  for (let i = 0; i < 9; i++) H[i] /= H[8];
  return H;
}

function solve3(M, b) {
  // 3x3 solve via Cramer
  const [a, b1, c, d, e, f, g, h, i] = M;
  const det = a * (e * i - f * h) - b1 * (d * i - f * g) + c * (d * h - e * g);
  if (Math.abs(det) < 1e-12) return null;
  const x = (b[0] * (e * i - f * h) - b1 * (b[1] * i - f * b[2]) + c * (b[1] * h - e * b[2])) / det;
  const y = (a * (b[1] * i - f * b[2]) - b[0] * (d * i - f * g) + c * (d * b[2] - b[1] * g)) / det;
  const z = (a * (e * b[2] - b[1] * h) - b1 * (d * b[2] - b[1] * g) + b[0] * (d * h - e * g)) / det;
  return [x, y, z];
}

const HEADER_HYPS = [0];
for (let i = 1; i <= 8; i++) HEADER_HYPS.push(i * 0.0125, -i * 0.0125);

export class Scanner {
  constructor() {
    this.w = 0;
    this.h = 0;
    // Last header read correctly. X, Y, mode, n and nsym do not change within a transfer and
    // seq is not needed to decode data, so it stands in when every copy of a header is unreadable.
    this.lastHeader = null;
    this.reuseMisses = 0;
  }

  // Buffers only grow: region-of-interest crops change the image size every frame.
  _alloc(w, h) {
    this.w = w;
    this.h = h;
    if (!this.luma || this.luma.length < w * h) this.luma = new Uint8Array(Math.ceil(w * h * 1.1));
    this.bs = Math.min(w, h) >= 1400 ? 5 : 4; // block shift (16 or 32 px)
    this.bw = (w + (1 << this.bs) - 1) >> this.bs;
    this.bh = (h + (1 << this.bs) - 1) >> this.bs;
    const nb = this.bw * this.bh;
    if (!this.thr || this.thr.length < nb) {
      this.thr = new Uint8Array(nb * 2);
      this.bmax = new Uint8Array(nb * 2);
      this.bmin = new Uint8Array(nb * 2);
    }
  }

  // ---------- pixel access
  _prep(img) {
    const { width: w, height: h, data } = img;
    this._alloc(w, h);
    this.rgba = data;
    const L = this.luma;
    for (let i = 0, j = 0, N = w * h; i < N; i++, j += 4) L[i] = (data[j] * 77 + data[j + 1] * 150 + data[j + 2] * 29) >> 8;
    // block min/max -> threshold at the midpoint of the 5x5-block neighbourhood range
    const { bs, bw, bh, bmax, bmin, thr } = this;
    const B = 1 << bs;
    for (let by = 0; by < bh; by++)
      for (let bx = 0; bx < bw; bx++) {
        let mn = 255;
        let mx = 0;
        const y1 = Math.min(h, (by + 1) * B);
        const x1 = Math.min(w, (bx + 1) * B);
        // every other pixel in each direction: finder features are >= 2 px wide
        for (let y = by * B; y < y1; y += 2) {
          const row = y * w;
          for (let x = bx * B; x < x1; x += 2) {
            const v = L[row + x];
            if (v < mn) mn = v;
            if (v > mx) mx = v;
          }
        }
        bmax[by * bw + bx] = mx;
        bmin[by * bw + bx] = mn;
      }
    // flat regions (background, sensor noise) must not binarise: require a local range
    // that is a meaningful fraction of the frame's overall range
    let gmin = 255;
    let gmax = 0;
    for (let k = 0; k < bw * bh; k++) {
      if (bmin[k] < gmin) gmin = bmin[k];
      if (bmax[k] > gmax) gmax = bmax[k];
    }
    const minRange = Math.max(40, 0.3 * (gmax - gmin));
    for (let by = 0; by < bh; by++)
      for (let bx = 0; bx < bw; bx++) {
        let mn = 255;
        let mx = 0;
        for (let dy = -2; dy <= 2; dy++) {
          const yy = by + dy;
          if (yy < 0 || yy >= bh) continue;
          for (let dx = -2; dx <= 2; dx++) {
            const xx = bx + dx;
            if (xx < 0 || xx >= bw) continue;
            const k = yy * bw + xx;
            if (bmin[k] < mn) mn = bmin[k];
            if (bmax[k] > mx) mx = bmax[k];
          }
        }
        thr[by * bw + bx] = mx - mn < minRange ? 255 : (mn + mx) >> 1;
      }
  }

  _bright(x, y) {
    return this.luma[y * this.w + x] > this.thr[(y >> this.bs) * this.bw + (x >> this.bs)];
  }

  lumaAt(x, y) {
    const { w, h, luma } = this;
    x -= 0.5;
    y -= 0.5;
    if (x < 0) x = 0;
    if (y < 0) y = 0;
    if (x > w - 1.001) x = w - 1.001;
    if (y > h - 1.001) y = h - 1.001;
    const ix = x | 0;
    const iy = y | 0;
    const ax = x - ix;
    const ay = y - iy;
    const i = iy * w + ix;
    const a = luma[i] + (luma[i + 1] - luma[i]) * ax;
    const b = luma[i + w] + (luma[i + w + 1] - luma[i + w]) * ax;
    return a + (b - a) * ay;
  }

  rgbAt(x, y, out, k) {
    const { w, h, rgba } = this;
    x -= 0.5;
    y -= 0.5;
    if (x < 0) x = 0;
    if (y < 0) y = 0;
    if (x > w - 1.001) x = w - 1.001;
    if (y > h - 1.001) y = h - 1.001;
    const ix = x | 0;
    const iy = y | 0;
    const ax = x - ix;
    const ay = y - iy;
    const i = (iy * w + ix) * 4;
    const j = i + w * 4;
    const w00 = (1 - ax) * (1 - ay);
    const w10 = ax * (1 - ay);
    const w01 = (1 - ax) * ay;
    const w11 = ax * ay;
    for (let c = 0; c < 3; c++) {
      out[k + c] += rgba[i + c] * w00 + rgba[i + 4 + c] * w10 + rgba[j + c] * w01 + rgba[j + 4 + c] * w11;
    }
  }

  // ---------- finder search
  // 1:1:3:1:1 with generous tolerance: blur/motion thin the dark ring, bloom fattens the
  // light parts. Later stages (module fit, header CRC) reject false positives.
  _ratioOK(c0, c1, c2, c3, c4) {
    const total = c0 + c1 + c2 + c3 + c4;
    if (total < 7) return false;
    const m = total / 7;
    const lo = 0.3 * m;
    const hi = 1.9 * m;
    return (
      c0 > lo && c0 < hi && c1 > lo * 0.8 && c1 < hi && c3 > lo * 0.8 && c3 < hi && c4 > lo && c4 < hi && c2 > 1.7 * m && c2 < 4.6 * m
    );
  }

  // corner pattern: light ring, 2-wide dark ring, light dot -> 1:2:1:2:1
  _ratioCorner(c0, c1, c2, c3, c4) {
    const total = c0 + c1 + c2 + c3 + c4;
    if (total < 7) return false;
    const m = total / 7;
    return (
      c0 > 0.35 * m && c0 < 1.9 * m && c4 > 0.35 * m && c4 < 1.9 * m &&
      c1 > 1.1 * m && c1 < 3.2 * m && c3 > 1.1 * m && c3 < 3.2 * m &&
      c2 > 0.3 * m && c2 < 1.9 * m
    );
  }

  _crossCheck(cx, cy, dx, dy, maxCount, origTotal, ratioFn) {
    const { w, h } = this;
    const c = [0, 0, 0, 0, 0];
    const inb = (x, y) => x >= 0 && y >= 0 && x < w && y < h;
    let x = cx;
    let y = cy;
    while (inb(x, y) && this._bright(x, y)) {
      c[2]++;
      x -= dx;
      y -= dy;
    }
    if (!inb(x, y)) return null;
    while (inb(x, y) && !this._bright(x, y) && c[1] <= maxCount) {
      c[1]++;
      x -= dx;
      y -= dy;
    }
    if (!inb(x, y) || c[1] > maxCount) return null;
    while (inb(x, y) && this._bright(x, y) && c[0] <= maxCount) {
      c[0]++;
      x -= dx;
      y -= dy;
    }
    if (c[0] > maxCount) return null;
    x = cx + dx;
    y = cy + dy;
    let f = 1;
    while (inb(x, y) && this._bright(x, y)) {
      c[2]++;
      x += dx;
      y += dy;
      f++;
    }
    if (!inb(x, y)) return null;
    while (inb(x, y) && !this._bright(x, y) && c[3] <= maxCount) {
      c[3]++;
      x += dx;
      y += dy;
      f++;
    }
    if (!inb(x, y) || c[3] > maxCount) return null;
    while (inb(x, y) && this._bright(x, y) && c[4] <= maxCount) {
      c[4]++;
      x += dx;
      y += dy;
      f++;
    }
    if (c[4] > maxCount) return null;
    const total = c[0] + c[1] + c[2] + c[3] + c[4];
    if (5 * Math.abs(total - origTotal) >= 3 * origTotal) return null;
    if (!ratioFn.call(this, c[0], c[1], c[2], c[3], c[4])) return null;
    const centerOff = f - c[4] - c[3] - c[2] / 2;
    return { off: centerOff, total };
  }

  findFinders() {
    const { w, h } = this;
    const step = Math.max(1, Math.floor(h / 360));
    const cands = [];
    const runLen = new Int32Array(w + 1);
    const runStart = new Int32Array(w + 1);
    const runBright = new Uint8Array(w + 1);
    for (let y = step >> 1; y < h; y += step) {
      let n = 0;
      let prev = -1;
      for (let x = 0; x < w; x++) {
        const b = this._bright(x, y) ? 1 : 0;
        if (b !== prev) {
          runStart[n] = x;
          runLen[n] = 1;
          runBright[n] = b;
          n++;
          prev = b;
        } else runLen[n - 1]++;
      }
      for (let i = 0; i + 4 < n; i++) {
        if (!runBright[i]) continue;
        const c0 = runLen[i];
        const c1 = runLen[i + 1];
        const c2 = runLen[i + 2];
        const c3 = runLen[i + 3];
        const c4 = runLen[i + 4];
        const isF = this._ratioOK(c0, c1, c2, c3, c4);
        const isC = !isF && this._ratioCorner(c0, c1, c2, c3, c4);
        if (!isF && !isC) continue;
        const ratioFn = isF ? this._ratioOK : this._ratioCorner;
        const total = c0 + c1 + c2 + c3 + c4;
        const m = total / 7;
        if (i > 0 && runLen[i - 1] < 0.5 * m) continue;
        if (i + 5 < n && runLen[i + 5] < 0.5 * m) continue;
        const cx0 = Math.floor(runStart[i + 2] + c2 / 2);
        const maxCount = isF ? c2 * 1.5 : total * 0.6;
        const vc = this._crossCheck(cx0, y, 0, 1, maxCount, total, ratioFn);
        if (!vc) continue;
        const cy = Math.round(y - 1 + vc.off);
        if (cy < 0 || cy >= h) continue;
        const hc = this._crossCheck(cx0, cy, 1, 0, maxCount, total, ratioFn);
        if (!hc) continue;
        const cx = cx0 - 1 + hc.off;
        const mod = (vc.total + hc.total) / 14;
        if (mod < 2) continue; // too small to carry readable cells
        this._addCandidate(cands, cx + 0.5, cy + 0.5, mod, isF ? 'F' : 'C');
      }
    }
    return cands;
  }

  _addCandidate(cands, x, y, m, type) {
    for (const c of cands) {
      if (c.type === type && Math.abs(c.x - x) <= c.m * 1.5 && Math.abs(c.y - y) <= c.m * 1.5 && Math.abs(c.m - m) <= Math.max(1, 0.5 * c.m)) {
        const n = c.count;
        c.x = (c.x * n + x) / (n + 1);
        c.y = (c.y * n + y) / (n + 1);
        c.m = (c.m * n + m) / (n + 1);
        c.count++;
        return;
      }
    }
    cands.push({ x, y, m, count: 1, type });
  }

  // Candidate corner sets. Any three of the four corners suffice: three finders, or two
  // finders plus the corner pattern (so one glared / blurred / out-of-frame corner is fine).
  selectSets(cands) {
    const byStrength = (a, b) => b.count - a.count || b.m - a.m;
    const Fall = cands.filter((c) => c.type === 'F').sort(byStrength);
    const C = cands.filter((c) => c.type === 'C' && c.count >= 2).sort(byStrength).slice(0, 4);
    const d = (p, q) => Math.hypot(p.x - q.x, p.y - q.y);
    const cross = (o, a, b) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    const build = (F, withCorners) => {
      const out = [];
      const consider = (P, withCorner) => {
        const ms = P.map((p) => p.m);
        const mmax = Math.max(...ms);
        const mmin = Math.min(...ms);
        if (mmax / mmin > 3) return;
        const sides = [d(P[1], P[2]), d(P[0], P[2]), d(P[0], P[1])]; // opposite vertex i
        let rv = 0; // right-angle vertex
        if (sides[1] > sides[rv]) rv = 1;
        if (sides[2] > sides[rv]) rv = 2;
        const V = P[rv];
        const A = P[(rv + 1) % 3];
        const B = P[(rv + 2) % 3];
        const la = d(V, A);
        const lb = d(V, B);
        const cos = ((A.x - V.x) * (B.x - V.x) + (A.y - V.y) * (B.y - V.y)) / (la * lb);
        if (Math.abs(cos) > 0.65) return;
        if (la < 18 * mmin || lb < 18 * mmin) return;
        if (la / lb > 2.6 || lb / la > 2.6) return;
        const set = { TL: null, TR: null, BL: null, BR: null };
        if (!withCorner) {
          set.TL = V;
          const c = cross(V, A, B) > 0;
          set.TR = c ? A : B;
          set.BL = c ? B : A;
        } else {
          const corner = P.find((p) => p.type === 'C');
          if (V === corner) {
            // right angle at BR: the two finders are TR and BL
            set.BR = V;
            const c = cross(V, A, B) < 0;
            set.TR = c ? A : B;
            set.BL = c ? B : A;
          } else {
            // right angle at TR or BL; the other finder is TL (diagonal to BR)
            const other = A === corner ? B : A;
            set.BR = corner;
            set.TL = other;
            if (cross(other, V, corner) > 0) set.TR = V;
            else set.BL = V;
          }
        }
        // perspective legitimately skews angles and module sizes: keep these penalties soft
        set.score = P.reduce((acc, p) => acc + 4 * Math.log2(1 + Math.min(p.count, 15)), 0) - 8 * Math.abs(cos) - 2 * (mmax / mmin - 1);
        set.key = P.map((p) => cands.indexOf(p)).sort((a, b) => a - b).join(',');
        out.push(set);
      };
      for (let a = 0; a < F.length; a++)
        for (let b = a + 1; b < F.length; b++) {
          if (!withCorners) for (let c = b + 1; c < F.length; c++) consider([F[a], F[b], F[c]], false);
          else for (const k of C) consider([F[a], F[b], k], true);
        }
      return out.sort((a, b) => b.score - a.score);
    };
    // order: confident finders, then corner-pattern fallbacks, then single-hit finders
    const strong = Fall.filter((c) => c.count >= 2).slice(0, 14);
    const all = Fall.slice(0, 16);
    const res = [];
    const seen = new Set();
    for (const list of [build(strong, false).slice(0, 8), build(strong, true).slice(0, 4), build(all, false).slice(0, 8), build(all, true).slice(0, 3)])
      for (const st of list) {
        if (seen.has(st.key)) continue;
        seen.add(st.key);
        res.push(st);
      }
    return res;
  }

  // ---------- finder refinement
  // Locate the six ring edges of a finder along one axis from luma-gradient extrema
  // (blur-robust), then fit t = c0 + m·k + δ·e (δ absorbs symmetric bloom/thinning).
  _measureAxis(cx, cy, ux, uy, m0) {
    const step = 0.25;
    const tmax = 5 * m0;
    const ns = Math.ceil(tmax / step);
    const N = 2 * ns + 1;
    const prof = new Float32Array(N);
    for (let i = 0; i < N; i++) {
      const t = (i - ns) * step;
      prof[i] = this.lumaAt(cx + ux * t, cy + uy * t);
    }
    const der = new Float32Array(N);
    const h = Math.max(1, Math.round(0.25 / step));
    for (let i = h; i < N - h; i++) der[i] = prof[i + h] - prof[i - h];
    let m = m0;
    let c0 = 0;
    for (let pass = 0; pass < 2; pass++) {
      const ks = [];
      const ts = [];
      for (const k of [-3.5, -2.5, -1.5, 1.5, 2.5, 3.5]) {
        // sign of d(luma)/dt at this edge
        const ak = Math.abs(k);
        const outwardDown = ak !== 2.5;
        const sign = k > 0 ? (outwardDown ? -1 : 1) : outwardDown ? 1 : -1;
        const tc = c0 + m * k;
        const win = (pass === 0 ? 0.5 : 0.35) * m;
        const i0 = Math.max(h + 1, Math.round((tc - win) / step) + ns);
        const i1 = Math.min(N - h - 2, Math.round((tc + win) / step) + ns);
        let bi = -1;
        let bv = 0;
        for (let i = i0; i <= i1; i++) {
          const v = der[i] * sign;
          if (v > bv) {
            bv = v;
            bi = i;
          }
        }
        if (bi < 0) continue;
        const a = der[bi - 1] * sign;
        const b = der[bi] * sign;
        const c = der[bi + 1] * sign;
        const den = a - 2 * b + c;
        const off = den < 0 ? (0.5 * (a - c)) / den : 0;
        ks.push(k);
        ts.push((bi - ns + Math.max(-0.5, Math.min(0.5, off))) * step);
      }
      if (ks.length < 5) return null;
      let S11 = 0, S1k = 0, S1e = 0, Skk = 0, Ske = 0, See = 0, b1 = 0, bk = 0, be = 0;
      for (let i = 0; i < ks.length; i++) {
        const k = ks[i];
        const e = Math.sign(k) * (Math.abs(k) === 2.5 ? -1 : 1);
        const t = ts[i];
        S11 += 1; S1k += k; S1e += e; Skk += k * k; Ske += k * e; See += e * e;
        b1 += t; bk += k * t; be += e * t;
      }
      const sol = solve3([S11, S1k, S1e, S1k, Skk, Ske, S1e, Ske, See], [b1, bk, be]);
      if (!sol) return null;
      c0 = sol[0];
      m = sol[1];
      if (!(m > 0.55 * m0 && m < 1.7 * m0) || Math.abs(c0) > 1.5 * m0) return null;
    }
    return { c0, m };
  }

  _refineFinder(F, u, v) {
    let { x, y } = F;
    const m = F.m;
    const white = this.lumaAt(x, y);
    const ink =
      (this.lumaAt(x + 2 * m * u[0], y + 2 * m * u[1]) +
        this.lumaAt(x - 2 * m * u[0], y - 2 * m * u[1]) +
        this.lumaAt(x + 2 * m * v[0], y + 2 * m * v[1]) +
        this.lumaAt(x - 2 * m * v[0], y - 2 * m * v[1])) /
      4;
    if (white - ink < 20) return null;
    const thr = (white + ink) / 2;
    // centroid of the bright centre
    const c = this._centroid(x, y, 2.1 * m, thr);
    if (c) {
      x = c[0];
      y = c[1];
    }
    const au = this._measureAxis(x, y, u[0], u[1], m);
    const av = this._measureAxis(x, y, v[0], v[1], m);
    if (!au || !av) return null;
    x += au.c0 * u[0] + av.c0 * v[0];
    y += au.c0 * u[1] + av.c0 * v[1];
    return { x, y, mu: au.m, mv: av.m, white, ink };
  }

  _centroid(x, y, r, thr) {
    const { w, h, luma } = this;
    const x0 = Math.max(0, Math.floor(x - r));
    const x1 = Math.min(w - 1, Math.ceil(x + r));
    const y0 = Math.max(0, Math.floor(y - r));
    const y1 = Math.min(h - 1, Math.ceil(y + r));
    let sw = 0;
    let sx = 0;
    let sy = 0;
    const r2 = r * r;
    for (let yy = y0; yy <= y1; yy++) {
      const dy = yy + 0.5 - y;
      for (let xx = x0; xx <= x1; xx++) {
        const dx = xx + 0.5 - x;
        if (dx * dx + dy * dy > r2) continue;
        const wgt = luma[yy * w + xx] - thr;
        if (wgt <= 0) continue;
        sw += wgt;
        sx += wgt * (xx + 0.5);
        sy += wgt * (yy + 0.5);
      }
    }
    if (sw <= 0) return null;
    return [sx / sw, sy / sw];
  }

  // ---------- header
  // Reads one header copy. Returns { hdr } when its CRC is correct; otherwise { soft }: the
  // bit values of the best-aligned scale hypothesis, normalised and unmasked, so that the
  // caller can combine several copies (a bit wrong in one copy is usually right in others).
  _readHeader(F, u, v, which) {
    const thr = (F.white + F.ink) / 2;
    const raw = new Uint8Array(HEADER_BITS);
    const vals = new Float32Array(HEADER_BITS);
    const offs = [];
    for (let r = 0; r < 7; r++)
      for (let c = 0; c < 10; c++) {
        if (which === 0) offs.push([c + 5, r - 3]);
        else if (which === 1) offs.push([r - 3, c + 5]);
        else if (which === 2) offs.push([c - 14, r - 3]);
        else offs.push([r - 3, c - 14]);
      }
    const horiz = which === 0 || which === 2;
    const mask = HEADER_MASKS[which];
    let best = null;
    let bestQ = -1;
    for (const ds of HEADER_HYPS) {
      const su = horiz ? 1 + ds : 1;
      const sv = horiz ? 1 : 1 + ds;
      const ux = u[0] * F.mu * su;
      const uy = u[1] * F.mu * su;
      const vx = v[0] * F.mv * sv;
      const vy = v[1] * F.mv * sv;
      for (let b = 0; b < HEADER_BITS; b++) {
        const [dx, dy] = offs[b];
        const px = F.x + dx * ux + dy * vx;
        const py = F.y + dx * uy + dy * vy;
        let acc = this.lumaAt(px, py) * 2;
        acc += this.lumaAt(px + 0.2 * ux, py + 0.2 * uy) + this.lumaAt(px - 0.2 * ux, py - 0.2 * uy);
        acc += this.lumaAt(px + 0.2 * vx, py + 0.2 * vy) + this.lumaAt(px - 0.2 * vx, py - 0.2 * vy);
        vals[b] = acc / 6;
      }
      // two-means threshold (bits are whitened, so roughly balanced); blur lowers contrast
      let t = thr;
      let m0 = thr;
      let m1 = thr;
      for (let it = 0; it < 4; it++) {
        let s0 = 0, n0 = 0, s1 = 0, n1 = 0;
        for (let b = 0; b < HEADER_BITS; b++) {
          if (vals[b] > t) {
            s1 += vals[b];
            n1++;
          } else {
            s0 += vals[b];
            n0++;
          }
        }
        if (!n0 || !n1) break;
        m0 = s0 / n0;
        m1 = s1 / n1;
        t = (m0 + m1) / 2;
      }
      for (const tt of t === thr ? [thr] : [t, thr]) {
        for (let b = 0; b < HEADER_BITS; b++) raw[b] = vals[b] > tt ? 1 : 0;
        const hdr = unpackHeader(raw, which);
        if (hdr) return { hdr };
      }
      // alignment quality: well-aligned samples sit far from the threshold
      const spread = Math.max(1, m1 - m0);
      let q = 0;
      for (let b = 0; b < HEADER_BITS; b++) q += Math.min(1, Math.abs(vals[b] - t) / (spread / 2));
      if (q > bestQ) {
        bestQ = q;
        best = new Float32Array(HEADER_BITS);
        for (let b = 0; b < HEADER_BITS; b++) {
          const sv2 = Math.max(-1, Math.min(1, (vals[b] - t) / (spread / 2)));
          best[b] = mask[b] ? -sv2 : sv2;
        }
      }
    }
    return { soft: best };
  }

  // Soft majority of several header copies (in unmasked bit space). A copy that disagrees
  // with the others on more than a quarter of the bits was misread and is left out.
  _combineHeaders(softs) {
    if (softs.length < 2) return null;
    let use = softs;
    if (softs.length >= 3) {
      use = softs.filter((sv, i) => {
        let agree = 0;
        for (let b = 0; b < HEADER_BITS; b++) {
          let others = 0;
          softs.forEach((o, j) => j !== i && (others += o[b]));
          if (sv[b] > 0 === others > 0) agree++;
        }
        return agree >= 0.75 * HEADER_BITS;
      });
      if (use.length < 2) return null;
    }
    const raw = new Uint8Array(HEADER_BITS);
    for (let b = 0; b < HEADER_BITS; b++) {
      let s = 0;
      for (const sv of use) s += sv[b];
      raw[b] = (s > 0 ? 1 : 0) ^ HEADER_MASKS[0][b];
    }
    return unpackHeader(raw, 0);
  }

  _refineCorner(Cc, u, v) {
    const m = Cc.m;
    const du = [u[0] * m, u[1] * m];
    const dv = [v[0] * m, v[1] * m];
    const res = this._search([Cc.x, Cc.y], du, dv, 0.9 * m, this._cornerScore);
    const fine = this._search([res.x, res.y], du, dv, 0.3 * m, this._cornerScore);
    let white = 0;
    let ink = 0;
    for (const sg of [-1, 1]) {
      white += this.lumaAt(fine.x + 3 * sg * du[0], fine.y + 3 * sg * du[1]) + this.lumaAt(fine.x + 3 * sg * dv[0], fine.y + 3 * sg * dv[1]);
      ink += this.lumaAt(fine.x + 1.5 * sg * du[0], fine.y + 1.5 * sg * du[1]) + this.lumaAt(fine.x + 1.5 * sg * dv[0], fine.y + 1.5 * sg * dv[1]);
    }
    white /= 4;
    ink /= 4;
    if (white - ink < 20) return null;
    // strict structure check along the code axes: light dot and outer ring, uniformly dark rings
    let minLight = this.lumaAt(fine.x, fine.y);
    let maxDark = 0;
    const at = (a, b) => this.lumaAt(fine.x + a * du[0] + b * dv[0], fine.y + a * du[1] + b * dv[1]);
    for (const [a, b] of [[3, 0], [-3, 0], [0, 3], [0, -3], [3, 1.5], [3, -1.5], [-3, 1.5], [-3, -1.5], [1.5, 3], [-1.5, 3], [1.5, -3], [-1.5, -3]])
      minLight = Math.min(minLight, at(a, b));
    for (const r of [1.1, 2])
      for (const [a, b] of [[1, 0], [-1, 0], [0, 1], [0, -1], [0.75, 0.75], [-0.75, 0.75], [0.75, -0.75], [-0.75, -0.75]])
        maxDark = Math.max(maxDark, at(a * r, b * r));
    if (minLight - maxDark < 0.12 * (white - ink)) return null;
    const c = this._centroid(fine.x, fine.y, 1.3 * m, (this.lumaAt(fine.x, fine.y) + ink) / 2);
    return { x: c ? c[0] : fine.x, y: c ? c[1] : fine.y, m, white, ink, corner: true };
  }

  _tryCorners(set, mirrored) {
    const K = { TL: set.TL, TR: mirrored ? set.BL : set.TR, BL: mirrored ? set.TR : set.BL, BR: set.BR };
    const nrm = (a, b) => {
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const l = Math.hypot(dx, dy);
      return [dx / l, dy / l];
    };
    const axes = (Q) => {
      const uTop = Q.TL && Q.TR ? nrm(Q.TL, Q.TR) : null;
      const uBot = Q.BL && Q.BR ? nrm(Q.BL, Q.BR) : null;
      const vLeft = Q.TL && Q.BL ? nrm(Q.TL, Q.BL) : null;
      const vRight = Q.TR && Q.BR ? nrm(Q.TR, Q.BR) : null;
      return {
        TL: [uTop || uBot, vLeft || vRight],
        TR: [uTop || uBot, vRight || vLeft],
        BL: [uBot || uTop, vLeft || vRight],
        BR: [uBot || uTop, vRight || vLeft],
      };
    };
    let ax = axes(K);
    const R = { TL: null, TR: null, BL: null, BR: null };
    for (const key of ['TL', 'TR', 'BL']) if (K[key]) R[key] = this._refineFinder(K[key], ax[key][0], ax[key][1]);
    if (K.BR) R.BR = this._refineCorner(K.BR, ax.BR[0], ax.BR[1]);
    const have = ['TL', 'TR', 'BL', 'BR'].filter((k) => R[k]);
    if (have.length < 3) return null;
    ax = axes(R);
    let hdr = null;
    const softs = [];
    for (const [key, which] of [['TL', 0], ['TL', 1], ['TR', 2], ['BL', 3]]) {
      if (!R[key]) continue;
      const r = this._readHeader(R[key], ax[key][0], ax[key][1], which);
      if (r.hdr) {
        hdr = r.hdr;
        break;
      }
      if (r.soft) softs.push(r.soft);
    }
    if (!hdr) hdr = this._combineHeaders(softs);
    if (!hdr) return { hdr: null, R }; // geometry without a header: kept for header reuse
    if (!this._fits(hdr, R)) return null;
    return { hdr, R };
  }

  // sanity: header dimensions vs. measured corner spacing
  _fits(hdr, R) {
    const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
    const Xest = R.TL && R.TR ? dist(R.TL, R.TR) / ((R.TL.mu + R.TR.mu) / 2) + 7 : R.BL && R.BR ? dist(R.BL, R.BR) / R.BL.mu + 7 : hdr.X;
    const Yest = R.TL && R.BL ? dist(R.TL, R.BL) / ((R.TL.mv + R.BL.mv) / 2) + 7 : R.TR && R.BR ? dist(R.TR, R.BR) / R.TR.mv + 7 : hdr.Y;
    return Math.abs(hdr.X - Xest) <= 0.2 * Xest + 3 && Math.abs(hdr.Y - Yest) <= 0.2 * Yest + 3;
  }

  // ---------- lattice
  // light centre, uniformly dark 8-ring (a data cell rarely has all eight neighbours dark)
  _markerScore(px, py, du, dv) {
    const c = this.lumaAt(px, py);
    let ring = 0;
    let rmax = 0;
    for (let a = -1; a <= 1; a++)
      for (let b = -1; b <= 1; b++) {
        if (!a && !b) continue;
        const v = this.lumaAt(px + a * du[0] + b * dv[0], py + a * du[1] + b * dv[1]);
        ring += v;
        if (v > rmax) rmax = v;
      }
    return c - (ring / 8 + rmax) / 2;
  }

  _cornerScore(px, py, du, dv) {
    const c = this.lumaAt(px, py);
    let ink = 0;
    let outer = 0;
    for (let a = -1; a <= 1; a++)
      for (let b = -1; b <= 1; b++) {
        if (!a && !b) continue;
        ink += this.lumaAt(px + 1.5 * (a * du[0] + b * dv[0]), py + 1.5 * (a * du[1] + b * dv[1]));
        outer += this.lumaAt(px + 3 * (a * du[0] + b * dv[0]), py + 3 * (a * du[1] + b * dv[1]));
      }
    return (c + outer / 8) / 2 - ink / 8;
  }

  _search(pred, du, dv, radius, scoreFn) {
    const cell = Math.max(Math.hypot(du[0], du[1]), Math.hypot(dv[0], dv[1]));
    const step = Math.max(0.5, Math.min(cell / 4, radius / 6));
    let best = -1e9;
    let bx = pred[0];
    let by = pred[1];
    for (let oy = -radius; oy <= radius; oy += step)
      for (let ox = -radius; ox <= radius; ox += step) {
        if (ox * ox + oy * oy > radius * radius) continue;
        const s = scoreFn.call(this, pred[0] + ox, pred[1] + oy, du, dv);
        if (s > best) {
          best = s;
          bx = pred[0] + ox;
          by = pred[1] + oy;
        }
      }
    return { x: bx, y: by, score: best };
  }

  /**
   * opts.skip: { [frameKey]: number[] } codeword indices already decoded for recent frames.
   * A frame whose codewords are all known returns right after the header (cheap), and a
   * partially known frame only samples and RS-decodes the missing codewords.
   */
  scan(img, opts = {}) {
    this.opts = opts;
    this._prep(img);
    const cands = this.findFinders();
    if (cands.length < 3) return { ok: false, reason: 'finders', candidates: cands.length };
    const sets = this.selectSets(cands);
    const headerless = [];
    for (const set of sets) {
      for (const mir of [false, true]) {
        const g = this._tryCorners(set, mir);
        if (!g) continue;
        if (!g.hdr) {
          headerless.push(g);
          continue;
        }
        const res = this._decode(g);
        if (res.ok) {
          const { mode, X, Y, n, nsym } = g.hdr;
          this.lastHeader = { mode, X, Y, n, nsym };
          this.reuseMisses = 0;
        }
        if (res.ok || res.reason !== 'lattice') return res;
      }
    }
    // Only after the normal search failed: decode the located grid with the last good header.
    // A wrong reuse is caught by the size check, the marker lattice and the packet CRCs.
    if (this.lastHeader && headerless.length) {
      const hdr = { ...this.lastHeader, seq: -1 };
      for (const g of headerless.slice(0, 4)) {
        if (!this._fits(hdr, g.R)) continue;
        const res = this._decode({ hdr, R: g.R, reused: true });
        if (!res.ok) continue;
        res.headerReused = true;
        this.reuseMisses = res.stats.cwOk > 0 ? 0 : this.reuseMisses + 1;
        if (this.reuseMisses >= 3) this.lastHeader = null;
        return res;
      }
      if (++this.reuseMisses >= 3) this.lastHeader = null;
    }
    return { ok: false, reason: sets.length ? 'header' : 'geometry', candidates: cands.length };
  }

  _decode(g) {
    const { hdr, R, reused } = g;
    const { X, Y, mode, n, nsym } = hdr;
    const layout = makeLayout(X, Y);
    const { lx, ly } = layout;
    const nx = lx.length;
    const ny = ly.length;

    const avail = ['TL', 'TR', 'BL', 'BR'].filter((k) => R[k]);
    const contrast = avail.reduce((a, k) => a + R[k].white - R[k].ink, 0) / avail.length;

    // lattice, grown outward from the three finders. Each point is predicted from a
    // weighted local affine fit of already-located neighbours, so perspective and lens
    // distortion are followed progressively; the corner pattern is just the last point.
    const NL = nx * ny;
    const P = new Float64Array(NL * 2);
    const state = new Uint8Array(NL); // 0 unknown, 1 found, 2 predicted
    const refs = new Float32Array(NL * 6); // white rgb, ink rgb
    const LDU = new Float64Array(NL * 4); // local cell vectors du, dv
    const gxOf = (k) => lx[k % nx] + 0.5;
    const gyOf = (k) => ly[(k / nx) | 0] + 0.5;
    const rgb = new Float32Array(3);
    const sampleRGB = (x, y, du, dv, spread) => {
      rgb[0] = rgb[1] = rgb[2] = 0;
      this.rgbAt(x, y, rgb, 0);
      this.rgbAt(x, y, rgb, 0);
      this.rgbAt(x + spread * du[0], y + spread * du[1], rgb, 0);
      this.rgbAt(x - spread * du[0], y - spread * du[1], rgb, 0);
      this.rgbAt(x + spread * dv[0], y + spread * dv[1], rgb, 0);
      this.rgbAt(x - spread * dv[0], y - spread * dv[1], rgb, 0);
      rgb[0] /= 6;
      rgb[1] /= 6;
      rgb[2] /= 6;
      return rgb;
    };
    const setRefs = (k, white, inkPts) => {
      for (let c = 0; c < 3; c++) refs[k * 6 + c] = white[c];
      const acc = new Float32Array(3);
      for (const [x, y] of inkPts) this.rgbAt(x, y, acc, 0);
      for (let c = 0; c < 3; c++) refs[k * 6 + 3 + c] = acc[c] / inkPts.length;
    };
    const ring = (x, y, du, dv, r) => {
      const pts = [];
      for (let a = -1; a <= 1; a++)
        for (let b = -1; b <= 1; b++) if (a || b) pts.push([x + r * (a * du[0] + b * dv[0]), y + r * (a * du[1] + b * dv[1])]);
      return pts;
    };

    // average cell vectors from whichever corner pairs are available
    const pairU = R.TL && R.TR ? [R.TL, R.TR] : [R.BL, R.BR];
    const pairV = R.TL && R.BL ? [R.TL, R.BL] : [R.TR, R.BR];
    const du0 = [(pairU[1].x - pairU[0].x) / (X - 7), (pairU[1].y - pairU[0].y) / (X - 7)];
    const dv0 = [(pairV[1].x - pairV[0].x) / (Y - 7), (pairV[1].y - pairV[0].y) / (Y - 7)];
    const cellPx = (Math.hypot(du0[0], du0[1]) + Math.hypot(dv0[0], dv0[1])) / 2;
    const lu = Math.hypot(du0[0], du0[1]);
    const lv = Math.hypot(dv0[0], dv0[1]);

    // a reused header has no seq: such a frame cannot be told apart from others (no skipping)
    const frameKey = reused ? null : `${hdr.seq}:${X}:${Y}:${mode}:${n}:${nsym}`;
    const count = codewordsPerFrame(layout, mode, n);
    const doneIdx = new Set((frameKey && this.opts && this.opts.skip && this.opts.skip[frameKey]) || []);
    if (doneIdx.size >= count) {
      const ref = avail.includes('TL') ? ['TL', 3.5, 3.5] : avail.includes('TR') ? ['TR', X - 3.5, 3.5] : ['BL', 3.5, Y - 3.5];
      const F = R[ref[0]];
      const corners = [[0, 0], [X, 0], [X, Y], [0, Y]].map(([gx, gy]) => [
        F.x + (gx - ref[1]) * du0[0] + (gy - ref[2]) * dv0[0],
        F.y + (gx - ref[1]) * du0[1] + (gy - ref[2]) * dv0[1],
      ]);
      return { ok: true, skipped: true, header: hdr, frameKey, codewords: [], stats: { cellPx, corners, cwOk: 0, cwTotal: 0, cwCount: count, corrected: 0 } };
    }

    const kTL = 0;
    const kTR = nx - 1;
    const kBL = (ny - 1) * nx;
    const kBR = NL - 1;
    const KEY = { TL: kTL, TR: kTR, BL: kBL, BR: kBR };
    const missingFinder = new Set();
    for (const key of ['TL', 'TR', 'BL']) {
      const F = R[key];
      const k = KEY[key];
      if (!F) {
        missingFinder.add(k);
        continue;
      }
      P[k * 2] = F.x;
      P[k * 2 + 1] = F.y;
      state[k] = 1;
      const du = [(du0[0] / lu) * F.mu, (du0[1] / lu) * F.mu];
      const dv = [(dv0[0] / lv) * F.mv, (dv0[1] / lv) * F.mv];
      LDU.set([du[0], du[1], dv[0], dv[1]], k * 4);
      setRefs(k, Array.from(sampleRGB(F.x, F.y, du, dv, 0.35)), ring(F.x, F.y, du, dv, 2));
    }
    const setCornerRefs = (k, px, py, du, dv) => {
      const pts = [];
      for (const s2 of [-3, 3]) {
        pts.push([px + s2 * du[0], py + s2 * du[1]]);
        pts.push([px + s2 * dv[0], py + s2 * dv[1]]);
      }
      const wacc = new Float32Array(3);
      for (const [x, y] of pts) this.rgbAt(x, y, wacc, 0);
      setRefs(k, [wacc[0] / 4, wacc[1] / 4, wacc[2] / 4], ring(px, py, du, dv, 1.5));
    };
    if (R.BR) {
      P[kBR * 2] = R.BR.x;
      P[kBR * 2 + 1] = R.BR.y;
      state[kBR] = 1;
      setCornerRefs(kBR, R.BR.x, R.BR.y, du0, dv0);
    }

    // Global LS homography over everything located so far (seeded with virtual points
    // along the corner-to-corner lines whose directions are actually known), plus smooth
    // residual interpolation for lens distortion.
    const seedSrc = [];
    const seedDst = [];
    const knownU = { TL: !!(R.TL && R.TR), TR: !!(R.TL && R.TR), BL: !!(R.BL && R.BR) };
    const knownV = { TL: !!(R.TL && R.BL), BL: !!(R.TL && R.BL), TR: !!(R.TR && R.BR) };
    for (const key of ['TL', 'TR', 'BL']) {
      const F = R[key];
      if (!F) continue;
      const k = KEY[key];
      const dd = LDU.subarray(k * 4, k * 4 + 4);
      const gx = gxOf(k);
      const gy = gyOf(k);
      const offs = [[0, 0]];
      if (knownU[key]) offs.push([3, 0], [-3, 0]);
      if (knownV[key]) offs.push([0, 3], [0, -3]);
      for (const [a, b] of offs) {
        seedSrc.push([gx + a, gy + b]);
        seedDst.push([F.x + a * dd[0] + b * dd[2], F.y + a * dd[1] + b * dd[3]]);
      }
    }
    if (R.BR) {
      seedSrc.push([gxOf(kBR), gyOf(kBR)]);
      seedDst.push([R.BR.x, R.BR.y]);
    }
    let Hcur = fitHomography(seedSrc, seedDst);
    if (!Hcur) return { ok: false, reason: 'homography' };
    let fitCount = avail.length;
    const refit = () => {
      const src = seedSrc.slice();
      const dst = seedDst.slice();
      const wts = new Array(src.length).fill(0.3);
      for (let k = 0; k < NL; k++) {
        if (state[k] !== 1) continue;
        src.push([gxOf(k), gyOf(k)]);
        dst.push([P[k * 2], P[k * 2 + 1]]);
        wts.push(1);
      }
      const H = fitHomography(src, dst, wts);
      if (H) Hcur = H;
    };
    const t2 = [0, 0];
    const predict = (k) => {
      const i = k % nx;
      const j = (k / nx) | 0;
      const gx = gxOf(k);
      const gy = gyOf(k);
      applyH(Hcur, gx, gy, t2);
      const base = [t2[0], t2[1]];
      let sw = 0;
      let rx = 0;
      let ry = 0;
      for (let jj = Math.max(0, j - 2); jj <= Math.min(ny - 1, j + 2); jj++)
        for (let ii = Math.max(0, i - 2); ii <= Math.min(nx - 1, i + 2); ii++) {
          const kk = jj * nx + ii;
          if (state[kk] !== 1) continue;
          applyH(Hcur, gxOf(kk), gyOf(kk), t2);
          const w = 1 / ((ii - i) * (ii - i) + (jj - j) * (jj - j));
          sw += w;
          rx += w * (P[kk * 2] - t2[0]);
          ry += w * (P[kk * 2 + 1] - t2[1]);
        }
      if (sw > 0) {
        base[0] += rx / sw;
        base[1] += ry / sw;
      }
      const a = applyH(Hcur, gx + 1, gy, [0, 0]);
      const b = applyH(Hcur, gx, gy + 1, [0, 0]);
      applyH(Hcur, gx, gy, t2);
      return { p: base, du: [a[0] - t2[0], a[1] - t2[1]], dv: [b[0] - t2[0], b[1] - t2[1]] };
    };

    let foundCount = 0;
    let brFound = !!R.BR;
    for (const k of missingFinder) state[k] = 2; // no marker there; positions predicted later
    for (;;) {
      // next: the unknown point with the most located neighbours
      let pick = -1;
      let pscore = -1;
      for (let k = 0; k < NL; k++) {
        if (state[k]) continue;
        const i = k % nx;
        const j = (k / nx) | 0;
        let sc = 0;
        for (let jj = Math.max(0, j - 1); jj <= Math.min(ny - 1, j + 1); jj++)
          for (let ii = Math.max(0, i - 1); ii <= Math.min(nx - 1, i + 1); ii++) if (state[jj * nx + ii] === 1) sc += 4;
        for (let jj = Math.max(0, j - 2); jj <= Math.min(ny - 1, j + 2); jj++)
          for (let ii = Math.max(0, i - 2); ii <= Math.min(nx - 1, i + 2); ii++) if (state[jj * nx + ii] === 1) sc += 1;
        if (k === kBR) sc -= 3; // prefer more support before the corner
        if (sc > pscore) {
          pscore = sc;
          pick = k;
        }
      }
      if (pick < 0) break;
      const k = pick;
      const { p: pred, du, dv } = predict(k);
      const cell = (Math.hypot(du[0], du[1]) + Math.hypot(dv[0], dv[1])) / 2;
      const isBR = k === kBR;
      const res = this._search(pred, du, dv, cell * (isBR ? 1.6 : 0.95), isBR ? this._cornerScore : this._markerScore);
      if (res.score > 0.3 * contrast) {
        const fine = this._search([res.x, res.y], du, dv, cell * 0.3, isBR ? this._cornerScore : this._markerScore);
        const thr = isBR
          ? (this.lumaAt(fine.x, fine.y) + this.lumaAt(fine.x + 1.5 * du[0], fine.y + 1.5 * du[1])) / 2
          : this.lumaAt(fine.x, fine.y) - fine.score / 2;
        const c = this._centroid(fine.x, fine.y, (isBR ? 1.3 : 1.15) * cell, thr);
        const px = c ? c[0] : fine.x;
        const py = c ? c[1] : fine.y;
        P[k * 2] = px;
        P[k * 2 + 1] = py;
        state[k] = 1;
        if (isBR) {
          brFound = true;
          setCornerRefs(k, px, py, du, dv);
        } else {
          foundCount++;
          setRefs(k, Array.from(sampleRGB(px, py, du, dv, 0.2)), ring(px, py, du, dv, 1));
        }
        if (foundCount + avail.length >= fitCount + Math.max(2, fitCount >> 2)) {
          refit();
          fitCount = foundCount + avail.length;
        }
      } else {
        P[k * 2] = pred[0];
        P[k * 2 + 1] = pred[1];
        state[k] = 2;
      }
    }
    // A wrong corner (a false finder) predicts the lattice in the wrong place and almost no
    // markers are found. Reject such a geometry so the caller tries the next corner set.
    const markerRatio = foundCount / Math.max(1, NL - 4);
    if (NL - 4 >= 4 && markerRatio < 0.6) return { ok: false, reason: 'lattice', markers: markerRatio };
    // corners that were never located take their position from the final fit
    for (const k of missingFinder) {
      applyH(Hcur, gxOf(k), gyOf(k), t2);
      P[k * 2] = t2[0];
      P[k * 2 + 1] = t2[1];
    }
    const Hg = homography(
      [[3.5, 3.5], [X - 3.5, 3.5], [X - 3.5, Y - 3.5], [3.5, Y - 3.5]],
      [kTL, kTR, kBR, kBL].map((k) => [P[k * 2], P[k * 2 + 1]]),
    );
    if (!Hg) return { ok: false, reason: 'homography' };
    // fill refs of unfound points from neighbours
    for (let k = 0; k < NL; k++) {
      if (state[k] === 1) continue;
      const i = k % nx;
      const j = (k / nx) | 0;
      const acc = new Float32Array(6);
      let cnt = 0;
      for (let r = 1; r < Math.max(nx, ny) && !cnt; r++)
        for (let jj = Math.max(0, j - r); jj <= Math.min(ny - 1, j + r); jj++)
          for (let ii = Math.max(0, i - r); ii <= Math.min(nx - 1, i + r); ii++) {
            const kk = jj * nx + ii;
            if (state[kk] !== 1) continue;
            for (let c = 0; c < 6; c++) acc[c] += refs[kk * 6 + c];
            cnt++;
          }
      if (cnt) for (let c = 0; c < 6; c++) refs[k * 6 + c] = acc[c] / cnt;
    }

    // per-quad homographies
    const qn = (nx - 1) * (ny - 1);
    const QH = new Array(qn);
    for (let j = 0; j < ny - 1; j++)
      for (let i = 0; i < nx - 1; i++) {
        const k00 = j * nx + i;
        const k10 = k00 + 1;
        const k01 = k00 + nx;
        const k11 = k01 + 1;
        const src = [
          [lx[i] + 0.5, ly[j] + 0.5],
          [lx[i + 1] + 0.5, ly[j] + 0.5],
          [lx[i + 1] + 0.5, ly[j + 1] + 0.5],
          [lx[i] + 0.5, ly[j + 1] + 0.5],
        ];
        const dst = [k00, k10, k11, k01].map((k) => [P[k * 2], P[k * 2 + 1]]);
        QH[j * (nx - 1) + i] = homography(src, dst) || Hg;
      }
    const qiOf = new Int32Array(X);
    for (let x = 0, i = 0; x < X; x++) {
      while (i < nx - 2 && x >= lx[i + 1]) i++;
      qiOf[x] = i;
    }
    const qjOf = new Int32Array(Y);
    for (let y = 0, j = 0; y < Y; y++) {
      while (j < ny - 2 && y >= ly[j + 1]) j++;
      qjOf[y] = j;
    }

    // per-quad average cell vectors
    const QDU = new Float64Array(qn * 4);
    for (let j = 0; j < ny - 1; j++)
      for (let i = 0; i < nx - 1; i++) {
        const k00 = j * nx + i;
        const sx = lx[i + 1] - lx[i];
        const sy = ly[j + 1] - ly[j];
        const q = (j * (nx - 1) + i) * 4;
        QDU[q] = (P[(k00 + 1) * 2] - P[k00 * 2] + P[(k00 + nx + 1) * 2] - P[(k00 + nx) * 2]) / (2 * sx);
        QDU[q + 1] = (P[(k00 + 1) * 2 + 1] - P[k00 * 2 + 1] + P[(k00 + nx + 1) * 2 + 1] - P[(k00 + nx) * 2 + 1]) / (2 * sx);
        QDU[q + 2] = (P[(k00 + nx) * 2] - P[k00 * 2] + P[(k00 + nx + 1) * 2] - P[(k00 + 1) * 2]) / (2 * sy);
        QDU[q + 3] = (P[(k00 + nx) * 2 + 1] - P[k00 * 2 + 1] + P[(k00 + nx + 1) * 2 + 1] - P[(k00 + 1) * 2 + 1]) / (2 * sy);
      }

    // only the slots of codewords still missing for this frame
    const { dataOrder } = layout;
    const D = dataOrder.length;
    const cpc = cellsPerCodeword(n, MODES[mode].bpc);
    const slotList = [];
    for (let j = 0; j < count; j++) if (!doneIdx.has(j)) for (let c = j * cpc; c < (j + 1) * cpc; c++) slotList.push(c);
    const slots = Int32Array.from(slotList);
    const norm = new Float32Array(D * 3);
    const p = [0, 0];
    const du = [0, 0];
    const dv = [0, 0];
    const acc3 = new Float32Array(3);
    for (let si = 0; si < slots.length; si++) {
      const s = slots[si];
      const cidx = dataOrder[s];
      const cx = cidx % X;
      const cy = (cidx / X) | 0;
      const qi = qiOf[cx];
      const qj = qjOf[cy];
      const q = qj * (nx - 1) + qi;
      applyH(QH[q], cx + 0.5, cy + 0.5, p);
      du[0] = QDU[q * 4] * 0.18;
      du[1] = QDU[q * 4 + 1] * 0.18;
      dv[0] = QDU[q * 4 + 2] * 0.18;
      dv[1] = QDU[q * 4 + 3] * 0.18;
      acc3[0] = acc3[1] = acc3[2] = 0;
      this.rgbAt(p[0], p[1], acc3, 0);
      acc3[0] *= 2;
      acc3[1] *= 2;
      acc3[2] *= 2;
      this.rgbAt(p[0] + du[0], p[1] + du[1], acc3, 0);
      this.rgbAt(p[0] - du[0], p[1] - du[1], acc3, 0);
      this.rgbAt(p[0] + dv[0], p[1] + dv[1], acc3, 0);
      this.rgbAt(p[0] - dv[0], p[1] - dv[1], acc3, 0);
      // reference interpolation
      const fx = Math.min(1, Math.max(0, (cx - lx[qi]) / (lx[qi + 1] - lx[qi])));
      const fy = Math.min(1, Math.max(0, (cy - ly[qj]) / (ly[qj + 1] - ly[qj])));
      const k00 = qj * nx + qi;
      const w00 = (1 - fx) * (1 - fy);
      const w10 = fx * (1 - fy);
      const w01 = (1 - fx) * fy;
      const w11 = fx * fy;
      for (let ch = 0; ch < 3; ch++) {
        const wr =
          refs[k00 * 6 + ch] * w00 + refs[(k00 + 1) * 6 + ch] * w10 + refs[(k00 + nx) * 6 + ch] * w01 + refs[(k00 + nx + 1) * 6 + ch] * w11;
        const kr =
          refs[k00 * 6 + 3 + ch] * w00 +
          refs[(k00 + 1) * 6 + 3 + ch] * w10 +
          refs[(k00 + nx) * 6 + 3 + ch] * w01 +
          refs[(k00 + nx + 1) * 6 + 3 + ch] * w11;
        norm[s * 3 + ch] = (acc3[ch] / 6 - kr) / Math.max(wr - kr, 12);
      }
    }

    // classification: k-means seeded by the palette, then local per-channel gain
    // correction (moiré, vignetting, uneven light) estimated from the cells' own labels
    const pal = MODES[mode].palette;
    const NC = pal.length;
    const cent = new Float32Array(NC * 3);
    for (let k = 0; k < NC; k++)
      for (let ch = 0; ch < 3; ch++) cent[k * 3 + ch] = (pal[k][ch] - INK[ch]) / (PAPER[ch] - INK[ch]);
    const symbols = new Uint8Array(D);
    const conf = new Float32Array(D);
    const sums = new Float64Array(NC * 4);
    const work = new Float32Array(D * 3);
    work.set(norm);
    const assign = (updateCentroids) => {
      sums.fill(0);
      for (let si = 0; si < slots.length; si++) {
        const s = slots[si];
        const r = work[s * 3];
        const gg = work[s * 3 + 1];
        const b = work[s * 3 + 2];
        let b1 = 1e9;
        let b2 = 1e9;
        let bi = 0;
        for (let k = 0; k < NC; k++) {
          const dr = r - cent[k * 3];
          const dg = gg - cent[k * 3 + 1];
          const db = b - cent[k * 3 + 2];
          const d = dr * dr + dg * dg + db * db;
          if (d < b1) {
            b2 = b1;
            b1 = d;
            bi = k;
          } else if (d < b2) b2 = d;
        }
        symbols[s] = bi;
        conf[s] = Math.sqrt(b2) - Math.sqrt(b1);
        sums[bi * 4] += r;
        sums[bi * 4 + 1] += gg;
        sums[bi * 4 + 2] += b;
        sums[bi * 4 + 3]++;
      }
      if (!updateCentroids) return;
      for (let k = 0; k < NC; k++) {
        const cnt = sums[k * 4 + 3];
        if (cnt < 8) continue;
        for (let ch = 0; ch < 3; ch++) cent[k * 3 + ch] = sums[k * 4 + ch] / cnt;
      }
    };
    const GB = 4; // gain block size in cells
    const gbw = Math.ceil(X / GB);
    const gbh = Math.ceil(Y / GB);
    const gacc = new Float64Array(gbw * gbh * 6);
    const gain = new Float32Array(gbw * gbh * 3);
    const cellX = new Int32Array(D);
    const cellY = new Int32Array(D);
    for (let s2 = 0; s2 < D; s2++) {
      cellX[s2] = dataOrder[s2] % X;
      cellY[s2] = (dataOrder[s2] / X) | 0;
    }
    const gainPass = () => {
      gacc.fill(0);
      for (let si = 0; si < slots.length; si++) {
        const s2 = slots[si];
        const b = ((cellY[s2] / GB) | 0) * gbw + ((cellX[s2] / GB) | 0);
        const k = symbols[s2];
        for (let ch = 0; ch < 3; ch++) {
          const c = cent[k * 3 + ch];
          gacc[b * 6 + ch] += norm[s2 * 3 + ch] * c;
          gacc[b * 6 + 3 + ch] += c * c;
        }
      }
      const lam = 0.6; // pulls sparse blocks towards unity gain
      for (let b = 0; b < gbw * gbh; b++)
        for (let ch = 0; ch < 3; ch++) gain[b * 3 + ch] = (gacc[b * 6 + ch] + lam) / (gacc[b * 6 + 3 + ch] + lam);
      // apply, bilinear between block centres
      for (let si = 0; si < slots.length; si++) {
        const s2 = slots[si];
        const fx = Math.min(gbw - 1.0001, Math.max(0, (cellX[s2] + 0.5) / GB - 0.5));
        const fy = Math.min(gbh - 1.0001, Math.max(0, (cellY[s2] + 0.5) / GB - 0.5));
        const ix = Math.min(gbw - 2, fx | 0);
        const iy = Math.min(gbh - 2, fy | 0);
        const ax = gbw > 1 ? fx - ix : 0;
        const ay = gbh > 1 ? fy - iy : 0;
        const b00 = iy * gbw + ix;
        const b10 = gbw > 1 ? b00 + 1 : b00;
        const b01 = gbh > 1 ? b00 + gbw : b00;
        const b11 = gbw > 1 ? b01 + 1 : b01;
        for (let ch = 0; ch < 3; ch++) {
          const g =
            gain[b00 * 3 + ch] * (1 - ax) * (1 - ay) +
            gain[b10 * 3 + ch] * ax * (1 - ay) +
            gain[b01 * 3 + ch] * (1 - ax) * ay +
            gain[b11 * 3 + ch] * ax * ay;
          work[s2 * 3 + ch] = norm[s2 * 3 + ch] / Math.max(0.25, g);
        }
      }
    };
    assign(true);
    gainPass();
    assign(true);
    gainPass();
    assign(false);

    const cfg = { mode, n, nsym };
    const cws = decodeCodewords(layout, cfg, symbols, conf, count, doneIdx);
    const corners = [
      [0, 0],
      [X, 0],
      [X, Y],
      [0, Y],
    ].map(([gx, gy]) => applyH(Hg, gx, gy, [0, 0]));
    const okCount = cws.reduce((a, c) => a + (c.ok ? 1 : 0), 0);
    return {
      ok: true,
      header: hdr,
      frameKey,
      codewords: cws,
      stats: {
        cellPx,
        corners,
        markers: foundCount / Math.max(1, NL - 4),
        cornerFound: brFound,
        cwOk: okCount,
        cwTotal: count - doneIdx.size,
        cwCount: count,
        corrected: cws.reduce((a, c) => a + (c.ok ? c.corrected : 0), 0),
      },
    };
  }
}
