<!-- NetworkSection: the box's view of its connection. Live traffic per adapter, a speed
     test, who the provider is, and what answers DNS. A box that is not the router sees
     only its own traffic; per-device traffic and ad blocking arrive when Castle is. -->
<script>
  import { untrack } from 'svelte';
  import { Activity, Gauge, Globe, ShieldBan, MonitorSmartphone, ArrowDown, ArrowUp, Play, Search, Tv, Laptop, Smartphone } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { prefs } from '../../lib/prefs.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import { DEMO_DEVICES, DEMO_DNS } from '../../lib/demo-shell.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';

  const VIEWS = [
    { id: 'live', label: 'Live', icon: Activity },
    { id: 'speed', label: 'Speed test', icon: Gauge },
    { id: 'provider', label: 'Provider', icon: Globe },
    { id: 'dns', label: 'DNS and ad block', icon: ShieldBan },
    { id: 'devices', label: 'Devices', icon: MonitorSmartphone },
  ];
  const SECOND_MS = 1000;
  const HISTORY_POINTS = 60; // samples kept for the live chart
  const CHART_W = 600;
  const CHART_H = 140;
  const BITS = 8;
  const MEGA = 1_000_000;
  const SPEED_HISTORY_KEY = 'castle-speed-history';
  const SPEED_HISTORY_MAX = 20;
  const DEVICE_ICONS = { tv: Tv, laptop: Laptop, phone: Smartphone };
  const PERCENT = 100;

  let view = $state('live');
  let reading = $state(null); // newest counters from the box
  let rates = $state([]); // per adapter: { name, down, up, rxBytes, txBytes } in bytes per second
  let history = $state([]); // [{ down, up }] for the busiest adapter, oldest first
  let problem = $state('');
  let testing = $state(false);
  let results = $state(readSpeedHistory());
  let provider = $state(null);
  let lookingUp = $state(false);
  let previous = null; // the reading before `reading`, to turn counters into rates

  const source = $derived(shell.networkSource);
  const main = $derived(rates[0] || null);
  const peak = $derived(Math.max(1, ...history.map((p) => Math.max(p.down, p.up))));
  const latest = $derived(results[0] || null);

  function readSpeedHistory() {
    try {
      return JSON.parse(localStorage.getItem(SPEED_HISTORY_KEY)) || [];
    } catch {
      return [];
    }
  }

  /** Bytes per second as megabits per second, the unit providers sell in. */
  const mbps = (bytesPerSecond) => (bytesPerSecond * BITS) / MEGA;
  const fmtMbps = (bytesPerSecond) => {
    const value = mbps(bytesPerSecond);
    return value >= 100 ? value.toFixed(0) : value >= 10 ? value.toFixed(1) : value.toFixed(2);
  };

  /** An SVG path through the history for one direction, scaled to the busiest moment. */
  function chartPath(key, closed) {
    if (history.length < 2) return '';
    const step = CHART_W / (HISTORY_POINTS - 1);
    const offset = (HISTORY_POINTS - history.length) * step;
    const points = history.map((p, i) => `${(offset + i * step).toFixed(1)},${(CHART_H - (p[key] / peak) * (CHART_H - 6)).toFixed(1)}`);
    const line = `M${points.join(' L')}`;
    return closed ? `${line} L${CHART_W},${CHART_H} L${offset.toFixed(1)},${CHART_H} Z` : line;
  }

  async function poll(from) {
    if (!from) return;
    try {
      const next = await from.network();
      if (previous && next.at > previous.at) {
        const seconds = (next.at - previous.at) / SECOND_MS;
        const before = new Map(previous.interfaces.map((i) => [i.name, i]));
        rates = next.interfaces
          .map((i) => {
            const was = before.get(i.name);
            // A counter that went backwards means the adapter was reset: show nothing, not a spike.
            const down = was && i.rxBytes >= was.rxBytes ? (i.rxBytes - was.rxBytes) / seconds : 0;
            const up = was && i.txBytes >= was.txBytes ? (i.txBytes - was.txBytes) / seconds : 0;
            return { ...i, down, up };
          })
          .sort((a, b) => b.rxBytes + b.txBytes - (a.rxBytes + a.txBytes));
        if (rates[0]) history = [...history, { down: rates[0].down, up: rates[0].up }].slice(-HISTORY_POINTS);
      }
      previous = next;
      reading = next;
      problem = '';
    } catch (err) {
      problem = err?.message || 'The network could not be read';
    }
  }

  async function runSpeedTest() {
    testing = true;
    try {
      const result = await source.networkSpeedTest();
      results = [result, ...results].slice(0, SPEED_HISTORY_MAX);
      if (!source.isDemo) localStorage.setItem(SPEED_HISTORY_KEY, JSON.stringify(results));
    } catch (err) {
      store.toast(err?.message || 'The speed test failed');
    }
    testing = false;
  }

  async function lookUpProvider() {
    lookingUp = true;
    try {
      provider = await source.networkProvider();
    } catch (err) {
      store.toast(err?.message || 'The lookup failed');
    }
    lookingUp = false;
  }

  const when = (ms) => new Date(ms).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });

  // Poll on the interval chosen in Settings > Network, and start over when the source changes.
  $effect(() => {
    const from = source;
    const every = prefs.get('network.livePollSeconds') * SECOND_MS;
    untrack(() => {
      previous = null;
      rates = [];
      history = [];
      provider = null;
      if (from?.isDemo) results = [];
      poll(from);
    });
    const timer = setInterval(() => poll(from), every);
    return () => clearInterval(timer);
  });
