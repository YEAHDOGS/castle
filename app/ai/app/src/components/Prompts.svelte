<script>
  import { onMount } from 'svelte'
  import { Search, Plus, Send, Copy, Check, Trash2, Image, Clapperboard, AudioLines, MessageSquare } from '@lucide/svelte'
  import { slide } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { api } from '../lib/api.js'
  import { store, KINDS } from '../lib/store.svelte.js'
  import { CATEGORIES, SPECIFICITY, categoryOf, specificityOf, ago } from '../lib/prompts.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import Confirm from './Confirm.svelte'

  /** @type {{ onCount?: (n: number) => void }} */
  let { onCount } = $props()

  const GROUPINGS = [{ id: 'category', label: 'Category' }, { id: 'specificity', label: 'Specificity' }]
  const KIND_ICONS = { image: Image, video: Clapperboard, audio: AudioLines, chat: MessageSquare }
  const INPUT = 'well rounded-lg px-3 py-2 text-sm 2xl:text-lg outline-none'
  const ACTION = 'p-1.5 rounded-lg text-neutral-400 hover:text-white hover:bg-white/10 transition-colors'
  const COPIED_MS = 1500

  /** @type {any[]} */ let list = $state([])
  let query = $state('')
  let category = $state('')
  let grouping = $state('category')
  let error = $state('')
  /** @type {string | null} */ let open = $state(null)
  /** @type {string | null} */ let copied = $state(null)
  /** @type {any} */ let pending = $state(null)
  let adding = $state(false)
  let draft = $state({ text: '', title: '', category: 'other' })

  const counts = $derived(Object.fromEntries(CATEGORIES.map((c) => [c.id, list.filter((p) => p.category === c.id).length])))
  const items = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return list.filter((p) => (!category || p.category === category)
      && (!q || [p.text, p.title, p.notes].some((s) => s?.toLowerCase().includes(q))))
  })
  // The list is already most-recently-used first; grouping keeps that order inside each group.
  const groups = $derived(
    (grouping === 'category' ? CATEGORIES : SPECIFICITY)
      .map((g) => ({ ...g, items: items.filter((p) => (grouping === 'category' ? p.category : specificityOf(p.text).id) === g.id) }))
      .filter((g) => g.items.length),
  )

  async function load() {
    try {
      list = await api('/api/prompts')
      onCount?.(list.length)
    } catch (e) { error = String(e.message || e) }
  }

  async function add(/** @type {SubmitEvent} */ e) {
    e.preventDefault()
    if (!draft.text.trim()) return
    await api('/api/prompts', { method: 'POST', json: draft })
    draft = { text: '', title: '', category: draft.category }
    adding = false
    await load()
  }

  async function recategorize(/** @type {any} */ p, /** @type {string} */ id) {
    const next = await api(`/api/prompts/${p.id}`, { method: 'POST', json: { category: id } })
    list = list.map((x) => (x.id === p.id ? next : x))
  }

  // Send it to Create (switching to its kind when it has one); generating there records the use.
  function use(/** @type {any} */ p) {
    if (KINDS.some((k) => k.id === p.kind)) store.setKind(p.kind)
    store.draftPrompt = p.text
    store.setView('create')
  }

  function copy(/** @type {any} */ p) {
    navigator.clipboard?.writeText(p.text)
    copied = p.id
    setTimeout(() => (copied = null), COPIED_MS)
  }

  onMount(load)
</script>

