<!-- HomeSection: the box at a glance. A running uptime clock, three live gauges for the
     machine it runs on, and a tile for every other section showing its state right now. A
     tile opens its section. -->
<script>
  import { untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import { House, ArrowRight } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { SECTIONS } from '../../lib/sections.js';
  import { fmtSize } from '../../lib/format.js';
  import { itemIn, reducedMotion } from '../../lib/motion.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';

  const HOME = 'home';
  const VITALS_MS = 2000;
  const STATUS_MS = 15_000;
  const PERCENT = 100;
  const RING_RADIUS = 52;
  const RING_LENGTH = 2 * Math.PI * RING_RADIUS;
  const BUSY_PERCENT = 85; // a gauge turns orange from here up
  const SECOND_MS = 1000;
  const MINUTE_MS = 60 * SECOND_MS;
  const HOUR_MS = 60 * MINUTE_MS;
  const DAY_MS = 24 * HOUR_MS;
  // The clock runs on this device between readings. A reading that disagrees with it by
  // more than this (the box slept, or restarted) resets it; smaller differences are only
  // the time the reading took to arrive, and following them would make the digits jitter.
  const CLOCK_DRIFT_MS = 750;
  const STILL_TICK_MS = 1000; // with motion reduced the clock steps once a second

  let vitals = $state(null);
  let problem = $state('');
  let statuses = $state({}); // section id -> one line about its state now
  let upMs = $state(null); // how long the box has been up, right now, in milliseconds
  let clock = null; // { base, at }: the uptime at one reading, and when this device took it

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

  const two = (n) => String(n).padStart(2, '0');
  const uptimeOf = (reading) => reading.uptimeMillis ?? reading.uptimeSeconds * SECOND_MS;

  /** The clock's digits: days apart, then hours, minutes, seconds and thousandths. */
  const up = $derived.by(() => {
    if (upMs == null) return null;
    const ms = Math.max(0, Math.floor(upMs));
    return {
      days: Math.floor(ms / DAY_MS),
      hours: two(Math.floor((ms % DAY_MS) / HOUR_MS)),
      minutes: two(Math.floor((ms % HOUR_MS) / MINUTE_MS)),
      seconds: two(Math.floor((ms % MINUTE_MS) / SECOND_MS)),
      millis: String(ms % SECOND_MS).padStart(3, '0'),
    };
  });
  /** The same in words, for a screen reader. It changes each reading, not each frame. */
  const upSpoken = $derived.by(() => {
    if (!vitals) return '';
    const ms = uptimeOf(vitals);
    const days = Math.floor(ms / DAY_MS);
    const hours = Math.floor((ms % DAY_MS) / HOUR_MS);
    const minutes = Math.floor((ms % HOUR_MS) / MINUTE_MS);
    return `Up ${days ? `${days} days, ` : ''}${hours} hours, ${minutes} minutes`;
  });
  const since = $derived(vitals ? new Date(vitals.at - uptimeOf(vitals)).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }) : '');

  async function readVitals(from) {
    try {
      const reading = await from.system();
      const base = uptimeOf(reading);
      const now = performance.now();
      if (!clock || Math.abs(clock.base + (now - clock.at) - base) > CLOCK_DRIFT_MS) clock = { base, at: now };
      vitals = reading;
      problem = '';
    } catch (err) {
      problem = err?.message || 'The box did not answer';
    }
  }

  /** Runs the uptime clock: every frame, or once a second when motion is reduced. */
  function runClock() {
    const tick = () => {
      if (clock) upMs = clock.base + (performance.now() - clock.at);
    };
    if (reducedMotion()) {
      const timer = setInterval(tick, STILL_TICK_MS);
      return () => clearInterval(timer);
    }
    let frame = requestAnimationFrame(function loop() {
      tick();
      frame = requestAnimationFrame(loop);
    });
    return () => cancelAnimationFrame(frame);
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
      clock = null;
      upMs = null;
      readVitals(from);
      readStatuses();
    });
    const fast = setInterval(() => readVitals(from), VITALS_MS);
    const slow = setInterval(readStatuses, STATUS_MS);
    const stopClock = runClock();
    return () => {
      clearInterval(fast);
      clearInterval(slow);
      stopClock();
    };
  });
