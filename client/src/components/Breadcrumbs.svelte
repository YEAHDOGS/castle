<!-- Breadcrumbs: drill path. Backspace goes up; so does clicking. -->
<script>
  import { ChevronRight } from '@lucide/svelte';
  import { store } from '../lib/store.svelte.js';
</script>

{#if store.browsing && !store.query}
  <nav class="flex shrink-0 items-center gap-1 overflow-x-auto border-b border-hairline-faint px-4 py-2.5 md:px-6" aria-label="Breadcrumb">
    {#each store.path as crumb, i (crumb.id)}
      {@const last = i === store.path.length - 1}
      {#if i > 0}
        <ChevronRight class="h-3.5 w-3.5 shrink-0 text-text-faint" strokeWidth={1.75} />
      {/if}
      <button
        class="expo t-hover micro whitespace-nowrap px-1 py-0.5 transition-colors {last ? 'text-chiral' : 'text-text-dim hover:text-white'}"
        onclick={() => store.goToCrumb(i)}
        aria-current={last ? 'page' : undefined}
        disabled={last}
      >
        {crumb.name}
      </button>
    {/each}
  </nav>
{:else if store.query}
  <div class="micro shrink-0 border-b border-hairline-faint px-4 py-2.5 text-text-dim md:px-6">
    RESULTS FOR “{store.query.toUpperCase()}”
  </div>
{:else}
  <div class="micro shrink-0 border-b border-hairline-faint px-4 py-2.5 text-text-dim md:px-6">
    {{ drive: 'MY DRIVE', computer: 'THIS COMPUTER', shared: 'SHARED', recent: 'RECENT', starred: 'STARRED', trash: 'TRASH' }[store.view]}
  </div>
{/if}
