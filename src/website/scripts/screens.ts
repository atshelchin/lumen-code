// Screenshots of the web app for the home page (static/screens/), taken from a running build
// with Chrome's fake camera playing a recorded transfer: a portrait phone camera, the code at a
// comfortable distance (rendered once through the camera simulator, about a minute).
//   bun run build && bunx vite preview --port 4173 &   then   bun scripts/screens.ts
import { chromium, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';

const repo = path.resolve(import.meta.dirname, '../../..');
const cam = path.join(repo, 'test/out/hero.y4m');
if (!existsSync(cam))
	execFileSync(
		'node',
		[
			'test/make-y4m.js',
			cam,
			'150000',
			'1',
			'100',
			'170',
			'8',
			'720',
			'1280',
			'0.62',
			'holiday-photos.zip'
		],
		{ cwd: repo, stdio: 'inherit' }
	);
const out = path.join(import.meta.dirname, '../static/screens');
const base = process.argv[2] || 'http://localhost:4173';
mkdirSync(out, { recursive: true });

const browser = await chromium.launch({
	args: [
		'--use-fake-device-for-media-stream',
		`--use-file-for-fake-video-capture=${cam}`,
		'--use-fake-ui-for-media-stream'
	]
});
// screenshots are not visits
const blockCounter = (page: Page) =>
	page.route('https://tj.appsdata.org/api/track*', (route) => route.fulfill({ status: 204 }));
const phone = {
	viewport: { width: 390, height: 844 },
	deviceScaleFactor: 1.5,
	isMobile: true,
	hasTouch: true
};

async function shot(page: Page, file: string) {
	await page.screenshot({ path: path.join(out, file), type: 'jpeg', quality: 84 });
	console.log('wrote', file);
}

for (const lang of ['en', 'zh']) {
	const prefix = lang === 'zh' ? '/zh' : '';
	// the receiver part way through a transfer
	const page = await browser.newPage(phone);
	await blockCounter(page);
	await page.goto(`${base}${prefix}/recv?engine=wasm`);
	await page.click('#startBtn');
	await page.waitForFunction(
		() => parseInt(document.getElementById('rxPct')?.textContent || '0') >= 55,
		null,
		{ timeout: 60_000 }
	);
	await shot(page, `recv-${lang}.jpg`);
	await page.close();
}
await browser.close();
