<!-- ComputerView: the top of This computer. The vault, every drive, and the plugged-in
     media with when it went in and how fast it is. A drive opens in the file browser. -->
<script>
  import { HardDrive, Usb, Disc3, MemoryStick, Network, Vault, Gauge, LoaderCircle, Clock3, ShieldOff, CornerLeftUp } from '@lucide/svelte';
  import { fly } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { ask } from '../lib/ask.svelte.js';
  import { fmtSize } from '../lib/format.js';
  import { fmtStamp, fmtSince } from '../lib/computer.js';
  import { itemIn } from '../lib/motion.js';

  const TEST_SIZE_MB = 256;
  const CLOCK_TICK_MS = 1000;

  const computer = $derived(store.computer);
  const fixed = $derived(computer ? computer.drives.filter((d) => !d.removable) : []);
  const removable = $derived(computer ? computer.drives.filter((d) => d.removable) : []);

  // "plugged in for 12 min" keeps counting while the page is open.
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), CLOCK_TICK_MS);
    return () => clearInterval(timer);
  });

  function iconFor(drive) {
    if (drive.kind === 'optical') return Disc3;
    if (drive.kind === 'network') return Network;
    if (drive.bus === 'SD' || drive.bus === 'MMC') return MemoryStick;
    if (drive.removable) return Usb;
    return HardDrive;
  }

  const kindLabel = (drive) =>
    ({ fixed: 'INTERNAL', external: 'EXTERNAL DISK', removable: drive.bus === 'SD' ? 'MEMORY CARD' : 'USB DRIVE', optical: 'DISC', network: 'NETWORK', memory: 'MEMORY' })[drive.kind] || drive.kind.toUpperCase();

  const usedPercent = (drive) => (drive.total ? Math.min(100, ((drive.total - drive.free) / drive.total) * 100) : 0);

  function open(drive) {
    if (!drive.media) return store.toast(drive.kind === 'optical' ? 'There is no disc in the drive' : 'Nothing in that drive');
    store.openHost(drive.path);
  }

  async function testWrite(drive) {
    const question =
      `Castle writes a test file of up to ${TEST_SIZE_MB} MB to ${drive.name}, reads it back, and deletes it. ` +
      'Nothing already on the drive is touched. Run the test?';
    if (await ask(question, { yes: 'Run the test' })) store.speedTest(drive, true);
  }
</script>