</script>

{#snippet stat(label, value, unit, tone)}
  <div class="glass p-4">
    <p class="micro text-text-faint">{label}</p>
    <p class="mt-2 flex items-baseline gap-1.5">
      <span class="text-[34px] font-black leading-none tabular-nums {tone}">{value}</span>
      <span class="micro text-text-faint">{unit}</span>
    </p>
  </div>
{/snippet}

<SectionFrame
  title="Network"
  subtitle={reading?.local?.adapter ? `${reading.local.adapter} · ${reading.local.address || 'no address'}` : 'This box\'s connection'}
  tag={store.isDemo ? 'SAMPLE DATA' : ''}
  items={VIEWS}
  active={view}
  onselect={(id) => (view = id)}
>
  {#if !source}
    <Notice icon={Globe} title="NO BOX CONNECTED">
      Network shows a Castle box's connection. Connect to one from Storage, or open the
      <a class="text-bridge underline" href="/demo">demo</a> to see it with sample traffic.
    </Notice>
  {:else if problem && !reading}
    <Notice icon={Globe} title="NETWORK UNAVAILABLE">{problem}</Notice>
  {:else if view === 'live'}
    <div class="grid gap-3 sm:grid-cols-2">
      {@render stat('DOWN NOW', main ? fmtMbps(main.down) : '0.00', 'MBPS', 'text-bridge')}
      {@render stat('UP NOW', main ? fmtMbps(main.up) : '0.00', 'MBPS', 'text-chiral')}
    </div>

    <div class="glass mt-3 p-4">
      <div class="micro mb-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-text-faint">
        <span>{main?.name || 'WAITING FOR A SECOND READING'}</span>
        <span class="flex items-center gap-1.5 text-bridge"><ArrowDown class="h-3 w-3" /> DOWN</span>
        <span class="flex items-center gap-1.5 text-chiral"><ArrowUp class="h-3 w-3" /> UP</span>
        <span class="ml-auto">PEAK {fmtMbps(peak)} MBPS</span>
      </div>
      <svg viewBox="0 0 {CHART_W} {CHART_H}" preserveAspectRatio="none" class="h-36 w-full" role="img" aria-label="Traffic over the last minute or so">
        <path d={chartPath('down', true)} fill="var(--color-bridge-dim)" />
        <path d={chartPath('down', false)} fill="none" stroke="var(--color-bridge)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
        <path d={chartPath('up', false)} fill="none" stroke="var(--color-chiral)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
      </svg>
    </div>

    <div class="mt-3 grid gap-3 lg:grid-cols-[minmax(0,1fr)_18rem]">
      <div class="glass overflow-x-auto p-4">
        <p class="micro mb-3 text-text-faint">ADAPTERS</p>
        <table class="w-full text-left text-[13px]">
          <thead class="micro text-text-faint">
            <tr><th class="pb-2 font-normal">NAME</th><th class="pb-2 text-right font-normal">DOWN</th><th class="pb-2 text-right font-normal">UP</th><th class="pb-2 text-right font-normal">RECEIVED</th><th class="pb-2 text-right font-normal">SENT</th></tr>
          </thead>
          <tbody>
            {#each rates.length ? rates : reading?.interfaces || [] as adapter (adapter.name)}
              <tr class="border-t border-hairline-faint">
                <td class="max-w-[14rem] truncate py-2 text-white">{adapter.name}</td>
                <td class="py-2 text-right tabular-nums text-bridge">{fmtMbps(adapter.down || 0)}</td>
                <td class="py-2 text-right tabular-nums text-chiral">{fmtMbps(adapter.up || 0)}</td>
                <td class="py-2 text-right tabular-nums text-text-dim">{fmtSize(adapter.rxBytes)}</td>
                <td class="py-2 text-right tabular-nums text-text-dim">{fmtSize(adapter.txBytes)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="micro mt-3 text-text-faint">RATES IN MBPS · TOTALS SINCE THE MACHINE STARTED</p>
      </div>
      <dl class="glass micro space-y-3 p-4 text-text-faint">
        <div><dt>THIS BOX</dt><dd class="mt-0.5 text-[13px] normal-case tracking-normal text-white">{reading?.local?.address || 'unknown'}</dd></div>
        <div><dt>ROUTER</dt><dd class="mt-0.5 text-[13px] normal-case tracking-normal text-white">{reading?.local?.gateway || 'unknown'}</dd></div>
        <div><dt>DNS</dt><dd class="mt-0.5 text-[13px] normal-case tracking-normal text-white">{reading?.local?.dns?.join(', ') || 'unknown'}</dd></div>
      </dl>
    </div>
    <p class="mt-4 text-[12px] leading-relaxed text-text-dim">
      This is the traffic of the machine the box runs on. Seeing every device on the network needs Castle to be the router.
    </p>
  {:else if view === 'speed'}
    <div class="grid gap-3 sm:grid-cols-3">
      {@render stat('DOWNLOAD', latest ? latest.downMbps.toFixed(1) : '—', 'MBPS', 'text-bridge')}
      {@render stat('UPLOAD', latest ? latest.upMbps.toFixed(1) : '—', 'MBPS', 'text-chiral')}
      {@render stat('RESPONSE', latest ? latest.latencyMs.toFixed(0) : '—', 'MS', 'text-white')}
    </div>
    <button
      disabled={testing}
      onclick={runSpeedTest}
      class="expo t-hover hud-corners micro mt-4 flex h-12 w-full items-center justify-center gap-2 bg-chiral font-bold text-black transition-all hover:brightness-110 active:scale-[0.99] disabled:opacity-50 sm:w-72"
    >
      <Play class="h-4 w-4 fill-black" strokeWidth={2} /> {testing ? 'TESTING. ABOUT 20 SECONDS' : 'RUN A SPEED TEST'}
    </button>
    <p class="mt-3 max-w-2xl text-[12px] leading-relaxed text-text-dim">
      The box downloads 50 MB and uploads 10 MB to Cloudflare's speed-test service over one connection and times each. On a very fast
      line this reads lower than a multi-connection test. Response is one request and answer, so it is a little longer than a ping.
    </p>
    {#if results.length}
      <div class="glass mt-5 overflow-x-auto p-4">
        <p class="micro mb-3 text-text-faint">EARLIER TESTS ON THIS DEVICE</p>
        <table class="w-full text-left text-[13px]">
          <thead class="micro text-text-faint">
            <tr><th class="pb-2 font-normal">WHEN</th><th class="pb-2 text-right font-normal">DOWN</th><th class="pb-2 text-right font-normal">UP</th><th class="pb-2 text-right font-normal">RESPONSE</th></tr>
          </thead>
          <tbody>
            {#each results as result (result.at)}
              <tr class="border-t border-hairline-faint">
                <td class="py-2 text-white">{when(result.at)}</td>
                <td class="py-2 text-right tabular-nums text-bridge">{result.downMbps.toFixed(1)}</td>
                <td class="py-2 text-right tabular-nums text-chiral">{result.upMbps.toFixed(1)}</td>
                <td class="py-2 text-right tabular-nums text-text-dim">{result.latencyMs.toFixed(0)} ms</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {:else if view === 'provider'}
    {#if provider}
      <div class="grid gap-3 sm:grid-cols-2">
        <div class="glass p-4 sm:col-span-2">
          <p class="micro text-text-faint">INTERNET PROVIDER</p>
          <p class="mt-2 text-[28px] font-black leading-tight text-white">{provider.provider || 'Not named by the lookup'}</p>
          {#if provider.asn}<p class="micro mt-1 text-text-faint">NETWORK {provider.asn}</p>{/if}
        </div>
        <div class="glass p-4"><p class="micro text-text-faint">PUBLIC ADDRESS</p><p class="mt-2 text-[18px] font-semibold tabular-nums text-bridge">{provider.address}</p></div>
        <div class="glass p-4"><p class="micro text-text-faint">SEEN FROM</p><p class="mt-2 text-[18px] font-semibold text-white">{[provider.city, provider.region, provider.country].filter(Boolean).join(', ') || 'unknown'}</p></div>
      </div>
      <p class="mt-4 max-w-2xl text-[12px] leading-relaxed text-text-dim">
        This is how the outside world sees your connection. The place is where the provider's equipment is registered, which is often not your town.
      </p>
    {:else}
      <Notice icon={Globe} title="WHO IS YOUR PROVIDER">
        <p>The box can ask ipinfo.io how your connection looks from outside: the provider's name, its network number and your public address. That service sees your public address when asked.</p>
        <button
          disabled={lookingUp}
          onclick={lookUpProvider}
          class="expo t-hover micro mx-auto mt-4 flex h-10 items-center gap-2 bg-chiral px-4 font-bold text-black transition-all hover:brightness-110 disabled:opacity-50"
        >
          <Search class="h-4 w-4" strokeWidth={2} /> {lookingUp ? 'LOOKING UP' : 'LOOK UP'}
        </button>
      </Notice>
    {/if}
  {:else if view === 'dns'}
    <div class="grid gap-3 sm:grid-cols-2">
      <div class="glass p-4">
        <p class="micro text-text-faint">NAMES ARE LOOKED UP BY</p>
        <p class="mt-2 text-[18px] font-semibold tabular-nums text-white">{reading?.local?.dns?.join(', ') || 'unknown'}</p>
        <p class="mt-2 text-[12px] leading-relaxed text-text-dim">
          {reading?.local?.dns?.[0] && reading.local.dns[0] === reading.local.gateway ? 'That is your router, which passes questions on to your provider.' : 'These are the DNS servers this machine is set to use.'}
        </p>
      </div>
      <div class="glass p-4">
        <p class="micro text-text-faint">AD AND TRACKER BLOCKING</p>
        {#if store.isDemo}
          <p class="mt-2 text-[18px] font-semibold text-live">{Math.round((DEMO_DNS.blockedToday / DEMO_DNS.queriesToday) * PERCENT)}% blocked today</p>
          <p class="micro mt-1 text-text-faint">{DEMO_DNS.blockedToday.toLocaleString()} OF {DEMO_DNS.queriesToday.toLocaleString()} LOOKUPS</p>
        {:else}
          <p class="mt-2 text-[18px] font-semibold text-text-dim">Not running</p>
        {/if}
      </div>
    </div>
    {#if store.isDemo}
      <div class="glass mt-3 p-4">
        <p class="micro mb-2 text-text-faint">MOST BLOCKED</p>
        <ul class="space-y-1.5 text-[13px] text-white">
          {#each DEMO_DNS.topBlocked as domain (domain)}<li class="truncate">{domain}</li>{/each}
        </ul>
      </div>
    {/if}
    <p class="mt-4 max-w-2xl text-[12px] leading-relaxed text-text-dim">
      Blocking works by having Castle answer DNS for the whole house, the way Pi-hole does. That resolver is not built into the box
      yet. The switches for it are in Settings, under Network, and are saved for when it is.
    </p>
  {:else if store.isDemo}
    <ul class="grid gap-3 lg:grid-cols-2">
      {#each DEMO_DEVICES as device (device.id)}
        {@const DeviceIcon = DEVICE_ICONS[device.kind] || MonitorSmartphone}
        <li class="glass flex items-center gap-4 p-4 {device.online ? '' : 'opacity-60'}">
          <DeviceIcon class="h-7 w-7 shrink-0 {device.online ? 'text-live' : 'text-text-faint'}" strokeWidth={1.25} />
          <div class="min-w-0 flex-1">
            <p class="truncate text-[15px] font-semibold text-white">{device.name}</p>
            <p class="micro mt-0.5 text-text-faint">{device.address} · {device.online ? 'ONLINE' : 'AWAY'}</p>
          </div>
          <p class="micro text-right text-text-faint">
            <span class="block text-bridge">↓ {fmtSize(device.downBytes)}</span>
            <span class="block text-chiral">↑ {fmtSize(device.upBytes)}</span>
          </p>
        </li>
      {/each}
    </ul>
    <p class="mt-4 text-[12px] text-text-dim">Sample devices, with what each has used today.</p>
  {:else}
    <Notice icon={MonitorSmartphone} title="DEVICES NEED CASTLE TO BE THE ROUTER">
      A box plugged into your router sees only its own traffic. Once Castle sits between the modem and the house, every device and
      what it uses appears here. The <a class="text-bridge underline" href="/demo">demo</a> shows the planned view.
    </Notice>
  {/if}
</SectionFrame>
