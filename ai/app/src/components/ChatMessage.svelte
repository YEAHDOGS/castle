<script>
  import { Brain, ChevronDown, Copy, Check, BookmarkPlus } from '@lucide/svelte'
  import { slide } from 'svelte/transition'
  import { api } from '../lib/api.js'
  import { renderMarkdown } from '../lib/markdown.js'
  import { EASE } from '../lib/motion.js'

  /** @type {{ role: string, content: string, thinking?: string | null, live?: boolean, meta?: string }} */
  let { role, content, thinking = '', live = false, meta = '' } = $props()

  const COPIED_MS = 1500
  let showThinking = $state(false)
  let copied = $state(false)
  let saved = $state(false)
  // Markdown output is escaped before any tag is produced (lib/markdown.js), so {@html} is safe here.
  const html = $derived(role === 'assistant' ? renderMarkdown(content) : '')

  function copy() {
    navigator.clipboard?.writeText(content)
    copied = true
    setTimeout(() => (copied = false), COPIED_MS)
  }

  // Chat messages stay out of the prompt library unless saved here.
  async function saveAsPrompt() {
    await api('/api/prompts', { method: 'POST', json: { text: content, kind: 'chat' } })
    saved = true
  }
</script>

{#if role === 'user'}
  <div class="group flex flex-col items-end gap-1">
    <div class="max-w-[85%] md:max-w-[70%] rounded-2xl rounded-br-md glass px-4 py-2.5 text-sm 2xl:text-lg whitespace-pre-wrap break-words select-text">{content}</div>
    <button class="flex items-center gap-1 text-[11px] 2xl:text-sm text-neutral-600 hover:text-white transition-opacity {saved ? '' : 'opacity-0 group-hover:opacity-100 focus:opacity-100'}"
      onclick={saveAsPrompt} disabled={saved}>
      {#if saved}<Check size={12} /> Saved to Prompts{:else}<BookmarkPlus size={12} /> Save as prompt{/if}
    </button>
  </div>
{:else}
  <div class="group flex flex-col gap-1.5 max-w-full">
    {#if thinking}
      <button class="self-start flex items-center gap-1.5 text-xs 2xl:text-base text-muted hover:text-white" onclick={() => (showThinking = !showThinking)}>
        <Brain size={13} class={live && !content ? 'animate-pulse text-neon-red' : ''} />
        {live && !content ? 'Thinking…' : 'Thought process'}
        <ChevronDown size={13} class="transition-transform duration-300 {showThinking ? 'rotate-180' : ''}" />
      </button>
      {#if showThinking}
        <div class="text-xs 2xl:text-base text-neutral-500 border-l-2 border-white/10 pl-3 whitespace-pre-wrap select-text" transition:slide={{ duration: 250, easing: EASE }}>{thinking}</div>
      {/if}
    {/if}
    <div class="md text-sm 2xl:text-lg text-neutral-100 leading-relaxed break-words select-text" class:caret={live}>{@html html}</div>
    {#if !live && content}
      <div class="flex items-center gap-2 text-[11px] 2xl:text-sm text-neutral-600 opacity-0 group-hover:opacity-100 transition-opacity">
        <button class="flex items-center gap-1 hover:text-white" onclick={copy} aria-label="Copy reply">
          {#if copied}<Check size={12} /> Copied{:else}<Copy size={12} /> Copy{/if}
        </button>
        {#if meta}<span>{meta}</span>{/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .md :global(p) { margin: 0 0 0.75em; }
  .md :global(p:last-child) { margin-bottom: 0; }
  .md :global(h3), .md :global(h4), .md :global(h5), .md :global(h6) { font-weight: 700; margin: 1em 0 0.4em; }
  .md :global(h3) { font-size: 1.15em; }
  .md :global(ul), .md :global(ol) { margin: 0 0 0.75em; padding-left: 1.4em; }
  .md :global(ul) { list-style: disc; }
  .md :global(ol) { list-style: decimal; }
  .md :global(li) { margin: 0.2em 0; }
  .md :global(a) { color: var(--color-neon-red); text-decoration: underline; text-underline-offset: 2px; }
  .md :global(blockquote) { border-left: 2px solid rgb(255 51 68 / 40%); padding-left: 0.8em; color: #a3a3a3; margin: 0 0 0.75em; }
  .md :global(hr) { border: 0; border-top: 1px solid rgb(255 255 255 / 10%); margin: 1em 0; }
  .md :global(code) { background: rgb(255 255 255 / 7%); border-radius: 0.35em; padding: 0.1em 0.35em; }
  .md :global(pre) {
    margin: 0 0 0.75em;
    padding: 0.8em 1em;
    border-radius: 0.75rem;
    background: rgb(0 0 0 / 55%);
    border: 1px solid rgb(255 255 255 / 7%);
    overflow-x: auto;
    font-size: 0.9em;
  }
  .md :global(pre code) { background: none; padding: 0; }
  /* Blinking licorice caret while the reply streams in */
  .caret::after {
    content: "";
    display: inline-block;
    width: 0.5em;
    height: 1em;
    margin-left: 0.15em;
    vertical-align: -0.12em;
    border-radius: 2px;
    background: var(--color-neon-red);
    animation: blink 1s steps(2, start) infinite;
  }
  @keyframes blink { to { visibility: hidden; } }
</style>
