<script>
  import { store } from '../lib/store.svelte.js'
  import { storageSegments } from '../lib/library.js'
  import { size } from '../lib/format.js'

  // Segment colors, strongest first (largest segment gets the brand red).
  const COLORS = ['#ff3344', '#ff8a3d', '#e6b900', '#38bdf8', '#a855f7', '#22c55e']

  const segs = $derived(storageSegments(store.catalog, store.llm))
  const used = $derived(segs.reduce((s, x) => s + x.bytes, 0))
  const disk = $derived(store.catalog.storage?.disk)
  // Scale against the whole drive when known, so the bar also shows how much room is left.
  const scale = $derived(disk?.total ? disk.total : used || 1)
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-baseline justify-between gap-3 text-xs 2xl:text-base">
    <span><span class="text-xl 2xl:text-3xl font-bold text-white">{size(used)}</span> <span class="text-muted">of model files</span></span>
    {#if disk}<span class="text-muted">{size(disk.free)} free of {size(disk.total)}</span>{/if}
  </div>
  <div class="h-3 2xl:h-4 rounded-full bg-white/5 overflow-hidden flex" role="img" aria-label="Disk used by model files">
    {#each segs as s, i (s.label)}
      <div class="h-full transition-[width] duration-700 ease-[cubic-bezier(0.16,1,0.3,1)] first:rounded-l-full"
        style="width: {(s.bytes / scale) * 100}%; background: {COLORS[i % COLORS.length]}" title="{s.label}: {size(s.bytes)}"></div>
    {/each}
  </div>
  <div class="flex flex-wrap gap-x-4 gap-y-1 text-[11px] 2xl:text-sm text-neutral-400">
    {#each segs as s, i (s.label)}
      <span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-full" style="background: {COLORS[i % COLORS.length]}"></span>{s.label} · {size(s.bytes)}</span>
    {/each}
  </div>
</div>
