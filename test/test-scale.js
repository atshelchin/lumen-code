// Adaptive resolution: decoding a downscaled frame (cells ~6 px) vs the full frame.
import { makeLayout, chooseCodeword, codewordsPerFrame } from '../src/core/layout.js';
import { encodeFrameCells, sealPacket } from '../src/core/frame.js';
import { Renderer } from '../src/core/render.js';
import { Scanner } from '../src/core/scan.js';
import { simulateCamera } from './camera.js';
import { Rng } from '../src/core/util.js';

function downscale(img, k) {
  // area average (what a GPU drawImage with smoothing approximates)
  const W = Math.round(img.width * k), H = Math.round(img.height * k);
  const out = new Uint8ClampedArray(W * H * 4);
  const inv = 1 / k;
  for (let y = 0; y < H; y++) {
    const y0 = Math.floor(y * inv), y1 = Math.min(img.height, Math.floor((y + 1) * inv)) || y0 + 1;
    for (let x = 0; x < W; x++) {
      const x0 = Math.floor(x * inv), x1 = Math.min(img.width, Math.floor((x + 1) * inv)) || x0 + 1;
      let r = 0, g = 0, b = 0, n = 0;
      for (let yy = y0; yy < Math.max(y1, y0 + 1); yy++) for (let xx = x0; xx < Math.max(x1, x0 + 1); xx++) {
        const i = (yy * img.width + xx) * 4; r += img.data[i]; g += img.data[i + 1]; b += img.data[i + 2]; n++;
      }
      const o = (y * W + x) * 4; out[o] = r / n; out[o + 1] = g / n; out[o + 2] = b / n; out[o + 3] = 255;
    }
  }
  return { width: W, height: H, data: out };
}

const base = { camW: 1080, camH: 1920, blur: 1.0, noise: 4, rot: 0.06, tilt: [0.08, -0.05], k1: -0.04,
  cross: [0.85, 0.12, 0.05, 0.1, 0.82, 0.1, 0.04, 0.15, 0.85], gains: [1.08, 1.0, 0.9], black: 14, vignette: 0.3, subsample: 1 };
const time = (sc, img) => { let t = 0, r; for (let i = 0; i < 5; i++) { const t0 = performance.now(); r = sc.scan(img); if (i) t += performance.now() - t0; } return [t / 4, r]; };
for (const [name, X, Y, mode, fill, extra] of [['phone C4 close', 86, 152, 1, 0.92, {}], ['phone C8 close', 86, 152, 2, 0.92, {}], ['desktop-portrait C4', 173, 307, 1, 0.92, {}], ['phone C4 close + moire', 86, 152, 1, 0.9, { moire: 0.12, bloom: 1 }]]) {
  const L = makeLayout(X, Y); const { n, nsym } = chooseCodeword(L, mode, 1); const m = codewordsPerFrame(L, mode, n);
  const rng = new Rng(9); const msgs = []; for (let i = 0; i < m; i++) msgs.push(sealPacket(new Uint8Array(n - nsym).map(() => rng.next() & 255)));
  const r = new Renderer(L, mode, 6); const px = new Uint8Array(r.width * r.height * 4);
  r.draw(encodeFrameCells(L, { mode, n, nsym }, 3, msgs), new Uint32Array(px.buffer));
  const img = simulateCamera({ width: r.width, height: r.height, data: px }, { ...base, fill, ...extra });
  const sc = new Scanner();
  const [tf, rf] = time(sc, img);
  const cell = rf.ok ? rf.stats.cellPx : 0;
  const k = Math.min(1, Math.max(0.35, 6 / cell));
  const small = downscale(img, k);
  const [ts, rs] = time(sc, small);
  const nf = rf.ok ? rf.codewords.filter((c) => c.ok).length : 0, ns = rs.ok ? rs.codewords.filter((c) => c.ok).length : 0;
  console.log(`${name.padEnd(24)} full ${img.width}x${img.height} cell=${cell.toFixed(1)}px ${nf}/${m} ${tf.toFixed(1)}ms | scaled x${k.toFixed(2)} ${small.width}x${small.height} ${ns}/${m} ${ts.toFixed(1)}ms  (${(tf / ts).toFixed(1)}x faster, ${(100 * k * k).toFixed(0)}% pixels)`);
}
