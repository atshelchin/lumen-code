<script lang="ts">
	// The version, for reports: the commit of the sources, linked to it on GitHub.
	import { buildText, commitUrl } from '#lib/build';
	let { label }: { label: string } = $props();
</script>

<a class="build" href={commitUrl} data-build>{label} {buildText()}</a>
