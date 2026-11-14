<!-- UserMenu: who is using Castle. Opens from the Castle mark, top left. Each person has
     their own storage and arcade; a locked person asks for their PIN before switching. -->
<script>
  import { X } from '@lucide/svelte';
  import { scale, fade } from 'svelte/transition';
  import { shell } from '../lib/shell.svelte.js';
  import { pop, reducedMotion } from '../lib/motion.js';
  import PeopleList from './PeopleList.svelte';

  function close() {
    shell.userMenuOpen = false;
  }
</script>

{#if shell.userMenuOpen}
  <div class="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-label="People">
    <div
      transition:fade={{ duration: reducedMotion() ? 0 : 150 }}
      class="absolute inset-0 bg-black/50"
      onclick={close}
      onkeydown={(e) => { if (e.key === 'Escape') close(); }}
      role="button"
      tabindex="-1"
      aria-label="Close"
    ></div>
    <div
      transition:scale={pop()}
      class="hud-corners glass absolute left-3 top-3 w-[min(20rem,calc(100vw-1.5rem))] md:left-4 md:top-16"
      style="transform-origin: top left"
    >
      <div class="flex items-center gap-3 border-b border-hairline px-4 py-3">
        <p class="micro flex-1 text-text-faint">WHO IS USING CASTLE</p>
        <button class="expo t-hover p-1 text-text-dim transition-colors hover:text-white" onclick={close} aria-label="Close">
          <X class="h-4 w-4" strokeWidth={1.75} />
        </button>
      </div>

      <div class="p-2 pb-3">
        <PeopleList ondone={close} />
      </div>
    </div>
  </div>
{/if}
