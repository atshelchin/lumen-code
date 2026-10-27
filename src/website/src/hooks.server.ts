import type { Handle } from '@sveltejs/kit/hooks';

// <html lang> of each page: the Chinese pages live under /zh
export const handle: Handle = ({ event, resolve }) =>
	resolve(event, {
		transformPageChunk: ({ html }) =>
			html.replace('%lang%', event.params.lang === 'zh' ? 'zh-CN' : 'en')
	});
