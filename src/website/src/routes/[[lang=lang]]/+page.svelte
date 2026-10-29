<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { t as tr, path, type Key } from '#lib/i18n';
	import { LANDING, STORES } from '#lib/landing';
	import { COMPANY, LEGAL } from '#lib/legal';
	import { REPO } from '#lib/build';
	import Mark from '#lib/ui/Mark.svelte';
	import Warn from '#lib/ui/Warn.svelte';
	import BuildLink from '#lib/ui/BuildLink.svelte';
	import Seo from '#lib/ui/Seo.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const lang = $derived(data.lang);
	const t = (key: Key) => tr(lang, key);
	const c = $derived(LANDING[lang]);
	const zh = $derived(lang === 'zh');
	const docs = $derived(
		zh
			? { spec: 'PROTOCOL.zh-CN.md', guide: 'guide/README.zh-CN.md' }
			: { spec: 'PROTOCOL.md', guide: 'guide/README.md' }
	);
	const buildGuide = `${REPO}#native-apps-ios-android-desktop`;
	const stores = [
		{ name: 'App Store', url: STORES.appStore },
		{ name: 'Google Play', url: STORES.googlePlay }
	];

	// the other language: remembered, then opened (the redirect in app.html honours the choice)
	function switchLang() {
		const next = zh ? 'en' : 'zh';
		try {
			localStorage.setItem('lumen.lang', next);
		} catch {
			/* ignore */
		}
		goto(path(next, ''));
	}

	// One real frame, drawn by the protocol library. It stays still: no flashing on this page.
	let canvas = $state<HTMLCanvasElement>();
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
		canvas!.width = r.width;
		canvas!.height = r.height;
		const g = canvas!.getContext('2d')!;
		const img = g.createImageData(r.width, r.height);
		r.draw(cells, new Uint32Array(img.data.buffer));
		g.putImageData(img, 0, 0);
	});

	// 24-pixel line icons, in the order of c.features.items and c.apps.items
	const featureIcons = [
		'M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h2.5v2.5H14zM17.5 17.5H20V20h-2.5z',
		'M12 3.5c3.2 4.2 5.2 6.9 5.2 9.6a5.2 5.2 0 0 1-10.4 0c0-2.7 2-5.4 5.2-9.6zM9.6 13.6a2.6 2.6 0 0 0 2.4 2.5',
		'M4 9V4h5M15 4h5v5M4 15v5h5M15.5 15.5l5 5M20.5 15.5l-5 5',
		'M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 5l-3 14'
	];
	const appIcons = [
		'M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18zM3.5 12h17M12 3c2.4 2.7 3.6 5.7 3.6 9s-1.2 6.3-3.6 9c-2.4-2.7-3.6-5.7-3.6-9S9.6 5.7 12 3z',
		'M8 2.5h8a2 2 0 0 1 2 2v15a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2v-15a2 2 0 0 1 2-2zM10.5 5h3M11 18.5h2',
		'M7.5 3h9A1.5 1.5 0 0 1 18 4.5v15a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 6 19.5v-15A1.5 1.5 0 0 1 7.5 3zM12 5.6h.01M9.5 18.4h5',
		'M3.5 4.5h17a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-17a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1zM8 20.5h8M12 16.5v4'
	];

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

