<script>
  import { onMount } from 'svelte'
  import { LoaderCircle, Unplug } from '@lucide/svelte'
  import { store } from '../lib/store.svelte.js'
  import { settings } from '../lib/api.js'
  import SwapText from './SwapText.svelte'

  /** @type {{ onSettings: () => void }} */
  let { onSettings } = $props()

  // The desktop app starts (and restarts) the engine itself, so a short outage is normal: keep retrying quietly
  // and only show help once it has clearly not come back.
  const RETRY_MS = 2000
  const PATIENCE_MS = 15_000
  let waited = $state(0)
  const patient = $derived(waited < PATIENCE_MS)

  onMount(() => {
    const t0 = Date.now()
    const timer = setInterval(() => {
      waited = Date.now() - t0
      if (!store.connecting) store.connect()
    }, RETRY_MS)
    return () => clearInterval(timer)
  })
</script>

<div class="flex-1 flex items-center justify-center p-6">
  <div class="max-w-md 2xl:max-w-2xl text-center flex flex-col items-center gap-4 text-sm 2xl:text-xl">
    {#if patient}
      <LoaderCircle size={40} class="text-neon-red animate-spin" />
      <h2 class="text-xl 2xl:text-4xl font-bold"><SwapText text="Starting engine…" /></h2>
      <p class="text-muted">Connecting to the local generation engine.</p>
    {:else}
      <Unplug size={40} class="text-neon-red" />
      <h2 class="text-xl 2xl:text-4xl font-bold">The engine isn't running</h2>
      <p class="text-muted">Tried <code>{settings.server || location.origin}</code>{store.error ? `: ${store.error}` : ''}. Still retrying.</p>
      <p class="text-neutral-400">The desktop app starts it by itself. In a browser, start it from the project folder:</p>
      <code class="bg-black/50 rounded-lg px-4 py-2 select-text">dogs serve</code>
      <button class="px-4 py-2 rounded-xl border border-white/10 hover:border-neon-red/50" onclick={onSettings}>Settings</button>
    {/if}
  </div>
</div>
