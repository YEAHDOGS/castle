<!-- Carousel: the big cards of a first visit, one at a time, in a bubble docked low on the
     screen with the app still showing behind it. What Castle is; that this is sample data
     and nothing is kept; how the wheel works. Dots, Skip, Next; a finger swipes between them
     and the arrow keys step. Not slides: there is no deck and nothing goes full screen. -->
<script>
  import { fly, scale } from 'svelte/transition';
  import { ArrowRight } from '@lucide/svelte';
  import { tour } from './tour.svelte.js';
  import { swipe } from '../../lib/swipe.js';
  import { reducedMotion, DUR, expoOut, modalIn, pop } from '../../lib/motion.js';
  import { store } from '../../lib/store.svelte.js';
  import { SHOWCASE } from '../../lib/showcase.js';

  // tail: where the bubble's tail points. The first card speaks from the box (down-left, where
  // the DOGS head sits at the foot of the rail), the second from the strip below, the third
  // towards the wheel in the corner.
  const CARDS = [
    { id: 'castle', kicker: 'WHAT THIS IS', title: 'This is Castle', text: 'A home server you own. Your files, photos, messages, games and films, on a box in your house. No cloud and no monthly bill.', tail: 'down-left' },
    { id: 'sample', kicker: 'WHAT YOU ARE LOOKING AT', title: 'Sample data', text: 'Everything here is made up, and nothing you do is kept. Click anything. Nothing can break.', tail: 'down', demoOnly: true },
    { id: 'wheel', kicker: 'HOW TO GET AROUND', title: 'Turn the wheel', text: 'Scroll over the button in the top-right corner, or click it, and the wheel of sections turns. The one under the beam opens.', tail: 'up-right' },
  ];

  let at = $state(0);
  let dir = $state(1); // which way the last step went, so the card flies in from that side
  let card = $state(null);

  const cards = $derived(CARDS.filter((c) => !c.demoOnly || SHOWCASE || store.isDemo));
  const current = $derived(cards[Math.min(at, cards.length - 1)]);
  const last = $derived(at >= cards.length - 1);

  const slideIn = (d) => (reducedMotion() ? { duration: 0 } : { duration: DUR.UI, easing: expoOut, x: 36 * d, opacity: 0 });

  function go(i) {
    if (i < 0 || i >= cards.length || i === at) return;
    dir = i > at ? 1 : -1;
    at = i;
  }

  function next() {
    if (last) tour.endCarousel();
    else go(at + 1);
  }

  function onKey(e) {
    if (e.key === 'ArrowRight') go(at + 1);
    else if (e.key === 'ArrowLeft') go(at - 1);
    else if (e.key === 'Escape') tour.endCarousel();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  // The arrows work straight away, and a screen reader lands on the first card.
  $effect(() => {
    requestAnimationFrame(() => card?.focus());
  });
</script>

<div class="dock fixed left-1/2 z-[62] -translate-x-1/2">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions: the region takes focus (tabindex) so the arrows step the cards -->
  <section
    bind:this={card}
    class="card {current.tail} outline-none"
    tabindex="-1"
    aria-roledescription="carousel"
    aria-label="A short tour of Castle"
    use:swipe={(d) => go(at + d)}
    onkeydown={onKey}
    in:scale={modalIn()}
    out:scale={pop()}
  >
    <svg class="tail" width="36" height="28" aria-hidden="true">
      {#if current.tail === 'down'}
        <path d="M5 0 Q 15 10 18 28 Q 21 10 31 0" />
      {:else}
        <path d="M7 0 Q 14 12 2 28 Q 20 14 32 0" />
      {/if}
    </svg>

    <div class="body">
      {#key current.id}
        <div in:fly={slideIn(dir)} aria-live="polite">
          <p class="micro kicker">
            <span class="text-chiral">{String(at + 1).padStart(2, '0')}</span><span class="text-text-faint"> / {String(cards.length).padStart(2, '0')} · {current.kicker}</span>
          </p>
          <h2 class="title">{current.title}</h2>
          <p class="text">{current.text}</p>
        </div>
      {/key}
    </div>

    <div class="controls">
      <button class="micro expo t-hover skip" onclick={() => tour.endCarousel()}>Skip</button>
      <div class="dots" role="tablist" aria-label="Cards">
        {#each cards as c, i (c.id)}
          <button class="dot" class:on={i === at} role="tab" aria-selected={i === at} aria-label={`Card ${i + 1} of ${cards.length}`} onclick={() => go(i)}></button>
        {/each}
      </div>
      <button class="micro expo t-hover next" onclick={next}>
        {last ? 'Go' : 'Next'}<ArrowRight class="h-3.5 w-3.5" strokeWidth={2.5} />
      </button>
    </div>
  </section>
</div>

<style>
  /* Above the showcase strip and, on a phone, above Storage's row of tabs. */
  .dock {
    bottom: calc(var(--strip) + 5.25rem);
    width: min(540px, calc(100vw - 1.5rem));
  }
  @media (min-width: 768px) {
    .dock { bottom: calc(var(--strip) + 3rem); }
  }

  .card {
    position: relative;
    padding: 18px 20px 14px;
    background: rgba(5, 5, 8, 0.95);
    border: 2px solid var(--color-chiral);
    border-radius: 26px 34px 22px 38px / 34px 22px 38px 26px;
    box-shadow: 6px 6px 0 color-mix(in srgb, var(--color-chiral) 35%, transparent), 0 24px 60px rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    color: #fff;
  }
  @media (min-width: 768px) {
    .card { padding: 22px 26px 18px; }
  }

  .body { min-height: 7.25rem; }
  .kicker { margin: 0; }
  .title {
    margin: 6px 0 0;
    font-size: clamp(22px, 5vw, 30px);
    font-weight: 900;
    line-height: 1;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .text {
    margin: 10px 0 0;
    font-size: 14px;
    line-height: 1.5;
    color: var(--color-text-dim);
  }
  @media (min-width: 768px) {
    .text { font-size: 15px; }
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 14px;
  }
  .skip {
    padding: 8px 6px;
    color: var(--color-text-faint);
  }
  .skip:hover { color: #fff; }
  .dots {
    display: flex;
    flex: 1;
    justify-content: center;
    gap: 8px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1px solid var(--color-text-faint);
    transition: transform 150ms var(--ease-expo), background-color 150ms var(--ease-expo), border-color 150ms var(--ease-expo);
  }
  .dot.on {
    background: var(--color-chiral);
    border-color: var(--color-chiral);
    transform: scale(1.3);
    box-shadow: 0 0 12px color-mix(in srgb, var(--color-chiral) 60%, transparent);
  }
  .next {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 36px;
    padding: 0 14px;
    background: var(--color-chiral);
    color: #000;
    font-weight: 700;
  }
  .next:hover { filter: brightness(1.1); }
  .next:active { transform: scale(0.97); }

  /* ---- the tail, in the bubble's colours, its base over the bubble's own line ---- */
  .tail {
    position: absolute;
    overflow: visible;
    fill: rgba(5, 5, 8, 0.98);
    stroke: var(--color-chiral);
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
    transition: left 250ms var(--ease-expo), right 250ms var(--ease-expo);
  }
  .down .tail,
  .down-left .tail { bottom: 0; left: 16%; transform: translateY(calc(100% - 2px)); }
  .down-left .tail { left: 9%; }
  .up-right .tail { top: 0; right: 9%; transform: translateY(calc(-100% + 2px)) scale(-1, -1); }
</style>
