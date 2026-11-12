<script>
  import { Image, Clapperboard, AudioLines } from '@lucide/svelte'
  import { store, KINDS } from '../lib/store.svelte.js'

  const ICONS = { image: Image, video: Clapperboard, audio: AudioLines }
  const index = $derived(KINDS.findIndex((k) => k.id === store.kind))
  const readyCount = (/** @type {string} */ kind) => store.catalog.models.filter((m) => m.kind === kind && m.status === 'ready').length

  function onKey(/** @type {KeyboardEvent} */ e) {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key]
    if (!step) return
    e.preventDefault()
    store.setKind(KINDS[(index + step + KINDS.length) % KINDS.length].id)
  }
</script>

<div class="switch relative grid grid-cols-3 p-1 rounded-xl well" role="tablist" aria-label="What to create" tabindex="-1" onkeydown={onKey} style="--i: {index}">
  <span class="pill" aria-hidden="true"></span>
  {#each KINDS as k (k.id)}
    {@const Icon = ICONS[k.id]}
    {@const active = store.kind === k.id}
    <button class="opt relative z-10 flex items-center justify-center gap-1.5 py-2 2xl:py-3 text-xs 2xl:text-base font-semibold" class:active
      role="tab" aria-selected={active} tabindex={active ? 0 : -1} onclick={() => store.setKind(k.id)}
      title="{readyCount(k.id)} {k.label.toLowerCase()} model(s) ready">
      <Icon size={15} /> {k.label}
    </button>
  {/each}
</div>

<style>
  .pill {
    position: absolute;
    top: 0.25rem;
    bottom: 0.25rem;
    left: 0.25rem;
    width: calc((100% - 0.5rem) / 3);
    border-radius: 0.6rem;
    background: rgb(255 51 68 / 16%);
    border: 1px solid rgb(255 51 68 / 40%);
    box-shadow: 0 0 14px rgb(255 51 68 / 22%);
    transform: translateX(calc(var(--i) * 100%));
    transition: transform 0.45s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .opt {
    color: var(--color-muted);
    transition: color 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .opt:hover {
    color: #fff;
  }

  .opt.active {
    color: #fff;
  }

  @media (prefers-reduced-motion: reduce) {
    .pill {
      transition: none;
    }
  }
</style>
