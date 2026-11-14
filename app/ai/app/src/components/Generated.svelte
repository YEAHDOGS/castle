<script>
  import { Image, Clapperboard, AudioLines, Search, Repeat, Download, Trash2, Copy } from '@lucide/svelte'
  import { slide } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { store } from '../lib/store.svelte.js'
  import { url } from '../lib/api.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import Confirm from './Confirm.svelte'
  import AnimateButton from './AnimateButton.svelte'
  import Animations from './Animations.svelte'

  /** @type {{ onConvert: (item: any) => void }} */
  let { onConvert } = $props()

  const FILTERS = [{ id: '', label: 'All' }, { id: 'image', label: 'Images' }, { id: 'video', label: 'Videos' }, { id: 'audio', label: 'Audio' }]
  const ICONS = { image: Image, video: Clapperboard, audio: AudioLines }
  const DAY_MS = 86_400_000
  const ACTION = 'p-1.5 rounded-lg text-neutral-400 hover:text-white hover:bg-white/10 transition-colors'

  let filter = $state('')
  let query = $state('')
  /** @type {any} */ let pending = $state(null)

  const fIndex = $derived(FILTERS.findIndex((f) => f.id === filter))
  const items = $derived(store.media.filter((m) => (!filter || m.kind === filter) && (!query || m.job.prompt.toLowerCase().includes(query.toLowerCase()))))

  // Day headings: Today, Yesterday, then the date.
  const groups = $derived.by(() => {
    const today = new Date().setHours(0, 0, 0, 0)
    /** @type {{ label: string, items: any[] }[]} */
    const out = []
    for (const it of items) {
      const day = new Date(it.job.created).setHours(0, 0, 0, 0)
      const label = day === today ? 'Today' : day === today - DAY_MS ? 'Yesterday' : new Date(day).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric', year: 'numeric' })
      if (out.at(-1)?.label !== label) out.push({ label, items: [] })
      out.at(-1)?.items.push(it)
    }
    return out
  })

  const time = (/** @type {number} */ t) => new Date(t).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
  const fullDate = (/** @type {number} */ t) => new Date(t).toLocaleString()
  const ext = (/** @type {string} */ f) => f.split('.').pop()?.toUpperCase()

  async function save(/** @type {any} */ it) {
    const a = Object.assign(document.createElement('a'), { href: url(it.url, true), download: it.file.split('/').pop() })
    a.click()
  }
</script>

<div class="flex flex-col gap-3">
  <div class="flex flex-col sm:flex-row gap-2 sm:items-center">
    <div class="filters relative grid grid-cols-4 p-1 rounded-xl well text-xs 2xl:text-base" style="--i: {fIndex}">
      <span class="pill" aria-hidden="true"></span>
      {#each FILTERS as f (f.id)}
        <button class="relative z-10 px-3 py-1.5 font-semibold transition-colors {filter === f.id ? 'text-white' : 'text-muted hover:text-white'}"
          onclick={() => (filter = f.id)} aria-pressed={filter === f.id}>{f.label}</button>
      {/each}
    </div>
    <label class="flex-1 flex items-center gap-2 well focus-within:border-neon-red/50 rounded-xl px-3 py-2">
      <Search size={15} class="text-muted" />
      <input class="flex-1 min-w-0 bg-transparent outline-none text-sm 2xl:text-lg" placeholder="Search prompts" bind:value={query} />
    </label>
  </div>

  {#if !items.length}
    <p class="text-sm text-muted py-6 text-center">{store.media.length ? 'Nothing matches.' : 'Nothing generated yet. Head to Create.'}</p>
  {/if}

  {#each groups as g (g.label)}
    <h4 class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted mt-1" transition:slide={{ duration: 250, easing: EASE }}>{g.label}</h4>
    <ul class="flex flex-col gap-2">
      {#each g.items as it (it.file)}
        {@const Icon = ICONS[it.kind] || Image}
        <li class="row flex gap-3 p-2 rounded-xl bg-black/25 border border-white/5 hover:border-white/15"
          animate:flip={{ duration: PAGE_MS, easing: EASE }} transition:slide={{ duration: 250, easing: EASE }}>
          <button class="thumb shrink-0 w-20 h-20 2xl:w-28 2xl:h-28 rounded-lg overflow-hidden bg-black/50 flex items-center justify-center"
            onclick={() => (store.preview = { job: it.job, output: it })} aria-label="Open">
            {#if it.kind === 'image'}
              <img src={url(it.url, true)} alt="" loading="lazy" class="w-full h-full object-cover" />
            {:else if it.kind === 'video'}
              <video src={url(it.url, true) + '#t=0.1'} muted preload="metadata" class="w-full h-full object-cover"></video>
            {:else}
              <Icon size={26} class="text-neutral-400" />
            {/if}
          </button>
          {#if it.kind === 'image'}<Animations file={it.file} />{/if}
          <div class="flex-1 min-w-0 flex flex-col gap-1">
            <p class="text-sm 2xl:text-lg text-neutral-100 line-clamp-2 select-text" title={it.job.prompt}>{it.job.prompt}</p>
            <p class="text-[11px] 2xl:text-sm text-neutral-500 truncate" title={fullDate(it.job.created)}>
              <Icon size={11} class="inline -mt-0.5" /> {time(it.job.created)} · {it.job.modelLabel} · {ext(it.file)} · seed {it.job.seed}
            </p>
            <div class="flex items-center gap-1 mt-auto">
              {#if it.kind === 'image'}<AnimateButton file={it.file} url={it.url} />{/if}
              <button class={ACTION} title="Convert / export" aria-label="Convert" onclick={() => onConvert(it)}><Repeat size={15} /></button>
              <button class={ACTION} title="Save original" aria-label="Save" onclick={() => save(it)}><Download size={15} /></button>
              <button class={ACTION} title="Copy prompt" aria-label="Copy prompt" onclick={() => navigator.clipboard?.writeText(it.job.prompt)}><Copy size={15} /></button>
              <button class="{ACTION} hover:!bg-neon-red ml-auto" title="Delete" aria-label="Delete" onclick={() => (pending = it)}><Trash2 size={15} /></button>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/each}
</div>

{#if pending}
  <Confirm title="Delete this {pending.kind}?" message="Deletes the file and its history entry (prompt, settings). This can't be undone."
    items={pending.job.outputs.map((/** @type {any} */ o) => `outputs/${o.file}`)}
    onConfirm={async () => { await store.removeJob(pending.job.id); pending = null }} onCancel={() => (pending = null)} />
{/if}

<style>
  .pill {
    position: absolute;
    top: 0.25rem;
    bottom: 0.25rem;
    left: 0.25rem;
    width: calc((100% - 0.5rem) / 4);
    border-radius: 0.6rem;
    background: rgb(255 51 68 / 16%);
    border: 1px solid rgb(255 51 68 / 40%);
    transform: translateX(calc(var(--i) * 100%));
    transition: transform 0.45s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .row {
    transition: border-color 0.3s;
  }

  .thumb img,
  .thumb video {
    transition: transform 0.5s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .row:hover .thumb img,
  .row:hover .thumb video {
    transform: scale(1.06);
  }
</style>
