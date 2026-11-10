<!-- NewMenu: dropdown under the NEW button. Uploads wire to hidden file inputs. -->
<script>
  import { FolderPlus, FileUp, FolderUp } from '@lucide/svelte';
  import { scale, fade } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { pop, reducedMotion } from '../lib/motion.js';

  let { onclose } = $props();
  let fileInput = $state(null);
  let folderInput = $state(null);

  function pick(kind) {
    onclose();
    if (kind === 'folder') store.newFolder();
    else if (kind === 'file') fileInput?.click();
    else folderInput?.click();
  }

  function onFiles(e) {
    store.uploadFiles(e.target.files);
    e.target.value = '';
  }

  const ROWS = [
    { kind: 'folder', label: 'New folder', hint: 'CTRL+SHIFT+N', icon: FolderPlus },
    { kind: 'file', label: 'Upload files', hint: '', icon: FileUp },
    { kind: 'dir', label: 'Upload folder', hint: '', icon: FolderUp },
  ];
</script>

<svelte:window onclick={(e) => { if (!e.target.closest('[data-newmenu]')) onclose(); }} />

<div class="fixed inset-0 z-40" data-newmenu>
  <div
    transition:fade={{ duration: reducedMotion() ? 0 : 150 }}
    class="absolute inset-0"
    onclick={onclose}
    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') onclose(); }}
    role="button"
    tabindex="-1"
    aria-label="Dismiss menu"
  ></div>
  <div
    transition:scale={pop()}
    role="menu"
    class="glass absolute right-3 top-14 z-50 w-60 py-1.5 md:right-5"
    style="transform-origin: top right"
  >
    {#each ROWS as row (row.kind)}
      {@const Icon = row.icon}
      <button
        role="menuitem"
        class="expo t-hover flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-white/[0.05]"
        onclick={() => pick(row.kind)}
      >
        <Icon class="h-4 w-4 text-chiral" strokeWidth={1.75} />
        <span class="flex-1 text-[13px] text-white">{row.label}</span>
        {#if row.hint}
          <span class="micro text-text-faint">{row.hint}</span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<input bind:this={fileInput} type="file" multiple class="hidden" onchange={onFiles} />
<input bind:this={folderInput} type="file" webkitdirectory class="hidden" onchange={onFiles} />
