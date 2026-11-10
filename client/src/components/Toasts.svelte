<!-- Toasts: mono microcopy, bottom-center, expo-out. -->
<script>
  import { fly } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { reducedMotion, DUR, expoOut } from '../lib/motion.js';
</script>

<div class="pointer-events-none fixed bottom-20 left-1/2 z-[60] flex -translate-x-1/2 flex-col items-center gap-2 md:bottom-8" aria-live="polite">
  {#each store.toasts as toast (toast.id)}
    <div
      transition:fly={reducedMotion() ? { duration: 0 } : { duration: DUR.UI, easing: expoOut, y: 12, opacity: 0 }}
      class="glass micro border-chiral/50 px-4 py-2.5 text-white"
      style="box-shadow: 0 4px 24px rgba(0,0,0,.5)"
    >
      {toast.msg.toUpperCase()}
    </div>
  {/each}
</div>
