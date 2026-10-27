<script lang="ts">
	// The privacy policy and the terms of service: one readable column, both languages.
	import { goto } from '$app/navigation';
	import { path, t, type Lang } from '#lib/i18n';
	import { COMPANY, LEGAL, UPDATED, inline } from '#lib/legal';
	import Mark from './Mark.svelte';
	import Seo from './Seo.svelte';
	import BuildLink from './BuildLink.svelte';

	let { lang, page }: { lang: Lang; page: 'privacy' | 'terms' } = $props();
	const L = $derived(LEGAL[lang]);
	const doc = $derived(L[page]);
	const other = $derived(lang === 'zh' ? 'en' : 'zh');

	function switchLang() {
		try {
			localStorage.setItem('lumen.lang', other);
		} catch {
			/* ignore */
		}
		goto(path(other, page));
	}
</script>

<Seo {lang} {page} title={`${doc.title} — Lumen Code`} description={doc.description} />

<div class="legal">
	<header class="bar">
		<nav class="wrap nav" aria-label="Lumen Code">
			<a class="brand" href={path(lang, '')}><Mark /><span>Lumen Code</span></a>
			<button class="pill" id="lang" onclick={switchLang} lang={other === 'zh' ? 'zh-CN' : 'en'}
				>{other === 'zh' ? '中文' : 'English'}</button
			>
		</nav>
	</header>

	<main class="wrap doc">
		<h1>{doc.title}</h1>
		<p class="updated">{L.updated}<time datetime={UPDATED}>{UPDATED}</time></p>

		{#if doc.summary}
			<section class="summary">
				<h2>{L.summaryTitle}</h2>
				<ul>
					{#each doc.summary as item (item)}<li>{item}</li>{/each}
				</ul>
			</section>
		{/if}

		{#each doc.sections as s, i (s.h)}
			<section>
				<h2>{i + 1}. {s.h}</h2>
				{#each s.p ?? [] as para (para)}
					<p>
						{#each inline(para) as part, j (j)}{#if part.href}<a
									href={part.href}
									rel={part.href.startsWith('http') ? 'external' : undefined}>{part.text}</a
								>{:else if part.bold}<strong>{part.text}</strong>{:else}{part.text}{/if}{/each}
					</p>
				{/each}
				{#if s.list}
					<ul>
						{#each s.list as item (item)}<li>{item}</li>{/each}
					</ul>
				{/if}
			</section>
		{/each}
	</main>

	<footer class="wrap foot">
		<a href={path(lang, '')}>{L.home}</a> ·
		<a href={path(lang, 'privacy')} aria-current={page === 'privacy' ? 'page' : undefined}
			>{L.privacy.title}</a
		>
		·
		<a href={path(lang, 'terms')} aria-current={page === 'terms' ? 'page' : undefined}
			>{L.terms.title}</a
		>
		<span class="quiet"><BuildLink label={t(lang, 'buildLabel')} /> · © 2026 {COMPANY}</span>
	</footer>
</div>

<style>
	.legal {
		--width: 760px;
		--gutter: 20px;
		min-height: 100%;
		font-size: 16.5px;
		line-height: 1.7;
	}
	.wrap {
		width: min(var(--width), 100% - 2 * var(--gutter));
		margin-inline: auto;
	}
	.bar {
		position: sticky;
		top: 0;
		z-index: 5;
		background: rgba(7, 9, 13, 0.85);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border-bottom: 1px solid var(--line);
		padding-top: env(safe-area-inset-top);
	}
	.nav {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 60px;
	}
	.brand {
		color: var(--text);
		font-size: 17px;
	}
	.pill {
		border: 1px solid var(--line-2);
		background: transparent;
		color: var(--text);
		border-radius: 999px;
		padding: 4px 14px;
		font-size: 14px;
		cursor: pointer;
	}
	.doc {
		padding-block: 56px 32px;
	}
	h1 {
		margin: 0;
		font-size: clamp(30px, 5vw, 42px);
		letter-spacing: -0.02em;
		line-height: 1.15;
	}
	.updated {
		margin: 10px 0 0;
		color: var(--faint);
		font-size: 14px;
	}
	section {
		margin-top: 36px;
	}
	h2 {
		margin: 0 0 10px;
		font-size: 20px;
		letter-spacing: -0.01em;
	}
	p {
		margin: 0 0 12px;
		color: #c3cad6;
	}
	ul {
		margin: 0 0 12px;
		padding-left: 22px;
		color: #c3cad6;
	}
	li + li {
		margin-top: 6px;
	}
	strong {
		color: var(--text);
	}
	a {
		color: var(--cyan);
	}
	.summary {
		padding: 20px 24px 10px;
		border: 1px solid var(--line-2);
		border-radius: 16px;
		background: rgba(34, 216, 255, 0.05);
	}
	.foot {
		display: flex;
		flex-wrap: wrap;
		gap: 6px 8px;
		padding-block: 28px calc(40px + env(safe-area-inset-bottom));
		border-top: 1px solid var(--line);
		color: var(--muted);
		font-size: 14px;
	}
	.foot a {
		color: var(--muted);
		text-decoration: none;
	}
	.foot a:hover,
	.foot a[aria-current='page'] {
		color: var(--text);
	}
	.quiet {
		margin-left: auto;
		color: var(--faint);
	}
	.quiet :global(.build) {
		margin: 0;
		display: inline;
	}
</style>
