// The sender: a file or a text becomes frames of the code, drawn on a canvas at a steady rate.
import {
	Sender,
	Renderer,
	preparePayload,
	loadWasm,
	WasmSender,
	MIN_DIM,
	MAX_DIM,
	MODES,
	wasmUrl
} from '#lib/protocol';

const QUIET = 2;
// The code fills the screen in its own shape; beyond this aspect ratio (ultra-wide monitors)
// a camera would see the code too thin, so the rest stays empty.
const MAX_ASPECT = 2.4;
const DENSITY = { low: 0.72, auto: 1, high: 1.3, max: 1.65 } as const;
// Text messages travel as a UTF-8 file with this name; receivers show them as text.
export const TEXT_NAME = 'message.txt';

export type Density = keyof typeof DENSITY;
export type Engine = 'js' | 'wasm';
export interface Prefs {
	density: Density;
	mode: number;
	ecc: number;
	fps: number;
	engine: Engine;
	source: 'file' | 'text';
}

const PREFS_KEY = 'lumen.send.v2';
function loadPrefs(): Prefs {
	const d: Prefs = { density: 'auto', mode: 1, ecc: 1, fps: 12, engine: 'js', source: 'file' };
	try {
		return { ...d, ...JSON.parse(localStorage.getItem(PREFS_KEY) || '{}') };
	} catch {
		return d;
	}
}

// ---------- geometry: how many cells the screen should carry
function mmPerCssPx() {
	const coarse = matchMedia('(pointer: coarse)').matches;
	const minScreen = Math.min(screen.width, screen.height);
	if (coarse && minScreen < 520) return 0.165; // phone
	if (coarse) return 0.2; // tablet
	return 0.25; // laptop / desktop
}

// The code box fills the available screen (any orientation): the more cells, the more data per
// frame. The receiver turns its phone to match a landscape code.
function computeGeometry(density: Density) {
	const dpr = window.devicePixelRatio || 1;
	const pad = 6;
	const cssW = Math.max(100, window.innerWidth - 2 * pad);
	const cssH = Math.max(100, window.innerHeight - 2 * pad);
	const boxW = Math.min(cssW, cssH * MAX_ASPECT);
	const boxH = Math.min(cssH, cssW * MAX_ASPECT);
	const devW = Math.floor(boxW * dpr);
	const devH = Math.floor(boxH * dpr);
	const shortMm = Math.min(boxW, boxH) * mmPerCssPx();
	// base: ~0.75 mm per cell, clamped to what a 1080p camera can resolve across its short side
	const base = Math.max(52, Math.min(150, Math.round(shortMm / 0.75)));
	const target = Math.round(base * DENSITY[density]);
	const shortDev = Math.min(devW, devH);
	let s = Math.max(3, Math.round(shortDev / (target + 2 * QUIET)));
	let X = 0;
	let Y = 0;
	const fit = () => {
		X = Math.floor(devW / s) - 2 * QUIET;
		Y = Math.floor(devH / s) - 2 * QUIET;
		if (X > Y * MAX_ASPECT) X = Math.floor(Y * MAX_ASPECT);
		if (Y > X * MAX_ASPECT) Y = Math.floor(X * MAX_ASPECT);
	};
	fit();
	// stay inside format limits
	while ((X > MAX_DIM || Y > MAX_DIM) && s < 64) {
		s++;
		fit();
	}
	while ((X < MIN_DIM || Y < MIN_DIM) && s > 2) {
		s--;
		fit();
	}
	X = Math.max(MIN_DIM, Math.min(MAX_DIM, X));
	Y = Math.max(MIN_DIM, Math.min(MAX_DIM, Y));
	return { s, X, Y, dpr };
}

export interface SendStats {
	rate: number;
	perFrame: number;
	pass: number;
	X: number;
	Y: number;
	bpc: number;
}

export class SendController {
	// ---------- what the page shows
	prefs = $state<Prefs>({
		density: 'auto',
		mode: 1,
		ecc: 1,
		fps: 12,
		engine: 'js',
		source: 'file'
	});
	sending = $state(false); // a file or text is on screen
	preparing = $state(false);
	playing = $state(true);
	label = $state(''); // file name, or the start of the text
	isText = $state(false);
	size = $state(0);
	stats = $state<SendStats | null>(null);
	buildMs = $state(0); // mean encode time per frame of the current engine
	dockAway = $state(false);
	settingsOpen = $state(false);

