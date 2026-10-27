<script lang="ts">
	import { page } from '$app/state';
</script>

<svelte:head>
	<title>{page.status} — Lumen Code</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<main class="landing">
	<section class="hero">
		<div class="hero-text">
			<h1>{page.status}</h1>
			<p>
				{page.status === 404 ? 'This page does not exist. · 页面不存在。' : page.error?.message}
			</p>
			<p><a href="/">Lumen Code</a> · <a href="/zh">中文</a></p>
		</div>
	</section>
</main>
