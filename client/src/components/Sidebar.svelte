<!-- Sidebar: the left rail every section shares. Top: the Castle mark, which opens the
     people menu. Middle: the current section's own navigation, or, in a section with no
     views of its own (Home), the people on this box, so the rail is never empty. Bottom: an optional slot
     (Storage puts its meter there) and the DOGS head, which goes back to Home. -->
<script>
  import dogsLogo from '../assets/dogs-logo.svg';
  import { fly } from 'svelte/transition';
  import { ChevronDown } from '@lucide/svelte';
  import { shell } from '../lib/shell.svelte.js';
  import { sectionById } from '../lib/sections.js';
  import { itemIn } from '../lib/motion.js';
  import Avatar from './Avatar.svelte';
  import PeopleList from './PeopleList.svelte';

  /**
   * items: [{ id, label, icon, count?, expanded?, indent?, action? }] -- the section's views.
   *   expanded (true/false) marks a group heading that opens and closes; indent marks its children;
   *   action marks a row that does something rather than show a view (Arcade's Shuffle).
   *   A controller's LB and RB step through the views, past headings and actions.
   * active: id of the current view; onselect(id) is called when one is picked
   * footer: optional snippet shown above the badge
   */
  let { items = [], active = null, onselect = () => {}, footer = null } = $props();

  const section = $derived(sectionById(shell.section));

  /** LB / RB (see lib/input.svelte.js): the previous or next view, round and round. */
  function onStep(e) {
    const views = items.filter((item) => item.expanded == null && !item.action);
    if (views.length < 2) return;
    const at = views.findIndex((item) => item.id === active);
    const next = views[(at + e.detail + views.length) % views.length];
    if (next) onselect(next.id);
  }

  $effect(() => {
    window.addEventListener('castle-view', onStep);
    return () => window.removeEventListener('castle-view', onStep);
  });
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
    {#if !items.length}
      <!-- wide rail only: the narrow one has no room for names or a PIN box -->
      <div class="hidden lg:block">
        <p class="micro px-2 pb-2 pt-1 text-text-faint">WHO IS USING CASTLE</p>
        <PeopleList />
      </div>
    {/if}
    {#each items as item, i (item.id)}
      {@const Icon = item.icon}
      {@const isActive = active === item.id}
      <button
        in:fly={itemIn(i)}
        class="expo t-hover relative flex w-full items-center gap-3 px-2 py-2.5 transition-all lg:px-3
          {item.indent ? 'lg:pl-10' : ''} {isActive ? 'text-white' : 'text-text-dim hover:text-white'}"
        onclick={() => onselect(item.id)}
        aria-current={isActive ? 'page' : undefined}
        aria-expanded={item.expanded ?? undefined}
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
        {#if item.expanded != null}
          <ChevronDown class="expo t-ui relative hidden h-4 w-4 shrink-0 text-text-faint transition-transform lg:block {item.expanded ? '' : '-rotate-90'}" strokeWidth={1.75} />
        {/if}
      </button>
    {/each}
  </nav>

  {#if footer}
    {@render footer()}
  {/if}

  <!-- The DOGS head is the way home from anywhere. -->
  <button
    class="expo t-hover block w-full border-t border-hairline-faint px-3 py-3 text-left opacity-70 transition-opacity hover:opacity-100 lg:px-6"
    onclick={() => shell.goHome()}
    aria-label="Go to Home"
    title="Home"
  >
    <img src={dogsLogo} alt="" class="h-10 w-10" />
  </button>
</aside>
