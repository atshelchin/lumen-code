// JS <-> Rust/WASM interoperability: identical frames, cross decoding, timing.
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { loadWasm, WasmScanner, WasmSender, WasmReceiver } from '../src/core/wasm-engine.js';
import { Sender, Receiver } from '../src/core/transfer.js';
import { Scanner } from '../src/core/scan.js';
import { Renderer } from '../src/core/render.js';
import { crc32, Rng } from '../src/core/util.js';

await loadWasm(readFileSync(new URL('../dist/wasm/lumen.wasm', import.meta.url)));
let fail = 0;
const rng = new Rng(3);
const file = new Uint8Array(30000).map(() => rng.next() & 255);
const prepared = { payload: file, meta: { name: 'interop.bin', fileSize: file.length, crc: crc32(file), flags: 0, payloadLength: file.length } };

// 1. bit-identical frames from both senders (same tid; JS seq aligned to the WASM one)
const js2 = new Sender(prepared, { mode: 1, ecc: 1, X: 86, Y: 152, tid: 0x77 });
const ws2 = new WasmSender(prepared, { mode: 1, ecc: 1, X: 86, Y: 152, tid: 0x77 });
let same = true;
for (let f = 0; f < 6; f++) {
  const b = ws2.nextFrame();
  js2.seq = b.seq;
  const a = js2.nextFrame();
  if (crc32(a.cells) !== crc32(b.cells)) same = false;
}
console.log('JS and WASM senders produce identical frames:', same);
if (!same) fail++;

// 2. cross decoding: JS packets -> WASM receiver, WASM-scanned frames -> JS receiver
const wrx = new WasmReceiver();
const jtx = new Sender(prepared, { mode: 1, ecc: 1, X: 86, Y: 152 });
let done = null;
for (let f = 0; f < 100 && !done; f++) {
  for (const p of jtx.nextPackets()) await wrx.push(p);
  done = await wrx.tryFinish();
}
const okA = done && done.bytes.length === file.length && done.bytes.every((v, i) => v === file[i]);
console.log('JS sender -> WASM receiver:', !!okA, done && done.name);
if (!okA) fail++;

// 3. the other direction through the visual layer: WASM sender frames -> render -> JS scanner -> JS receiver
{
  const wtx = new WasmSender(prepared, { mode: 1, ecc: 1, X: 86, Y: 152 });
  const sc = new Scanner();
  const jrx = new Receiver();
  let out = null;
  for (let f = 0; f < 100 && !out; f++) {
    const { cells, layout } = wtx.nextFrame();
    const r = new Renderer(layout, 1, 4);
    const rgba = new Uint8Array(r.width * r.height * 4);
    r.draw(cells, new Uint32Array(rgba.buffer));
    const res = sc.scan({ width: r.width, height: r.height, data: rgba });
    if (res.ok) for (const c of res.codewords) if (c.ok) await jrx.push(c.bytes);
    out = await jrx.tryFinish();
  }
  const okB = out && out.bytes && out.bytes.length === file.length && out.bytes.every((v, i) => v === file[i]);
  console.log('WASM sender -> JS scanner and receiver:', !!okB);
  if (!okB) fail++;
  wtx.free();
}

// 4. the receiver state machine of the WASM build against the stream vectors (PROTOCOL §11)
{
  const V = JSON.parse(readFileSync(new URL('./vectors/test-vectors.json', import.meta.url), 'utf8'));
  const unhex = (h) => new Uint8Array(Buffer.from(h, 'hex'));
  let bad = 0;
  for (const st of V.streams) {
    const rx = new WasmReceiver(st.limits ? { maxSize: 256 * 1024 * 1024, maxKept: 4096, maxTransfers: 4, ...st.limits } : null);
    for (const s of st.steps) {
      if (s.push && (await rx.push(unhex(s.push))) !== s.result) bad++;
      else if ('finish' in s) {
        const got = await rx.tryFinish();
        const norm = got && (got.bytes ? { file: Buffer.from(got.bytes).toString('hex'), name: got.name } : { error: 'check' });
        if (JSON.stringify(norm) !== JSON.stringify(s.finish)) bad++;
      } else if (s.state && ((rx.progress() || { state: 'new' }).state !== s.state)) bad++;
    }
    rx.free();
  }
  console.log(`WASM receiver, stream vectors: ${bad ? bad + ' wrong steps' : 'all ' + V.streams.length + ' streams pass'}`);
  if (bad) fail++;
}

// 5. scanners on the JS camera fixtures
if (existsSync('test/out/raw')) {
  const sj = new Scanner();
  const sw = new WasmScanner();
  for (const f of readdirSync('test/out/raw')) {
    const d = readFileSync('test/out/raw/' + f);
    const w = d.readUInt32LE(0), h = d.readUInt32LE(4);
    const img = { width: w, height: h, data: new Uint8Array(d.buffer, d.byteOffset + 12, w * h * 4) };
    const time = (sc) => { let t = 0, r; for (let i = 0; i < 6; i++) { const t0 = performance.now(); r = sc.scan(img); if (i >= 2) t += performance.now() - t0; } return [t / 4, r]; };
    const [tj, rj] = time(sj);
    const [tw, rw] = time(sw);
    const nj = rj.codewords.filter((c) => c.ok).length, nw = rw.codewords.filter((c) => c.ok).length;
    console.log(`${f.padEnd(16)} JS ${tj.toFixed(1)} ms (${nj})   WASM ${tw.toFixed(1)} ms (${nw})   x${(tj / tw).toFixed(2)}`);
    if (nw + 1 < nj) fail++;
  }
}
if (fail) { console.log(fail, 'failures'); process.exit(1); }
console.log('wasm: all passed');
