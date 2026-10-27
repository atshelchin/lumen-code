<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { t as tr, path, type Key } from '#lib/i18n';
	import { REPO } from '#lib/build';
	import Mark from '#lib/ui/Mark.svelte';
	import Warn from '#lib/ui/Warn.svelte';
	import BuildLink from '#lib/ui/BuildLink.svelte';
	import Seo from '#lib/ui/Seo.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const lang = $derived(data.lang);
	const t = (key: Key) => tr(lang, key);
	const docs = $derived(
		lang === 'zh'
			? { spec: 'PROTOCOL.zh-CN.md', guide: 'guide/README.zh-CN.md' }
			: { spec: 'PROTOCOL.md', guide: 'guide/README.md' }
	);

	// the other language: remembered, then opened (the redirect in app.html honours the choice)
	function switchLang() {
		const next = lang === 'zh' ? 'en' : 'zh';
		try {
			localStorage.setItem('lumen.lang', next);
		} catch {
			/* ignore */
		}
		goto(path(next, ''));
	}

	// One real frame, drawn by the protocol library. It stays still: no flashing on this page.
	let canvas: HTMLCanvasElement;
	onMount(async () => {
		const { Sender, Renderer, crc32 } = await import('#lib/protocol');
		const text = new TextEncoder().encode('Lumen Code: files as moving light. '.repeat(40));
		const meta = {
			name: 'hello.txt',
			fileSize: text.length,
			crc: crc32(text),
			flags: 0,
			payloadLength: text.length
		};
		const tx = new Sender({ payload: text, meta }, { mode: 1, ecc: 1, X: 86, Y: 54, tid: 0x4c43 });
		tx.seq = 7;
		const { cells, layout } = tx.nextFrame();
		const r = new Renderer(layout, 1, 4);
		canvas.width = r.width;
		canvas.height = r.height;
		const g = canvas.getContext('2d')!;
		const img = g.createImageData(r.width, r.height);
		r.draw(cells, new Uint32Array(img.data.buffer));
		g.putImageData(img, 0, 0);
	});

	const jsonLd = $derived({
		'@context': 'https://schema.org',
		'@type': 'SoftwareApplication',
		name: 'Lumen Code',
		description: t('descHome'),
		url: `${__SITE__}${path(lang, '')}`,
		applicationCategory: 'UtilitiesApplication',
		operatingSystem: 'Web, macOS, Windows, Linux, iOS, Android',
		license: 'https://opensource.org/licenses/MIT',
		offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' }
	});
</script>

<Seo {lang} page="" title={t('titleHome')} description={t('descHome')} {jsonLd} />

<main class="landing">
	<header>
		<span class="brand"><Mark />Lumen Code</span>
		<span class="tools">
			<a class="gh" href={REPO} title={t('ghTitle')} aria-label={t('ghTitle')}>
				<svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"
					><path
						d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"
					/></svg
				>
			</a>
			<button class="lang" id="lang" onclick={switchLang}>中 / EN</button>
		</span>
	</header>

	<section class="hero">
		<div class="hero-text">
			<h1><span>{t('heroA')}</span><br /><em>{t('heroB')}</em></h1>
			<p>{t('heroP')}</p>
		</div>
		<figure class="hero-code">
			<canvas id="sample" width="360" height="232" aria-hidden="true" bind:this={canvas}></canvas>
			<figcaption>{t('sampleCap')}</figcaption>
		</figure>
	</section>

	<section class="choices">
		<a class="choice" href={path(lang, 'send')}>
			<span class="ico">
				<svg
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="1.8"
					stroke-linecap="round"
					stroke-linejoin="round"
					><rect x="3" y="4" width="18" height="13" rx="2.5" /><path d="M8 21h8M12 17v4" /><path
						d="M9.5 10.5 12 8l2.5 2.5M12 8v6"
					/></svg
				>
			</span>
			<h2>{t('sendTitle')}</h2>
			<p>{t('sendDesc')}</p>
			<span class="go">{t('go')}</span>
		</a>
		<a class="choice recv" href={path(lang, 'recv')}>
			<span class="ico">
				<svg
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="1.8"
					stroke-linecap="round"
					stroke-linejoin="round"
					><path
						d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2"
					/><circle cx="12" cy="12" r="3.2" /></svg
				>
			</span>
			<h2>{t('recvTitle')}</h2>
			<p>{t('recvDesc')}</p>
			<span class="go">{t('go')}</span>
		</a>
	</section>

	<Warn text={t('flashWarn')} />

	<section class="stats">
		<div><b>96 KB/s</b><span>{t('s1')}</span></div>
		<div><b>3 / 4</b><span>{t('s2')}</span></div>
		<div><b>0</b><span>{t('s3')}</span></div>
		<div><b>1.0</b><span>{t('s4')}</span></div>
	</section>

	<section class="facts">
		{#each [['f1t', 'f1p'], ['f2t', 'f2p'], ['f3t', 'f3p'], ['f4t', 'f4p']] as const as [h, p] (h)}
			<div>
				<h3>{t(h)}</h3>
				<p>{t(p)}</p>
			</div>
		{/each}
	</section>

	<section class="more">
		<div class="box">
			<h2>{t('devT')}</h2>
			<p>{t('devP')}</p>
			<p class="links">
				<a href={`${REPO}/blob/main/docs/${docs.spec}`}>{t('specLink')}</a>
				<a href={`${REPO}/blob/main/docs/${docs.guide}`}>{t('guideLink')}</a>
				<a href={`${REPO}/blob/main/test/vectors/test-vectors.json`}>{t('vectorsLink')}</a>
				<a href={REPO}>{t('ghLink')}</a>
			</p>
		</div>
		<div class="box">
			<h2>{t('appsT')}</h2>
			<p>{t('appsP')}</p>
			<p class="links">
				<a href={`${REPO}/releases`}>{t('releasesLink')}</a>
				<a href={`${REPO}#native-apps-ios-android-desktop`}>{t('buildLink')}</a>
			</p>
		</div>
	</section>

	<p class="foot">
		<span>{t('privacy')}</span> <span>{t('foot')}</span>
		<BuildLink label={t('buildLabel')} />
	</p>
</main>
