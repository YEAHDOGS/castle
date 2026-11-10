<!-- ModeSwitch: LIVE or DEMO, in every header. Live is your own box; Demo is sample data
     that changes nothing. Switching keeps you in the same section. -->
<script>
  import { shell } from '../lib/shell.svelte.js';
  import { store } from '../lib/store.svelte.js';

  const MODES = [
    { demo: false, label: 'LIVE' },
    { demo: true, label: 'DEMO' },
  ];

  let switching = $state(false);

  const liveHint = $derived(store.boxUrl ? `Your box at ${store.boxUrl}` : 'Your own files. No box is connected yet');
  const hint = (mode) => (mode.demo ? 'Sample data. Nothing here is saved or sent to a box' : liveHint);

  async function choose(mode) {
    if (switching || mode.demo === store.isDemo) return;
    switching = true;
    try {
      await shell.setDemo(mode.demo);
    } finally {
      switching = false;
    }
  }
</script>

<div class="relative flex h-10 shrink-0 items-stretch border border-hairline" role="group" aria-label="Live or demo data">
  <!-- the highlight slides between the two halves -->
  <span
    class="expo t-ui pointer-events-none absolute inset-y-0 left-0 w-1/2 transition-transform {store.isDemo ? 'translate-x-full bg-bridge-dim' : 'bg-chiral-dim'}"
    aria-hidden="true"
  ></span>
  {#each MODES as mode (mode.label)}
    {@const active = mode.demo === store.isDemo}
    <button
      class="micro relative flex w-[4.25rem] items-center justify-center gap-1.5 font-bold {active ? (mode.demo ? 'text-bridge' : 'text-chiral') : 'text-text-faint hover:text-white'}"
      aria-pressed={active}
      disabled={switching}
      title={hint(mode)}
      onclick={() => choose(mode)}
    >
      {#if !mode.demo}
        <!-- green when a box is answering, hollow when live has no box yet -->
        <span class="h-1.5 w-1.5 rounded-full {store.boxUrl ? 'bg-live' : 'border border-current'}" aria-hidden="true"></span>
      {/if}
      {mode.label}
    </button>
  {/each}
</div>
