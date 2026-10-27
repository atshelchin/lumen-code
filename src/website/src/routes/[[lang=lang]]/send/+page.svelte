<script lang="ts">
	import { onMount } from 'svelte';
	import { t as tr, path, type Key } from '#lib/i18n';
	import { fmtBytes } from '#lib/format';
	import { BUILD } from '#lib/build';
	import { SendController, TEXT_NAME, type Density } from '#lib/send/sender.svelte';
	import HomeLink from '#lib/ui/HomeLink.svelte';
	import Warn from '#lib/ui/Warn.svelte';
	import BuildLink from '#lib/ui/BuildLink.svelte';
	import Seo from '#lib/ui/Seo.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const lang = $derived(data.lang);
	const t = (key: Key) => tr(lang, key);

	let tx = $state<SendController | null>(null);
	let canvas = $state<HTMLCanvasElement>();
	let fileInput = $state<HTMLInputElement>();
	let textArea = $state<HTMLTextAreaElement>();
	let text = $state('');
	let dropOver = $state(false);
	// before the controller exists (pre-rendered HTML): the file tab
	const source = $derived(tx ? tx.prefs.source : 'file');
	const textInfo = $derived(
		text
			? `${[...text].length} ${t('chars')} · ${fmtBytes(new TextEncoder().encode(text).length)}`
			: ''
	);

	onMount(() => {
		const c = new SendController();
		c.attach(canvas!);
		tx = c;
		console.info('Lumen Code', BUILD);
		const sample = setInterval(() => c.sample(), 1000);
		let resizeTimer = 0;
		const onResize = () => {
			clearTimeout(resizeTimer);
			resizeTimer = window.setTimeout(() => c.resized(), 150);
		};
		const onVisible = () => document.visibilityState === 'visible' && c.active && c.wake();
		window.addEventListener('resize', onResize);
		document.addEventListener('visibilitychange', onVisible);
		// test hook: ?demo=<bytes> starts with random data
		const demo = new URLSearchParams(location.search).get('demo');
		if (demo) {
			const n = Math.max(1, Number(demo) || 20000);
			const bytes = new Uint8Array(n);
			crypto.getRandomValues(bytes.subarray(0, Math.min(n, 65536)));
			for (let i = 65536; i < n; i++) bytes[i] = bytes[i % 65536] ^ (i & 255);
			c.start(`demo-${n}.bin`, bytes);
		}
		return () => {
			clearInterval(sample);
			window.removeEventListener('resize', onResize);
			document.removeEventListener('visibilitychange', onVisible);
			c.destroy();
		};
	});

	async function startFile(f: File | undefined | null) {
		if (f && tx) tx.start(f.name || 'pasted', new Uint8Array(await f.arrayBuffer()));
	}
	function sendText() {
		if (text && tx) tx.start(TEXT_NAME, new TextEncoder().encode(text), text);
	}
	function setSource(v: 'file' | 'text') {
		tx?.setSource(v);
		if (v === 'text') setTimeout(() => textArea?.focus(), 0);
	}
	function close() {
		tx?.close();
		if (fileInput) fileInput.value = '';
	}
	function fullscreen() {
		if (document.fullscreenElement) document.exitFullscreen();
		else document.documentElement.requestFullscreen?.().catch(() => {});
	}

	// ---------- page-wide input: drop, paste, keys, the mouse at the bottom edge
	function onDragOver(e: DragEvent) {
		e.preventDefault();
		dropOver = true;
	}
	async function onDrop(e: DragEvent) {
		e.preventDefault();
		dropOver = false;
		const f = e.dataTransfer?.files[0];
		if (f) return startFile(f);
		const s = e.dataTransfer?.getData('text/plain');
		if (s && !tx?.sending) {
			setSource('text');
			text = s;
		}
	}
	async function onPaste(e: ClipboardEvent) {
		if (!tx || tx.sending || e.target === textArea) return;
		const f = e.clipboardData?.files[0];
		if (f) return startFile(f);
		const s = e.clipboardData?.getData('text');
		if (s) {
			e.preventDefault();
			setSource('text');
			text = s;
		}
	}
	function onKey(e: KeyboardEvent) {
		if (!tx || !tx.active || (e.target as HTMLElement).tagName === 'TEXTAREA') return;
		if (e.code === 'Space') {
			tx.togglePlay();
			tx.showDock();
			e.preventDefault();
		} else if (e.key === 'f') fullscreen();
		else if (e.key === 'Escape') tx.hideDock();
		else if (e.key === 'h') tx.toggleDock();
		else tx.showDock();
	}
	// The bottom edge shows a hidden dock, but only after the mouse has been above the dock's area:
	// right after the hide button the mouse is still there, and the dock must stay hidden.
	let edgeArmed = false;
	function onPointerMove(e: PointerEvent) {
		if (e.pointerType !== 'mouse' || !tx?.active) return;
		if (!tx.dockAway) edgeArmed = false;
		else if (e.clientY < window.innerHeight - 220) edgeArmed = true;
		else if (e.clientY > window.innerHeight - 90 && edgeArmed) tx.showDock();
	}

	const densities: [Density, Key][] = [
		['low', 'dLow'],
		['auto', 'dAuto'],
		['high', 'dHigh'],
		['max', 'dMax']
	];
	const eccs: [number, Key][] = [
		[0, 'eLow'],
		[1, 'eMid'],
		[2, 'eHigh']
	];
