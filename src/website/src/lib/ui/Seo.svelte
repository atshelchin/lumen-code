<script lang="ts">
	// <head> of a page: title, description, the language versions, and link previews.
	// href of an alternate is the public address (for search engines); data-local is the path on
	// this host, used by the redirect in app.html and by the language button.
	import { LANGS, htmlLang, path, type Lang, type Page } from '#lib/i18n';

	let {
		lang,
		page,
		title,
		description,
		jsonLd = null
	}: {
		lang: Lang;
		page: Page;
		title: string;
		description: string;
		jsonLd?: object | null;
	} = $props();

	const abs = (l: Lang) => `${__SITE__}${path(l, page)}`;
	const ld = $derived(
		jsonLd
			? `<script type="application/ld+json">${JSON.stringify(jsonLd).replace(/</g, '\\u003c')}</` +
					'script>'
			: ''
	);
</script>

<svelte:head>
	<title>{title}</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={abs(lang)} />
	{#each LANGS as l (l)}
		<link rel="alternate" hreflang={htmlLang(l)} href={abs(l)} data-local={path(l, page)} />
	{/each}
	<link rel="alternate" hreflang="x-default" href={abs('en')} data-local={path('en', page)} />
	<meta property="og:type" content="website" />
	<meta property="og:site_name" content="Lumen Code" />
	<meta property="og:title" content={title} />
	<meta property="og:description" content={description} />
	<meta property="og:url" content={abs(lang)} />
	<meta property="og:locale" content={lang === 'zh' ? 'zh_CN' : 'en_US'} />
	<meta property="og:image" content={`${__SITE__}/img/social-${lang}.png`} />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta name="twitter:card" content="summary_large_image" />
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html ld}
</svelte:head>
