<script>
  // Collapsible Library section: a header row with a chevron, an optional summary on the right, and content that
  // slides open. `peek` content stays visible while collapsed (e.g. the first row of model tiles).
  import { ChevronDown } from '@lucide/svelte'
  import { slide } from 'svelte/transition'
  import { EASE } from '../lib/motion.js'

  /** @type {{ title: string, open?: boolean, summary?: import('svelte').Snippet, peek?: import('svelte').Snippet, children: import('svelte').Snippet }} */
  let { title, open = $bindable(false), summary, peek, children } = $props()
  const DURATION_MS = 380
</script>

<section class="rounded-2xl glass">
  <header class="flex items-center gap-3 px-4 py-3 2xl:px-6 2xl:py-4">
    <button class="flex items-center gap-2 font-bold tracking-wide text-sm 2xl:text-xl hover:text-neon-red transition-colors"
      onclick={() => (open = !open)} aria-expanded={open}>
      <ChevronDown size={18} class="transition-transform duration-500 ease-[cubic-bezier(0.16,1,0.3,1)] {open ? '' : '-rotate-90'}" />
      {title}
    </button>
    <div class="ml-auto min-w-0 flex items-center gap-2 text-xs 2xl:text-base text-muted">{@render summary?.()}</div>
  </header>
  {#if peek}<div class="px-4 pb-3 2xl:px-6">{@render peek()}</div>{/if}
  {#if open}
    <div class="px-4 pb-4 2xl:px-6 2xl:pb-6" transition:slide={{ duration: DURATION_MS, easing: EASE }}>
      {@render children()}
    </div>
  {/if}
</section>
