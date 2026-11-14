<script>
  import { onMount } from 'svelte'
  import { X, Trash2, Plus, ChevronUp, ChevronDown, Star, Upload, MessageCircle, ImagePlus, UserRound, LoaderCircle, Music, Film } from '@lucide/svelte'
  import { fade, fly, scale, slide } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { api, url, settings } from '../lib/api.js'
  import { store } from '../lib/store.svelte.js'
  import { chat } from '../lib/chat.svelte.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import { portal } from '../lib/portal.js'
  import Confirm from './Confirm.svelte'

  /** @type {{ id: string, onClose: () => void, onDeleted: () => void }} */
  let { id, onClose, onDeleted } = $props()

  const SAVE_DELAY_MS = 600
  const INPUT = 'well rounded-lg px-3 py-2 outline-none'
  const ACCEPT = 'image/*,video/*,audio/*'

  /** @type {any} */ let c = $state(null)
  let saving = $state(false)
  let error = $state('')
  let dragging = $state(0)
  /** @type {{ name: string, pct: number }[]} */ let uploads = $state([])
  let picking = $state(false)
  let confirmDelete = $state(false)
  /** @type {Record<string, string>} */ let newItem = $state({})
  /** @type {HTMLInputElement} */ let filePicker
  let timer = 0

  const avatar = $derived(c?.media.find((/** @type {any} */ m) => m.id === c.avatar))
  const attached = $derived(new Set((c?.media || []).map((/** @type {any} */ m) => m.file)))

  onMount(async () => {
    try { c = await api(`/api/characters/${id}`) } catch (e) { error = String(e.message || e) }
  })

  // Autosave name / bio / lists shortly after the last change.
  function changed() {
    clearTimeout(timer)
    timer = setTimeout(save, SAVE_DELAY_MS)
  }

  async function save(extra = {}) {
    clearTimeout(timer)
    saving = true
    try {
      const next = await api(`/api/characters/${id}`, { method: 'POST', json: { name: c.name, bio: c.bio, lists: c.lists, ...extra } })
      c.avatar = next.avatar
      c.updated = next.updated
    } catch (e) { error = String(e.message || e) } finally { saving = false }
  }

  // ---- lists
  function addList() {
    c.lists = [...c.lists, { id: crypto.randomUUID().slice(0, 12), title: 'New list', items: [] }]
    changed()
  }
  function moveList(/** @type {number} */ i, /** @type {number} */ d) {
    const j = i + d
    if (j < 0 || j >= c.lists.length) return
    const l = [...c.lists];
    [l[i], l[j]] = [l[j], l[i]]
    c.lists = l
    changed()
  }
  function removeList(/** @type {number} */ i) { c.lists = c.lists.filter((/** @type {any} */ _, k) => k !== i); changed() }
  function addItem(/** @type {any} */ l) {
    const v = (newItem[l.id] || '').trim()
    if (!v) return
    l.items = [...l.items, v]
    newItem[l.id] = ''
    changed()
  }
  function removeItem(/** @type {any} */ l, /** @type {number} */ k) { l.items = l.items.filter((/** @type {any} */ _, x) => x !== k); changed() }

  // ---- media
  // Raw upload with progress (fetch can't report upload progress).
  function upload(/** @type {File} */ file) {
    const entry = { name: file.name, pct: 0 }
    uploads = [...uploads, entry]
    return new Promise((resolve) => {
      const xhr = new XMLHttpRequest()
      xhr.open('POST', url(`/api/characters/${id}/media`))
      xhr.setRequestHeader('content-type', 'application/octet-stream')
      xhr.setRequestHeader('x-filename', encodeURIComponent(file.name))
      if (settings.key) xhr.setRequestHeader('authorization', `Bearer ${settings.key}`)
      xhr.upload.onprogress = (e) => { entry.pct = e.lengthComputable ? e.loaded / e.total : 0; uploads = [...uploads] }
      xhr.onload = () => {
        uploads = uploads.filter((u) => u !== entry)
        try {
          const body = JSON.parse(xhr.responseText)
          if (xhr.status >= 300) throw new Error(body.error || `HTTP ${xhr.status}`)
          c.media = [body, ...c.media]
          if (!c.avatar && body.kind === 'image') save({ avatar: body.id })
        } catch (e) { error = `${file.name}: ${e.message}` }
        resolve(null)
      }
      xhr.onerror = () => { uploads = uploads.filter((u) => u !== entry); error = `${file.name}: upload failed`; resolve(null) }
      xhr.send(file)
    })
  }

  async function addFiles(/** @type {FileList | File[] | null | undefined} */ files) {
    error = ''
    for (const f of files || []) await upload(f)
  }

  function drop(/** @type {DragEvent} */ e) {
    e.preventDefault()
    dragging = 0
    addFiles(e.dataTransfer?.files)
  }

  async function attach(/** @type {any} */ it) {
    try {
      const m = await api(`/api/characters/${id}/media`, { method: 'POST', json: { file: it.file } })
      c.media = [m, ...c.media]
      if (!c.avatar && m.kind === 'image') await save({ avatar: m.id })
    } catch (e) { error = String(e.message || e) }
  }

  async function removeMedia(/** @type {any} */ m) {
    await api(`/api/characters/${id}/media/${m.id}`, { method: 'DELETE' })
    c.media = c.media.filter((/** @type {any} */ x) => x.id !== m.id)
    if (c.avatar === m.id) c.avatar = null
  }

  async function remove() {
    await api(`/api/characters/${id}`, { method: 'DELETE' })
    onDeleted()
  }

  async function startChat() {
    await save()
    await chat.create({ characterId: id })
    onClose()
    store.setView('chat')
  }

  function close() {
    if (timer) save()
    onClose()
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !confirmDelete && !picking && close()} />

