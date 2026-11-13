<!-- SteamCache: Steam games kept on the box, so a game downloads from the internet once and
     every PC in the home gets it from the box. Shows whether the cache is on and whether
     Steam is finding it, who is downloading now, the games in it (with what is kept, what
     Steam has now, and when it last changed), and ways to add one: from the games on this
     computer or by searching the Steam store. A game can be purged; until then it stays,
     ready for the next PC that installs it. -->
<script>
  import { Download, Power, Network, Copy, Check, Search, Trash2, X, Pin, PinOff, RefreshCw, LoaderCircle, ExternalLink, Monitor, CircleCheck, CircleAlert } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { ask } from '../../lib/ask.svelte.js';
  import { SvelteMap } from 'svelte/reactivity';
  import { store } from '../../lib/store.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import { fmtStamp } from '../../lib/computer.js';
  import { itemIn, reducedMotion } from '../../lib/motion.js';

  /** source: the box (or the demo) to read the cache from. */
  let { source } = $props();

  const POLL_MS = 4000;
  const STATE = {
    cached: { label: 'KEPT', tone: 'text-live border-live/50' },
    partial: { label: 'PARTLY KEPT', tone: 'text-bridge border-bridge/50' },
    update: { label: 'UPDATE WAITING', tone: 'text-chiral border-chiral/50' },
    empty: { label: 'NOT DOWNLOADED YET', tone: 'text-text-dim border-hairline' },
  };

  let view = $state(null);
  let problem = $state('');
  let open = $state(null); // the game whose details are shown
  let local = $state(null); // games installed on this computer, when asked for
  let query = $state('');
  let found = $state(null);
  let searching = $state(false);
  let busy = $state(''); // what is being done, for the buttons
  let copied = $state(false);
  const art = new SvelteMap();

  const hitRate = $derived(view && view.totals.hits + view.totals.misses ? Math.round((view.totals.hits / (view.totals.hits + view.totals.misses)) * 100) : 0);
  const hostsLine = $derived(view?.boxAddress ? `${view.boxAddress}  lancache.steamcontent.com` : '');
  const apps = $derived(view ? [...view.apps].sort((a, b) => (b.lastServed || 0) - (a.lastServed || 0) || a.name.localeCompare(b.name)) : []);

  async function load(from = source) {
    try {
      const next = await from.steamCache();
      if (from !== source) return;
      view = next;
      problem = '';
      if (open) open = next.apps.find((a) => a.app === open.app) || null;
      for (const app of next.apps) if (app.header && !art.get(app.app)) from.steamArt(app.app).then((url) => url && art.set(app.app, url));
    } catch (err) {
      problem = err?.message || 'The Steam cache could not be read';
    }
  }

  $effect(() => {
    const from = source;
    untrack(() => {
      view = null;
      local = null;
      found = null;
      art.clear();
      load(from);
    });
    const timer = setInterval(() => load(from), POLL_MS);
    return () => clearInterval(timer);
  });

  async function act(what, work, done) {
    busy = what;
    try {
      const result = await work();
      if (done) store.toast(typeof done === 'function' ? done(result) : done);
      await load();
    } catch (err) {
      store.toast(err?.message || 'That did not work');
    } finally {
      busy = '';
    }
  }

  const turnOn = () => act('on', () => source.steamEnable(), 'The Steam cache is on');
  const turnOnDns = () => act('dns', () => source.steamDnsEnable(), 'Castle answers DNS now. Point your router at it.');
  const keep = (app, on = true) => act(`keep:${app}`, () => source.steamKeep(app, on), on ? 'Kept. Its details are on the way.' : 'No longer kept');
  const refresh = () => act('refresh', () => source.steamRefresh(), 'Looking up game details');

  async function purge(app) {
    const question = `Purge ${app.name}? Its ${fmtSize(app.cached)} in the cache are deleted. The next PC to install it downloads it from the internet again.`;
    if (!(await ask(question, { yes: 'Purge', danger: true }))) return;
    act(`purge:${app.app}`, () => source.steamPurge(app.app), (r) => `${fmtSize(r.freed)} freed`).then(() => (open = null));
  }

  async function showLocal() {
    try {
      local = await source.steamLocal();
    } catch (err) {
      store.toast(err?.message || 'The games on this computer could not be read');
    }
  }

  async function search() {
    if (!query.trim()) return;
    searching = true;
    try {
      found = await source.steamSearch(query);
    } catch (err) {
      store.toast(err?.message || 'The Steam store did not answer');
    } finally {
      searching = false;
    }
  }

  async function copyHosts() {
    try {
      await navigator.clipboard.writeText(hostsLine);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      store.toast('Copy it by hand: the clipboard is blocked here');
    }
  }

  const kept = (app) => view?.apps.some((a) => a.app === app && a.kept);
