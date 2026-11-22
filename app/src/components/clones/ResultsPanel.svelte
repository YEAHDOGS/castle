<!-- ResultsPanel: what a Castle script's runs found, from the box's database. The update in
     words first (today against the days before it), the tracked values over time, the newest
     run and the line it is on or failed at, its pictures, the rows of each dataset, and the
     runs before it. -->
<script>
  import { untrack } from 'svelte';
  import { TrendingDown, TrendingUp, CircleAlert, CircleCheck, LoaderCircle, Image, ScrollText, Table } from '@lucide/svelte';
  import { BTN } from '../../lib/clones-ui.js';
  import Sparkline from './Sparkline.svelte';

  let { source, automation } = $props();

  const ROWS_SHOWN = 25;
  const STATUS = {
    starting: { label: 'STARTING', tone: 'text-chiral', icon: LoaderCircle },
    running: { label: 'RUNNING', tone: 'text-chiral', icon: LoaderCircle },
    done: { label: 'DONE', tone: 'text-live', icon: CircleCheck },
    failed: { label: 'FAILED', tone: 'text-chiral', icon: CircleAlert },
    stopped: { label: 'STOPPED', tone: 'text-text-faint', icon: CircleAlert },
  };

  let results = $state(null);
  let problem = $state('');
  let shots = $state({}); // "<run>/<n>" -> object URL
  let bigShot = $state('');
  let openSet = $state('');
  let showAll = $state(false);
  let showLog = $state(false);

  const latest = $derived(results?.runs?.[0]);
  /** Re-read when the newest run moves on: a new run, a new line, a new status. */
  const pulse = $derived(`${automation.latest?.run?.id}|${automation.latest?.run?.status}|${automation.latest?.run?.line}`);
  const money = (name) => /price|cost/i.test(name);
  const fmt = (name, v) => (v == null ? '-' : money(name) ? `$${Number(v).toFixed(2)}` : Number.isInteger(v) ? String(v) : Number(v).toFixed(2));
  const when = (ms) => (ms ? new Date(ms).toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }) : '');

  async function load() {
    try {
      results = await source.cloneResults(automation.id);
      problem = '';
      for (const run of results.runs.slice(0, 1)) {
        for (const s of run.shots) {
          const key = `${run.id}/${s.n}`;
          if (!shots[key] && source.cloneResultShot) shots[key] = await source.cloneResultShot(key).catch(() => '');
        }
      }
    } catch (err) {
      problem = err?.message || 'The results could not be read';
    }
  }

  $effect(() => {
    pulse;
    untrack(load);
  });
  $effect(() => () => Object.values(shots).forEach((url) => url?.startsWith('blob:') && URL.revokeObjectURL(url)));

  /** The cell as text: numbers rounded, links as links. */
  function cell(value) {
    if (value == null) return '';
    if (typeof value === 'number') return Number.isInteger(value) ? String(value) : value.toFixed(2);
    return String(value);
  }
  const isLink = (v) => typeof v === 'string' && /^https?:\/\//.test(v);
</script>