<div class="flex flex-col gap-3">
  <div class="flex flex-col sm:flex-row gap-2 sm:items-center">
    <label class="flex-1 flex items-center gap-2 well focus-within:border-neon-red/50 rounded-xl px-3 py-2">
      <Search size={15} class="text-muted" />
      <input class="flex-1 min-w-0 bg-transparent outline-none text-sm 2xl:text-lg" placeholder="Find a prompt: words, title, notes" bind:value={query} />
    </label>
    <div class="flex gap-2">
      <div class="flex p-1 rounded-xl well text-xs 2xl:text-base" role="group" aria-label="Group by">
        {#each GROUPINGS as g (g.id)}
          <button class="px-3 py-1.5 rounded-lg font-semibold transition-colors {grouping === g.id ? 'bg-neon-red/15 text-white' : 'text-muted hover:text-white'}"
            onclick={() => (grouping = g.id)} aria-pressed={grouping === g.id}>{g.label}</button>
        {/each}
      </div>
      <button class="flex items-center gap-1.5 px-3 rounded-xl border border-white/10 hover:border-neon-red/50 text-sm 2xl:text-lg"
        onclick={() => (adding = !adding)} aria-expanded={adding}><Plus size={15} /> New</button>
    </div>
  </div>

  <div class="flex flex-wrap gap-1.5 text-xs 2xl:text-base">
    <button class="px-2.5 py-1 rounded-full border transition-colors {category === '' ? 'border-neon-red/50 text-white' : 'border-white/10 text-muted hover:text-white'}"
      onclick={() => (category = '')}>All {list.length}</button>
    {#each CATEGORIES.filter((c) => counts[c.id]) as c (c.id)}
      {@const Icon = c.icon}
      <button class="flex items-center gap-1.5 px-2.5 py-1 rounded-full border transition-colors {category === c.id ? c.tint + ' text-white' : 'border-white/10 text-muted hover:text-white'}"
        onclick={() => (category = category === c.id ? '' : c.id)} aria-pressed={category === c.id}>
        <Icon size={13} class={c.color} /> {c.label} <span class="text-neutral-500">{counts[c.id]}</span>
      </button>
    {/each}
  </div>

  {#if adding}
    <form class="flex flex-col gap-2 p-3 rounded-xl bg-black/25 border border-white/10" onsubmit={add} transition:slide={{ duration: 250, easing: EASE }}>
      <textarea class="{INPUT} resize-y min-h-20" rows="3" placeholder="The prompt" bind:value={draft.text}></textarea>
      <div class="flex flex-col sm:flex-row gap-2">
        <input class="{INPUT} flex-1" placeholder="Title (optional)" bind:value={draft.title} />
        <select class={INPUT} bind:value={draft.category}>
          {#each CATEGORIES as c (c.id)}<option value={c.id}>{c.label}</option>{/each}
        </select>
        <button type="submit" disabled={!draft.text.trim()} class="px-4 py-2 rounded-lg font-bold licorice disabled:opacity-40">Save</button>
      </div>
    </form>
  {/if}

  {#if !items.length}
    <p class="text-sm text-muted py-6 text-center">{list.length ? 'Nothing matches.' : 'No prompts yet. Everything you generate lands here.'}</p>
  {/if}

  {#each groups as g (g.id)}
    {@const GroupIcon = 'icon' in g ? g.icon : null}
    <h4 class="flex items-center gap-1.5 text-[11px] 2xl:text-sm uppercase tracking-wider text-muted mt-1">
      {#if GroupIcon}<GroupIcon size={12} class={'color' in g ? g.color : ''} />{/if}
      {g.label} <span class="normal-case tracking-normal text-neutral-600">· {'about' in g ? g.about + ' · ' : ''}{g.items.length}</span>
    </h4>
    <ul class="flex flex-col gap-2">
      {#each g.items as p (p.id)}
        {@const c = categoryOf(p.category)}
        {@const CatIcon = c.icon}
        {@const KindIcon = KIND_ICONS[p.kind]}
        <li class="flex gap-3 p-2.5 rounded-xl bg-black/25 border border-white/5 hover:border-white/15 transition-colors"
          animate:flip={{ duration: PAGE_MS, easing: EASE }} transition:slide={{ duration: 250, easing: EASE }}>
          <span class="shrink-0 w-9 h-9 2xl:w-11 2xl:h-11 rounded-lg border flex items-center justify-center {c.tint}" title={c.label}>
            <CatIcon size={17} class={c.color} />
          </span>
          <div class="flex-1 min-w-0 flex flex-col gap-1">
            <button class="text-left" onclick={() => (open = open === p.id ? null : p.id)} aria-expanded={open === p.id}>
              {#if p.title}<p class="text-sm 2xl:text-lg font-semibold text-neutral-100 truncate">{p.title}</p>{/if}
              <p class="text-sm 2xl:text-lg text-neutral-300 select-text whitespace-pre-wrap break-words {open === p.id ? '' : 'line-clamp-2'}">{p.text}</p>
            </button>
            {#if open === p.id && p.notes}<p class="text-xs 2xl:text-base text-neutral-500 select-text">{p.notes}</p>{/if}
            <p class="text-[11px] 2xl:text-sm text-neutral-500 flex flex-wrap items-center gap-x-1.5">
              <span class="px-1.5 rounded bg-white/5 text-neutral-400">{specificityOf(p.text).label}</span>
              {#if KindIcon}<KindIcon size={11} />{/if}
              <span title={p.last_used ? new Date(p.last_used).toLocaleString() : `Saved ${new Date(p.created).toLocaleString()}`}>
                {p.last_used ? `used ${ago(p.last_used)}` : 'never used'}</span>
              {#if p.uses}<span>· {p.uses}×</span>{/if}
            </p>
            <div class="flex items-center gap-1">
              <button class={ACTION} title="Use in Create" aria-label="Use in Create" onclick={() => use(p)}><Send size={15} /></button>
              <button class={ACTION} title="Copy prompt" aria-label="Copy prompt" onclick={() => copy(p)}>
                {#if copied === p.id}<Check size={15} />{:else}<Copy size={15} />{/if}
              </button>
              <select class="bg-transparent text-[11px] 2xl:text-sm text-neutral-500 hover:text-white outline-none cursor-pointer" aria-label="Category"
                value={p.category} onchange={(e) => recategorize(p, e.currentTarget.value)}>
                {#each CATEGORIES as o (o.id)}<option value={o.id}>{o.label}</option>{/each}
              </select>
              <button class="{ACTION} hover:!bg-neon-red ml-auto" title="Delete" aria-label="Delete" onclick={() => (pending = p)}><Trash2 size={15} /></button>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/each}
  {#if error}<p class="text-xs text-neon-red">{error}</p>{/if}
</div>

{#if pending}
  <Confirm title="Delete this prompt?" message="Removes it from your prompt library. Generated files that used it stay." items={[pending.title || pending.text.slice(0, 80)]}
    onConfirm={async () => { await api(`/api/prompts/${pending.id}`, { method: 'DELETE' }); pending = null; await load() }} onCancel={() => (pending = null)} />
{/if}
