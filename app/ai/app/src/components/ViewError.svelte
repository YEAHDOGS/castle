<script>
  // Shown in place of a view that crashed while rendering, so it never just freezes.
  import { TriangleAlert, RotateCcw } from '@lucide/svelte'
  import { messageOf } from '../lib/errors.svelte.js'

  /** @type {{ error: unknown, reset: () => void }} */
  let { error, reset } = $props()
</script>

<div class="m-4 2xl:m-6 p-5 rounded-2xl glass border border-neon-red/40 flex flex-col gap-3 max-w-2xl">
  <p class="flex items-center gap-2 font-bold 2xl:text-xl"><TriangleAlert size={18} class="text-neon-red" /> Something broke on this page</p>
  <p class="text-sm 2xl:text-lg text-neutral-300 whitespace-pre-wrap break-words select-text">{messageOf(error)}</p>
  <button class="self-start flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-white/10 hover:border-neon-red/50 text-sm 2xl:text-lg" onclick={reset}>
    <RotateCcw size={14} /> Try again
  </button>
</div>
