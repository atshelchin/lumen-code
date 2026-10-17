// Randomised "hand-held, badly aimed" frames: reports detection and codeword rates.
import { runSim } from './sim.js';
import { Rng } from '../src/core/util.js';
const N = Number(process.argv[2] || 30);
const mode = Number(process.argv[3] ?? 1);
const [X, Y, s] = (process.argv[4] || '86,152,8').split(',').map(Number);
const rng = new Rng(Number(process.argv[5] || 11));
const U = (a, b) => a + (b - a) * (rng.next() / 2 ** 32);
let found = 0, cw = 0, tot = 0;
const reasons = {};
for (let i = 0; i < N; i++) {
  const cam = { camW: 1920, camH: 1080, fill: U(0.45, 0.85), blur: U(0.7, 1.5), noise: U(2, 6), rot: U(-0.6, 0.6),
    tilt: [U(-0.3, 0.3), U(-0.3, 0.3)], shift: [U(-0.12, 0.12), U(-0.08, 0.08)], k1: U(-0.06, 0),
    cross: [0.85, 0.12, 0.05, 0.1, 0.82, 0.1, 0.04, 0.15, 0.85], gains: [U(0.8, 1.4), U(0.8, 1.3), U(0.7, 1.2)], black: U(5, 25),
    vignette: U(0, 0.5), bloom: U(0, 2), moire: U(0, 0.2), motion: U(0, 5), motionDir: rng.next() & 1 ? 'v' : 'h', subsample: 1, seed: i + 1 };
  const r = runSim(X, Y, mode, s, cam);
  if (r.ok) { found++; cw += r.good; } else { reasons[r.reason] = (reasons[r.reason] || 0) + 1; if (process.env.SHOWFAIL) console.log('FAIL', i, r.reason, 'cands', r.candidates, JSON.stringify(cam, (k, v) => typeof v === 'number' ? +v.toFixed(3) : v)); }
  tot += r.total;
}
console.log(`mode ${mode} ${X}x${Y}: frames decoded ${found}/${N}, codewords ${cw}/${tot} (${((100 * cw) / tot).toFixed(0)}%)`, reasons);
