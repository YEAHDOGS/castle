<script>
  import { onMount, tick } from 'svelte'
  import { Plus, Send, Square, Brain, Trash2, PanelLeft, MessageSquare, UserRound } from '@lucide/svelte'
  import { fly, fade } from 'svelte/transition'
  import { chat } from '../lib/chat.svelte.js'
  import { store } from '../lib/store.svelte.js'
  import { api } from '../lib/api.js'
  import { tokens } from '../lib/library.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import ChatMessage from './ChatMessage.svelte'

  const MAX_INPUT_ROWS = 10
  const SUGGESTIONS = [
    'Write a prompt for a cinematic image of a neon city in the rain',
    'Explain how diffusion models make images, simply',
    'Give me 5 ideas for a short animated loop',
  ]

  let input = $state('')
  /** @type {HTMLTextAreaElement} */ let box
  /** @type {HTMLDivElement} */ let scroller
  let showList = $state(false)
  /** @type {any[]} */ let characters = $state([])
  let pinned = $state(true)

  const models = $derived(store.llm.models || [])
  const modelInfo = $derived(models.find((m) => m.name === (chat.model || chat.current?.model)) || models[0])
  const messages = $derived(chat.current?.messages || [])
  // Context used by the last reply (prompt + answer tokens) against the configured window.
  const used = $derived([...messages].reverse().find((m) => m.context)?.context || 0)
  const windowSize = $derived(modelInfo?.context?.configured || modelInfo?.context?.max || 0)
  const pct = $derived(windowSize ? Math.min(100, (used / windowSize) * 100) : 0)
  const character = $derived(characters.find((c) => c.id === chat.current?.character_id))

  onMount(async () => {
    await Promise.all([chat.load(), store.loadLlm().catch(() => {}), api('/api/characters').then((c) => (characters = c)).catch(() => {})])
    if (!chat.model) chat.model = models.find((m) => /fast/.test(m.name))?.name || models[0]?.name || ''
    if (!chat.current && chat.list[0]) await chat.open(chat.list[0].id)
  })

  // Follow the reply as it streams, unless the user scrolled up to read.
  $effect(() => {
    void chat.draft.content; void chat.draft.thinking; void messages.length
    if (pinned) tick().then(() => scroller?.scrollTo({ top: scroller.scrollHeight }))
  })

  function onScroll() {
    pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 80
  }

  function grow() {
    box.style.height = 'auto'
    const line = parseFloat(getComputedStyle(box).lineHeight) || 20
    box.style.height = `${Math.min(box.scrollHeight, line * MAX_INPUT_ROWS)}px`
  }

  async function send() {
    const text = input.trim()
    if (!text || chat.streaming) return
    input = ''
    await tick()
    grow()
    pinned = true
    await chat.send(text)
  }

  function onKey(/** @type {KeyboardEvent} */ e) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); send() }
  }

  async function newChat(/** @type {string} */ characterId = '') {
    await chat.create(characterId ? { characterId } : {})
    showList = false
    box?.focus()
  }
</script>

