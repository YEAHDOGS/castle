<script>
  import { RefreshCw, Download, X, CircleCheck, CircleAlert, FolderOpen, ExternalLink, Lock } from '@lucide/svelte'
  import { store } from '../lib/store.svelte.js'
  import { url } from '../lib/api.js'
  import { size } from '../lib/format.js'
  import SwapText from './SwapText.svelte'
  import HubSearch from './HubSearch.svelte'

  /** @type {{ onClose: (() => void) | null }} */
  let { onClose } = $props()

  const STATUS_CLASS = { ready: 'text-emerald-400', incomplete: 'text-amber-400', broken: 'text-neon-red' }
  const BTN = 'flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs 2xl:text-base border border-white/10 hover:border-neon-red/50 hover:text-neon-red transition-colors disabled:opacity-40'
  const LINK = 'inline-flex items-center gap-1 text-neutral-400 hover:text-neon-red'

  const installedFamilies = $derived(new Set(store.models.map((m) => m.family)))
  const notInstalled = $derived(store.catalog.families.filter((f) => f.kind === store.kind && !installedFamilies.has(f.id)))
  const running = $derived(new Set(store.downloads.filter((d) => d.status === 'running').map((d) => d.target)))
  let error = $state('')

  async function download(/** @type {string} */ id) {
    error = ''
    try { await store.download(id) } catch (e) { error = String(e.message || e) }
  }
</script>

