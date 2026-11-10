<!-- FileBrowser: grid/list, selection, inline rename, drag-drop upload.
     Keyed by view+folder so folder drill-ins play a directional view-swap. -->
<script>
  import { Folder, File, FileText, Image as ImageIcon, Music, Film, Archive, Star, CloudOff } from '@lucide/svelte';
  import { fly } from 'svelte/transition';
  import { flip } from 'svelte/animate';
  import { store } from '../lib/store.svelte.js';
  import { handleDriveKey } from '../lib/keys.js';
  import { fmtSize, fmtDate } from '../lib/format.js';
  import { itemIn, viewSwap, expoOut, reducedMotion } from '../lib/motion.js';

  let root = $state(null);
  let renameInput = $state(null);

  // keyboard nav needs to know grid columns
  $effect(() => {
    store.columnsProvider = () => {
      if (!root || store.viewMode !== 'grid') return 1;
      return Math.max(2, Math.floor(root.clientWidth / 190));
    };
  });

  // the just-created folder lands editable in the same beat
  $effect(() => {
    if (store.renamingId && renameInput) {
      renameInput.focus();
      renameInput.select();
    }
  });

  function iconFor(node) {
    if (node.kind === 'folder') return Folder;
    const m = node.mime || '';
    if (m.startsWith('image/')) return ImageIcon;
    if (m.startsWith('audio/')) return Music;
    if (m.startsWith('video/')) return Film;
    if (m.startsWith('text/') || m.includes('pdf') || m.includes('markdown')) return FileText;
    if (m.includes('zip') || m.includes('archive')) return Archive;
    return File;
  }

  const viewKey = $derived(`${store.view}:${store.cwd.id}:${store.query}`);

  // ---- drag & drop upload ----
  function onDragEnter(e) {
    if ([...e.dataTransfer.types].includes('Files')) {
      e.preventDefault();
      store.dragDepth++;
    }
  }
  function onDragLeave(e) {
    if ([...e.dataTransfer.types].includes('Files')) store.dragDepth = Math.max(0, store.dragDepth - 1);
  }
  function onDrop(e) {
    e.preventDefault();
    store.dragDepth = 0;
    if (e.dataTransfer.files?.length) store.uploadFiles(e.dataTransfer.files);
  }

  function commitRename(node, value) {
    store.commitRename(node.id, value);
  }
</script>

<div
  bind:this={root}
  data-kb
  tabindex="-1"
  onkeydown={handleDriveKey}
  class="relative min-h-0 flex-1 overflow-y-auto px-4 py-4 outline-none md:px-6"
  ondragenter={onDragEnter}
  ondragleave={onDragLeave}
  ondragover={(e) => e.preventDefault()}
  ondrop={onDrop}
  onclick={() => { if (!store.renamingId) store.clearSelection(); }}
  role="listbox"
  aria-label="Files"
  aria-multiselectable="true"
