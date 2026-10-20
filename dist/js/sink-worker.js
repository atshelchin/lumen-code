// Collects packets and runs the fountain decode off the main thread (JS or Rust/WASM engine).
//   { reset, engine }  new reception
//   { engine }         switch engine, keeping the reception: the unique packets received so far
//                      are replayed into the new engine's receiver
//   { packets }        add packets
import { Receiver } from './core/transfer.js?v=be07636865';
import { loadWasm, WasmReceiver } from './core/wasm-engine.js?v=be07636865';

const LOG_LIMIT = 256 * 1024 * 1024; // bytes of packets kept for an engine switch

let engine = 'js';
let rx = new Receiver();
let chain = Promise.resolve();
let log = []; // unique packets of this reception, in arrival order
let seen = new Set(); // TID, M|SBN, ESI and length of each logged packet
let logBytes = 0;

// Packet identity: bytes 0–4 (TID, M|SBN, ESI) and the length (which gives T).
const keyOf = (p) => ((p[0] << 8) | p[1]) * 2 ** 33 + (p[2] * 65536 + ((p[3] << 8) | p[4])) * 512 + p.length;

async function makeReceiver(want) {
  if (want === 'wasm') {
    try {
      await loadWasm();
      return { engine: 'wasm', rx: new WasmReceiver() };
    } catch (err) {
      self.postMessage({ type: 'error', wasmError: true, error: `WASM unavailable: ${err && err.message ? err.message : err}` });
      return null;
    }
  }
  return { engine: 'js', rx: new Receiver() };
}

async function report() {
  const prog = rx.progress();
  if (prog) prog.T = rx.active ? rx.active.T : 0;
  const t0 = performance.now();
  const out = await rx.tryFinish();
  const ms = performance.now() - t0;
  self.postMessage({ type: 'progress', prog, decodeMs: ms, engine });
  if (out && out.bytes) self.postMessage({ type: 'done', name: out.name, bytes: out.bytes, engine }, [out.bytes.buffer]);
  else if (out && out.error) self.postMessage({ type: 'error', error: out.error });
}

async function handle(msg) {
  if (msg.reset) {
    const made = (await makeReceiver(msg.engine === 'wasm' ? 'wasm' : 'js')) || (await makeReceiver('js'));
    ({ engine, rx } = made);
    log = [];
    seen = new Set();
    logBytes = 0;
    return;
  }
  if (msg.engine && !msg.packets) {
    const want = msg.engine === 'wasm' ? 'wasm' : 'js';
    if (want === engine) return;
    const made = await makeReceiver(want);
    if (!made) return; // WASM failed: keep the current receiver and everything in it
    ({ engine, rx } = made);
    for (const p of log) await rx.push(p);
    if (log.length) await report();
    return;
  }
  for (const p of msg.packets) {
    const k = keyOf(p);
    if (!seen.has(k) && logBytes < LOG_LIMIT) {
      seen.add(k);
      log.push(p);
      logBytes += p.length;
    }
    await rx.push(p);
  }
  await report();
}

self.onmessage = (e) => {
  chain = chain.then(() => handle(e.data)).catch((err) => self.postMessage({ type: 'error', error: String(err) }));
};
