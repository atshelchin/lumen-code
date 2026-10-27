<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { t as tr, path, type Key } from '#lib/i18n';
	import { BUILD } from '#lib/build';
	import { formatJson, linkParts } from '#lib/text-view';
	import {
		ReceiveController,
		TEXT_DISPLAY_LIMIT,
		HIGHLIGHT_LIMIT,
		LINKIFY_LIMIT
	} from '#lib/recv/receiver.svelte';
	import HomeLink from '#lib/ui/HomeLink.svelte';
	import BuildLink from '#lib/ui/BuildLink.svelte';
	import Seo from '#lib/ui/Seo.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const lang = $derived(data.lang);
	const t = (key: Key) => tr(lang, key);

	let rx = $state<ReceiveController | null>(null);
	let video = $state<HTMLVideoElement>();
	let overlay = $state<HTMLCanvasElement>();
	let mediaInput = $state<HTMLInputElement>();
	let stage = $state<HTMLDivElement>();
	let preview = $state<HTMLDivElement>();
	let wide = $state(true);
	let hudH = $state(0); // the bottom panel's height: the framing guides end above it
	// the engine, cell size, success rate and diagnostics: folded away unless asked for
	let details = $state(false);
	const DETAILS_KEY = 'lumen.recv.details';
	// "29 KB/s · 12s left", once data arrives
	const rateLine = $derived(
		!rx || rx.mSpeed === '–'
			? ''
			: rx.mEta === '–'
				? rx.mSpeed
				: `${rx.mSpeed} · ${t('timeLeft').replace('{t}', rx.mEta)}`
	);
	function toggleDetails() {
		details = !details;
		try {
			localStorage.setItem(DETAILS_KEY, details ? '1' : '0');
		} catch {
			/* storage unavailable */
		}
	}

	onMount(() => {
		const c = new ReceiveController(lang);
		c.attach(video!, overlay!);
		rx = c;
		console.info('Lumen Code', BUILD);
		wide = innerWidth >= 720;
		try {
			details = localStorage.getItem(DETAILS_KEY) === '1';
		} catch {
			/* storage unavailable */
		}
		const onVisible = () => c.visible();
		const onResize = () => {
			wide = innerWidth >= 720;
			if (c.done) moreHint();
		};
		document.addEventListener('visibilitychange', onVisible);
		window.addEventListener('resize', onResize);
		const off = gestures(stage!, c);
		return () => {
			off();
			document.removeEventListener('visibilitychange', onVisible);
			window.removeEventListener('resize', onResize);
			c.destroy();
		};
	});

	// zoom: pinch on the camera image; ctrl + wheel (trackpad pinch) on desktop
	function gestures(stage: HTMLElement, c: ReceiveController) {
		const onImage = (e: Event) =>
			!(e.target as Element).closest('.topbar, .recv-dock, .start, .done');
		let pinch: { d: number; z: number } | null = null;
		const spread = (t: TouchList) =>
			Math.hypot(t[0].clientX - t[1].clientX, t[0].clientY - t[1].clientY);
		const start = (e: TouchEvent) => {
			if (e.touches.length === 2 && c.zoom && onImage(e))
				pinch = { d: spread(e.touches), z: c.zoomLevel };
		};
		const move = (e: TouchEvent) => {
			if (!pinch || e.touches.length !== 2) return;
			e.preventDefault();
			c.setZoom(pinch.z * (spread(e.touches) / pinch.d));
		};
		const end = (e: TouchEvent) => {
			if (e.touches.length < 2) pinch = null;
		};
		// iOS Safari: keep the page itself from zooming
		const gesture = (e: Event) => onImage(e) && e.preventDefault();
		const wheel = (e: WheelEvent) => {
			if (!c.zoom || !onImage(e)) return;
			e.preventDefault();
			c.setZoom(c.zoomLevel * Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.002)));
		};
		stage.addEventListener('touchstart', start, { passive: true });
		stage.addEventListener('touchmove', move, { passive: false });
		stage.addEventListener('touchend', end);
		stage.addEventListener('gesturestart', gesture);
		stage.addEventListener('gesturechange', gesture);
		stage.addEventListener('wheel', wheel, { passive: false });
		return () => {
			stage.removeEventListener('touchstart', start);
			stage.removeEventListener('touchmove', move);
			stage.removeEventListener('touchend', end);
			stage.removeEventListener('gesturestart', gesture);
			stage.removeEventListener('gesturechange', gesture);
			stage.removeEventListener('wheel', wheel);
		};
	}

	// ---------- the result
	const done = $derived(rx?.done ?? null);
	const reader = $derived(rx?.reader ?? {});
	const plain = $derived(!!done && done.message && !done.json); // a text message: linked, always wrapped
	const pretty = $derived(!!done?.json && reader.view !== 'raw');
	const colour = $derived(!!done?.text && done.text.length <= HIGHLIGHT_LIMIT);
	const shown = $derived.by(() => {
		if (!done || done.text === null) return '';
		const all = pretty ? (done.lines ?? []).join('\n') : done.text;
		return all.length > TEXT_DISPLAY_LIMIT ? all.slice(0, TEXT_DISPLAY_LIMIT) : all;
	});
	// indentation becomes padding, so a wrapped line continues under its own start (hanging indent)
	const jsonHtml = $derived(
		done?.lines && pretty && colour
			? done.lines
					.map((l) => {
						const sp = l.search(/\S|$/);
						return `<span class="l" style="--d:${sp / 2}">${l.slice(sp)}</span>`;
					})
					.join('')
			: ''
	);
	const truncated = $derived(
		!!done &&
			done.text !== null &&
			!(pretty && colour) &&
			(pretty ? (done.lines ?? []).join('\n') : done.text).length > TEXT_DISPLAY_LIMIT
	);
	const showStats = $derived(reader.stats ?? wide); // open by default on wide screens, folded on phones
	const isLink = $derived(!!done?.text && /^https?:\/\/\S+$/i.test(done.text.trim()));
	let copied = $state(false);
	let overflow = $state(false);
	let atEnd = $state(true);

	function setReader(change: Partial<typeof reader>) {
		if (!rx) return;
		rx.reader = { ...rx.reader, ...change };
		rx.saveReader();
	}

	// Long previews scroll; phones show no scrollbar, so fade the edge (and say so in the small card).
	function moreHint() {
		const pv = preview;
		if (!pv) return void (overflow = false);
		overflow = pv.scrollHeight > pv.clientHeight + 4;
		atEnd = pv.scrollTop + pv.clientHeight >= pv.scrollHeight - 8;
	}
	const hint = $derived(
		[
			overflow && done && !(done.image || done.text !== null) ? t('scrollHint') : '',
			truncated ? t('truncatedHint').replace('{n}', TEXT_DISPLAY_LIMIT.toLocaleString()) : ''
		]
			.filter(Boolean)
			.join(' · ')
	);
	$effect(() => {
		// a new result or another view: back to the top
		void [done, pretty, reader.wrap];
		tick().then(() => {
			if (preview) preview.scrollTop = preview.scrollLeft = 0;
			requestAnimationFrame(moreHint);
		});
	});

	async function copy() {
		if (!done?.text) return;
		// copies what is on screen: formatted JSON in the Formatted view; Save keeps the original bytes
		const out = pretty ? formatJson(done.text).join('\n') : done.text;
		if (await copyText(out)) {
			copied = true;
			setTimeout(() => (copied = false), 1500);
		}
	}
	async function copyText(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			return true;
		} catch {
			// fallback for browsers without async clipboard access
			const ta = document.createElement('textarea');
			ta.value = text;
			ta.style.cssText = 'position:fixed;opacity:0;top:0;left:0';
			document.body.appendChild(ta);
			ta.select();
			let ok = false;
			try {
				ok = document.execCommand('copy');
			} catch {
				/* ignore */
			}
			ta.remove();
			return ok;
		}
	}

	function onMedia() {
		const f = mediaInput?.files?.[0];
		if (f && rx) rx.openMedia(f);
	}
