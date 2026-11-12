<script>
  // Destructive-action confirmation. Lists exactly what will be removed; the red button needs a deliberate click
  // (Enter does not confirm, Escape cancels).
  import { TriangleAlert, LoaderCircle } from '@lucide/svelte'
  import { fade, fly } from 'svelte/transition'
  import { EASE, PAGE_MS } from '../lib/motion.js'

  /**
   * @type {{ title: string, message: string, items?: string[], confirmLabel?: string,
   *          onConfirm: () => Promise<void> | void, onCancel: () => void }}
   */
  let { title, message, items = [], confirmLabel = 'Delete', onConfirm, onCancel } = $props()
  let busy = $state(false)
  let error = $state('')
  const MAX_ITEMS = 8

  async function confirm() {
    busy = true
    error = ''
    try { await onConfirm() } catch (e) { error = String(e.message || e); busy = false }
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !busy && onCancel()} />

<div class="scrim z-[60] flex items-center justify-center p-4" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" transition:fade={{ duration: PAGE_MS / 2 }}>
  <div class="w-full max-w-md 2xl:max-w-xl glass-panel border-neon-red/30 rounded-2xl p-5 flex flex-col gap-3 shadow-[0_0_40px_rgb(255_51_68/15%)]" in:fly={{ y: 20, duration: PAGE_MS, easing: EASE }}>
    <h2 id="confirm-title" class="flex items-center gap-2 font-bold text-neon-red"><TriangleAlert size={18} /> {title}</h2>
    <p class="text-sm 2xl:text-lg text-neutral-300">{message}</p>
    {#if items.length}
      <ul class="text-[11px] 2xl:text-sm font-mono text-neutral-400 bg-black/40 rounded-lg p-2 max-h-40 overflow-y-auto">
        {#each items.slice(0, MAX_ITEMS) as it (it)}<li class="break-all">{it}</li>{/each}
        {#if items.length > MAX_ITEMS}<li class="text-neutral-600">…and {items.length - MAX_ITEMS} more</li>{/if}
      </ul>
    {/if}
    {#if error}<p class="text-xs text-neon-red break-words">{error}</p>{/if}
    <div class="flex justify-end gap-2 mt-1">
      <button class="px-4 py-2 rounded-xl border border-white/10 hover:border-white/30" onclick={onCancel} disabled={busy}>Cancel</button>
      <button class="px-4 py-2 rounded-xl font-bold licorice flex items-center gap-2 disabled:opacity-60" onclick={confirm} disabled={busy}>
        {#if busy}<LoaderCircle size={16} class="animate-spin" />{/if}{confirmLabel}
      </button>
    </div>
  </div>
</div>
