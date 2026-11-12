<script>
  import { onMount } from 'svelte'
  import { Plus, UserRound, Image as ImageIcon } from '@lucide/svelte'
  import { scale } from 'svelte/transition'
  import { flip } from 'svelte/animate'
  import { api, url } from '../lib/api.js'
  import { EASE, PAGE_MS } from '../lib/motion.js'
  import CharacterEditor from './CharacterEditor.svelte'

  /** @type {any[]} */ let list = $state([])
  /** @type {string | null} */ let editing = $state(null)
  let error = $state('')

  async function load() {
    try { list = await api('/api/characters') } catch (e) { error = String(e.message || e) }
  }

  async function create() {
    const c = await api('/api/characters', { method: 'POST', json: {} })
    list = [c, ...list]
    editing = c.id
  }

  onMount(load)
</script>

<div class="grid gap-3 grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 2xl:grid-cols-6">
  <button class="card aspect-[4/5] rounded-2xl border border-dashed border-white/15 hover:border-neon-red/60 flex flex-col items-center justify-center gap-2 text-muted hover:text-white"
    onclick={create}>
    <Plus size={26} /><span class="text-sm 2xl:text-lg font-semibold">New character</span>
  </button>
  {#each list as c (c.id)}
    <button class="card group relative aspect-[4/5] rounded-2xl glass overflow-hidden text-left flex flex-col"
      animate:flip={{ duration: PAGE_MS, easing: EASE }} in:scale={{ start: 0.92, duration: PAGE_MS, easing: EASE }}
      onclick={() => (editing = c.id)}>
      <div class="flex-1 min-h-0 bg-black/40 flex items-center justify-center overflow-hidden">
        {#if c.avatarFile}
          <img src={url(`/files/${c.avatarFile}`, true)} alt="" class="w-full h-full object-cover transition-transform duration-700 group-hover:scale-105" />
        {:else}
          <UserRound size={42} class="text-neutral-600" />
        {/if}
      </div>
      <div class="p-3">
        <p class="font-bold truncate 2xl:text-lg">{c.name}</p>
        <p class="text-[11px] 2xl:text-sm text-neutral-500 line-clamp-2 min-h-[2.5em]">{c.bio || 'No bio yet'}</p>
        <p class="text-[10px] 2xl:text-xs text-neutral-600 mt-1 flex items-center gap-1"><ImageIcon size={11} /> {c.media_count} media · {c.lists.length} lists</p>
      </div>
    </button>
  {/each}
</div>
{#if error}<p class="text-xs text-neon-red mt-2">{error}</p>{/if}

{#if editing}
  <CharacterEditor id={editing} onClose={() => { editing = null; load() }} onDeleted={() => { editing = null; load() }} />
{/if}

<style>
  .card {
    transition: transform 0.35s cubic-bezier(0.16, 1, 0.3, 1), border-color 0.3s, box-shadow 0.35s;
  }

  .card:hover {
    transform: translateY(-4px);
    box-shadow: 0 12px 30px rgb(0 0 0 / 50%), 0 0 18px rgb(255 51 68 / 12%);
  }
</style>
