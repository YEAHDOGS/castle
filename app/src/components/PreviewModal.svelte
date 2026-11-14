<!-- PreviewModal: file preview in a HUD frame. Images, text, or a deadpan fallback. -->
<script>
  import { X, FileQuestion } from '@lucide/svelte';
  import { scale, fade } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { fmtSize, fmtDate } from '../lib/format.js';
  import { modalIn, reducedMotion } from '../lib/motion.js';

  const node = $derived(store.previewNode);
  const url = $derived(node ? store.adapter.objectUrl(node) : null);
  const isImage = $derived(!!node && (node.mime || '').startsWith('image/'));
  const isText = $derived(!!node && !!node.text);
</script>

{#if node}
  <div class="fixed inset-0 z-50 grid place-items-center p-4" role="dialog" aria-modal="true" aria-label={node.name}>
    <div
      transition:fade={{ duration: reducedMotion() ? 0 : 200 }}
      class="absolute inset-0 bg-black/70"
      onclick={() => store.closePreview()}
      onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') store.closePreview(); }}
      role="button"
      tabindex="-1"
      aria-label="Close preview"
    ></div>
    <div
      transition:scale={modalIn()}
      class="hud-corners scanlines glass relative z-10 flex max-h-[86dvh] w-full max-w-3xl flex-col"
      style="transform-origin: center"
    >
      <!-- header -->
      <div class="flex items-center gap-3 border-b border-hairline px-5 py-3.5">
        <div class="min-w-0 flex-1">
          <p class="truncate text-[15px] font-semibold text-white">{node.name}</p>
          <p class="micro mt-0.5 text-text-faint">{fmtSize(node.size)} · {fmtDate(node.modified).toUpperCase()}</p>
        </div>
        <button
          class="expo t-hover p-2 text-text-dim transition-colors hover:text-white"
          onclick={() => store.closePreview()}
          aria-label="Close preview"
        >
          <X class="h-5 w-5" strokeWidth={1.75} />
        </button>
      </div>
      <!-- body -->
      <div class="grid min-h-0 flex-1 place-items-center overflow-auto bg-black/50 p-6">
        {#if isImage && url}
          <img src={url} alt={node.name} class="max-h-[62dvh] max-w-full border border-hairline object-contain" draggable="false" />
        {:else if isText}
          <pre class="micro w-full whitespace-pre-wrap normal-case tracking-normal text-text-dim">{node.text}</pre>
        {:else}
          <div class="flex flex-col items-center gap-3 py-10">
            <FileQuestion class="h-10 w-10 text-text-faint" strokeWidth={1.25} />
            <p class="micro text-text-faint">NO PREVIEW</p>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
