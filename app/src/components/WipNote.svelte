<!-- WipNote: the mark on a feature that is mostly not built yet. Say plainly what is missing.
     Its sidebar entry gets `wip: true`, which shows a WIP tag beside it (Sidebar, SectionFrame). -->
<script>
  import { Construction } from '@lucide/svelte';

  /** title: what the feature is. children: what is not done yet. */
  let { title = 'Work in progress', children } = $props();
</script>

<div class="wip mb-3 flex gap-3 border border-dashed border-chiral/50 bg-chiral-dim/40 p-3 md:p-4" role="note">
  <Construction class="mt-0.5 h-5 w-5 shrink-0 text-chiral" strokeWidth={1.75} />
  <div class="min-w-0">
    <p class="micro font-bold tracking-[0.14em] text-chiral">WORK IN PROGRESS · {title.toUpperCase()}</p>
    <p class="mt-1 text-[13px] leading-snug text-text-dim">{@render children?.()}</p>
  </div>
</div>