>
  {#key viewKey}
    <div in:fly={viewSwap(store.slideDir)}>
      {#if store.items.length === 0}
        <div class="wiregrid flex h-[50dvh] flex-col items-center justify-center gap-3">
          <CloudOff class="h-8 w-8 text-text-faint" strokeWidth={1.25} />
          <p class="micro text-text-faint">EMPTY SECTOR</p>
          <p class="micro text-text-faint opacity-60">DROP FILES OR PRESS CTRL+SHIFT+N</p>
        </div>
      {:else if store.viewMode === 'grid'}
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6">
          {#each store.items as node, i (node.id)}
            {@const Icon = iconFor(node)}
            {@const selected = store.selected.has(node.id)}
            {@const focused = store.focusId === node.id}
            {@const renaming = store.renamingId === node.id}
            <div
              data-node={node.id}
              role="option"
              aria-selected={selected}
              aria-label={node.name}
              tabindex={focused || (!store.focusId && i === 0) ? 0 : -1}
              onkeydown={(e) => { if (handleDriveKey(e)) e.stopPropagation(); }}
              in:fly={itemIn(i)}
              animate:flip={reducedMotion() ? { duration: 0 } : { duration: 200, easing: expoOut }}
              onclick={(e) => { e.stopPropagation(); e.currentTarget.focus({ preventScroll: true }); store.select(node.id, e.ctrlKey || e.metaKey, e.shiftKey); }}
              ondblclick={() => store.openNode(node)}
              oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); store.openMenu(e.clientX, e.clientY, node.id); }}
              class="expo t-hover group relative cursor-pointer border bg-white/[0.02] transition-all
                {selected ? 'border-chiral/70 bg-chiral-dim/40' : 'border-hairline hover:border-white/20 hover:bg-white/[0.04]'}
                {focused ? 'hud-focus' : ''}"
              style={selected ? 'box-shadow: 0 0 18px rgba(255,106,0,.18)' : ''}
            >
              <!-- thumb -->
              <div class="relative aspect-[16/10] overflow-hidden border-b border-hairline-faint bg-black/40">
                {#if node.thumb}
                  <img src={node.thumb} alt="" draggable="false" class="expo t-ui h-full w-full object-cover transition-transform group-hover:scale-[1.03]" loading="lazy" />
                {:else}
                  <div class="grid h-full w-full place-items-center">
                    <Icon
                      class="expo t-hover h-10 w-10 transition-transform group-hover:scale-110 {node.kind === 'folder' ? 'text-chiral' : 'text-text-dim'}"
                      strokeWidth={1.25}
                    />
                  </div>
                {/if}
                {#if node.starred}
                  <Star class="absolute right-2 top-2 h-3.5 w-3.5 fill-chiral text-chiral" />
                {/if}
                {#if node.shared}
                  <span class="micro absolute left-2 top-2 bg-black/60 px-1.5 py-0.5 text-bridge">LINKED</span>
                {/if}
              </div>
              <!-- label -->
              <div class="px-3 py-2.5">
                {#if renaming}
                  <input
                    bind:this={renameInput}
                    type="text"
                    value={node.name}
                    onclick={(e) => e.stopPropagation()}
                    onkeydown={(e) => {
                      // Enter commits; Escape + everything else bubbles to the
                      // browser's key handler (Escape cancels the rename)
                      if (e.key === 'Enter') { e.stopPropagation(); commitRename(node, e.target.value); }
                    }}
                    onblur={(e) => commitRename(node, e.target.value)}
                    class="micro h-6 w-full border border-bridge/60 bg-black/70 px-1 text-white focus:outline-none"
                    aria-label="Rename file"
                  />
                {:else}
                  <p class="truncate text-[13px] font-medium text-white">{node.name}</p>
                  <p class="micro mt-1 text-text-faint">
                    {node.kind === 'folder' ? 'FOLDER' : fmtSize(node.size)} · {fmtDate(node.modified)}
                  </p>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <!-- list mode -->
        <div class="overflow-hidden border border-hairline">
          {#each store.items as node, i (node.id)}
            {@const Icon = iconFor(node)}
            {@const selected = store.selected.has(node.id)}
            {@const focused = store.focusId === node.id}
            {@const renaming = store.renamingId === node.id}
            <div
              data-node={node.id}
              role="option"
              aria-selected={selected}
              tabindex={focused || (!store.focusId && i === 0) ? 0 : -1}
              onkeydown={(e) => { if (handleDriveKey(e)) e.stopPropagation(); }}
              in:fly={itemIn(Math.min(i, 8))}
              onclick={(e) => { e.stopPropagation(); e.currentTarget.focus({ preventScroll: true }); store.select(node.id, e.ctrlKey || e.metaKey, e.shiftKey); }}
              ondblclick={() => store.openNode(node)}
              oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); store.openMenu(e.clientX, e.clientY, node.id); }}
              class="expo t-hover flex cursor-pointer items-center gap-3 border-b border-hairline-faint px-3 py-2.5 transition-colors last:border-0
                {selected ? 'bg-chiral-dim/50' : 'hover:bg-white/[0.03]'}
                {focused ? 'hud-focus' : ''}"
              style={selected ? 'box-shadow: inset 2px 0 0 var(--color-chiral)' : ''}
            >
              <Icon class="h-[18px] w-[18px] shrink-0 {node.kind === 'folder' ? 'text-chiral' : 'text-text-dim'}" strokeWidth={1.75} />
              <div class="min-w-0 flex-1">
                {#if renaming}
                  <input
                    bind:this={renameInput}
                    type="text"
                    value={node.name}
                    onclick={(e) => e.stopPropagation()}
                    onkeydown={(e) => {
                      // Enter commits; Escape + everything else bubbles to the
                      // browser's key handler (Escape cancels the rename)
                      if (e.key === 'Enter') { e.stopPropagation(); commitRename(node, e.target.value); }
                    }}
                    onblur={(e) => commitRename(node, e.target.value)}
                    class="micro h-6 w-full border border-bridge/60 bg-black/70 px-1 text-white focus:outline-none"
                    aria-label="Rename file"
                  />
                {:else}
                  <p class="truncate text-[13px] text-white">{node.name}</p>
                {/if}
              </div>
              {#if node.starred}<Star class="h-3.5 w-3.5 shrink-0 fill-chiral text-chiral" />{/if}
              <span class="micro hidden w-24 shrink-0 text-right text-text-faint sm:block">{fmtDate(node.modified)}</span>
              <span class="micro hidden w-20 shrink-0 text-right text-text-faint md:block">
                {node.kind === 'folder' ? '—' : fmtSize(node.size)}
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/key}

  <!-- drop overlay -->
  {#if store.dragDepth > 0}
    <div class="pointer-events-none absolute inset-3 z-30 grid place-items-center">
      <div class="hud-corners hud-breathe glass px-10 py-8 text-center">
        <p class="micro text-chiral">DROP TO UPLOAD</p>
        <p class="micro mt-2 text-text-faint">{store.cwd.name.toUpperCase()}</p>
      </div>
    </div>
  {/if}
</div>
