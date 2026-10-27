<script lang="ts">
	import Mark from './Mark.svelte';
	let { href, title }: { href: string; title: string } = $props();
</script>

<a class="home" {href} {title} aria-label={title}
	><svg
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2.2"
		stroke-linecap="round"
		stroke-linejoin="round"><path d="m14.5 6-6 6 6 6" /></svg
	><Mark /></a
>
