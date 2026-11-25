<!-- PhoenixSection: free up space, and set a fresh Windows PC up (castled/src/phoenix.rs).
     Phoenix is its own project; the box runs its engine and this section shows the result.

     Three views:
       This PC      what could be cleaned on the computer the box runs on. The scan deletes
                    nothing; it lists each kind of leftover with its size, what it is and
                    its biggest files. Only what is ticked is deleted, after one question.
       A phone      the same for an Android phone plugged into the box with USB debugging on.
       Set up a PC  the apps a fresh machine gets (Chocolatey, searched live), the optional
                    hardening, and the two things made from them: a script to run by hand
                    and a Windows answer file that runs it at the first logon. -->
<script>
  import { Flame, Monitor, Smartphone, PackagePlus, LoaderCircle, RefreshCw, Trash2, ChevronDown, CircleAlert, ShieldAlert, Search, Plus, X, Download, FileCode2, Check, ShieldCheck, Usb } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { store } from '../../lib/store.svelte.js';
  import { shell } from '../../lib/shell.svelte.js';
  import { ask } from '../../lib/ask.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';

  const VIEWS = [
    { id: 'pc', label: 'This PC', icon: Monitor },
    { id: 'phone', label: 'A phone', icon: Smartphone },
    { id: 'setup', label: 'Set up a PC', icon: PackagePlus },
  ];
  const SEARCH_WAIT_MS = 350;
  const BIGGEST_SHOWN = 12;

  // ?view=setup (or phone) opens straight on that view, so a link can point at it.
  const asked = new URLSearchParams(location.search).get('view');
  let view = $state(VIEWS.some((v) => v.id === asked) ? asked : 'pc');
  let status = $state(null); // { available, reason, isAdmin, android, canChange }, null until read
  let problem = $state('');

  // The cleaner. One scan per place: 'windows', or 'android:<serial>'.
  let scan = $state(null);
  let scanning = $state(false);
  let scanProblem = $state('');
  let picked = $state({}); // rule id -> ticked
  let opened = $state({}); // rule id -> details showing
  let cleaning = $state(false);
  let lastClean = $state(null);
  let devices = $state(null); // Android phones on USB, null until asked
  let serial = $state('');

  // Set up a PC.
  let setup = $state(null);
  let apps = $state([]);
  let names = $state({ computerName: '', adminName: '', userName: '' });
  let harden = $state({ enabled: false, chosen: {} });
  let dirty = $state(false);
  let saving = $state(false);
  let query = $state('');
  let found = $state(null);
  let searching = $state(false);
  let showCatalog = $state(false);
  let showHarden = $state(false);
  let adminPassword = $state('');
  let userPassword = $state('');
  let making = $state('');

  const source = $derived(shell.phoenixSource);
  const platform = $derived(view === 'phone' ? 'android' : 'windows');
  const items = $derived(scan?.items || []);
  const groups = $derived.by(() => {
    const by = new Map();
    for (const item of items) {
      if (!item.files && !item.bytes && !item.blocked && !item.sizeUnknown) continue; // nothing there on this machine
      if (!by.has(item.group)) by.set(item.group, []);
      by.get(item.group).push(item);
    }
    return [...by].map(([name, list]) => ({ name, list, bytes: list.reduce((sum, i) => sum + i.bytes, 0) }));
  });
  /** Worth ticking: something is there, or (a phone's app caches) Android has not said how much. */
  const cleanable = (i) => !i.blocked && (i.bytes > 0 || !!i.sizeUnknown);
  const chosen = $derived(items.filter((i) => picked[i.id] && cleanable(i)));
  const chosenBytes = $derived(chosen.reduce((sum, i) => sum + i.bytes, 0));
  const chosenFiles = $derived(chosen.reduce((sum, i) => sum + i.files, 0));
  const blockedBytes = $derived(items.filter((i) => i.blocked).reduce((sum, i) => sum + i.bytes, 0));
  const canChange = $derived(store.isDemo || !!status?.canChange);
  const subtitle = $derived.by(() => {
    if (view === 'setup') return apps.length ? `${apps.length} ${apps.length === 1 ? 'app' : 'apps'} for a fresh machine` : '';
    if (scan) return `${fmtSize(scan.freeBytes)} free · ${fmtSize(scan.totalBytes)} could go`;
    return '';
  });
  const railItems = $derived(VIEWS.map((v) => ({ ...v, count: v.id === 'setup' && apps.length ? apps.length : null })));
  const hardenGroups = $derived.by(() => {
    const by = new Map();
    for (const option of setup?.harden?.options || []) {
      if (!by.has(option.group)) by.set(option.group, []);
      by.get(option.group).push(option);
    }
    return [...by];
  });
  const catalogGroups = $derived.by(() => {
    const by = new Map();
    for (const app of setup?.catalog || []) {
      const key = app.category || 'Other';
      if (!by.has(key)) by.set(key, []);
      by.get(key).push(app);
    }
    return [...by];
  });
  const hardenOn = (option) => (option.name in harden.chosen ? !!harden.chosen[option.name] : option.default);
  const hasApp = (id) => apps.some((a) => a.toLowerCase() === id.toLowerCase());

  // ---------------------------------------------------------------- the cleaner

  async function runScan(from = source, what = platform, which = serial) {
    if (!from) return;
    scanning = true;
    scanProblem = '';
    lastClean = null;
    try {
      const next = await from.phoenixScan(what, which);
      if (from !== source || what !== platform) return;
      scan = next;
      picked = Object.fromEntries(next.items.map((i) => [i.id, i.default && cleanable(i)]));
    } catch (err) {
      scan = null;
      scanProblem = err?.message || 'The scan did not finish';
    } finally {
      scanning = false;
    }
  }

  async function findPhones(from = source) {
    devices = null;
    scan = null;
    scanProblem = '';
    try {
      const list = (await from.phoenixDevices()).devices || [];
      if (from !== source) return;
      devices = list;
      const ready = list.filter((d) => d.state === 'device');
      if (ready.length === 1) {
        serial = ready[0].serial;
        runScan(from, 'android', serial);
      }
    } catch (err) {
      devices = [];
      scanProblem = err?.message || 'The box could not look for a phone';
    }
  }

  async function clean() {
    if (!chosen.length || cleaning) return;
    const where = view === 'phone' ? 'this phone' : 'this PC';
    const what = chosenBytes ? ` files (${fmtSize(chosenBytes)})` : 'what is ticked';
    const sure = await ask(`Delete ${what} from ${where}? Nothing goes to the Recycle Bin.`, { yes: 'Delete', danger: true });
    if (!sure) return;
    cleaning = true;
    try {
      const done = await source.phoenixClean(platform, serial, chosen.map((i) => i.id));
      await runScan();
      lastClean = done;
      store.toast(`${fmtSize(done.freedBytes)} freed`);
    } catch (err) {
      store.toast(err?.message || 'The clean did not finish');
    } finally {
      cleaning = false;
    }
  }

  function tickGroup(group, on) {
    for (const item of group.list) if (cleanable(item)) picked[item.id] = on;
  }

  // ---------------------------------------------------------------- set up a PC

  async function loadSetup(from = source) {
    try {
      const next = await from.phoenixSetup();
      if (from !== source) return;
      setup = next;
      apps = [...next.apps];
      names = { computerName: next.computerName, adminName: next.adminName, userName: next.userName };
      harden = { enabled: !!next.harden?.enabled, chosen: { ...(next.harden?.chosen || {}) } };
      dirty = !next.saved;
    } catch (err) {
      problem = err?.message || 'Phoenix could not be read';
    }
  }

  function addApp(id) {
    if (hasApp(id)) return;
    apps = [...apps, id];
    dirty = true;
  }

  function dropApp(id) {
    apps = apps.filter((a) => a.toLowerCase() !== id.toLowerCase());
    dirty = true;
  }

  /** Saves the tab. True when it is saved (or was already). */
  async function save() {
    if (!dirty) return true;
    saving = true;
    try {
      const body = { apps, harden, computerName: names.computerName, adminName: names.adminName, userName: names.userName };
      setup = await source.phoenixSaveSetup(body);
      dirty = false;
      return true;
    } catch (err) {
      store.toast(err?.message || 'Phoenix could not save that');
      return false;
    } finally {
      saving = false;
    }
  }

  function download(name, text, type) {
    const href = URL.createObjectURL(new Blob([text], { type }));
    const a = document.createElement('a');
    a.href = href;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(href), 1000);
  }

  async function getScript() {
    making = 'script';
    try {
      if (!(await save())) return;
      const made = await source.phoenixScript();
      download('phoenix-setup.ps1', made.script, 'text/plain');
      store.toast(`phoenix-setup.ps1 installs ${made.apps.length} ${made.apps.length === 1 ? 'app' : 'apps'}`);
    } catch (err) {
      store.toast(err?.message || 'The script could not be made');
    } finally {
      making = '';
    }
  }

  async function getAnswerFile() {
    making = 'answer';
    try {
      if (!(await save())) return;
      const made = await source.phoenixAnswerFile(adminPassword, userPassword);
      download('autounattend.xml', made.xml, 'application/xml');
      adminPassword = '';
      userPassword = '';
      for (const warning of made.warnings || []) store.toast(warning);
      store.toast('autounattend.xml is in your downloads. It holds the passwords: delete it once the PC is installed.');
    } catch (err) {
      store.toast(err?.message || 'The answer file could not be made');
    } finally {
      making = '';
    }
  }

  // ---------------------------------------------------------------- loading

  // A new source starts again: is Phoenix on the box at all?
  $effect(() => {
    const from = source;
    untrack(() => {
      status = null;
      scan = null;
      setup = null;
      devices = null;
      problem = '';
      if (!from) return;
      from.phoenixStatus().then(
        (next) => {
          if (from === source) status = next;
        },
        (err) => {
          if (from === source) problem = err?.message || 'Phoenix could not be read';
        },
      );
    });
  });

  // Each view reads what it shows the first time it is opened.
  $effect(() => {
    const from = source;
    const which = view;
    if (!from || !status?.available) return;
    untrack(() => {
      if (which === 'setup') {
        if (!setup) loadSetup(from);
        return;
      }
      scan = null;
      lastClean = null;
      serial = '';
      if (which === 'phone') findPhones(from);
      else runScan(from, 'windows', '');
    });
  });

  // Searching Chocolatey waits for a pause in the typing.
  $effect(() => {
    const q = query.trim();
    const from = source;
    if (q.length < 2 || !from) {
      found = null;
      searching = false;
      return;
    }
    searching = true;
    const timer = setTimeout(async () => {
      try {
        const hits = await from.phoenixChoco(q);
        if (q === query.trim()) found = hits;
      } catch (err) {
        if (q === query.trim()) found = [];
        store.toast(err?.message || 'The search did not work');
      } finally {
        if (q === query.trim()) searching = false;
      }
    }, SEARCH_WAIT_MS);
    return () => clearTimeout(timer);
  });
