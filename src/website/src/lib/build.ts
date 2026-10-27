// The version of this build (vite.config.ts), what a user quotes in a report.
export const BUILD = __BUILD__;
export const REPO = 'https://github.com/atshelchin/lumen-code';

/** "1a2b3c4 · 2026-10-20", "+dev" when the sources had uncommitted changes. */
export function buildText(): string {
	return `${BUILD.commit}${BUILD.dirty ? '+dev' : ''}${BUILD.date ? ` · ${BUILD.date}` : ''}`;
}

export const commitUrl = BUILD.full ? `${REPO}/commit/${BUILD.full}` : REPO;
