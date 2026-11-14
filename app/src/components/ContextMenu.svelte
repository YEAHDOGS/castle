<!-- ContextMenu: right-click actions. Pops at the cursor, expo-out. -->
<script>
  import { FolderOpen, PenLine, Star, StarOff, Download, Trash2, RotateCcw, XOctagon } from '@lucide/svelte';
  import { scale, fade } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { isHostId } from '../lib/computer.js';
  import { pop, reducedMotion } from '../lib/motion.js';

  const menu = $derived(store.menu);
  const node = $derived(menu ? store.items.find((i) => i.id === menu.id) : null);

  let el = $state(null);
  let pos = $state({ x: 0, y: 0 });

  $effect(() => {
    if (menu && el) {
      const r = el.getBoundingClientRect();
      pos = {
        x: Math.min(menu.x, window.innerWidth - r.width - 8),
        y: Math.min(menu.y, window.innerHeight - r.height - 8),
      };
    }
  });

  const rows = $derived.by(() => {
    if (!node) return [];
    if (store.isTrash) {
      return [
        { label: 'Restore', icon: RotateCcw, fn: () => store.restoreSelection() },
        { label: 'Delete forever', icon: XOctagon, danger: true, fn: () => store.deleteForeverSelection() },
      ];
    }
    // A file on the computer: no stars, and a delete goes to the system's bin.
    if (isHostId(node.id)) {
      const rows = [
        { label: node.kind === 'folder' ? 'Open' : 'Preview', icon: FolderOpen, fn: () => store.openNode(node) },
        { label: 'Rename', hint: 'F2', icon: PenLine, fn: () => store.startRename(node.id) },
      ];
      if (node.kind === 'file') rows.push({ label: 'Download', icon: Download, fn: () => store.download(node.id) });
      rows.push({ label: `Move to ${store.binName.replace(/^the /, '')}`, hint: 'DEL', icon: Trash2, danger: true, fn: () => store.trashSelection() });
      rows.push({ label: 'Delete forever', icon: XOctagon, danger: true, fn: () => store.deleteForeverSelection() });
      return rows;
    }
    const base = [
      { label: node.kind === 'folder' ? 'Open' : 'Preview', icon: FolderOpen, fn: () => store.openNode(node) },
      { label: 'Rename', hint: 'F2', icon: PenLine, fn: () => store.startRename(node.id) },
      node.starred
        ? { label: 'Unstar', icon: StarOff, fn: () => store.toggleStar(node.id) }
        : { label: 'Star', icon: Star, fn: () => store.toggleStar(node.id) },
    ];
    if (node.kind === 'file') base.push({ label: 'Download', icon: Download, fn: () => store.download(node.id) });
    base.push({ label: 'Move to Trash', hint: 'DEL', icon: Trash2, danger: true, fn: () => store.trashSelection() });
    return base;
  });
</script>

{#if menu}
  <!-- modal for the keyboard and a controller: focus goes into the menu, and back to the file after -->
  <div class="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-label="Actions">
    <div transition:fade={{ duration: reducedMotion() ? 0 : 120 }} class="absolute inset-0" onclick={() => store.closeMenu()} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') store.closeMenu(); }} role="button" tabindex="-1" aria-label="Dismiss menu"></div>
    <div
      bind:this={el}
      transition:scale={pop()}
      role="menu"
      class="glass absolute z-50 w-56 py-1.5"
      style="left: {pos.x}px; top: {pos.y}px; transform-origin: top left;"
    >
      {#if node}
        <p class="micro truncate px-4 pb-1.5 pt-1 text-text-faint">{node.name.toUpperCase()}</p>
      {/if}
      {#each rows as row (row.label)}
        {@const Icon = row.icon}
        <button
          role="menuitem"
          class="expo t-hover flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-white/[0.05] {row.danger ? 'text-red-400' : 'text-white'}"
          onclick={() => { store.closeMenu(); row.fn(); }}
        >
          <Icon class="h-4 w-4 {row.danger ? '' : 'text-text-dim'}" strokeWidth={1.75} />
          <span class="flex-1 text-[13px]">{row.label}</span>
          {#if row.hint}<span class="micro text-text-faint">{row.hint}</span>{/if}
        </button>
      {/each}
    </div>
  </div>
{/if}