</script>

{#snippet riskTag(risk)}
  {#if risk === 'caution'}<span class="micro shrink-0 border border-chiral/50 px-1.5 py-0.5 text-chiral">THINK FIRST</span>{/if}
  {#if risk === 'danger'}<span class="micro shrink-0 border border-red-400/60 px-1.5 py-0.5 text-red-300">CAN LOCK YOU OUT</span>{/if}
{/snippet}

{#snippet cleaner()}
  <div class="mx-auto max-w-4xl">
    <!-- ---------------- the total, and the two buttons ---------------- -->
    <div class="mb-4 flex flex-wrap items-end gap-x-6 gap-y-3 border border-hairline bg-white/[0.02] p-4">
      <div class="min-w-0 flex-1">
        <p class="micro text-text-faint">TICKED TO DELETE</p>
        <p class="text-[34px] font-black leading-none tracking-tight text-white">{fmtSize(chosenBytes)}</p>
        <p class="micro mt-1.5 text-text-faint">
          {chosenFiles.toLocaleString()} FILES · {fmtSize(scan.totalBytes)} FOUND IN ALL{#if scan.freeBytes != null} · {fmtSize(scan.freeBytes)} FREE NOW{/if}
        </p>
      </div>
      <div class="flex shrink-0 gap-2">
        <button class="micro expo t-hover flex h-10 items-center gap-1.5 border border-hairline px-3 text-text-dim hover:border-white/30 hover:text-white disabled:opacity-40" onclick={() => runScan()} disabled={scanning || cleaning}>
          <RefreshCw class="h-3.5 w-3.5 {scanning ? 'animate-spin' : ''}" />SCAN AGAIN
        </button>
        <button class="micro expo t-hover flex h-10 items-center gap-1.5 bg-chiral px-4 font-bold text-black hover:opacity-90 disabled:opacity-40" onclick={clean} disabled={!chosen.length || cleaning || scanning || !canChange}>
          {#if cleaning}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />DELETING{:else}<Trash2 class="h-3.5 w-3.5" />DELETE TICKED{/if}
        </button>
      </div>
    </div>

    {#if !canChange}
      <p class="mb-4 flex items-start gap-2 text-[13px] leading-relaxed text-text-dim"><CircleAlert class="mt-0.5 h-4 w-4 shrink-0 text-bridge" />You can look from here, but deleting only works in Castle on the computer the box runs on.</p>
    {/if}
    {#if lastClean}
      <p class="mb-4 flex items-start gap-2 border border-live/30 bg-live/[0.06] p-3 text-[13px] leading-relaxed text-white" aria-live="polite">
        <Check class="mt-0.5 h-4 w-4 shrink-0 text-live" />
        <span>{fmtSize(lastClean.freedBytes)} freed{#if lastClean.files} in {lastClean.files.toLocaleString()} files{/if}.{#if lastClean.failed} {lastClean.failed.toLocaleString()} files were in use and were left; close the program that has them and scan again.{/if}</span>
      </p>
    {/if}
    {#if blockedBytes > 0}
      <p class="mb-4 flex items-start gap-2 text-[13px] leading-relaxed text-text-dim"><ShieldAlert class="mt-0.5 h-4 w-4 shrink-0 text-chiral" />{fmtSize(blockedBytes)} more belongs to Windows itself. To clean it, start Castle with "Run as administrator".</p>
    {/if}

    {#if !groups.length}
      <Notice icon={Check} title="NOTHING TO CLEAN">Phoenix looked in every place it knows and found nothing worth deleting.</Notice>
    {/if}

    <!-- ---------------- every kind of leftover, by group ---------------- -->
    {#each groups as group (group.name)}
      <section class="mb-5">
        <div class="mb-1.5 flex items-center gap-3">
          <h2 class="micro text-white">{group.name.toUpperCase()}</h2>
          <span class="micro text-text-faint">{fmtSize(group.bytes)}</span>
          <span class="flex-1"></span>
          <button class="micro expo t-hover text-text-faint hover:text-white" onclick={() => tickGroup(group, true)}>ALL</button>
          <button class="micro expo t-hover text-text-faint hover:text-white" onclick={() => tickGroup(group, false)}>NONE</button>
        </div>
        <ul class="border border-hairline">
          {#each group.list as item (item.id)}
            {@const off = !cleanable(item)}
            <li class="border-b border-hairline-faint last:border-b-0">
              <div class="flex items-center gap-3 px-3 py-2.5">
                <input type="checkbox" class="h-4 w-4 shrink-0 accent-[#ff6a00] disabled:opacity-30" bind:checked={picked[item.id]} disabled={off} aria-label={`Delete ${item.name}`} />
                <button class="flex min-w-0 flex-1 items-center gap-2 text-left" onclick={() => (opened[item.id] = !opened[item.id])} aria-expanded={!!opened[item.id]}>
                  <span class="min-w-0 truncate text-[14px] {off ? 'text-text-dim' : 'text-white'}">{item.name}</span>
                  {@render riskTag(item.risk)}
                  {#if item.blocked}<span class="micro shrink-0 border border-hairline px-1.5 py-0.5 text-text-faint">NEEDS ADMINISTRATOR</span>{/if}
                  {#if item.running?.length}<span class="micro shrink-0 border border-bridge/40 px-1.5 py-0.5 text-bridge">CLOSE {item.running[0].toUpperCase()} FIRST</span>{/if}
                  <ChevronDown class="h-3.5 w-3.5 shrink-0 text-text-faint transition-transform {opened[item.id] ? 'rotate-180' : ''}" />
                </button>
                <span class="micro shrink-0 text-right text-text-faint">{item.sizeUnknown ? 'SIZE KNOWN ONCE CLEANED' : `${item.files.toLocaleString()} ${item.id === 'android.app-caches' ? 'APPS' : 'FILES'}`}</span>
                <span class="w-20 shrink-0 text-right text-[14px] tabular-nums {off ? 'text-text-dim' : 'text-white'}">{fmtSize(item.bytes)}</span>
              </div>
              {#if opened[item.id]}
                <div class="border-t border-hairline-faint bg-black/30 px-3 py-3 pl-10">
                  <p class="max-w-2xl text-[13px] leading-relaxed text-text-dim">{item.why}</p>
                  {#each item.notes || [] as line (line)}<p class="mt-2 text-[12px] text-bridge">{line}</p>{/each}
                  {#if item.biggest?.length}
                    <p class="micro mt-3 text-text-faint">THE BIGGEST OF THEM</p>
                    <ul class="mt-1">
                      {#each item.biggest.slice(0, BIGGEST_SHOWN) as file (file.path)}
                        <li class="flex items-baseline gap-3 py-0.5 font-mono text-[11.5px] text-text-dim">
                          <span class="w-16 shrink-0 text-right tabular-nums text-white/80">{fmtSize(file.bytes)}</span>
                          <span class="min-w-0 break-all">{file.path}</span>
                        </li>
                      {/each}
                    </ul>
                  {/if}
                  {#if item.folders?.length}
                    <p class="micro mt-3 text-text-faint">WHERE IT LOOKS</p>
                    <ul class="mt-1">{#each item.folders.slice(0, 8) as folder (folder)}<li class="break-all font-mono text-[11.5px] text-text-dim">{folder}</li>{/each}</ul>
                  {/if}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
{/snippet}

<SectionFrame title="Phoenix" {subtitle} tag={store.isDemo ? 'SAMPLE DATA' : ''} items={railItems} active={view} onselect={(id) => (view = id)}>
  {#if !source}
    <Notice icon={Flame} title="NO BOX CONNECTED">Phoenix runs on your box. Connect one from Settings, or switch to Demo to look around.</Notice>
  {:else if problem}
    <Notice icon={CircleAlert} title="PHOENIX COULD NOT BE READ">{problem}</Notice>
  {:else if !status}
    <p class="micro flex items-center gap-2 text-text-faint"><LoaderCircle class="h-3.5 w-3.5 animate-spin" />READING</p>
  {:else if !status.available}
    <Notice icon={Flame} title="PHOENIX IS NOT ON THIS BOX">{status.reason}</Notice>
  {:else if view === 'pc'}
    {#if scanProblem}
      <Notice icon={CircleAlert} title="THE SCAN DID NOT FINISH">{scanProblem}</Notice>
    {:else if !scan}
      <p class="micro flex items-center gap-2 text-text-faint" aria-live="polite"><LoaderCircle class="h-3.5 w-3.5 animate-spin" />LOOKING. NOTHING IS DELETED BY LOOKING. A FULL PC TAKES ABOUT 20 SECONDS.</p>
    {:else}
      {@render cleaner()}
    {/if}
  {:else if view === 'phone'}
    {#if !status.android && !store.isDemo}
      <Notice icon={Usb} title="THE BOX HAS NO ANDROID TOOLS YET">Start an Android machine once in Clones and Castle installs them, or put adb on the box's PATH.</Notice>
    {:else if scanProblem && !scan}
      <Notice icon={CircleAlert} title="NO PHONE TO CLEAN">{scanProblem}</Notice>
    {:else if !devices}
      <p class="micro flex items-center gap-2 text-text-faint"><LoaderCircle class="h-3.5 w-3.5 animate-spin" />LOOKING FOR A PHONE</p>
    {:else if !devices.length}
      <Notice icon={Usb} title="NO PHONE PLUGGED IN">
        Plug an Android phone into the box with a USB cable. On the phone, turn on Developer options, then USB debugging, and accept the question it asks.
        <button class="micro expo t-hover mx-auto mt-4 flex h-9 items-center gap-1.5 border border-hairline px-3 text-text-dim hover:border-white/30 hover:text-white" onclick={() => findPhones()}><RefreshCw class="h-3.5 w-3.5" />LOOK AGAIN</button>
      </Notice>
    {:else}
      <div class="mx-auto mb-4 flex max-w-4xl flex-wrap gap-2">
        {#each devices as device (device.serial)}
          <button
            class="micro expo t-hover flex h-10 items-center gap-2 border px-3 {serial === device.serial ? 'border-chiral/70 text-chiral' : 'border-hairline text-text-dim hover:border-white/30 hover:text-white'} disabled:opacity-40"
            disabled={device.state !== 'device' || scanning}
            onclick={() => { serial = device.serial; scan = null; runScan(source, 'android', device.serial); }}
          >
            <Smartphone class="h-3.5 w-3.5" />{(device.model || device.serial).toUpperCase()}{#if device.state !== 'device'} · UNLOCK IT AND ALLOW THIS COMPUTER{/if}
          </button>
        {/each}
        <button class="micro expo t-hover flex h-10 items-center gap-1.5 border border-hairline px-3 text-text-dim hover:border-white/30 hover:text-white" onclick={() => findPhones()}><RefreshCw class="h-3.5 w-3.5" />LOOK AGAIN</button>
      </div>
      {#if scan}
        {@render cleaner()}
      {:else if scanning}
        <p class="micro mx-auto flex max-w-4xl items-center gap-2 text-text-faint"><LoaderCircle class="h-3.5 w-3.5 animate-spin" />LOOKING THROUGH THE PHONE</p>
      {:else}
        <p class="micro mx-auto max-w-4xl text-text-faint">PICK A PHONE TO LOOK THROUGH IT.</p>
      {/if}
    {/if}
  {:else if !setup}
    <p class="micro flex items-center gap-2 text-text-faint"><LoaderCircle class="h-3.5 w-3.5 animate-spin" />READING</p>
  {:else}
    <div class="mx-auto max-w-4xl">
      <!-- ---------------- the app list ---------------- -->
      <section class="mb-6">
        <h2 class="micro text-white">APPS A FRESH PC GETS</h2>
        <p class="mt-1 max-w-2xl text-[13px] leading-relaxed text-text-dim">Each is installed with Chocolatey, at its newest version, the first time the new PC starts. Search finds anything in Chocolatey's community list.</p>

        <ul class="mt-3 flex flex-wrap gap-1.5">
          {#each apps as app (app)}
            <li class="flex items-center gap-1 border border-hairline bg-white/[0.03] py-1 pl-2.5 pr-1 font-mono text-[12px] text-white">
              {app}
              <button class="expo t-hover grid h-5 w-5 place-items-center text-text-faint hover:text-white" onclick={() => dropApp(app)} aria-label={`Remove ${app}`}><X class="h-3 w-3" /></button>
            </li>
          {:else}
            <li class="micro text-text-faint">NO APPS YET. SEARCH BELOW.</li>
          {/each}
        </ul>

        <label class="relative mt-3 block">
          {#if searching}<LoaderCircle class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 animate-spin text-text-faint" />{:else}<Search class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-faint" />{/if}
          <input type="search" bind:value={query} autocapitalize="off" autocomplete="off" spellcheck="false" placeholder="Search Chocolatey: powertoys, obs, steam..." class="h-11 w-full border border-hairline bg-white/[0.03] pl-9 pr-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search Chocolatey" />
        </label>
        {#if found}
          <ul class="mt-2 border border-hairline">
            {#each found as hit (hit.id)}
              <li class="flex items-center gap-3 border-b border-hairline-faint px-3 py-2 last:border-b-0">
                <span class="min-w-0 flex-1">
                  <span class="flex items-baseline gap-2">
                    <span class="truncate text-[14px] text-white">{hit.title || hit.id}</span>
                    <span class="shrink-0 font-mono text-[11.5px] text-text-faint">{hit.id}{hit.version ? ` ${hit.version}` : ''}</span>
                  </span>
                  {#if hit.summary}<span class="line-clamp-1 text-[12px] text-text-dim">{hit.summary}</span>{/if}
                </span>
                {#if hit.downloads}<span class="micro hidden shrink-0 text-text-faint sm:block">{Number(hit.downloads).toLocaleString()} INSTALLS</span>{/if}
                {#if hasApp(hit.id)}
                  <span class="micro flex h-8 shrink-0 items-center gap-1 px-2 text-live"><Check class="h-3.5 w-3.5" />ON THE LIST</span>
                {:else}
                  <button class="micro expo t-hover flex h-8 shrink-0 items-center gap-1 border border-hairline px-2.5 text-text-dim hover:border-chiral/60 hover:text-chiral" onclick={() => addApp(hit.id)}><Plus class="h-3.5 w-3.5" />ADD</button>
                {/if}
              </li>
            {:else}
              <li class="micro px-3 py-3 text-text-faint">NOTHING IN CHOCOLATEY BY THAT NAME.</li>
            {/each}
          </ul>
        {/if}

        <button class="micro expo t-hover mt-3 flex items-center gap-1.5 text-text-faint hover:text-white" onclick={() => (showCatalog = !showCatalog)} aria-expanded={showCatalog}>
          <ChevronDown class="h-3.5 w-3.5 transition-transform {showCatalog ? 'rotate-180' : ''}" />PHOENIX'S OWN LIST ({setup.catalog.length})
        </button>
        {#if showCatalog}
          {#each catalogGroups as [category, list] (category)}
            <p class="micro mt-3 text-text-faint">{category.toUpperCase()}</p>
            <ul class="mt-1 flex flex-wrap gap-1.5">
              {#each list as app (app.id)}
                {@const on = hasApp(app.id)}
                <li>
                  <button class="expo t-hover border px-2.5 py-1 text-[12px] {on ? 'border-chiral/60 bg-chiral-dim text-white' : 'border-hairline text-text-dim hover:border-white/30 hover:text-white'}" title={app.description} aria-pressed={on} onclick={() => (on ? dropApp(app.id) : addApp(app.id))}>{app.name}</button>
                </li>
              {/each}
            </ul>
          {/each}
        {/if}
      </section>

      <!-- ---------------- hardening ---------------- -->
      {#if setup.harden?.options?.length}
        <section class="mb-6 border border-hairline p-4">
          <label class="flex cursor-pointer items-start gap-3">
            <input type="checkbox" class="mt-0.5 h-4 w-4 shrink-0 accent-[#ff6a00]" bind:checked={harden.enabled} onchange={() => { dirty = true; showHarden = harden.enabled; }} />
            <span class="min-w-0">
              <span class="flex items-center gap-2 text-[14px] text-white"><ShieldCheck class="h-4 w-4 text-text-dim" />Also harden Windows</span>
              <span class="mt-1 block max-w-2xl text-[13px] leading-relaxed text-text-dim">{setup.harden.description}</span>
              <span class="micro mt-1.5 block text-text-faint">{setup.harden.name?.toUpperCase()} · PINNED TO {String(setup.harden.commit).slice(0, 10)} · NOT YET TRIED END TO END</span>
            </span>
          </label>
          {#if harden.enabled}
            <button class="micro expo t-hover mt-3 flex items-center gap-1.5 text-text-faint hover:text-white" onclick={() => (showHarden = !showHarden)} aria-expanded={showHarden}>
              <ChevronDown class="h-3.5 w-3.5 transition-transform {showHarden ? 'rotate-180' : ''}" />WHICH PARTS ({setup.harden.options.filter(hardenOn).length} OF {setup.harden.options.length})
            </button>
            {#if showHarden}
              {#each hardenGroups as [group, list] (group)}
                <p class="micro mt-3 text-text-faint">{group.toUpperCase()}</p>
                <ul class="mt-1">
                  {#each list as option (option.name)}
                    <li>
                      <label class="flex cursor-pointer items-start gap-3 py-1.5">
                        <input type="checkbox" class="mt-0.5 h-4 w-4 shrink-0 accent-[#ff6a00]" checked={hardenOn(option)} onchange={(e) => { harden.chosen[option.name] = e.currentTarget.checked; dirty = true; }} />
                        <span class="min-w-0">
                          <span class="flex flex-wrap items-center gap-2 text-[13.5px] text-white">{option.label}{@render riskTag(option.risk)}</span>
                          <span class="block text-[12px] leading-relaxed text-text-dim">{option.why}</span>
                        </span>
                      </label>
                    </li>
                  {/each}
                </ul>
              {/each}
            {/if}
          {/if}
        </section>
      {/if}

      <!-- ---------------- the two things made from the list ---------------- -->
      <section class="mb-6 grid gap-3 md:grid-cols-2">
        <div class="flex flex-col border border-hairline p-4">
          <h2 class="micro flex items-center gap-2 text-white"><FileCode2 class="h-4 w-4 text-text-dim" />A SCRIPT, FOR A PC THAT IS ALREADY RUNNING</h2>
          <p class="mt-2 flex-1 text-[13px] leading-relaxed text-text-dim">One PowerShell file. Right-click it on the PC and choose Run with PowerShell. It installs Chocolatey, then the apps above, and can be run again later.</p>
          <button class="micro expo t-hover mt-4 flex h-10 items-center justify-center gap-1.5 bg-chiral px-4 font-bold text-black hover:opacity-90 disabled:opacity-40" onclick={getScript} disabled={!!making || saving || (!apps.length && !harden.enabled)}>
            {#if making === 'script'}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />MAKING{:else}<Download class="h-3.5 w-3.5" />GET THE SCRIPT{/if}
          </button>
        </div>

        <form class="flex flex-col border border-hairline p-4" onsubmit={(e) => { e.preventDefault(); getAnswerFile(); }}>
          <h2 class="micro flex items-center gap-2 text-white"><Usb class="h-4 w-4 text-text-dim" />AN ANSWER FILE, FOR A PC BEING WIPED</h2>
          <p class="mt-2 text-[13px] leading-relaxed text-text-dim">Put autounattend.xml on a USB stick beside the Windows installer. Windows then installs with no questions, <strong class="font-medium text-white">erasing the first disk</strong>, makes these accounts, and runs the script above at the first logon.</p>
          <div class="mt-3 grid grid-cols-2 gap-2">
            <label class="col-span-2 block">
              <span class="micro text-text-faint">COMPUTER NAME</span>
              <input type="text" bind:value={names.computerName} oninput={() => (dirty = true)} maxlength="15" autocomplete="off" spellcheck="false" class="mt-1 h-9 w-full border border-hairline bg-white/[0.03] px-2 font-mono text-[13px] text-white focus:border-chiral/60 focus:outline-none" />
            </label>
            <label class="block">
              <span class="micro text-text-faint">ADMINISTRATOR</span>
              <input type="text" bind:value={names.adminName} oninput={() => (dirty = true)} maxlength="20" autocomplete="off" spellcheck="false" class="mt-1 h-9 w-full border border-hairline bg-white/[0.03] px-2 font-mono text-[13px] text-white focus:border-chiral/60 focus:outline-none" />
            </label>
            <label class="block">
              <span class="micro text-text-faint">ITS PASSWORD</span>
              <input type="password" bind:value={adminPassword} maxlength="127" autocomplete="new-password" class="mt-1 h-9 w-full border border-hairline bg-white/[0.03] px-2 text-[13px] text-white focus:border-chiral/60 focus:outline-none" />
            </label>
            <label class="block">
              <span class="micro text-text-faint">EVERYDAY ACCOUNT (OPTIONAL)</span>
              <input type="text" bind:value={names.userName} oninput={() => (dirty = true)} maxlength="20" autocomplete="off" spellcheck="false" class="mt-1 h-9 w-full border border-hairline bg-white/[0.03] px-2 font-mono text-[13px] text-white focus:border-chiral/60 focus:outline-none" />
            </label>
            <label class="block">
              <span class="micro text-text-faint">ITS PASSWORD</span>
              <input type="password" bind:value={userPassword} maxlength="127" autocomplete="new-password" disabled={!names.userName.trim()} class="mt-1 h-9 w-full border border-hairline bg-white/[0.03] px-2 text-[13px] text-white focus:border-chiral/60 focus:outline-none disabled:opacity-40" />
            </label>
          </div>
          <p class="micro mt-3 text-text-faint">THE PASSWORDS GO INTO THE FILE AND NOWHERE ELSE. CASTLE DOES NOT KEEP THEM.</p>
          <button type="submit" class="micro expo t-hover mt-3 flex h-10 items-center justify-center gap-1.5 border border-chiral/70 px-4 font-bold text-chiral hover:bg-chiral hover:text-black disabled:opacity-40" disabled={!!making || saving || !canChange || !adminPassword || !names.adminName.trim() || !names.computerName.trim()}>
            {#if making === 'answer'}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />MAKING{:else}<Download class="h-3.5 w-3.5" />MAKE THE ANSWER FILE{/if}
          </button>
          {#if !canChange}<p class="micro mt-2 text-bridge">ONLY IN CASTLE ON THE COMPUTER THE BOX RUNS ON, SO PASSWORDS NEVER CROSS THE NETWORK.</p>{/if}
        </form>
      </section>

      <div class="flex items-center gap-3">
        <button class="micro expo t-hover flex h-9 items-center gap-1.5 border border-hairline px-3 text-text-dim hover:border-white/30 hover:text-white disabled:opacity-40" onclick={save} disabled={!dirty || saving || !canChange}>
          {#if saving}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />SAVING{:else}<Check class="h-3.5 w-3.5" />{dirty ? 'SAVE THE LIST' : 'SAVED'}{/if}
        </button>
        <p class="micro text-text-faint">THE LIST IS SHARED WITH THE PHOENIX APP ON THIS COMPUTER.</p>
      </div>
    </div>
  {/if}
</SectionFrame>
