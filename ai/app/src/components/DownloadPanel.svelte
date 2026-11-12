<script>
  import { Download, ExternalLink, Lock, RefreshCw } from '@lucide/svelte'
  import { store, KINDS } from '../lib/store.svelte.js'
  import { size } from '../lib/format.js'
  import HubSearch from './HubSearch.svelte'

  const BTN = 'flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs 2xl:text-base border border-white/10 hover:border-neon-red/50 hover:text-neon-red transition-colors disabled:opacity-40'
  const LINK = 'inline-flex items-center gap-1 text-neutral-400 hover:text-neon-red'

  // Anything not fully installed: families with no copy at all, plus installed copies missing files.
  const offers = $derived.by(() => {
    const installed = new Map(store.catalog.models.filter((m) => !m.custom).map((m) => [m.family, m]))
    return store.catalog.families
      .map((f) => {
        const m = installed.get(f.id)
        if (m?.status === 'ready') return null
        return { family: f, download: m ? m.download : f.download, partial: !!m }
      })
      .filter(Boolean)
  })
  const running = $derived(new Set(store.downloads.filter((d) => d.status === 'running').map((d) => d.target)))
  let error = $state('')

  async function get(/** @type {string} */ id) {
    error = ''
    try { await store.download(id) } catch (e) { error = String(e.message || e) }
  }
</script>

<div class="flex flex-col gap-4">
  <HubSearch />

  <div class="flex items-center justify-between gap-3 pt-2 border-t border-white/5">
    <h3 class="text-xs 2xl:text-base uppercase tracking-wider text-muted">Built-in models</h3>
    <button class={BTN} onclick={() => store.scan()} disabled={store.scanning}><RefreshCw size={14} class={store.scanning ? 'animate-spin' : ''} /> Rescan disks</button>
  </div>

  {#each KINDS as k (k.id)}
    {@const list = offers.filter((o) => o?.family.kind === k.id)}
    {#if list.length}
      <h4 class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted">{k.label}</h4>
      <div class="grid gap-2 lg:grid-cols-2">
        {#each list as o (o?.family.id)}
          {#if o}
            <div class="p-3 rounded-xl bg-black/25 border border-white/5 flex flex-col gap-2">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="font-semibold">{o.family.label}{#if o.partial}<span class="text-amber-400 text-xs font-normal"> · missing files</span>{/if}</p>
                  <p class="text-xs 2xl:text-sm text-neutral-500">{o.family.blurb}</p>
                </div>
                <button class="{BTN} shrink-0" onclick={() => get(o.family.id)} disabled={running.has(o.family.id)}>
                  <Download size={14} /> {running.has(o.family.id) ? 'Downloading…' : (o.download.sizeUnknown ? '≥ ' : '') + size(o.download.size)}
                </button>
              </div>
              <div class="flex flex-col gap-0.5 text-[11px] 2xl:text-sm">
                {#each o.download.items as it (it.file)}
                  <a class={LINK} href={it.url} target="_blank" rel="noopener noreferrer" title="{it.repo}/{it.file}">
                    <span class="w-14 shrink-0 text-right text-neutral-500">{size(it.size)}</span><span class="truncate">{it.file.split('/').pop()}</span><ExternalLink size={10} class="shrink-0" />
                  </a>
                {/each}
                {#if o.download.gated}
                  <a class="{LINK} text-amber-400" href={o.download.gated} target="_blank" rel="noopener noreferrer"><Lock size={10} /> Gated: accept the license on Hugging Face first</a>
                {/if}
              </div>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  {/each}
  {#if !offers.length}<p class="text-sm text-muted">Every built-in model is installed.</p>{/if}

  {#each store.downloads.slice(0, 4) as d (d.id)}
    <div class="p-3 rounded-xl well text-xs 2xl:text-sm font-mono">
      <div class="mb-1 {d.status === 'error' ? 'text-neon-red' : d.status === 'done' ? 'text-emerald-400' : 'text-muted'}">{d.label}: {d.status}</div>
      <div class="text-neutral-500 break-all">{d.error || d.log.at(-1) || ''}</div>
    </div>
  {/each}
  {#if error}<p class="text-neon-red text-xs">{error}</p>{/if}
</div>