</script>

<Seo {lang} page="recv" title={t('titleRecv')} description={t('descRecv')} />

<div class="stage recv" id="stage" bind:this={stage} style:--hud-h="{hudH}px">
	<video id="video" playsinline muted autoplay bind:this={video}></video>
	<canvas id="overlay" bind:this={overlay}></canvas>

	<!-- where to put the code: shown until a code is found, then the detected outline takes over -->
	<div
		class="guides"
		class:off={!rx || rx.phase !== 'scanning' || rx.found}
		id="guides"
		aria-hidden="true"
	>
		<i></i><i></i><i></i><i></i>
	</div>

	<div class="topbar">
		<HomeLink href={path(lang, '')} title={t('home')} />
		<span class="chip" class:hidden={!rx || rx.phase === 'start'} id="statusChip"
			><span class="dot" class:live={rx?.found} id="statusDot"></span><b id="statusText"
				>{rx?.found ? t('receiving') : t('waiting')}</b
			></span
		>
		<span class="spacer"></span>
	</div>

	<div
		class="recv-dock"
		class:hidden={rx?.phase === 'start' || !rx}
		bind:clientHeight={hudH}
		id="recvDock"
	>
		<!-- camera tools sit above the panel, at the edge of the image, never over its middle -->
		<div class="cam-tools" class:hidden={!rx?.zoom && !rx?.multiCam}>
			<button
				class="zoom-chip"
				class:hidden={!rx?.zoom}
				class:active={!!rx?.zoom && rx.zoomLevel > rx.zoom.min + 0.05}
				id="zoomChip"
				title={t('zoomHint')}
				onclick={() => rx?.zoom && rx.setZoom(rx.zoom.min)}
				><b class="num" id="zoomVal">{(rx?.zoomLevel || 1).toFixed(1)}×</b></button
			>
			<button
				class="btn icon"
				class:hidden={!rx?.multiCam}
				id="switchCam"
				title={t('openCam')}
				aria-label={t('openCam')}
				onclick={() => rx?.switchCamera()}
			>
				<svg
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="1.8"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
					><path
						d="M4 8.5A2.5 2.5 0 0 1 6.5 6H8l1.5-2h5L16 6h1.5A2.5 2.5 0 0 1 20 8.5v8a2.5 2.5 0 0 1-2.5 2.5h-11A2.5 2.5 0 0 1 4 16.5Z"
					/><path d="M9 12.5a3 3 0 0 1 5.2-2M15 12a3 3 0 0 1-5.2 2" /><path
						d="M14.5 8.8 14.2 10.6 16 10.4M9.5 15.2l.3-1.8-1.8.2"
					/></svg
				>
			</button>
		</div>

		<div class="recv-bottom panel" id="hud">
			<div class="recv-title">
				<span class="name" id="rxName">{rx?.rxName ?? t('noData')}</span>
				<span class="pct" id="rxPct">{Math.floor(rx?.pct ?? 0)}%</span>
			</div>
			<div class="progress"><div id="rxBar" style:width="{rx?.pct ?? 0}%"></div></div>
			<div class="rate-row">
				<span class="rate" id="mSpeed">{rateLine}</span>
				<button
					class="more-btn"
					id="detailsBtn"
					aria-expanded={details}
					aria-controls="recvDetails"
					onclick={toggleDetails}
					><span>{t('details')}</span><svg
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
						aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg
					></button
				>
			</div>
			<div class="tip" class:ok={rx?.tipGood} id="tip">{rx?.tip ?? ''}</div>
			{#if details}
				<div class="recv-details" id="recvDetails">
					<div class="row">
						<span>{t('engine')}</span>
						<div class="seg" id="segEngine">
							{#each ['js', 'wasm'] as const as e (e)}
								<button
									data-v={e}
									aria-pressed={(rx?.engine ?? 'wasm') === e}
									onclick={() => rx?.setEngine(e)}>{e.toUpperCase()}</button
								>
							{/each}
						</div>
					</div>
					<div class="row">
						<span>{t('cellM')} <b id="mCell">{rx?.mCell ?? '–'}</b></span>
						<span>{t('okM')} <b id="mOk">{rx?.mOk ?? '–'}</b></span>
					</div>
					<div class="diag num" id="diag">{rx?.diag ?? ''}</div>
				</div>
			{/if}
		</div>
	</div>

	<div class="start" class:hidden={rx && rx.phase !== 'start'} id="start">
		<div class="inner">
			<div class="drop-art">
				<svg
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="1.8"
					stroke-linecap="round"
					stroke-linejoin="round"
					><path
						d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2"
					/><circle cx="12" cy="12" r="3.2" /></svg
				>
			</div>
			<h1>{t('recvStartTitle')}</h1>
			<p>{t('recvStartDesc')}</p>
			<div style="display: flex; gap: 10px; justify-content: center; flex-wrap: wrap">
				<button class="btn primary" id="startBtn" onclick={() => rx?.openCamera()}
					>{t('openCam')}</button
				>
				<button class="btn" id="fileBtn" onclick={() => mediaInput?.click()}
					>{t('fromImage')}</button
				>
			</div>
			<input
				type="file"
				id="mediaFile"
				accept="image/*,video/*"
				hidden
				bind:this={mediaInput}
				onchange={onMedia}
			/>
			<div class="err" id="startErr">{rx?.startErr ?? ''}</div>
			<a class="switch-page" href={path(lang, 'send')}
				><span>{t('goSend')}</span><svg
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

	{#if done}
		<div
			class="done"
			class:reader={done.image || done.text !== null}
			class:show-stats={showStats}
			id="done"
		>
			<div class="panel">
				<div class="done-head">
					<div class="badge">
						<svg
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2.2"
							stroke-linecap="round"
							stroke-linejoin="round"><path d="m5 12.5 4.5 4.5L19 7.5" /></svg
						>
					</div>
					<div class="done-title">
						<h2 id="doneName">{done.title}</h2>
						<div class="sub" id="doneSub">{done.sub}</div>
					</div>
					{#if done.text !== null}
						<span class="head-tools" id="headTools">
							<button
								class="tbtn"
								id="fontDown"
								title={t('smaller')}
								onclick={() => setReader({ fs: Math.max(-4, (reader.fs || 0) - 1) })}
								><span class="a-sm">A</span></button
							>
							<button
								class="tbtn"
								id="fontUp"
								title={t('larger')}
								onclick={() => setReader({ fs: Math.min(8, (reader.fs || 0) + 1) })}
								><span class="a-lg">A</span></button
							>
						</span>
					{/if}
					<button
						class="stats-btn"
						id="statsBtn"
						aria-expanded={showStats}
						onclick={() => setReader({ stats: !showStats })}
						><span>{t('details')}</span><svg
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"><path d="m6 9 6 6 6-6" /></svg
						></button
					>
				</div>
				<div class="done-stats" id="doneStats">
					{#each done.stats as [k, v] (k)}
						<div>{k}<b>{v}</b></div>
					{/each}
					{#if done.code}<div class="code">{done.code}</div>{/if}
				</div>
				{#if done.text !== null && !plain}
					<div class="reader-bar" id="readerBar">
						{#if done.json}
							<div class="seg" id="segView">
								<button
									data-v="pretty"
									aria-pressed={pretty}
									onclick={() => setReader({ view: 'pretty' })}>{t('vPretty')}</button
								>
								<button
									data-v="raw"
									aria-pressed={!pretty}
									onclick={() => setReader({ view: 'raw' })}>{t('vRaw')}</button
								>
							</div>
						{/if}
						<span class="spacer"></span>
						<button
							class="tbtn"
							id="wrapBtn"
							title={t('wrap')}
							aria-pressed={reader.wrap !== false}
							onclick={() => setReader({ wrap: reader.wrap === false })}
						>
							<svg
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="1.9"
								stroke-linecap="round"
								stroke-linejoin="round"
								><path d="M4 6h16M4 12h13a3 3 0 0 1 0 6h-4" /><path
									d="m15 16-2 2 2 2M4 18h5"
								/></svg
							>
						</button>
					</div>
				{/if}
				{#if done.text !== null}
					<div
						class="preview"
						class:text={plain}
						class:nowrap={!plain && reader.wrap === false}
						class:fade={overflow && !atEnd}
						style:--rfs-d={reader.fs || 0}
						id="preview"
						bind:this={preview}
						onscroll={moreHint}
					>
						{#if pretty && colour}
							<!-- eslint-disable svelte/no-at-html-tags -- formatJson escapes the text -->
							<pre
								class="json"
								style:--gut="{String(done.lines!.length).length + 2}ch">{@html jsonHtml}</pre>
							<!-- eslint-enable svelte/no-at-html-tags -->
						{:else if plain && shown.length <= LINKIFY_LIMIT}
							<pre>{#each linkParts(shown) as p, i (i)}{#if p.href}<a
											href={p.href}
											target="_blank"
											rel="noopener noreferrer">{p.text}</a
										>{:else}{p.text}{/if}{/each}</pre>
						{:else}
							<pre>{shown}</pre>
						{/if}
					</div>
				{:else if done.image}
					<div class="preview image" id="preview" bind:this={preview}>
						<img src={done.url} alt={done.name} />
					</div>
				{/if}
				{#if hint}<div class="more-hint" id="moreHint">{hint}</div>{/if}
				<div class="row">
					{#if done.text !== null}
						<button class="btn primary" id="copyBtn" onclick={copy}
							>{copied ? t('copied') : t('copy')}</button
						>
					{/if}
					{#if isLink}
						<a
							class="btn"
							id="linkBtn"
							target="_blank"
							rel="noopener noreferrer"
							href={done.text!.trim()}>{t('openLink')}</a
						>
					{/if}
					<a
						class="btn"
						class:primary={!done.message}
						id="saveBtn"
						href={done.url}
						download={done.name}>{done.message ? t('saveAsFile') : t('save')}</a
					>
					{#if !done.message}
						<a class="btn" id="openBtn" target="_blank" rel="noopener" href={done.url}
							>{t('open')}</a
						>
					{/if}
					<button class="btn" id="againBtn" onclick={() => rx?.again()}>{t('again')}</button>
				</div>
			</div>
		</div>
	{/if}
</div>
