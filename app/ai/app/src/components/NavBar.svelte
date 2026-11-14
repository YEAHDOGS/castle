<script>
  import { Sparkles, Library, MessageCircle, Image, Clapperboard, AudioLines } from '@lucide/svelte'
  import { fly, fade } from 'svelte/transition'
  import { store, KINDS, VIEWS } from '../lib/store.svelte.js'
  import { EASE } from '../lib/motion.js'
  import SwapText from './SwapText.svelte'

  const HOLD_MS = 320
  const PICKER_STAGGER_MS = 45
  const BUZZ_MS = 8
  const VIEW_ICONS = { create: Sparkles, chat: MessageCircle, library: Library }
  const KIND_ICONS = { image: Image, video: Clapperboard, audio: AudioLines }
  const TABS = VIEWS.filter((v) => v.id in VIEW_ICONS)

  const index = $derived(Math.max(0, TABS.findIndex((t) => t.id === store.view)))
  const kindLabel = $derived(KINDS.find((k) => k.id === store.kind)?.label || '')

  let pickerOpen = $state(false)
  let hovered = $state('')
  let holdTimer = 0
  let held = false
  let slid = false

  const buzz = () => navigator.vibrate?.(BUZZ_MS)

  function choose(/** @type {string} */ kind) {
    store.setKind(kind)
    store.setView('create')
    pickerOpen = false
    hovered = ''
  }

  // Press: start the hold timer. Hold long enough and the picker slides up under your finger.
  function down(/** @type {PointerEvent} */ e) {
    if (e.button !== 0) return
    held = false
    slid = false
    // Capture keeps move/up events coming to this button while the finger slides over the picker.
    try { /** @type {HTMLElement} */ (e.currentTarget).setPointerCapture(e.pointerId) } catch {}
    holdTimer = setTimeout(() => { held = true; pickerOpen = true; buzz() }, HOLD_MS)
  }

  // While held: whatever kind is under the pointer lights up.
  function move(/** @type {PointerEvent} */ e) {
    if (!pickerOpen || !held) return
    const el = /** @type {HTMLElement | null} */ (document.elementFromPoint(e.clientX, e.clientY)?.closest('[data-kind]'))
    const kind = el?.dataset.kind || ''
    if (kind && kind !== hovered) { buzz(); slid = true }
    hovered = kind
  }

  // Release: on a kind -> pick it. Held but released in place -> leave the picker open to tap.
  // A quick tap -> go to Create (or toggle the picker when already there).
  function up() {
    clearTimeout(holdTimer)
    if (held) {
      if (hovered) choose(hovered)
      else if (slid) pickerOpen = false
      held = false
      return
    }
    if (store.view === 'create') pickerOpen = !pickerOpen
    else store.setView('create')
  }

  function cancel() {
    clearTimeout(holdTimer)
    held = false
  }

  function onKey(/** @type {KeyboardEvent} */ e) {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key]
    if (!step) return
    e.preventDefault()
    store.setView(TABS[(index + step + TABS.length) % TABS.length].id)
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (pickerOpen = false)} />