	// ---------- engine and drawing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	private sender: any = null;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	private renderer: any = null;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	private prepared: any = null;
	private canvas: HTMLCanvasElement | null = null;
	private ctx: CanvasRenderingContext2D | null = null;
	// Per frame only a tiny 1-pixel-per-cell image changes; the GPU scales it up and draws the
	// static rounded mask on top. No per-pixel JavaScript work at screen resolution.
	private cellCanvas: HTMLCanvasElement | null = null;
	private cellCtx: CanvasRenderingContext2D | null = null;
	private maskCanvas: HTMLCanvasElement | null = null;
	private cellImage: ImageData | null = null;
	private pending = false;
	private buildTimes: number[] = [];
	private tickTimer = 0;
	private nextFlipAt = 0;
	private dockTimer = 0;
	private wakeLock: WakeLockSentinel | null = null;
	shownFrames = 0;

	constructor() {
		this.prefs = loadPrefs();
		const e = new URLSearchParams(location.search).get('engine');
		if (e) this.prefs.engine = e === 'wasm' ? 'wasm' : 'js';
		this.settingsOpen = window.innerWidth >= 900;
	}

	savePrefs() {
		try {
			localStorage.setItem(PREFS_KEY, JSON.stringify(this.prefs));
		} catch {
			/* ignore */
		}
	}

	attach(canvas: HTMLCanvasElement) {
		this.canvas = canvas;
		this.ctx = canvas.getContext('2d', { alpha: false });
		this.cellCanvas = document.createElement('canvas');
		this.cellCtx = this.cellCanvas.getContext('2d', { alpha: false });
		this.maskCanvas = document.createElement('canvas');
	}

	private setupCanvas() {
		const canvas = this.canvas!;
		const geo = computeGeometry(this.prefs.density);
		this.sender.resize(geo.X, geo.Y);
		const r = (this.renderer = new Renderer(this.sender.layout, this.sender.mode, geo.s, QUIET));
		canvas.width = r.width;
		canvas.height = r.height;
		canvas.style.width = `${r.width / geo.dpr}px`;
		canvas.style.height = `${r.height / geo.dpr}px`;
		const mask = this.maskCanvas!;
		mask.width = r.width;
		mask.height = r.height;
		const mctx = mask.getContext('2d')!;
		const mimg = mctx.createImageData(r.width, r.height);
		new Uint32Array(mimg.data.buffer).set(r.maskLayer());
		mctx.putImageData(mimg, 0, 0);
		this.cellCanvas!.width = this.sender.layout.X + 2 * QUIET;
		this.cellCanvas!.height = this.sender.layout.Y + 2 * QUIET;
		this.cellImage = this.cellCtx!.createImageData(this.cellCanvas!.width, this.cellCanvas!.height);
		new Uint32Array(this.cellImage.data.buffer).fill(r.ink);
		this.pending = false;
		this.prepare();
		this.flip();
		this.updateStats();
	}

	private prepare() {
		const t0 = performance.now();
		const { cells } = this.sender.nextFrame();
		this.buildTimes.push(performance.now() - t0);
		if (this.buildTimes.length > 30) this.buildTimes.shift();
		this.renderer.cellPixels(cells, new Uint32Array(this.cellImage!.data.buffer));
		this.cellCtx!.putImageData(this.cellImage!, 0, 0);
		this.pending = true;
	}

	private flip() {
		if (!this.pending) return;
		const ctx = this.ctx!;
		ctx.imageSmoothingEnabled = false;
		ctx.drawImage(this.cellCanvas!, 0, 0, this.canvas!.width, this.canvas!.height);
		ctx.drawImage(this.maskCanvas!, 0, 0);
		this.pending = false;
		this.shownFrames++;
	}

	// Flip on the display refresh closest to a fixed schedule, so any rate keeps its cadence
	// (45 fps on a 60 Hz screen alternates one and two refreshes instead of falling to 30).
	// Slow rates sleep until shortly before a flip; fast ones look at every refresh.
	schedule() {
		clearTimeout(this.tickTimer);
		if (!this.sender || !this.playing) return;
		const period = 1000 / this.prefs.fps;
		const wait = this.nextFlipAt - performance.now() - 20;
		if (period > 40 && wait > 0)
			this.tickTimer = window.setTimeout(() => requestAnimationFrame(this.tick), wait);
		else requestAnimationFrame(this.tick);
	}

	private tick = (ts: number) => {
		if (!this.sender || !this.playing) return;
		const period = 1000 / this.prefs.fps;
		// start, or after a pause or a stall: from now
		if (!this.nextFlipAt || ts - this.nextFlipAt > 2 * period) this.nextFlipAt = ts;
		if (ts + 4 >= this.nextFlipAt) {
			this.flip();
			this.prepare();
			this.nextFlipAt = Math.max(this.nextFlipAt + period, ts + period / 2);
		}
		this.schedule();
	};