{#snippet links(/** @type {any} */ d)}
  <div class="flex flex-col gap-0.5 text-[11px] 2xl:text-sm">
    {#each d.items as it (it.file)}
      <a class={LINK} href={it.url} target="_blank" rel="noopener noreferrer" title={`${it.repo}/${it.file}`}>
        <span class="w-14 shrink-0 text-right text-neutral-500">{size(it.size)}</span>
        <span class="truncate">{it.file.split('/').pop()}</span><ExternalLink size={10} class="shrink-0" />
      </a>
    {/each}
    {#if d.gated}
      <a class="{LINK} text-amber-400" href={d.gated} target="_blank" rel="noopener noreferrer"><Lock size={10} /> Gated: accept the license on Hugging Face first</a>
    {/if}
  </div>
{/snippet}

<div class="flex flex-col gap-4 text-sm 2xl:text-lg">
  <div class="flex items-center justify-between">
    <h2 class="font-bold tracking-wide"><SwapText text={`${store.kind[0].toUpperCase() + store.kind.slice(1)} models`} /></h2>
    <div class="flex gap-2">
      <button class={BTN} onclick={() => store.scan()} disabled={store.scanning}>
        <RefreshCw size={14} class={store.scanning ? 'animate-spin' : ''} /> Rescan
      </button>
      {#if onClose}<button class={BTN} onclick={onClose} aria-label="Close"><X size={14} /></button>{/if}
    </div>
  </div>

  {#if store.startFrame && store.kind === 'video'}
    <div class="flex items-center gap-3 p-2 rounded-xl bg-neon-red/10 border border-neon-red/30">
      <img src={url(store.startFrame.url, true)} alt="Start frame" class="w-14 h-14 rounded-lg object-cover" />
      <p class="text-xs 2xl:text-base text-neutral-300">This image is ready to animate. Install a video model below and it'll be waiting as the start frame.</p>
    </div>
  {/if}

  {#if !store.ready.length}
    <p class="text-muted">No {store.kind} model is ready yet. A model needs all of its files (weights, text encoder, VAE). Download one below, or bring files you already have.</p>
  {/if}

  {#each store.models as m (m.id)}
    <div class="p-3 rounded-xl bg-card/60 border border-white/5 flex flex-col gap-1.5">
      <div class="flex items-center justify-between gap-2">
        <span class="font-semibold truncate">{m.label}</span>
        <span class="text-xs {STATUS_CLASS[m.status]}">{m.status}</span>
      </div>
      {#each Object.entries(m.files) as [role, f] (role)}
        <div class="flex items-center gap-1.5 text-xs 2xl:text-sm {f && !f.broken ? 'text-neutral-400' : 'text-amber-400'}">
          {#if f && !f.broken}<CircleCheck size={12} />{:else}<CircleAlert size={12} />{/if}
          <span class="w-24 shrink-0 text-muted">{role.replace('_', ' ')}</span>
          <span class="truncate" title={f?.path || ''}>{f ? `${f.name} · ${size(f.size)} · ${f.location}${f.broken ? ' · broken link' : ''}` : 'missing'}</span>
        </div>
      {/each}
      {#if m.status !== 'ready' && m.download?.items.length}
        {@render links(m.download)}
        <button class="{BTN} self-start mt-1" onclick={() => download(m.family)} disabled={running.has(m.family)}>
          <Download size={14} /> {running.has(m.family) ? 'Downloading…' : `Download missing files · ${size(m.download.size)}`}
        </button>
      {/if}
      {#if m.requiresNodes?.length}
        <p class="text-[11px] 2xl:text-sm text-amber-400">Needs ComfyUI node {m.requiresNodes.join(', ')}: <code>dogs comfy add-node gguf</code></p>
      {/if}
    </div>
  {/each}

  {#if notInstalled.length}
    <h3 class="text-xs 2xl:text-base uppercase tracking-wider text-muted mt-2">Available to download</h3>
    {#each notInstalled as f (f.id)}
      <div class="p-3 rounded-xl bg-card/40 border border-white/5 flex flex-col gap-2">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0">
            <div class="font-semibold">{f.label}</div>
            <div class="text-xs 2xl:text-sm text-neutral-500">{f.blurb}</div>
          </div>
          <button class="{BTN} shrink-0" onclick={() => download(f.id)} disabled={running.has(f.id)}>
            <Download size={14} /> {running.has(f.id) ? '…' : size(f.download.size)}
          </button>
        </div>
        {@render links(f.download)}
        <code class="text-[11px] 2xl:text-sm text-neutral-500">dogs download {f.id}</code>
      </div>
    {/each}
  {/if}

  {#each store.downloads.slice(0, 5) as d (d.id)}
    <div class="p-3 rounded-xl well text-xs 2xl:text-sm font-mono">
      <div class="mb-1 {d.status === 'error' ? 'text-neon-red' : d.status === 'done' ? 'text-emerald-400' : 'text-muted'}">{d.label}: {d.status}</div>
      <div class="text-neutral-500 break-all">{d.error || d.log.at(-1) || ''}</div>
    </div>
  {/each}
  {#if error}<p class="text-neon-red text-xs">{error}</p>{/if}

  <HubSearch />

  <details class="rounded-xl bg-card/40 border border-white/5 p-3" open={!store.ready.length}>
    <summary class="cursor-pointer flex items-center gap-2 font-semibold"><FolderOpen size={14} /> Where models are found</summary>
    <ul class="mt-2 flex flex-col gap-1 text-xs 2xl:text-sm">
      {#each store.catalog.locations as l (l.dir)}
        <li class="flex gap-2 {l.exists ? 'text-neutral-300' : 'text-neutral-600'}">
          <span class="w-28 shrink-0 truncate">{l.label}</span><span class="break-all">{l.dir}</span>
        </li>
      {/each}
    </ul>
    <div class="mt-3 text-xs 2xl:text-sm text-neutral-400 flex flex-col gap-2">
      <p>The Hugging Face CLI saves downloads to <code>.cache\huggingface\hub</code> in the profile of whoever ran it, often an admin account. Every profile on this machine is scanned, so those show up here automatically.</p>
      <p>You can also cut and paste a whole <code>models--org--name</code> folder (including its <code>blobs</code> folder) or plain <code>.safetensors</code> files into the project's <code>models</code> folder, then Rescan.</p>
      <p>From a terminal:</p>
      <code class="block bg-black/40 rounded p-2 text-neutral-300 select-text">dogs hf auth login<br />dogs download flux3-klein<br />dogs hf search wan --kind video</code>
    </div>
  </details>

  {#if store.catalog.unrecognized.length}
    <details class="rounded-xl bg-card/40 border border-white/5 p-3">
      <summary class="cursor-pointer font-semibold">{store.catalog.unrecognized.length} unrecognized weight files</summary>
      <p class="mt-2 text-xs text-neutral-500">No built-in workflow uses these. Export an API-format workflow from ComfyUI into <code>workflows/</code> to use them.</p>
      <ul class="mt-2 text-xs text-neutral-400">
        {#each store.catalog.unrecognized as f (f.id)}<li class="truncate">{f.name} · {size(f.size)} · {f.location}</li>{/each}
      </ul>
    </details>
  {/if}
</div>
