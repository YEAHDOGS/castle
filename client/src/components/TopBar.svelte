<!-- TopBar: search + view toggle + New. Minimal words, HUD chrome. -->
<script>
  import { Search, LayoutGrid, List, Plus, X, ArrowDownUp, ArrowUp, ArrowDown, Server } from '@lucide/svelte';
  import { scale } from 'svelte/transition';
  import { store, SORT_LABELS } from '../lib/store.svelte.js';
  import { pop } from '../lib/motion.js';
  import NewMenu from './NewMenu.svelte';
  import Avatar from './Avatar.svelte';
  import { shell } from '../lib/shell.svelte.js';

  let searchEl = $state(null);
  let debounce = null;

  function onSearch(e) {
    clearTimeout(debounce);
    const v = e.target.value;
    debounce = setTimeout(() => store.setQuery(v), 160);
  }

  function clearSearch() {
    if (searchEl) searchEl.value = '';
    store.setQuery('');
  }
</script>

<!-- pr-16 keeps the bar clear of the section wheel button in the corner -->
<header class="flex h-16 shrink-0 items-center gap-2 border-b border-hairline bg-black/40 pl-3 pr-16 md:gap-3 md:pl-5">
  <button class="md:hidden" onclick={() => (shell.userMenuOpen = true)} aria-label={`Signed in as ${shell.user.name}. Change person`}>
    <Avatar person={shell.user} />
  </button>
  <!-- search -->
  <div class="group relative min-w-0 flex-1 md:max-w-md">
    <Search class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-faint transition-colors group-focus-within:text-chiral" strokeWidth={1.75} />
    <input
      bind:this={searchEl}
      type="text"
      placeholder="SEARCH"
      oninput={onSearch}
      class="micro expo t-hover h-10 w-full border border-hairline bg-white/[0.03] pl-9 pr-8 text-white placeholder:text-text-faint focus:border-chiral/60 focus:bg-white/[0.05] focus:outline-none"
      aria-label="Search files"
    />
    {#if store.query}
      <button
        transition:scale={pop()}
        class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-faint hover:text-white"
        onclick={clearSearch}
        aria-label="Clear search"
      >
        <X class="h-3.5 w-3.5" />
      </button>
    {/if}
  </div>

  <div class="flex-1 md:hidden"></div>

  <!-- box connection: cyan when files are on a real box, faint otherwise -->
  <button
    class="expo t-hover flex h-10 items-center gap-1.5 border border-hairline px-2.5 transition-colors {store.boxUrl ? 'text-bridge' : 'text-text-faint hover:text-white'}"
    onclick={() => (store.connectOpen = true)}
    title={store.boxUrl ? `Connected to ${store.boxUrl}` : store.isDemo ? 'Demo: sample files, reset on reload' : 'Local drive. Connect to a Castle box'}
    aria-label={store.boxUrl ? `Connected to ${store.boxUrl}` : 'Connect to a Castle box'}
  >
    <Server class="h-4 w-4" strokeWidth={1.75} />
    <span class="micro hidden lg:inline">{store.boxUrl ? 'BOX' : store.isDemo ? 'DEMO' : 'LOCAL'}</span>
  </button>

  <!-- density toggle: compact grid / comfortable list -->
  <div class="flex items-center border border-hairline" role="group" aria-label="Density">
    <button
      class="expo t-hover p-2.5 transition-colors {store.viewMode === 'grid' ? 'bg-white/[0.07] text-chiral' : 'text-text-faint hover:text-white'}"
      onclick={() => store.setViewMode('grid')}
      aria-label="Compact grid"
      aria-pressed={store.viewMode === 'grid'}
    >
      <LayoutGrid class="h-4 w-4" strokeWidth={1.75} />
    </button>
    <button
      class="expo t-hover p-2.5 transition-colors {store.viewMode === 'list' ? 'bg-white/[0.07] text-chiral' : 'text-text-faint hover:text-white'}"
      onclick={() => store.setViewMode('list')}
      aria-label="Comfortable list"
      aria-pressed={store.viewMode === 'list'}
    >
      <List class="h-4 w-4" strokeWidth={1.75} />
    </button>
  </div>

  <!-- sort control -->
  <div class="flex items-center border border-hairline" role="group" aria-label="Sort">
    <button
      class="expo t-hover flex items-center gap-1.5 p-2.5 transition-colors text-text-faint hover:text-white"
      onclick={() => store.cycleSortKey()}
      title={`Sort by ${SORT_LABELS[store.sortKey]} (${store.sortDir === 1 ? 'ascending' : 'descending'}) — tap to cycle`}
      aria-label={`Sort key: ${SORT_LABELS[store.sortKey]}. Activate to cycle name, size, modified.`}
    >
      <ArrowDownUp class="h-4 w-4" strokeWidth={1.75} />
      <span class="micro hidden lg:inline">{SORT_LABELS[store.sortKey]}</span>
    </button>
    <button
      class="expo t-hover border-l border-hairline p-2.5 transition-colors text-chiral hover:brightness-125"
      onclick={() => store.toggleSortDir()}
      aria-label={store.sortDir === 1 ? 'Sort ascending — activate for descending' : 'Sort descending — activate for ascending'}
    >
      {#if store.sortDir === 1}
        <ArrowUp class="h-4 w-4" strokeWidth={1.75} />
      {:else}
        <ArrowDown class="h-4 w-4" strokeWidth={1.75} />
      {/if}
    </button>
  </div>

  <!-- new -->
  <div class="relative">
    <button
      class="expo t-hover hud-corners flex h-10 items-center gap-2 bg-chiral px-3 font-semibold text-black transition-all hover:brightness-110 active:scale-[0.97] md:px-4"
      onclick={() => (store.newMenuOpen = !store.newMenuOpen)}
      aria-haspopup="menu"
      aria-expanded={store.newMenuOpen}
    >
      <Plus class="h-4 w-4" strokeWidth={2.5} />
      <span class="micro hidden font-bold md:block">NEW</span>
    </button>
    {#if store.newMenuOpen}
      <NewMenu onclose={() => (store.newMenuOpen = false)} />
    {/if}
  </div>
</header>
