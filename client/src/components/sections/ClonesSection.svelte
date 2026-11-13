<!-- ClonesSection: a workbench of virtual machines. Systems is the catalogue: click a tile
     to start a disk that exists, make a new machine, or run a temporary one. Machines lists
     what exists. Automations are recipes with any number of bots, each on its own machine.
     Benchmarks shows how long launches took. -->
<script>
  import { untrack } from 'svelte';
  import { Boxes, Bot, Monitor, Play, Square, Power, Plus, Trash2, Pencil, Search, Timer, LayoutGrid, ScrollText } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import { prefs } from '../../lib/prefs.svelte.js';
  import { STATE_LABEL, STATE_TONE, INPUT, BTN, BTN_PRIMARY, BTN_STOP, FAMILIES, fmtSeconds } from '../../lib/clones-ui.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';
  import VmScreen from '../VmScreen.svelte';
  import BotPanel from '../clones/BotPanel.svelte';
  import LaunchDialog from '../clones/LaunchDialog.svelte';
  import AutomationEditor from '../clones/AutomationEditor.svelte';

  const SECOND_MS = 1000;
  const CONFIRM_MS = 4000;
  const NO_CLONES = 'not started with --clonewars';
  const ADMIN = 'Admin';
  const VIEWS = [
    { id: 'systems', label: 'Systems', icon: LayoutGrid },
    { id: 'machines', label: 'Machines', icon: Monitor },
    { id: 'automations', label: 'Automations', icon: Bot },
    { id: 'benchmarks', label: 'Benchmarks', icon: Timer },
  ];
  const KIND_LABEL = { main: 'MAIN DISK', instance: 'OWN DISK', temporary: 'TEMPORARY' };
  const MODE_LABEL = { existing: 'EXISTING DISK', new: 'NEW MACHINE', temporary: 'TEMPORARY', automation: 'AUTOMATION' };

  let view = $state('systems');
  let automations = $state([]);
  let lab = $state({ targets: [], machines: [], bench: [] });
  let problem = $state('');
  let loaded = $state(false);
  let busy = $state(false);
  let turningOn = $state(false);
  let query = $state('');
  let family = $state('');
  let picked = $state(null); // the system whose launch dialog is open
  let editing = $state(null); // { initial } while the automation editor is open
  let openMachine = $state(''); // id of the machine whose log and screen are shown
  let confirming = $state(''); // id of the thing whose delete button was pressed once

  const source = $derived(shell.clonesSource);
  const allowed = $derived(shell.user.role === ADMIN);
  const running = $derived(new Set(lab.machines.filter((m) => m.state === 'running' || m.state === 'launching').map((m) => m.target)));
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return lab.targets
      .filter((t) => (!family || t.family === family) && (!q || `${t.id} ${t.name} ${t.description} ${t.family} ${t.runtime}`.toLowerCase().includes(q)))
      .sort((a, b) => Number(b.built) - Number(a.built));
  });
  /** Fastest good launch per system, apart for first builds and for ordinary boots. */
  const records = $derived.by(() => {
    const best = new Map();
    for (const b of lab.bench) {
      if (!b.ok) continue;
      const entry = best.get(b.target) || { target: b.target, build: null, boot: null };
      const slot = b.firstBoot ? 'build' : 'boot';
      if (entry[slot] == null || b.readySeconds < entry[slot]) entry[slot] = b.readySeconds;
      best.set(b.target, entry);
    }
    return [...best.values()];
  });
  const targetName = (id) => lab.targets.find((t) => t.id === id)?.name || id;

  /** Clones is off on this box: ask the box to find its clonewars folder and turn it on. */
  async function turnOn() {
    turningOn = true;
    try {
      await source.clonesEnable();
      await refresh();
      store.toast('Clones is on');
    } catch (err) {
      store.toast(err?.message || 'Clones could not be turned on');
    } finally {
      turningOn = false;
    }
  }

  async function refresh(from = source) {
    if (!from) {
      automations = [];
      loaded = true;
      return;
    }
    try {
      const [list, machines] = await Promise.all([from.clones(), from.cloneMachines()]);
      automations = list;
      lab = machines;
      problem = '';
    } catch (err) {
      problem = err?.message || 'Clones could not be read';
    }
    loaded = true;
  }

  /** Runs one box call, hands its answer to `apply`, and resolves true when it worked. */
  async function act(call, done, apply) {
    busy = true;
    try {
      apply(await call());
      if (done) store.toast(done);
      return true;
    } catch (err) {
      store.toast(err?.message || 'That did not work');
      return false;
    } finally {
      busy = false;
    }
  }

  /** The box answers an automation call with that automation, or with the whole list. */
  function applyAutomation(result) {
    if (Array.isArray(result)) automations = result;
    else if (automations.some((a) => a.id === result.id)) automations = automations.map((a) => (a.id === result.id ? result : a));
    else automations = [...automations, result];
  }
  const applyLab = (result) => (lab = result);
  const actAutomation = (call, done) => act(call, done, applyAutomation);

  async function launch(body) {
    const ok = await act(() => source.cloneMachineStart(body), 'Starting the machine', applyLab);
    if (!ok) return;
    picked = null;
    view = 'machines';
  }

  async function saveAutomation(def) {
    if (await actAutomation(() => source.cloneAutomationSave(def), def.id ? 'Saved' : 'Automation created')) editing = null;
  }

  /** Deleting takes two presses: the first arms the button for a few seconds. */
  function twice(id, run) {
    if (confirming !== id) {
      confirming = id;
      setTimeout(() => confirming === id && (confirming = ''), CONFIRM_MS);
      return;
    }
    confirming = '';
    run();
  }

  $effect(() => {
    const from = source;
    untrack(() => {
      loaded = false;
      refresh(from);
    });
  });

  // Re-read states and logs on the interval chosen in Settings > Clones.
  $effect(() => {
    const timer = setInterval(() => {
      if (!busy) refresh();
    }, prefs.get('clones.pollSeconds') * SECOND_MS);
    return () => clearInterval(timer);
  });
