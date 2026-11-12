<script>
  import { untrack } from 'svelte'
  import { Search, ExternalLink, Download, ChevronDown, LoaderCircle, Lock, Heart, X, MessageSquare, Library } from '@lucide/svelte'
  import { slide } from 'svelte/transition'
  import { api } from '../lib/api.js'
  import { store } from '../lib/store.svelte.js'
  import { size, count } from '../lib/format.js'
  import { EASE } from '../lib/motion.js'

  const INPUT = 'flex-1 min-w-0 bg-transparent py-2 text-sm 2xl:text-lg outline-none'
  const BTN = 'flex items-center gap-1 px-2 py-1 rounded-md text-[11px] 2xl:text-sm border border-white/10 hover:border-neon-red/50 hover:text-neon-red transition-colors disabled:opacity-40'
  const CHIP = 'px-2.5 py-1 rounded-full text-[11px] 2xl:text-sm border transition-colors'
  const SELECT = 'well rounded-lg px-2 py-1 text-[11px] 2xl:text-sm outline-none'
  const PAGE = 20
  const MAX = 100
  const DEBOUNCE_MS = 350

  const KIND_CHIPS = [
    { id: 'all', label: 'All' }, { id: 'image', label: 'Image' }, { id: 'video', label: 'Video' },
    { id: 'audio', label: 'Audio' }, { id: 'chat', label: 'Text / chat' },
  ]
  const SORTS = [
    { id: 'downloads', label: 'Most downloaded' }, { id: 'likes', label: 'Most liked' }, { id: 'trending', label: 'Trending' },
    { id: 'modified', label: 'Recently updated' }, { id: 'created', label: 'Newest' },
  ]
  const FORMATS = [{ id: '', label: 'Any format' }, { id: 'gguf', label: 'GGUF' }, { id: 'safetensors', label: 'Safetensors' }, { id: 'diffusers', label: 'Diffusers' }]

  let query = $state('')
  let kind = $state('all')
  let sort = $state('downloads')
  let format = $state('')
  let limit = $state(PAGE)
  let busy = $state(false)
  let error = $state('')
  /** @type {any[] | null} */ let results = $state(null)
  /** @type {Record<string, any>} */ let files = $state({})
  let open = $state('')
  let seq = 0

  const pulling = $derived(new Set(store.downloads.filter((d) => d.status === 'running').map((d) => d.target)))
  const installed = $derived(new Set((store.llm.models || []).map((/** @type {any} */ m) => m.name.toLowerCase())))

  async function run() {
    const mine = ++seq
    if (!query.trim() && kind === 'all' && !format) { results = null; return }
    busy = true
    error = ''
    try {
      const q = new URLSearchParams({ q: query.trim(), kind, sort, limit: String(limit) })
      if (format) q.set('format', format)
      const r = await api(`/api/hub/search?${q}`)
      if (mine === seq) results = r
    } catch (err) {
      if (mine === seq) error = String(err.message || err)
    } finally {
      if (mine === seq) busy = false
    }
  }

  // Search as you type or change a filter; a pasted link opens its files right away.
  $effect(() => {
    void [query, kind, sort, format, limit]
    const t = setTimeout(run, DEBOUNCE_MS)
    return () => clearTimeout(t)
  })
  $effect(() => {
    const first = results?.[0]?.repo
    const pasted = /huggingface\.co|hf\.co|^[\w.-]+\/[\w.-]+$/.exec(untrack(() => query.trim()))
    if (first && pasted && untrack(() => query.trim()).toLowerCase().includes(first.toLowerCase())) untrack(() => { if (open !== first) toggle(first) })
  })

  function setFilter(/** @type {() => void} */ change) { change(); limit = PAGE }

  async function toggle(/** @type {string} */ repo) {
    open = open === repo ? '' : repo
    if (!open || files[repo]) return
    try { files[repo] = await api(`/api/hub/files?repo=${encodeURIComponent(repo)}`) } catch (err) { files[repo] = { error: String(err.message || err) } }
  }

  async function get(/** @type {string} */ repo, /** @type {string} */ file) {
    error = ''
    try { await api('/api/downloads', { method: 'POST', json: { target: repo, files: [file] } }) } catch (err) { error = String(err.message || err) }
  }

  async function pull(/** @type {string} */ repo, /** @type {string} */ file) {
    error = ''
    try { await store.pullLlm(repo, file) } catch (err) { error = String(err.message || err) }
  }

  function ago(/** @type {string | null} */ iso) {
    if (!iso) return ''
    const d = (Date.now() - Date.parse(iso)) / 86400e3
    return d < 1 ? 'today' : d < 30 ? `${Math.floor(d)}d ago` : d < 365 ? `${Math.floor(d / 30)}mo ago` : `${Math.floor(d / 365)}y ago`
  }
</script>

