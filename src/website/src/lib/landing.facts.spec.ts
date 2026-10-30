// The home page and the store descriptions state numbers about the protocol and the apps. Each
// test here recomputes a number from the implementation (src/core, the app core's sources, the
// web app) or from the protocol (docs/PROTOCOL.md) and checks that the texts say exactly that,
// in both languages. Change the code or the spec and these fail until the texts follow.
// Not covered: field measurements (the 96 KB/s end-to-end result), which the texts label as such.
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { LANDING } from './landing';
import { Sender, RECEIVER_LIMITS } from '../../../core/transfer.js';
import { MAX_BLOCK_SYMBOLS } from '../../../core/frame.js';
import { ECC_PERCENT, HEADER_BITS, MODES } from '../../../core/layout.js';

const repo = new URL('../../../../', import.meta.url);
const read = (path: string) => readFileSync(new URL(path, repo), 'utf8');
const LANGS = ['en', 'zh'] as const;
const page = (lang: (typeof LANGS)[number]) => JSON.stringify(LANDING[lang]);
const store = { en: read('store/description-en.txt'), zh: read('store/description-zh.txt') };
const coreApp = read('src/app/core/src/app/mod.rs');
const coreView = read('src/app/core/src/app/view.rs');
const sendPage = read('src/website/src/routes/[[lang=lang]]/send/+page.svelte');
const webReceiver = read('src/website/src/lib/recv/receiver.svelte.ts');
const num = (re: RegExp, text: string) => Number(re.exec(text)?.[1]);

/** Payload bytes per frame of the reference sender for a grid (default ECC). */
function bytesPerFrame(X: number, Y: number, mode: number) {
	const payload = new Uint8Array(3_000_000);
	const meta = {
		name: 'x.bin',
		fileSize: payload.length,
		crc: 0,
		flags: 0,
		payloadLength: payload.length
	};
	const tx = new Sender({ payload, meta }, { mode, ecc: 1, X, Y });
	return (tx.perFrame ?? 0) * (tx.T ?? 0);
}
const kb = (bytes: number, digits = 1) => (bytes / 1024).toFixed(digits); // KB = 1024 bytes
const rate = (bytes: number) => Math.round((bytes * 12) / 1024); // KB/s at 12 frames a second

describe('performance table', () => {
	for (const lang of LANGS) {
		it(`${lang}: per-frame payload and raw rate come from the sender`, () => {
			const rows = LANDING[lang].speed.rows;
			const grid = (cell: string) => cell.split('×').map((n) => Number(n.trim()));
			const [phoneX, phoneY] = grid(rows[0][2]);
			const [lapX, lapY] = grid(rows[0][3]);
			// animated QR v25 at ECC L: 17 + 4·25 modules, 1273 bytes in byte mode (ISO/IEC 18004)
			expect(rows[0][1]).toBe('117 × 117');
			expect(rows[2][1]).toBe(`${kb(1273, 2)} KB`);
			expect(rows[3][1]).toBe(`${rate(1273)} KB/s`);
			// bits per cell: 4 colours (C4) on the phone, 4 or 8 colours (C8) on the laptop
			expect(rows[1][2]).toBe(String(MODES[1].bpc));
			expect(rows[1][3]).toBe(`${MODES[1].bpc} / ${MODES[2].bpc}`);
			const phone = bytesPerFrame(phoneX, phoneY, 1);
			const lap4 = bytesPerFrame(lapX, lapY, 1);
			const lap8 = bytesPerFrame(lapX, lapY, 2);
			expect(rows[2][2]).toBe(`${kb(phone)} KB`);
			expect(rows[2][3]).toBe(`${kb(lap4)} / ${kb(lap8)} KB`);
			expect(rows[3][2]).toBe(`${rate(phone)} KB/s`);
			expect(rows[3][3]).toBe(`${rate(lap4)} / ${rate(lap8)} KB/s`);
			// the store descriptions quote the laptop figures
			expect(store[lang]).toContain(`${kb(lap4)} KB`);
			expect(store[lang]).toContain(`${rate(lap4)} KB/s`);
		});
	}
});

