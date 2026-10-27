import { expect, test, type Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { camVideo } from '../playwright.config';

// The pages count visits on tj.appsdata.org; tests load the counter but do not report to it.
test.beforeEach(async ({ context }) => {
	await context.route('https://tj.appsdata.org/api/track*', (route) =>
		route.fulfill({ status: 204 })
	);
});

const pages = [
	['/', 'Lumen Code', 'Turn files into'],
	['/send', 'Send a file — Lumen Code', 'Choose file'],
	['/recv', 'Receive a file — Lumen Code', 'Open camera'],
	['/zh', 'Lumen Code — 用屏幕和相机传文件', '把文件变成'],
	['/zh/send', '发送文件 — Lumen Code', '选择文件'],
	['/zh/recv', '接收文件 — Lumen Code', '打开相机'],
	['/privacy', 'Privacy Policy — Lumen Code', 'never leave your devices'],
	['/terms', 'Terms of Service — Lumen Code', 'MIT License'],
	['/zh/privacy', '隐私政策 — Lumen Code', '不离开你的设备'],
	['/zh/terms', '服务条款 — Lumen Code', 'MIT 许可证']
] as const;

// Search engines and the first paint read the HTML as served: the words must be in it.
for (const [url, title, text] of pages) {
	test(`prerendered ${url}: title, words, language versions, version`, async ({ request }) => {
		const html = await (await request.get(url)).text();
		expect(html).toContain(`<title>${title}`);
		expect(html).toContain(text);
		expect(html).toContain(`<html lang="${url.startsWith('/zh') ? 'zh-CN' : 'en'}"`);
		expect(html).toMatch(/rel="canonical" href="https:\/\/lumencode\.link[^"]*"/);
		expect(html).toContain('hreflang="x-default"');
		expect(html).toContain(
			'<script src="https://tj.appsdata.org/api/script.js" data-site-id="b9594b9f1c70" defer></script>'
		);
		expect(html).toMatch(/data-build[^>]*>[^<]*[0-9a-f]{7}/);
	});
}

test('home page: pricing, the store apps not yet published, the receiver screenshot', async ({
	page,
	request
}) => {
	for (const [url, soon] of [
		['/', 'Coming soon'],
		['/zh', '即将上架']
	]) {
		await page.goto(url);
		const pricing = page.locator('#pricing');
		await expect(pricing.locator('.plan')).toHaveCount(3);
		await expect(pricing.locator('.plan.featured .price')).toContainText('US$2.99');
		await expect(pricing.locator('.store')).toHaveText([
			`App Store · ${soon}`,
			`Google Play · ${soon}`
		]);
		const shot = await page.locator('img.phone').getAttribute('src');
		expect((await request.get(shot!)).status()).toBe(200);
	}
});

test('the home page links the privacy policy and the terms, in its language', async ({ page }) => {
	await page.goto('/zh');
	await page.click('footer >> text=隐私政策');
	await expect(page).toHaveURL(/\/zh\/privacy$/);
	await expect(page.locator('h1')).toHaveText('隐私政策');
	await page.click('footer >> text=服务条款');
	await expect(page).toHaveURL(/\/zh\/terms$/);
	await page.click('#lang');
	await expect(page).toHaveURL(/:4173\/terms$/);
	await expect(page.locator('h1')).toHaveText('Terms of Service');
});

test('the content security policy: no violations while sending and receiving', async ({ page }) => {
	const violations: string[] = [];
	await page.exposeFunction('cspViolation', (v: string) => violations.push(v));
	await page.addInitScript(() =>
		document.addEventListener('securitypolicyviolation', (e) =>
			(window as unknown as { cspViolation(v: string): void }).cspViolation(
				`${e.violatedDirective} ${e.blockedURI}`
			)
		)
	);
	for (const url of ['/', '/zh/privacy', '/send?demo=20000', '/recv']) {
		await page.goto(url);
		await page.waitForTimeout(800);
	}
	const csp = await page
		.locator('meta[http-equiv="content-security-policy"]')
		.getAttribute('content');
	expect(csp).toContain(
		"script-src 'self' 'wasm-unsafe-eval' https://tj.appsdata.org/api/script.js"
	);
	expect(violations).toEqual([]);
});

test('sitemap lists every page in both languages; robots points to it', async ({ request }) => {
	const xml = await (await request.get('/sitemap.xml')).text();
	expect(xml.match(/<loc>/g)?.length).toBe(10);
	expect(await (await request.get('/robots.txt')).text()).toContain(
		'Sitemap: https://lumencode.link/sitemap.xml'
	);
	expect((await request.get('/no-such-page')).status()).toBe(404);
});

function collectErrors(page: Page) {
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
	return errors;
}

for (const [engine, lang] of [
	['wasm', 'en'],
	['js', 'zh']
] as const) {
	test(`receiver (${engine}, ${lang}): the fake camera delivers the file intact`, async ({
		page
	}) => {
		const errors = collectErrors(page);
		const want = createHash('sha256')
			.update(readFileSync(camVideo + '.src'))
			.digest('hex');
		await page.goto(`${lang === 'zh' ? '/zh' : ''}/recv?engine=${engine}`);
		await page.click('#startBtn');
		await expect(page.locator('#done')).toBeVisible({ timeout: 90_000 });
		const got = await page.evaluate(async () => {
			const href = (document.getElementById('saveBtn') as HTMLAnchorElement).href;
			const b = await (await fetch(href)).arrayBuffer();
			return [...new Uint8Array(await crypto.subtle.digest('SHA-256', b))]
				.map((x) => x.toString(16).padStart(2, '0'))
				.join('');
		});
		expect(got).toBe(want);
		await expect(page.locator('#doneStats')).toContainText(engine.toUpperCase());
		expect(errors).toEqual([]);
	});
}

test('home page draws one still frame with the protocol library', async ({ page }) => {
	const errors = collectErrors(page);
	await page.goto('/');
	await expect
		.poll(() =>
			page.evaluate(() => {
				const c = document.getElementById('sample') as HTMLCanvasElement;
				const px = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data;
				let yellow = 0;
				for (let i = 0; i < px.length; i += 4) if (px[i] > 200 && px[i + 2] < 60) yellow++; // C4 cells
				return c.width > 300 && yellow > 1000;
			})
		)
		.toBe(true);
	expect(errors).toEqual([]);
});

test('sender: typed text becomes moving frames', async ({ page }) => {
	const errors = collectErrors(page);
	await page.goto('/send');
	await page.click('#srcTabs [data-v="text"]');
	await page.fill('#textInput', 'Lumen Code browser test ' + 'x'.repeat(5000));
	await page.click('#textSend');
	await expect(page.locator('#codewrap')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => (document.getElementById('code') as HTMLCanvasElement).width))
		.toBeGreaterThan(100);
	expect(errors).toEqual([]);
});

