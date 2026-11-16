<!-- WheelMenu: the section switcher. A dial centred on the top-right corner of the screen;
     only its lower-left quarter is visible. Sections repeat around the rim so it turns
     forever in either direction. Scroll, drag, arrow keys or click to turn it; the section
     under the beam is the one Enter (or a click) opens.

     The corner button itself does three things: a click opens or closes the dial, scrolling
     over it opens the dial and turns it, and holding it goes back to Home.

     From the keyboard, Shift+Enter opens and closes it anywhere (a controller's Start does
     the same, see lib/input.svelte.js). While it is open, a section's letter (underlined
     under its icon) or its number turns straight to it and opens it. -->
<script>
  import { untrack } from 'svelte';
  import { X } from '@lucide/svelte';
  import { shell } from '../lib/shell.svelte.js';
  import { SECTIONS, sectionById, sectionForKey } from '../lib/sections.js';
  import { prefs } from '../lib/prefs.svelte.js';
  import { input } from '../lib/input.svelte.js';

  const MIN_STEP_DEG = 30; // neighbouring slots sit at least this far apart, so they never overlap
  // The sections repeat a whole number of times around the rim, so the order never
  // breaks where the last slot meets the first. Only as many repeats as keep the slots
  // MIN_STEP_DEG apart: with eleven sections that is once round.
  const REPEATS = Math.max(1, Math.floor(360 / (MIN_STEP_DEG * SECTIONS.length)));
  const SLOT_COUNT = SECTIONS.length * REPEATS;
  const STEP_DEG = 360 / SLOT_COUNT; // angle between neighbouring slots
  const BEAM_DEG = 135; // where the selected slot rests: the diagonal into the screen
  const SCROLL_THRESHOLD = 40; // wheel delta that counts as one notch
  const CONFIRM_DELAY_MS = 260; // lets the dial finish turning before the section changes
  const DRAG_CLICK_DEG = 4; // a drag shorter than this is treated as a click
  const HOLD_HOME_MS = 550; // how long the corner button is held before it goes Home
  const SLOTS = Array.from({ length: SLOT_COUNT }, (_, k) => ({
    k,
    section: SECTIONS[k % SECTIONS.length],
    angle: k * STEP_DEG,
  }));

  let index = $state(0); // which slot is under the beam; unbounded so the dial never rewinds
  let dragDeg = $state(0); // extra rotation while a finger or mouse is dragging the dial
  let dragging = $state(false);
  let scrollSum = 0;
  let dragStart = null; // { angle, moved }
  let dial = $state(null);

  // How far the dial has turned. It is written straight onto the few elements that turn
  // (the disc, the ticks, the inner rim, each face) as their own `rotate`, never as a
  // custom property on the overlay: a property every descendant could read restyles all
  // of them (over a hundred elements) on every notch and every pointer move, which is
  // what made the dial stutter on phones.
  const rotation = $derived(BEAM_DEG - index * STEP_DEG + dragDeg);
  const activeSlot = $derived(((index % SLOT_COUNT) + SLOT_COUNT) % SLOT_COUNT);
  const active = $derived(SECTIONS[activeSlot % SECTIONS.length]);
  const current = $derived(sectionById(shell.section));
  const CurrentIcon = $derived(current.icon);

  // However the dial opens (the button, Shift+Enter, a controller's Start), it starts on the
  // section showing and takes the keyboard.
  $effect(() => {
    if (!shell.wheelOpen) return;
    untrack(() => {
      index = SECTIONS.findIndex((s) => s.id === shell.section);
      dragDeg = 0;
      scrollSum = 0;
      shell.userMenuOpen = false;
    });
    requestAnimationFrame(() => dial?.focus());
  });

  function open() {
    shell.wheelOpen = true;
  }

  function close() {
    clearTimeout(confirmTimer);
    shell.wheelOpen = false;
  }

  let confirmTimer = null;

  function confirm() {
    const chosen = active.id;
    clearTimeout(confirmTimer); // a second choice before the first lands replaces it
    confirmTimer = setTimeout(() => {
      shell.setSection(chosen);
      shell.wheelOpen = false;
    }, CONFIRM_DELAY_MS);
  }

  /** Turns to the nearest slot of a section and opens it: what a letter or number does. */
  function chooseSection(id) {
    let best = null;
    for (const slot of SLOTS) {
      if (slot.section.id !== id) continue;
      let turn = (((slot.k - activeSlot) % SLOT_COUNT) + SLOT_COUNT) % SLOT_COUNT;
      if (turn > SLOT_COUNT / 2) turn -= SLOT_COUNT;
      if (best === null || Math.abs(turn) < Math.abs(best)) best = turn;
    }
    if (best === null) return;
    index += best;
    confirm();
  }

  /** A section's label split around its letter, so the letter can be marked. */
  function labelParts(section) {
    const at = section.label.toLowerCase().indexOf(section.key);
    if (at < 0) return { before: section.label, key: '', after: '' };
    return { before: section.label.slice(0, at), key: section.label[at], after: section.label.slice(at + 1) };
  }

  /** Turns the shortest way round to bring slot `k` under the beam, then opens it. */
  function choose(k) {
    let turn = (((k - activeSlot) % SLOT_COUNT) + SLOT_COUNT) % SLOT_COUNT;
    if (turn > SLOT_COUNT / 2) turn -= SLOT_COUNT;
    index += turn;
    confirm();
  }

  function onWheel(e) {
    e.preventDefault();
    scrollSum += e.deltaY || e.deltaX;
    if (Math.abs(scrollSum) < SCROLL_THRESHOLD) return;
    index += Math.sign(scrollSum) * (prefs.get('look.wheelInvert') ? -1 : 1);
    scrollSum = 0;
  }

  /** Scrolling over the corner button: the dial opens if it is shut, and turns either way. */
  function onTriggerWheel(e) {
    if (!shell.wheelOpen) {
      scrollSum = 0;
      open();
    }
    onWheel(e);
  }

  // ---- holding the corner button goes Home ----
  let holding = $state(false);
  let holdTimer = null;
  let heldHome = false; // the click that ends a completed hold must not also toggle the dial

  function startHold(e) {
    if (e.button) return; // primary button or a finger only
    heldHome = false;
    holding = true;
    clearTimeout(holdTimer);
    holdTimer = setTimeout(() => {
      holding = false;
      heldHome = true;
      shell.goHome();
    }, HOLD_HOME_MS);
  }

  function endHold() {
    holding = false;
    clearTimeout(holdTimer);
  }

  function onTriggerClick() {
    if (heldHome) {
      heldHome = false;
      return;
    }
    if (shell.wheelOpen) close();
    else open();
  }

  function onKey(e) {
    const jump = !e.ctrlKey && !e.metaKey && !e.altKey ? sectionForKey(e.key) : null;
    // The next slot waits to the left of the beam, the previous one below it.
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') index += 1;
    else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') index -= 1;
    else if (e.key === 'Enter' || e.key === ' ') confirm();
    else if (e.key === 'Escape') close();
    else if (jump) chooseSection(jump.id);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  /** Angle of the pointer around the dial's centre (the screen's top-right corner). */
  function pointerAngle(e) {
    return (Math.atan2(e.clientY, e.clientX - window.innerWidth) * 180) / Math.PI;
  }

  function onPointerDown(e) {
    // No pointer capture: a captured pointer would swallow the click on a slot.
    dragStart = { angle: pointerAngle(e), moved: 0, deg: 0 };
    dragging = true;
  }

  // A finger can report its position far more often than the screen draws. Each move only
  // notes the angle; the dial takes it once per frame.
  let moveFrame = 0;

  function onPointerMove(e) {
    if (!dragStart) return;
    dragStart.deg = pointerAngle(e) - dragStart.angle;
    dragStart.moved = Math.max(dragStart.moved, Math.abs(dragStart.deg));
    if (!moveFrame) moveFrame = requestAnimationFrame(applyMove);
  }

  function applyMove() {
    moveFrame = 0;
    if (dragStart) dragDeg = dragStart.deg;
  }

  function onPointerUp() {
    if (!dragStart) return;
    cancelAnimationFrame(moveFrame);
    moveFrame = 0;
    dragDeg = dragStart.deg;
    index -= Math.round(dragDeg / STEP_DEG);
    dragDeg = 0;
    dragging = false;
    // Keep `moved` for the click that follows this pointerup, then forget the drag.
    const wasDrag = dragStart.moved > DRAG_CLICK_DEG;
    dragStart = null;
    suppressClick = wasDrag;
  }

  let suppressClick = false;

  function onSlotClick(k) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    choose(k);
  }
