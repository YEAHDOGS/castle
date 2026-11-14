<script>
  import { onMount } from 'svelte'
  import { store } from '../lib/store.svelte.js'
  import { buildTiles } from '../lib/library.js'
  import { size } from '../lib/format.js'
  import Section from './Section.svelte'
  import StorageBar from './StorageBar.svelte'
  import ModelTiles from './ModelTiles.svelte'
  import Generated from './Generated.svelte'
  import DownloadPanel from './DownloadPanel.svelte'
  import Converter from './Converter.svelte'
  import StoragePanel from './StoragePanel.svelte'
  import Characters from './Characters.svelte'
  import Prompts from './Prompts.svelte'

  const LLM_POLL_MS = 5000

  let downloadsOpen = $state(false)
  let modelsOpen = $state(false)
  let generatedOpen = $state(true)
  let storageOpen = $state(true)
  let charactersOpen = $state(true)
  let promptsOpen = $state(true)
  let promptCount = $state(0)
  let storageTotal = $state(0)
  /** @type {any} */ let converting = $state(null)

  const tileCount = $derived(buildTiles(store.catalog, store.llm).length)

  // Keep "loaded / VRAM / context" live while the Library is open.
  onMount(() => {
    store.loadLlm().catch(() => {})
    const t = setInterval(() => store.loadLlm().catch(() => {}), LLM_POLL_MS)
    return () => clearInterval(t)
  })
</script>

<div class="flex flex-col gap-3 2xl:gap-5 max-w-7xl 2xl:max-w-none mx-auto w-full">
  <Section title="Download models" bind:open={downloadsOpen}>
    {#snippet summary()}Search Hugging Face: image, video, audio and text models{/snippet}
    <DownloadPanel />
  </Section>

  <Section title="Models" bind:open={modelsOpen}>
    {#snippet summary()}{tileCount} · {size((store.catalog.storage?.bytes || 0) + (store.llm.disk?.bytes || 0))}{/snippet}
    {#snippet peek()}
      <div class="flex flex-col gap-4">
        <StorageBar />
        <ModelTiles expanded={modelsOpen} />
      </div>
    {/snippet}
    {#if tileCount}<p class="text-[11px] 2xl:text-sm text-neutral-600">Context windows show configured / model maximum. Hover a tile to delete it.</p>{/if}
  </Section>

  <Section title="Storage" bind:open={storageOpen}>
    {#snippet summary()}{storageTotal ? `${size(storageTotal)} on disk` : 'Where your disk space goes'}{/snippet}
    <StoragePanel onReport={(r) => (storageTotal = r.total)} />
  </Section>

  <Section title="Characters" bind:open={charactersOpen}>
    {#snippet summary()}Personas for chat{/snippet}
    <Characters />
  </Section>

  <Section title="Prompts" bind:open={promptsOpen}>
    {#snippet summary()}{promptCount} saved{/snippet}
    <Prompts onCount={(n) => (promptCount = n)} />
  </Section>

  <Section title="Generated" bind:open={generatedOpen}>
    {#snippet summary()}{store.media.length} items{/snippet}
    <Generated onConvert={(it) => (converting = it)} />
  </Section>
</div>

{#if converting}<Converter item={converting} onClose={() => (converting = null)} />{/if}
