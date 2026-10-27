# Lumen Code website

The web app at <https://lumencode.awesometools.dev>: the home page, the sender and the receiver,
in English (at the root) and Chinese (under `/zh`). SvelteKit with Svelte 5, built for Cloudflare
Workers. Every page is prerendered, so the words, titles and language links are in the HTML as
served (for the first paint and for search engines); the code only adds the moving parts.

The protocol itself is not here: the pages use the zero-dependency JavaScript library in
[`src/core`](../core) and the Rust engine compiled to WebAssembly (`dist/wasm/lumen.wasm`, built
from [`src/rust`](../rust) by `npm run build:wasm` at the repository root). Only this app has
dependencies.

```sh
bun install
bun run dev          # http://localhost:5173 (the camera needs localhost or HTTPS)
bun run build        # .svelte-kit/cloudflare: the Worker and the prerendered pages
bun run preview      # the build in the local Workers runtime, http://localhost:4173
bun run check        # types (svelte-check)
bun run lint         # prettier + eslint
bun run test:unit -- --run
bun run test:e2e     # Playwright, Chromium
```

## Layout

| Path | What |
| --- | --- |
| `src/routes/[[lang=lang]]/` | the three pages; `lang` is absent (English) or `zh` (`src/params.ts`) |
| `src/lib/send/sender.svelte.ts` | the sender: encoding, the frame loop, settings, the dock |
| `src/lib/recv/receiver.svelte.ts` | the receiver: camera, scan scheduling, workers, dashboard, result |
| `src/lib/recv/*.worker.ts` | frame decoding (one per core, up to 3) and the protocol receiver |
| `src/lib/i18n.ts` | all text, both languages, and the page paths |
| `src/lib/ui/Seo.svelte` | title, description, canonical and language links, link previews |
| `src/app.html` | opens the remembered language (or a Chinese browser's) before the first paint |
| `vite.config.ts` | the version shown on every page: the last commit of the site's sources |
| `e2e/` | the receiver with Chrome's fake camera, messages, sender, languages, the served HTML |

## Version

Every page shows `Version <commit> · <date>`, linked to that commit: the last commit that
changed the site's sources (`src/website`, `src/core`, `src/rust`, `docs`). `+dev` means the
build had uncommitted changes. Build from a committed tree for a release.

## Tests

`e2e/site.e2e.ts` plays a recorded transfer (60 000 random bytes, rendered through the camera
simulator of `test/camera.js`; `e2e/setup.ts` makes it once at `test/out/cam.y4m`) as Chrome's
webcam and checks that both engines, in both languages, deliver the file byte for byte.
`e2e/messages.e2e.ts` loads one-frame images of text messages to check the reader view.