</script>

<svelte:window onpointermove={onPointerMove} onpointerup={onPointerUp} onpointercancel={onPointerUp} />

<button
  class="trigger fixed right-3 top-3 z-[70] grid h-10 w-10 place-items-center rounded-full text-chiral"
  class:open={shell.wheelOpen}
  class:holding
  style="--hold: {HOLD_HOME_MS}ms"
  onclick={onTriggerClick}
  onwheel={onTriggerWheel}
  onpointerdown={startHold}
  onpointerup={endHold}
  onpointerleave={endHold}
  onpointercancel={endHold}
  oncontextmenu={(e) => e.preventDefault()}
  aria-label={shell.wheelOpen ? 'Close the section wheel' : `Sections. Now in ${current.label}. Scroll to turn the wheel, hold to go Home`}
  title="Click, scroll or Shift+Enter for sections · hold for Home"
  data-nav-skip
  data-tour="wheel"
  aria-expanded={shell.wheelOpen}
  aria-haspopup="dialog"
>
  {#if shell.wheelOpen}
    <X class="h-4 w-4" strokeWidth={2} />
  {:else}
    <CurrentIcon class="h-4 w-4" strokeWidth={1.75} />
  {/if}
</button>

{#if shell.wheelOpen}
  <div
    bind:this={dial}
    class="overlay fixed inset-0 z-[65] outline-none"
    class:dragging
    role="dialog"
    aria-modal="true"
    aria-label="Sections"
    aria-keyshortcuts="Shift+Enter"
    data-nav-own
    tabindex="-1"
    onwheel={onWheel}
    onkeydown={onKey}
  >
    <button class="backdrop absolute inset-0 cursor-default" onclick={close} aria-label="Close" tabindex="-1"></button>

    <div
      class="wheel"
      role="presentation"
      onpointerdown={onPointerDown}
    >
      <div class="band"></div>
      <div class="ticks" style:rotate="{rotation * -0.5}deg"></div>
      <div class="beam"></div>
      <div class="rim rim-outer"></div>
      <div class="rim rim-inner" style:rotate="{rotation * 1.5}deg"></div>
      <div class="disc" style:rotate="{rotation}deg">
        {#each SLOTS as slot (slot.k)}
          {@const Icon = slot.section.icon}
          {@const isActive = slot.k === activeSlot}
          {@const label = labelParts(slot.section)}
          <div class="slot" class:front={isActive} style="--a: {slot.angle}deg">
            <button
              class="slot-face"
              class:active={isActive}
              style:rotate="{-(slot.angle + rotation)}deg"
              onclick={() => onSlotClick(slot.k)}
              tabindex={isActive ? 0 : -1}
              aria-label={slot.section.label}
              aria-keyshortcuts={slot.section.key.toUpperCase()}
              aria-current={slot.section.id === shell.section ? 'page' : undefined}
            >
              <span class="tile"><Icon class="h-6 w-6" strokeWidth={1.5} /></span>
              <span class="micro label">{label.before}<span class="hotkey">{label.key}</span>{label.after}</span>
            </button>
          </div>
        {/each}
      </div>
    </div>

    <div class="readout pointer-events-none">
      <p class="micro text-text-faint">
        {String((activeSlot % SECTIONS.length) + 1).padStart(2, '0')} / {String(SECTIONS.length).padStart(2, '0')}
      </p>
      {#key active.id}
        <p class="name">{active.label}</p>
        <p class="tagline">{active.tagline}</p>
      {/key}
      {#if input.mode === 'pad'}
        <p class="micro mt-3 text-text-faint">STICK OR D-PAD TO TURN · {input.padKind === 'playstation' ? '✕' : 'A'} TO OPEN · {input.padKind === 'playstation' ? '○' : 'B'} TO CLOSE</p>
      {:else if input.mode === 'keys'}
        <p class="micro mt-3 text-text-faint">PRESS A SECTION'S LETTER · ARROWS TO TURN · ENTER TO OPEN · SHIFT+ENTER TO CLOSE</p>
      {:else}
        <p class="micro mt-3 text-text-faint">SCROLL OR DRAG TO TURN · ENTER TO OPEN · HOLD THE CORNER FOR HOME</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* ---- the corner button: a slow ring of light around the current section's icon ---- */
  .trigger {
    background: var(--color-panel-solid);
    border: 1px solid var(--color-hairline);
    transition: transform 150ms var(--ease-expo), border-color 150ms var(--ease-expo);
  }
  .trigger::before {
    content: '';
    position: absolute;
    inset: -3px;
    border-radius: 50%;
    background: conic-gradient(from 0deg, transparent 0 62%, var(--color-chiral) 78%, var(--color-bridge) 92%, transparent 100%);
    mask: radial-gradient(closest-side, transparent calc(100% - 2px), #000 calc(100% - 2px));
    -webkit-mask: radial-gradient(closest-side, transparent calc(100% - 2px), #000 calc(100% - 2px));
    animation: ring-spin 5s linear infinite;
  }
  .trigger:hover { transform: scale(1.08); border-color: color-mix(in srgb, var(--color-chiral) 60%, transparent); }
  .trigger:active { transform: scale(0.94); }
  .trigger.open { color: #fff; }
  .trigger.open::before { animation-duration: 1.6s; }
  @keyframes ring-spin { to { transform: rotate(360deg); } }
  /* A long press must not select text or raise the phone's own press menu. */
  .trigger { user-select: none; -webkit-user-select: none; -webkit-touch-callout: none; touch-action: manipulation; }
  /* Held: the button fills from the rim inwards, and is full at the moment it goes Home. */
  .trigger.holding {
    animation: hold-fill var(--hold) linear forwards;
    border-color: var(--color-chiral);
  }
  .trigger.holding::before { animation-duration: 0.9s; }
  @keyframes hold-fill {
    from { box-shadow: inset 0 0 0 0 color-mix(in srgb, var(--color-chiral) 45%, transparent); }
    to { box-shadow: inset 0 0 0 20px color-mix(in srgb, var(--color-chiral) 45%, transparent), 0 0 24px color-mix(in srgb, var(--color-chiral) 60%, transparent); }
  }

  /* ---- overlay ---- */
  .overlay {
    /* The dial's diameter: big on a desktop, and never wider or taller than the screen
       can show a quarter of. */
    --d: min(700px, 176vw, 176vh);
    --turn: 520ms;
  }
  .overlay.dragging { --turn: 0ms; }
  .backdrop {
    background: radial-gradient(circle at 100% 0%, rgba(0, 0, 0, 0.5), rgba(0, 0, 0, 0.82) 70%);
    animation: fade-in 250ms var(--ease-expo);
  }
  /* The blur behind the dial is re-filtered under every frame the dial moves; a phone's
     screen is dense enough that this alone costs it frames, so a touch screen gets a
     slightly deeper shade instead. */
  @media (pointer: fine) {
    .backdrop {
      backdrop-filter: blur(6px);
      -webkit-backdrop-filter: blur(6px);
    }
  }
  @media (pointer: coarse) {
    .backdrop { background: radial-gradient(circle at 100% 0%, rgba(0, 0, 0, 0.6), rgba(0, 0, 0, 0.88) 70%); }
  }
  @keyframes fade-in { from { opacity: 0; } }

  /* ---- the dial: a circle whose centre is the screen's top-right corner ---- */
  .wheel {
    position: absolute;
    top: 0;
    right: 0;
    width: var(--d);
    height: var(--d);
    transform: translate(50%, -50%);
    border-radius: 50%;
    touch-action: none;
    contain: layout;
    animation: wheel-in 520ms var(--ease-expo);
  }
  @keyframes wheel-in {
    from { transform: translate(50%, -50%) scale(0.55) rotate(-60deg); opacity: 0; }
  }
  .wheel > * { position: absolute; inset: 0; border-radius: 50%; pointer-events: none; }
  /* Everything that turns does so as `rotate` (and `scale`) on its own compositor layer,
     so a turn is a transform the GPU interpolates, not a repaint. The layers only exist
     while the dial is open, as the whole dial is. */
  .ticks, .rim-inner, .disc, .slot-face {
    transition: rotate var(--turn) var(--ease-expo);
    will-change: transform;
  }

  /* the dark track the slots ride on */
  .band {
    background: radial-gradient(closest-side, transparent 0 40%, rgba(8, 10, 14, 0.94) 40.4% 98%, transparent 98.4%);
    box-shadow: 0 0 80px rgba(0, 0, 0, 0.8);
  }
  /* fine tick marks on the outer edge; they counter-turn at half speed for depth */
  .ticks {
    background: repeating-conic-gradient(from 0deg, rgba(255, 255, 255, 0.34) 0 0.35deg, transparent 0.35deg 3deg);
    mask: radial-gradient(closest-side, transparent calc(100% - 26px), #000 calc(100% - 26px) calc(100% - 14px), transparent calc(100% - 14px));
    -webkit-mask: radial-gradient(closest-side, transparent calc(100% - 26px), #000 calc(100% - 26px) calc(100% - 14px), transparent calc(100% - 14px));
  }
  /* the beam: a fixed wedge of light marking the selected slot. Conic angles start at
     12 o'clock, so the 135deg diagonal is 225deg here. */
  .beam {
    background: conic-gradient(
      from 207deg,
      transparent 0deg,
      color-mix(in srgb, var(--color-chiral) 30%, transparent) 18deg,
      transparent 36deg,
      transparent 360deg
    );
    mask: radial-gradient(closest-side, transparent 40%, #000 40.4% 98%, transparent 98.4%);
    -webkit-mask: radial-gradient(closest-side, transparent 40%, #000 40.4% 98%, transparent 98.4%);
    animation: beam-breathe 2.4s ease-in-out infinite;
  }
  @keyframes beam-breathe { 50% { opacity: 0.55; } }
  .rim-outer {
    inset: 5px;
    border: 1px solid color-mix(in srgb, var(--color-chiral) 45%, transparent);
    box-shadow: 0 0 30px color-mix(in srgb, var(--color-chiral) 22%, transparent), inset 0 0 30px color-mix(in srgb, var(--color-chiral) 10%, transparent);
  }
  .rim-inner {
    inset: 30%;
    border: 1px dashed color-mix(in srgb, var(--color-bridge) 45%, transparent);
  }

  /* ---- slots: the disc turns; each face counter-turns so it always reads upright ---- */
  .slot {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 0;
    height: 0;
    transform: rotate(var(--a)) translateX(calc(var(--d) * 0.345));
  }
  /* The selected slot is drawn over its neighbours, never under them. */
  .slot.front { z-index: 1; }
  .slot-face {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    pointer-events: auto;
    color: var(--color-text-dim);
    /* translate, then rotate (set inline, counter to the disc), then scale: the same order
       the old single `transform` had. */
    translate: -50% -50%;
    scale: 0.86;
    /* The text and icon colour changes at once rather than fading: `color` is inherited, so
       fading it restyles every element of two faces on every frame of the fade. The glow,
       the border and the scale still ease in, which is what the eye reads as the fade. */
    transition: rotate var(--turn) var(--ease-expo), scale var(--turn) var(--ease-expo);
  }
  .slot-face.active {
    color: #fff;
    scale: 1.18;
  }
  .tile {
    position: relative;
    display: grid;
    place-items: center;
    width: 68px;
    height: 68px;
    border-radius: 50%;
    background: var(--color-panel-solid);
    border: 1px solid var(--color-hairline);
    transition: border-color 250ms var(--ease-expo);
  }
  /* The selected tile's glow. A shadow that fades in by changing its colour has to be
     repainted every frame; one painted once and faded in with opacity is composited. */
  .tile::after {
    content: '';
    position: absolute;
    inset: -1px;
    border-radius: 50%;
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--color-chiral) 14%, transparent), 0 0 38px color-mix(in srgb, var(--color-chiral) 55%, transparent);
    opacity: 0;
    transition: opacity 250ms var(--ease-expo);
  }
  .slot-face:hover .tile { border-color: rgba(255, 255, 255, 0.3); }
  .slot-face.active .tile {
    color: var(--color-chiral);
    border-color: var(--color-chiral);
  }
  .slot-face.active .tile::after { opacity: 1; }
  .label { white-space: nowrap; }
  /* the letter that opens a section from the keyboard */
  .hotkey { text-decoration: underline; text-decoration-thickness: 1px; text-underline-offset: 3px; }
  .slot-face.active .hotkey { color: var(--color-chiral); }

  /* ---- readout: the selected section, in words, under the dial ---- */
  .readout {
    position: absolute;
    top: calc(var(--d) / 2 + 18px);
    right: 24px;
    text-align: right;
  }
  .name {
    font-size: clamp(34px, 6vw, 60px);
    font-weight: 900;
    line-height: 1;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #fff;
    animation: readout-in 320ms var(--ease-expo);
  }
  .tagline {
    margin-top: 8px;
    font-size: 14px;
    color: var(--color-text-dim);
    animation: readout-in 320ms var(--ease-expo);
  }
  @keyframes readout-in { from { opacity: 0; transform: translateX(18px); } }

  /* Short screens (a phone on its side): the readout would fall off the bottom, so it
     moves beside the dial instead. */
  @media (max-height: 520px) {
    .readout { top: auto; bottom: 16px; right: calc(var(--d) / 2 + 18px); }
  }
</style>
