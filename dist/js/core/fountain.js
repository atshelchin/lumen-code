// Raptor-style fountain code with inactivation decoding.
//
// Intermediate symbols C[0..L): K source, S LDPC parity, P "PI" (dense) parity.
//   PI h    = XOR of a pseudo-random half of the source symbols
//   LDPC s  = XOR of ~3K/S source symbols + two PI symbols
// Encoded symbol x = XOR of d LT symbols (from C[0..W), W = K+S, prime) + 2–3 PI symbols.
// The decoder peels the sparse part, inactivating columns when it stalls, then solves
// the small dense remainder with GF(2) Gauss–Jordan. A symbolic pass checks rank before
// any payload bytes are touched, so retrying with more packets is cheap.

import { Rng, mix32, isPrime, nextPrime, xor32 } from './util.js?v=a6f3ca6730';

// Degree distribution (cumulative, out of 2^20), RaptorQ-like.
const DEG = [
  0, 5243, 529531, 704294, 791675, 844104, 879057, 904023, 922747, 937311, 948962, 958494,
  966438, 973160, 978921, 983914, 988283, 992138, 995565, 998631, 1001391, 1003887, 1006157,
  1008229, 1010129, 1011876, 1013490, 1014983, 1016370, 1017662, 1048576,
];

function bitLength(k) {
  return 32 - Math.clz32(k);
}

// All parameters use integer arithmetic only (portable, bit-exact across platforms).
export function fountainParams(K) {
  let X = 2;
  while (X * (X - 1) < 2 * K) X++; // smallest X >= 2 with X(X-1) >= 2K
  let S = Math.max(3, Math.floor((K + 99) / 100) + X);
  while (!isPrime(K + S)) S++; // W = K + S is prime
  const W = K + S;
  const P = 12 + bitLength(K);
  const P1 = nextPrime(P + 1);
  return { K, S, P, P1, W, L: W + P };
}

function strideOf(T) {
  return (T + 3) & ~3;
}

// LDPC membership: source i -> three parity rows
function ldpcRows(K, S) {
  const rows = Array.from({ length: S }, () => []);
  for (let i = 0; i < K; i++) {
    let a = 1 + (Math.floor(i / S) % (S - 1));
    if ((2 * a) % S === 0) a = 1;
    const b = i % S;
    rows[b].push(i);
    rows[(b + a) % S].push(i);
    rows[(b + 2 * a) % S].push(i);
  }
  return rows;
}

// PI row h: pseudo-random half of sources
function hdpcMembers(K, h, seed) {
  const rng = new Rng(mix32(seed ^ Math.imul(h + 1, 0x5bd1e995)));
  const out = [];
  for (let i = 0; i < K; i += 32) {
    let bits = rng.next();
    const lim = Math.min(32, K - i);
    for (let b = 0; b < lim; b++) {
      if (bits & 1) out.push(i + b);
      bits >>>= 1;
    }
  }
  return out;
}

function tuple(p, seed, esi) {
  const { W, P, P1 } = p;
  const r = new Rng(mix32(seed ^ Math.imul(esi + 1, 0x9e3779b1)));
  const v = r.next() & 0xfffff;
  let d = 1;
  while (v >= DEG[d]) d++;
  d = Math.max(1, Math.min(d, W - 2));
  const a = 1 + (r.next() % (W - 1));
  let b = r.next() % W;
  const lt = [b];
  for (let j = 1; j < d; j++) {
    b = (b + a) % W;
    lt.push(b);
  }
  const d1 = d < 4 ? 2 + (r.next() & 1) : 2;
  const a1 = 1 + (r.next() % (P1 - 1));
  let b1 = r.next() % P1;
  const pi = [];
  for (let j = 0; j < d1; j++) {
    while (b1 >= P) b1 = (b1 + a1) % P1;
    pi.push(b1);
    b1 = (b1 + a1) % P1;
  }
  return { lt, pi };
}