<div class="h-full min-h-0 flex">
  <!-- Conversations: a rail on wide screens, a drawer on narrow ones -->
  {#if showList}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="md:hidden fixed inset-0 z-30 bg-black/50" transition:fade={{ duration: 150 }} onclick={() => (showList = false)}></div>
  {/if}
  <aside class="list fixed md:static inset-y-0 left-0 z-40 w-72 2xl:w-96 shrink-0 flex flex-col gap-2 p-3 border-r border-glass-edge bg-black/60 md:bg-transparent backdrop-blur-xl md:backdrop-blur-none"
    class:open={showList}>
    <button class="licorice flex items-center justify-center gap-2 py-2.5 rounded-xl font-semibold text-sm 2xl:text-lg" onclick={() => newChat()}>
      <Plus size={16} /> New chat
    </button>
    {#if characters.length}
      <select class="well rounded-lg px-3 py-2 text-xs 2xl:text-base outline-none" onchange={(e) => { if (e.currentTarget.value) newChat(e.currentTarget.value); e.currentTarget.value = '' }}>
        <option value="">Chat as a character…</option>
        {#each characters as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
      </select>
    {/if}
    <div class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-1 -mx-1 px-1">
      {#each chat.list as c (c.id)}
        <div class="row group flex items-center gap-1 rounded-lg {chat.current?.id === c.id ? 'bg-white/8 text-white' : 'text-neutral-400 hover:bg-white/4 hover:text-white'}">
          <button class="flex-1 min-w-0 flex items-center gap-2 px-2.5 py-2 text-left text-sm 2xl:text-lg" onclick={() => { chat.open(c.id); showList = false }}>
            {#if c.character_id}<UserRound size={14} class="shrink-0 text-neon-red" />{:else}<MessageSquare size={14} class="shrink-0 opacity-60" />{/if}
            <span class="truncate">{c.title}</span>
          </button>
          <button class="p-1.5 mr-1 rounded opacity-0 group-hover:opacity-100 hover:text-neon-red" aria-label="Delete chat" onclick={() => chat.remove(c.id)}><Trash2 size={13} /></button>
        </div>
      {/each}
    </div>
  </aside>

  <!-- Conversation -->
  <section class="flex-1 min-w-0 flex flex-col">
    <header class="flex items-center gap-2 px-3 py-2 border-b border-glass-edge">
      <button class="md:hidden p-2 rounded-lg hover:bg-white/5" onclick={() => (showList = true)} aria-label="Conversations"><PanelLeft size={18} /></button>
      <p class="flex-1 min-w-0 truncate text-sm 2xl:text-lg font-semibold">
        {#if character}<span class="text-neon-red">{character.name}</span> · {/if}{chat.current?.title || 'New chat'}
      </p>
      <select class="well rounded-lg px-2 py-1.5 text-xs 2xl:text-base outline-none max-w-44" bind:value={chat.model} aria-label="Model">
        {#each models as m (m.name)}<option value={m.name}>{m.name.replace(/:latest$/, '')}</option>{/each}
      </select>
      <button class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-xs 2xl:text-base border transition-colors {chat.think ? 'border-neon-red/50 text-white bg-neon-red/10' : 'border-white/10 text-muted hover:text-white'}"
        onclick={() => (chat.think = !chat.think)} aria-pressed={chat.think} title="Let the model think step by step before answering (slower, better for hard questions)">
        <Brain size={14} /> Think
      </button>
    </header>

    <div class="flex-1 min-h-0 overflow-y-auto" bind:this={scroller} onscroll={onScroll}>
      <div class="max-w-3xl 2xl:max-w-5xl mx-auto px-4 py-6 flex flex-col gap-6">
        {#if !messages.length && !chat.streaming}
          <div class="py-10 md:py-20 flex flex-col items-center text-center gap-5" in:fade={{ duration: PAGE_MS }}>
            <h2 class="text-2xl md:text-3xl 2xl:text-5xl font-bold tracking-tight">{character ? `Talk to ${character.name}` : 'What are we making?'}</h2>
            <p class="text-sm 2xl:text-lg text-muted max-w-md">Runs on {modelInfo?.name?.replace(/:latest$/, '') || 'your local model'}, entirely on this computer.</p>
            {#if !character}
              <div class="flex flex-col sm:flex-row flex-wrap justify-center gap-2 w-full">
                {#each SUGGESTIONS as s, i (s)}
                  <button class="glass rounded-xl px-3 py-2 text-xs 2xl:text-base text-neutral-300 hover:text-white hover:border-neon-red/40 text-left sm:max-w-56"
                    in:fly={{ y: 10, duration: PAGE_MS, delay: 60 * i, easing: EASE }} onclick={() => { input = s; send() }}>{s}</button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
        {#each messages as m (m.id)}
          <div in:fly={{ y: 8, duration: 300, easing: EASE }}>
            <ChatMessage role={m.role} content={m.content} thinking={m.thinking}
              meta={m.tokens ? `${m.tokens} tokens${m.model ? ` · ${m.model.replace(/:latest$/, '')}` : ''}` : ''} />
          </div>
        {/each}
        {#if chat.streaming}
          <ChatMessage role="assistant" content={chat.draft.content} thinking={chat.draft.thinking} live />
        {/if}
        {#if chat.error}<p class="text-sm text-neon-red break-words" in:fade>{chat.error}</p>{/if}
      </div>
    </div>

    <!-- Composer -->
    <div class="px-3 pb-3 pt-1">
      <div class="max-w-3xl 2xl:max-w-5xl mx-auto glass rounded-2xl p-2 flex flex-col gap-1">
        <textarea bind:this={box} bind:value={input} oninput={grow} onkeydown={onKey} rows="1"
          class="w-full resize-none bg-transparent outline-none px-2 py-1.5 text-sm 2xl:text-lg placeholder:text-neutral-600"
          placeholder={character ? `Message ${character.name}…` : 'Message DOGS AI…'}></textarea>
        <div class="flex items-center gap-2 px-1">
          {#if windowSize}
            <div class="flex items-center gap-2 text-[10px] 2xl:text-sm text-neutral-500" title="Context used by this conversation: {used} of {windowSize} tokens">
              <div class="w-20 h-1 rounded-full bg-white/8 overflow-hidden">
                <div class="h-full rounded-full transition-[width] duration-700 {pct > 85 ? 'bg-neon-red' : 'bg-white/40'}" style="width: {pct}%"></div>
              </div>
              {tokens(used)} / {tokens(windowSize)}
            </div>
          {/if}
          <span class="ml-auto text-[10px] 2xl:text-sm text-neutral-600 hidden sm:inline">Enter to send · Shift+Enter for a new line</span>
          {#if chat.streaming}
            <button class="licorice p-2 rounded-xl" onclick={() => chat.stop()} aria-label="Stop"><Square size={15} /></button>
          {:else}
            <button class="licorice p-2 rounded-xl disabled:opacity-40" onclick={send} disabled={!input.trim()} aria-label="Send"><Send size={15} /></button>
          {/if}
        </div>
      </div>
    </div>
  </section>
</div>

<style>
  /* Narrow screens: the conversation list slides in from the left */
  @media (max-width: 47.99rem) {
    .list {
      transform: translateX(-105%);
      transition: transform 0.45s cubic-bezier(0.16, 1, 0.3, 1);
    }

    .list.open {
      transform: none;
    }
  }
</style>
