<script>
  import { X, DatabaseZap } from '@lucide/svelte'
  import { api, settings } from '../lib/api.js'
  import { store } from '../lib/store.svelte.js'
  import { fade, fly } from 'svelte/transition'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import Confirm from './Confirm.svelte'

  /** @type {{ onClose: () => void }} */
  let { onClose } = $props()

  let server = $state(settings.server)
  let key = $state(settings.key)
  const INPUT = 'w-full well rounded-lg px-3 py-2 text-sm outline-none'

  function save(/** @type {SubmitEvent} */ e) {
    e.preventDefault()
    settings.server = server
    settings.key = key
    store.connect()
    onClose()
  }

  let confirmClear = $state(false)
  const CLEARED = ['Generation history (Library, Generated)', 'Prompts you saved (the built-in ones stay)', 'Chats and their messages', 'Characters and their lists']

  // Every view holds rows from the database, so start the app over rather than emptying each one.
  async function clearDatabase() {
    await api('/api/database/clear', { method: 'POST' })
    location.reload()
  }
</script>

<div class="scrim z-50 flex items-center justify-center p-4" role="dialog" aria-modal="true" transition:fade={{ duration: PAGE_MS / 2 }}>
  <form in:fly={{ y: 20, duration: PAGE_MS, easing: EASE }} class="w-full max-w-md 2xl:max-w-xl glass-panel rounded-2xl p-5 flex flex-col gap-4" onsubmit={save}>
    <div class="flex items-center justify-between">
      <h2 class="font-bold tracking-wide">Settings</h2>
      <button type="button" onclick={onClose} aria-label="Close" class="hover:text-neon-red"><X size={18} /></button>
    </div>
    <label class="flex flex-col gap-1.5 text-xs uppercase tracking-wider text-muted">
      Server URL
      <input class={INPUT} bind:value={server} placeholder="http://127.0.0.1:8787 (empty = this page's server)" />
    </label>
    <label class="flex flex-col gap-1.5 text-xs uppercase tracking-wider text-muted">
      API key
      <input class={INPUT} type="password" bind:value={key} placeholder="only if the server sets DOGS_API_KEY" />
    </label>
    <p class="text-[11px] text-neutral-500">Point this at another machine running <code>dogs serve --host 0.0.0.0</code> to use its GPU.</p>
    <button class="py-2.5 rounded-xl font-bold licorice">Save &amp; reconnect</button>
    <div class="flex items-center gap-3 pt-4 border-t border-white/8">
      <div class="flex-1 min-w-0">
        <p class="text-xs uppercase tracking-wider text-muted">Database</p>
        <p class="text-[11px] text-neutral-500">History, your prompts, chats and characters on this server. Built-in prompts and generated files stay.</p>
      </div>
      <button type="button" onclick={() => (confirmClear = true)}
        class="shrink-0 flex items-center gap-1.5 px-3 py-2 rounded-xl text-sm font-semibold border border-neon-red/40 text-neon-red hover:bg-neon-red hover:text-white transition-colors">
        <DatabaseZap size={15} /> Clear database…
      </button>
    </div>
  </form>
</div>

{#if confirmClear}
  <Confirm title="Clear the database?" confirmLabel="Clear database"
    message="Removes everything below from this server's database. The built-in prompts and the files in outputs/ (your images, videos, audio and uploads) are kept. This can't be undone."
    items={CLEARED} onConfirm={clearDatabase} onCancel={() => (confirmClear = false)} />
{/if}
