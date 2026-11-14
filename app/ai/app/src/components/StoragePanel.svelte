<script>
  import { onMount } from 'svelte'
  import { FolderOpen, Eraser, RefreshCw, LoaderCircle, Link2 } from '@lucide/svelte'
  import { api } from '../lib/api.js'
  import { size } from '../lib/format.js'
  import Confirm from './Confirm.svelte'

  /** @type {{ onReport?: (r: any) => void }} */
  let { onReport } = $props()

  const KIND_LABEL = { media: 'Your media', models: 'Models', links: 'Links', cache: 'Cache', runtime: 'Engine', data: 'Data', dev: 'Build tools' }
  const KIND_COLOR = { media: '#38bdf8', models: '#ff3344', links: '#737373', cache: '#e6b900', runtime: '#a855f7', data: '#22c55e', dev: '#a3a3a3' }

  /** @type {any} */ let report = $state(null)
  let loading = $state(false)
  let error = $state('')
  /** @type {any} */ let clearing = $state(null)
  let note = $state('')

  const biggest = $derived(Math.max(1, ...(report?.places || []).map((p) => p.bytes)))

  async function load(refresh = false) {
    loading = true
    error = ''
    try {
      report = await api(`/api/storage${refresh ? '?refresh=1' : ''}`)
      onReport?.(report)
    } catch (e) { error = String(e.message || e) } finally { loading = false }
  }

  async function open(/** @type {string} */ id) {
    error = ''
    try { await api(`/api/storage/${id}/open`, { method: 'POST' }) } catch (e) { error = String(e.message || e) }
  }

  async function clear() {
    const r = await api(`/api/storage/${clearing.id}/clear`, { method: 'POST' })
    note = `Cleared ${clearing.label}: freed ${size(r.freed)}${r.errors.length ? ` (${r.errors.length} items were in use and stayed)` : ''}.`
    clearing = null
    await load(true)
  }

  onMount(() => load())
</script>

<div class="flex flex-col gap-3">
  <div class="flex items-center gap-3 text-xs 2xl:text-base text-muted">
    {#if report}
      <span><span class="text-xl 2xl:text-3xl font-bold text-white">{size(report.total)}</span> used by this app</span>
      {#if report.disk}<span class="hidden sm:inline">· {size(report.disk.free)} free on the drive</span>{/if}
    {:else if loading}
      <LoaderCircle size={14} class="animate-spin" /> Measuring every folder… (large caches take a few seconds)
    {/if}
    <button class="ml-auto flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border border-white/10 hover:border-neon-red/50 hover:text-white disabled:opacity-40"
      onclick={() => load(true)} disabled={loading}><RefreshCw size={13} class={loading ? 'animate-spin' : ''} /> Re-measure</button>
  </div>

  {#if note}<p class="text-xs 2xl:text-base text-emerald-400">{note}</p>{/if}
  {#if error}<p class="text-xs text-neon-red break-words">{error}</p>{/if}

  {#if report}
    <ul class="flex flex-col gap-1.5">
      {#each report.places as p (p.id)}
        <li class="row relative rounded-xl well overflow-hidden">
          <!-- Size bar behind the row, relative to the biggest folder -->
          <div class="absolute inset-y-0 left-0 opacity-15 transition-[width] duration-700" style="width: {(p.bytes / biggest) * 100}%; background: {KIND_COLOR[p.kind]}"></div>
          <div class="relative flex items-center gap-3 px-3 py-2.5">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 text-sm 2xl:text-lg">
                <span class="font-semibold truncate">{p.label}</span>
                <span class="shrink-0 text-[10px] 2xl:text-xs px-1.5 py-0.5 rounded-md border" style="color: {KIND_COLOR[p.kind]}; border-color: {KIND_COLOR[p.kind]}55">{KIND_LABEL[p.kind]}</span>
              </div>
              <button class="block max-w-full truncate text-left text-[11px] 2xl:text-sm text-neutral-500 hover:text-white font-mono" title="Open in file manager: {p.paths.join('\n')}" onclick={() => open(p.id)}>{p.path}</button>
              <p class="text-[11px] 2xl:text-sm text-neutral-600 line-clamp-2">{p.about}</p>
              {#if p.shared}
                <p class="text-[11px] 2xl:text-sm text-neutral-500 flex items-center gap-1"><Link2 size={11} /> plus {size(p.shared)} of hard links that take no extra space</p>
              {/if}
            </div>
            <span class="shrink-0 text-right font-semibold text-sm 2xl:text-lg tabular-nums w-20">{size(p.bytes)}</span>
            <div class="shrink-0 flex gap-1">
              <button class="p-2 rounded-lg hover:bg-white/10 text-neutral-300" title="Open folder" aria-label="Open {p.label}" onclick={() => open(p.id)}><FolderOpen size={16} /></button>
              {#if p.clearable && p.bytes}
                <button class="p-2 rounded-lg hover:bg-neon-red text-neutral-300 hover:text-white" title="Clear this cache" aria-label="Clear {p.label}" onclick={() => (clearing = p)}><Eraser size={16} /></button>
              {/if}
            </div>
          </div>
        </li>
      {/each}
    </ul>
    <p class="text-[11px] 2xl:text-sm text-neutral-600">Delete individual models in Models above, and generations in Generated below. Only caches can be cleared here: they refill themselves when needed.</p>
  {/if}
</div>

{#if clearing}
  <Confirm title="Clear {clearing.label}?" message="{clearing.about} Frees about {size(clearing.bytes)}." items={clearing.paths}
    confirmLabel="Clear · {size(clearing.bytes)}" onConfirm={clear} onCancel={() => (clearing = null)} />
{/if}