{#snippet usage(drive)}
  {#if drive.media && drive.total}
    <div class="mt-3 h-[3px] overflow-hidden rounded-full bg-white/[0.07]">
      <div
        class="h-full rounded-full"
        style="width: {usedPercent(drive)}%; background: {usedPercent(drive) > 90 ? 'var(--color-chiral)' : 'linear-gradient(90deg, var(--color-chiral), var(--color-bridge))'};"
      ></div>
    </div>
    <p class="micro mt-1.5 text-text-faint">{fmtSize(drive.free)} FREE OF {fmtSize(drive.total)}</p>
  {:else}
    <p class="micro mt-3 text-text-faint">{drive.kind === 'optical' ? 'NO DISC' : 'NO MEDIA'}</p>
  {/if}
{/snippet}

{#snippet speed(test)}
  {#if test}
    <div class="flex items-baseline gap-4">
      {#if test.writeMbps != null}
        <span><span class="font-mono text-[17px] text-white tabular-nums">{test.writeMbps}</span> <span class="micro text-text-faint">MB/S WRITE</span></span>
      {/if}
      <span><span class="font-mono text-[17px] text-bridge tabular-nums">{test.readMbps}</span> <span class="micro text-text-faint">MB/S READ</span></span>
    </div>
    <p class="micro mt-1 text-text-faint">TESTED {fmtStamp(test.at).toUpperCase()}{test.mode === 'read' ? ' · READ ONLY' : ''}</p>
  {:else}
    <p class="micro text-text-faint">NOT TESTED YET</p>
  {/if}
{/snippet}

<div class="min-h-0 flex-1 overflow-y-auto px-4 py-4 md:px-6">
  {#if !computer}
    <div class="wiregrid flex h-[50dvh] flex-col items-center justify-center gap-3 px-6 text-center">
      <ShieldOff class="h-8 w-8 text-text-faint" strokeWidth={1.25} />
      <p class="micro text-text-faint">THIS COMPUTER IS NOT AVAILABLE</p>
      <p class="max-w-md text-[13px] text-text-dim">{store.computerOff || 'This drive is kept in the browser. Connect to a Castle box to open the computer it runs on.'}</p>
    </div>
  {:else}
    <!-- the vault, and the way back to it -->
    <section aria-label="Castle vault">
      <button
        class="expo t-hover hud-corners glass group flex w-full items-center gap-4 px-5 py-4 text-left transition-colors hover:bg-white/[0.04]"
        onclick={() => store.setView('drive')}
      >
        <Vault class="h-8 w-8 shrink-0 text-chiral" strokeWidth={1.25} />
        <div class="min-w-0 flex-1">
          <p class="text-[15px] font-semibold text-white">My Drive</p>
          <p class="micro mt-0.5 truncate text-text-faint">THE CASTLE VAULT · {computer.vault.toUpperCase()}</p>
        </div>
        <CornerLeftUp class="expo t-hover h-5 w-5 rotate-90 text-text-faint transition-transform group-hover:translate-x-1" strokeWidth={1.75} />
      </button>
    </section>

    <!-- plugged-in media first: that is what changes -->
    <section class="mt-7" aria-labelledby="removable-heading">
      <div class="mb-3 flex items-baseline justify-between">
        <h2 id="removable-heading" class="micro text-chiral">REMOVABLE MEDIA</h2>
        <span class="micro text-text-faint">CHECKED {fmtStamp(computer.at).toUpperCase()}</span>
      </div>
      {#if removable.length === 0}
        <div class="border border-dashed border-hairline px-5 py-6 text-center">
          <Usb class="mx-auto h-6 w-6 text-text-faint" strokeWidth={1.25} />
          <p class="micro mt-2 text-text-faint">PLUG IN A USB DRIVE, A MEMORY CARD OR A DISC</p>
          <p class="mt-1 text-[12px] text-text-faint">It appears here within a few seconds, with the time it went in.</p>
        </div>
      {:else}
        <div class="grid gap-3 lg:grid-cols-2">
          {#each removable as drive, i (drive.key)}
            {@const Icon = iconFor(drive)}
            {@const busyWrite = store.testing === `${drive.path}|write`}
            {@const busyRead = store.testing === `${drive.path}|read`}
            <article in:fly={itemIn(i)} class="glass hud-corners flex flex-col px-5 py-4" aria-label={drive.name}>
              <div class="flex items-start gap-4">
                <Icon class="mt-0.5 h-8 w-8 shrink-0 text-bridge" strokeWidth={1.25} />
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 class="truncate text-[15px] font-semibold text-white">{drive.name}</h3>
                    <span class="micro border border-bridge/40 px-1.5 py-px text-bridge">{kindLabel(drive)}</span>
                    {#if drive.readOnly}<span class="micro border border-hairline px-1.5 py-px text-text-dim">READ ONLY</span>{/if}
                  </div>
                  <p class="micro mt-1 truncate text-text-faint">
                    {[drive.model, drive.bus, drive.fileSystem].filter(Boolean).join(' · ').toUpperCase() || '—'}
                  </p>
                  {@render usage(drive)}
                </div>
              </div>

              <dl class="mt-4 grid grid-cols-2 gap-x-4 gap-y-2.5 border-t border-hairline-faint pt-3">
                <div>
                  <dt class="micro text-text-faint">PLUGGED IN</dt>
                  <dd class="mt-0.5 flex items-center gap-1.5 text-[13px] text-white">
                    <Clock3 class="h-3.5 w-3.5 text-chiral" strokeWidth={1.75} />{fmtStamp(drive.pluggedIn)}
                  </dd>
                  <dd class="micro mt-0.5 text-text-faint">IN FOR {fmtSince(drive.pluggedIn, now).toUpperCase()}</dd>
                </div>
                <div>
                  <dt class="micro text-text-faint">FIRST SEEN</dt>
                  <dd class="mt-0.5 text-[13px] text-white">{fmtStamp(drive.firstSeen)}</dd>
                  {#if drive.serial}<dd class="micro mt-0.5 text-text-faint">VOLUME {drive.serial}</dd>{/if}
                </div>
                <div class="col-span-2">
                  <dt class="micro mb-1 text-text-faint">SPEED</dt>
                  <dd>{@render speed(drive.lastTest)}</dd>
                </div>
              </dl>

              <div class="mt-4 flex flex-wrap gap-2">
                <button class="expo t-hover micro border border-hairline px-3 py-2 text-white transition-colors hover:border-white/30" onclick={() => open(drive)}>
                  OPEN
                </button>
                {#if drive.media && !drive.readOnly}
                  <button
                    class="expo t-hover micro flex items-center gap-1.5 border border-chiral/50 px-3 py-2 text-chiral transition-colors hover:bg-chiral/10 disabled:opacity-50"
                    onclick={() => testWrite(drive)}
                    disabled={!!store.testing}
                  >
                    {#if busyWrite}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />TESTING…{:else}<Gauge class="h-3.5 w-3.5" />TEST SPEED{/if}
                  </button>
                {/if}
                {#if drive.media}
                  <button
                    class="expo t-hover micro flex items-center gap-1.5 border border-hairline px-3 py-2 text-text-dim transition-colors hover:text-white disabled:opacity-50"
                    onclick={() => store.speedTest(drive, false)}
                    disabled={!!store.testing}
                    title="Reads files already on the drive. Writes nothing."
                  >
                    {#if busyRead}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />TESTING…{:else}READ-ONLY TEST{/if}
                  </button>
                {/if}
              </div>
            </article>
          {/each}
        </div>
      {/if}
    </section>

    <!-- the drives that stay -->
    <section class="mt-7" aria-labelledby="drives-heading">
      <h2 id="drives-heading" class="micro mb-3 text-chiral">DRIVES</h2>
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {#each fixed as drive, i (drive.key)}
          {@const Icon = iconFor(drive)}
          <button
            in:fly={itemIn(i)}
            class="expo t-hover group border border-hairline bg-white/[0.02] px-4 py-4 text-left transition-all hover:border-white/20 hover:bg-white/[0.04]"
            onclick={() => open(drive)}
            ondblclick={() => open(drive)}
          >
            <div class="flex items-start gap-3">
              <Icon class="expo t-hover h-7 w-7 shrink-0 text-chiral transition-transform group-hover:scale-110" strokeWidth={1.25} />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <p class="truncate text-[14px] font-semibold text-white">{drive.name}</p>
                  {#if drive.vault}<span class="micro border border-chiral/40 px-1.5 py-px text-chiral">VAULT</span>{/if}
                </div>
                <p class="micro mt-0.5 truncate text-text-faint">{[drive.model, drive.bus, drive.fileSystem].filter(Boolean).join(' · ').toUpperCase()}</p>
              </div>
            </div>
            {@render usage(drive)}
            {#if drive.lastTest}
              <p class="micro mt-2 text-text-faint">
                {drive.lastTest.writeMbps != null ? `${drive.lastTest.writeMbps} WRITE · ` : ''}{drive.lastTest.readMbps} READ MB/S
              </p>
            {/if}
          </button>
        {/each}
      </div>
    </section>

    {#if computer.gone.length}
      <section class="mt-7" aria-labelledby="gone-heading">
        <h2 id="gone-heading" class="micro mb-3 text-text-dim">SEEN BEFORE</h2>
        <div class="overflow-hidden border border-hairline">
          {#each computer.gone as drive (drive.path + drive.lastSeen)}
            <div class="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-hairline-faint px-4 py-3 last:border-0">
              <p class="min-w-0 flex-1 truncate text-[13px] text-text-dim">{drive.name}</p>
              <span class="micro text-text-faint">{fmtSize(drive.total)}</span>
              <span class="micro text-text-faint">LAST IN {fmtStamp(drive.lastSeen).toUpperCase()}</span>
              {#if drive.lastTest}
                <span class="micro text-text-faint">{drive.lastTest.writeMbps != null ? `${drive.lastTest.writeMbps} W · ` : ''}{drive.lastTest.readMbps} R MB/S</span>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>
