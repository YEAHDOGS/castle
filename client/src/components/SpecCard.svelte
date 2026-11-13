<!-- SpecCard: the box's numbers on one picture, to post. Cores, memory, terabytes, disks
     and the oldest one's power-on hours, uptime, clones, Theater and Arcade (lib/spec-card.js).
     It is drawn on this device; Save keeps it as a PNG, and Share hands it to another app
     where the device can. Escape or B closes it. -->
<script>
  import { scale, fade } from 'svelte/transition';
  import { Download, Share2, X, LoaderCircle } from '@lucide/svelte';
  import { shell } from '../lib/shell.svelte.js';
  import { store } from '../lib/store.svelte.js';
  import { modalIn, reducedMotion } from '../lib/motion.js';
  import { gatherSpecs, drawSpecCard, specCardFile, canShareFile, downloadFile } from '../lib/spec-card.js';
  import { demoSpecs } from '../lib/demo-specs.js';

  let { open = false, onclose = () => {} } = $props();

  let canvas = $state(null);
  let file = $state(null);
  let busy = $state(false);
  let problem = $state('');

  /** Where the numbers come from. The demo box has no /specs, so it borrows the demo's. */
  function sources() {
    const box = shell.boxSource;
    return {
      box: store.isDemo ? { system: () => box.system(), specs: async () => demoSpecs() } : box,
      storage: store.adapter,
      clones: shell.clonesSource,
      theater: shell.theaterSource,
      arcade: shell.arcadeSource,
    };
  }

  async function make() {
    if (!shell.boxSource) return;
    busy = true;
    problem = '';
    file = null;
    try {
      const specs = await gatherSpecs(sources());
      const drawn = await drawSpecCard(specs, canvas);
      file = await specCardFile(drawn, specs.host);
    } catch (err) {
      problem = err?.message || 'The card could not be made';
    }
    busy = false;
  }

  $effect(() => {
    if (open && canvas) make();
  });

  function save() {
    if (!file) return;
    downloadFile(file);
    store.toast(`Saved ${file.name}`);
  }

  async function share() {
    if (!file) return;
    try {
      await navigator.share({ files: [file], title: 'My Castle', text: 'The box.' });
    } catch (err) {
      if (err?.name !== 'AbortError') store.toast('Sharing did not work here. Save it instead.');
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 z-[80] grid place-items-center p-4" role="dialog" aria-modal="true" aria-labelledby="spec-card-title">
    <div class="absolute inset-0 bg-black/75" transition:fade={{ duration: reducedMotion() ? 0 : 150 }} role="presentation" onclick={onclose}></div>
    <div transition:scale={modalIn()} class="hud-corners glass relative z-10 w-full max-w-4xl px-5 py-5">
      <div class="mb-4 flex items-start gap-3">
        <div class="min-w-0 flex-1">
          <h2 id="spec-card-title" class="micro text-chiral">SPEC CARD</h2>
          <p class="mt-1 text-[13px] text-text-dim">Your box in one picture, for posting. Made on this device; nothing leaves it unless you share it.</p>
        </div>
        <button class="expo t-hover grid h-9 w-9 shrink-0 place-items-center border border-hairline text-text-dim hover:text-white" onclick={onclose} aria-label="Close spec card">
          <X class="h-4 w-4" />
        </button>
      </div>

      <div class="relative border border-hairline bg-void">
        <canvas bind:this={canvas} class="block h-auto w-full" style="aspect-ratio: 1200 / 630;" aria-label="The spec card"></canvas>
        {#if busy}
          <div class="absolute inset-0 grid place-items-center bg-black/40"><LoaderCircle class="h-6 w-6 animate-spin text-text-faint" /></div>
        {/if}
      </div>
      {#if problem}<p class="micro mt-3 text-chiral">{problem.toUpperCase()}</p>{/if}

      <div class="mt-4 flex flex-wrap justify-end gap-2">
        {#if file && canShareFile(file)}
          <button class="micro expo t-hover flex h-10 items-center gap-1.5 border border-bridge/50 px-4 text-bridge hover:bg-bridge/10" onclick={share}>
            <Share2 class="h-3.5 w-3.5" />SHARE
          </button>
        {/if}
        <button data-autofocus class="micro expo t-hover flex h-10 items-center gap-1.5 bg-chiral px-4 font-bold text-black hover:brightness-110 disabled:opacity-50" onclick={save} disabled={!file}>
          <Download class="h-3.5 w-3.5" />SAVE PNG
        </button>
      </div>
    </div>
  </div>
{/if}
