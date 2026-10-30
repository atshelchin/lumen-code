// The version of this build (vite.config.ts), what a user quotes in a report.
export const BUILD = __BUILD__;
export const REPO = 'https://github.com/mondaylabsltd/lumen-code';

/** "1.0.0 · 1a2b3c4 · 2026-10-20" (as in the apps), "+dev" when the sources had uncommitted changes. */
export function buildText(): string {
	return `${BUILD.version} · ${BUILD.commit}${BUILD.dirty ? '+dev' : ''}${BUILD.date ? ` · ${BUILD.date}` : ''}`;
}

export const commitUrl = BUILD.full ? `${REPO}/commit/${BUILD.full}` : REPO;
