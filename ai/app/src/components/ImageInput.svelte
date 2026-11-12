<script>
  // Optional input image (e.g. a video's start frame). Value is either a data: URL (picked or dropped file) or an
  // outputs-relative path (an image from the Library); the server accepts both.
  import { ImagePlus, X } from '@lucide/svelte'
  import { scale } from 'svelte/transition'
  import { url } from '../lib/api.js'
  import { EASE } from '../lib/motion.js'

  // No fallback: Svelte refuses bind:value={undefined} into a prop that has one, and "no image yet" is undefined.
  /** @type {{ label: string, value?: string }} */
  let { label, value = $bindable() } = $props()

  const ACCEPT = 'image/png,image/jpeg,image/webp'
  const MAX_BYTES = 25 * 2 ** 20
  let dragging = $state(false)
  let error = $state('')
  /** @type {HTMLInputElement} */ let picker

  const preview = $derived(!value ? '' : value.startsWith('data:') ? value : url(`/files/${value}`, true))

  function take(/** @type {File | undefined} */ file) {
    error = ''
    if (!file) return
    if (!ACCEPT.split(',').includes(file.type)) { error = 'Use a PNG, JPG or WebP image.'; return }
    if (file.size > MAX_BYTES) { error = 'Images up to 25 MB.'; return }
    const r = new FileReader()
    r.onload = () => (value = String(r.result))
    r.readAsDataURL(file)
  }

  function drop(/** @type {DragEvent} */ e) {
    e.preventDefault()
    dragging = false
    take(e.dataTransfer?.files[0])
  }
</script>

<div class="flex flex-col gap-1">
  <span class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted">{label}</span>
  {#if value}
    <div class="relative rounded-xl overflow-hidden border border-neon-red/30 bg-black/40" in:scale={{ start: 0.95, duration: 300, easing: EASE }}>
      <img src={preview} alt="Start frame" class="w-full max-h-48 object-contain" />
      <button type="button" class="absolute top-2 right-2 p-1.5 rounded-lg bg-black/70 hover:bg-neon-red" onclick={() => (value = '')} aria-label="Remove image"><X size={14} /></button>
    </div>
  {:else}
    <button type="button"
      class="flex flex-col items-center justify-center gap-1.5 py-5 rounded-xl border border-dashed text-xs 2xl:text-base transition-colors
        {dragging ? 'border-neon-red bg-neon-red/10 text-white' : 'border-white/15 text-muted hover:text-white hover:border-white/30'}"
      onclick={() => picker.click()}
      ondragover={(e) => { e.preventDefault(); dragging = true }}
      ondragleave={() => (dragging = false)}
      ondrop={drop}>
      <ImagePlus size={20} />
      <span>Drop an image or click to pick. Leave empty for text-to-video.</span>
    </button>
  {/if}
  <input bind:this={picker} type="file" accept={ACCEPT} class="hidden" onchange={(e) => take(e.currentTarget.files?.[0])} />
  {#if error}<span class="text-xs text-neon-red">{error}</span>{/if}
</div>
