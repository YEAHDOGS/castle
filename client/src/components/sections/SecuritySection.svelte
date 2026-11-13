<!-- SecuritySection: Castle Security. What protects the box and what does not yet, the
     tamper-evident record of everything that changed, who holds the key, and read-only
     audits of the box itself. Protections that are not built are listed as such. -->
<script>
  import { untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import { ask } from '../../lib/ask.svelte.js';
  import { ShieldCheck, ScrollText, KeyRound, ScanSearch, Hammer, Check, TriangleAlert, Minus, CircleHelp, RefreshCw, Play, Link2 } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { itemIn, viewIn } from '../../lib/motion.js';
  import { PLANNED_PROTECTIONS } from '../../lib/demo-shell.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';

  const ADMIN = 'Admin';
  const VIEWS = [
    { id: 'overview', label: 'Overview', icon: ShieldCheck },
    { id: 'activity', label: 'Activity record', icon: ScrollText },
    { id: 'access', label: 'Access', icon: KeyRound },
    { id: 'audits', label: 'Audits', icon: ScanSearch },
    { id: 'planned', label: 'Not built yet', icon: Hammer },
  ];
  const REFRESH_MS = 10_000;
  const LOG_LIMIT = 500;
  const HASH_SHOWN = 12; // leading characters of a hash, enough to compare by eye
  const STATES = {
    ok: { icon: Check, tone: 'text-live', label: 'OK' },
    warn: { icon: TriangleAlert, tone: 'text-chiral', label: 'NEEDS ATTENTION' },
    idle: { icon: Minus, tone: 'text-text-dim', label: 'NOT SET UP' },
    off: { icon: Minus, tone: 'text-text-faint', label: 'NOT BUILT' },
    unknown: { icon: CircleHelp, tone: 'text-bridge', label: 'UNKNOWN' },
  };
  const STATE_ORDER = ['warn', 'unknown', 'ok', 'idle', 'off'];
  // The record stores short action names; these are the words shown for them.
  const ACTION_WORDS = {
    'box.start': 'Box started',
    'access.denied': 'Refused a call with a wrong token',
    upload: 'Uploaded',
    mkdir: 'Made a folder',
    rename: 'Renamed',
    move: 'Moved',
    star: 'Starred or unstarred',
    trash: 'Moved to Trash',
    restore: 'Restored from Trash',
    delete: 'Deleted for good',
    settings: 'Changed settings',
    'clones.login': 'Saved an automation login',
    'clones.start': 'Started an automation',
    'clones.stop': 'Stopped an automation',
    'arcade.play': 'Launched a game',
    'network.speedtest': 'Ran a speed test',
    'security.token.rotate': 'Replaced the box token',
    'security.audits.run': 'Ran an audit',
  };

  let view = $state('overview');
  let overview = $state(null);
  let entries = $state([]);
  let verdict = $state(null); // result of the last explicit re-check
  let verifying = $state(false);
  let rotating = $state(false);
  let audits = $state(null); // { available, reason, audits }
  let report = $state(null); // the audit shown
  let running = $state(''); // id of the audit in flight
  let problem = $state('');

  const source = $derived(shell.securitySource);
  const allowed = $derived(shell.user.role === ADMIN);
  const checks = $derived([...(overview?.checks || [])].sort((a, b) => STATE_ORDER.indexOf(a.state) - STATE_ORDER.indexOf(b.state)));
  const counts = $derived(Object.fromEntries(STATE_ORDER.map((state) => [state, checks.filter((c) => c.state === state).length])));
  const record = $derived(verdict || overview?.record || null);
  const subtitle = $derived(
    !overview ? 'What protects this box' : counts.warn ? `${counts.warn} ${counts.warn === 1 ? 'check needs' : 'checks need'} attention` : `${counts.ok} checks pass, none need attention`,
  );

  const when = (ms) => new Date(ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'medium' });
  const actionWords = (action) => ACTION_WORDS[action] || action;
  /** "204 id=notes.txt" -> { failed, what }: the status decides the tone, the rest says what it touched. */
  function detailOf(entry) {
    const [first, ...rest] = entry.detail.split(' ');
    const status = Number(first);
    if (!Number.isInteger(status) || first.length !== 3) return { failed: false, what: entry.detail };
    return { failed: status >= 400, what: decodeSafely(rest.join(' ')) };
  }
  function decodeSafely(text) {
    try {
      return decodeURIComponent(text.replaceAll('+', ' '));
    } catch {
      return text;
    }
  }

  async function load(from) {
    if (!from) return;
    try {
      const [nextOverview, nextEntries] = await Promise.all([from.security(), from.securityLog(LOG_LIMIT)]);
      overview = nextOverview;
      entries = nextEntries;
      problem = '';
    } catch (err) {
      problem = err?.message || 'Security is unavailable';
    }
  }

  async function verifyNow() {
    verifying = true;
    try {
      verdict = await source.securityVerify();
      store.toast(verdict.intact ? `Record intact: ${verdict.entries} entries` : `Record broken at entry ${verdict.brokenAt}`);
    } catch (err) {
      store.toast(err?.message || 'Could not check the record');
    } finally {
      verifying = false;
    }
  }

  async function rotateToken() {
    const question = 'Replace the box token? Every other device connected to this box is signed out and needs the new token.';
    if (!(await ask(question, { yes: 'Replace the token', danger: true }))) return;
    rotating = true;
    try {
      await source.securityRotate();
      store.toast('Token replaced. This device stays connected.');
      await load(source);
    } catch (err) {
      store.toast(err?.message || 'Could not replace the token');
    } finally {
      rotating = false;
    }
  }

  async function runAudit(id) {
    running = id;
    try {
      report = await source.securityAudit(id);
    } catch (err) {
      store.toast(err?.message || 'The audit did not run');
    } finally {
      running = '';
    }
  }

  // Load when the box changes, then keep the checks and the record fresh.
  $effect(() => {
    const from = source;
    if (!from || !allowed) return;
    untrack(() => {
      overview = null;
      entries = [];
      verdict = null;
      audits = null;
      report = null;
      load(from);
      from.securityAudits().then((list) => (audits = list)).catch(() => (audits = { available: false, reason: 'The audit list could not be read.', audits: [] }));
    });
    const timer = setInterval(() => load(from), REFRESH_MS);
    return () => clearInterval(timer);
  });
