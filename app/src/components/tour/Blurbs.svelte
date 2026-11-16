<!-- Blurbs: the layer of speech bubbles scattered over the interface (blurbs.js says what
     and where). Only the bubbles for the section showing appear, and only those whose target
     is on screen: the rail's bubble waits until there is a rail. Their targets are measured
     again every so often, so a bubble follows a tile that scrolls and lets go of one that
     leaves. Each bubble is placed clear of the ones before it (place.js), and a phone shows
     only a few at a time, the next arriving as one is put away. While the wheel or the
     people menu is open, the bubbles step aside. -->
<script>
  import { shell } from '../../lib/shell.svelte.js';
  import { reducedMotion, DUR } from '../../lib/motion.js';
  import { tour } from './tour.svelte.js';
  import { BLURBS } from './blurbs.js';
  import { SvelteMap } from 'svelte/reactivity';
  import { place, onScreen } from './place.js';
  import Blurb from './Blurb.svelte';

  const SETTLE_MS = DUR.MOMENT + 140; // a new section fades and settles in (App.svelte) before anything is measured
  const RECHECK_MS = 400; // how often the targets are measured again
  const NARROW_PX = 768; // under this width (a phone), only a few bubbles show at once
  const NARROW_MAX = 3;

  let ready = $state(false);
  let shown = $state([]); // [{ blurb, rect }]: the blurbs whose target is on screen right now
  let bounds = $state({ w: 0, h: 0 });
  const sizes = new SvelteMap(); // blurb id -> { w, h }, reported by each bubble once drawn

  /** A bubble's size, kept only when it changed, or placing would never settle. */
  function sized(id, w, h) {
    const was = sizes.get(id);
    if (was && was.w === w && was.h === h) return;
    sizes.set(id, { w, h });
  }

  const hidden = $derived(shell.wheelOpen || shell.userMenuOpen);
  // A bubble just put away leaves at once, before the next measurement.
  const visible = $derived.by(() => {
    const live = shown.filter((s) => !tour.dismissed.has(s.blurb.id));
    return bounds.w < NARROW_PX ? live.slice(0, NARROW_MAX) : live;
  });
  // Each bubble is placed after the ones before it, and keeps clear of them.
  const placed = $derived.by(() => {
    const taken = [];
    return visible.map((item) => {
      const size = sizes.get(item.blurb.id) || { w: 0, h: 0 };
      const pos = place(item.rect, size, item.blurb.side, item.blurb.at ?? 0.5, bounds, taken);
      if (size.w) taken.push({ left: pos.left, top: pos.top, w: size.w, h: size.h });
      return { ...item, pos };
    });
  });

  const find = (name) => (name ? document.querySelector(`[data-tour="${name}"]`) : null);
  const box = (r) => ({ left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height });
  const same = (a, b) => a.length === b.length && a.every((s, i) => s.blurb === b[i].blurb && JSON.stringify(s.rect) === JSON.stringify(b[i].rect));

  function measure() {
    const strip = find('strip');
    // Bubbles are placed above the strip; a target is looked for anywhere in the window (the strip itself is one).
    const screen = { w: window.innerWidth, h: window.innerHeight };
    const next = { w: screen.w, h: strip ? strip.getBoundingClientRect().top : screen.h };
    const list = [];
    for (const blurb of BLURBS) {
      if (tour.dismissed.has(blurb.id)) continue;
      if (blurb.section && blurb.section !== shell.section) continue;
      for (const name of [blurb.target, blurb.or]) {
        const rect = find(name)?.getBoundingClientRect();
        if (!onScreen(rect, screen)) continue;
        list.push({ blurb, rect: box(rect) });
        break;
      }
    }
    if (next.w !== bounds.w || next.h !== bounds.h) bounds = next;
    if (!same(list, shown)) shown = list;
  }

  // A new section: let it settle, then look at what is there.
  $effect(() => {
    shell.section;
    ready = false;
    const timer = setTimeout(() => {
      ready = true;
      measure();
    }, reducedMotion() ? 60 : SETTLE_MS);
    return () => clearTimeout(timer);
  });

  // Then keep looking: a list scrolls, the window changes size, a view changes.
  $effect(() => {
    if (!ready) return;
    const timer = setInterval(measure, RECHECK_MS);
    window.addEventListener('resize', measure);
    window.addEventListener('scroll', measure, { capture: true, passive: true });
    return () => {
      clearInterval(timer);
      window.removeEventListener('resize', measure);
      window.removeEventListener('scroll', measure, { capture: true });
    };
  });
</script>

{#if ready && !hidden}
  {#each placed as item, i (item.blurb.id)}
    <Blurb
      blurb={item.blurb}
      pos={item.pos}
      index={i}
      onsize={(w, h) => sized(item.blurb.id, w, h)}
      onclose={() => tour.dismiss(item.blurb.id)}
      onskip={() => tour.dismissAll()}
    />
  {/each}
{/if}
