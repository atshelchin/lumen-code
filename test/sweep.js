import { runSim } from './sim.js';
const harsh = (o) => ({ camW: 1920, camH: 1080, fill: 0.85, blur: 1.0, noise: 4, rot: 0.12, tilt: [0.12, -0.08], k1: -0.04,
  cross: [0.85, 0.12, 0.05, 0.1, 0.82, 0.1, 0.04, 0.15, 0.85], gains: [1.08, 1.0, 0.86], black: 14, vignette: 0.35, ...o });
const only = process.argv[2];
const cases = [
  ['C8 100x180 1080p harsh', 100, 180, 2, 8, harsh({})],
  ['C8 100x180 1080p blur1.4', 100, 180, 2, 8, harsh({ blur: 1.4 })],
  ['C8 100x180 1080p blur1.8', 100, 180, 2, 8, harsh({ blur: 1.8 })],
  ['C4 100x180 1080p blur1.8', 100, 180, 1, 8, harsh({ blur: 1.8 })],
  ['C2 100x180 1080p blur1.9', 100, 180, 0, 8, harsh({ blur: 1.9 })],
  ['C8 150x260 1080p fill.95', 150, 260, 2, 6, harsh({ fill: 0.86 })],
  ['C8 120x120 720p rot0.6', 120, 120, 2, 6, harsh({ camW: 1280, camH: 720, rot: 0.6, fill: 0.62 })],
  ['C8 120x120 720p rot2.5', 120, 120, 2, 6, harsh({ camW: 1280, camH: 720, rot: 2.5, fill: 0.62 })],
  ['C8 120x120 720p mirrored', 120, 120, 2, 6, harsh({ camW: 1280, camH: 720, tilt: [0.25, 0.2], mirror: 1, fill: 0.8 })],
  ['C8 100x180 1080p small 4.2px', 100, 180, 2, 8, harsh({ fill: 0.72 })],
  ['C4 64x110 720p phone-low', 64, 110, 1, 10, harsh({ camW: 1280, camH: 720, fill: 0.8 })],
  ['C8 100x180 1080p overexposed', 100, 180, 2, 8, harsh({ gains: [1.5, 1.4, 1.3] })],
  ['C8 100x180 1080p underexposed', 100, 180, 2, 8, harsh({ gains: [0.55, 0.5, 0.45], noise: 6 })],
  ['C8 100x180 1080p strong persp', 100, 180, 2, 8, harsh({ tilt: [0.3, 0.25], fill: 0.78 })],
  ['C8 100x180 1080p barrel k-.12', 100, 180, 2, 8, harsh({ k1: -0.12 })],
  ['C8 140x140 1080p desktop', 140, 140, 2, 8, harsh({ fill: 0.8 })],
];
for (const [name, X, Y, mode, s, cam] of cases) {
  if (only && !name.includes(only)) continue;
  const r = runSim(X, Y, mode, s, cam);
  console.log(name.padEnd(32), r.ok ? `ok ${r.good}/${r.total} cellPx=${r.stats.cellPx.toFixed(2)} corr=${r.stats.corrected} markers=${r.stats.markers.toFixed(2)} br=${r.stats.cornerFound} scan=${r.tScan.toFixed(0)}ms` : `FAIL ${r.reason}`);
}
