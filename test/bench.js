// Throughput of each stage of the JS implementation (the Rust numbers come from
// `cargo run --release --example measure -- bench` in src/rust).
//   node test/bench.js
import { Rng, crc32 } from '../src/core/util.js';
import { rsEncode, rsDecode } from '../src/core/rs.js';
import { FountainEncoder, FountainDecoder } from '../src/core/fountain.js';
import { makeLayout, chooseCodeword, codewordsPerFrame } from '../src/core/layout.js';
import { encodeFrameCells } from '../src/core/frame.js';
import { inflate } from '../src/core/inflate.js';
import { Sender, Receiver, fountainSeed } from '../src/core/transfer.js';

const r = new Rng(9);
const time = (reps, f) => {
  f();
  const t0 = performance.now();
  for (let i = 0; i < reps; i++) f();
  return (performance.now() - t0) / reps;
};

{
  const msg = new Uint8Array(215).map(() => r.next() & 255);
  const cw = rsEncode(msg, 40, new Uint8Array(255));
  const bad = cw.slice();
  for (let i = 0; i < 20; i++) bad[i * 12] ^= 0x5a;
  const enc = time(20000, () => rsEncode(msg, 40, new Uint8Array(255)));
  const clean = time(20000, () => rsDecode(cw.slice(), 40));
  const dec = time(5000, () => rsDecode(bad.slice(), 40));
  console.log(`RS(255,215) encode ${(enc * 1e3).toFixed(2)} us, decode clean ${(clean * 1e3).toFixed(2)} us, decode 20 errors ${(dec * 1e3).toFixed(1)} us`);
}

for (const K of [100, 1000, 16384]) {
  const T = 200;
  const data = new Uint8Array(K * T).map(() => r.next() & 255);
  const seed = fountainSeed(1, 0);
  let enc;
  const pre = time(3, () => (enc = new FountainEncoder(data, T, seed)));
  const out = new Uint8Array(T);
  const sym = time(20000, () => enc.encode(r.next() & 0xffff, out));
  const syms = [];
  for (let esi = 0; syms.length < K + 10; esi++) syms.push([esi, enc.encode(esi, new Uint8Array(T))]);
  const d = time(3, () => {
    const dec = new FountainDecoder(K, T, seed);
    for (const [e, s] of syms) dec.add(e, s);
    if (!dec.decode()) throw new Error('decode');
  });
  console.log(`fountain K=${String(K).padEnd(6)} T=${T}: precode ${pre.toFixed(1)} ms, one symbol ${(sym * 1e3).toFixed(2)} us, decode of K+10 symbols ${d.toFixed(1)} ms (${((K * T) / d / 1e3).toFixed(0)} MB/s)`);
}

for (const [X, Y] of [[150, 240], [90, 160]]) {
  const layout = makeLayout(X, Y);
  const { n, nsym } = chooseCodeword(layout, 1, 1);
  const m = codewordsPerFrame(layout, 1, n);
  const packets = Array.from({ length: m }, () => new Uint8Array(n - nsym).map(() => r.next() & 255));
  const f = time(300, () => encodeFrameCells(layout, { mode: 1, n, nsym }, 7, packets.map((p) => p.slice())));
  console.log(`frame ${X}x${Y} C4: ${m} codewords of n=${n}, cells ${f.toFixed(2)} ms`);
}

{
  const words = ['lumen ', 'code ', 'frame ', 'camera ', '\n', 'fountain ', '0123 '];
  const text = new TextEncoder().encode(Array.from({ length: 400000 }, () => words[r.next() % words.length]).join(''));
  const z = new Uint8Array(await new Response(new Blob([text]).stream().pipeThrough(new CompressionStream('deflate-raw'))).arrayBuffer());
  const ms = time(5, () => inflate(z, text.length));
  console.log(`inflate ${(text.length / 1e6).toFixed(1)} MB text (zlib stream of ${(z.length / 1e3).toFixed(0)} KB): ${ms.toFixed(0)} ms (${(text.length / ms / 1e3).toFixed(0)} MB/s)`);
}

{
  const file = new Uint8Array(2_000_000).map(() => r.next() & 255);
  const prepared = { payload: file, meta: { name: 'bench.bin', fileSize: file.length, crc: crc32(file), flags: 0, payloadLength: file.length } };
  let t0 = performance.now();
  const tx = new Sender(prepared, { mode: 1, ecc: 1, X: 150, Y: 240, tid: 0x4242 });
  const setup = performance.now() - t0;
  const rx = new Receiver();
  let sending = 0;
  let frames = 0;
  let packets = 0;
  t0 = performance.now();
  for (;;) {
    const t1 = performance.now();
    const ps = tx.nextPackets();
    sending += performance.now() - t1;
    frames++;
    for (const p of ps) {
      await rx.push(p);
      packets++;
    }
    if (frames % 8 === 0) {
      const out = await rx.tryFinish();
      if (out && out.bytes) break;
    }
  }
  const total = performance.now() - t0;
  console.log(`2 MB transfer, packets only: sender setup ${setup.toFixed(0)} ms, ${frames} frames / ${packets} packets, sender ${sending.toFixed(0)} ms, receiver ${(total - sending).toFixed(0)} ms (${(2000 / (total - sending)).toFixed(0)} MB/s)`);
}
