// Collects packets and runs the fountain decode off the main thread (JS or Rust/WASM engine).
import { Receiver } from './core/transfer.js';
import { loadWasm, WasmReceiver } from './core/wasm-engine.js';

let engine = 'js';
let rx = new Receiver();
let chain = Promise.resolve();

async function handle(msg) {
  if (msg.reset) {
    engine = msg.engine === 'wasm' ? 'wasm' : 'js';
    if (engine === 'wasm') await loadWasm();
    rx = engine === 'wasm' ? new WasmReceiver() : new Receiver();
    return;
  }
  for (const p of msg.packets) await rx.push(p);
  const prog = rx.progress();
  if (prog) prog.T = rx.active ? rx.active.T : 0;
  const t0 = performance.now();
  const out = await rx.tryFinish();
  const ms = performance.now() - t0;
  self.postMessage({ type: 'progress', prog, decodeMs: ms, engine });
  if (out && out.bytes) self.postMessage({ type: 'done', name: out.name, bytes: out.bytes, engine }, [out.bytes.buffer]);
  else if (out && out.error) self.postMessage({ type: 'error', error: out.error });
}

self.onmessage = (e) => {
  chain = chain.then(() => handle(e.data)).catch((err) => self.postMessage({ type: 'error', error: String(err) }));
};
