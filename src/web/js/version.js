// The build of these pages: the commit of the sources, its date, and whether the sources had
// uncommitted changes. src/tools/build.js writes the real values into dist/js/version.js; the
// sources served by the dev server say "dev".
export const BUILD = { commit: 'dev', full: '', date: '', dirty: true, protocol: '1.0' };

// "Version 1a2b3c4 · 2026-10-20" — what a user quotes in a report
export function buildText(label) {
  if (BUILD.commit === 'dev') return `${label} dev`;
  return `${label} ${BUILD.commit}${BUILD.dirty ? '+dev' : ''} · ${BUILD.date}`;
}
