// Decodes camera frames off the main thread, with the JS or the Rust/WASM engine.
import { Scanner } from './core/scan.js';
import { loadWasm, WasmScanner } from './core/wasm-engine.js';

const scanners = { js: new Scanner(), wasm: null };
let canvas = null;
let ctx = null;
// Region of interest from this worker's last success (full-frame coordinates). Only that
// part of the frame is read back and processed; a miss falls back to the whole frame.
let roi = null;

async function scannerFor(engine) {
  if (engine !== 'wasm') return scanners.js;
  if (!scanners.wasm) {
    await loadWasm();
    scanners.wasm = new WasmScanner();
  }
  return scanners.wasm;
}

self.onmessage = async (e) => {
  const { id, bitmap, image, skip, engine } = e.data;
  let img = image;
  try {
    const scanner = await scannerFor(engine);
    if (bitmap) {
      if (!canvas || canvas.width !== bitmap.width || canvas.height !== bitmap.height) {
        canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
        ctx = canvas.getContext('2d', { willReadFrequently: true });
      }
      ctx.drawImage(bitmap, 0, 0);
      bitmap.close();
      img = roi ? ctx.getImageData(roi.x, roi.y, roi.w, roi.h) : ctx.getImageData(0, 0, canvas.width, canvas.height);
    }
    const ox = bitmap && roi ? roi.x : 0;
    const oy = bitmap && roi ? roi.y : 0;
    const fullW = bitmap ? canvas.width : img.width;
    const fullH = bitmap ? canvas.height : img.height;
    const t0 = performance.now();
    const res = scanner.scan(img, { skip });
    const ms = performance.now() - t0;
    if (res.ok && res.stats) {
      res.stats.corners = res.stats.corners.map(([x, y]) => [x + ox, y + oy]);
      const xs = res.stats.corners.map((c) => c[0]);
      const ys = res.stats.corners.map((c) => c[1]);
      const mx = (Math.max(...xs) - Math.min(...xs)) * 0.18 + 24;
      const my = (Math.max(...ys) - Math.min(...ys)) * 0.18 + 24;
      const x0 = Math.max(0, Math.floor(Math.min(...xs) - mx));
      const y0 = Math.max(0, Math.floor(Math.min(...ys) - my));
      const x1 = Math.min(fullW, Math.ceil(Math.max(...xs) + mx));
      const y1 = Math.min(fullH, Math.ceil(Math.max(...ys) + my));
      roi = x1 - x0 > 64 && y1 - y0 > 64 && (x1 - x0) * (y1 - y0) < fullW * fullH * 0.85 ? { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } : null;
    } else roi = null;
    const packets = [];
    const okIdx = [];
    if (res.ok)
      for (const c of res.codewords)
        if (c.ok) {
          packets.push(c.bytes);
          okIdx.push(c.index);
        }
    self.postMessage(
      {
        id,
        engine: engine === 'wasm' ? 'wasm' : 'js',
        ok: res.ok,
        reason: res.reason,
        ms,
        w: fullW,
        h: fullH,
        header: res.header || null,
        frameKey: res.frameKey || null,
        skipped: !!res.skipped,
        okIdx,
        stats: res.ok ? res.stats : null,
        packets,
      },
      packets.map((p) => p.buffer),
    );
  } catch (err) {
    self.postMessage({ id, ok: false, reason: 'error', error: String(err && err.stack ? err.stack : err), packets: [] });
  }
};
