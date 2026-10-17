// Writes a simulated camera photo of frame 0 of a real transfer (for UI tests).
// usage: node test/make-frame.js out.png [bytes]
import { Sender, preparePayload } from '../src/core/transfer.js';
import { Renderer } from '../src/core/render.js';
import { simulateCamera } from './camera.js';
import { writePNG } from './png.js';

const out = process.argv[2] || 'test/out/frame.png';
const text = process.argv[3] || 'Hello from Lumen Code 👋\n\n这是一段通过屏幕和相机传输的文本。\nNo network, no pairing — just light.\n';
const name = process.argv[4] || 'hello.txt';
const prepared = await preparePayload(new TextEncoder().encode(text), name);
const sender = new Sender(prepared, { mode: 2, ecc: 1, X: 90, Y: 150 });
const { cells, layout } = sender.nextFrame();
const r = new Renderer(layout, 2, 8);
const rgba = new Uint8Array(r.width * r.height * 4);
r.draw(cells, new Uint32Array(rgba.buffer));
const img = simulateCamera({ width: r.width, height: r.height, data: rgba }, { camW: 1920, camH: 1080, fill: 0.8, blur: 0.9, noise: 3, rot: -0.06, tilt: [0.06, 0.04], k1: -0.03, gains: [1.05, 1, 0.9], black: 12, vignette: 0.3 });
writePNG(out, img.width, img.height, img.data);
console.log('wrote', out, 'K=', sender.K);
