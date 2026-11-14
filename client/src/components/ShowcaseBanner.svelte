<!-- ShowcaseBanner: the strip the public demo wears across the bottom of every screen (see
     lib/showcase.js). It warns that this is test development, links to the code, and leads
     back to the waitlist. It cannot be tucked away: html.showcase (set in App.svelte) gives
     it its own room, --strip, so it never covers the app (see app.css). -->
<script>
  import { ArrowUpRight, TriangleAlert } from '@lucide/svelte';
  import { WAITLIST_URL, GITHUB_URL, WARNING } from '../lib/showcase.js';
</script>

<div class="strip fixed inset-x-0 bottom-0 z-[45] flex items-center gap-3 border-t border-hairline bg-black/90 px-3 backdrop-blur md:px-4" role="note" aria-label="About this site">
  <TriangleAlert class="h-4 w-4 shrink-0 text-chiral" strokeWidth={2} />
  <p class="min-w-0 flex-1 text-[11px] leading-tight text-text-dim md:text-xs">
    <span class="font-bold uppercase tracking-[0.12em] text-chiral">Warning:</span> {WARNING}
  </p>
  <a href={GITHUB_URL} target="_blank" rel="noreferrer" class="micro expo t-hover flex shrink-0 items-center gap-1 border border-hairline px-2 py-1 font-bold text-white transition-colors hover:border-white/40">
    GITHUB<ArrowUpRight class="h-3.5 w-3.5" strokeWidth={2.25} />
  </a>
  <a href={WAITLIST_URL} class="micro expo t-hover hidden shrink-0 items-center gap-1 bg-chiral px-2.5 py-1 font-bold text-black transition-opacity hover:opacity-90 sm:flex">
    JOIN THE WAITLIST<ArrowUpRight class="h-3.5 w-3.5" strokeWidth={2.25} />
  </a>
</div>

<style>
  .strip {
    height: var(--strip);
    padding-bottom: env(safe-area-inset-bottom);
  }
</style>
