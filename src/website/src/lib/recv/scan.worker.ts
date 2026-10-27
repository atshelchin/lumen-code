// Decodes camera frames off the main thread, with the JS or the Rust/WASM engine.
// The main thread sends only the region of interest, already downscaled on the GPU
// (see grab() in receiver.svelte.ts); `region` maps the result back to camera pixels.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { Scanner, loadWasm, WasmScanner, wasmUrl } from '#lib/protocol';

// the worker's global scope (the project's TypeScript library is the DOM one)
const worker = self as unknown as {
	postMessage(message: unknown, transfer?: Transferable[]): void;
	onmessage: ((e: MessageEvent) => void) | null;
};

const scanners: { js: any; wasm: any } = { js: new Scanner(), wasm: null };
let canvas: OffscreenCanvas | null = null;
let ctx: OffscreenCanvasRenderingContext2D | null = null;

async function scannerFor(engine: string) {
	if (engine !== 'wasm') return scanners.js;
	if (!scanners.wasm) {
		try {
			await loadWasm(wasmUrl);
			scanners.wasm = new WasmScanner();
		} catch (err) {
			const e: any = new Error(`WASM unavailable: ${err instanceof Error ? err.message : err}`);
			e.wasm = true;
			throw e;
		}
	}
	return scanners.wasm;
}

worker.onmessage = async (e: MessageEvent) => {
	const { id, bitmap, image, region, dw, dh, skip, engine } = e.data;
	let img = image;
	const tPrep = performance.now();
	try {
		const scanner = await scannerFor(engine);
		const { x: ox, y: oy, kx, ky, fullW, fullH } = region;
		if (bitmap) {
			if (!canvas) {
				canvas = new OffscreenCanvas(dw, dh);
				ctx = canvas.getContext('2d', { willReadFrequently: true })!;
			}
			if (canvas.width < dw || canvas.height < dh) {
				canvas.width = Math.max(canvas.width, dw);
				canvas.height = Math.max(canvas.height, dh);
			}
			// 1:1 when the browser resized the bitmap; otherwise (no resize support) scale here
			ctx!.imageSmoothingEnabled = bitmap.width !== dw;
			ctx!.drawImage(bitmap, 0, 0, bitmap.width, bitmap.height, 0, 0, dw, dh);
			bitmap.close();
			img = ctx!.getImageData(0, 0, dw, dh);
		}
		const t0 = performance.now();
		const prepMs = t0 - tPrep; // frame copy + read-back: part of the real cost, not of the engine
		const res = scanner.scan(img, { skip });
		const ms = performance.now() - t0;
		if (res.ok && res.stats) {
			// back to camera-frame coordinates
			res.stats.corners = res.stats.corners.map(([x, y]: number[]) => [x / kx + ox, y / ky + oy]);
			res.stats.cellPx /= (kx + ky) / 2;
			res.stats.scale = (kx + ky) / 2;
		}
		const packets: Uint8Array[] = [];
		const okIdx: number[] = [];
		if (res.ok)
			for (const c of res.codewords)
				if (c.ok) {
					packets.push(c.bytes);
					okIdx.push(c.index);
				}
		worker.postMessage(
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
				packets
			},
			packets.map((p) => p.buffer)
		);
	} catch (err: any) {
		if (bitmap) bitmap.close?.();
		worker.postMessage({
			id,
			ok: false,
			reason: 'error',
			wasmError: !!(err && err.wasm),
			error: String(err && err.stack ? err.stack : err),
			packets: []
		});
	}
};
