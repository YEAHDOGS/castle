<!-- BucketBar: above the files of a connected bucket. Which bucket it is, how full, what is
     on its way up, the Catalog button when the bucket keeps one, and Disconnect. -->
<script>
  import { Cloud, ListMusic, LoaderCircle, Unplug } from '@lucide/svelte';
  import { store } from '../lib/store.svelte.js';
  import { fmtSize } from '../lib/format.js';
  import { CATALOG_KEY, parseCatalog } from '../lib/catalog.js';
  import BucketCatalog from './BucketCatalog.svelte';

  /** usage: { used, objects, quota, complete } from the box, or null before the first reading. */
  let { usage = null } = $props();

  let catalog = $state(null); // the bucket's music catalog, when it keeps one
  let open = $state(false);

  const BUTTON = 'expo t-hover micro flex h-9 shrink-0 items-center gap-2 border border-hairline px-3 text-text-dim transition-colors hover:border-chiral/60 hover:text-white';

  const held = $derived.by(() => {
    if (!usage) return '';
    const size = usage.quota ? `${fmtSize(usage.used)} OF ${fmtSize(usage.quota)}` : fmtSize(usage.used);
    const count = `${usage.objects.toLocaleString()}${usage.complete ? '' : '+'} ${usage.objects === 1 ? 'FILE' : 'FILES'}`;
    return `${size} · ${count}`;
  });

  // The catalog is looked for once per bucket: one small read.
  $effect(() => {
    const name = store.bucket?.bucket;
    const adapter = store.adapter;
    catalog = null;
    if (!name || !adapter.bucketRead) return;
    adapter.bucketRead(CATALOG_KEY).then((text) => {
      if (store.bucket?.bucket === name) catalog = parseCatalog(text);
    }).catch(() => {}); // no catalog to offer: the bucket is still a bucket
  });
</script>

<div class="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-b border-hairline-faint bg-black/30 px-4 py-2 md:px-6 2xl:py-3">
  <div class="flex min-w-0 flex-1 items-center gap-2.5">
    <Cloud class="h-4 w-4 shrink-0 text-bridge 2xl:h-5 2xl:w-5" strokeWidth={1.75} />
    <div class="min-w-0">
      <p class="truncate text-[13px] font-semibold text-white 2xl:text-[15px]">{store.bucket.bucket}</p>
      <p class="micro truncate text-text-faint">
        {held || 'ADDING IT UP'}{#if store.bucket.publicUrl}<span class="hidden sm:inline">{' · '}{store.bucket.publicUrl.replace(/^https:\/\//, '').toUpperCase()}</span>{/if}
      </p>
    </div>
  </div>
  {#if store.uploading}
    <p class="micro flex items-center gap-2 text-bridge" role="status">
      <LoaderCircle class="h-3.5 w-3.5 animate-spin" strokeWidth={2} />
      UPLOADING {store.uploading}
    </p>
  {/if}
  {#if catalog}
    <button class={BUTTON} onclick={() => (open = true)}>
      <ListMusic class="h-4 w-4" strokeWidth={1.75} />
      <span>CATALOG<span class="hidden md:inline">{' · '}{catalog.tracks.length} {catalog.tracks.length === 1 ? 'SONG' : 'SONGS'}</span></span>
    </button>
  {/if}
  <button class={BUTTON} onclick={() => store.disconnectBucket()} aria-label="Disconnect this bucket" title="Disconnect this bucket">
    <Unplug class="h-4 w-4" strokeWidth={1.75} />
    <span class="hidden lg:inline">DISCONNECT</span>
  </button>
</div>

{#if open && catalog}
  <BucketCatalog {catalog} onsaved={(next) => (catalog = next)} onclose={() => (open = false)} />
{/if}
