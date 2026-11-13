<!-- ShowcaseBanner: the slim mark the public demo wears on every screen (see lib/showcase.js).
     It says what this is, and leads back to the waitlist. It can be tucked away to a dot and
     brought back; the demo stays read-only either way. -->
<script>
  import { X, ArrowUpRight } from '@lucide/svelte';
  import { fade } from 'svelte/transition';
  import { WAITLIST_URL } from '../lib/showcase.js';
  import { reducedMotion } from '../lib/motion.js';

  let tucked = $state(false);
</script>

{#if tucked}
  <button
    in:fade={{ duration: reducedMotion() ? 0 : 150 }}
    class="micro fixed bottom-3 left-3 z-[70] border border-bridge/40 bg-black/85 px-2 py-1 font-bold text-bridge backdrop-blur"
    onclick={() => (tucked = false)}
    aria-label="Show what this demo is"
  >
    DEMO
  </button>
{:else}
  <div
    in:fade={{ duration: reducedMotion() ? 0 : 150 }}
    class="fixed bottom-3 left-3 z-[70] flex max-w-[calc(100vw-1.5rem)] items-center gap-3 border border-bridge/40 bg-black/85 py-1.5 pl-3 pr-1.5 backdrop-blur"
    role="note"
    aria-label="About this demo"
  >
    <span class="micro font-bold text-bridge">DEMO</span>
    <span class="micro hidden text-text-dim sm:inline">SAMPLE DATA · LOOK AROUND, NOTHING RUNS</span>
    <a
      href={WAITLIST_URL}
      class="micro expo t-hover flex items-center gap-1 bg-chiral px-2.5 py-1 font-bold text-black transition-opacity hover:opacity-90"
    >
      JOIN THE WAITLIST<ArrowUpRight class="h-3.5 w-3.5" strokeWidth={2.25} />
    </a>
    <button class="expo t-hover p-1 text-text-faint transition-colors hover:text-white" onclick={() => (tucked = true)} aria-label="Tuck the demo note away">
      <X class="h-3.5 w-3.5" />
    </button>
  </div>
{/if}
