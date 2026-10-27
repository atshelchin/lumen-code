<script lang="ts">
	import { page } from '$app/state';
</script>

<svelte:head>
	<title>{page.status} — Lumen Code</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<main>
	<h1>{page.status}</h1>
	<p>{page.status === 404 ? 'This page does not exist. · 页面不存在。' : page.error?.message}</p>
	<p><a href="/">Lumen Code</a> · <a href="/zh">中文</a></p>
</main>

<style>
	main {
		display: grid;
		place-content: center;
		gap: 12px;
		min-height: 100%;
		padding: 24px;
		text-align: center;
	}
	h1 {
		margin: 0;
		font-size: 64px;
		letter-spacing: -0.03em;
	}
	p {
		margin: 0;
		color: var(--muted);
	}
	a {
		color: var(--cyan);
	}
</style>