</script>

<Seo {lang} page="send" title={t('titleSend')} description={t('descSend')} />

<svelte:window
	ondragenter={onDragOver}
	ondragover={onDragOver}
	ondragleave={() => (dropOver = false)}
	ondrop={onDrop}
	onpaste={onPaste}
	onkeydown={onKey}
	onpointermove={onPointerMove}
/>

<div class="stage" id="stage">
	{#if !tx?.sending}
		<div class="topbar" id="topbar">
			<HomeLink href={path(lang, '')} title={t('home')} />
			<span class="spacer"></span>
		</div>

		<div class="drop" class:over={dropOver} id="drop">
			<div class="drop-card">
				<div class="seg tabs" id="srcTabs" role="tablist">
					<button
						data-v="file"
						role="tab"
						aria-selected={source === 'file'}
						onclick={() => setSource('file')}>{t('tabFile')}</button
					>
					<button
						data-v="text"
						role="tab"
						aria-selected={source === 'text'}
						onclick={() => setSource('text')}>{t('tabText')}</button
					>
				</div>
				{#if source === 'file'}
					<div id="paneFile">
						<div class="drop-art">
							<svg
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="1.8"
								stroke-linecap="round"
								stroke-linejoin="round"
								><path d="M12 15V4M7.5 8.5 12 4l4.5 4.5" /><path
									d="M4 14v3.5A2.5 2.5 0 0 0 6.5 20h11a2.5 2.5 0 0 0 2.5-2.5V14"
								/></svg
							>
						</div>
						<h1>{t('dropTitle')}</h1>
						<p>{t('dropDesc')}</p>
						<div class="row">
							<button class="btn primary" id="pick" onclick={() => fileInput?.click()}
								>{t('choose')}</button
							>
						</div>
					</div>
				{:else}
					<div id="paneText">
						<textarea
							id="textInput"
							placeholder={t('textPh')}
							spellcheck="false"
							bind:this={textArea}
							bind:value={text}
							onkeydown={(e) => {
								if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
									e.preventDefault();
									sendText();
								}
							}}></textarea>
						<div class="text-meta">
							<span class="num" id="textCount">{textInfo}</span><span>{t('textKbd')}</span>
						</div>
						<div class="row">
							<button class="btn primary" id="textSend" disabled={!text} onclick={sendText}
								>{t('sendText')}</button
							>
						</div>
					</div>
				{/if}
				<div class="hint">{t('dropHint')}</div>
				<Warn text={t('flashWarn')} />
				<input
					type="file"
					id="file"
					hidden
					bind:this={fileInput}
					onchange={() => startFile(fileInput?.files?.[0])}
				/>
				<a class="switch-page" href={path(lang, 'recv')}
					><span>{t('goRecv')}</span><svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"><path d="M5 12h14M13 6l6 6-6 6" /></svg
					></a
				>
				<BuildLink label={t('buildLabel')} />
			</div>
		</div>
	{/if}

	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="codewrap" class:hidden={!tx?.sending} id="codewrap" onclick={() => tx?.toggleDock()}>
		<canvas id="code" bind:this={canvas}></canvas>
	</div>

	{#if tx?.sending}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="dock panel"
			class:away={tx.dockAway}
			id="dock"
			onpointermove={() => tx?.pokeDock()}
			onpointerdown={() => tx?.pokeDock()}
			onpointerenter={() => tx && (tx.dockHovered = true)}
			onpointerleave={() => tx && (tx.dockHovered = false)}
		>
			<div class="row">
				<button
					class="btn icon"
					id="playBtn"
					title={t('paused')}
					aria-label={t('paused')}
					onclick={() => tx?.togglePlay()}
				>
					{#if tx.playing}
						<svg viewBox="0 0 24 24" fill="currentColor"
							><rect x="6" y="5" width="4" height="14" rx="1" /><rect
								x="14"
								y="5"
								width="4"
								height="14"
								rx="1"
							/></svg
						>
					{:else}
						<svg viewBox="0 0 24 24" fill="currentColor"
							><path
								d="M7 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 7 5.5Z"
							/></svg
						>
					{/if}
				</button>
				<div class="fileinfo grow">
					<div class="fname">
						<span class="dot {tx.playing ? 'live' : 'warn'}" id="liveDot"></span><b id="fileName"
							>{tx.isText ? `${t('textLabel')}: ${tx.label}` : tx.label}</b
						><span id="fileSize" class="num">{fmtBytes(tx.size)}</span>
					</div>
					<div class="stats" id="stats">
						{#if tx.stats}
							{@const s = tx.stats}
							<span>{t('rate')} <b class="num">{fmtBytes(s.rate)}/s</b></span>
							<span>{t('perFrame')} <b class="num">{fmtBytes(s.perFrame)}</b></span>
							<span class="wide"
								>{t('loop')}
								<b class="num">{s.pass < 1 ? '<1' : s.pass.toFixed(s.pass < 10 ? 1 : 0)} s</b></span
							>
							<span class="wide">{t('grid')} <b class="num">{s.X}×{s.Y}</b> · {s.bpc} bit</span>
							<span class="wide"
								>{tx.prefs.engine.toUpperCase()}
								<b class="num" id="buildMs">{tx.buildMs ? `${tx.buildMs.toFixed(2)} ms` : '–'}</b
								></span
							>
						{:else}
							{t('preparing')}
						{/if}
					</div>
				</div>
				<button
					class="btn icon"
					id="setBtn"
					title="Settings"
					aria-label="Settings"
					aria-expanded={tx.settingsOpen}
					onclick={() => tx && (tx.settingsOpen = !tx.settingsOpen)}
				>
					<svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						><path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2" /><circle
							cx="10"
							cy="17"
							r="2"
						/></svg
					>
				</button>
				<button
					class="btn icon"
					id="fsBtn"
					title="Fullscreen"
					aria-label="Fullscreen"
					onclick={fullscreen}
				>
					<svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						><path
							d="M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4"
						/></svg
					>
				</button>
				<button
					class="btn icon"
					id="hideBtn"
					title={t('hideDock')}
					aria-label={t('hideDock')}
					onclick={(e) => {
						e.stopPropagation();
						tx?.hideDock();
					}}
				>
					<svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						stroke-linejoin="round"><path d="m6 9.5 6 6 6-6" /></svg
					>
				</button>
				<button class="btn icon" id="closeBtn" title="Close" aria-label="Close" onclick={close}>
					<svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"><path d="M6 6l12 12M18 6 6 18" /></svg
					>
				</button>
			</div>
			{#if tx.settingsOpen}
				<div class="settings" id="settings">
					<div class="field">
						<span>{t('density')}</span>
						<div class="seg" id="segDensity">
							{#each densities as [v, k] (v)}
								<button
									data-v={v}
									aria-pressed={tx.prefs.density === v}
									onclick={() => tx?.set('density', v)}>{t(k)}</button
								>
							{/each}
						</div>
					</div>
					<div class="field">
						<span>{t('colors')}</span>
						<div class="seg" id="segMode">
							{#each [1, 2] as v (v)}
								<button
									data-v={v}
									aria-pressed={tx.prefs.mode === v}
									onclick={() => tx?.set('mode', v)}>{v === 1 ? 4 : 8}</button
								>
							{/each}
						</div>
					</div>
					<div class="field">
						<span>{t('ecc')}</span>
						<div class="seg" id="segEcc">
							{#each eccs as [v, k] (v)}
								<button
									data-v={v}
									aria-pressed={tx.prefs.ecc === v}
									onclick={() => tx?.set('ecc', v)}>{t(k)}</button
								>
							{/each}
						</div>
					</div>
					<div class="field">
						<span>{t('speed')}</span><input
							type="range"
							id="fps"
							min="4"
							max="60"
							step="1"
							value={tx.prefs.fps}
							oninput={(e) => tx?.setFps(Number((e.target as HTMLInputElement).value))}
						/><b class="num" id="fpsVal">{tx.prefs.fps}</b>
					</div>
					<div class="field">
						<span>{t('engine')}</span>
						<div class="seg" id="segEngine">
							<button
								data-v="js"
								aria-pressed={tx.prefs.engine === 'js'}
								onclick={() => tx?.set('engine', 'js')}>JS</button
							><button
								data-v="wasm"
								aria-pressed={tx.prefs.engine === 'wasm'}
								onclick={() => tx?.set('engine', 'wasm')}>Rust · WASM</button
							>
						</div>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
