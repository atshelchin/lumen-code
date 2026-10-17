// Writes simulated camera frames as raw RGBA (+ width, height, JS-decoded codeword count)
// for the Rust interop/benchmark test.
import { mkdirSync, writeFileSync } from 'node:fs';
import { makeLayout, chooseCodeword, codewordsPerFrame } from '../src/core/layout.js';
import { encodeFrameCells, sealPacket } from '../src/core/frame.js';
import { Renderer } from '../src/core/render.js';
import { Scanner } from '../src/core/scan.js';
import { simulateCamera } from './camera.js';
import { Rng } from '../src/core/util.js';
mkdirSync('test/out/raw', { recursive: true });
const base = { camW: 1920, camH: 1080, fill: 0.8, blur: 1.0, noise: 4, rot: 0.1, tilt: [0.12, -0.08], k1: -0.04,
  cross: [0.85, 0.12, 0.05, 0.1, 0.82, 0.1, 0.04, 0.15, 0.85], gains: [1.08, 1.0, 0.86], black: 14, vignette: 0.3, subsample: 1 };
const cases = [['phone-c4', 86, 152, 1, 8, {}], ['phone-c8', 86, 152, 2, 8, {}], ['desktop-c8', 249, 153, 2, 7, { fill: 0.62 }], ['tilted-c4', 86, 152, 1, 8, { tilt: [0.3, 0.2], rot: 0.4, fill: 0.6, moire: 0.12, bloom: 1 }]];
for (const [name, X, Y, mode, s, extra] of cases) {
  const layout = makeLayout(X, Y);
  const { n, nsym } = chooseCodeword(layout, mode, 1);
  const m = codewordsPerFrame(layout, mode, n);
  const rng = new Rng(7);
  const msgs = [];
  for (let i = 0; i < m; i++) msgs.push(sealPacket(new Uint8Array(n - nsym).map(() => rng.next() & 255)));
  const r = new Renderer(layout, mode, s);
  const rgba = new Uint8Array(r.width * r.height * 4);
  r.draw(encodeFrameCells(layout, { mode, n, nsym }, 5, msgs), new Uint32Array(rgba.buffer));
  const img = simulateCamera({ width: r.width, height: r.height, data: rgba }, { ...base, ...extra });
  const res = new Scanner().scan(img);
  const ok = res.ok ? res.codewords.filter((c) => c.ok).length : 0;
  const head = new Uint8Array(12);
  new DataView(head.buffer).setUint32(0, img.width, true);
  new DataView(head.buffer).setUint32(4, img.height, true);
  new DataView(head.buffer).setUint32(8, ok, true);
  writeFileSync(`test/out/raw/${name}.rgba`, Buffer.concat([head, Buffer.from(img.data)]));
  console.log(name, `JS ${ok}/${m}`);
}