export class FountainEncoder {
  constructor(payload, T, seed) {
    const K = Math.max(1, Math.ceil(payload.length / T));
    this.p = fountainParams(K);
    this.T = T;
    this.seed = seed >>> 0;
    const { S, P, W, L } = this.p;
    const ts = strideOf(T);
    this.ts = ts;
    this.buf = new Uint8Array(L * ts);
    this.words = new Uint32Array(this.buf.buffer);
    for (let i = 0; i < K; i++) {
      this.buf.set(payload.subarray(i * T, Math.min(payload.length, (i + 1) * T)), i * ts);
    }
    for (let h = 0; h < P; h++) {
      const dst = this.sym32(W + h);
      for (const i of hdpcMembers(K, h, this.seed)) xor32(dst, this.sym32(i));
    }
    const ldpc = ldpcRows(K, S);
    for (let s = 0; s < S; s++) {
      const dst = this.sym32(K + s);
      for (const i of ldpc[s]) xor32(dst, this.sym32(i));
      xor32(dst, this.sym32(W + (s % P)));
      xor32(dst, this.sym32(W + ((s + 1) % P)));
    }
    this._tmp = new Uint32Array(ts >> 2);
  }

  get K() {
    return this.p.K;
  }

  sym32(i) {
    const w = this.ts >> 2;
    return this.words.subarray(i * w, (i + 1) * w);
  }

  // Writes encoded symbol `esi` (T bytes) into out (Uint8Array).
  encode(esi, out) {
    const { lt, pi } = tuple(this.p, this.seed, esi);
    const t = this._tmp;
    t.fill(0);
    for (const c of lt) xor32(t, this.sym32(c));
    for (const j of pi) xor32(t, this.sym32(this.p.W + j));
    out.set(new Uint8Array(t.buffer, 0, this.T));
    return out;
  }
}

const ACTIVE = 0;
const PIVOT = 1;
const INACTIVE = 2;

export class FountainDecoder {
  constructor(K, T, seed) {
    this.p = fountainParams(K);
    this.T = T;
    this.seed = seed >>> 0;
    this.ts = strideOf(T);
    this.esis = [];
    this.data = []; // Uint32Array per received symbol
    this.seen = new Set();
    this.ldpc = ldpcRows(K, this.p.S);
    this.hdpc = null; // lazy
    this.lastStats = null;
  }

  get K() {
    return this.p.K;
  }

  get count() {
    return this.esis.length;
  }

  add(esi, bytes) {
    if (this.seen.has(esi)) return false;
    this.seen.add(esi);
    const d = new Uint8Array(this.ts);
    d.set(bytes.subarray(0, this.T));
    this.esis.push(esi);
    this.data.push(new Uint32Array(d.buffer));
    return true;
  }