	// ---------- start / reconfigure
	async start(name: string, bytes: Uint8Array, text?: string) {
		this.sending = true;
		this.preparing = true;
		this.isText = text !== undefined;
		this.label = text !== undefined ? text.replace(/\s+/g, ' ').slice(0, 40) : name;
		this.size = bytes.length;
		this.stats = null;
		this.prepared = await preparePayload(bytes, name);
		if (this.prefs.engine === 'wasm') await loadWasm(wasmUrl);
		this.newSender();
		this.preparing = false;
		this.playing = true;
		this.wake();
		this.showDock();
		this.schedule();
	}

	// Both engines emit bit-identical streams, so switching keeps the transfer id and the
	// receiver keeps its progress.
	private newSender(keepTid?: number) {
		const geo = computeGeometry(this.prefs.density);
		if (this.sender && this.sender.free) this.sender.free();
		const Impl = this.prefs.engine === 'wasm' ? WasmSender : Sender;
		this.sender = new Impl(this.prepared, {
			mode: this.prefs.mode,
			ecc: this.prefs.ecc,
			X: geo.X,
			Y: geo.Y,
			tid: keepTid
		});
		this.buildTimes = [];
		this.setupCanvas();
	}

	private updateStats() {
		if (!this.sender) return;
		const per = this.sender.bytesPerFrame();
		const rate = per * this.prefs.fps;
		const L = this.sender.layout;
		this.stats = {
			rate,
			perFrame: per,
			pass: this.prepared.payload.length / rate,
			X: L.X,
			Y: L.Y,
			bpc: MODES[this.sender.mode].bpc
		};
	}

	/** Called once a second by the page: the encode time shown in the dock. */
	sample() {
		if (this.buildTimes.length)
			this.buildMs = this.buildTimes.reduce((a, b) => a + b, 0) / this.buildTimes.length;
	}

	async set<K extends 'density' | 'mode' | 'ecc' | 'engine'>(key: K, value: Prefs[K]) {
		if (this.prefs[key] === value) return;
		this.prefs[key] = value;
		this.savePrefs();
		if (!this.sender) return;
		if (key === 'density') this.setupCanvas();
		else if (key === 'engine') {
			if (value === 'wasm') await loadWasm(wasmUrl);
			this.newSender(this.sender.tid);
		} else this.reconfigure();
	}

	// Mode/ECC changes keep the same transfer (and the receiver's progress) when the
	// packet size can be preserved; otherwise a new transfer starts.
	private reconfigure() {
		const keep = this.sender.reconfigure({ mode: this.prefs.mode, ecc: this.prefs.ecc });
		if (!keep) this.newSender();
		else this.setupCanvas();
	}

	setFps(fps: number) {
		this.prefs.fps = fps;
		this.schedule();
		this.savePrefs();
		this.updateStats();
	}

	setSource(v: 'file' | 'text') {
		this.prefs.source = v;
		this.savePrefs();
	}

	togglePlay() {
		this.playing = !this.playing;
		this.schedule();
		this.armDockTimer();
	}

	resized() {
		if (this.sender) this.setupCanvas();
	}

	close() {
		clearTimeout(this.tickTimer);
		if (this.sender && this.sender.free) this.sender.free();
		this.sender = null;
		this.prepared = null;
		this.sending = false;
		this.stats = null;
	}

	get active() {
		return !!this.sender;
	}

	// ---------- dock visibility
	// Hidden by: the hide button, a tap/click on the code, H/Esc, or 2.6 s of inactivity while playing.
	// Shown by: a tap/click on the code, the mouse reaching the bottom edge, or any key.
	dockHovered = false;
	hideDock() {
		clearTimeout(this.dockTimer);
		this.dockAway = true;
	}
	showDock() {
		this.dockAway = false;
		this.armDockTimer();
	}
	armDockTimer() {
		clearTimeout(this.dockTimer);
		this.dockTimer = window.setTimeout(() => {
			if (this.sender && this.playing && !this.dockHovered) this.hideDock();
			else if (!this.dockAway) this.armDockTimer();
		}, 2600);
	}
	pokeDock() {
		if (!this.dockAway) this.armDockTimer();
	}
	toggleDock() {
		if (this.dockAway) this.showDock();
		else this.hideDock();
	}

	// keep the screen on
	async wake() {
		try {
			if ('wakeLock' in navigator && !this.wakeLock) {
				this.wakeLock = await navigator.wakeLock.request('screen');
				this.wakeLock.addEventListener('release', () => (this.wakeLock = null));
			}
		} catch {
			/* not available */
		}
	}

	destroy() {
		clearTimeout(this.tickTimer);
		clearTimeout(this.dockTimer);
		if (this.sender && this.sender.free) this.sender.free();
		this.sender = null;
		if (this.wakeLock) this.wakeLock.release().catch(() => {});
	}
}
