import { describe, expect, it } from 'vitest';
import { linkParts, safeName } from './text-view';

describe('text view', () => {
	it('links stop before trailing punctuation and Chinese text', () => {
		expect(linkParts('see https://example.com/a?b=1. then')).toEqual([
			{ text: 'see ' },
			{ text: 'https://example.com/a?b=1', href: 'https://example.com/a?b=1' },
			{ text: '. then' }
		]);
		expect(linkParts('打开https://x.dev/p，谢谢')[1]).toEqual({
			text: 'https://x.dev/p',
			href: 'https://x.dev/p'
		});
	});

	it('received names are never paths or disguised extensions', () => {
		expect(safeName('../../etc/passwd')).toBe('.._.._etc_passwd');
		expect(safeName('photo‮gnp.exe')).toBe('photognp.exe');
		expect(safeName('..')).toBe('file');
		expect(safeName(null)).toBe('file');
	});
});
