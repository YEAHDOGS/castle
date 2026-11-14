<!-- StorageSection: the drive. Views in the rail, search and New in the bar, files below. -->
<script>
  import { onMount } from 'svelte';
  import { HardDrive, Users, History, Star, Trash2, Monitor, Usb, Disc3, MemoryStick } from '@lucide/svelte';
  import { attachShortcuts } from '../../lib/keys.js';
  import { store } from '../../lib/store.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import Sidebar from '../Sidebar.svelte';
  import TopBar from '../TopBar.svelte';
  import Breadcrumbs from '../Breadcrumbs.svelte';
  import FileBrowser from '../FileBrowser.svelte';
  import ComputerView from '../ComputerView.svelte';
  import { hostId } from '../../lib/computer.js';

  const VIEWS = [
    { id: 'drive', label: 'My Drive', short: 'Drive', icon: HardDrive },
    { id: 'computer', label: 'This computer', short: 'Computer', icon: Monitor },
    { id: 'shared', label: 'Shared', short: 'Shared', icon: Users },
    { id: 'recent', label: 'Recent', short: 'Recent', icon: History },
    { id: 'starred', label: 'Starred', short: 'Starred', icon: Star },
    { id: 'trash', label: 'Trash', short: 'Trash', icon: Trash2 },
  ];
  const FULL = 100;
  // How often the drive list is asked for while Storage is open, so a drive plugged in shows up.
  const DRIVE_POLL_MS = 4000;
  const DRIVE_ITEM = 'drive:';

  const driveIcon = (drive) => (drive.kind === 'optical' ? Disc3 : drive.bus === 'SD' || drive.bus === 'MMC' ? MemoryStick : Usb);

  // This computer appears only where there is one; each plugged-in drive gets its own row under it.
  const views = $derived.by(() => {
    const list = VIEWS.filter((v) => v.id !== 'computer' || store.computer);
    if (!store.computer) return list;
    const plugged = store.computer.drives
      .filter((d) => d.removable)
      .map((d) => ({ id: DRIVE_ITEM + d.path, label: d.name, short: d.name, icon: driveIcon(d), indent: true }));
    const at = list.findIndex((v) => v.id === 'computer');
    return [...list.slice(0, at + 1), ...plugged, ...list.slice(at + 1)];
  });

  // Inside a plugged-in drive, its own row is the one lit.
  const active = $derived.by(() => {
    if (store.view !== 'computer') return store.view;
    const top = store.path[1]?.id;
    const drive = store.computer?.drives.find((d) => d.removable && hostId(d.path) === top);
    return drive ? DRIVE_ITEM + drive.path : 'computer';
  });

  function pick(id) {
    if (id.startsWith(DRIVE_ITEM)) store.openHost(id.slice(DRIVE_ITEM.length));
    else store.setView(id);
  }

  let stats = $state({ used: 0, quota: 1 });
  const usedPercent = $derived(stats.quota ? Math.min(FULL, (stats.used / stats.quota) * FULL) : 0);

  // Drive shortcuts (Delete, type-ahead, arrows) belong to this section only.
  onMount(attachShortcuts);

  onMount(() => {
    const timer = setInterval(() => store.loadComputer(), DRIVE_POLL_MS);
    return () => clearInterval(timer);
  });

  $effect(() => {
    store.items; // refresh the meter whenever the drive changes
    store.adapter.stats().then((s) => (stats = s)).catch(() => {}); // unreachable box: keep the last reading
  });
</script>

{#snippet meter()}
  <div class="hidden border-t border-hairline-faint px-6 py-4 lg:block">
    <div class="micro mb-2 flex justify-between text-text-faint">
      <span>STORAGE</span>
      <span class="text-text-dim">{Math.round(usedPercent)}%</span>
    </div>
    <div class="h-[3px] overflow-hidden rounded-full bg-white/[0.07]">
      <div
        class="expo t-ui h-full rounded-full transition-[width]"
        style="width: {usedPercent}%; background: linear-gradient(90deg, var(--color-chiral), var(--color-bridge));"
      ></div>
    </div>
    <div class="micro mt-2 text-text-faint">{fmtSize(stats.used)} / {fmtSize(stats.quota)}</div>
  </div>
{/snippet}

<div class="flex h-app overflow-hidden bg-void text-white">
  <div class="hidden h-full shrink-0 md:block">
    <Sidebar items={views} {active} onselect={pick} footer={meter} />
  </div>

  <div class="flex min-w-0 flex-1 flex-col pb-16 md:pb-0">
    <TopBar />
    <Breadcrumbs />
    {#if store.atComputerTop}
      <ComputerView />
    {:else}
      <FileBrowser />
    {/if}
  </div>
</div>

<!-- phone tab rail -->
<nav class="glass fixed inset-x-0 bottom-[var(--strip)] z-40 flex border-x-0 border-b-0 md:hidden" aria-label="Drive views">
  {#each VIEWS.filter((v) => v.id !== 'computer' || store.computer) as item (item.id)}
    {@const Icon = item.icon}
    {@const active = store.view === item.id}
    <button
      class="expo t-hover relative flex flex-1 flex-col items-center gap-1 py-2.5 transition-colors {active ? 'text-chiral' : 'text-text-faint'}"
      onclick={() => store.setView(item.id)}
      aria-current={active ? 'page' : undefined}
    >
      <Icon class="h-5 w-5" strokeWidth={1.75} />
      <span class="micro" style="font-size: 8px">{item.short.toUpperCase()}</span>
      {#if active}
        <span class="absolute top-0 h-[2px] w-8 bg-chiral"></span>
      {/if}
    </button>
  {/each}
</nav>
