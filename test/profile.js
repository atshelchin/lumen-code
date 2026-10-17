// Per-stage timing of Scanner.scan on a simulated 1080p capture.
import { makeLayout, chooseCodeword, codewordsPerFrame } from '../src/core/layout.js';
import { encodeFrameCells, sealPacket } from '../src/core/frame.js';
import { Renderer } from '../src/core/render.js';
import { Scanner } from '../src/core/scan.js';
import { simulateCamera } from './camera.js';
import { Rng } from '../src/core/util.js';
const [X, Y, mode, s, W, H] = (process.argv[2] || '249,153,2,7,1920,1080').split(',').map(Number);
const layout = makeLayout(X, Y);
const { n, nsym } = chooseCodeword(layout, mode, 1);
const m = codewordsPerFrame(layout, mode, n);
const rng = new Rng(3);
const msgs = []; for (let i = 0; i < m; i++) msgs.push(sealPacket(new Uint8Array(n - nsym).map(() => rng.next() & 255)));
const r = new Renderer(layout, mode, s);
const rgba = new Uint8Array(r.width * r.height * 4);
r.draw(encodeFrameCells(layout, { mode, n, nsym }, 1, msgs), new Uint32Array(rgba.buffer));
const img = simulateCamera({ width: r.width, height: r.height, data: rgba }, { camW: W, camH: H, fill: 0.62, blur: 0.9, noise: 3, rot: 0.04, tilt: [0.05, 0.03], k1: -0.03, subsample: 1 });
const sc = new Scanner();
const T = {};
const orig = {};
for (const k of ['_prep', 'findFinders', 'selectSets', '_tryCorners', '_decode']) {
  orig[k] = sc[k].bind(sc);
  sc[k] = (...a) => { const t0 = performance.now(); const v = orig[k](...a); T[k] = (T[k] || 0) + performance.now() - t0; return v; };
}
let res;
for (let i = 0; i < 12; i++) { if (i === 2) for (const k in T) T[k] = 0; res = sc.scan(img); }
for (const k in T) T[k] = +(T[k] / 10).toFixed(1);
console.log(`${X}x${Y} mode${mode} @${W}x${H}: ok=${res.ok} cw=${res.stats?.cwOk}/${res.stats?.cwTotal} cell=${res.stats?.cellPx.toFixed(2)}`, T);

// already-decoded frame (skip all) and partially decoded frame (2 missing codewords)
const key = res.frameKey;
const all = { [key]: Array.from({ length: res.stats.cwTotal }, (_, i) => i) };
const part = { [key]: Array.from({ length: res.stats.cwTotal }, (_, i) => i).filter((i) => i % 20 !== 3) };
for (const [label, skip] of [['skip-all', all], ['partial', part]]) {
  let t = 0, r;
  for (let i = 0; i < 10; i++) { const t0 = performance.now(); r = sc.scan(img, { skip }); if (i >= 2) t += performance.now() - t0; }
  console.log(`  ${label}: ${(t / 8).toFixed(1)} ms, skipped=${!!r.skipped}, decoded ${r.stats.cwOk}/${r.stats.cwTotal}`);
}
