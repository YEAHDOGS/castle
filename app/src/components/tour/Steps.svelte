<!-- Steps: a section's tutorial as a stepper. One pop-up at a time, pointing at a real part
     of the interface (or sitting in the middle when it is about the whole thing). Back and
     Next sit at mid-height on the left and right edges of the window, always in the same
     place, so stepping never needs aiming; Skip tutorial sits top right, beside the wheel.
     The left and right arrow keys step too, Escape skips, and after the last step Next is a
     check mark that ends it. The page behind is blurred and dimmed a little, with the step's
     target cut out of the blur and bracketed, so the tutorial is what you look at; the page
     still works underneath. Everything drawn here is one colour nothing else in Castle uses
     (--color-tour, app.css). The target is measured again every so often, so the pop-up
     follows a layout that changes, and the whole thing steps aside while the wheel, the
     people menu or any dialog is open. Words: steps.js. Where the pop-up goes: place.js. -->
<script>
  import { untrack } from 'svelte';
  import { fade, fly, scale } from 'svelte/transition';
  import { ArrowLeft, ArrowRight, Check } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { SHOWCASE } from '../../lib/showcase.js';
  import { modalOpen } from '../../lib/input.svelte.js';
  import { reducedMotion, DUR, modalIn, pop, viewSwap } from '../../lib/motion.js';
  import { tour } from './tour.svelte.js';
  import { place, between, under, centred, onScreen } from './place.js';

  const SETTLE_MS = DUR.MOMENT + 140; // a new section fades and settles in (App.svelte) before anything is measured
  const SETTLE_STILL_MS = 60; // the same wait when motion is reduced and nothing fades
  const RECHECK_MS = 300; // how often the target is measured again
  const RING_PAD = 6; // room between a target and the brackets around it
  const RING_INSET = 2; // the brackets stay this far inside the window
  const KEEP_CLEAR = 8; // room the pop-up keeps from the tour's own buttons
  const DEFAULT_SIDE = 'bottom';
  const DEFAULT_AT = 0.5;
  const COUNT_DIGITS = 2;
  const TYPING = 'input, textarea, select, [contenteditable="true"]'; // where the arrow keys and Escape belong to the field
  const LABEL = 'Tutorial';
  const BACK = 'Back';
  const NEXT = 'Next';
  const DONE = 'Done';
  const SKIP = 'Skip tutorial';
  // The tail, by the side of the target the pop-up ended up on: one below its target points up.
  const TAILS = {
    bottom: { dir: 'up', w: 22, h: 14, d: 'M3 14 Q 9 9 11 0 Q 12 9 19 14' },
    top: { dir: 'down', w: 22, h: 14, d: 'M3 0 Q 9 5 11 14 Q 12 5 19 0' },
    left: { dir: 'right', w: 14, h: 22, d: 'M0 3 Q 5 9 14 11 Q 5 12 0 19' },
    right: { dir: 'left', w: 14, h: 22, d: 'M14 3 Q 9 9 0 11 Q 9 12 14 19' },
  };

  const steps = $derived(tour.steps());
  const step = $derived(steps[Math.min(tour.at, steps.length - 1)]);
  const last = $derived(tour.at >= steps.length - 1);
  const text = $derived((SHOWCASE || store.isDemo) && step?.demo ? `${step.text} ${step.demo}` : step?.text);

  let ready = $state(false); // the section has settled in
  let covered = $state(false); // a dialog is open over the page
  let glide = $state(false); // the pop-up has a place, and moves to its next one rather than jumping
  /** @type {{ left: number, top: number, right: number, bottom: number, width: number, height: number } | null} the target's box, when it is on screen */
  let rect = $state.raw(null);
  /** The room there is: the window, minus the showcase strip at the bottom. */
  let bounds = $state.raw({ w: 0, h: 0 });
  /** @type {{ arrows: { left: number, top: number, w: number, h: number }[], skip: { left: number, top: number, w: number, h: number } } | null} where the tour's own buttons are, with room around them */
  let buttons = $state.raw(null);
  let w = $state(0);
  let h = $state(0);
  let backEl = $state();
  let nextEl = $state();
  let skipEl = $state();

  const shown = $derived(ready && !!step && !covered && !shell.wheelOpen && !shell.userMenuOpen);
  /** Where the pop-up goes: beside its target, clear of the buttons, or the middle when it has no target in view. */
  const pos = $derived.by(() => {
    const size = { w, h };
    if (!buttons || !w) return centred(size, bounds);
    if (!rect) return centred(size, bounds, [buttons.skip]);
    const [back, next] = buttons.arrows;
    const placed = place(rect, size, step.side ?? DEFAULT_SIDE, step.at ?? DEFAULT_AT, bounds, [...buttons.arrows, buttons.skip]);
    return between(under(placed, size, buttons.skip, bounds), size, buttons.arrows, { left: back.left + back.w, right: next.left });
  });
  const tail = $derived(TAILS[pos.side]);
  /** The brackets around the target, and the hole in the blur: the target's box with a little room, inside the window. */
  const ring = $derived.by(() => {
    if (!rect) return null;
    const left = Math.max(RING_INSET, rect.left - RING_PAD);
    const top = Math.max(RING_INSET, rect.top - RING_PAD);
    return { left, top, right: Math.min(bounds.w - RING_INSET, rect.right + RING_PAD), bottom: Math.min(bounds.h - RING_INSET, rect.bottom + RING_PAD) };
  });
  // The blur covers the window; drawn as one shape with the target's box left out of it.
  const hole = $derived(
    ring
      ? `polygon(evenodd, 0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${ring.left}px ${ring.top}px, ${ring.right}px ${ring.top}px, ${ring.right}px ${ring.bottom}px, ${ring.left}px ${ring.bottom}px, ${ring.left}px ${ring.top}px)`
      : 'none',
  );

  const find = (name) => (name ? document.querySelector(`[data-tour="${name}"]`) : null);
  const changed = (a, b) => JSON.stringify(a) !== JSON.stringify(b);
  const count = (n) => String(n).padStart(COUNT_DIGITS, '0');
  /** A fixed button's box with room around it. From offsets, so a transition scaling it does not count. */
  const clearOf = (el) => ({ left: el.offsetLeft - KEEP_CLEAR, top: el.offsetTop - KEEP_CLEAR, w: el.offsetWidth + KEEP_CLEAR * 2, h: el.offsetHeight + KEEP_CLEAR * 2 });

  function measure() {
    if (!step) return;
    const strip = find('strip');
    const room = { w: window.innerWidth, h: strip ? strip.getBoundingClientRect().top : window.innerHeight };
    let found = null;
    for (const name of [step.target, step.or]) {
      const box = find(name)?.getBoundingClientRect();
      if (!onScreen(box, room)) continue;
      found = { left: box.left, top: box.top, right: box.right, bottom: box.bottom, width: box.width, height: box.height };
      break;
    }
    const boxes = backEl && nextEl && skipEl ? { arrows: [clearOf(backEl), clearOf(nextEl)], skip: clearOf(skipEl) } : null;
    covered = modalOpen();
    if (changed(room, bounds)) bounds = room;
    if (changed(found, rect)) rect = found;
    if (boxes && changed(boxes, buttons)) buttons = boxes;
  }

  function onKey(e) {
    if (!shown || e.ctrlKey || e.metaKey || e.altKey || modalOpen() || e.target?.closest?.(TYPING)) return;
    if (e.key === 'ArrowRight') tour.next();
    else if (e.key === 'ArrowLeft') tour.back();
    else if (e.key === 'Escape') tour.finish();
    else return;
    // The key was the tour's: no section and no focus move also acts on it.
    e.preventDefault();
    e.stopPropagation();
  }

  // A new section: let it settle, then look at what is there.
  $effect(() => {
    tour.section;
    ready = false;
    const timer = setTimeout(() => {
      measure();
      ready = true;
    }, reducedMotion() ? SETTLE_STILL_MS : SETTLE_MS);
    return () => clearTimeout(timer);
  });

  // A new step, or the tour back from behind the wheel: bring its target into view and look at once.
  $effect(() => {
    if (!shown) return;
    const target = step.target;
    untrack(() => {
      find(target)?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: reducedMotion() ? 'auto' : 'smooth' });
      measure();
    });
  });

  // Then keep looking: a list scrolls, the window changes size, a dialog opens or closes.
  $effect(() => {
    if (!ready) return;
    const timer = setInterval(measure, RECHECK_MS);
    window.addEventListener('scroll', measure, { capture: true, passive: true });
    return () => {
      clearInterval(timer);
      window.removeEventListener('scroll', measure, { capture: true });
    };
  });

  // The pop-up's first place is where it appears; from the next frame on, it glides.
  $effect(() => {
    if (!shown || !w) {
      glide = false;
      return;
    }
    const frame = requestAnimationFrame(() => (glide = true));
    return () => cancelAnimationFrame(frame);
  });
