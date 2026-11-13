<!-- VmScreen: the screen of a running virtual machine, shown in the app. The box asks the
     machine for a picture of its display about once a second. View only: the machine's own
     window stays where it is and is still the place to click or type. -->
<script>
  import { onDestroy } from 'svelte';
  import { Monitor, Maximize2, Minimize2, MousePointer2 } from '@lucide/svelte';
  import LiveScreen from './LiveScreen.svelte';

  /** source: where Clones reads from. id: the machine to show. controllable: it runs in a
      container, so Take control can open a live viewer on it. name: what to call it. */
  let { source, id, controllable = false, name = 'the machine' } = $props();
  let controlling = $state(false);

  const FRAME_MS = 1000; // pause between one picture arriving and the next being asked for
  const RETRY_MS = 3000; // after a failed picture

  let frame = $state(null); // object URL of the newest picture
  let problem = $state('');
  let large = $state(false);
  let seenAt = $state(0);

  // The card re-reads its automation every couple of seconds and hands the same id down
  // again. These only change when the value really does, so that does not restart the view.
  const from = $derived(source);
  const machine = $derived(id);
  let shown = null; // the object URL on screen; kept across restarts so the picture never blanks

  onDestroy(() => {
    if (shown) URL.revokeObjectURL(shown);
  });

  $effect(() => {
    const box = from;
    const which = machine;
    let stopped = false;
    let timer;

    async function next() {
      try {
        const url = await box.cloneScreen(which);
        // Load the new picture before showing it, so the view never blinks empty between frames.
        // (The load event, not decode(): decode waits for the page to be drawn, which a
        // minimised window never does.)
        await new Promise((loaded, failed) => {
          const loading = new Image();
          loading.onload = loaded;
          loading.onerror = () => failed(new Error('The machine sent a picture that could not be shown'));
          loading.src = url;
        });
        if (stopped) return URL.revokeObjectURL(url);
        if (shown) URL.revokeObjectURL(shown);
        shown = url;
        frame = url;
        seenAt = Date.now();
        problem = '';
        timer = setTimeout(next, FRAME_MS);
      } catch (err) {
        if (stopped) return;
        problem = err?.message || 'The screen could not be read';
        timer = setTimeout(next, RETRY_MS);
      }
    }
    next();

    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });
</script>

<div class="mt-4">
  <div class="mb-2 flex items-center gap-3">
    <p class="micro flex flex-1 items-center gap-2 text-text-faint">
      <Monitor class="h-3.5 w-3.5" /> THE MACHINE'S SCREEN
      {#if frame && !problem}<span class="flex items-center gap-1.5 text-live"><span class="h-1.5 w-1.5 animate-pulse rounded-full bg-current"></span> LIVE</span>{/if}
    </p>
    {#if controllable && source.cloneLive}
      <button
        class="expo t-hover micro flex h-8 items-center gap-1.5 border border-chiral/60 px-2.5 font-bold text-chiral transition-colors hover:bg-chiral-dim"
        onclick={() => (controlling = true)}
        title="Open a live viewer: click and type in the machine"
      >
        <MousePointer2 class="h-3.5 w-3.5" /> TAKE CONTROL
      </button>
    {/if}
    <button
      class="expo t-hover micro flex h-8 items-center gap-1.5 border border-hairline px-2.5 font-bold text-white transition-colors hover:border-chiral/60"
      onclick={() => (large = !large)}
      aria-pressed={large}
    >
      {#if large}<Minimize2 class="h-3.5 w-3.5" /> SMALLER{:else}<Maximize2 class="h-3.5 w-3.5" /> LARGER{/if}
    </button>
  </div>
  <div class="expo t-ui grid place-items-center overflow-hidden border border-hairline bg-black transition-all {large ? 'max-h-[80dvh]' : 'max-h-[26rem]'}">
    {#if frame}
      <img src={frame} alt="The virtual machine's screen" draggable="false" class="h-auto max-w-full object-contain {large ? 'max-h-[80dvh]' : 'max-h-[26rem]'}" />
    {:else}
      <p class="micro p-10 text-text-faint">{problem || 'ASKING THE MACHINE FOR ITS SCREEN'}</p>
    {/if}
  </div>
  <p class="mt-2 text-[12px] leading-relaxed text-text-dim">
    {#if problem && frame}
      <span class="text-chiral">{problem}.</span> Showing the last picture, from {new Date(seenAt).toLocaleTimeString()}.
    {:else}
    {:else if controllable}
      A new picture about once a second. This view only watches: Take control to click and type in it.
    {:else}
      A new picture about once a second. This view only watches: to click or type, use the machine's own window.
    {/if}
  </p>
</div>

{#if controlling}
  <LiveScreen {source} {id} {name} onclose={() => (controlling = false)} />
{/if}