{#if pickerOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-30 bg-black/40 backdrop-blur-[2px]" transition:fade={{ duration: 180 }} onclick={() => (pickerOpen = false)}></div>
{/if}

<!-- wearedogs-style dock: square glass tiles, centered. Bottom of the screen on phones, a rail from md up.
     A glowing marker glides to the active tile. -->
<nav class="nav order-last md:order-first shrink-0 flex justify-center md:items-start border-t md:border-t-0 md:border-r border-white/5 bg-app/70 backdrop-blur-md px-4 py-2 md:px-3 md:py-5 z-40">
  <div class="dock relative flex md:flex-col" role="tablist" aria-label="Sections" tabindex="-1" onkeydown={onKey} style="--i: {index}">
    <span class="marker" aria-hidden="true"></span>
    {#each TABS as t (t.id)}
      {@const Icon = VIEW_ICONS[t.id]}
      {@const active = store.view === t.id}
      <div class="slot relative flex flex-col items-center">
        {#if t.id === 'create' && pickerOpen}
          <!-- Slides up from the Create tile (out to the right on the side rail) -->
          <div class="picker absolute z-50 flex flex-col gap-1.5 p-1.5 rounded-2xl glass-panel" role="menu" aria-label="Create">
            {#each KINDS as k, i (k.id)}
              {@const KIcon = KIND_ICONS[k.id]}
              <button
                data-kind={k.id}
                role="menuitem"
                class="kind flex items-center gap-3 px-4 py-3 2xl:px-6 2xl:py-4 rounded-xl text-sm 2xl:text-lg font-semibold whitespace-nowrap"
                class:hot={hovered === k.id}
                class:current={store.kind === k.id}
                in:fly={{ y: 16, duration: 380, delay: (KINDS.length - 1 - i) * PICKER_STAGGER_MS, easing: EASE }}
                out:fade={{ duration: 120 }}
                onpointerenter={() => (hovered = k.id)}
                onpointerleave={() => !held && (hovered = '')}
                onclick={() => choose(k.id)}
              >
                <KIcon size={20} /> {k.label}
              </button>
            {/each}
          </div>
        {/if}
        {#if t.id === 'create'}
          <button
            class="tile touch-none select-none"
            class:active
            role="tab"
            aria-selected={active}
            aria-label="Create {kindLabel}"
            aria-haspopup="menu"
            aria-expanded={pickerOpen}
            title="Tap to create · hold to pick image, video or audio"
            onpointerdown={down}
            onpointermove={move}
            onpointerup={up}
            onpointercancel={cancel}
            oncontextmenu={(e) => e.preventDefault()}
            onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), up())}
          >
            <Icon size={24} />
          </button>
          <span class="label">{t.label}</span>
          <SwapText text={kindLabel} class="sub" />
        {:else}
          <button class="tile" class:active role="tab" aria-selected={active} aria-label={t.label} onclick={() => store.setView(t.id)}>
            <Icon size={24} />
          </button>
          <span class="label">{t.label}</span>
          <SwapText text={t.id === 'chat' ? 'local LLM' : `${store.media.length} items`} class="sub" />
        {/if}
      </div>
    {/each}
  </div>
</nav>

<style>
  .nav {
    animation: slide-up-fade 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  @keyframes slide-up-fade {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
  }

  /* Tile size and spacing drive both the layout and the marker's travel. */
  .dock {
    --tile: 3.25rem;
    --gap: 2rem;
    gap: var(--gap);
  }

  @media (min-width: 96rem) {
    .dock {
      --tile: 4.5rem;
      --gap: 2.5rem;
    }
  }

  .slot {
    width: calc(var(--tile) + 1rem);
  }

  .tile {
    width: var(--tile);
    height: var(--tile);
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 1rem;
    background: rgb(10 10 15 / 45%);
    backdrop-filter: blur(12px) saturate(160%);
    border: 2px solid rgb(255 255 255 / 12%);
    color: rgb(255 255 255 / 65%);
    box-shadow: 0 4px 12px rgb(0 0 0 / 30%);
    transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    -webkit-touch-callout: none;
  }

  .tile:hover {
    color: #fff;
    background: rgb(255 255 255 / 8%);
    transform: translateY(-4px) scale(1.08);
  }

  .tile:active {
    transform: translateY(-1px) scale(0.96);
  }

  .tile.active {
    color: var(--color-neon-red);
    border-color: rgb(255 51 68 / 55%);
    background: rgb(255 51 68 / 10%);
    box-shadow: 0 0 20px rgb(255 51 68 / 30%), inset 0 1px 0 rgb(255 255 255 / 8%);
    transform: translateY(-2px);
  }

  .label {
    margin-top: 0.3rem;
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.03em;
    color: var(--color-muted);
    transition: color 0.3s;
  }

  .slot:has(.tile.active) .label {
    color: #fff;
  }

  .slot :global(.sub) {
    font-size: 0.56rem;
    color: rgb(255 255 255 / 40%);
  }

  @media (min-width: 96rem) {
    .label {
      font-size: 0.95rem;
    }

    .slot :global(.sub) {
      font-size: 0.8rem;
    }
  }

  /* Glowing marker: a short bar under the active tile on the bottom dock, beside it on the rail. */
  .marker {
    position: absolute;
    left: 0;
    bottom: -0.45rem;
    width: calc(var(--tile) + 1rem);
    height: 3px;
    display: flex;
    justify-content: center;
    transform: translateX(calc(var(--i) * (var(--tile) + 1rem + var(--gap))));
    transition: transform 0.5s cubic-bezier(0.16, 1, 0.3, 1);
    pointer-events: none;
  }

  .marker::after {
    content: "";
    width: 1.5rem;
    height: 100%;
    border-radius: 999px;
    background: var(--color-neon-red);
    box-shadow: 0 0 10px 1px rgb(255 51 68 / 80%);
  }

  /* Picker above the Create tile on the bottom dock */
  .picker {
    bottom: calc(100% + 0.6rem);
    left: 50%;
    translate: -50% 0;
  }

  @media (min-width: 48rem) {
    .marker {
      bottom: auto;
      top: 0;
      left: -0.75rem;
      width: 3px;
      height: var(--tile);
      align-items: center;
      transform: translateY(calc(var(--i) * (var(--slot-h, calc(var(--tile) + 2.2rem)) + var(--gap))));
    }

    .marker::after {
      width: 100%;
      height: 1.5rem;
    }

    /* ...and out to the right of it on the rail */
    .picker {
      bottom: auto;
      top: 0;
      left: calc(100% + 0.75rem);
      translate: 0 0;
    }
  }

  .kind {
    color: #d4d4d8;
    border: 1px solid transparent;
    transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1), background-color 0.2s, color 0.2s, border-color 0.2s;
  }

  .kind.current {
    color: var(--color-neon-red);
  }

  .kind.hot {
    color: #fff;
    background: rgb(255 51 68 / 22%);
    border-color: rgb(255 51 68 / 50%);
    box-shadow: 0 0 18px rgb(255 51 68 / 30%);
    transform: scale(1.06);
  }

  @media (prefers-reduced-motion: reduce) {
    .nav,
    .marker,
    .tile,
    .kind {
      animation: none;
      transition: none;
    }
  }
</style>
