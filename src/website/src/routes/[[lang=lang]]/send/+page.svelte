<script lang="ts">
	import { onMount } from 'svelte';
	import { t as tr, path, type Key } from '#lib/i18n';
	import { fmtBytes, fmtTime } from '#lib/format';
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
	let codeArea = $state<HTMLDivElement>();
	let isFs = $state(false);
	let fsOk = $state(false); // iPhone Safari has no fullscreen for pages: no button there
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
		fsOk = !!document.fullscreenEnabled;
		const sample = setInterval(() => c.sample(), 1000);
		// the code's area changes with the window, fullscreen and the settings panel
		let resizeTimer = 0;
		const ro = new ResizeObserver(() => {
			clearTimeout(resizeTimer);
			resizeTimer = window.setTimeout(() => c.resized(), 100);
		});
		ro.observe(codeArea!);
		const onFs = () => (isFs = !!document.fullscreenElement);
		const onVisible = () => document.visibilityState === 'visible' && c.active && c.wake();
		document.addEventListener('fullscreenchange', onFs);
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
			clearTimeout(resizeTimer);
			ro.disconnect();
			document.removeEventListener('fullscreenchange', onFs);
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

	// ---------- page-wide input: drop, paste, keys
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
	// Space: play/pause (on the warning card: play). F: fullscreen. Esc: closes the settings, and
	// nothing else: closing the code would end the transfer, and a receiver part way through
	// would lose its progress, a heavy price for a key that people press to "get out" of things
	// (in fullscreen the browser takes Esc to leave fullscreen anyway).
	function onKey(e: KeyboardEvent) {
		if (!tx || !tx.active || (e.target as HTMLElement).tagName === 'TEXTAREA') return;
		if (e.ctrlKey || e.metaKey || e.altKey) return;
		if (e.code === 'Space') {
			tx.togglePlay();
			e.preventDefault();
		} else if (e.key === 'f' || e.key === 'F') fullscreen();
		else if (e.key === 'Escape' && tx.settingsOpen) tx.settingsOpen = false;
	}

	// A long file name loses its middle, never its extension: "holiday-ph…2024.zip".
	const nameParts = $derived.by((): [string, string] => {
		if (!tx) return ['', ''];
		if (tx.isText) return [t('textLabel'), ''];
		const n = tx.label;
		const dot = n.lastIndexOf('.');
		const ext = dot > 0 && n.length - dot <= 8 ? n.slice(dot) : '';
		const stem = n.slice(0, n.length - ext.length);
		const keep = Math.min(stem.length, 4);
		return [stem.slice(0, stem.length - keep), stem.slice(stem.length - keep) + ext];
	});
	const passText = (s: number) =>
		s < 1 ? '< 1 s' : s < 60 ? `≈ ${Math.ceil(s)} s` : `≈ ${fmtTime(s)}`;

	const densities: [Density, Key][] = [
		['auto', 'dAuto'],
		['low', 'dLow'],
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

	<!-- while sending: the code above, the settings (when open) and the bar below; nothing overlaps -->
	<div class="send-view" class:hidden={!tx?.sending}>
		<div class="codewrap" id="codewrap" bind:this={codeArea}>
			<canvas id="code" bind:this={canvas}></canvas>
		</div>
		{#if tx?.sending}
			{#if tx.settingsOpen}
				<div class="settings" id="settings" role="group" aria-label={t('settings')}>
					<div class="fields">
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
						<label class="field">
							<span>{t('speed')}</span>
							<input
								type="range"
								id="fps"
								min="4"
								max="60"
								step="1"
								value={tx.prefs.fps}
								oninput={(e) => tx?.setFps(Number((e.target as HTMLInputElement).value))}
							/><b class="num" id="fpsVal">{tx.prefs.fps}</b>
						</label>
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
					<div class="details" id="stats">
						{#if tx.stats}
							{@const s = tx.stats}
							<b>{t('details')}</b>
							{t('rate')}
							{fmtBytes(s.rate)}/s · {t('perFrame')}
							{fmtBytes(s.perFrame)} · {t('grid')}
							{s.X}×{s.Y} · {s.bpc} bit · {tx.prefs.engine.toUpperCase()}
							<span id="buildMs">{tx.buildMs ? `${tx.buildMs.toFixed(2)} ms` : '–'}</span>
						{/if}
					</div>
				</div>
			{/if}
			<div class="sendbar" id="bar">
				<button
					class="btn icon play"
					id="playBtn"
					data-state={tx.playing ? 'playing' : 'paused'}
					title={tx.playing ? t('pause') : t('play')}
					aria-label={tx.playing ? t('pause') : t('play')}
					onclick={() => tx?.togglePlay()}
				>
					{#if tx.playing}
						<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"
							><rect x="6" y="5" width="4" height="14" rx="1" /><rect
								x="14"
								y="5"
								width="4"
								height="14"
								rx="1"
							/></svg
						>
					{:else}
						<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"
							><path
								d="M7 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 7 5.5Z"
							/></svg
						>
					{/if}
				</button>
				<div class="what">
					<b id="fileName" title={tx.isText ? undefined : tx.label}
						><span class="head">{nameParts[0]}</span><span class="tail">{nameParts[1]}</span></b
					>
					<span class="sub"
						><span id="fileSize">{fmtBytes(tx.size)}</span>
						{#if !tx.stats}
							· {t('preparing')}
						{:else if !tx.playing}
							· <span class="paused">{t('paused')}</span>
						{:else}
							· <span id="passTime" title={t('passTitle')}>{passText(tx.stats.pass)}</span>
						{/if}</span
					>
				</div>
				<button
					class="btn icon ghost"
					id="setBtn"
					title={t('settings')}
					aria-label={t('settings')}
					aria-expanded={tx.settingsOpen}
					aria-controls="settings"
					onclick={() => tx && (tx.settingsOpen = !tx.settingsOpen)}
				>
					<svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						aria-hidden="true"
						><path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2" /><circle
							cx="10"
							cy="17"
							r="2"
						/></svg
					>
				</button>
				{#if fsOk}
					<button
						class="btn icon ghost"
						id="fsBtn"
						title={isFs ? t('exitFullscreen') : t('fullscreen')}
						aria-label={isFs ? t('exitFullscreen') : t('fullscreen')}
						onclick={fullscreen}
					>
						<svg
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="1.8"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							{#if isFs}
								<path
									d="M9 4v4a1 1 0 0 1-1 1H4M20 9h-4a1 1 0 0 1-1-1V4M15 20v-4a1 1 0 0 1 1-1h4M4 15h4a1 1 0 0 1 1 1v4"
								/>
							{:else}
								<path
									d="M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4"
								/>
							{/if}
						</svg>
					</button>
				{/if}
				<button
					class="btn icon ghost"
					id="closeBtn"
					title={t('closeCode')}
					aria-label={t('closeCode')}
					onclick={close}
				>
					<svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.8"
						stroke-linecap="round"
						aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg
					>
				</button>
			</div>
		{/if}
	</div>

	{#if tx?.flashAsk}
		<!-- the first code waits here, paused, until the warning is read -->
		<div
			class="flash-ask"
			role="alertdialog"
			aria-labelledby="flashTitle"
			aria-describedby="flashText"
		>
			<div class="panel">
				<h2 id="flashTitle">{t('flashTitle')}</h2>
				<p id="flashText">{t('flashWarn')}</p>
				<div class="row">
					<button class="btn" onclick={close}>{t('closeCode')}</button>
					<!-- svelte-ignore a11y_autofocus -->
					<button class="btn primary" id="flashPlay" autofocus onclick={() => tx?.acceptFlash()}
						>{t('flashPlay')}</button
					>
				</div>
			</div>
		</div>
	{/if}
</div>
