import type { ParamMatcher } from '@sveltejs/kit/params';

// Route parameters. lang: the optional first path segment, "zh" for the Chinese pages.
const lang: ParamMatcher<'zh'> = {
	'~standard': {
		version: 1,
		vendor: 'lumen-code',
		validate: (value) =>
			value === 'zh' ? { value: 'zh' } : { issues: [{ message: 'not a language' }] }
	}
};

export const params = { lang };
