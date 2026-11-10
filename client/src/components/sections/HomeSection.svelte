<!-- HomeSection: the box at a glance. Three live gauges for the machine it runs on, and a
     tile for every other section showing its state right now. A tile opens its section. -->
<script>
  import { untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import { House, ArrowRight } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { SECTIONS } from '../../lib/sections.js';
  import { fmtSize } from '../../lib/format.js';
  import { itemIn } from '../../lib/motion.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';

  const HOME = 'home';
  const VITALS_MS = 2000;
  const STATUS_MS = 15_000;
  const PERCENT = 100;
  const RING_RADIUS = 52;
  const RING_LENGTH = 2 * Math.PI * RING_RADIUS;
  const BUSY_PERCENT = 85; // a gauge turns orange from here up
  const MINUTE = 60;
  const HOUR = 60 * MINUTE;
  const DAY = 24 * HOUR;

  let vitals = $state(null);
  let problem = $state('');
  let statuses = $state({}); // section id -> one line about its state now

  const source = $derived(shell.boxSource);
  const tiles = SECTIONS.filter((s) => s.id !== HOME);
  const gauges = $derived(
    vitals
      ? [
          { id: 'cpu', label: 'PROCESSOR', percent: vitals.cpuPercent, detail: `${vitals.cores} cores` },
          { id: 'memory', label: 'MEMORY', percent: share(vitals.memoryUsed, vitals.memoryTotal), detail: `${fmtSize(vitals.memoryUsed)} of ${fmtSize(vitals.memoryTotal)}` },
          { id: 'disk', label: 'VAULT DISK', percent: share(vitals.diskTotal - vitals.diskFree, vitals.diskTotal), detail: `${fmtSize(vitals.diskFree)} free of ${fmtSize(vitals.diskTotal)}` },
        ]
      : [],
  );

  function share(part, whole) {
    return whole > 0 ? (part / whole) * PERCENT : 0;
  }

  /** 190000 seconds -> "2 d 4 h"; under a day -> "4 h 12 min". */
  function uptime(seconds) {
    const days = Math.floor(seconds / DAY);
    const hours = Math.floor((seconds % DAY) / HOUR);
    const minutes = Math.floor((seconds % HOUR) / MINUTE);
    return days ? `${days} d ${hours} h` : hours ? `${hours} h ${minutes} min` : `${minutes} min`;
  }

  async function readVitals(from) {
    try {
      vitals = await from.system();
      problem = '';
    } catch (err) {
      problem = err?.message || 'The box did not answer';
    }
  }

  /** One short line per section. A section that is off or unreachable keeps its tagline. */
  async function readStatuses() {
    const readers = {
      storage: async () => {
        const stats = await store.adapter.stats();
        return `${fmtSize(stats.used)} stored`;
      },
      clones: async () => {
        const list = await shell.clonesSource.clones();
        const running = list.filter((a) => a.state === 'running').length;
        return running ? `${running} running` : `${list.length} ${list.length === 1 ? 'automation' : 'automations'}, none running`;
      },
      arcade: async () => {
        const systems = await shell.arcadeSource.arcadeSystems();
        const games = systems.reduce((sum, system) => sum + (system.games || 0), 0);
        return `${games} games on ${systems.length} systems`;
      },
      security: async () => {
        const { checks } = await shell.securitySource.security();
        const failing = checks.filter((c) => c.state === 'warn').length;
        return failing ? `${failing} ${failing === 1 ? 'check needs' : 'checks need'} attention` : `${checks.filter((c) => c.state === 'ok').length} checks pass`;
      },
      network: async () => {
        const reading = await shell.networkSource.network();
        return reading.local?.address ? `${reading.local.adapter} · ${reading.local.address}` : 'Connected';
      },
    };
    const next = {};
    await Promise.all(
      Object.entries(readers).map(async ([id, read]) => {
        try {
          next[id] = await read();
        } catch {
          /* off, or the box did not answer: the tile keeps its tagline */
        }
      }),
    );
    statuses = next;
  }

  $effect(() => {
    const from = source;
    if (!from) return;
    untrack(() => {
      vitals = null;
      statuses = {};
      readVitals(from);
      readStatuses();
    });
    const fast = setInterval(() => readVitals(from), VITALS_MS);
    const slow = setInterval(readStatuses, STATUS_MS);
    return () => {
      clearInterval(fast);
      clearInterval(slow);
    };
  });
</script>

<SectionFrame
  title="Home"
  subtitle={vitals ? `${vitals.host || 'This box'} · ${vitals.os} · up ${uptime(vitals.uptimeSeconds)}` : 'The box at a glance'}
  tag={store.isDemo ? 'SAMPLE DATA' : ''}
>
  {#if !source}
    <Notice icon={House} title="NO BOX CONNECTED">
      Home shows a Castle box at a glance. Connect to one from Storage, or switch to
      <span class="text-bridge">Demo</span> at the top to see it with sample data.
    </Notice>
  {:else}
    <div class="grid gap-3 sm:grid-cols-3">
      {#each gauges as gauge (gauge.id)}
        {@const busy = gauge.percent >= BUSY_PERCENT}
        <div class="glass flex items-center gap-4 p-4">
          <svg viewBox="0 0 120 120" class="h-24 w-24 shrink-0 -rotate-90" role="img" aria-label={`${gauge.label} ${Math.round(gauge.percent)} percent`}>
            <circle cx="60" cy="60" r={RING_RADIUS} fill="none" stroke="var(--color-hairline)" stroke-width="6" />
            <circle
              class="gauge-arc"
              cx="60"
              cy="60"
              r={RING_RADIUS}
              fill="none"
              stroke={busy ? 'var(--color-chiral)' : 'var(--color-bridge)'}
              stroke-width="6"
              stroke-linecap="round"
              stroke-dasharray={RING_LENGTH}
              stroke-dashoffset={RING_LENGTH * (1 - gauge.percent / PERCENT)}
            />
          </svg>
          <div class="min-w-0">
            <p class="micro text-text-faint">{gauge.label}</p>
            <p class="mt-1 text-[34px] font-black leading-none tabular-nums {busy ? 'text-chiral' : 'text-white'}">
              {Math.round(gauge.percent)}<span class="micro ml-1 text-text-faint">%</span>
            </p>
            <p class="micro mt-1.5 truncate text-text-dim">{gauge.detail}</p>
          </div>
        </div>
      {:else}
        <div class="glass p-4 sm:col-span-3">
          <p class="micro text-text-faint">{problem ? `VITAL SIGNS UNAVAILABLE: ${problem}` : 'READING THE BOX'}</p>
        </div>
      {/each}
    </div>

    <ul class="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {#each tiles as tile, i (tile.id)}
        <li in:fly={itemIn(i)}>
          <button
            class="expo t-ui group flex h-full w-full items-center gap-4 border border-hairline bg-white/[0.02] p-4 text-left hover:border-chiral/60 hover:bg-white/[0.04] active:scale-[0.99]"
            onclick={() => shell.setSection(tile.id)}
          >
            <tile.icon class="h-7 w-7 shrink-0 text-text-faint transition-colors group-hover:text-chiral" strokeWidth={1.25} />
            <span class="min-w-0 flex-1">
              <span class="block text-[15px] font-bold text-white">{tile.label}</span>
              <span class="mt-0.5 block truncate text-[13px] text-text-dim">{statuses[tile.id] || tile.tagline}</span>
            </span>
            <ArrowRight class="expo t-ui h-4 w-4 shrink-0 text-text-faint transition-transform group-hover:translate-x-1 group-hover:text-chiral" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</SectionFrame>

<style>
  /* The arc glides to each new reading instead of jumping. */
  .gauge-arc {
    transition:
      stroke-dashoffset 900ms var(--ease-expo),
      stroke 250ms var(--ease-expo);
  }
</style>