</script>

{#snippet stat(label, value, tone)}
  <div class="glass p-4">
    <p class="micro text-text-faint">{label}</p>
    <p class="mt-2 text-[34px] font-black leading-none tabular-nums {tone}">{value}</p>
  </div>
{/snippet}

<SectionFrame title="Security" {subtitle} tag={store.isDemo ? 'SAMPLE DATA' : ''} items={VIEWS} active={view} onselect={(id) => (view = id)}>
  {#if !allowed}
    <Notice icon={ShieldCheck} title="ADMINS ONLY">
      Security shows who can reach the box and can replace its key, so only an admin can use this section. Switch person from the Castle
      mark, top left.
    </Notice>
  {:else if !source}
    <Notice icon={ShieldCheck} title="NO BOX CONNECTED">
      Security reports on a Castle box. Connect to one from Storage, or open the
      <a class="text-bridge underline" href="/demo">demo</a> to see it with sample data.
    </Notice>
  {:else if problem && !overview}
    <Notice icon={ShieldCheck} title="SECURITY UNAVAILABLE">{problem}</Notice>
  {:else}
        {#if view === 'overview'}
          <div class="grid gap-3 sm:grid-cols-3">
            {@render stat('NEED ATTENTION', counts.warn ?? 0, counts.warn ? 'text-chiral' : 'text-white')}
            {@render stat('PASS', counts.ok ?? 0, 'text-live')}
            {@render stat('NOT BUILT YET', counts.off ?? 0, 'text-text-dim')}
          </div>
          <ul class="mt-3 grid gap-3 lg:grid-cols-2">
            {#each checks as item, i (item.id)}
              {@const look = STATES[item.state] || STATES.unknown}
              <li in:fly={itemIn(i)} class="flex gap-3 border bg-white/[0.02] p-4 {item.state === 'warn' ? 'border-chiral/60' : 'border-hairline'}">
                <look.icon class="mt-0.5 h-5 w-5 shrink-0 {look.tone}" strokeWidth={2} />
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-baseline justify-between gap-x-3">
                    <h2 class="text-[15px] font-bold text-white">{item.label}</h2>
                    <span class="micro {look.tone}">{look.label}</span>
                  </div>
                  <p class="mt-1 text-[13px] leading-relaxed text-text-dim">{item.detail}</p>
                </div>
              </li>
            {/each}
          </ul>
          <p class="mt-4 max-w-3xl text-[12px] leading-relaxed text-text-dim">
            These are the checks the box can make about itself today. It does not scan for malware or watch other devices. "Not built" means
            Castle does not have that protection yet.
          </p>
        {:else if view === 'activity'}
          <div class="glass flex flex-wrap items-center gap-x-6 gap-y-3 p-4">
            <div class="min-w-0 flex-1">
              <p class="micro flex items-center gap-2 {record?.intact ? 'text-live' : 'text-chiral'}">
                <Link2 class="h-3.5 w-3.5" />
                {record ? (record.intact ? `INTACT · ${record.entries} ENTRIES` : `BROKEN AT ENTRY ${record.brokenAt}`) : 'READING'}
              </p>
              <p class="mt-1 truncate font-mono text-[12px] text-text-dim" title="The newest entry's fingerprint">
                {record?.head || ''}
              </p>
            </div>
            <button
              disabled={verifying}
              onclick={verifyNow}
              class="expo t-hover micro flex h-10 items-center gap-2 border border-hairline px-4 font-bold text-white transition-colors hover:border-chiral/60 disabled:opacity-50"
            >
              <RefreshCw class="h-3.5 w-3.5 {verifying ? 'animate-spin' : ''}" /> {verifying ? 'CHECKING' : 'CHECK EVERY ENTRY'}
            </button>
          </div>
          <p class="mt-3 max-w-3xl text-[12px] leading-relaxed text-text-dim">
            Every change made through Castle is written here. Each entry carries the fingerprint of the one before it, so changing or
            removing an old entry breaks every fingerprint after it. Someone with the disk could still rewrite the whole file; catching
            that needs a copy of the newest fingerprint on another device, which is not built yet.
          </p>
          <div class="glass mt-3 overflow-x-auto p-4">
            <table class="w-full text-left text-[13px]">
              <thead class="micro text-text-faint">
                <tr><th class="pb-2 pr-3 font-normal">#</th><th class="pb-2 pr-3 font-normal">WHEN</th><th class="pb-2 pr-3 font-normal">WHAT HAPPENED</th><th class="pb-2 pr-3 font-normal">ON</th><th class="pb-2 font-normal">FINGERPRINT</th></tr>
              </thead>
              <tbody>
                {#each entries as entry (entry.seq)}
                  {@const detail = detailOf(entry)}
                  <tr class="border-t border-hairline-faint align-top">
                    <td class="py-2 pr-3 tabular-nums text-text-faint">{entry.seq}</td>
                    <td class="whitespace-nowrap py-2 pr-3 text-text-dim">{when(entry.at)}</td>
                    <td class="py-2 pr-3 {entry.action === 'access.denied' || detail.failed ? 'text-chiral' : 'text-white'}">
                      {actionWords(entry.action)}{detail.failed ? ' (failed)' : ''}
                    </td>
                    <td class="max-w-[22rem] break-words py-2 pr-3 text-text-dim">{detail.what}</td>
                    <td class="py-2 font-mono text-[12px] text-text-faint" title={entry.hash}>{entry.hash.slice(0, HASH_SHOWN)}</td>
                  </tr>
                {:else}
                  <tr><td colspan="5" class="py-6 text-center text-text-dim">Nothing recorded yet.</td></tr>
                {/each}
              </tbody>
            </table>
            {#if entries.length >= LOG_LIMIT}
              <p class="micro mt-3 text-text-faint">SHOWING THE NEWEST {LOG_LIMIT}</p>
            {/if}
          </div>
        {:else if view === 'access'}
          <div class="grid gap-3 sm:grid-cols-3">
            {@render stat('REFUSED CALLS, 24 H', overview?.deniedLastDay ?? 0, overview?.deniedLastDay ? 'text-chiral' : 'text-white')}
            <div class="glass p-4 sm:col-span-2">
              <p class="micro text-text-faint">BOX TOKEN</p>
              <p class="mt-2 text-[15px] text-white">{overview?.tokenMadeAt ? `Made ${when(overview.tokenMadeAt)}` : 'Age unknown'}</p>
              <p class="micro mt-1 text-text-faint">BOX RUNNING SINCE {overview?.startedAt ? when(overview.startedAt).toUpperCase() : 'UNKNOWN'}</p>
            </div>
          </div>
          <div class="glass mt-3 p-4">
            <h2 class="text-[15px] font-bold text-white">Replace the token</h2>
            <p class="mt-1 max-w-2xl text-[13px] leading-relaxed text-text-dim">
              The token is the one key to this box: any device that has it can read and change everything. Replace it if a device that
              held it is lost or you no longer trust it. Every other device is signed out at once; this one stays connected.
            </p>
            <button
              disabled={rotating}
              onclick={rotateToken}
              class="expo t-hover hud-corners micro mt-4 flex h-11 items-center gap-2 bg-chiral px-5 font-bold text-black transition-all hover:brightness-110 active:scale-[0.98] disabled:opacity-50"
            >
              <KeyRound class="h-4 w-4" strokeWidth={2} /> {rotating ? 'REPLACING' : 'REPLACE THE TOKEN'}
            </button>
          </div>
          <p class="mt-4 max-w-3xl text-[12px] leading-relaxed text-text-dim">
            Refused calls are counted since the box last started. There are no per-person accounts or per-device keys yet, so the box cannot
            say which device made a call.
          </p>
        {:else if view === 'audits'}
          {#if audits && !audits.available}
            <div class="glass mb-3 flex gap-3 p-4">
              <TriangleAlert class="mt-0.5 h-5 w-5 shrink-0 text-chiral" />
              <p class="text-[13px] leading-relaxed text-text-dim">{audits.reason} The list below is what will run once the box is on Linux.</p>
            </div>
          {/if}
          <ul class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
            {#each audits?.audits || [] as audit, i (audit.id)}
              <li in:fly={itemIn(i)} class="flex flex-col border bg-white/[0.02] p-4 {report?.id === audit.id ? 'border-chiral/60' : 'border-hairline'}">
                <h2 class="text-[15px] font-bold text-white">{audit.label}</h2>
                <p class="mt-1 flex-1 text-[13px] leading-relaxed text-text-dim">{audit.about}</p>
                <button
                  disabled={!audits.available || !!running}
                  onclick={() => runAudit(audit.id)}
                  class="expo t-hover micro mt-3 flex h-9 items-center justify-center gap-2 border border-hairline font-bold text-white transition-colors hover:border-chiral/60 disabled:opacity-40"
                >
                  <Play class="h-3.5 w-3.5" /> {running === audit.id ? 'RUNNING' : 'RUN'}
                </button>
              </li>
            {/each}
          </ul>
          {#if report}
            {#key `${report.id}:${report.at}`}
              <div in:fly={viewIn()} class="glass mt-3 overflow-x-auto p-4">
                <p class="micro mb-3 flex flex-wrap gap-x-4 text-text-faint">
                  <span class="text-white">{(audits?.audits.find((a) => a.id === report.id)?.label || report.id).toUpperCase()}</span>
                  <span>{report.rows.length} ROWS</span>
                  <span>{when(report.at).toUpperCase()}</span>
                  {#if report.limited}<span class="text-chiral">THE BOX IS NOT RUNNING AS ROOT, SO SOME DETAILS ARE HIDDEN</span>{/if}
                </p>
                <table class="w-full text-left text-[13px]">
                  <thead class="micro text-text-faint">
                    <tr>{#each report.columns as column (column)}<th class="pb-2 pr-4 font-normal">{column}</th>{/each}</tr>
                  </thead>
                  <tbody>
                    {#each report.rows as row, r (r)}
                      <tr class="border-t border-hairline-faint align-top">
                        {#each row as cell, c (c)}
                          <td class="max-w-[28rem] break-words py-1.5 pr-4 {c === 0 ? 'text-white' : 'text-text-dim'}">{cell}</td>
                        {/each}
                      </tr>
                    {:else}
                      <tr><td colspan={report.columns.length} class="py-6 text-center text-text-dim">Nothing found.</td></tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/key}
          {/if}
          <p class="mt-4 max-w-3xl text-[12px] leading-relaxed text-text-dim">
            Audits only read; none changes the box. They list what is there and do not judge it: an unfamiliar entry is a prompt to look,
            not a verdict. They began as the Phoenix project's system listings, rewritten in bash for a Linux box.
          </p>
        {:else}
          <ul class="grid gap-3 lg:grid-cols-2">
            {#each PLANNED_PROTECTIONS as item, i (item.title)}
              <li in:fly={itemIn(i)} class="border border-hairline bg-white/[0.02] p-4">
                <div class="flex items-baseline justify-between gap-3">
                  <h2 class="text-[15px] font-bold text-white">{item.title}</h2>
                  <span class="micro shrink-0 text-text-faint">NOT BUILT</span>
                </div>
                <p class="mt-1 text-[13px] leading-relaxed text-text-dim">{item.what}</p>
              </li>
            {/each}
          </ul>
        {/if}
  {/if}
</SectionFrame>
