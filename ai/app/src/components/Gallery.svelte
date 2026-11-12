<script>
  import { LoaderCircle, X, TriangleAlert } from '@lucide/svelte'
  import { store } from '../lib/store.svelte.js'
  import { url } from '../lib/api.js'
  import Media from './Media.svelte'
  import AnimateButton from './AnimateButton.svelte'
  import Animations from './Animations.svelte'
  import { scale } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { EASE, PAGE_MS } from '../lib/motion.js'

  const STAGE = (/** @type {any} */ j) => {
    if (j.status === 'queued') return 'Waiting in queue'
    const p = j.progress
    if (!p) return j.status
    return p.max ? `${p.stage} ${p.value}/${p.max}` : p.stage
  }
  const pct = (/** @type {any} */ p) => (p?.max ? Math.round((p.value / p.max) * 100) : null)
  const elapsed = (/** @type {any} */ j) => (j.finished && j.started ? `${Math.round((j.finished - j.started) / 1000)}s` : '')
</script>

{#if !store.kindJobs.length}
  <div class="h-full min-h-48 flex items-center justify-center text-center text-muted text-sm 2xl:text-xl">
    <p>Nothing generated yet.<br />Write a prompt and hit Generate.</p>
  </div>
{:else}
  <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4 gap-3 2xl:gap-5">
    {#each store.kindJobs as j (j.id)}
      <article class="rounded-xl overflow-hidden bg-card/60 border border-white/5 flex flex-col" animate:flip={{ duration: PAGE_MS, easing: EASE }} in:scale|local={{ start: 0.92, duration: PAGE_MS, easing: EASE }}>
        {#if j.status === 'done'}
          {#each j.outputs as o (o.file)}
            <Media src={url(o.url, true)} mime={o.mime} onOpen={() => (store.preview = { job: j, output: o })} />
          {/each}
        {:else if j.status === 'error' || j.status === 'cancelled'}
          <div class="aspect-square flex flex-col items-center justify-center gap-2 p-4 text-center text-xs 2xl:text-base {j.status === 'error' ? 'text-neon-red' : 'text-muted'}">
            <TriangleAlert size={22} />
            <span class="max-h-40 overflow-y-auto [overflow-wrap:anywhere] select-text" title={j.error || ''}>{j.error || 'cancelled'}</span>
          </div>
        {:else}
          <div class="aspect-square flex flex-col items-center justify-center gap-3 p-4">
            <LoaderCircle size={28} class="animate-spin text-neon-red" />
            <span class="text-xs 2xl:text-base text-muted capitalize">{STAGE(j)}</span>
            {#if pct(j.progress) !== null}
              <div class="w-2/3 h-1 bg-white/10 rounded"><div class="h-1 bg-neon-red rounded transition-all" style="width: {pct(j.progress)}%"></div></div>
            {/if}
            <button class="text-xs flex items-center gap-1 text-muted hover:text-neon-red" onclick={() => store.cancel(j.id)}><X size={12} /> Cancel</button>
          </div>
        {/if}
        <div class="p-2.5 2xl:p-4 text-[11px] 2xl:text-sm text-neutral-400 flex flex-col gap-0.5">
          <p class="line-clamp-2 text-neutral-200" title={j.prompt}>{j.prompt}</p>
          <p class="text-neutral-600 truncate">{j.modelLabel} · seed {j.seed} {elapsed(j) ? '· ' + elapsed(j) : ''}</p>
          {#if j.status === 'done' && j.kind === 'image'}
            {#each j.outputs as o (o.file)}
              <div class="flex items-center gap-1.5 mt-1.5 flex-wrap">
                <AnimateButton file={o.file} url={o.url} label />
                <Animations file={o.file} size="w-12 h-12 2xl:w-16 2xl:h-16" />
              </div>
            {/each}
          {/if}
        </div>
      </article>
    {/each}
  </div>
{/if}