<div class="flex flex-col gap-2">
  <form class="flex items-center gap-2 well rounded-xl px-3" onsubmit={(e) => { e.preventDefault(); run() }}>
    {#if busy}<LoaderCircle size={16} class="shrink-0 animate-spin text-neon-red" />{:else}<Search size={16} class="shrink-0 text-muted" />{/if}
    <input class={INPUT} bind:value={query} oninput={() => (limit = PAGE)} placeholder="Search Hugging Face, or paste a model link / org/name" aria-label="Search Hugging Face" />
    {#if query}<button type="button" class="shrink-0 text-muted hover:text-neon-red" onclick={() => (query = '')} aria-label="Clear search"><X size={14} /></button>{/if}
  </form>

  <div class="flex flex-wrap items-center gap-1.5">
    {#each KIND_CHIPS as k (k.id)}
      <button class="{CHIP} {kind === k.id ? 'border-neon-red/60 text-neon-red bg-neon-red/10' : 'border-white/10 text-neutral-400 hover:border-white/25'}"
        onclick={() => setFilter(() => (kind = k.id))} aria-pressed={kind === k.id}>{k.label}</button>
    {/each}
    <span class="flex-1"></span>
    <select class={SELECT} value={format} onchange={(e) => setFilter(() => (format = e.currentTarget.value))} aria-label="File format">
      {#each FORMATS as f (f.id)}<option value={f.id}>{f.label}</option>{/each}
    </select>
    <select class={SELECT} value={sort} onchange={(e) => setFilter(() => (sort = e.currentTarget.value))} aria-label="Sort by">
      {#each SORTS as s (s.id)}<option value={s.id}>{s.label}</option>{/each}
    </select>
  </div>
  <p class="text-[11px] 2xl:text-sm text-neutral-600">Tip: <code>org/words</code> or <code>author:org words</code> searches one uploader. Text models install into Ollama and show up in your library and chat.</p>
  {#if error}<p class="text-neon-red text-xs break-words">{error}</p>{/if}

  {#if results}
    {#if !results.length}<p class="text-xs text-muted">No matches.</p>{/if}
    {#each results as r (r.repo)}
      <div class="rounded-lg bg-card/40 border border-white/5 text-xs 2xl:text-sm" transition:slide={{ duration: 250, easing: EASE }}>
        <div class="flex items-center gap-2 p-2">
          <button class="flex-1 min-w-0 flex items-center gap-1.5 text-left" onclick={() => toggle(r.repo)}>
            <ChevronDown size={14} class="shrink-0 transition-transform duration-300 {open === r.repo ? 'rotate-180' : ''}" />
            <span class="truncate font-semibold" title={r.repo}>{r.repo}</span>
            {#if r.gated}<Lock size={12} class="shrink-0 text-amber-400" />{/if}
          </button>
          <span class="text-neutral-500 shrink-0" title="Downloads">{count(r.downloads)} dl</span>
          <span class="hidden sm:flex items-center gap-0.5 text-neutral-500 shrink-0" title="Likes"><Heart size={11} />{count(r.likes)}</span>
          <a class="shrink-0 hover:text-neon-red" href={r.url} target="_blank" rel="noopener noreferrer" aria-label="Open on Hugging Face"><ExternalLink size={14} /></a>
        </div>
        <div class="px-2 pb-2 flex flex-wrap items-center gap-1.5 text-[10px] 2xl:text-xs text-neutral-500">
          {#if r.kind}<span class="px-1.5 py-0.5 rounded bg-white/5 {r.kind === 'chat' ? 'text-sky-300' : 'text-neutral-300'}">{r.kind === 'chat' ? 'text' : r.kind}</span>{:else if r.pipeline}<span class="px-1.5 py-0.5 rounded bg-white/5">{r.pipeline}</span>{/if}
          {#each r.formats as f (f)}<span class="px-1.5 py-0.5 rounded bg-white/5 uppercase">{f}</span>{/each}
          {#if r.license}<span>{r.license}</span>{/if}
          {#if r.updated}<span>updated {ago(r.updated)}</span>{/if}
          {#if r.family}<span class="text-emerald-400">Built-in workflow: {r.family}</span>{/if}
        </div>
        {#if open === r.repo}
          <div class="border-t border-white/5 p-2 flex flex-col gap-1" transition:slide={{ duration: 250, easing: EASE }}>
            {#if !files[r.repo]}
              <LoaderCircle size={14} class="animate-spin" />
            {:else if files[r.repo].error}
              <p class="text-neon-red break-words">{files[r.repo].error}</p>
            {:else if !files[r.repo].files.length}
              <p class="text-muted">No .safetensors / .gguf files (diffusers-format repos don't run in ComfyUI).</p>
            {:else}
              {#each files[r.repo].files as f (f.file)}
                <div class="flex items-center gap-2">
                  <a class="flex-1 min-w-0 truncate text-neutral-300 hover:text-neon-red" href={f.url} target="_blank" rel="noopener noreferrer" title={f.file}>{f.file}</a>
                  <span class="shrink-0 text-neutral-500">{size(f.size)}</span>
                  {#if f.ollama}
                    {@const have = installed.has(f.ollama.toLowerCase()) || installed.has(`${f.ollama.toLowerCase()}:latest`)}
                    <button class={BTN} onclick={() => pull(r.repo, f.file)} disabled={have || pulling.has(f.ollama)} title="Pull into Ollama as {f.ollama}">
                      {#if pulling.has(f.ollama)}<LoaderCircle size={12} class="animate-spin" />{:else}<Library size={12} />{/if}
                      {have ? 'In library' : 'Add to library'}
                    </button>
                  {:else}
                    <button class={BTN} onclick={() => get(r.repo, f.file)} aria-label="Download {f.file}"><Download size={12} /></button>
                  {/if}
                </div>
              {/each}
              {#if files[r.repo].chat}
                <p class="text-neutral-600 mt-1 flex items-center gap-1"><MessageSquare size={11} class="shrink-0" /> Text model: added to Ollama, then pick it in Chat. Smaller quants (Q4) need less memory; Q8 is closest to the original.</p>
              {:else}
                <p class="text-neutral-600 mt-1">Files without a built-in workflow show under "unrecognized"; add a ComfyUI workflow in <code>workflows/</code> to use them.</p>
              {/if}
            {/if}
          </div>
        {/if}
      </div>
    {/each}
    {#if results.length >= limit && limit < MAX}
      <button class="{BTN} self-center" onclick={() => (limit = Math.min(MAX, limit + PAGE))} disabled={busy}>Show more</button>
    {/if}
  {/if}
</div>
