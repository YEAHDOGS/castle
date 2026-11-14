<!-- PadHints: what the controller's buttons do, along the bottom of the screen. Shows only
     while a controller is in use; a section, a card or the player says what its buttons do
     with setHints (lib/input.svelte.js), and these are shown otherwise. Button names follow
     the controller: A B X Y on an Xbox pad, the shapes on a PlayStation one. -->
<script>
  import { fly } from 'svelte/transition';
  import { input } from '../lib/input.svelte.js';
  import { reducedMotion } from '../lib/motion.js';

  const DEFAULT = [
    { btn: 'A', label: 'Select' },
    { btn: 'B', label: 'Back' },
    { btn: 'LB/RB', label: 'Views' },
    { btn: 'START', label: 'Sections' },
    { btn: 'VIEW', label: 'Home' },
  ];

  const NAMES = {
    xbox: { A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'LB', RB: 'RB', LT: 'LT', RT: 'RT', START: '☰', VIEW: '⧉', DPAD: '✚' },
    playstation: { A: '✕', B: '○', X: '□', Y: '△', LB: 'L1', RB: 'R1', LT: 'L2', RT: 'R2', START: 'OPTIONS', VIEW: 'CREATE', DPAD: '✚' },
  };
  // Face buttons keep their colours so they read at a glance.
  const TONE = { A: 'text-live', B: 'text-red-400', X: 'text-bridge', Y: 'text-chiral' };

  const list = $derived(input.hints || DEFAULT);
  const names = $derived(NAMES[input.padKind] || NAMES.xbox);
  const label = (btn) => btn.split('/').map((b) => names[b] || b).join(' ');
</script>

{#if input.mode === 'pad'}
  <div
    class="pad-hints pointer-events-none fixed left-1/2 z-[80] flex -translate-x-1/2 items-center gap-3 border border-hairline bg-black/80 px-3 py-1.5 {input.lift ? 'bottom-[calc(var(--strip)+4.75rem)]' : 'bottom-[calc(var(--strip)+0.75rem)]'}"
    transition:fly={{ y: 12, duration: reducedMotion() ? 0 : 200 }}
    aria-hidden="true"
  >
    {#each list as hint (hint.btn + hint.label)}
      <span class="micro flex items-center gap-1.5 whitespace-nowrap text-text-dim">
        <span class="key {TONE[hint.btn] || 'text-white'}">{label(hint.btn)}</span>{hint.label}
      </span>
    {/each}
  </div>
{/if}

<style>
  .key {
    display: inline-grid;
    place-items: center;
    min-width: 20px;
    height: 20px;
    padding: 0 5px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
  }
  /* Phones and narrow windows: the hints would not fit, and a phone rarely has a controller. */
  @media (max-width: 640px) {
    .pad-hints { display: none; }
  }
</style>
