<script>
  import { X, Download, Copy } from '@lucide/svelte'
  import { store } from '../lib/store.svelte.js'
  import { url } from '../lib/api.js'
  import Media from './Media.svelte'
  import AnimateButton from './AnimateButton.svelte'
  import { fade, scale } from 'svelte/transition'
  import { EASE, PAGE_MS } from '../lib/motion.js'

  const p = $derived(store.preview)
  const close = () => (store.preview = null)
  const onKey = (/** @type {KeyboardEvent} */ e) => { if (e.key === 'Escape') close() }
  const BTN = 'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs 2xl:text-base border border-white/10 hover:border-neon-red/50 hover:text-neon-red'
</script>

<svelte:window onkeydown={onKey} />

{#if p}
  <div class="fixed inset-0 z-50 bg-black/90 flex flex-col p-3 sm:p-6 gap-3" role="dialog" aria-modal="true" transition:fade={{ duration: PAGE_MS / 2 }}>
    <div class="flex items-center justify-between gap-3">
      <p class="text-xs sm:text-sm 2xl:text-lg text-neutral-300 line-clamp-2 select-text">{p.job.prompt}</p>
      <div class="flex gap-2 shrink-0">
        {#if p.output.mime.startsWith('image')}<AnimateButton file={p.output.file} url={p.output.url} label onQueued={close} />{/if}
        <button class={BTN} onclick={() => navigator.clipboard?.writeText(p.job.prompt)}><Copy size={14} /> Prompt</button>
        <a class={BTN} href={url(p.output.url, true)} download><Download size={14} /> Save</a>
        <button class={BTN} onclick={close} aria-label="Close"><X size={14} /></button>
      </div>
    </div>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="flex-1 min-h-0 flex items-center justify-center" in:scale={{ start: 0.94, duration: PAGE_MS, easing: EASE }} onclick={(e) => e.target === e.currentTarget && close()}>
      <Media src={url(p.output.url, true)} mime={p.output.mime} large />
    </div>
    <p class="text-[11px] 2xl:text-sm text-neutral-500 text-center">{p.job.modelLabel} · seed {p.job.seed} · {p.output.file}</p>
  </div>
{/if}
