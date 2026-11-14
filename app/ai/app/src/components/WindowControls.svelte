<script>
  // Minimize / maximize / close for the frameless desktop window on Windows and Linux (macOS keeps its traffic lights).
  import { onMount } from 'svelte'
  import { Minus, Square, Copy, X } from '@lucide/svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'

  const win = getCurrentWindow()
  let maximized = $state(false)

  onMount(() => {
    const sync = async () => { maximized = await win.isMaximized() }
    sync()
    const off = win.onResized(sync)
    return () => { off.then((f) => f()) }
  })

  const BTN = 'w-[46px] self-stretch grid place-items-center text-neutral-300 transition-colors'
</script>

<div class="flex self-stretch -my-px">
  <button class="{BTN} hover:bg-white/10" onclick={() => win.minimize()} title="Minimize" aria-label="Minimize"><Minus size={16} /></button>
  <button class="{BTN} hover:bg-white/10" onclick={() => win.toggleMaximize()} title={maximized ? 'Restore' : 'Maximize'} aria-label={maximized ? 'Restore' : 'Maximize'}>
    {#if maximized}<Copy size={13} class="-scale-x-100" />{:else}<Square size={13} />{/if}
  </button>
  <button class="{BTN} hover:bg-[#e81123] hover:text-white" onclick={() => win.close()} title="Close" aria-label="Close"><X size={17} /></button>
</div>
