<!-- TvStrip: shown along the top while TV remote mode is on (lib/tv.svelte.js). It says the
     mode is on, how to drive it, and the last thing the remote sent, which is what there is
     to read out when the mode misbehaves on a TV that has no developer tools. -->
<script>
  import { fly } from 'svelte/transition';
  import { Tv } from '@lucide/svelte';
  import { tv } from '../lib/tv.svelte.js';
  import { reducedMotion } from '../lib/motion.js';
</script>

{#if tv.on}
  <div
    class="pointer-events-none fixed left-1/2 top-2 z-[90] flex max-w-[calc(100vw-1rem)] -translate-x-1/2 items-center gap-3 border border-chiral/60 bg-black/85 px-3 py-1.5"
    transition:fly={{ y: -12, duration: reducedMotion() ? 0 : 200 }}
    role="status"
  >
    <Tv class="h-4 w-4 shrink-0 text-chiral" strokeWidth={1.5} />
    <span class="micro whitespace-nowrap text-white">TV REMOTE</span>
    <span class="micro hidden whitespace-nowrap text-text-dim xl:inline">NUDGE TO MOVE · CLICK TO PRESS · BACK TO CLOSE OR GO HOME</span>
    <span class="micro min-w-0 truncate text-chiral">{tv.last.toUpperCase()}</span>
  </div>
{/if}
