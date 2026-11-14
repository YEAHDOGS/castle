<!-- SectionFrame: the layout AI, Arcade and Clones share. Left rail with the section's
     views (a row of chips on phones, where the rail is hidden), a header that leaves room
     for the section wheel, and a scrolling body. -->
<script>
  import { fly } from 'svelte/transition';
  import { shell } from '../lib/shell.svelte.js';
  import { viewIn } from '../lib/motion.js';
  import Sidebar from './Sidebar.svelte';
  import Avatar from './Avatar.svelte';
  import ModeSwitch from './ModeSwitch.svelte';
  import { SHOWCASE } from '../lib/showcase.js';

  /**
   * title, subtitle: header text. tag: small label beside the title (e.g. SAMPLE DATA).
   *   The showcase says SAMPLE DATA once, on its strip at the bottom, so the header drops it there.
   * items/active/onselect: the section's views, as Sidebar takes them.
   * actions: optional snippet on the right of the header. children: the body.
   * bare: the body fills the space edge to edge and does not scroll (for an app shown in a frame).
   */
  let { title, subtitle = '', tag = '', items = [], active = null, onselect = () => {}, actions = null, bare = false, children } = $props();
</script>

<div class="flex h-app overflow-hidden bg-void text-white">
  <div class="hidden h-full shrink-0 md:block">
    <Sidebar {items} {active} {onselect} />
  </div>

  <div class="flex min-w-0 flex-1 flex-col">
    <!-- pr-16 keeps the header clear of the wheel button in the corner -->
    <header class="flex min-h-16 shrink-0 items-center gap-3 border-b border-hairline bg-black/40 py-2 pl-3 pr-16 md:pl-6">
      <button class="md:hidden" onclick={() => (shell.userMenuOpen = true)} aria-label={`Signed in as ${shell.user.name}. Change person`}>
        <Avatar person={shell.user} />
      </button>
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <h1 class="truncate text-[17px] font-black uppercase tracking-[0.16em]">{title}</h1>
          {#if tag && !(SHOWCASE && tag === 'SAMPLE DATA')}
            <span class="micro shrink-0 border border-bridge/40 px-1.5 py-0.5 text-bridge">{tag}</span>
          {/if}
        </div>
        {#if subtitle}
          <p class="micro mt-0.5 truncate text-text-faint">{subtitle}</p>
        {/if}
      </div>
      {#if actions}
        {@render actions()}
      {/if}
      <div class="hidden sm:block"><ModeSwitch /></div>
    </header>

    {#if items.length}
      <nav class="chips flex shrink-0 gap-1 overflow-x-auto border-b border-hairline-faint py-2 pl-3 pr-10 md:hidden" aria-label={`${title} views`}>
        {#each items as item (item.id)}
          <button
            class="micro expo t-hover shrink-0 border px-2.5 py-1.5 transition-colors {active === item.id ? 'border-chiral/70 text-chiral' : 'border-hairline text-text-dim'}"
            onclick={() => onselect(item.id)}
            aria-current={active === item.id ? 'page' : undefined}
          >
            {item.label}{item.count != null ? ` ${item.count}` : ''}{#if item.wip}<span class="ml-1.5 text-chiral">WIP</span>{/if}
          </button>
        {/each}
      </nav>
    {/if}

    <main class="min-h-0 flex-1 {bare ? 'overflow-hidden' : 'overflow-y-auto p-4 md:p-6'}">
      <!-- Every view change in every section animates here, so no section has to remember to. -->
      {#key active}
        <div in:fly={viewIn()} class={bare ? 'h-full' : ''}>
          {@render children()}
        </div>
      {/key}
    </main>
  </div>
</div>

<style>
  /* The phone's row of views scrolls sideways with a finger, without a scroll bar under
     it; the right edge fades out so it is plain there is more to see. */
  .chips {
    scrollbar-width: none;
    mask-image: linear-gradient(90deg, #000 calc(100% - 2.5rem), transparent);
    -webkit-mask-image: linear-gradient(90deg, #000 calc(100% - 2.5rem), transparent);
  }
  .chips::-webkit-scrollbar { display: none; }
</style>
