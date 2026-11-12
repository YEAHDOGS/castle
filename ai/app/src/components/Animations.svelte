<script>
  // Videos made from one image, shown next to it: a progress ring while rendering, then the clip (plays on hover).
  import { TriangleAlert, X } from '@lucide/svelte'
  import { scale } from 'svelte/transition'
  import { store } from '../lib/store.svelte.js'
  import { url } from '../lib/api.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'

  /** @type {{ file: string, size?: string }} */
  let { file, size = 'w-20 h-20 2xl:w-28 2xl:h-28' } = $props()

  const RING_R = 15
  const RING_C = 2 * Math.PI * RING_R

  const jobs = $derived((store.animations.get(file) || []).filter((j) => j.status !== 'cancelled'))
  const pct = (/** @type {any} */ p) => (p?.max ? p.value / p.max : 0)
  const STAGE = (/** @type {any} */ j) => (j.status === 'queued' ? 'Waiting in queue' : j.progress?.max ? `${j.progress.stage} ${j.progress.value}/${j.progress.max}` : j.progress?.stage || j.status)

  const play = (/** @type {Event} */ e) => /** @type {HTMLVideoElement} */ (e.currentTarget).play().catch(() => {})
  const rewind = (/** @type {Event} */ e) => { const v = /** @type {HTMLVideoElement} */ (e.currentTarget); v.pause(); v.currentTime = 0.1 }
</script>

{#each jobs as j (j.id)}
  {@const o = j.outputs[0]}
  <div class="relative shrink-0 {size} rounded-lg overflow-hidden bg-black/50 border border-neon-red/25" in:scale={{ start: 0.8, duration: PAGE_MS, easing: EASE }}>
    {#if j.status === 'done' && o}
      <button class="block w-full h-full" onclick={() => (store.preview = { job: j, output: o })} aria-label="Open video" title="Video · {j.modelLabel}">
        <video src={url(o.url, true) + '#t=0.1'} muted loop playsinline preload="metadata" class="w-full h-full object-cover"
          onmouseenter={play} onmouseleave={rewind}></video>
      </button>
    {:else if j.status === 'error'}
      <div class="w-full h-full flex items-center justify-center text-neon-red/80" title={j.error || 'failed'}><TriangleAlert size={18} /></div>
    {:else}
      <div class="group w-full h-full flex items-center justify-center" title={STAGE(j)}>
        <svg viewBox="0 0 36 36" class="w-3/5 h-3/5 -rotate-90">
          <circle cx="18" cy="18" r={RING_R} fill="none" stroke="rgb(255 255 255 / 10%)" stroke-width="3" />
          <circle cx="18" cy="18" r={RING_R} fill="none" stroke="#ff3344" stroke-width="3" stroke-linecap="round"
            stroke-dasharray={RING_C} stroke-dashoffset={RING_C * (1 - pct(j.progress))} class="transition-[stroke-dashoffset] duration-500" />
        </svg>
        <button class="absolute inset-0 hidden group-hover:flex items-center justify-center bg-black/60 text-neutral-300 hover:text-neon-red"
          onclick={() => store.cancel(j.id)} aria-label="Cancel" title="Cancel"><X size={16} /></button>
      </div>
    {/if}
  </div>
{/each}