<section class="border border-hairline bg-black/30 p-4" aria-label="Results">
  {#if problem}
    <p class="text-[13px] text-text-dim">{problem}</p>
  {:else if !results}
    <p class="micro text-text-faint">READING THE RESULTS</p>
  {:else if !results.runs.length}
    <p class="text-[13px] leading-relaxed text-text-dim">No runs yet. Press Start, or turn on Run every day in its rules: what it finds shows up here and stays in Castle's database.</p>
  {:else}
    {#if results.update}
      <p class="text-[15px] leading-relaxed text-white">{results.update}</p>
    {/if}

    {#each results.metrics as m (m.name)}
      {@const values = m.points.map((p) => p.value)}
      {@const last = values.at(-1)}
      {@const first = values[0]}
      {@const Trend = last <= first ? TrendingDown : TrendingUp}
      <div class="mt-4 grid items-center gap-4 sm:grid-cols-[minmax(0,14rem)_minmax(0,1fr)]">
        <div>
          <p class="micro text-text-faint">{m.name.toUpperCase()}</p>
          <p class="mt-1 flex items-center gap-2 text-[24px] font-bold text-white">
            {fmt(m.name, last)}
            {#if values.length > 1}<Trend class="h-4 w-4 text-text-dim" aria-hidden="true" />{/if}
          </p>
          <p class="micro text-text-faint">
            {m.points.length} READING{m.points.length === 1 ? '' : 'S'} · LOW {fmt(m.name, Math.min(...values))} · HIGH {fmt(m.name, Math.max(...values))}
            {#if m.better}· {m.better.toUpperCase()} IS BETTER{/if}
          </p>
        </div>
        {#if m.points.length > 1}
          <Sparkline points={m.points} label={m.name} format={(v) => fmt(m.name, v)} />
        {:else}
          <p class="text-[12px] text-text-dim">One reading so far. The line draws itself as the days add up.</p>
        {/if}
      </div>
    {/each}

    {#if latest}
      {@const st = STATUS[latest.status] || STATUS.stopped}
      <div class="mt-4 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-hairline pt-3 text-[13px]">
        <span class="micro flex items-center gap-1.5 {st.tone}"><st.icon class="h-3.5 w-3.5 {latest.status === 'running' || latest.status === 'starting' ? 'animate-spin' : ''}" /> {st.label}</span>
        <span class="text-text-dim">{when(latest.started)}{latest.trigger === 'schedule' ? ' · on its schedule' : ''}</span>
        {#if latest.status === 'running' && latest.said.startsWith('waiting for you')}
          <span class="text-[13px] font-semibold text-chiral">line {latest.line}: {latest.said}</span>
        {:else if latest.status === 'running' && latest.line}
          <span class="font-mono text-[12px] text-white">line {latest.line}: {latest.said}</span>
        {:else if latest.status === 'starting'}
          <span class="text-text-dim">getting the machine ready (the first run installs a browser)</span>
        {/if}
      </div>
      {#if latest.message}
        <p class="mt-1 text-[13px] leading-relaxed {latest.status === 'failed' ? 'text-chiral' : 'text-text-dim'}">
          {#if latest.line}<span class="font-mono text-[12px]">line {latest.line}{latest.said ? `: ${latest.said}` : ''}. </span>{/if}{latest.message}
        </p>
      {:else if latest.status === 'done' && latest.said}
        <p class="mt-1 text-[13px] text-text-dim">{latest.said}</p>
      {/if}
      {#if latest.shots.length}
        <div class="mt-3 flex flex-wrap gap-2">
          {#each latest.shots as s (s.n)}
            {@const url = shots[`${latest.id}/${s.n}`]}
            <button type="button" class="group w-36 text-left" title={s.label} onclick={() => (bigShot = url || '')}>
              {#if url}<img src={url} alt={s.label} class="aspect-video w-full border border-hairline object-cover object-top group-hover:border-chiral/60" />{:else}<span class="grid aspect-video w-full place-items-center border border-hairline"><Image class="h-4 w-4 text-text-faint" /></span>{/if}
              <span class="micro mt-1 block truncate text-text-faint">{s.label.toUpperCase()}</span>
            </button>
          {/each}
        </div>
      {/if}
    {/if}

    {#each results.datasets as d (d.name)}
      {@const open = openSet === d.name || (openSet === '' && d === results.datasets[0])}
      <div class="mt-4">
        <button type="button" class="micro flex items-center gap-2 text-text-faint hover:text-white" aria-expanded={open} onclick={() => (openSet = open ? '-' : d.name)}>
          <Table class="h-3.5 w-3.5" /> {d.name.toUpperCase()} · {d.total} ROW{d.total === 1 ? '' : 'S'} · {when(d.at)}
        </button>
        {#if open}
          <div class="mt-2 max-h-80 overflow-auto border border-hairline">
            <table class="w-full text-left text-[12px]">
              <thead class="micro sticky top-0 bg-panel-solid text-text-faint">
                <tr>{#each d.columns as c (c)}<th class="whitespace-nowrap p-2 font-normal">{c.toUpperCase()}</th>{/each}</tr>
              </thead>
              <tbody>
                {#each d.rows.slice(0, showAll ? d.rows.length : ROWS_SHOWN) as row, i (i)}
                  <tr class="border-t border-hairline/60 text-text-dim">
                    {#each d.columns as c (c)}
                      <td class="max-w-[24rem] truncate p-2 {typeof row[c] === 'number' ? 'text-right font-mono text-white' : ''}" title={cell(row[c])}>
                        {#if isLink(row[c])}<a class="text-bridge underline" href={row[c]} target="_blank" rel="noreferrer noopener">open</a>{:else}{cell(row[c])}{/if}
                      </td>
                    {/each}
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
          {#if d.rows.length > ROWS_SHOWN}
            <button type="button" class="{BTN} mt-2" onclick={() => (showAll = !showAll)}>{showAll ? 'FEWER ROWS' : `ALL ${d.rows.length} ROWS`}</button>
          {/if}
        {/if}
      </div>
    {/each}

    {#if results.runs.length > 1}
      <details class="mt-4">
        <summary class="micro cursor-pointer text-text-faint hover:text-white">EARLIER RUNS</summary>
        <ul class="mt-2 space-y-1 text-[12px]">
          {#each results.runs.slice(1) as r (r.id)}
            {@const st = STATUS[r.status] || STATUS.stopped}
            <li class="grid grid-cols-[8rem_5rem_minmax(0,1fr)] gap-3 text-text-dim">
              <span>{when(r.started)}</span>
              <span class="micro {st.tone}">{st.label}</span>
              <span class="truncate" title={r.update || r.message || r.said}>{r.status === 'failed' ? `line ${r.line}: ${r.message}` : r.update || r.said || `${r.rows} rows`}</span>
            </li>
          {/each}
        </ul>
      </details>
    {/if}

    {#if results.log.length}
      <button type="button" class="{BTN} mt-3" aria-pressed={showLog} onclick={() => (showLog = !showLog)}><ScrollText class="h-3.5 w-3.5" /> WHAT THE SCRIPT SAID</button>
      {#if showLog}
        <pre class="mt-2 max-h-56 overflow-auto border border-hairline bg-black/60 p-3 font-mono text-[11px] leading-relaxed text-text-dim">{results.log.map((l) => `${l.line ? `line ${l.line}  ` : ''}${l.text}`).join('\n')}</pre>
      {/if}
    {/if}
  {/if}
</section>

{#if bigShot}
  <button type="button" class="fixed inset-0 z-50 grid place-items-center bg-black/85 p-6" aria-label="Close the picture" onclick={() => (bigShot = '')}>
    <img src={bigShot} alt="What the machine's browser showed" class="max-h-full max-w-full border border-hairline" />
  </button>
{/if}
