// "Badly aimed phone" scenarios: bloom, moiré, motion blur, off-centre, tilted, small.
import { runSim } from './sim.js';
const base = (o) => ({ camW: 1920, camH: 1080, fill: 0.7, blur: 1.0, noise: 4, rot: 0.1, tilt: [0.1, -0.06], k1: -0.04,
  cross: [0.85, 0.12, 0.05, 0.1, 0.82, 0.1, 0.04, 0.15, 0.85], gains: [1.15, 1.08, 0.95], black: 14, vignette: 0.35, subsample: 1, ...o });
const only = process.argv[2];
const cases = [
  ['bloom 1.0', base({ bloom: 1.0 })],
  ['bloom 2.0', base({ bloom: 2.0 })],
  ['bloom 2 overexposed', base({ bloom: 2.0, gains: [1.5, 1.4, 1.3] })],
  ['moire 0.15', base({ moire: 0.15 })],
  ['moire 0.3', base({ moire: 0.3 })],
  ['motion 4px', base({ motion: 4 })],
  ['motion 7px', base({ motion: 7 })],
  ['off-centre small', base({ fill: 0.5, shift: [0.18, 0.12] })],
  ['tilt 25deg', base({ tilt: [0.35, 0.2], fill: 0.62 })],
  ['tilt+rot+off', base({ tilt: [0.28, -0.25], rot: 0.5, fill: 0.55, shift: [-0.1, 0.06] })],
  ['rot 45deg', base({ rot: 0.785, fill: 0.55 })],
  ['everything', base({ bloom: 1.5, moire: 0.15, motion: 3, tilt: [0.2, 0.15], rot: 0.3, fill: 0.58, shift: [0.08, -0.05] })],
];
const geoms = [['phone C8', 86, 152, 2, 8], ['desktop C8', 249, 153, 2, 7], ['phone C4', 86, 152, 1, 8]];
for (const [gname, X, Y, mode, s] of geoms) {
  for (const [name, cam] of cases) {
    if (only && !(`${gname} ${name}`).includes(only)) continue;
    const r = runSim(X, Y, mode, s, cam);
    console.log(`${gname.padEnd(11)} ${name.padEnd(22)}`, r.ok ? `ok ${String(r.good).padStart(2)}/${r.total} cell=${r.stats.cellPx.toFixed(2)} markers=${r.stats.markers.toFixed(2)}` : `FAIL ${r.reason} (cands ${r.candidates ?? '?'})`);
  }
}
