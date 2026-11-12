<script>
  import { onMount } from 'svelte'
  import { X, LoaderCircle, Download, Repeat, Lock, Unlock } from '@lucide/svelte'
  import { fade, fly } from 'svelte/transition'
  import { url } from '../lib/api.js'
  import { size } from '../lib/format.js'
  import { saveBlob, slug } from '../lib/save.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import { VIDEO_FORMAT_IDS } from '../lib/formats.js'

  // The encoders (Mediabunny + MP3) are a big download, so they load only when the converter opens.
  /** @type {typeof import('../lib/convert.js') | null} */ let conv = null

  /** @type {{ item: any, onClose: () => void }} */
  let { item, onClose } = $props()

  const INPUT = 'w-full well rounded-lg px-3 py-2 text-sm 2xl:text-lg outline-none'
  const LABEL = 'text-[11px] 2xl:text-sm uppercase tracking-wider text-muted'
  const CHIP = 'px-3 py-1.5 rounded-lg text-xs 2xl:text-base font-semibold border transition-colors'
  const UNITS = { KB: 1024, MB: 1024 ** 2 }
  const AUDIO_KBPS = [32, 64, 96, 128, 192, 256, 320]
  const SAMPLE_RATES = [0, 22050, 32000, 44100, 48000]
  const VIDEO_WIDTHS = [0, 1920, 1280, 854, 640, 480]

  /** @type {Blob | null} */ let source = $state(null)
  /** @type {any} */ let info = $state({})
  /** @type {any[]} */ let formats = $state([])
  let format = $state('')
  let loading = $state(true)
  let error = $state('')

  // Options
  let width = $state(0)
  let height = $state(0)
  let lockAspect = $state(true)
  let quality = $state(85)
  let kbps = $state(192)
  let sampleRate = $state(0)
  let channels = $state(0)
  let videoWidth = $state(0)
  let videoKbps = $state(0)
  let useTarget = $state(false)
  let targetValue = $state(1)
  let targetUnit = $state(/** @type {'KB'|'MB'} */ ('MB'))

  // Run state
  let busy = $state(false)
  let progress = $state(0)
  /** @type {Blob | null} */ let result = $state(null)
  /** @type {AbortController | null} */ let abort = null

  const kind = $derived(item.kind)
  const fmt = $derived(formats.find((f) => f.id === format))
  const isVideoOut = $derived(VIDEO_FORMAT_IDS.includes(format) && kind === 'video')
  const lossless = $derived(!!fmt?.lossless || (kind === 'image' && !fmt?.lossy))
  const baseName = $derived(slug(item.job.prompt))

  onMount(async () => {
    try {
      conv = await import('../lib/convert.js')
      source = await (await fetch(url(item.url, true))).blob()
      info = await conv.probe(source, kind)
      formats = await conv.supportedFormats(kind)
      format = formats[0]?.id || ''
      width = info.width || 0
      height = info.height || 0
      targetValue = Math.max(0.1, Math.round((source.size / UNITS.MB / 2) * 10) / 10)
    } catch (e) {
      error = String(e.message || e)
    } finally {
      loading = false
    }
  })

  function setWidth(/** @type {number} */ w) {
    width = w
    if (lockAspect && info.width) height = Math.round((w * info.height) / info.width)
  }
  function setHeight(/** @type {number} */ h) {
    height = h
    if (lockAspect && info.height) width = Math.round((h * info.width) / info.height)
  }

  async function run() {
    if (!source || !conv || busy) return
    busy = true
    error = ''
    result = null
    progress = 0
    abort = new AbortController()
    const targetBytes = useTarget ? Math.round(targetValue * UNITS[targetUnit]) : undefined
    try {
      result = kind === 'image'
        ? await conv.convertImage(source, { format, width, height, quality: quality / 100, targetBytes })
        : await conv.convertMedia(source, {
          format, kbps, sampleRate: sampleRate || undefined, channels: channels || undefined,
          width: isVideoOut ? videoWidth || undefined : undefined, videoKbps: isVideoOut ? videoKbps || undefined : undefined,
          targetBytes, onProgress: (p) => (progress = p), signal: abort.signal,
        })
    } catch (e) {
      if (!abort.signal.aborted) error = String(e.message || e)
    } finally {
      busy = false
    }
  }

  function close() {
    abort?.abort()
    onClose()
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="scrim z-50 flex items-end md:items-center justify-center md:p-6" role="dialog" aria-modal="true" aria-label="Convert" transition:fade={{ duration: PAGE_MS / 2 }}>
  <div class="w-full md:max-w-3xl 2xl:max-w-5xl max-h-full overflow-y-auto glass-panel rounded-t-3xl md:rounded-3xl p-4 sm:p-6 flex flex-col gap-4"
    in:fly={{ y: 60, duration: PAGE_MS, easing: EASE }} out:fly={{ y: 40, duration: 200 }}>
    <div class="flex items-start gap-3">
      <div class="min-w-0 flex-1">
        <h2 class="font-bold tracking-wide flex items-center gap-2"><Repeat size={18} class="text-neon-red" /> Convert</h2>
        <p class="text-xs 2xl:text-base text-neutral-400 line-clamp-1">{item.job.prompt}</p>
      </div>
      <button class="p-2 rounded-lg border border-white/10 hover:border-neon-red/50" onclick={close} aria-label="Close"><X size={16} /></button>
    </div>

    {#if loading}
      <div class="py-10 flex justify-center"><LoaderCircle class="animate-spin text-neon-red" /></div>
    {:else}
      <div class="grid md:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] gap-5">
        <!-- Source -->
        <div class="flex flex-col gap-2">
          <div class="rounded-xl overflow-hidden bg-black/50 flex items-center justify-center min-h-32">
            {#if kind === 'image'}<img src={url(item.url, true)} alt="Source" class="max-h-64 object-contain" />
            {:else if kind === 'video'}<!-- svelte-ignore a11y_media_has_caption --><video src={url(item.url, true)} controls muted class="max-h-64"></video>
            {:else}<audio src={url(item.url, true)} controls class="w-full m-4"></audio>{/if}
          </div>
          <p class="text-[11px] 2xl:text-sm text-neutral-500">
            Original: {item.file.split('.').pop()?.toUpperCase()} · {size(source?.size)}{info.width ? ` · ${info.width}×${info.height}` : ''}{info.duration ? ` · ${info.duration.toFixed(1)} s` : ''}
          </p>
        </div>

        <!-- Options -->
        <div class="flex flex-col gap-4">
          <div class="flex flex-col gap-1.5">
            <span class={LABEL}>Format</span>
            <div class="flex flex-wrap gap-1.5">
              {#each formats as f (f.id)}
                <button class="{CHIP} {format === f.id ? 'bg-neon-red/15 border-neon-red/50 text-white' : 'border-white/10 text-muted hover:text-white'}"
                  onclick={() => { format = f.id; result = null }}>
                  {f.id.toUpperCase()}{kind === 'video' && !VIDEO_FORMAT_IDS.includes(f.id) ? ' ♪' : ''}
                </button>
              {/each}
            </div>
            {#if kind === 'video'}<span class="text-[10px] 2xl:text-sm text-neutral-600">♪ = soundtrack only</span>{/if}
          </div>

          {#if kind === 'image'}
            <div class="grid grid-cols-[1fr_auto_1fr] items-end gap-2">
              <label class="flex flex-col gap-1"><span class={LABEL}>Width</span>
                <input class={INPUT} type="number" min="16" value={width} oninput={(e) => setWidth(+e.currentTarget.value)} /></label>
              <button class="mb-2 p-1.5 rounded-lg {lockAspect ? 'text-neon-red' : 'text-muted'}" onclick={() => (lockAspect = !lockAspect)}
                aria-label="Lock aspect ratio" title="Keep aspect ratio">{#if lockAspect}<Lock size={14} />{:else}<Unlock size={14} />{/if}</button>
              <label class="flex flex-col gap-1"><span class={LABEL}>Height</span>
                <input class={INPUT} type="number" min="16" value={height} oninput={(e) => setHeight(+e.currentTarget.value)} /></label>
            </div>
          {/if}

          {#if isVideoOut}
            <div class="grid grid-cols-2 gap-2">
              <label class="flex flex-col gap-1"><span class={LABEL}>Resolution</span>
                <select class={INPUT} bind:value={videoWidth}>
                  {#each VIDEO_WIDTHS as w (w)}<option value={w}>{w ? `${w} px wide` : 'Keep'}</option>{/each}
                </select></label>
              <label class="flex flex-col gap-1"><span class={LABEL}>Video kbps</span>
                <input class={INPUT} type="number" min="0" step="100" placeholder="auto" value={videoKbps || ''} disabled={useTarget}
                  oninput={(e) => (videoKbps = +e.currentTarget.value)} /></label>
            </div>
          {/if}

          {#if kind !== 'image' && !lossless}
            <div class="grid grid-cols-3 gap-2">
              <label class="flex flex-col gap-1"><span class={LABEL}>Audio kbps</span>
                <select class={INPUT} bind:value={kbps} disabled={useTarget && !isVideoOut}>
                  {#each AUDIO_KBPS as k (k)}<option value={k}>{k}</option>{/each}
                </select></label>
              <label class="flex flex-col gap-1"><span class={LABEL}>Sample rate</span>
                <select class={INPUT} bind:value={sampleRate}>
                  {#each SAMPLE_RATES as r (r)}<option value={r}>{r ? `${r / 1000} kHz` : 'Keep'}</option>{/each}
                </select></label>
              <label class="flex flex-col gap-1"><span class={LABEL}>Channels</span>
                <select class={INPUT} bind:value={channels}>
                  <option value={0}>Keep</option><option value={2}>Stereo</option><option value={1}>Mono</option>
                </select></label>
            </div>
          {/if}

          {#if kind === 'image' && !lossless}
            <label class="flex flex-col gap-1">
              <span class="{LABEL} flex justify-between"><span>Quality</span><span>{quality}</span></span>
              <input type="range" min="1" max="100" bind:value={quality} disabled={useTarget} class="accent-[#ff3344]" />
            </label>
          {/if}

          {#if !(kind !== 'image' && lossless)}
            <div class="flex flex-col gap-1.5 p-3 rounded-xl bg-black/30 border border-white/5">
              <label class="flex items-center gap-2 text-sm 2xl:text-lg cursor-pointer">
                <input type="checkbox" bind:checked={useTarget} class="accent-[#ff3344]" /> Fit a target file size
              </label>
              {#if useTarget}
                <div class="flex gap-2" transition:fade={{ duration: 150 }}>
                  <input class={INPUT} type="number" min="0.01" step="0.1" bind:value={targetValue} />
                  <select class="{INPUT} w-24" bind:value={targetUnit}><option>KB</option><option>MB</option></select>
                </div>
                <span class="text-[10px] 2xl:text-sm text-neutral-500">
                  {kind === 'image' ? 'Finds the best quality that fits, shrinking the image only if it has to.' : 'Sets the bitrate from the length so the file lands just under this size.'}
                </span>
              {/if}
            </div>
          {/if}

          <button class="flex items-center justify-center gap-2 py-3 rounded-xl font-bold licorice disabled:opacity-50"
            onclick={run} disabled={busy || !format || !source}>
            {#if busy}<LoaderCircle size={16} class="animate-spin" /> Converting{progress ? ` ${Math.round(progress * 100)}%` : '…'}{:else}<Repeat size={16} /> Convert to {format.toUpperCase()}{/if}
          </button>
          {#if busy && kind !== 'image'}
            <div class="h-1 rounded bg-white/10 -mt-2"><div class="h-1 rounded bg-neon-red transition-[width] duration-300" style="width: {progress * 100}%"></div></div>
          {/if}
          {#if error}<p class="text-xs text-neon-red break-words">{error}</p>{/if}

          {#if result}
            <div class="flex items-center gap-3 p-3 rounded-xl bg-emerald-500/10 border border-emerald-500/30" in:fly={{ y: 10, duration: PAGE_MS, easing: EASE }}>
              <div class="flex-1 min-w-0 text-sm 2xl:text-lg">
                <p class="font-semibold">{baseName}.{format}</p>
                <p class="text-xs 2xl:text-sm text-neutral-400">
                  {size(result.size)}
                  {#if source}· {result.size < source.size ? `${Math.round((1 - result.size / source.size) * 100)}% smaller` : `${Math.round((result.size / source.size - 1) * 100)}% larger`}{/if}
                  {#if useTarget}· target {targetValue} {targetUnit}{/if}
                </p>
                {#if useTarget && result.size > targetValue * UNITS[targetUnit]}
                  <p class="text-[11px] 2xl:text-sm text-amber-400">
                    This is the smallest {format.toUpperCase()} this device can make{kind !== 'image' ? ' (its audio encoder has a minimum bitrate)' : ''}.
                    {kind !== 'image' ? 'WebM or OGG (Opus) go much smaller.' : 'Try JPG or WebP, or a smaller size.'}
                  </p>
                {/if}
              </div>
              <button class="flex items-center gap-1.5 px-4 py-2 rounded-xl font-bold bg-white text-black hover:bg-neutral-200"
                onclick={() => saveBlob(/** @type {Blob} */ (result), `${baseName}.${format}`)}><Download size={15} /> Save</button>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
