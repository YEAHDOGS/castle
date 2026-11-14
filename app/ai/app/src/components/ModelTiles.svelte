<script>
  import { Image, Clapperboard, AudioLines, MessageSquareText, FileQuestion, Trash2, Power } from '@lucide/svelte'
  import { scale } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { store } from '../lib/store.svelte.js'
  import { buildTiles, tokens } from '../lib/library.js'
  import { size } from '../lib/format.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import Confirm from './Confirm.svelte'

  /** @type {{ expanded: boolean }} */
  let { expanded } = $props()

  const ICONS = { image: Image, video: Clapperboard, audio: AudioLines, llm: MessageSquareText, file: FileQuestion }
  const KIND_LABEL = { image: 'Image', video: 'Video', audio: 'Audio', llm: 'Chat LLM', file: 'Unused file' }
  const STATUS_CLASS = { ready: 'text-emerald-400', loaded: 'text-sky-400', incomplete: 'text-amber-400', broken: 'text-neon-red', unused: 'text-neutral-500' }
  const TILE_MIN_PX = 150
  const GAP_PX = 12

  let width = $state(0)
  const cols = $derived(Math.max(1, Math.floor((width + GAP_PX) / (TILE_MIN_PX + GAP_PX))))
  const tiles = $derived(buildTiles(store.catalog, store.llm))
  const shown = $derived(expanded ? tiles : tiles.slice(0, cols))
  /** @type {import('../lib/library.js').Tile | null} */
  let pending = $state(null)

  async function remove() {
    const t = /** @type {import('../lib/library.js').Tile} */ (pending)
    if (t.llm) await store.deleteLlm(t.llm)
    else {
      const r = await store.deleteModelFiles(t.deleteIds || [])
      if (r.errors.length) throw new Error(r.errors.join('; '))
    }
    pending = null
  }

  const confirmText = (/** @type {import('../lib/library.js').Tile} */ t) =>
    t.llm
      ? t.shares?.length
        ? `Removes the "${t.name}" tag from Ollama. Its weights stay on disk because ${t.shares.join(', ')} use them too.`
        : `Removes "${t.name}" from Ollama and frees ${size(t.freed)}.`
      : t.deleteIds?.length
        ? `Permanently deletes these files and frees ${size(t.freed)}. Files shared with other models are kept. This can't be undone.`
        : 'Every file of this model is shared with another model, so there is nothing to delete on its own.'
</script>

<div class="grid gap-3" bind:clientWidth={width} style="grid-template-columns: repeat({cols}, minmax(0, 1fr))">
  {#each shown as t (t.key)}
    {@const Icon = ICONS[t.kind]}
    <div class="tile group relative aspect-square rounded-xl bg-black/30 border border-white/5 hover:border-neon-red/40 p-3 flex flex-col"
      animate:flip={{ duration: PAGE_MS, easing: EASE }} in:scale={{ start: 0.9, duration: PAGE_MS, easing: EASE }} out:scale={{ start: 0.9, duration: 150 }}
      title="{t.name}{t.sub ? `\n${t.sub}` : ''}">
      <div class="flex-1 flex items-center justify-center">
        <div class="icon w-12 h-12 2xl:w-16 2xl:h-16 rounded-2xl flex items-center justify-center bg-white/5 text-neutral-300 group-hover:text-neon-red">
          <Icon size={26} />
        </div>
      </div>
      <p class="text-xs 2xl:text-base font-semibold truncate">{t.name}</p>
      <p class="text-[10px] 2xl:text-sm text-neutral-500 truncate">{KIND_LABEL[t.kind]} · {size(t.size)}</p>
      {#if t.ctx}
        <p class="text-[10px] 2xl:text-sm text-neutral-400 truncate" title="Context window: configured / model maximum{t.ctx.loaded ? ' / in memory now' : ''}">
          ctx {t.ctx.configured ? tokens(t.ctx.configured) : 'default'} / {tokens(t.ctx.max)}
        </p>
      {:else}
        <p class="text-[10px] 2xl:text-sm truncate {STATUS_CLASS[t.status] || ''}">{t.status}</p>
      {/if}
      {#if t.loaded}
        <button class="absolute top-2 left-2 p-1.5 rounded-lg bg-sky-500/15 text-sky-300 hover:bg-sky-500/30" title="In memory ({size(t.loaded.vram)} VRAM). Click to unload."
          aria-label="Unload {t.name}" onclick={() => store.unloadLlm(/** @type {string} */ (t.llm))}><Power size={13} /></button>
      {/if}
      <button class="del absolute top-2 right-2 p-1.5 rounded-lg bg-black/50 text-neutral-400 hover:text-white hover:bg-neon-red"
        aria-label="Delete {t.name}" title="Delete" onclick={() => (pending = t)}><Trash2 size={13} /></button>
    </div>
  {/each}
</div>
{#if !tiles.length}<p class="text-sm text-muted">No models on this machine yet. Open "Download models" above.</p>{/if}

{#if pending}
  <Confirm title="Delete {pending.name}?" message={confirmText(pending)} items={pending.deletePaths || (pending.llm ? [pending.llm] : [])}
    confirmLabel={pending.llm ? 'Remove' : `Delete · ${size(pending.freed)}`} onConfirm={remove} onCancel={() => (pending = null)} />
{/if}

<style>
  .tile {
    transition: border-color 0.3s cubic-bezier(0.16, 1, 0.3, 1), transform 0.3s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.3s;
  }

  .tile:hover {
    transform: translateY(-3px);
    box-shadow: 0 8px 24px rgb(0 0 0 / 40%), 0 0 16px rgb(255 51 68 / 12%);
  }

  .icon {
    transition: color 0.3s, transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .tile:hover .icon {
    transform: scale(1.08);
  }

  /* Delete shows on hover with a mouse; always visible on touch screens */
  .del {
    opacity: 0;
    transition: opacity 0.2s, background-color 0.2s;
  }

  .tile:hover .del,
  .del:focus-visible {
    opacity: 1;
  }

  @media (hover: none) {
    .del {
      opacity: 1;
    }
  }
</style>
