// The build of these pages: the last commit of the site's sources, its date, and whether they
// had uncommitted changes (src/tools/build.js). src/tools/build.js writes the real values into dist/js/version.js; the
// sources served by the dev server say "dev".
export const BUILD = {"commit":"9cc2078","full":"9cc20783ee33ce59f4581347123f2947f8d9979a","date":"2026-10-20","dirty":false,"protocol":"1.0"};

// "Version 1a2b3c4 · 2026-10-20" — what a user quotes in a report
export function buildText(label) {
  if (BUILD.commit === 'dev') return `${label} dev`;
  return `${label} ${BUILD.commit}${BUILD.dirty ? '+dev' : ''} · ${BUILD.date}`;
}
