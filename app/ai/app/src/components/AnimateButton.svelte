<script>
  // One click turns an image into a video (the server picks the model, size and prompt). The caret opens an
  // optional motion prompt, and a link to Create → Video for every setting.
  import { Film, ChevronDown, LoaderCircle, SlidersHorizontal } from '@lucide/svelte'
  import { scale } from 'svelte/transition'
  import { store } from '../lib/store.svelte.js'
  import { errors } from '../lib/errors.svelte.js'
  import { EASE } from '../lib/motion.js'

  /** @type {{ file: string, url: string, label?: boolean, onQueued?: () => void }} */
  let { file, url, label = false, onQueued } = $props()

  const POPOVER_W = 288
  const POPOVER_H = 200
  const GAP = 6
  const EDGE = 8
  const MOTION_ROWS = 3

  let busy = $state(false)
  let open = $state(false)
  let motion = $state('')
  let pos = $state({ top: 0, left: 0 })
  /** @type {HTMLDivElement} */ let group
  /** @type {HTMLDivElement | undefined} */ let pop = $state()

  const running = $derived((store.animations.get(file) || []).some((j) => j.status === 'queued' || j.status === 'running'))

  async function go() {
    if (busy) return
    busy = true
    try {
      await store.animate(file, motion)
      open = false
      motion = ''
      onQueued?.()
    } catch (e) {
      errors.report(e)
    } finally {
      busy = false
    }
  }

  // Fixed position, under the button (or above it near the bottom of the window).
  function toggle() {
    if (!open) {
      const r = group.getBoundingClientRect()
      const left = Math.min(Math.max(EDGE, r.left), innerWidth - POPOVER_W - EDGE)
      const below = r.bottom + GAP + POPOVER_H < innerHeight
      pos = { top: below ? r.bottom + GAP : r.top - GAP - POPOVER_H, left }
    }
    open = !open
  }

  // Rows animate with transforms, which would make `position: fixed` relative to the row: render on <body>.
  // Moving the node defeats `autofocus`, so focus the motion box here.
  const portal = (/** @type {HTMLElement} */ node) => {
    document.body.appendChild(node)
    node.querySelector('textarea')?.focus()
    return () => node.remove()
  }

  function outside(/** @type {PointerEvent} */ e) {
    if (open && !pop?.contains(/** @type {Node} */ (e.target)) && !group.contains(/** @type {Node} */ (e.target))) open = false
  }

  const onKey = (/** @type {KeyboardEvent} */ e) => {
    if (e.key === 'Escape') open = false
    else if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); go() }
  }

  const BTN = 'flex items-center gap-1 py-1.5 text-neutral-300 hover:text-white hover:bg-white/10 transition-colors disabled:opacity-50'
</script>

<svelte:window onpointerdown={outside} onresize={() => (open = false)} />

<div bind:this={group} class="inline-flex rounded-lg border border-white/10 hover:border-neon-red/40 overflow-hidden text-xs 2xl:text-sm">
  <button class="{BTN} pl-2 {label ? 'pr-2' : 'pr-1.5'}" onclick={go} disabled={busy}
    title="Animate: turn this image into a short video" aria-label="Animate">
    {#if busy}<LoaderCircle size={15} class="animate-spin" />{:else}<Film size={15} class={running ? 'text-neon-red' : ''} />{/if}
    {#if label}<span class="font-semibold">Animate</span>{/if}
  </button>
  <button class="{BTN} px-1 border-l border-white/10" onclick={toggle} aria-expanded={open} title="Describe the motion" aria-label="Animate with a motion prompt">
    <ChevronDown size={13} class="transition-transform {open ? 'rotate-180' : ''}" />
  </button>
</div>

{#if open}
  <div bind:this={pop} {@attach portal} class="fixed z-[60] bg-card rounded-xl p-3 flex flex-col gap-2 shadow-2xl shadow-black border border-white/15"
    style="top: {pos.top}px; left: {pos.left}px; width: {POPOVER_W}px" transition:scale={{ start: 0.95, duration: 180, easing: EASE }}>
    <label class="flex flex-col gap-1">
      <span class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted">Motion (optional)</span>
      <textarea class="well rounded-lg px-2.5 py-2 text-sm outline-none resize-none" rows={MOTION_ROWS} bind:value={motion} onkeydown={onKey}
        placeholder="hair blowing in the wind, slow zoom out"></textarea>
    </label>
    <button class="flex items-center justify-center gap-2 py-2 rounded-lg font-bold text-sm licorice disabled:opacity-40" onclick={go} disabled={busy}>
      {#if busy}<LoaderCircle size={15} class="animate-spin" />{:else}<Film size={15} />{/if} Animate
    </button>
    <button class="flex items-center justify-center gap-1.5 text-[11px] 2xl:text-sm text-muted hover:text-white"
      onclick={() => { open = false; store.preview = null; store.openInCreate({ file, url }) }}>
      <SlidersHorizontal size={12} /> More settings in Create → Video
    </button>
  </div>
{/if}
