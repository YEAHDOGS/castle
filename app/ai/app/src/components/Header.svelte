<script>
  import { Settings } from '@lucide/svelte'
  import logo from '../assets/dogs-logo-cropped.png'
  import { store } from '../lib/store.svelte.js'
  import { api } from '../lib/api.js'
  import { errors } from '../lib/errors.svelte.js'
  import { isTauri, isMac } from '../lib/shell.js'
  import WindowControls from './WindowControls.svelte'

  /** @type {{ onSettings: () => void }} */
  let { onSettings } = $props()

  const gpu = $derived(store.health?.comfy?.gpus?.[0])
  let startClicked = $state(false)
  const comfyUp = $derived(!!store.health?.comfy?.up)
  const comfyStarting = $derived(!comfyUp && (startClicked || !!store.health?.comfy?.starting))
  const comfyError = $derived(comfyUp || comfyStarting ? '' : store.health?.comfy?.error || '')
  const comfyTitle = $derived(comfyUp ? 'ComfyUI running' : comfyStarting ? 'ComfyUI is starting…' : comfyError ? `${comfyError}\n\nClick to try again` : 'ComfyUI is stopped. Click to start it')
  const comfyDot = $derived(comfyUp ? 'bg-emerald-400' : comfyStarting ? 'bg-amber-400' : comfyError ? 'bg-neon-red' : 'bg-neutral-600')

  async function startComfy() {
    startClicked = true
    try {
      await api('/api/comfy/start', { method: 'POST' })
    } catch (e) {
      errors.report(e)
    } finally {
      startClicked = false
      store.refreshHealth().catch(() => {})
    }
  }
  const ICON_BTN = 'p-2 rounded-lg border border-white/5 hover:border-neon-red/40 hover:text-neon-red transition-colors'

  // Exposed as --header-h so overlays below the header (they sit in a lower stacking context) can start under it.
  let height = $state(0)
  $effect(() => { if (height) document.documentElement.style.setProperty('--header-h', `${height}px`) })
</script>

<!-- Also the window's title bar in the desktop app (see .titlebar in app.css): drag it to move the window. Tauri only
     drags from elements that carry data-tauri-drag-region themselves, so the non-interactive parts all do. -->
<header data-tauri-drag-region bind:offsetHeight={height} class="titlebar relative flex items-center justify-between gap-3 px-4 2xl:px-6 border-b border-glass-edge bg-black/25 backdrop-blur-xl z-20">
  <div data-tauri-drag-region class="flex items-center gap-2.5 min-w-0">
    <img data-tauri-drag-region draggable="false" src={logo} alt="DOGS logo" class="w-8 h-8 2xl:w-12 2xl:h-12 object-contain" />
    <h1 data-tauri-drag-region class="text-lg 2xl:text-3xl font-bold tracking-wider">DOGS <span class="text-neon-red">AI</span></h1>
  </div>
  <div data-tauri-drag-region class="self-stretch flex items-center gap-2 sm:gap-3 text-xs 2xl:text-base text-muted">
    <span class="hidden sm:flex items-center gap-1.5" title={store.connected ? 'Engine running' : 'Engine offline'}>
      <span class="w-2 h-2 rounded-full {store.connected ? 'bg-emerald-400' : 'bg-neon-red'}"></span> Engine
    </span>
    {#if comfyUp}
      <span class="hidden sm:flex items-center gap-1.5" title={comfyTitle}>
        <span class="w-2 h-2 rounded-full {comfyDot}"></span> ComfyUI
      </span>
    {:else}
      <button class="hidden sm:flex items-center gap-1.5 px-2 py-1 -mx-2 rounded-md hover:bg-white/10 hover:text-white disabled:cursor-wait transition-colors {comfyError ? 'text-neon-red' : ''}"
        title={comfyTitle} onclick={startComfy} disabled={comfyStarting}>
        <span class="w-2 h-2 rounded-full {comfyDot}"></span> {comfyStarting ? 'Starting ComfyUI…' : comfyError ? 'ComfyUI · retry' : 'Start ComfyUI'}
      </button>
    {/if}
    {#if gpu}
      <span class="hidden lg:inline truncate max-w-64">{gpu.name.replace(/^cuda:\d+\s*/, '').replace(/\s*:\s*\w+$/, '')} · {gpu.vram_free_gb}/{gpu.vram_total_gb} GB</span>
    {/if}
    <button class={ICON_BTN} onclick={onSettings} title="Settings" aria-label="Settings"><Settings size={18} /></button>
    {#if isTauri && !isMac}<WindowControls />{/if}
  </div>
</header>
