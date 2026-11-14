<script>
  import { Sparkles, Dice5, LoaderCircle } from '@lucide/svelte'
  import { store } from '../lib/store.svelte.js'
  import ParamField from './ParamField.svelte'
  import SwapText from './SwapText.svelte'
  import { textIn } from '../lib/motion.js'

  const INPUT = 'w-full well rounded-lg px-3 py-2 text-sm 2xl:text-lg outline-none'
  const PROMPT_ROWS = 5

  let prompt = $state('')
  let seed = $state('')
  let busy = $state(false)
  let error = $state('')
  /** @type {Record<string, any>} */
  let values = $state({})

  const model = $derived(store.model)
  // Long text params (lyrics, negative prompt) go under the prompt; numbers and selects go in a grid.
  const textParams = $derived(Object.entries(model?.params || {}).filter(([, s]) => s.type === 'text'))
  const imageParams = $derived(Object.entries(model?.params || {}).filter(([, s]) => s.type === 'image'))
  const gridParams = $derived(Object.entries(model?.params || {}).filter(([, s]) => s.type !== 'text' && s.type !== 'image'))

  $effect(() => {
    values = Object.fromEntries(Object.entries(model?.params || {}).map(([k, s]) => [k, s.default]))
  })

  // An image sent over from the Library's "Animate" becomes the start frame.
  $effect(() => {
    if (!store.startFrame || !imageParams.some(([k]) => k === 'start_image')) return
    values.start_image = store.startFrame.file
    store.startFrame = null
  })

  // A prompt picked in the Library's Prompts fills the prompt box.
  $effect(() => {
    if (store.draftPrompt === null) return
    prompt = store.draftPrompt
    store.draftPrompt = null
  })

  async function submit(/** @type {SubmitEvent} */ e) {
    e.preventDefault()
    if (!model || !prompt.trim() || busy) return
    busy = true
    error = ''
    try {
      await store.generate({ model: model.id, prompt, ...values, seed: seed === '' ? undefined : Number(seed) })
    } catch (err) {
      error = String(err.message || err)
    } finally {
      busy = false
    }
  }

  const onKey = (/** @type {KeyboardEvent} */ e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) /** @type {HTMLFormElement} */ (e.currentTarget.form)?.requestSubmit()
  }
</script>

<form class="flex flex-col gap-4 2xl:gap-6" onsubmit={submit}>
  <label class="flex flex-col gap-1.5">
    <span class="text-xs 2xl:text-base uppercase tracking-wider text-muted">Model</span>
    <select class={INPUT} value={model?.id} onchange={(e) => (store.selected[store.kind] = e.currentTarget.value)}>
      {#each store.ready as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
    </select>
    {#if model?.blurb}{#key model.id}<span class="text-[11px] 2xl:text-sm text-neutral-500" in:textIn>{model.blurb}</span>{/key}{/if}
    {#if model?.requiresNodes?.length}
      <span class="text-[11px] 2xl:text-sm text-amber-400">Needs ComfyUI node {model.requiresNodes.join(', ')}: run <code>dogs comfy add-node gguf</code></span>
    {/if}
  </label>

  {#each imageParams as [k, s] (k)}
    <ParamField name={k} spec={s} bind:value={values[k]} />
  {/each}

  <label class="flex flex-col gap-1.5">
    <span class="text-xs 2xl:text-base uppercase tracking-wider text-muted">{model?.promptLabel || 'Prompt'}</span>
    <textarea class="{INPUT} resize-y min-h-24" rows={PROMPT_ROWS} bind:value={prompt} onkeydown={onKey}
      placeholder={store.kind === 'audio' ? 'lo-fi hip hop, warm rhodes, vinyl crackle' : store.kind === 'video' ? 'a drone shot over neon skyscrapers at night, rain' : 'skyscrapers at dusk, cinematic, wet streets reflecting neon'}
    ></textarea>
  </label>

  {#each textParams as [k, s] (k)}
    <ParamField name={k} spec={s} bind:value={values[k]} />
  {/each}

  <div class="grid grid-cols-2 gap-3">
    {#each gridParams as [k, s] (k)}
      <ParamField name={k} spec={s} bind:value={values[k]} />
    {/each}
    <label class="flex flex-col gap-1">
      <span class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted">Seed</span>
      <div class="flex gap-1">
        <input class={INPUT} type="number" min="0" placeholder="random" bind:value={seed} />
        <button type="button" class="px-2 rounded-lg border border-white/10 hover:border-neon-red/50" title="Random seed" aria-label="Random seed"
          onclick={() => (seed = '')}><Dice5 size={16} /></button>
      </div>
    </label>
  </div>

  <button type="submit" disabled={!prompt.trim() || busy}
    class="flex items-center justify-center gap-2 py-3 2xl:py-4 rounded-xl font-bold tracking-wide licorice disabled:opacity-40 disabled:cursor-not-allowed transition-colors">
    {#if busy}<LoaderCircle size={18} class="animate-spin" />{:else}<Sparkles size={18} />{/if}
    <SwapText text={busy ? 'Queuing…' : `Generate ${store.kind}`} />
  </button>
  <span class="text-[11px] 2xl:text-sm text-neutral-600 -mt-2 text-center">Ctrl+Enter to generate</span>
  {#if error}<p class="text-sm text-neon-red break-words">{error}</p>{/if}
</form>
