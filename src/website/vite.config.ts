import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import adapter from '@sveltejs/adapter-cloudflare';
import basicSsl from '@vitejs/plugin-basic-ssl';
import { sveltekit } from '@sveltejs/kit/vite';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const repo = fileURLToPath(new URL('../..', import.meta.url));
const SITE = 'https://lumencode.awesometools.dev';

// The inline scripts of src/app.html (the language redirect) are not SvelteKit's, so the CSP gets
// their hashes from here; they follow any edit to the file.
const appScripts = [
	...readFileSync(new URL('src/app.html', import.meta.url), 'utf8').matchAll(
		/<script>([\s\S]*?)<\/script>/g
	)
].map((m) => `sha256-${createHash('sha256').update(m[1]).digest('base64')}`);

// The version shown on the pages: the last commit that changed a source of this site (the site
// itself, the JS protocol library, the Rust code of the WASM module, the docs it links), its
// date, and "dirty" when these sources have uncommitted changes. Commit first, then build.
const SOURCES = ['src/website', 'src/core', 'src/rust', 'docs', ':!docs/STATUS.md', 'LICENSE'];
function buildInfo() {
	const git = (...args: string[]) =>
		execFileSync('git', args, {
			cwd: repo,
			encoding: 'utf8',
			stdio: ['ignore', 'pipe', 'ignore']
		}).trim();
	try {
		const full = git('log', '-1', '--format=%H', '--', ...SOURCES);
		return {
			commit: full.slice(0, 7),
			full,
			date: git('log', '-1', '--format=%cs', '--', ...SOURCES),
			dirty: git('status', '--porcelain', '--', ...SOURCES) !== '',
			protocol: '1.0'
		};
	} catch {
		return { commit: 'unknown', full: '', date: '', dirty: false, protocol: '1.0' };
	}
}

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter(),
			// pre-rendered pages see this origin: canonical links, sitemap, previews
			paths: { origin: SITE },
			// Prerendered pages get this as a <meta> tag, with hashes for the inline scripts. The visit
			// counter may load its one script file and send to its own host, nothing else, so its server
			// cannot switch on extras such as session replay. WebAssembly runs the faster engine; blob:
			// URLs show and save received files; frame-ancestors and the headers a <meta> tag cannot
			// carry are in _headers.
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': [
						'self',
						'wasm-unsafe-eval',
						'https://tj.appsdata.org/api/script.js',
						...(appScripts as `sha256-${string}`[])
					],
					'worker-src': ['self'],
					'connect-src': ['self', 'blob:', 'https://tj.appsdata.org'],
					'style-src': ['self'],
					'style-src-attr': ['unsafe-inline'],
					'img-src': ['self', 'data:', 'blob:'],
					'media-src': ['self', 'blob:', 'mediastream:'],
					'object-src': ['none'],
					'base-uri': ['self'],
					'form-action': ['none']
				}
			}
		}),
		// `bun run dev:https`: phones on the local network need HTTPS for the camera
		process.env.HTTPS ? basicSsl() : null
	],
	define: { __BUILD__: JSON.stringify(buildInfo()), __SITE__: JSON.stringify(SITE) },
	// the protocol library (src/core) and the WASM module (dist/wasm) live outside this project
	server: { fs: { allow: [repo] } },
	worker: { format: 'es' },
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'client',
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }]
					},
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					exclude: ['src/lib/server/**']
				}
			},
			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
