import { describe, expect, it } from 'vitest';
import { LANDING, PRICE } from './landing';

// the same shape in both languages: every list has as many items, every text is filled
function shape(v: unknown): unknown {
	if (Array.isArray(v)) return v.map(shape);
	if (v && typeof v === 'object')
		return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, shape(x)]));
	return typeof v;
}
function texts(v: unknown): string[] {
	if (typeof v === 'string') return [v];
	if (v && typeof v === 'object') return Object.values(v).flatMap(texts);
	return [];
}

describe('home page content', () => {
	it('English and Chinese have the same structure, no empty text', () => {
		expect(shape(LANDING.zh)).toEqual(shape(LANDING.en));
		for (const lang of ['en', 'zh'] as const) {
			const { head, ...rest } = LANDING[lang].speed; // head[0] is the table's empty corner
			const all = [...texts({ ...LANDING[lang], speed: rest }), ...head.slice(1)];
			for (const s of all) expect(s.trim()).not.toBe('');
		}
	});

	it('one price, shown wherever the apps are priced', () => {
		expect(PRICE).toBe('US$2.99');
		for (const lang of ['en', 'zh'] as const) {
			const c = LANDING[lang];
			expect(c.pricing.store.price).toBe(PRICE);
			expect(c.pricing.title).toContain(PRICE);
			expect(c.hero.note).toContain(PRICE);
			// no other amount anywhere
			expect(
				texts(c)
					.join(' ')
					.match(/US\$[\d.]+/g)
			).toEqual(Array(3).fill(PRICE));
		}
	});
});