</script>

<div class="px-4 py-5 md:px-6">
  {#if problem && !view}
    <p class="text-[13px] text-text-dim">{problem}</p>
  {:else if !view}
    <div class="grid min-h-[30dvh] place-items-center"><LoaderCircle class="h-6 w-6 animate-spin text-text-faint" /></div>
  {:else}
    <!-- the cache, and whether Steam is finding it -->
    <section class="grid gap-3 lg:grid-cols-[1.1fr_1fr]">
      <div class="hud-corners glass px-5 py-4">
        <p class="micro text-chiral">STEAM CACHE</p>
        <div class="mt-2 flex flex-wrap items-baseline gap-x-6 gap-y-1">
          <p><span class="font-mono text-[28px] text-white tabular-nums">{fmtSize(view.totals.bytes)}</span> <span class="micro text-text-faint">KEPT</span></p>
          <p><span class="font-mono text-[20px] text-live tabular-nums">{fmtSize(view.totals.saved)}</span> <span class="micro text-text-faint">NOT DOWNLOADED AGAIN</span></p>
          <p><span class="font-mono text-[20px] text-bridge tabular-nums">{hitRate}%</span> <span class="micro text-text-faint">FROM THE CACHE</span></p>
        </div>
        <p class="micro mt-2 truncate normal-case text-text-faint">{view.cacheDir} · {fmtSize(view.totals.free)} free</p>
        <div class="mt-3 flex items-center gap-2 border-t border-hairline-faint pt-3">
          {#if view.listening}
            <CircleCheck class="h-4 w-4 shrink-0 text-live" />
            <p class="text-[13px] text-text-dim">Listening for Steam on <span class="font-mono text-white">{view.listening}</span></p>
          {:else}
            <CircleAlert class="h-4 w-4 shrink-0 text-chiral" />
            <p class="flex-1 text-[13px] text-text-dim">The cache is off.</p>
            {#if view.canTurnOn}
              <button class="micro expo t-hover flex items-center gap-1.5 bg-chiral px-3 py-2 font-bold text-black hover:opacity-90 disabled:opacity-50" onclick={turnOn} disabled={busy === 'on'}>
                <Power class="h-3.5 w-3.5" />{busy === 'on' ? 'TURNING ON…' : 'TURN ON'}
              </button>
            {:else}
              <p class="micro text-text-faint">START THE BOX WITH --steam-cache 0.0.0.0:80</p>
            {/if}
          {/if}
        </div>
      </div>

      <div class="border border-hairline bg-white/[0.02] px-5 py-4">
        <p class="micro flex items-center gap-1.5 text-chiral"><Network class="h-3.5 w-3.5" />HOW STEAM FINDS IT</p>
        {#if view.steamPointsHere}
          <p class="mt-2 flex items-center gap-2 text-[13px] text-white"><CircleCheck class="h-4 w-4 text-live" />Steam on this computer downloads through Castle.</p>
          <p class="micro mt-1 text-text-faint">LANCACHE.STEAMCONTENT.COM → {view.resolved.join(', ')}</p>
        {:else}
          <p class="mt-2 text-[13px] text-text-dim">Steam looks for a cache at <span class="font-mono text-white">lancache.steamcontent.com</span>. Point that name at this box{view.boxAddress ? ` (${view.boxAddress})` : ''} and Steam uses it on its own, with nothing to change in Steam.</p>
          <ol class="mt-3 space-y-2.5 text-[13px] text-text-dim">
            <li>
              <span class="micro text-white">EVERY PC IN THE HOME:</span>
              {#if view.dns}
                Castle answers DNS on <span class="font-mono text-white">{view.dns}</span>. Set your router's DNS server to <span class="font-mono text-white">{view.boxAddress}</span>.
              {:else}
                let Castle answer DNS, then set your router's DNS server to it.
                {#if view.canTurnOn}
                  <button class="micro expo t-hover ml-1 border border-bridge/50 px-2 py-1 text-bridge hover:bg-bridge/10 disabled:opacity-50" onclick={turnOnDns} disabled={busy === 'dns'}>{busy === 'dns' ? 'STARTING…' : 'TURN ON DNS'}</button>
                {/if}
              {/if}
            </li>
            <li>
              <span class="micro text-white">ONE PC:</span> add this line to its hosts file (as administrator):
              {#if hostsLine}
                <span class="mt-1.5 flex items-center gap-2">
                  <code class="micro flex-1 border border-hairline bg-black/50 px-2 py-1.5 normal-case text-white">{hostsLine}</code>
                  <button class="expo t-hover border border-hairline p-1.5 text-text-dim hover:text-white" onclick={copyHosts} aria-label="Copy the hosts line">
                    {#if copied}<Check class="h-3.5 w-3.5 text-live" />{:else}<Copy class="h-3.5 w-3.5" />{/if}
                  </button>
                </span>
              {/if}
            </li>
          </ol>
        {/if}
      </div>
    </section>

    <!-- downloading now -->
    {#if view.downloading.length}
      <section class="mt-5" aria-labelledby="now-heading">
        <h2 id="now-heading" class="micro mb-2 flex items-center gap-1.5 text-chiral"><Download class="h-3.5 w-3.5" />DOWNLOADING NOW</h2>
        <div class="overflow-hidden border border-hairline">
          {#each view.downloading as d (d.client + d.app)}
            <div class="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-hairline-faint px-4 py-2.5 last:border-0">
              <Monitor class="h-4 w-4 text-text-faint" />
              <span class="font-mono text-[13px] text-white">{d.client}</span>
              <span class="min-w-0 flex-1 truncate text-[13px] text-text-dim">{d.name}</span>
              <span class="micro text-text-faint">{fmtSize(d.bytes)}</span>
              <span class="micro text-bridge">{d.mbps} MB/S</span>
              <span class="micro {d.hitPercent > 50 ? 'text-live' : 'text-text-faint'}">{d.hitPercent}% FROM CACHE</span>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- the games -->
    <section class="mt-6" aria-labelledby="games-heading">
      <div class="mb-3 flex items-baseline justify-between">
        <h2 id="games-heading" class="micro text-chiral">GAMES IN THE CACHE</h2>
        <button class="micro expo t-hover flex items-center gap-1 text-text-faint hover:text-white disabled:opacity-50" onclick={refresh} disabled={busy === 'refresh'}>
          <RefreshCw class="h-3 w-3 {busy === 'refresh' ? 'animate-spin' : ''}" />CHECK FOR UPDATES
        </button>
      </div>
      {#if !apps.length}
        <p class="border border-dashed border-hairline px-5 py-6 text-center text-[13px] text-text-faint">Nothing kept yet. Download a game on any PC that uses the cache, or choose one below.</p>
      {:else}
        <ul class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          {#each apps as app, i (app.app)}
            <li in:fly={itemIn(Math.min(i, 16))}>
              <button class="expo t-hover group block w-full border border-hairline bg-white/[0.02] text-left transition-all hover:border-white/25" onclick={() => (open = app)}>
                <div class="relative aspect-[460/215] overflow-hidden bg-black/40">
                  {#if art.get(app.app)}<img src={art.get(app.app)} alt="" class="expo t-ui h-full w-full object-cover transition-transform group-hover:scale-[1.03]" />{/if}
                  <span class="micro absolute left-2 top-2 border bg-black/70 px-1.5 py-0.5 {STATE[app.state].tone}">{STATE[app.state].label}</span>
                  {#if app.kept}<Pin class="absolute right-2 top-2 h-3.5 w-3.5 text-chiral" />{/if}
                </div>
                <div class="px-3 py-2.5">
                  <p class="truncate text-[13px] font-semibold text-white">{app.name}</p>
                  <p class="micro mt-1 text-text-faint">{fmtSize(app.cached)}{app.full ? ` OF ${fmtSize(app.full)}` : ''}{app.updated ? ` · UPDATED ${fmtStamp(app.updated).split(',')[0].toUpperCase()}` : ''}</p>
                  {#if app.full}
                    <div class="mt-1.5 h-[2px] overflow-hidden bg-white/[0.07]"><div class="h-full bg-chiral" style="width: {Math.min(100, (app.cached / app.full) * 100)}%"></div></div>
                  {/if}
                </div>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <!-- adding a game -->
    <section class="mt-6 grid gap-3 lg:grid-cols-2">
      <div class="border border-hairline bg-white/[0.02] px-5 py-4">
        <p class="micro text-chiral">ADD A GAME FROM THE STEAM STORE</p>
        <form class="mt-2 flex gap-2" onsubmit={(e) => { e.preventDefault(); search(); }}>
          <input bind:value={query} placeholder="SEARCH STEAM" class="micro h-9 min-w-0 flex-1 border border-hairline bg-white/[0.03] px-3 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search the Steam store" />
          <button class="micro expo t-hover flex h-9 items-center gap-1.5 border border-hairline px-3 text-text-dim hover:text-white" type="submit">
            {#if searching}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />{:else}<Search class="h-3.5 w-3.5" />{/if}SEARCH
          </button>
        </form>
        {#if found}
          <ul class="mt-3 max-h-72 space-y-1 overflow-y-auto">
            {#each found as f (f.app)}
              <li class="flex items-center gap-3 px-1 py-1">
                <span class="min-w-0 flex-1 truncate text-[13px] text-white">{f.name}</span>
                {#if kept(f.app)}
                  <span class="micro text-live">KEPT</span>
                {:else}
                  <button class="micro expo t-hover border border-chiral/50 px-2 py-1 text-chiral hover:bg-chiral/10" onclick={() => keep(f.app)} disabled={busy === `keep:${f.app}`}>KEEP</button>
                {/if}
              </li>
            {:else}
              <li class="micro text-text-faint">NOTHING FOUND</li>
            {/each}
          </ul>
        {/if}
        <p class="micro mt-3 leading-relaxed text-text-faint">A KEPT GAME IS DOWNLOADED INTO THE CACHE THE FIRST TIME ANY PC IN THE HOME INSTALLS IT THROUGH CASTLE, AND ITS UPDATES WITH THE FIRST PC THAT UPDATES.</p>
      </div>
      <div class="border border-hairline bg-white/[0.02] px-5 py-4">
        <div class="flex items-baseline justify-between">
          <p class="micro text-chiral">INSTALLED ON THIS COMPUTER</p>
          {#if !local}<button class="micro expo t-hover text-text-faint hover:text-white" onclick={showLocal}>SHOW {view.installedHere}</button>{/if}
        </div>
        {#if local}
          <ul class="mt-2 max-h-72 space-y-1 overflow-y-auto">
            {#each local as g (g.app)}
              <li class="flex items-center gap-3 px-1 py-1">
                <span class="min-w-0 flex-1 truncate text-[13px] text-white">{g.name}</span>
                <span class="micro hidden text-text-faint sm:inline">{fmtSize(g.size)}</span>
                {#if kept(g.app)}
                  <button class="micro expo t-hover text-live hover:text-white" onclick={() => keep(g.app, false)} title="Stop keeping">KEPT</button>
                {:else}
                  <button class="micro expo t-hover border border-chiral/50 px-2 py-1 text-chiral hover:bg-chiral/10" onclick={() => keep(g.app)} disabled={busy === `keep:${g.app}`}>KEEP</button>
                {/if}
              </li>
            {:else}
              <li class="micro text-text-faint">NO STEAM GAMES ON THIS COMPUTER</li>
            {/each}
          </ul>
        {:else}
          <p class="mt-2 text-[13px] text-text-faint">{view.installedHere ? `${view.installedHere} Steam games are installed where the box runs.` : 'Steam is not installed where the box runs.'}</p>
        {/if}
      </div>
    </section>
  {/if}
</div>

<!-- a game's details -->
{#if open}
  {@const app = open}
  <div class="fixed inset-0 z-50 grid place-items-center p-3 md:p-6" role="dialog" aria-modal="true" aria-label={app.name}>
    <div class="absolute inset-0 bg-black/75" transition:fade={{ duration: reducedMotion() ? 0 : 160 }} onclick={() => (open = null)} onkeydown={(e) => { if (e.key === 'Escape') open = null; }} role="presentation"></div>
    <div class="hud-corners glass relative z-10 max-h-[90dvh] w-full max-w-2xl overflow-y-auto" transition:fly={{ y: 16, duration: reducedMotion() ? 0 : 200 }}>
      <div class="relative aspect-[460/215] bg-black/40">
        {#if art.get(app.app)}<img src={art.get(app.app)} alt="" class="h-full w-full object-cover" />{/if}
        <button class="absolute right-2 top-2 bg-black/60 p-1.5 text-white hover:bg-black/80" onclick={() => (open = null)} aria-label="Close"><X class="h-5 w-5" /></button>
      </div>
      <div class="px-5 py-4">
        <div class="flex flex-wrap items-center gap-2">
          <h2 class="text-[20px] font-black text-white">{app.name}</h2>
          <span class="micro border px-1.5 py-0.5 {STATE[app.state].tone}">{STATE[app.state].label}</span>
        </div>
        <p class="micro mt-1 text-text-faint">{[app.developers.join(', '), app.released, ...app.genres.slice(0, 3)].filter(Boolean).join(' · ').toUpperCase()}</p>
        {#if app.description}<p class="mt-3 text-[14px] leading-relaxed text-text-dim">{app.description}</p>{/if}
        <dl class="micro mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-text-faint">
          <dt>IN THE CACHE</dt><dd class="text-white">{fmtSize(app.cached)}{app.full ? ` of ${fmtSize(app.full)}` : ''}</dd>
          <dt>SAVED</dt><dd class="text-text-dim">{fmtSize(app.saved)} sent to PCs without the internet</dd>
          {#if app.buildid}<dt>STEAM BUILD</dt><dd class="text-text-dim">{app.buildid}{app.updated ? `, updated ${fmtStamp(app.updated)}` : ''}</dd>{/if}
          <dt>LAST SENT</dt><dd class="text-text-dim">{app.lastServed ? fmtStamp(app.lastServed) : 'Never'}</dd>
          <dt>ON THIS PC</dt><dd class="text-text-dim">{app.installedHere ? 'Installed' : 'Not installed'}</dd>
        </dl>
        {#if app.depots.length}
          <table class="micro mt-4 w-full border border-hairline text-left">
            <thead class="text-text-faint"><tr><th class="px-2 py-1.5">DEPOT</th><th class="px-2">KEPT</th><th class="px-2">STEAM NOW</th><th class="px-2">STATE</th></tr></thead>
            <tbody>
              {#each app.depots as d (d.depot)}
                <tr class="border-t border-hairline-faint text-text-dim">
                  <td class="px-2 py-1.5 font-mono">{d.depot}</td>
                  <td class="px-2">{fmtSize(d.cached)}</td>
                  <td class="px-2">{d.size ? fmtSize(d.size) : '—'}</td>
                  <td class="px-2 {d.upToDate ? 'text-live' : 'text-chiral'}">{d.cached ? (d.upToDate ? 'CURRENT' : 'OLDER BUILD') : 'NOT YET'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
        {#if app.state === 'update'}
          <p class="micro mt-3 leading-relaxed text-chiral">STEAM HAS A NEWER BUILD. THE FIRST PC TO UPDATE BRINGS IT INTO THE CACHE; EVERY PC AFTER GETS IT FROM HERE.</p>
        {/if}
        <div class="mt-4 flex flex-wrap gap-2">
          {#if app.kept}
            <button class="micro expo t-hover flex items-center gap-1.5 border border-hairline px-3 py-2 text-text-dim hover:text-white" onclick={() => keep(app.app, false)}><PinOff class="h-3.5 w-3.5" />STOP KEEPING</button>
          {:else if app.app}
            <button class="micro expo t-hover flex items-center gap-1.5 border border-chiral/50 px-3 py-2 text-chiral hover:bg-chiral/10" onclick={() => keep(app.app)}><Pin class="h-3.5 w-3.5" />KEEP</button>
          {/if}
          {#if app.app}
            <a class="micro expo t-hover flex items-center gap-1.5 border border-hairline px-3 py-2 text-text-dim hover:text-white" href={`https://store.steampowered.com/app/${app.app}/`} target="_blank" rel="noreferrer noopener"><ExternalLink class="h-3.5 w-3.5" />STORE PAGE</a>
          {/if}
          <button class="micro expo t-hover ml-auto flex items-center gap-1.5 border border-red-500/50 px-3 py-2 text-red-400 hover:bg-red-500/10 disabled:opacity-50" onclick={() => purge(app)} disabled={busy === `purge:${app.app}`}>
            <Trash2 class="h-3.5 w-3.5" />PURGE
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