  // Returns Uint8Array of K*T bytes on success, null if not yet solvable.
  decode() {
    const { K, S, P, W } = this.p;
    if (this.count < K) return null;
    if (!this.hdpc) {
      this.hdpc = [];
      for (let h = 0; h < P; h++) this.hdpc.push(hdpcMembers(K, h, this.seed));
    }

    // ---- build sparse rows: LDPC rows then received rows
    const N = this.count;
    const R = S + N;
    const rowLt = new Array(R);
    const rowPi = new Array(R);
    for (let s = 0; s < S; s++) {
      rowLt[s] = this.ldpc[s].concat([K + s]);
      rowPi[s] = [s % P, (s + 1) % P];
    }
    for (let i = 0; i < N; i++) {
      const t = tuple(this.p, this.seed, this.esis[i]);
      rowLt[S + i] = t.lt;
      rowPi[S + i] = t.pi;
    }
    const rhsOf = (r) => (r >= S ? this.data[r - S] : null);

    // column -> rows (CSR)
    const colCount = new Int32Array(W + 1);
    for (let r = 0; r < R; r++) for (const c of rowLt[r]) colCount[c + 1]++;
    for (let c = 0; c < W; c++) colCount[c + 1] += colCount[c];
    const colRows = new Int32Array(colCount[W]);
    const fill = colCount.slice(0, W);
    for (let r = 0; r < R; r++) for (const c of rowLt[r]) colRows[fill[c]++] = r;

    // ---- phase 1: peeling with inactivation
    const deg = new Int32Array(R);
    let maxDeg = 0;
    for (let r = 0; r < R; r++) {
      deg[r] = rowLt[r].length;
      if (deg[r] > maxDeg) maxDeg = deg[r];
    }
    const buckets = Array.from({ length: maxDeg + 1 }, () => []);
    for (let r = R - 1; r >= 0; r--) buckets[deg[r]].push(r);
    const used = new Uint8Array(R);
    const state = new Uint8Array(W);
    const inactIdx = new Int32Array(W).fill(-1);
    const pivotRow = new Int32Array(W).fill(-1);
    const pos = new Int32Array(W).fill(-1);
    const order = [];
    let nInact = 0;
    let active = W;

    const touch = (c, except) => {
      for (let k = colCount[c]; k < colCount[c + 1]; k++) {
        const r2 = colRows[k];
        if (r2 === except || used[r2]) continue;
        const d = --deg[r2];
        if (d >= 1) buckets[d].push(r2);
      }
    };
    const inactivate = (c) => {
      state[c] = INACTIVE;
      inactIdx[c] = nInact++;
      active--;
      touch(c, -1);
    };

    let bmin = 1;
    while (active > 0) {
      let r = -1;
      for (let b = 1; b <= maxDeg && r < 0; b++) {
        const bk = buckets[b];
        while (bk.length) {
          const cand = bk.pop();
          if (!used[cand] && deg[cand] === b) {
            r = cand;
            break;
          }
        }
      }
      if (r < 0) break;
      if (deg[r] > 1) {
        // keep the column with the fewest rows, inactivate the rest
        let keep = -1;
        let best = 1e9;
        for (const c of rowLt[r]) {
          if (state[c] !== ACTIVE) continue;
          const cnt = colCount[c + 1] - colCount[c];
          if (cnt < best) {
            best = cnt;
            keep = c;
          }
        }
        for (const c of rowLt[r]) if (state[c] === ACTIVE && c !== keep) inactivate(c);
      }
      let c = -1;
      for (const cc of rowLt[r]) if (state[cc] === ACTIVE) c = cc;
      if (c < 0) continue; // degree dropped to 0 meanwhile
      state[c] = PIVOT;
      used[r] = 1;
      pivotRow[c] = r;
      pos[c] = order.length;
      order.push(c);
      active--;
      touch(c, r);
    }
    for (let c = 0; c < W; c++) if (state[c] === ACTIVE) inactivate(c);
    void bmin;

    const I = P + nInact;
    const wd = (I + 31) >>> 5;
    const npiv = order.length;

    // ---- phase 2 (symbolic): inactive-dependency bitsets
    const bits = new Uint32Array(npiv * wd);
    const tog = (arr, off, k) => {
      arr[off + (k >>> 5)] ^= 1 << (k & 31);
    };
    for (let k = 0; k < npiv; k++) {
      const c = order[k];
      const r = pivotRow[c];
      const off = k * wd;
      for (const c2 of rowLt[r]) {
        if (c2 === c) continue;
        if (state[c2] === PIVOT) {
          const o2 = pos[c2] * wd;
          for (let w = 0; w < wd; w++) bits[off + w] ^= bits[o2 + w];
        } else tog(bits, off, P + inactIdx[c2]);
      }
      for (const j of rowPi[r]) tog(bits, off, j);
    }

    // leftover equations: unused sparse rows + PI (HDPC) rows
    const eqRows = []; // {lt, pi, rhsRow}
    for (let r = 0; r < R; r++) if (!used[r]) eqRows.push({ lt: rowLt[r], pi: rowPi[r], r });
    for (let h = 0; h < P; h++) eqRows.push({ lt: this.hdpc[h], pi: [h], r: -1 });
    const E = eqRows.length;
    this.lastStats = { N, inactive: I, pivots: npiv, equations: E };
    if (E < I) return null;

    const M = new Uint32Array(E * wd);
    for (let e = 0; e < E; e++) {
      const off = e * wd;
      const { lt, pi } = eqRows[e];
      for (const c of lt) {
        if (state[c] === PIVOT) {
          const o2 = pos[c] * wd;
          for (let w = 0; w < wd; w++) M[off + w] ^= bits[o2 + w];
        } else tog(M, off, P + inactIdx[c]);
      }
      for (const j of pi) tog(M, off, j);
    }

    // Gauss–Jordan, recording row operations
    const perm = new Int32Array(E);
    for (let e = 0; e < E; e++) perm[e] = e;
    const ops = []; // pairs dst, src (physical row indices)
    for (let col = 0; col < I; col++) {
      const w = col >>> 5;
      const m = 1 << (col & 31);
      let p = -1;
      for (let i = col; i < E; i++) {
        if (M[perm[i] * wd + w] & m) {
          p = i;
          break;
        }
      }
      if (p < 0) return null; // rank deficient: need more symbols
      const t = perm[p];
      perm[p] = perm[col];
      perm[col] = t;
      const po = t * wd;
      for (let i = 0; i < E; i++) {
        if (i === col) continue;
        const q = perm[i];
        const qo = q * wd;
        if (M[qo + w] & m) {
          for (let x = w; x < wd; x++) M[qo + x] ^= M[po + x];
          ops.push(q, t);
        }
      }
    }

    // ---- numeric phase
    const sw = this.ts >> 2;
    const D = new Uint32Array(npiv * sw);
    const dv = (k) => D.subarray(k * sw, (k + 1) * sw);
    for (let k = 0; k < npiv; k++) {
      const c = order[k];
      const r = pivotRow[c];
      const dst = dv(k);
      const rhs = rhsOf(r);
      if (rhs) dst.set(rhs);
      for (const c2 of rowLt[r]) if (c2 !== c && state[c2] === PIVOT) xor32(dst, dv(pos[c2]));
    }
    const Rv = new Uint32Array(E * sw);
    const rv = (e) => Rv.subarray(e * sw, (e + 1) * sw);
    for (let e = 0; e < E; e++) {
      const dst = rv(e);
      const { lt, r } = eqRows[e];
      const rhs = r >= 0 ? rhsOf(r) : null;
      if (rhs) dst.set(rhs);
      for (const c of lt) if (state[c] === PIVOT) xor32(dst, dv(pos[c]));
    }
    for (let i = 0; i < ops.length; i += 2) xor32(rv(ops[i]), rv(ops[i + 1]));
    const y = (k) => rv(perm[k]);

    // back-substitute pivots (reuse D)
    for (let k = 0; k < npiv; k++) {
      const c = order[k];
      const r = pivotRow[c];
      const dst = dv(k);
      const rhs = rhsOf(r);
      if (rhs) dst.set(rhs);
      else dst.fill(0);
      for (const c2 of rowLt[r]) {
        if (c2 === c) continue;
        if (state[c2] === PIVOT) xor32(dst, dv(pos[c2]));
        else xor32(dst, y(P + inactIdx[c2]));
      }
      for (const j of rowPi[r]) xor32(dst, y(j));
    }

    const T = this.T;
    const out = new Uint8Array(K * T);
    for (let i = 0; i < K; i++) {
      const src = state[i] === PIVOT ? dv(pos[i]) : y(P + inactIdx[i]);
      out.set(new Uint8Array(src.buffer, src.byteOffset, T), i * T);
    }
    return out;
  }
}

// exposed for test vectors and ports
export const _internals = { tuple, ldpcRows, hdpcMembers, DEG };
