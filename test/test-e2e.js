// End-to-end: file -> Sender frames -> render -> simulated camera (with drops and torn
// frames) -> Scanner -> Receiver -> file.
import { Sender, Receiver, preparePayload } from '../src/core/transfer.js';
import { Renderer } from '../src/core/render.js';
import { Scanner } from '../src/core/scan.js';
import { simulateCamera } from './camera.js';
import { Rng } from '../src/core/util.js';

async function transfer({ size, X, Y, mode, s, cam, dropRate = 0.25, tearRate = 0.2, compressible = false, label }) {
  const rng = new Rng(size + X);
  const file = new Uint8Array(size);
  if (compressible) {
    const words = ['alpha ', 'beta ', 'gamma ', 'delta ', 'lumen ', 'frame ', '\n'];
    let i = 0;
    while (i < size) for (const ch of new TextEncoder().encode(words[rng.next() % words.length])) if (i < size) file[i++] = ch;
  } else for (let i = 0; i < size; i++) file[i] = rng.next() & 255;

  const prepared = await preparePayload(file, 'test.bin');
  const sender = new Sender(prepared, { mode, ecc: 1, X, Y });
  const rx = new Receiver();
  const scanner = new Scanner();
  let prev = null;
  let shown = 0;
  let scanned = 0;
  let okCw = 0;
  let totalCw = 0;
  const t0 = performance.now();
  for (let f = 0; f < 400; f++) {
    const { cells, layout } = sender.nextFrame();
    shown++;
    if (rng.next() / 2 ** 32 < dropRate) continue;
    const r = new Renderer(layout, mode, s);
    const rgba = new Uint8Array(r.width * r.height * 4);
    r.draw(cells, new Uint32Array(rgba.buffer));
    let src = rgba;
    if (prev && rng.next() / 2 ** 32 < tearRate) {
      // rolling-shutter tear: top part from the previous frame
      src = rgba.slice();
      const cut = Math.floor(r.height * (0.2 + 0.6 * (rng.next() / 2 ** 32)));
      src.set(prev.subarray(0, cut * r.width * 4));
    }
    prev = rgba;
    const img = simulateCamera({ width: r.width, height: r.height, data: src }, { ...cam, seed: f + 1 });
    const res = scanner.scan(img);
    scanned++;
    if (!res.ok) continue;
    for (const c of res.codewords) {
      totalCw++;
      if (!c.ok) continue;
      okCw++;
      await rx.push(c.bytes);
    }
    const out = await rx.tryFinish();
    if (out && out.bytes) {
      const same = out.bytes.length === file.length && out.bytes.every((v, i) => v === file[i]);
      const p = rx.progress();
      console.log(
        `${label.padEnd(30)} ${same ? 'OK ' : 'BAD'} ${size}B payload=${prepared.payload.length}B K=${sender.K} frames shown=${shown} scanned=${scanned} cw=${okCw}/${totalCw} got=${p.received} (${((performance.now() - t0) / 1000).toFixed(1)}s)`,
      );
      return same;
    }
  }
  console.log(`${label} FAILED: did not complete`, rx.progress());
  return false;
}

const cam = { camW: 1920, camH: 1080, fill: 0.85, blur: 1.0, noise: 4, rot: 0.08, tilt: [0.1, -0.06], k1: -0.04,
  cross: [0.85, 0.12, 0.05, 0.1, 0.82, 0.1, 0.04, 0.15, 0.85], gains: [1.08, 1.0, 0.86], black: 14, vignette: 0.3, subsample: 1 };

let ok = true;
ok = (await transfer({ size: 300, X: 64, Y: 64, mode: 2, s: 10, cam: { ...cam, camW: 1280, camH: 720 }, label: 'tiny C8 64x64' })) && ok;
ok = (await transfer({ size: 40000, X: 100, Y: 180, mode: 2, s: 8, cam, label: 'phone C8 100x180 40KB' })) && ok;
ok = (await transfer({ size: 60000, X: 100, Y: 180, mode: 1, s: 8, cam, compressible: true, label: 'phone C4 text 60KB' })) && ok;
ok = (await transfer({ size: 80000, X: 140, Y: 240, mode: 2, s: 6, cam, label: 'desktop C8 140x240 80KB' })) && ok;
if (!ok) process.exit(1);
console.log('e2e: all passed');
