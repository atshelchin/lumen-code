<script lang="ts">
	// Photosensitivity warning (PROTOCOL §10.4): shown before any code plays.
	let { text }: { text: string } = $props();
</script>

<p class="warn" role="note">
	<svg
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
		><path
			d="M10.3 4.2 2.6 17.5A2 2 0 0 0 4.3 20.5h15.4a2 2 0 0 0 1.7-3L13.7 4.2a2 2 0 0 0-3.4 0Z"
		/><path d="M12 9.5v4M12 17h.01" /></svg
	><span>{text}</span>
</p>