<!-- On <body>: the Library section's glass would otherwise clip this fixed-position dialog to the section. -->
<div class="contents" {@attach portal}>
<div class="scrim z-50 flex items-end md:items-center justify-center md:p-6" role="dialog" aria-modal="true" aria-label="Character"
  transition:fade={{ duration: PAGE_MS / 2 }}
  ondragenter={(e) => { e.preventDefault(); dragging++ }} ondragleave={() => dragging--} ondragover={(e) => e.preventDefault()} ondrop={drop}>
  <div class="relative w-full md:max-w-4xl 2xl:max-w-6xl max-h-full overflow-y-auto glass-panel rounded-t-3xl md:rounded-3xl"
    in:fly={{ y: 60, duration: PAGE_MS, easing: EASE }} out:fly={{ y: 40, duration: 200 }}>
    {#if !c}
      <div class="py-16 flex justify-center"><LoaderCircle class="animate-spin text-neon-red" /></div>
    {:else}
      <!-- Header: avatar, name, actions -->
      <div class="sticky top-0 z-10 flex items-center gap-3 p-4 sm:p-5 bg-black/60 backdrop-blur-xl border-b border-glass-edge">
        <div class="w-14 h-14 2xl:w-20 2xl:h-20 shrink-0 rounded-2xl overflow-hidden bg-black/50 border border-white/10 flex items-center justify-center">
          {#if avatar}<img src={url(`/files/${avatar.file}`, true)} alt="" class="w-full h-full object-cover" />{:else}<UserRound size={26} class="text-neutral-600" />{/if}
        </div>
        <input class="flex-1 min-w-0 bg-transparent outline-none text-xl sm:text-2xl 2xl:text-4xl font-bold" bind:value={c.name} oninput={changed} aria-label="Name" />
        <span class="text-[11px] text-neutral-500 hidden sm:inline">{saving ? 'Saving…' : 'Saved'}</span>
        <button class="licorice flex items-center gap-1.5 px-3 py-2 rounded-xl text-sm font-semibold" onclick={startChat}><MessageCircle size={15} /> Chat</button>
        <button class="p-2 rounded-lg hover:bg-neon-red/80 text-neutral-400 hover:text-white" onclick={() => (confirmDelete = true)} aria-label="Delete character"><Trash2 size={16} /></button>
        <button class="p-2 rounded-lg border border-white/10 hover:border-neon-red/50" onclick={close} aria-label="Close"><X size={16} /></button>
      </div>

      <div class="p-4 sm:p-6 grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
        <!-- Bio + lists -->
        <div class="flex flex-col gap-5">
          <label class="flex flex-col gap-1.5">
            <span class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted">Bio</span>
            <textarea class="{INPUT} min-h-32 resize-y text-sm 2xl:text-lg" bind:value={c.bio} oninput={changed}
              placeholder="Who are they? Personality, backstory, how they talk…"></textarea>
          </label>

          <div class="flex flex-col gap-3">
            {#each c.lists as l, i (l.id)}
              <div class="rounded-xl well p-3 flex flex-col gap-2" animate:flip={{ duration: 300, easing: EASE }} transition:slide={{ duration: 250, easing: EASE }}>
                <div class="flex items-center gap-1">
                  <input class="flex-1 min-w-0 bg-transparent outline-none font-semibold text-sm 2xl:text-lg" bind:value={l.title} oninput={changed} aria-label="List name" />
                  <button class="p-1 rounded hover:bg-white/10 disabled:opacity-20" disabled={i === 0} onclick={() => moveList(i, -1)} aria-label="Move up"><ChevronUp size={14} /></button>
                  <button class="p-1 rounded hover:bg-white/10 disabled:opacity-20" disabled={i === c.lists.length - 1} onclick={() => moveList(i, 1)} aria-label="Move down"><ChevronDown size={14} /></button>
                  <button class="p-1 rounded hover:text-neon-red" onclick={() => removeList(i)} aria-label="Remove list"><Trash2 size={14} /></button>
                </div>
                <div class="flex flex-wrap gap-1.5">
                  {#each l.items as it, k (it + k)}
                    <span class="flex items-center gap-1 pl-2.5 pr-1 py-1 rounded-full bg-white/6 border border-white/8 text-xs 2xl:text-base" in:scale={{ start: 0.8, duration: 200 }}>
                      {it}<button class="p-0.5 rounded-full hover:bg-neon-red" onclick={() => removeItem(l, k)} aria-label="Remove {it}"><X size={11} /></button>
                    </span>
                  {/each}
                  <input class="min-w-28 flex-1 bg-transparent outline-none text-xs 2xl:text-base px-1 py-1 placeholder:text-neutral-600"
                    placeholder="Add… (Enter)" bind:value={newItem[l.id]} onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), addItem(l))} />
                </div>
              </div>
            {/each}
            <button class="self-start flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-dashed border-white/15 hover:border-neon-red/50 text-xs 2xl:text-base text-muted hover:text-white" onclick={addList}>
              <Plus size={13} /> Add a list
            </button>
          </div>
        </div>

        <!-- Media -->
        <div class="flex flex-col gap-3">
          <div class="flex items-center gap-2">
            <span class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted">Media · {c.media.length}</span>
            <button class="ml-auto flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border border-white/10 hover:border-neon-red/50 text-xs 2xl:text-base" onclick={() => (picking = true)}>
              <ImagePlus size={13} /> From library
            </button>
            <button class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border border-white/10 hover:border-neon-red/50 text-xs 2xl:text-base" onclick={() => filePicker.click()}>
              <Upload size={13} /> Files
            </button>
            <input bind:this={filePicker} type="file" accept={ACCEPT} multiple class="hidden" onchange={(e) => addFiles(e.currentTarget.files)} />
          </div>

          {#each uploads as u (u)}
            <div class="text-xs text-neutral-400 flex items-center gap-2" transition:slide={{ duration: 200 }}>
              <span class="truncate flex-1">{u.name}</span>
              <div class="w-24 h-1 rounded bg-white/10"><div class="h-1 rounded bg-neon-red" style="width: {u.pct * 100}%"></div></div>
            </div>
          {/each}

          <div class="grid grid-cols-3 sm:grid-cols-4 gap-2">
            {#each c.media as m (m.id)}
              <div class="group relative aspect-square rounded-xl overflow-hidden bg-black/50 border {c.avatar === m.id ? 'border-neon-red/70' : 'border-white/8'}"
                animate:flip={{ duration: 300, easing: EASE }} in:scale={{ start: 0.85, duration: 300, easing: EASE }}>
                {#if m.kind === 'image'}
                  <img src={url(`/files/${m.file}`, true)} alt={m.name || ''} loading="lazy" class="w-full h-full object-cover" />
                {:else if m.kind === 'video'}
                  <video src={url(`/files/${m.file}`, true) + '#t=0.1'} muted preload="metadata" class="w-full h-full object-cover"></video>
                  <Film size={14} class="absolute bottom-1.5 left-1.5 text-white drop-shadow" />
                {:else}
                  <div class="w-full h-full flex flex-col items-center justify-center gap-1 p-2 text-neutral-400"><Music size={22} /><span class="text-[10px] truncate max-w-full">{m.name}</span></div>
                {/if}
                <div class="absolute inset-x-0 top-0 flex justify-between p-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  {#if m.kind === 'image'}
                    <button class="p-1 rounded-md bg-black/70 {c.avatar === m.id ? 'text-neon-red' : 'text-white hover:text-neon-red'}" title="Use as profile picture" aria-label="Use as profile picture" onclick={() => save({ avatar: m.id })}><Star size={12} /></button>
                  {:else}<span></span>{/if}
                  <button class="p-1 rounded-md bg-black/70 hover:bg-neon-red" title="Remove" aria-label="Remove media" onclick={() => removeMedia(m)}><X size={12} /></button>
                </div>
              </div>
            {/each}
          </div>
          <div class="rounded-xl border border-dashed border-white/12 py-6 text-center text-xs 2xl:text-base text-neutral-500">
            Drag images, video or audio anywhere onto this window to add them.
          </div>
          {#if error}<p class="text-xs text-neon-red break-words">{error}</p>{/if}
        </div>
      </div>
    {/if}

    {#if dragging > 0}
      <div class="absolute inset-0 z-20 rounded-3xl border-2 border-dashed border-neon-red bg-neon-red/10 backdrop-blur-sm flex items-center justify-center pointer-events-none" transition:fade={{ duration: 120 }}>
        <p class="text-lg font-bold flex items-center gap-2"><Upload size={20} /> Drop to add to {c?.name}</p>
      </div>
    {/if}
  </div>
</div>

<!-- Pick generated media from the Library -->
{#if picking}
  <div class="scrim z-[55] flex items-center justify-center p-4" transition:fade={{ duration: 150 }}>
    <div class="w-full max-w-3xl max-h-full overflow-y-auto glass-panel rounded-2xl p-4 flex flex-col gap-3" in:fly={{ y: 20, duration: PAGE_MS, easing: EASE }}>
      <div class="flex items-center justify-between">
        <h3 class="font-bold">Add from your creations</h3>
        <button class="p-2 rounded-lg border border-white/10 hover:border-neon-red/50" onclick={() => (picking = false)} aria-label="Done"><X size={15} /></button>
      </div>
      {#if !store.media.length}<p class="text-sm text-muted">Nothing generated yet.</p>{/if}
      <div class="grid grid-cols-3 sm:grid-cols-5 gap-2">
        {#each store.media as it (it.file)}
          {@const added = attached.has(it.file)}
          <button class="relative aspect-square rounded-xl overflow-hidden bg-black/50 border {added ? 'border-neon-red/70 opacity-60' : 'border-white/8 hover:border-neon-red/50'}"
            disabled={added} onclick={() => attach(it)} title={it.job.prompt}>
            {#if it.kind === 'image'}<img src={url(it.url, true)} alt="" class="w-full h-full object-cover" loading="lazy" />
            {:else if it.kind === 'video'}<video src={url(it.url, true) + '#t=0.1'} muted preload="metadata" class="w-full h-full object-cover"></video>
            {:else}<Music size={22} class="m-auto text-neutral-400" />{/if}
            {#if added}<span class="absolute bottom-1 right-1 text-[10px] px-1.5 rounded bg-black/70">added</span>{/if}
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}

{#if confirmDelete && c}
  <Confirm title="Delete {c.name}?" message="Deletes the character, its lists, and files you uploaded to it. Generated media stays in your Library."
    onConfirm={remove} onCancel={() => (confirmDelete = false)} />
{/if}
</div>
