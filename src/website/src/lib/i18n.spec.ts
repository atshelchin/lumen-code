import { describe, expect, it } from 'vitest';
import { DICT, LANGS, path, t } from './i18n';

describe('i18n', () => {
	it('every language has every key, none empty', () => {
		const keys = Object.keys(DICT.en).sort();
		for (const lang of LANGS) {
			expect(Object.keys(DICT[lang]).sort()).toEqual(keys);
			for (const k of keys) expect(t(lang, k as keyof typeof DICT.en).trim()).not.toBe('');
		}
	});

	it('placeholders match across languages', () => {
		for (const k of Object.keys(DICT.en) as (keyof typeof DICT.en)[]) {
			const holes = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort();
			expect(holes(DICT.zh[k]), k).toEqual(holes(DICT.en[k]));
		}
	});

	it('paths: English at the root, Chinese under /zh', () => {
		expect([path('en', ''), path('en', 'send'), path('zh', ''), path('zh', 'recv')]).toEqual([
			'/',
			'/send',
			'/zh',
			'/zh/recv'
		]);
	});
});