</script>

<SectionFrame
  title="Home"
  subtitle={vitals ? `${vitals.host || 'This box'} · ${vitals.os}` : 'The box at a glance'}
  tag={store.isDemo ? 'SAMPLE DATA' : ''}
>
  {#if !source}
    <Notice icon={House} title="NO BOX CONNECTED">
      Home shows a Castle box at a glance. Connect to one from Storage, or switch to
      <span class="text-bridge">Demo</span> at the top to see it with sample data.
    </Notice>
  {:else}
    {#if up}
      <div class="uptime hud-corners glass mb-3 flex flex-wrap items-end justify-between gap-x-6 gap-y-2 px-5 py-4">
        <div class="min-w-0">
          <p class="micro flex items-center gap-2 text-text-faint"><span class="pulse"></span> UPTIME</p>
          <!-- The digits change every frame, so screen readers get the line in words instead. -->
          <p class="clock mt-1" aria-hidden="true">
            {#if up.days}<span class="days">{up.days}<span class="unit">D</span></span>{/if}
            <span>{up.hours}</span><span class="colon">:</span><span>{up.minutes}</span><span class="colon">:</span><span>{up.seconds}</span><span class="millis">.{up.millis}</span>
          </p>
          <p class="sr-only" role="timer">{upSpoken}</p>
        </div>
        <p class="micro pb-1 text-right text-text-faint">
          <span class="block text-text-dim">{vitals?.host || 'THIS BOX'}</span>
          <span class="block">RUNNING SINCE {since.toUpperCase()}</span>
        </p>
      </div>
    {/if}

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
  /* The uptime clock: fixed-width digits so nothing shifts as they run. Hours, minutes and
     seconds are the reading; the thousandths are the motion, smaller and in the accent. */
  .clock {
    display: flex;
    align-items: baseline;
    font-family: ui-monospace, 'Cascadia Mono', 'SF Mono', Menlo, Consolas, monospace;
    font-variant-numeric: tabular-nums;
    font-size: clamp(34px, 7.2vw, 68px);
    font-weight: 800;
    line-height: 1;
    letter-spacing: 0.02em;
    color: #fff;
    text-shadow: 0 0 28px color-mix(in srgb, var(--color-bridge) 30%, transparent);
  }
  .clock .days {
    margin-right: 0.35em;
    color: var(--color-bridge);
  }
  .clock .unit {
    margin-left: 0.08em;
    font-size: 0.42em;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--color-text-faint);
  }
  .clock .colon {
    padding: 0 0.04em;
    color: var(--color-text-faint);
    animation: colon-beat 1s steps(2, jump-none) infinite;
  }
  .clock .millis {
    margin-left: 0.06em;
    font-size: 0.5em;
    font-weight: 700;
    color: var(--color-chiral);
    text-shadow: 0 0 18px color-mix(in srgb, var(--color-chiral) 55%, transparent);
  }
  @keyframes colon-beat { 50% { opacity: 0.35; } }
  .pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-live);
    box-shadow: 0 0 10px var(--color-live);
  }
  .uptime {
    background-image: linear-gradient(90deg, color-mix(in srgb, var(--color-bridge) 7%, transparent), transparent 55%);
  }
  :global([data-motion='reduced']) .clock .colon { animation: none; }
  @media (prefers-reduced-motion: reduce) {
    .clock .colon { animation: none; }
  }

  /* The arc glides to each new reading instead of jumping. */
  .gauge-arc {
    transition:
      stroke-dashoffset 900ms var(--ease-expo),
      stroke 250ms var(--ease-expo);
  }
</style>
