<!-- ConnectModal: point the app at a Castle box (address + token), or go back to the local drive. -->
<script>
  import { X, Server } from '@lucide/svelte';
  import { scale, fade } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { DEFAULT_BOX_URL } from '../lib/box.js';
  import { modalIn, reducedMotion } from '../lib/motion.js';

  let url = $state(DEFAULT_BOX_URL);
  let token = $state('');
  let busy = $state(false);
  let error = $state('');

  async function submit(e) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    error = '';
    try {
      await store.connect(url, token);
      token = '';
    } catch (err) {
      error = err?.message || 'Could not connect';
    }
    busy = false;
  }
</script>

{#if store.connectOpen}
  <div class="fixed inset-0 z-50 grid place-items-center p-4" role="dialog" aria-modal="true" aria-label="Connect to a Castle box">
    <div
      transition:fade={{ duration: reducedMotion() ? 0 : 200 }}
      class="absolute inset-0 bg-black/70"
      onclick={() => (store.connectOpen = false)}
      onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') store.connectOpen = false; }}
      role="button"
      tabindex="-1"
      aria-label="Close"
    ></div>
    <div transition:scale={modalIn()} class="hud-corners scanlines glass relative z-10 w-full max-w-md" style="transform-origin: center">
      <div class="flex items-center gap-3 border-b border-hairline px-5 py-3.5">
        <Server class="h-5 w-5 text-chiral" strokeWidth={1.75} />
        <p class="min-w-0 flex-1 text-[15px] font-semibold text-white">Castle box</p>
        <button class="expo t-hover p-2 text-text-dim transition-colors hover:text-white" onclick={() => (store.connectOpen = false)} aria-label="Close">
          <X class="h-5 w-5" strokeWidth={1.75} />
        </button>
      </div>

      {#if store.boxUrl}
        <div class="space-y-4 p-5">
          <p class="micro text-text-faint">CONNECTED TO</p>
          <p class="break-all text-[14px] text-bridge">{store.boxUrl}</p>
          <p class="text-[13px] text-text-dim">Files you see are on this box. Disconnecting returns to the local drive and forgets the token on this device.</p>
          <button class="expo t-hover micro h-10 w-full border border-hairline font-bold text-white transition-colors hover:border-chiral/60" onclick={() => store.disconnect()}>
            DISCONNECT
          </button>
        </div>
      {:else}
        <form class="space-y-4 p-5" onsubmit={submit}>
          {#if store.isDemo}
            <p class="text-[13px] text-text-dim">This is the demo: sample files that reset on every reload. Connect to a box to leave it and store files for real.</p>
          {:else}
            <p class="text-[13px] text-text-dim">Files here stay in this browser. Connect to a box to store them for real, or open the <a class="text-bridge underline" href="/demo">demo</a> to try sample files.</p>
          {/if}
          <label class="block">
            <span class="micro mb-1.5 block text-text-faint">BOX ADDRESS</span>
            <input
              bind:value={url}
              type="text"
              autocomplete="off"
              spellcheck="false"
              class="expo t-hover h-10 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white focus:border-chiral/60 focus:outline-none"
            />
          </label>
          <label class="block">
            <span class="micro mb-1.5 block text-text-faint">BOX TOKEN</span>
            <input
              bind:value={token}
              type="password"
              autocomplete="off"
              spellcheck="false"
              placeholder="printed when the box starts"
              class="expo t-hover h-10 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none"
            />
          </label>
          {#if error}
            <p class="text-[13px] text-chiral" role="alert">{error}</p>
          {/if}
          <button
            type="submit"
            disabled={busy}
            class="expo t-hover micro h-10 w-full bg-chiral font-bold text-black transition-all hover:brightness-110 disabled:opacity-50"
          >
            {busy ? 'CONNECTING' : 'CONNECT'}
          </button>
        </form>
      {/if}
    </div>
  </div>
{/if}
