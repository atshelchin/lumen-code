// Decodes camera frames off the main thread, with the JS or the Rust/WASM engine.
// The main thread sends only the region of interest, already downscaled on the GPU
// (see grab() in recv-app.js); `region` maps the result back to camera pixels.
import { Scanner } from './core/scan.js?v=c5d69ca5e8';
import { loadWasm, WasmScanner } from './core/wasm-engine.js?v=c5d69ca5e8';

const scanners = { js: new Scanner(), wasm: null };
let canvas = null;
let ctx = null;

async function scannerFor(engine) {
  if (engine !== 'wasm') return scanners.js;
  if (!scanners.wasm) {
    try {
      await loadWasm();
      scanners.wasm = new WasmScanner();
    } catch (err) {
      const e = new Error(`WASM unavailable: ${err && err.message ? err.message : err}`);
      e.wasm = true;
      throw e;
    }
  }
  return scanners.wasm;
}

self.onmessage = async (e) => {
  const { id, bitmap, image, region, dw, dh, skip, engine } = e.data;
  let img = image;
  const tPrep = performance.now();
  try {
    const scanner = await scannerFor(engine);
    const { x: ox, y: oy, kx, ky, fullW, fullH } = region;
    if (bitmap) {
      if (!canvas) {
        canvas = new OffscreenCanvas(dw, dh);
        ctx = canvas.getContext('2d', { willReadFrequently: true });
      }
      if (canvas.width < dw || canvas.height < dh) {
        canvas.width = Math.max(canvas.width, dw);
        canvas.height = Math.max(canvas.height, dh);
      }
      // 1:1 when the browser resized the bitmap; otherwise (no resize support) scale here
      ctx.imageSmoothingEnabled = bitmap.width !== dw;
      ctx.drawImage(bitmap, 0, 0, bitmap.width, bitmap.height, 0, 0, dw, dh);
      bitmap.close();
      img = ctx.getImageData(0, 0, dw, dh);
    }
    const t0 = performance.now();
    const prepMs = t0 - tPrep; // frame copy + read-back: part of the real cost, not of the engine
    const res = scanner.scan(img, { skip });
    const ms = performance.now() - t0;
    if (res.ok && res.stats) {
      // back to camera-frame coordinates
      res.stats.corners = res.stats.corners.map(([x, y]) => [x / kx + ox, y / ky + oy]);
      res.stats.cellPx /= (kx + ky) / 2;
      res.stats.scale = (kx + ky) / 2;
    }
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
        prepMs,
        px: img.width * img.height,
        w: fullW,
        h: fullH,
        header: res.header || null,
        headerReused: !!res.headerReused,
        frameKey: res.frameKey || null,
        skipped: !!res.skipped,
        okIdx,
        stats: res.ok ? res.stats : null,
        packets,
      },
      packets.map((p) => p.buffer),
    );
  } catch (err) {
    if (bitmap) bitmap.close?.();
    self.postMessage({ id, ok: false, reason: 'error', wasmError: !!(err && err.wasm), error: String(err && err.stack ? err.stack : err), packets: [] });
  }
};
