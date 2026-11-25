<!-- BucketDoor: the bucket view before a bucket is connected. Asks for its address and keys,
     which go to the box once and stay there. -->
<script>
  import { Cloud, LoaderCircle } from '@lucide/svelte';
  import { store } from '../lib/store.svelte.js';

  const FIELD = 'expo t-hover h-10 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none';
  const FAILED = 'The bucket could not be connected';

  let endpoint = $state('');
  let bucket = $state('');
  let accessKeyId = $state('');
  let secretAccessKey = $state('');
  let publicUrl = $state('');
  let quotaGb = $state('');
  let busy = $state(false);
  let error = $state('');

  async function submit(e) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    error = '';
    try {
      await store.connectBucket({ endpoint, bucket, accessKeyId, secretAccessKey, publicUrl, quotaGb: Number(quotaGb) || 0 });
      secretAccessKey = '';
    } catch (err) {
      error = err?.message || FAILED;
    } finally {
      busy = false;
    }
  }
</script>

<div class="min-h-0 flex-1 overflow-y-auto px-4 py-6 md:px-6 2xl:py-10">
  <div class="mx-auto grid w-full max-w-5xl grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] 2xl:max-w-6xl 2xl:gap-10">
    <div class="min-w-0 space-y-3">
      <Cloud class="h-9 w-9 text-bridge 2xl:h-12 2xl:w-12" strokeWidth={1.25} />
      <h2 class="text-[20px] font-semibold text-white 2xl:text-[26px]">Connect a bucket</h2>
      <p class="text-[13px] text-text-dim 2xl:text-[15px]">
        A bucket is storage rented from a provider: Cloudflare R2, Backblaze B2, Amazon S3, or anything else that speaks the S3 API.
        Connect one and its files sit here beside My Drive, to upload, rename, move and delete.
      </p>
      <p class="text-[13px] text-text-dim 2xl:text-[15px]">
        The keys go to your box once and stay on it. The app never talks to the provider itself, and no screen shows the secret again.
      </p>
      <p class="micro text-text-faint">A BUCKET HAS NO TRASH: A DELETE THERE IS FOR GOOD.</p>
    </div>

    <form class="hud-corners glass min-w-0 space-y-4 p-5" onsubmit={submit}>
      <label class="block min-w-0">
        <span class="micro mb-1.5 block text-text-faint">S3 ADDRESS</span>
        <input bind:value={endpoint} type="text" required autocomplete="off" spellcheck="false" placeholder="https://<account>.r2.cloudflarestorage.com" class={FIELD} />
      </label>
      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <label class="block min-w-0">
          <span class="micro mb-1.5 block text-text-faint">BUCKET NAME</span>
          <input bind:value={bucket} type="text" required autocomplete="off" spellcheck="false" class={FIELD} />
        </label>
        <label class="block min-w-0">
          <span class="micro mb-1.5 block text-text-faint">SIZE LIMIT IN GB (OPTIONAL)</span>
          <input bind:value={quotaGb} type="number" min="0" step="any" inputmode="decimal" placeholder="such as 10" class={FIELD} />
        </label>
      </div>
      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <label class="block min-w-0">
          <span class="micro mb-1.5 block text-text-faint">ACCESS KEY ID</span>
          <input bind:value={accessKeyId} type="text" required autocomplete="off" spellcheck="false" class={FIELD} />
        </label>
        <label class="block min-w-0">
          <span class="micro mb-1.5 block text-text-faint">SECRET ACCESS KEY</span>
          <input bind:value={secretAccessKey} type="password" required autocomplete="off" spellcheck="false" class={FIELD} />
        </label>
      </div>
      <label class="block min-w-0">
        <span class="micro mb-1.5 block text-text-faint">PUBLIC ADDRESS (OPTIONAL)</span>
        <input bind:value={publicUrl} type="text" autocomplete="off" spellcheck="false" placeholder="https://data.example.com" class={FIELD} />
        <span class="mt-1.5 block text-[12px] text-text-faint">Where the bucket's files are public, if they are. With it, every file gets a link to copy.</span>
      </label>
      {#if error}
        <p class="text-[13px] text-chiral" role="alert">{error}</p>
      {/if}
      <button type="submit" disabled={busy} class="expo t-hover micro flex h-10 w-full items-center justify-center gap-2 bg-chiral font-bold text-black transition-all hover:brightness-110 disabled:opacity-50">
        {#if busy}<LoaderCircle class="h-4 w-4 animate-spin" strokeWidth={2} />{/if}
        {busy ? 'CHECKING THE KEYS' : 'CONNECT'}
      </button>
    </form>
  </div>
</div>
