// Packet-level transfer tests (no visual layer): source blocks, loss, reordering, descriptor timing.
import { Sender, Receiver, preparePayload, blockPlan } from '../src/core/transfer.js';
import { Rng } from '../src/core/util.js';

let failures = 0;
async function run(label, size, { loss = 0.2, X = 140, Y = 240, mode = 1, compressible = false, shuffle = false } = {}) {
  const rng = new Rng(size ^ 0x5eed);
  const file = new Uint8Array(size);
  if (compressible) for (let i = 0; i < size; i++) file[i] = 97 + ((i * 7) % 13) + (rng.next() % 50 === 0 ? 1 : 0);
  else for (let i = 0; i < size; i += 4) { const v = rng.next(); file[i] = v; file[i + 1] = v >>> 8; file[i + 2] = v >>> 16; file[i + 3] = v >>> 24; }
  const prepared = await preparePayload(file, 'blocks.bin');
  const tx = new Sender(prepared, { mode, X, Y });
  const rx = new Receiver();
  const t0 = performance.now();
  let sent = 0;
  let queue = [];
  for (let f = 0; f < 200000; f++) {
    for (const p of tx.nextPackets()) {
      sent++;
      if (rng.next() / 2 ** 32 < loss) continue;
      queue.push(p);
    }
    if (shuffle && queue.length > 64) for (let i = queue.length - 1; i > 0; i--) { const j = rng.next() % (i + 1); [queue[i], queue[j]] = [queue[j], queue[i]]; }
    for (const p of queue) await rx.push(p);
    queue = [];
    if (f % 4 === 3) {
      const out = await rx.tryFinish();
      if (out && out.bytes) {
        const ok = out.bytes.length === size && out.bytes.every((v, i) => v === file[i]);
        const pr = rx.progress();
        console.log(`${label.padEnd(28)} ${ok ? 'OK ' : 'BAD'} size=${size} K=${tx.K} blocks=${pr.blocks} T=${tx.T} sent=${sent} overhead=${((sent * (1 - loss)) / tx.K - 1).toFixed(3)} ${((performance.now() - t0) / 1000).toFixed(1)}s`);
        if (!ok) failures++;
        return;
      }
      if (out && out.error) { console.log(label, 'ERROR', out.error); failures++; return; }
    }
  }
  console.log(label, 'did not finish'); failures++;
}

// block planning invariants
for (const [L, T] of [[1, 200], [200, 200], [201, 200], [16384 * 200, 200], [16384 * 200 + 1, 200], [5e6, 191]]) {
  const p = blockPlan(L, T);
  const sum = p.sizes.reduce((a, b) => a + b, 0);
  if (sum !== p.Kt || Math.max(...p.sizes) - Math.min(...p.sizes) > 1 || Math.max(...p.sizes) > 16384) { console.log('plan FAIL', L, T, p); failures++; }
}
await run('single small', 3000);
await run('single, shuffled, 35% loss', 400000, { loss: 0.35, shuffle: true });
await run('two blocks (5 MB)', 5_000_000, { loss: 0.2 });
await run('compressible text', 2_000_000, { compressible: true });
if (failures) { console.log(failures, 'failures'); process.exit(1); }
console.log('transfer: all passed');
