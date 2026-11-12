<script>
  import { CircleAlert, X } from '@lucide/svelte'
  import { fly } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { errors } from '../lib/errors.svelte.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
</script>

<div class="fixed bottom-4 right-4 z-[60] flex flex-col gap-2 w-[min(26rem,calc(100vw-2rem))] pointer-events-none" role="alert" aria-live="assertive">
  {#each errors.items as e (e.id)}
    <div class="pointer-events-auto flex items-start gap-2.5 p-3 rounded-xl glass-panel border border-neon-red/50 shadow-lg"
      animate:flip={{ duration: PAGE_MS, easing: EASE }} transition:fly={{ x: 24, duration: PAGE_MS, easing: EASE }}>
      <CircleAlert size={18} class="shrink-0 mt-0.5 text-neon-red" />
      <p class="flex-1 min-w-0 text-sm 2xl:text-lg text-neutral-100 break-words select-text">{e.message}</p>
      <button class="shrink-0 p-1 rounded-md text-neutral-400 hover:text-white hover:bg-white/10" aria-label="Dismiss" onclick={() => errors.dismiss(e.id)}>
        <X size={15} />
      </button>
    </div>
  {/each}
</div>
