<script lang="ts">
	import LegalPage from '#lib/ui/LegalPage.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<LegalPage lang={data.lang} page="privacy" />
