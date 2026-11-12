<script>
  // Recent creations of the current kind as a horizontal strip. No visible scrollbar: swipe, drag, mouse wheel
  // or the arrow buttons move it, and it snaps to thumbnails.
  import { ChevronLeft, ChevronRight, AudioLines, TriangleAlert } from '@lucide/svelte'
  import { scale } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { store } from '../lib/store.svelte.js'
  import { url } from '../lib/api.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'

  const PAGE_FRACTION = 0.8
  const DRAG_THRESHOLD_PX = 6

  /** @type {HTMLDivElement} */ let strip
  let canLeft = $state(false)
  let canRight = $state(false)
  let drag = { x: 0, left: 0, active: false, moved: false }

  const items = $derived(store.kindJobs.slice(0, 60))

  function update() {
    if (!strip) return
    canLeft = strip.scrollLeft > 2
    canRight = strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 2
  }

  $effect(() => { void items.length; requestAnimationFrame(update) })

  const page = (/** @type {number} */ dir) => strip.scrollBy({ left: dir * strip.clientWidth * PAGE_FRACTION, behavior: 'smooth' })

  // Vertical wheel scrolls the strip sideways.
  function wheel(/** @type {WheelEvent} */ e) {
    if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return
    e.preventDefault()
    strip.scrollBy({ left: e.deltaY })
  }

  // Mouse drag to scroll (touch already scrolls natively).
  function down(/** @type {PointerEvent} */ e) {
    if (e.pointerType !== 'mouse') return
    drag = { x: e.clientX, left: strip.scrollLeft, active: true, moved: false }
  }
  function move(/** @type {PointerEvent} */ e) {
    if (!drag.active) return
    const dx = e.clientX - drag.x
    if (Math.abs(dx) > DRAG_THRESHOLD_PX) drag.moved = true
    strip.scrollLeft = drag.left - dx
  }
  const up = () => (drag.active = false)

  function open(/** @type {any} */ j) {
    if (drag.moved || j.status !== 'done' || !j.outputs[0]) return
    store.preview = { job: j, output: j.outputs[0] }
  }

  const pct = (/** @type {any} */ p) => (p?.max ? p.value / p.max : 0)
</script>

{#if items.length}
  <div class="relative glass rounded-2xl px-1 py-1.5">
    <div bind:this={strip} class="strip flex gap-2 overflow-x-auto scrollbar-none snap-x snap-mandatory px-1"
      onscroll={update} onwheel={wheel} onpointerdown={down} onpointermove={move} onpointerup={up} onpointerleave={up} role="list" aria-label="Recent creations">
      {#each items as j (j.id)}
        {@const o = j.outputs[0]}
        <button class="thumb relative shrink-0 snap-start w-20 h-20 2xl:w-28 2xl:h-28 rounded-xl overflow-hidden bg-black/50 border border-white/8 hover:border-neon-red/50"
          role="listitem" title={j.prompt} onclick={() => open(j)}
          animate:flip={{ duration: PAGE_MS, easing: EASE }} in:scale={{ start: 0.8, duration: PAGE_MS, easing: EASE }}>
          {#if j.status === 'done' && o}
            {#if o.mime.startsWith('image')}
              <img src={url(o.url, true)} alt="" loading="lazy" draggable="false" class="w-full h-full object-cover" />
            {:else if o.mime.startsWith('video')}
              <video src={url(o.url, true) + '#t=0.1'} muted preload="metadata" class="w-full h-full object-cover pointer-events-none"></video>
            {:else}
              <AudioLines size={24} class="m-auto text-neutral-400" />
            {/if}
          {:else if j.status === 'error' || j.status === 'cancelled'}
            <TriangleAlert size={20} class="m-auto text-neon-red/70" />
          {:else}
            <!-- Progress ring while it renders -->
            <svg viewBox="0 0 36 36" class="absolute inset-0 m-auto w-11 h-11 -rotate-90">
              <circle cx="18" cy="18" r="15" fill="none" stroke="rgb(255 255 255 / 10%)" stroke-width="3" />
              <circle cx="18" cy="18" r="15" fill="none" stroke="#ff3344" stroke-width="3" stroke-linecap="round"
                stroke-dasharray={2 * Math.PI * 15} stroke-dashoffset={2 * Math.PI * 15 * (1 - pct(j.progress))} class="transition-[stroke-dashoffset] duration-500" />
            </svg>
          {/if}
        </button>
      {/each}
    </div>
    {#if canLeft}
      <button class="arrow left-1" onclick={() => page(-1)} aria-label="Scroll left" transition:scale={{ duration: 150 }}><ChevronLeft size={16} /></button>
    {/if}
    {#if canRight}
      <button class="arrow right-1" onclick={() => page(1)} aria-label="Scroll right" transition:scale={{ duration: 150 }}><ChevronRight size={16} /></button>
    {/if}
  </div>
{/if}

<style>
  .strip {
    cursor: grab;
    /* Fade the edges so it's clear there's more to see */
    mask-image: linear-gradient(90deg, transparent, #000 1.25rem, #000 calc(100% - 1.25rem), transparent);
  }

  .strip:active {
    cursor: grabbing;
  }

  .thumb {
    transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1), border-color 0.3s;
  }

  .thumb:hover {
    transform: translateY(-2px) scale(1.04);
  }

  .arrow {
    position: absolute;
    top: 50%;
    translate: 0 -50%;
    padding: 0.45rem;
    border-radius: 999px;
    background: rgb(0 0 0 / 70%);
    border: 1px solid rgb(255 255 255 / 12%);
    backdrop-filter: blur(10px);
  }

  .arrow:hover {
    border-color: rgb(255 51 68 / 60%);
    color: #fff;
  }
</style>