</script>

<SectionFrame
  title="Clones"
  subtitle="Virtual machines and automations"
  tag={store.isDemo ? 'SAMPLE DATA' : ''}
  items={VIEWS}
  active={view}
  onselect={(id) => (view = id)}
>
  {#if !allowed}
    <Notice icon={Boxes} title="ADMINS ONLY">
      Clones start virtual machines on the box, so only an admin can use this section. Switch person from the Castle mark, top left.
    </Notice>
  {:else if !source}
    <Notice icon={Boxes} title="NO BOX CONNECTED">
      Clones run on a Castle box. Connect to one from Storage, or open the
      <a class="text-bridge underline" href="/demo">demo</a> to try it with a pretend launch.
    </Notice>
  {:else if !loaded}
    <p class="micro text-text-faint">CHECKING THE BOX</p>
  {:else if problem.includes(NO_CLONES)}
    <Notice icon={Boxes} title="CLONES ARE OFF ON THIS BOX">
      This box was started without its <span class="text-white">clonewars</span> folder, so it cannot run machines yet. Turn it
      on here and the box finds the folder itself. No restart needed.
      <button disabled={turningOn} onclick={turnOn} class="{BTN_PRIMARY} mx-auto mt-4">
        <Power class="h-4 w-4" strokeWidth={2} /> {turningOn ? 'TURNING ON' : 'TURN ON CLONES'}
      </button>
    </Notice>
  {:else if problem}
    <Notice icon={Boxes} title="CLONES UNAVAILABLE">{problem}</Notice>
  {:else if view === 'systems'}
    <div class="mb-4 flex flex-wrap items-center gap-3">
      <label class="relative min-w-[14rem] flex-1">
        <Search class="pointer-events-none absolute left-3 top-3 h-4 w-4 text-text-faint" />
        <input bind:value={query} type="search" placeholder="Search {lab.targets.length} systems" spellcheck="false" class="{INPUT} pl-9" />
      </label>
      <div class="flex gap-1" role="group" aria-label="Kind of system">
        {#each FAMILIES as f (f.id)}
          <button class="{BTN} {family === f.id ? '!border-chiral/70 !text-chiral' : ''}" aria-pressed={family === f.id} onclick={() => (family = f.id)}>{f.label.toUpperCase()}</button>
        {/each}
      </div>
    </div>
    <ul class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
      {#each shown as t (t.id)}
        <li>
          <button onclick={() => (picked = t)} class="expo t-hover hud-corners glass flex h-full w-full flex-col p-4 text-left transition-colors hover:border-chiral/60">
            <span class="flex w-full items-start gap-2">
              <span class="min-w-0 flex-1 text-[15px] font-bold leading-snug text-white">{t.name}</span>
              {#if running.has(t.id)}<span class="mt-1.5 h-2 w-2 shrink-0 animate-pulse rounded-full bg-live" title="Running"></span>{/if}
            </span>
            <span class="mt-1 line-clamp-2 flex-1 text-[12px] leading-relaxed text-text-dim">{t.description}</span>
            <span class="micro mt-3 flex flex-wrap gap-x-3 gap-y-1 text-text-faint">
              <span>{(t.family || 'other').toUpperCase()}</span>
              {#if t.built}<span class="text-live">BUILT</span>{/if}
              {#if t.disks > 1}<span>{t.disks} DISKS</span>{/if}
              {#if t.inject}<span class="text-chiral">SCRIPTABLE</span>{/if}
              {#if t.apk}<span class="text-chiral">TAKES APKS</span>{/if}
              {#if t.runtime}<span>{t.runtime === 'avd' ? 'EMULATOR' : t.runtime.toUpperCase()}</span>{/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>
    {#if !shown.length}
      <p class="mt-6 text-[13px] leading-relaxed text-text-dim">
        Nothing matches. The catalogue is Clonewars' own target list (clonewars/modules/targets.ps1): a system added there shows up here.
      </p>
    {/if}
  {:else if view === 'machines'}
    {#if !lab.machines.length}
      <Notice icon={Monitor} title="NO MACHINES YET">Pick a system under Systems and start one. It appears here.</Notice>
    {/if}
    <ul class="grid gap-3 xl:grid-cols-2">
      {#each lab.machines as m (m.id)}
        {@const live = m.state === 'running' || m.state === 'launching'}
        {@const open = openMachine === m.id}
        <li class="glass p-4 {open ? 'xl:col-span-2' : ''}">
          <div class="flex flex-wrap items-center gap-3">
            <Monitor class="h-7 w-7 shrink-0 {m.state === 'running' ? 'text-live' : 'text-text-faint'}" strokeWidth={1.25} />
            <div class="min-w-0 flex-1">
              <p class="truncate text-[15px] font-semibold text-white">{m.name}</p>
              <p class="micro mt-1 flex flex-wrap gap-x-3 text-text-faint">
                <span>{KIND_LABEL[m.kind]}</span>
                {#if m.kind !== 'main'}<span>{targetName(m.target)}</span>{/if}
                {#if m.diskBytes}<span>{fmtSize(m.diskBytes)}</span>{/if}
              </p>
            </div>
            <p class="micro flex items-center gap-2 {STATE_TONE[m.state]}" role="status">
              <span class="h-2 w-2 rounded-full bg-current {m.state === 'launching' ? 'animate-pulse' : ''}"></span>
              {STATE_LABEL[m.state] || m.state}
            </p>
            {#if live}
              <button disabled={busy} class={BTN_STOP} onclick={() => act(() => source.cloneMachineStop(m.id), m.engine === 'container' ? 'Stopping the container' : 'Shutting the machine down', applyLab)}>
                <Square class="h-3.5 w-3.5 fill-current" /> STOP
              </button>
            {:else if m.kind !== 'temporary'}
              <button disabled={busy} class={BTN} onclick={() => act(() => source.cloneMachineStart({ mode: 'existing', machine: m.id }), 'Starting the machine', applyLab)}>
                <Play class="h-3.5 w-3.5 fill-current" /> START
              </button>
            {/if}
            <button class={BTN} aria-pressed={open} title="Log and screen" onclick={() => (openMachine = open ? '' : m.id)}><ScrollText class="h-4 w-4" /></button>
            {#if m.kind !== 'main' && !live}
              <button disabled={busy} class={BTN} title="Delete this machine and its disk" onclick={() => twice(m.id, () => act(() => source.cloneMachineDelete(m.id), 'Machine deleted', applyLab))}>
                <Trash2 class="h-4 w-4" />{#if confirming === m.id}DELETE IT AND ITS DISK?{/if}
              </button>
            {/if}
          </div>
          {#if open}
            <pre class="mt-3 max-h-56 overflow-auto border border-hairline bg-black/60 p-3 font-mono text-[11px] leading-relaxed text-text-dim">{m.log.length ? m.log.join('\n') : m.state === 'running' ? 'Running. It was started outside this screen, so there is no launch log here.' : 'Nothing yet.'}</pre>
            {#if m.state === 'running' && source.cloneScreen}<VmScreen {source} id={m.key} controllable={m.controllable} name={m.name} />{/if}
          {/if}
        </li>
      {/each}
    </ul>
  {:else if view === 'automations'}
    <div class="mb-4 flex items-center gap-3">
      <p class="flex-1 text-[13px] text-text-dim">A recipe and its bots. Each bot signs in as itself and runs on its own machine, so several can run at once.</p>
      <button class={BTN_PRIMARY} onclick={() => (editing = { initial: null })}><Plus class="h-4 w-4" /> NEW AUTOMATION</button>
    </div>
    <div class="space-y-4">
      {#each automations as a (a.id)}
        <article class="hud-corners glass">
          <header class="flex flex-wrap items-center gap-3 border-b border-hairline px-5 py-4">
            <span class="grid h-10 w-10 shrink-0 place-items-center border border-chiral/60 bg-chiral-dim">
              <Bot class="h-5 w-5 text-chiral" strokeWidth={1.5} />
            </span>
            <div class="min-w-0 flex-1">
              <h2 class="text-[17px] font-bold text-white">{a.name}</h2>
              <p class="text-[13px] text-text-dim">{a.description}</p>
              <p class="micro mt-1 flex flex-wrap gap-x-3 text-text-faint">
                <span>{targetName(a.target)}</span>
                <span>{a.cores} CORES · {a.memoryGb} GB</span>
                {#if a.builtin}<span>EXAMPLE</span>{/if}
                {#if a.temporary}<span>TEMPORARY</span>{/if}
                {#if a.restart}<span>LOOPS</span>{/if}
                {#if a.stopAfterMinutes}<span>STOPS AFTER {a.stopAfterMinutes} MIN</span>{/if}
              </p>
            </div>
            <button class={BTN} onclick={() => (editing = { initial: a })}><Pencil class="h-3.5 w-3.5" /> CHANGE</button>
            {#if !a.builtin}
              <button disabled={busy} class={BTN} onclick={() => twice(a.id, () => actAutomation(() => source.cloneAutomationDelete(a.id), 'Automation removed'))}>
                <Trash2 class="h-3.5 w-3.5" />{#if confirming === a.id}REMOVE IT?{/if}
              </button>
            {/if}
          </header>
          <div class="p-5">
            {#if !a.repoPresent}
              <p class="mb-3 text-[12px] text-text-dim">Its project is not on the box yet. It is fetched on the first start when the project is a git address; a folder has to be put there by hand.</p>
            {/if}
            <div class="grid gap-3 {a.bots.length > 1 ? 'xl:grid-cols-2' : ''}">
              {#each a.bots as bot (bot.id)}
                <BotPanel {source} automation={a} {bot} {busy} onact={actAutomation} />
              {/each}
            </div>
            <button disabled={busy} class="{BTN} mt-3" onclick={() => actAutomation(() => source.cloneBotSave(a.id, '', '', {}), 'Bot added')}>
              <Plus class="h-3.5 w-3.5" /> ADD ANOTHER BOT
            </button>
          </div>
        </article>
      {/each}
    </div>
  {:else}
    {#if !lab.bench.length}
      <Notice icon={Timer} title="NO LAUNCHES TIMED YET">Every launch from this screen is timed. Start a machine and it shows up here.</Notice>
    {:else}
      <ul class="mb-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {#each records as r (r.target)}
          <li class="glass p-4">
            <p class="truncate text-[13px] font-semibold text-white">{targetName(r.target)}</p>
            <p class="mt-2 text-[22px] font-bold text-live">{fmtSeconds(r.boot)}</p>
            <p class="micro text-text-faint">FASTEST START</p>
            {#if r.build != null}<p class="micro mt-2 text-text-faint">FIRST BUILD {fmtSeconds(r.build)}</p>{/if}
          </li>
        {/each}
      </ul>
      <div class="overflow-auto border border-hairline">
        <table class="w-full text-left text-[13px]">
          <thead class="micro text-text-faint">
            <tr class="border-b border-hairline">
              <th class="p-3 font-normal">WHEN</th><th class="p-3 font-normal">WHAT</th><th class="p-3 font-normal">HOW</th>
              <th class="p-3 font-normal" title="From pressing Start until the machine is handed to QEMU: downloads, checks and disk work">PREPARED</th>
              <th class="p-3 font-normal" title="From pressing Start until the launcher finished: the system is up and anything put into it has run">READY</th>
              <th class="p-3 font-normal">RESULT</th>
            </tr>
          </thead>
          <tbody>
            {#each lab.bench as b (b.at + b.key)}
              <tr class="border-b border-hairline/60 text-text-dim">
                <td class="whitespace-nowrap p-3">{new Date(b.at).toLocaleString()}</td>
                <td class="p-3 text-white">{b.label}</td>
                <td class="micro p-3">{MODE_LABEL[b.mode] || b.mode}{b.firstBoot ? ' · FIRST BUILD' : ''}</td>
                <td class="p-3">{fmtSeconds(b.prepSeconds)}</td>
                <td class="p-3 text-white">{fmtSeconds(b.readySeconds)}</td>
                <td class="micro p-3 {b.ok ? 'text-live' : 'text-chiral'}">{b.ok ? 'OK' : 'FAILED'}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="mt-3 text-[12px] leading-relaxed text-text-dim">
        Prepared is the time until the machine is handed to QEMU. Ready is the time until the launcher finished: for a scriptable system that includes installing and running what was put into it.
      </p>
    {/if}
  {/if}
</SectionFrame>

{#if picked}
  <LaunchDialog target={picked} machines={lab.machines.filter((m) => m.target === picked.id && m.kind !== 'temporary')} {busy} onstart={launch} onclose={() => (picked = null)} />
{/if}
{#if editing}
  <AutomationEditor initial={editing.initial} targets={lab.targets} {busy} onsave={saveAutomation} onclose={() => (editing = null)} />
{/if}
