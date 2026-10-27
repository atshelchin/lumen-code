// Renders a whole transfer through the camera simulator into a .y4m video that Chrome
// can use as a fake webcam (--use-file-for-fake-video-capture).
// usage: node test/make-y4m.js out.y4m bytes [mode] [X] [Y] [cellPx] [camW] [camH] [fill] [name]
//   fill: share of the camera image the code covers (0.88); name: the file name sent
import { openSync, writeSync, closeSync, writeFileSync } from 'node:fs';
import { Sender, preparePayload } from '../src/core/transfer.js';
import { Renderer } from '../src/core/render.js';
import { simulateCamera } from './camera.js';
import { Rng } from '../src/core/util.js';

const [out = 'test/out/cam.y4m', size = '50000', mode = '2', X = '100', Y = '170', s = '8', cw = '1280', ch = '720', fill = '0.88', name = 'camera-test.bin'] = process.argv.slice(2);
const W = Number(cw), H = Number(ch);
const rng = new Rng(7);
const file = new Uint8Array(Number(size)).map(() => rng.next() & 255);
writeFileSync(out + '.src', file);
const prepared = await preparePayload(file, name);
const sender = new Sender(prepared, { mode: Number(mode), X: Number(X), Y: Number(Y) });
const frames = Math.ceil((sender.K * 1.25) / sender.perFrame) + 2;
const fd = openSync(out, 'w');
writeSync(fd, `YUV4MPEG2 W${W} H${H} F30:1 Ip A1:1 C420jpeg\n`);
const yuv = new Uint8Array(W * H * 1.5);
let prev = null;
for (let f = 0; f < frames; f++) {
  const { cells, layout } = sender.nextFrame();
  const r = new Renderer(layout, Number(mode), Number(s));
  const rgba = new Uint8Array(r.width * r.height * 4);
  r.draw(cells, new Uint32Array(rgba.buffer));
  // each displayed frame is seen by 3 camera frames; the first one is torn with the previous frame
  for (let rep = 0; rep < 3; rep++) {
    let src = rgba;
    if (rep === 0 && prev) {
      src = rgba.slice();
      src.set(prev.subarray(0, Math.floor(r.height * 0.45) * r.width * 4));
    }
    const img = simulateCamera({ width: r.width, height: r.height, data: src }, {
      camW: W, camH: H, fill: Number(fill), blur: 0.9, noise: 3, rot: 0.05 + 0.01 * rep, tilt: [0.06, -0.04], k1: -0.03,
      cross: [0.88, 0.1, 0.04, 0.08, 0.86, 0.08, 0.03, 0.12, 0.88], gains: [1.06, 1, 0.9], black: 12, vignette: 0.25, subsample: 1, seed: f * 3 + rep + 1,
    });
    const d = img.data;
    for (let i = 0, j = 0; i < W * H; i++, j += 4) yuv[i] = Math.max(0, Math.min(255, 0.299 * d[j] + 0.587 * d[j + 1] + 0.114 * d[j + 2]));
    let u = W * H, v = W * H + (W * H) / 4;
    for (let y = 0; y < H; y += 2)
      for (let x = 0; x < W; x += 2) {
        let rr = 0, gg = 0, bb = 0;
        for (const [dx, dy] of [[0, 0], [1, 0], [0, 1], [1, 1]]) { const j = ((y + dy) * W + x + dx) * 4; rr += d[j]; gg += d[j + 1]; bb += d[j + 2]; }
        rr /= 4; gg /= 4; bb /= 4;
        yuv[u++] = Math.max(0, Math.min(255, 128 - 0.168736 * rr - 0.331264 * gg + 0.5 * bb));
        yuv[v++] = Math.max(0, Math.min(255, 128 + 0.5 * rr - 0.418688 * gg - 0.081312 * bb));
      }
    writeSync(fd, 'FRAME\n');
    writeSync(fd, yuv);
  }
  prev = rgba;
}
closeSync(fd);
console.log(`wrote ${out}: ${frames} sender frames, K=${sender.K}, perFrame=${sender.perFrame} packets`);
