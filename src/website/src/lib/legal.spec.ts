import { describe, expect, it } from 'vitest';
import { LEGAL, inline } from './legal';
import { PRICE } from './landing';

describe('privacy policy and terms', () => {
	it('both languages have the same sections, in the same order', () => {
		for (const doc of ['privacy', 'terms'] as const) {
			const count = (lang: 'en' | 'zh') =>
				LEGAL[lang][doc].sections.map((s) => [s.p?.length ?? 0, s.list?.length ?? 0]);
			expect(count('zh')).toEqual(count('en'));
			expect(LEGAL.zh[doc].summary?.length).toBe(LEGAL.en[doc].summary?.length);
		}
	});

	it('the terms quote the current app price', () => {
		for (const lang of ['en', 'zh'] as const) {
			const text = JSON.stringify(LEGAL[lang].terms);
			expect(text).toContain(PRICE);
			expect(text.match(/US\$[\d.]+/g)?.every((p) => p === PRICE)).toBe(true);
		}
	});

	it('inline text: bold and links', () => {
		expect(inline('**Hosting.** See [the policy](https://x.dev/p). Done')).toEqual([
			{ text: 'Hosting.', bold: true },
			{ text: ' See ' },
			{ text: 'the policy', href: 'https://x.dev/p' },
			{ text: '. Done' }
		]);
	});
});