<div class="site">
	<a class="skip" href="#main">{zh ? '跳到正文' : 'Skip to content'}</a>

	<header class="bar">
		<nav class="wrap nav" aria-label="Lumen Code">
			<a class="brand" href={path(lang, '')}><Mark /><span>Lumen Code</span></a>
			<div class="links">
				<a href="#how">{c.nav.how}</a>
				<a href="#speed">{c.nav.speed}</a>
				<a href="#apps">{c.nav.apps}</a>
				<a href="#pricing">{c.nav.pricing}</a>
				<a href="#faq">{c.nav.faq}</a>
				<a class="gh" href={REPO} title={t('ghTitle')} aria-label={t('ghTitle')} rel="external">
					<svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"
						><path
							d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"
						/></svg
					>
				</a>
				<button class="pill" id="lang" onclick={switchLang} lang={zh ? 'en' : 'zh-CN'}
					>{zh ? 'English' : '中文'}</button
				>
			</div>
		</nav>
	</header>

	<main id="main">
		<section class="hero wrap">
			<div class="hero-text">
				<p class="eyebrow">{c.hero.eyebrow}</p>
				<h1><span>{t('heroA')}</span><br /><em>{t('heroB')}</em></h1>
				<p class="lead">{c.hero.lead}</p>
				<div class="actions">
					<a class="button primary" href={path(lang, 'send')}>{c.hero.send}</a>
					<a class="button tonal" href={path(lang, 'recv')}>{c.hero.recv}</a>
				</div>
				<p class="note">{c.hero.note}</p>
				<Warn text={t('flashWarn')} />
			</div>
			<div class="hero-visual">
				<figure class="screen">
					<canvas id="sample" width="360" height="232" aria-hidden="true" bind:this={canvas}
					></canvas>
					<figcaption>{t('sampleCap')}</figcaption>
				</figure>
				<img
					class="phone"
					src="/screens/recv-{lang}.jpg"
					alt={zh ? '手机正在接收文件' : 'A phone receiving a file'}
					width="585"
					height="1266"
				/>
			</div>
		</section>

		<section id="how" class="section wrap">
			<h2>{c.how.title}</h2>
			<ol class="flow" aria-label={c.how.flow.map(([a]) => a).join(' → ')}>
				{#each c.how.flow as [what, note], i (i)}
					<li class:mid={i > 0 && i < c.how.flow.length - 1}>
						<span class:mono={i === 0 || i === c.how.flow.length - 1}>{what}</span><small
							>{note}</small
						>
					</li>
				{/each}
			</ol>
			<ol class="steps">
				{#each c.how.steps as step, i (step.title)}
					<li>
						<span class="step-number">{i + 1}</span>
						<h3>{step.title}</h3>
						<p>{step.text}</p>
					</li>
				{/each}
			</ol>
		</section>

		<section id="speed" class="section wrap">
			<h2>{c.speed.title}</h2>
			<p class="section-lead">{c.speed.lead}</p>
			<div class="card table-card">
				<table>
					<thead>
						<tr>
							{#each c.speed.head as cell, i (i)}<th scope="col" class:ours={i === 2}>{cell}</th
								>{/each}
						</tr>
					</thead>
					<tbody>
						{#each c.speed.rows as row (row[0])}
							<tr><th scope="row">{row[0]}</th><td>{row[1]}</td><td class="ours">{row[2]}</td></tr>
						{/each}
					</tbody>
				</table>
			</div>
			<p class="caption">{c.speed.note}</p>
		</section>

		<section class="section wrap">
			<h2>{c.features.title}</h2>
			<ul class="grid three">
				{#each c.features.items as item, i (item.title)}
					<li class="card">
						<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
							><path d={featureIcons[i]} /></svg
						>
						<h3>{item.title}</h3>
						<p>{item.text}</p>
					</li>
				{/each}
			</ul>
		</section>

		<section id="apps" class="section wrap">
			<h2>{c.apps.title}</h2>
			<p class="section-lead">{c.apps.lead}</p>
			<ul class="grid four">
				{#each c.apps.items as item, i (item.name)}
					<li class="card">
						<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d={appIcons[i]} /></svg>
						<h3>{item.name}</h3>
						<p class="req">{item.req}</p>
						<p>{item.text}</p>
					</li>
				{/each}
			</ul>
			<div class="actions">
				<a class="button tonal" href={`${REPO}/releases`} rel="external">{t('releasesLink')}</a>
				<a class="button tonal" href={buildGuide} rel="external">{c.pricing.build.action}</a>
			</div>
		</section>

		<section class="section wrap">
			<h2>{c.open.title}</h2>
			<ul class="open">
				{#each c.open.items as item (item.title)}
					<li>
						<h3>{item.title}</h3>
						<p>{item.text}</p>
					</li>
				{/each}
			</ul>
			<div class="actions">
				<a class="button tonal" href={`${REPO}/blob/main/docs/${docs.spec}`} rel="external"
					>{t('specLink')}</a
				>
				<a class="button tonal" href={`${REPO}/blob/main/docs/${docs.guide}`} rel="external"
					>{t('guideLink')}</a
				>
				<a
					class="button tonal"
					href={`${REPO}/blob/main/test/vectors/test-vectors.json`}
					rel="external">{t('vectorsLink')}</a
				>
				<a class="button tonal" href={REPO} rel="external">{t('ghLink')}</a>
			</div>
		</section>

		<section id="pricing" class="section wrap">
			<h2>{c.pricing.title}</h2>
			<p class="section-lead">{c.pricing.lead}</p>
			<div class="plans">
				{#each [c.pricing.web, c.pricing.build, c.pricing.store] as plan, i (plan.name)}
					<div class="card plan" class:featured={i === 2}>
						<h3>{plan.name}</h3>
						<p class="price">{plan.price} <small>{plan.per}</small></p>
						<ul>
							{#each plan.points as point (point)}<li>{point}</li>{/each}
						</ul>
						{#if i === 0}
							<a class="button primary" href={path(lang, 'send')}>{plan.action}</a>
						{:else if i === 1}
							<a class="button tonal" href={buildGuide} rel="external">{plan.action}</a>
						{:else}
							<div class="stores">
								{#each stores as s (s.name)}
									{#if s.url}<a class="store" href={s.url} rel="external">{s.name}</a>
									{:else}<span class="store soon">{s.name} · {c.soon}</span>{/if}
								{/each}
							</div>
						{/if}
					</div>
				{/each}
			</div>
			<p class="caption">{c.pricing.note}</p>
		</section>

		<section id="faq" class="section wrap">
			<h2>{c.faq.title}</h2>
			<div class="questions">
				{#each c.faq.items as item (item.q)}
					<details>
						<summary>{item.q}</summary>
						<p>{item.a}</p>
					</details>
				{/each}
			</div>
		</section>

		<section class="cta wrap">
			<span class="cta-mark"><Mark /></span>
			<h2>{c.cta.title}</h2>
			<p>{c.cta.text}</p>
			<div class="actions center">
				<a class="button primary" href={path(lang, 'send')}>{c.hero.send}</a>
				<a class="button tonal" href={path(lang, 'recv')}>{c.hero.recv}</a>
			</div>
		</section>
	</main>

	<footer class="footer">
		<div class="wrap footer-inner">
			<div class="brand"><Mark /><span>Lumen Code</span></div>
			<p>{t('privacy')}</p>
			<p>
				{c.license}
				<a href={REPO} rel="external">GitHub</a> ·
				<a href={`${REPO}/releases`} rel="external">{t('releasesLink')}</a> ·
				<a href={path(zh ? 'en' : 'zh', '')} hreflang={zh ? 'en' : 'zh-CN'}
					>{zh ? 'English' : '中文'}</a
				>
			</p>
			<p class="legal-links">
				<a href={path(lang, 'privacy')}>{LEGAL[lang].privacy.title}</a> ·
				<a href={path(lang, 'terms')}>{LEGAL[lang].terms.title}</a>
			</p>
			<p><BuildLink label={t('buildLabel')} /></p>
			<p class="quiet">© 2026 {COMPANY}</p>
		</div>
	</footer>
</div>

<style>
	.site {
		--width: 1120px;
		--gutter: 20px;
		--surface: #0f131b;
		--accent: #7fe3ff;
		font-size: 17px;
		line-height: 1.6;
		min-height: 100%;
		background:
			radial-gradient(60rem 30rem at 85% -5%, rgba(34, 216, 255, 0.08), transparent 70%),
			radial-gradient(50rem 30rem at -10% 10%, rgba(255, 61, 154, 0.06), transparent 70%), var(--bg);
	}
	@media (max-width: 640px) {
		.site {
			--gutter: 16px;
			font-size: 16px;
		}
	}
	:global(html) {
		scroll-padding-top: 80px;
	}
	@media (prefers-reduced-motion: no-preference) {
		:global(html) {
			scroll-behavior: smooth;
		}
	}
	.wrap {
		width: min(var(--width), 100% - 2 * var(--gutter));
		margin-inline: auto;
	}
	h1,
	h2,
	h3,
	p {
		margin: 0;
	}
	h2,
	h3 {
		line-height: 1.2;
		letter-spacing: -0.01em;
	}
	a {
		color: var(--accent);
	}
	.mono {
		font-family: var(--mono);
	}
	.skip {
		position: absolute;
		left: -999px;
		top: 8px;
		z-index: 10;
		padding: 8px 12px;
		border-radius: 8px;
		background: var(--surface);
	}
	.skip:focus {
		left: 8px;
	}

	/* header */
	.bar {
		position: sticky;
		top: 0;
		z-index: 5;
		background: rgba(7, 9, 13, 0.82);
		backdrop-filter: saturate(1.4) blur(12px);
		-webkit-backdrop-filter: saturate(1.4) blur(12px);
		border-bottom: 1px solid var(--line);
		padding-top: env(safe-area-inset-top);
	}
	.nav {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 64px;
		gap: 16px;
	}
	.brand {
		color: var(--text);
		font-size: 18px;
	}
	.links {
		display: flex;
		align-items: center;
		gap: 22px;
		font-size: 15px;
	}
	.links a {
		color: var(--muted);
		text-decoration: none;
	}
	.links a:hover {
		color: var(--text);
	}
	.gh {
		display: grid;
		place-items: center;
	}
	.gh svg {
		width: 19px;
		height: 19px;
	}
	.pill {
		border: 1px solid var(--line-2);
		background: var(--surface);
		color: var(--text);
		border-radius: 999px;
		padding: 5px 14px;
		font-size: 15px;
		cursor: pointer;
	}
	@media (max-width: 760px) {
		.links a:not(.gh) {
			display: none;
		}
	}

	/* hero */
	.hero {
		display: grid;
		grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr);
		gap: 56px;
		align-items: center;
		padding-block: 72px 48px;
	}
	.eyebrow {
		color: var(--accent);
		font-weight: 600;
		font-size: 14px;
		letter-spacing: 0.02em;
		margin-bottom: 16px;
	}
	h1 {
		font-size: clamp(40px, 6.4vw, 72px);
		line-height: 1.04;
		letter-spacing: -0.03em;
		font-weight: 750;
	}
	h1 em {
		font-style: normal;
		background: linear-gradient(90deg, var(--cyan), var(--pink) 55%, var(--amber));
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent;
	}
	.lead {
		margin-top: 24px;
		color: var(--muted);
		font-size: 1.12em;
		max-width: 36em;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		margin-top: 32px;
	}
	.actions.center {
		justify-content: center;
	}
	.note {
		margin-top: 16px;
		color: var(--faint);
		font-size: 14px;
	}
	.hero-text :global(.warn) {
		max-width: 36em;
		margin: 20px 0 0;
	}
	.button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 48px;
		padding: 0 24px;
		border-radius: 12px;
		font-weight: 600;
		text-decoration: none;
		transition:
			background 0.15s,
			transform 0.1s;
	}
	.button:active {
		transform: scale(0.98);
	}
	.button.primary {
		background: #f2f4f8;
		color: #0a0d14;
		box-shadow: 0 8px 28px rgba(34, 216, 255, 0.18);
	}
	.button.primary:hover {
		background: #fff;
	}
	.button.tonal {
		background: rgba(255, 255, 255, 0.06);
		border: 1px solid var(--line-2);
		color: var(--text);
	}
	.button.tonal:hover {
		background: rgba(255, 255, 255, 0.1);
	}
	.hero-visual {
		position: relative;
		padding: 0 84px 96px 0;
	}
	.screen {
		margin: 0;
		padding: 14px 14px 0;
		border-radius: 18px;
		background: #05070a;
		border: 1px solid var(--line-2);
		box-shadow:
			0 30px 80px rgba(0, 0, 0, 0.55),
			0 0 120px rgba(34, 216, 255, 0.07);
	}
	.screen canvas {
		display: block;
		width: 100%;
		height: auto;
		image-rendering: pixelated;
		border-radius: 6px;
	}
	.screen figcaption {
		padding: 10px 2px 12px;
		color: var(--faint);
		font-size: 12.5px;
		line-height: 1.5;
		max-width: 62%;
	}
	.phone {
		position: absolute;
		right: 0;
		bottom: 0;
		width: 34%;
		height: auto;
		border-radius: 26px;
		border: 5px solid #1b2130;
		box-shadow: 0 24px 60px rgba(0, 0, 0, 0.6);
		background: #000;
	}
	@media (max-width: 900px) {
		.hero {
			grid-template-columns: minmax(0, 1fr);
			gap: 40px;
			padding-block: 40px 32px;
		}
		.hero-visual {
			max-width: 560px;
		}
	}
	@media (max-width: 480px) {
		.hero-visual {
			padding: 0 36px 90px 0;
		}
		.phone {
			width: 42%;
			border-width: 4px;
			border-radius: 20px;
		}
		.screen figcaption {
			max-width: 52%;
		}
	}

	/* sections */
	.section {
		padding-block: 72px;
	}
	.section h2,
	.cta h2 {
		font-size: clamp(28px, 4vw, 42px);
		letter-spacing: -0.02em;
	}
	.section-lead {
		margin-top: 14px;
		color: var(--muted);
		max-width: 46em;
	}
	.caption {
		margin-top: 16px;
		color: var(--faint);
		font-size: 14px;
		max-width: 60em;
	}
	.card {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: var(--radius);
	}
	h3 {
		font-size: 18px;
	}

	/* how it works */
	.flow {
		display: flex;
		align-items: stretch;
		list-style: none;
		padding: 0;
		margin: 40px 0 0;
	}
	.flow li {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 2px;
		padding: 14px 16px;
		border: 1px solid var(--line-2);
		border-radius: 14px;
		background: var(--surface);
		position: relative;
		min-width: 0;
	}
	.flow li + li {
		margin-left: 34px;
	}
	.flow li + li::before {
		content: '';
		position: absolute;
		left: -28px;
		top: 50%;
		width: 22px;
		height: 2px;
		background: linear-gradient(90deg, var(--cyan), var(--pink));
	}
	.flow li + li::after {
		content: '';
		position: absolute;
		left: -10px;
		top: calc(50% - 4px);
		border: 5px solid transparent;
		border-left-color: var(--pink);
	}
	.flow li.mid {
		background: linear-gradient(135deg, rgba(34, 216, 255, 0.1), rgba(255, 61, 154, 0.1));
	}
	.flow span {
		font-weight: 600;
		font-size: 15px;
		overflow-wrap: anywhere;
	}
	.flow small {
		color: var(--faint);
		font-size: 13px;
	}
	@media (max-width: 860px) {
		.flow {
			flex-direction: column;
		}
		.flow li + li {
			margin: 26px 0 0;
		}
		.flow li + li::before {
			left: 24px;
			top: -22px;
			width: 2px;
			height: 16px;
			background: linear-gradient(var(--cyan), var(--pink));
		}
		.flow li + li::after {
			left: 20px;
			top: -8px;
			border-left-color: transparent;
			border-top-color: var(--pink);
		}
	}
	.steps {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 28px;
		list-style: none;
		padding: 0;
		margin: 48px 0 0;
	}
	.steps p {
		margin-top: 8px;
		color: var(--muted);
		font-size: 15.5px;
	}
	.step-number {
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		margin-bottom: 14px;
		border-radius: 50%;
		background: rgba(34, 216, 255, 0.12);
		color: var(--cyan);
		font-weight: 700;
		font-size: 15px;
	}
	@media (max-width: 900px) {
		.steps {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	@media (max-width: 520px) {
		.steps {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	/* speed */
	.table-card {
		margin-top: 36px;
		overflow-x: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 15.5px;
	}
	th,
	td {
		text-align: left;
		padding: 14px 20px;
		border-bottom: 1px solid var(--line);
		vertical-align: top;
	}
	thead th {
		color: var(--faint);
		font-weight: 600;
		font-size: 14px;
	}
	tbody tr:last-child > * {
		border-bottom: 0;
	}
	tbody th {
		font-weight: 500;
		color: var(--text);
	}
	td {
		color: var(--muted);
	}
	.ours {
		color: var(--text);
		background: rgba(34, 216, 255, 0.05);
	}
	thead .ours {
		color: var(--cyan);
	}
	@media (max-width: 560px) {
		th,
		td {
			padding: 12px 12px;
			font-size: 14.5px;
		}
	}

	/* cards */
	.grid {
		display: grid;
		gap: 16px;
		list-style: none;
		padding: 0;
		margin: 36px 0 0;
	}
	.grid.three {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}
	.grid.four {
		grid-template-columns: repeat(4, minmax(0, 1fr));
	}
	.grid li {
		padding: 24px;
	}
	.grid p {
		margin-top: 8px;
		color: var(--muted);
		font-size: 15.5px;
	}
	.grid p.req {
		margin-top: 4px;
		color: var(--accent);
		font-size: 14px;
	}
	.icon {
		width: 28px;
		height: 28px;
		margin-bottom: 14px;
		fill: none;
		stroke: var(--cyan);
		stroke-width: 1.7;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	@media (max-width: 960px) {
		.grid.three,
		.grid.four {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	@media (max-width: 560px) {
		.grid.three,
		.grid.four {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	/* open protocol */
	.open {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 28px 40px;
		list-style: none;
		padding: 0;
		margin: 36px 0 0;
	}
	.open li {
		padding-left: 20px;
		border-left: 3px solid;
		border-image: linear-gradient(var(--cyan), var(--pink)) 1;
	}
	.open p {
		margin-top: 8px;
		color: var(--muted);
		font-size: 15.5px;
	}
	@media (max-width: 700px) {
		.open {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	/* pricing */
	.plans {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 16px;
		margin-top: 36px;
	}
	.plan {
		display: flex;
		flex-direction: column;
		padding: 28px;
	}
	.plan.featured {
		border-color: rgba(34, 216, 255, 0.55);
		background:
			linear-gradient(160deg, rgba(34, 216, 255, 0.1), rgba(255, 61, 154, 0.06) 60%, transparent),
			var(--surface);
		box-shadow:
			0 0 0 1px rgba(34, 216, 255, 0.15),
			0 20px 60px rgba(34, 216, 255, 0.08);
	}
	.price {
		margin-top: 18px;
		font-size: 38px;
		font-weight: 700;
		letter-spacing: -0.02em;
	}
	.price small {
		font-size: 15px;
		font-weight: 400;
		color: var(--faint);
		letter-spacing: 0;
	}
	.plan ul {
		margin: 16px 0 24px;
		padding-left: 20px;
		color: var(--muted);
		font-size: 15.5px;
		flex: 1;
	}
	.plan li + li {
		margin-top: 6px;
	}
	.plan .button {
		align-self: flex-start;
	}
	.stores {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.store {
		display: inline-flex;
		align-items: center;
		min-height: 40px;
		padding: 0 14px;
		border-radius: 10px;
		font-size: 14px;
		text-decoration: none;
		color: var(--text);
		border: 1px solid var(--line-2);
	}
	.store.soon {
		color: var(--faint);
		border-style: dashed;
	}
	@media (max-width: 900px) {
		.plans {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	/* faq */
	.questions {
		margin-top: 32px;
		border-top: 1px solid var(--line);
	}
	details {
		border-bottom: 1px solid var(--line);
	}
	summary {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		padding: 20px 0;
		font-weight: 600;
		cursor: pointer;
		list-style: none;
	}
	summary::-webkit-details-marker {
		display: none;
	}
	summary::after {
		content: '+';
		color: var(--cyan);
		font-size: 22px;
		line-height: 1;
		transition: transform 0.2s;
	}
	details[open] summary::after {
		transform: rotate(45deg);
	}
	details p {
		padding-bottom: 20px;
		color: var(--muted);
		max-width: 48em;
	}

	/* closing */
	.cta {
		margin-block: 40px 80px;
		padding: 64px 24px;
		text-align: center;
		border-radius: 24px;
		border: 1px solid var(--line);
		background:
			radial-gradient(40rem 16rem at 50% 0%, rgba(34, 216, 255, 0.12), transparent 70%),
			radial-gradient(30rem 14rem at 50% 100%, rgba(255, 61, 154, 0.1), transparent 70%),
			var(--surface);
	}
	.cta-mark {
		display: inline-block;
		margin-bottom: 24px;
		transform: scale(1.8);
	}
	.cta p {
		margin-top: 12px;
		color: var(--muted);
	}

	/* footer */
	.footer {
		border-top: 1px solid var(--line);
		padding-block: 40px calc(48px + env(safe-area-inset-bottom));
		color: var(--muted);
		font-size: 14px;
	}
	.footer-inner {
		display: grid;
		gap: 10px;
	}
	.footer .brand {
		color: var(--text);
		font-size: 16px;
		margin-bottom: 4px;
	}
	.footer :global(.build) {
		margin: 0;
	}
	.footer a {
		text-decoration: none;
	}
	.footer a:hover {
		text-decoration: underline;
	}
	.quiet {
		color: var(--faint);
	}
</style>
