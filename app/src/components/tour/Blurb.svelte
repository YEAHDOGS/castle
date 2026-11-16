<!-- Blurb: one comic-book bubble, pointing at a part of the interface. A black panel with a
     chiral line that is not quite a rectangle, a tail (or, for a thought, three little
     circles) from its edge to the thing it talks about, one or two sentences, and an X.
     Where it goes is Blurbs.svelte's call (through place.js), once this bubble has reported
     its size. It pops in from its tail, each one a beat after the last. -->
<script>
  import { untrack } from 'svelte';
  import { X } from '@lucide/svelte';
  import { scale } from 'svelte/transition';
  import { pop, reducedMotion } from '../../lib/motion.js';

  /**
   * blurb: an entry of blurbs.js. pos: where it goes, from place.js. index: its place in the
   * stagger (and which way it tilts). onsize(w, h): its size, whenever that changes.
   * onclose(): the X. onskip(): the "skip the tour" line.
   */
  let { blurb, pos, index = 0, onsize, onclose, onskip } = $props();

  const FIRST_MS = 220; // before the first bubble
  const STAGGER_MS = 260; // between one bubble and the next
  const TAIL_DIR = { bottom: 'up', top: 'down', left: 'right', right: 'left' }; // a bubble below its target points up

  let w = $state(0);
  let h = $state(0);

  const tail = $derived(TAIL_DIR[pos.side]);
  const delay = $derived(reducedMotion() ? 0 : FIRST_MS + index * STAGGER_MS);
  const tilt = $derived(index % 2 ? 0.7 : -0.7);

  // Only the size is watched: the callback itself changes whenever the parent places again.
  $effect(() => {
    const size = [w, h];
    untrack(() => onsize(...size));
  });
</script>

<div
  class="blurb {tail}"
  class:thought={blurb.kind === 'thought'}
  class:ready={w > 0}
  bind:clientWidth={w}
  bind:clientHeight={h}
  style="left: {pos.left}px; top: {pos.top}px; --tx: {pos.tx}px; --ty: {pos.ty}px; --delay: {delay}ms; --tilt: {tilt}deg"
  out:scale={pop()}
  role="note"
>
  {#if blurb.kind === 'thought'}
    <!-- three circles, the biggest nearest the bubble -->
    <svg class="tail" width={tail === 'up' || tail === 'down' ? 22 : 24} height={tail === 'up' || tail === 'down' ? 24 : 22} aria-hidden="true">
      {#if tail === 'up'}
        <circle cx="11" cy="19" r="4.5" /><circle cx="12" cy="10" r="3" /><circle cx="11" cy="3" r="1.8" />
      {:else if tail === 'down'}
        <circle cx="11" cy="5" r="4.5" /><circle cx="10" cy="14" r="3" /><circle cx="11" cy="21" r="1.8" />
      {:else if tail === 'right'}
        <circle cx="5" cy="11" r="4.5" /><circle cx="14" cy="10" r="3" /><circle cx="21" cy="11" r="1.8" />
      {:else}
        <circle cx="19" cy="11" r="4.5" /><circle cx="10" cy="12" r="3" /><circle cx="3" cy="11" r="1.8" />
      {/if}
    </svg>
  {:else}
    <svg class="tail" width={tail === 'up' || tail === 'down' ? 22 : 14} height={tail === 'up' || tail === 'down' ? 14 : 22} aria-hidden="true">
      {#if tail === 'up'}
        <path d="M3 14 Q 9 9 11 0 Q 12 9 19 14" />
      {:else if tail === 'down'}
        <path d="M3 0 Q 9 5 11 14 Q 12 5 19 0" />
      {:else if tail === 'right'}
        <path d="M0 3 Q 5 9 14 11 Q 5 12 0 19" />
      {:else}
        <path d="M14 3 Q 9 9 0 11 Q 9 12 14 19" />
      {/if}
    </svg>
  {/if}

  <p class="text">{blurb.text}</p>
  <button class="micro skip" onclick={onskip} data-nav-skip>Skip the tour</button>
  <button class="close" onclick={onclose} aria-label="Put this away" data-nav-skip>
    <X class="h-3 w-3" strokeWidth={2.5} />
  </button>
</div>

<style>
  .blurb {
    position: fixed;
    z-index: 50;
    width: max-content;
    max-width: min(252px, calc(100vw - 16px));
    padding: 10px 30px 8px 14px;
    background: rgba(5, 5, 8, 0.94);
    border: 2px solid var(--color-chiral);
    /* two sets of radii, so no two corners match: a line drawn by hand */
    border-radius: 18px 22px 16px 24px / 22px 16px 24px 18px;
    box-shadow: 4px 4px 0 color-mix(in srgb, var(--color-chiral) 35%, transparent), 0 12px 32px rgba(0, 0, 0, 0.65);
    color: #fff;
    font-size: 13px;
    line-height: 1.45;
    transform: rotate(var(--tilt));
    animation: blurb-in 460ms cubic-bezier(0.34, 1.56, 0.64, 1) var(--delay) both;
    visibility: hidden; /* until measured: place.js needs the size before the bubble has a place */
  }
  .blurb.ready { visibility: visible; }
  @keyframes blurb-in {
    from { opacity: 0; transform: rotate(var(--tilt)) scale(0.3); }
  }
  /* it grows from its tail */
  .blurb.up { transform-origin: var(--tx) 0; }
  .blurb.down { transform-origin: var(--tx) 100%; }
  .blurb.right { transform-origin: 100% var(--ty); }
  .blurb.left { transform-origin: 0 var(--ty); }

  .blurb.thought {
    border-style: dashed;
    border-radius: 26px 30px 24px 32px / 30px 24px 32px 26px;
    color: var(--color-text-dim);
  }

  .text { margin: 0; }
  .text::first-letter { color: var(--color-chiral); font-weight: 700; }

  .skip {
    display: block;
    margin: 6px 0 0 -2px;
    padding: 2px;
    font-size: 9px;
    color: var(--color-text-faint);
  }
  .skip:hover { color: var(--color-chiral); }

  .close {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    color: var(--color-text-faint);
  }
  .close:hover { color: #fff; background: rgba(255, 255, 255, 0.08); }

  /* ---- the tail: drawn in the bubble's colours, its base over the bubble's own line ---- */
  .tail {
    position: absolute;
    overflow: visible;
    fill: rgba(5, 5, 8, 0.98);
    stroke: var(--color-chiral);
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  .up .tail { left: var(--tx); top: 0; transform: translate(-50%, calc(-100% + 2px)); }
  .down .tail { left: var(--tx); bottom: 0; transform: translate(-50%, calc(100% - 2px)); }
  .right .tail { top: var(--ty); right: 0; transform: translate(calc(100% - 2px), -50%); }
  .left .tail { top: var(--ty); left: 0; transform: translate(calc(-100% + 2px), -50%); }
  /* a thought's circles float clear of the bubble */
  .thought.up .tail { transform: translate(-50%, calc(-100% - 3px)); }
  .thought.down .tail { transform: translate(-50%, calc(100% + 3px)); }
  .thought.right .tail { transform: translate(calc(100% + 3px), -50%); }
  .thought.left .tail { transform: translate(calc(-100% - 3px), -50%); }
</style>
