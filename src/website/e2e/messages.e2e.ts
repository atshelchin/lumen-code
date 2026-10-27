// Text messages: a short message fits one frame, so a still image of that frame (loaded with
// "from image or video") delivers it. Covers the reader view: JSON formatting, links, copy.
import { expect, test, type Page } from '@playwright/test';
import { Sender, preparePayload } from '../../core/transfer.js';
import { Renderer } from '../../core/render.js';
import { writePNG } from '../../../test/png.js';

async function frameOf(text: string, file: string) {
	const prepared = await preparePayload(new TextEncoder().encode(text), 'message.txt');
	const tx = new Sender(prepared, { mode: 1, X: 100, Y: 100 });
	if (tx.K > (tx.perFrame ?? 0) - 2) throw new Error('message too long for one frame');
	const { cells, layout } = tx.nextFrame();
	const r = new Renderer(layout, 1, 8);
	const rgba = new Uint8Array(r.width * r.height * 4);
	r.draw(cells, new Uint32Array(rgba.buffer));
	writePNG(file, r.width, r.height, rgba);
}

async function receive(page: Page, text: string, file: string, url = '/recv') {
	await frameOf(text, file);
	await page.goto(url);
	await page.setInputFiles('#mediaFile', file);
	await expect(page.locator('#done')).toBeVisible({ timeout: 20_000 });
}

test('a JSON message opens formatted, with the raw view and copy', async ({
	page,
	context
}, info) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const value = { name: 'Lumen <b>Code</b>', tags: ['a', 'b'], n: 1.5, ok: true, none: null };
	const raw = JSON.stringify(value);
	await receive(page, raw, info.outputPath('json.png'));
	await expect(page.locator('#doneName')).toHaveText('JSON received');
	// formatted: one line per value, markup in strings shown as text, not parsed
	const pre = page.locator('#preview pre.json');
	await expect(pre.locator('.l')).toHaveCount(JSON.stringify(value, null, 2).split('\n').length);
	await expect(pre).toContainText('"Lumen <b>Code</b>"');
	await expect(pre.locator('b')).toHaveCount(0);
	await page.click('#copyBtn');
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
		JSON.stringify(value, null, 2)
	);
	// raw: exactly what was sent
	await page.click('#segView [data-v="raw"]');
	await expect(page.locator('#preview pre')).toHaveText(raw);
	await page.click('#copyBtn');
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(raw);
	// the choice is remembered for the next message
	expect(await page.evaluate(() => JSON.parse(localStorage.getItem('lumen.reader')!).view)).toBe(
		'raw'
	);
});

test('a text message shows its links; a lone link gets an Open button', async ({ page }, info) => {
	await receive(
		page,
		'你好，见 https://example.com/a?b=1。谢谢',
		info.outputPath('text.png'),
		'/zh/recv'
	);
	await expect(page.locator('#doneName')).toHaveText('收到文本');
	const link = page.locator('#preview a');
	await expect(link).toHaveAttribute('href', 'https://example.com/a?b=1');
	await expect(link).toHaveAttribute('rel', 'noopener noreferrer');
	await expect(page.locator('#preview pre')).toHaveText('你好，见 https://example.com/a?b=1。谢谢');
	await expect(page.locator('#linkBtn')).toHaveCount(0);
	await expect(page.locator('#openBtn')).toHaveCount(0); // a message is not a file to open

	await page.click('#againBtn');
	await expect(page.locator('#done')).toHaveCount(0);
	await receive(page, 'https://example.com/only', info.outputPath('link.png'), '/zh/recv');
	await expect(page.locator('#linkBtn')).toHaveAttribute('href', 'https://example.com/only');
});
