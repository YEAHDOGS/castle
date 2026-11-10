<!-- Sidebar: the left rail every section shares. Top: the Castle mark, which opens the
     people menu. Middle: the current section's own navigation. Bottom: an optional slot
     (Storage puts its meter there) and the DOGS badge. -->
<script>
  import dogsLogo from '../assets/dogs-logo.svg';
  import { fly } from 'svelte/transition';
  import { shell } from '../lib/shell.svelte.js';
  import { sectionById } from '../lib/sections.js';
  import { itemIn } from '../lib/motion.js';
  import Avatar from './Avatar.svelte';

  /**
   * items: [{ id, label, icon, count? }] -- the section's views
   * active: id of the current view; onselect(id) is called when one is picked
   * footer: optional snippet shown above the badge
   */
  let { items = [], active = null, onselect = () => {}, footer = null } = $props();

  const section = $derived(sectionById(shell.section));
</script>

<aside class="scanlines flex h-full w-16 flex-col border-r border-hairline bg-black/40 lg:w-60">
  <!-- Castle mark + who is signed in: opens the people menu -->
  <button
    class="expo t-hover group flex items-center gap-3 px-4 py-4 text-left transition-colors hover:bg-white/[0.03] lg:px-5"
    onclick={() => (shell.userMenuOpen = !shell.userMenuOpen)}
    aria-label={`Castle. Signed in as ${shell.user.name}. Change person`}
    aria-haspopup="dialog"
    aria-expanded={shell.userMenuOpen}
  >
    <Avatar person={shell.user} />
    <span class="hidden min-w-0 lg:block">
      <span class="block text-[15px] font-black tracking-[0.22em] text-white">CASTLE</span>
      <span class="micro block truncate text-text-faint">{shell.user.name} · {section.label}</span>
    </span>
  </button>

  <!-- the section's views -->
  <nav class="relative flex-1 space-y-1 overflow-y-auto px-2 py-2 lg:px-3" aria-label={`${section.label} views`}>
    {#each items as item, i (item.id)}
      {@const Icon = item.icon}
      {@const isActive = active === item.id}
      <button
        in:fly={itemIn(i)}
        class="expo t-hover relative flex w-full items-center gap-3 px-2 py-2.5 transition-all lg:px-3
          {isActive ? 'text-white' : 'text-text-dim hover:text-white'}"
        onclick={() => onselect(item.id)}
        aria-current={isActive ? 'page' : undefined}
        title={item.label}
      >
        {#if isActive}
          <span class="absolute inset-0 border border-hairline bg-white/[0.05]" style="box-shadow: inset 2px 0 0 var(--color-chiral)"></span>
        {/if}
        {#if Icon}
          <Icon class="relative h-[18px] w-[18px] shrink-0 {isActive ? 'text-chiral' : ''}" strokeWidth={1.75} />
        {:else}
          <span class="micro relative w-[18px] shrink-0 text-center {isActive ? 'text-chiral' : ''} lg:hidden">{item.label.slice(0, 2)}</span>
        {/if}
        <span class="relative hidden min-w-0 flex-1 truncate text-left text-[13px] font-medium tracking-wide lg:block">{item.label}</span>
        {#if item.count != null}
          <span class="micro relative hidden text-text-faint lg:block">{item.count}</span>
        {/if}
      </button>
    {/each}
  </nav>

  {#if footer}
    {@render footer()}
  {/if}

  <a
    href="https://wearedogs.net"
    target="_blank"
    rel="noreferrer"
    class="expo t-hover block border-t border-hairline-faint px-6 py-3 opacity-70 transition-opacity hover:opacity-100"
    aria-label="DOGS"
    title="DOGS"
  >
    <img src={dogsLogo} alt="DOGS" class="h-10 w-10" />
  </a>
</aside>