test.describe('languages', () => {
	test('a saved choice opens the other language before anything is drawn', async ({ page }) => {
		await page.goto('/send');
		await page.evaluate(() => localStorage.setItem('lumen.lang', 'zh'));
		await page.goto('/send');
		await expect(page).toHaveURL(/\/zh\/send$/);
		await page.evaluate(() => localStorage.setItem('lumen.lang', 'en'));
		await page.goto('/zh');
		await expect(page).toHaveURL(/:4173\/$/);
	});

	test('the language button switches and is remembered', async ({ page }) => {
		await page.goto('/');
		await page.click('#lang');
		await expect(page).toHaveURL(/\/zh$/);
		await expect(page.locator('h1')).toContainText('把文件变成');
		await page.click('#lang');
		await expect(page).toHaveURL(/:4173\/$/);
	});
});

test.describe('a Chinese browser with no saved choice', () => {
	test.use({ locale: 'zh-CN' });
	test('English pages open their Chinese versions; Chinese pages stay', async ({ page }) => {
		await page.goto('/recv');
		await expect(page).toHaveURL(/\/zh\/recv$/);
		await page.goto('/zh/send');
		await expect(page).toHaveURL(/\/zh\/send$/);
	});
});

test.describe('an English browser with no saved choice', () => {
	test.use({ locale: 'en-US' });
	test('Chinese pages stay (crawlers read them)', async ({ page }) => {
		await page.goto('/zh/recv');
		await expect(page).toHaveURL(/\/zh\/recv$/);
	});
});