describe('settings the apps offer', () => {
	it('error correction: the levels every interface offers', () => {
		const appMax = num(/self\.ecc = self\.ecc\.min\((\d)\)/, coreApp);
		const webLevels = [...sendPage.matchAll(/\[(\d), 'e(?:Low|Mid|High|Max)'\]/g)].map((m) =>
			Number(m[1])
		);
		expect(webLevels).toEqual([...Array(appMax + 1).keys()]);
		const offered = ECC_PERCENT.slice(0, appMax + 1); // 10, 16, 22 — very high (30) is library-only
		const dflt = ECC_PERCENT[num(/ecc: (\d), fps/, coreApp)];
		expect(page('en')).toContain(
			`${offered.slice(0, -1).join(', ')} or ${offered.at(-1)} % parity (${dflt} % by default)`
		);
		expect(page('zh')).toContain(
			`${offered.slice(0, -1).join(' %、')} % 或 ${offered.at(-1)} %（默认 ${dflt} %）`
		);
		expect(store.en).toContain(`${offered.slice(0, -1).join(', ')} or ${offered.at(-1)} % parity`);
		expect(store.zh).toContain(`${offered.slice(0, -1).join(' %、')} % 或 ${offered.at(-1)} %`);
	});

	it('frame rate: the range of the apps and the web, and the default', () => {
		const min = num(/pub const MIN_FPS: u32 = (\d+)/, coreApp);
		const max = num(/pub const MAX_FPS: u32 = (\d+)/, coreApp);
		expect([num(/min="(\d+)"/, sendPage), num(/max="(\d+)"/, sendPage)]).toEqual([min, max]);
		const dflt = num(/fps: (\d+), source/, coreApp);
		expect(page('en')).toContain(`${min}–${max} per second`);
		expect(page('en')).toContain(`${min}–${max} fps (${dflt} by default)`);
		expect(page('zh')).toContain(`每秒 ${min}–${max} 帧`);
		expect(page('zh')).toContain(`${min}–${max} fps（默认 ${dflt}）`);
	});

	it('camera: the cell sizes at which the receivers stop and become steady', () => {
		for (const src of [webReceiver, coreView]) {
			expect(src).toMatch(/cell < 3\.4/);
			expect(src).toMatch(/cell < 4\.5/);
		}
		expect(page('en')).toContain('at least 3.4 pixels per cell, 4.5 or more');
		expect(page('zh')).toContain('每格至少 3.4 像素，4.5 以上');
	});
});

describe('protocol figures', () => {
	it('receiver size limit (JS and Rust reference receivers)', () => {
		const mib = RECEIVER_LIMITS.maxSize / 2 ** 20;
		expect(read('src/rust/src/transfer.rs')).toContain(`max_size: ${mib} << 20`);
		for (const lang of LANGS) {
			expect(page(lang)).toContain(`${mib} MiB`);
			expect(store[lang]).toContain(`${mib} MiB`);
		}
	});

	it('block size, header bits', () => {
		const block = MAX_BLOCK_SYMBOLS.toLocaleString('en').replace(',', ' ');
		for (const lang of LANGS) {
			expect(page(lang)).toContain(`≤ ${block}`);
			expect(page(lang)).toContain(String(HEADER_BITS));
		}
	});

	it('fountain overhead: the means of Appendix C', () => {
		const spec = read('docs/PROTOCOL.md');
		const appendix = spec.slice(spec.indexOf('### C.2 Fountain overhead'));
		const mean = (k: string) =>
			new RegExp(`^\\| ${k} \\|(?:[^|]*\\|){9} ([\\d.]+) \\|`, 'm').exec(appendix)?.[1];
		const [small, large] = [mean('1000'), mean('16 384')];
		expect(small).toBeTruthy();
		expect(large).toBeTruthy();
		expect(page('en')).toContain(`e averages ${small} up to K = 1000 and ${large} at K = 16 384`);
		expect(page('zh')).toContain(`K ≤ 1000 时 e 平均 ${small}，K = 16 384 时平均 ${large}`);
		expect(store.en).toContain(`on average ${small} extra for blocks of up to 1000 symbols`);
		expect(store.zh).toContain(`1000 个符号以内的块平均多 ${small} 个`);
	});

	it('the worked example: 1 MB at 50 KB/s', () => {
		expect(Math.round(1024 / 50)).toBe(20);
		expect(page('en')).toContain('1 MB takes about 20 s at 50 KB/s');
		expect(page('zh')).toContain('按 50 KB/s 计算 1 MB 约 20 秒');
	});
});