</script>

<svelte:window onkeydowncapture={onKey} onresize={measure} />

{#if shown}
  <div class="tour" style:--clear="{KEEP_CLEAR}px">
    <div class="veil" aria-hidden="true" style:clip-path={hole} transition:fade|global={{ duration: reducedMotion() ? 0 : DUR.UI }}></div>

    {#if ring}
      {#key step.id}
        <div class="ring hud-corners" aria-hidden="true" style="left: {ring.left}px; top: {ring.top}px; width: {ring.right - ring.left}px; height: {ring.bottom - ring.top}px"></div>
      {/key}
    {/if}

    <section
      class="pop"
      class:placed={w > 0}
      class:glide
      bind:offsetWidth={w}
      bind:offsetHeight={h}
      style:transform="translate({Math.round(pos.left)}px, {Math.round(pos.top)}px)"
      style:--tx="{pos.tx}px"
      style:--ty="{pos.ty}px"
      aria-label={LABEL}
      aria-live="polite"
    >
      <div class="bubble {tail?.dir ?? ''}" in:scale|global={modalIn()} out:scale|global={pop()}>
        {#if tail}
          <svg class="tail" width={tail.w} height={tail.h} aria-hidden="true"><path d={tail.d} /></svg>
        {/if}
        {#key step.id}
          <div in:fly={viewSwap(tour.dir)}>
            <p class="micro count">{count(tour.at + 1)} / {count(steps.length)}</p>
            <h2 class="title">{step.title}</h2>
            <p class="text">{text}</p>
          </div>
        {/key}
      </div>
    </section>

    <button bind:this={skipEl} class="micro skip" onclick={() => tour.finish()} transition:scale|global={pop()} data-nav-skip>{SKIP}</button>

    <button bind:this={backEl} class="arrow back" onclick={() => tour.back()} disabled={tour.at === 0} aria-label={BACK} transition:scale|global={pop()} data-nav-skip>
      <ArrowLeft class="h-6 w-6" strokeWidth={2.5} />
    </button>
    <button bind:this={nextEl} class="arrow next" onclick={() => tour.next()} aria-label={last ? DONE : NEXT} transition:scale|global={pop()} data-nav-skip>
      {#if last}
        <Check class="h-6 w-6" strokeWidth={2.5} />
      {:else}
        <ArrowRight class="h-6 w-6" strokeWidth={2.5} />
      {/if}
    </button>
  </div>
{/if}

<style>
  /* Not a box of its own: only where the tour's sizes live. */
  .tour {
    display: contents;
    --arrow: 3.5rem; /* Back and Next: big enough to hit without aiming */
    --edge: 0.75rem; /* how far they sit from the window's sides */
  }
  @media (max-width: 480px), (max-height: 640px) {
    .tour {
      --arrow: 2.75rem;
      --edge: 0.5rem;
    }
  }

  /* ---- the page behind: blurred and dimmed a little, everywhere but the target ---- */
  .veil {
    position: fixed;
    inset: 0;
    z-index: 46;
    background: color-mix(in srgb, var(--color-void) 30%, transparent);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    pointer-events: none; /* the page still works underneath */
  }

  /* ---- the brackets around what the step is about: they close in on it ---- */
  .ring {
    --corner: var(--color-tour);
    --cs: 12px;
    position: fixed;
    z-index: 47;
    border: 1px solid color-mix(in srgb, var(--color-tour) 45%, transparent);
    pointer-events: none;
    animation: ring-in 320ms var(--ease-expo) both;
  }
  @keyframes ring-in {
    from { opacity: 0; transform: scale(1.2); }
  }

  /* ---- the pop-up: never wider than the room between Back and Next ---- */
  .pop {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 50;
    width: max-content;
    max-width: min(20rem, calc(100vw - 2 * (var(--edge) + var(--arrow) + var(--clear))));
    visibility: hidden; /* until measured: place.js needs the size before the pop-up has a place */
  }
  .pop.placed { visibility: visible; }
  .pop.glide { transition: transform 320ms var(--ease-expo); }

  .bubble {
    position: relative;
    padding: 12px 16px 14px;
    background: var(--color-tour);
    /* two sets of radii, so no two corners match: a line drawn by hand */
    border-radius: 18px 22px 16px 24px / 22px 16px 24px 18px;
    box-shadow: 4px 4px 0 color-mix(in srgb, var(--color-tour) 35%, transparent), 0 12px 32px color-mix(in srgb, var(--color-abyss) 65%, transparent);
    color: var(--color-void);
    user-select: none;
  }
  /* it grows from its tail */
  .bubble.up { transform-origin: var(--tx) 0; }
  .bubble.down { transform-origin: var(--tx) 100%; }
  .bubble.right { transform-origin: 100% var(--ty); }
  .bubble.left { transform-origin: 0 var(--ty); }

  .count {
    margin: 0;
    font-weight: 700;
  }
  .title {
    margin: 4px 0 0;
    font-size: 18px;
    font-weight: 900;
    line-height: 1.1;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .text {
    margin: 6px 0 0;
    font-size: 13px;
    font-weight: 500;
    line-height: 1.45;
  }

  /* ---- the tail: the pop-up's colour, its base just inside the pop-up's edge ---- */
  .tail {
    position: absolute;
    overflow: visible;
    fill: var(--color-tour);
  }
  .up .tail { left: var(--tx); top: 0; transform: translate(-50%, calc(-100% + 1px)); }
  .down .tail { left: var(--tx); bottom: 0; transform: translate(-50%, calc(100% - 1px)); }
  .right .tail { top: var(--ty); right: 0; transform: translate(calc(100% - 1px), -50%); }
  .left .tail { top: var(--ty); left: 0; transform: translate(calc(-100% + 1px), -50%); }

  /* ---- the tour's buttons: they never move ---- */
  .skip,
  .arrow {
    position: fixed;
    z-index: 51;
    background: var(--color-tour);
    color: var(--color-void);
    box-shadow: 0 6px 24px color-mix(in srgb, var(--color-abyss) 70%, transparent);
  }
  .skip:hover,
  .arrow:hover:not(:disabled) { filter: brightness(1.15); }

  /* Top right, beside the wheel's button (WheelMenu), which stays where it is. */
  .skip {
    top: 0.75rem;
    right: 4rem;
    height: 2.5rem;
    padding: 0 0.85rem;
    font-weight: 700;
  }
  .skip:active { transform: scale(0.96); }

  .arrow {
    top: calc((100dvh - var(--strip)) / 2 - var(--arrow) / 2);
    display: grid;
    place-items: center;
    width: var(--arrow);
    height: var(--arrow);
    border-radius: 50%;
  }
  .back { left: var(--edge); }
  .next { right: var(--edge); }
  .arrow:hover:not(:disabled) { transform: scale(1.08); }
  .arrow:active:not(:disabled) { transform: scale(0.94); }
  .arrow:disabled { opacity: 0.4; }
</style>
