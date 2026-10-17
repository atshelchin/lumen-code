// One-shot simulation: render a frame, pass it through the camera model, scan it.
// usage: node test/sim.js X Y mode cellPx [key=value ...]
import { makeLayout, chooseCodeword, codewordsPerFrame } from '../src/core/layout.js';
import { encodeFrameCells, sealPacket } from '../src/core/frame.js';
import { Renderer } from '../src/core/render.js';
import { Scanner } from '../src/core/scan.js';
import { simulateCamera } from './camera.js';
import { writePNG } from './png.js';
import { Rng } from '../src/core/util.js';

export function runSim(X, Y, mode, s, cam, ecc = 1, save = null) {
  const layout = makeLayout(X, Y);
  const { n, nsym } = chooseCodeword(layout, mode, ecc);
  const m = codewordsPerFrame(layout, mode, n);
  const rng = new Rng(cam.seed || 3);
  const msgs = [];
  for (let i = 0; i < m; i++) msgs.push(sealPacket(new Uint8Array(n - nsym).map(() => rng.next() & 255)));
  const cells = encodeFrameCells(layout, { mode, n, nsym }, 42, msgs);
  const r = new Renderer(layout, mode, s);
  const rgba = new Uint8Array(r.width * r.height * 4);
  r.draw(cells, new Uint32Array(rgba.buffer));
  const t0 = performance.now();
  const img = simulateCamera({ width: r.width, height: r.height, data: rgba }, cam);
  const tSim = performance.now() - t0;
  if (save) writePNG(save, img.width, img.height, img.data);
  const sc = new Scanner();
  const t1 = performance.now();
  const res = sc.scan(img);
  const tScan = performance.now() - t1;
  let good = 0;
  if (res.ok) {
    for (const c of res.codewords) if (c.ok && c.bytes.every((v, i) => v === msgs[c.index][i])) good++;
  }
  return { ok: res.ok, reason: res.reason, candidates: res.candidates, good, total: m, stats: res.stats, tScan, tSim };
}

if ((process.argv[1] || '').endsWith('sim.js')) {
  const [X, Y, mode, s] = process.argv.slice(2, 6).map(Number);
  const cam = { camW: 1280, camH: 720, fill: 0.85, blur: 0.8, noise: 3 };
  let ecc = 1;
  for (const kv of process.argv.slice(6)) {
    const [k, v] = kv.split('=');
    if (k === 'ecc') ecc = Number(v);
    else cam[k] = v.includes(',') ? v.split(',').map(Number) : Number(v);
  }
  const res = runSim(X, Y, mode, s, cam, ecc, 'test/out/sim.png');
  console.log(JSON.stringify(res, (k, v) => (typeof v === 'number' ? Math.round(v * 100) / 100 : v)));
}
