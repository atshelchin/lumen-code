// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	/** The version of this build (vite.config.ts): the last commit of the site's sources. */
	const __BUILD__: { commit: string; full: string; date: string; dirty: boolean; protocol: string };
	/** The public address of the site, e.g. https://lumencode.awesometools.dev */
	const __SITE__: string;

	namespace App {
		interface Platform {
			env: Env;
			ctx: ExecutionContext;
			caches: CacheStorage;
			cf?: IncomingRequestCfProperties;
		}

		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
	}
}

export {};
