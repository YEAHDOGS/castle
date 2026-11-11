<!-- ClonesSection: virtual machines and the automations that run in them. An automation is
     a one-click recipe: save its login here, press Start, and the box boots its VM, fetches
     the project and sets it running. -->
<script>
  import { untrack } from 'svelte';
  import { Boxes, Bot, Monitor, Play, Square, Save, Coins, Swords, Clock, Cpu, Check, Power } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import { prefs } from '../../lib/prefs.svelte.js';
  import { AUTOMATION_SKILLS, PLANNED_AUTOMATIONS, DEMO_MACHINES } from '../../lib/demo-shell.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';
  import VmScreen from '../VmScreen.svelte';

  const SECOND_MS = 1000;
  const NO_CLONES = 'not started with --clonewars';
  const ADMIN = 'Admin';
  const HOUR_MS = 3600000;
  const MINUTE_MS = 60000;
  const VIEWS = [
    { id: 'automations', label: 'Automations', icon: Bot },
    { id: 'machines', label: 'Machines', icon: Monitor },
  ];
  const STATE_LABEL = { stopped: 'STOPPED', launching: 'STARTING', running: 'RUNNING', failed: 'FAILED', 'not built': 'NOT BUILT' };
  const STATE_TONE = { stopped: 'text-text-faint', launching: 'text-chiral', running: 'text-live', failed: 'text-chiral', 'not built': 'text-text-faint' };
  const SKILL_ICONS = { 'cart-surfer': Coins, 'card-jitsu': Swords };

  let view = $state('automations');
  let automations = $state([]);
  let problem = $state('');
  let loaded = $state(false);
  let busy = $state(''); // id of the automation with a request in flight
  let forms = $state({}); // id -> { username, password, dirty }
  let logEl = $state(null);

  const source = $derived(shell.clonesSource);
  const allowed = $derived(shell.user.role === ADMIN);

  /** Copies the saved account name into the form unless the person is typing over it. */
  function syncForms(list) {
    for (const a of list) {
      const form = forms[a.id];
      if (!form) forms[a.id] = { username: a.username, password: '', dirty: false };
      else if (!form.dirty) form.username = a.username;
    }
  }

  let turningOn = $state(false);

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
      const list = await from.clones();
      automations = list;
      syncForms(list);
      problem = '';
    } catch (err) {
      problem = err?.message || 'Clones could not be read';
    }
    loaded = true;
  }

  /** Runs one box call for an automation, then shows the state it returned. */
  async function act(id, call, done) {
    busy = id;
    try {
      const updated = await call();
      automations = automations.map((a) => (a.id === id ? updated : a));
      if (done) store.toast(done);
      return true;
    } catch (err) {
      store.toast(err?.message || 'That did not work');
      return false;
    } finally {
      busy = '';
    }
  }

  async function saveLogin(a) {
    const form = forms[a.id];
    const ok = await act(a.id, () => source.cloneLogin(a.id, form.username, form.password), 'Login saved');
    if (!ok) return;
    form.password = '';
    form.dirty = false;
  }

  function uptime(ms) {
    if (!ms) return '';
    return `${Math.floor(ms / HOUR_MS)}h ${Math.floor((ms % HOUR_MS) / MINUTE_MS)}m`;
  }

  $effect(() => {
    const from = source;
    untrack(() => {
      loaded = false;
      forms = {};
      refresh(from);
    });
  });

  // Keep the newest log line in view while a launch is scrolling past.
  $effect(() => {
    automations;
    if (logEl) logEl.scrollTop = logEl.scrollHeight;
  });

  // Re-read the launch state and log on the interval chosen in Settings > Clones.
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
      This box was started without its <span class="text-white">clonewars</span> folder, so it cannot run automations yet. Turn it
      on here and the box finds the folder itself. No restart needed.
      <button
        disabled={turningOn}
        onclick={turnOn}
        class="expo t-hover hud-corners micro mx-auto mt-4 flex h-11 items-center gap-2 bg-chiral px-5 font-bold text-black transition-all hover:brightness-110 active:scale-[0.98] disabled:opacity-50"
      >
        <Power class="h-4 w-4" strokeWidth={2} /> {turningOn ? 'TURNING ON' : 'TURN ON CLONES'}
      </button>
    </Notice>
  {:else if problem}
    <Notice icon={Boxes} title="CLONES UNAVAILABLE">{problem}</Notice>
  {:else if view === 'automations'}
    <div class="grid gap-4 xl:grid-cols-2">
      {#each automations as a (a.id)}
        {@const form = forms[a.id]}
        {@const live = a.state === 'running' || a.state === 'launching'}
        {@const ready = !!a.username && a.hasPassword}
        <article class="hud-corners glass flex flex-col xl:col-span-2">
          <header class="flex flex-wrap items-center gap-3 border-b border-hairline px-5 py-4">
            <span class="grid h-10 w-10 shrink-0 place-items-center border border-chiral/60 bg-chiral-dim">
              <Bot class="h-5 w-5 text-chiral" strokeWidth={1.5} />
            </span>
            <div class="min-w-0 flex-1">
              <h2 class="text-[17px] font-bold text-white">{a.name}</h2>
              <p class="text-[13px] text-text-dim">{a.description}</p>
            </div>
            <p class="micro flex items-center gap-2 {STATE_TONE[a.state]}" role="status">
              <span class="h-2 w-2 rounded-full bg-current {a.state === 'launching' ? 'animate-pulse' : ''}"></span>
              {STATE_LABEL[a.state] || a.state}
            </p>
          </header>

          <div class="grid gap-5 p-5 lg:grid-cols-[minmax(0,20rem)_minmax(0,1fr)]">
            <div class="space-y-4">
              <!-- login -->
              {#if form}
                <form class="space-y-3" onsubmit={(e) => { e.preventDefault(); saveLogin(a); }}>
                  <p class="micro text-text-faint">ACCOUNT IT SIGNS IN WITH</p>
                  <label class="block">
                    <span class="micro mb-1 block text-text-faint">PENGUIN NAME</span>
                    <input
                      bind:value={form.username}
                      oninput={() => (form.dirty = true)}
                      type="text"
                      autocomplete="off"
                      spellcheck="false"
                      class="h-10 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white focus:border-chiral/60 focus:outline-none"
                    />
                  </label>
                  <label class="block">
                    <span class="micro mb-1 block text-text-faint">PASSWORD</span>
                    <input
                      bind:value={form.password}
                      oninput={() => (form.dirty = true)}
                      type="password"
                      autocomplete="new-password"
                      placeholder={a.hasPassword ? 'saved. type to replace it' : 'not saved yet'}
                      class="h-10 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none"
                    />
                  </label>
                  <button
                    type="submit"
                    disabled={busy === a.id || !form.dirty || !form.username.trim()}
                    class="expo t-hover micro flex h-10 w-full items-center justify-center gap-2 border border-hairline font-bold text-white transition-colors hover:border-chiral/60 disabled:opacity-40"
                  >
                    <Save class="h-3.5 w-3.5" strokeWidth={1.75} /> SAVE LOGIN
                  </button>
                  <p class="text-[12px] leading-relaxed text-text-dim">
                    Kept on the box, in the project's own settings file. The password is never sent back to this screen.
                  </p>
                </form>
              {/if}

              <!-- start / stop -->
              {#if live}
                <button
                  disabled={busy === a.id}
                  onclick={() => act(a.id, () => source.cloneStop(a.id), 'Shutting the VM down')}
                  class="expo t-hover micro flex h-12 w-full items-center justify-center gap-2 border border-chiral/70 font-bold text-chiral transition-colors hover:bg-chiral-dim disabled:opacity-40"
                >
                  <Square class="h-3.5 w-3.5 fill-current" strokeWidth={2} /> STOP
                </button>
              {:else}
                <button
                  disabled={busy === a.id || !ready}
                  onclick={() => act(a.id, () => source.cloneStart(a.id), 'Starting the VM')}
                  class="expo t-hover hud-corners micro flex h-12 w-full items-center justify-center gap-2 bg-chiral font-bold text-black transition-all hover:brightness-110 active:scale-[0.98] disabled:opacity-40"
                >
                  <Play class="h-4 w-4 fill-black" strokeWidth={2} /> START
                </button>
                {#if !ready}
                  <p class="text-[12px] text-text-dim">Save the penguin name and password to enable Start.</p>
                {/if}
              {/if}
              {#if !a.repoPresent}
                <p class="text-[12px] text-text-dim">The project is not next to Castle on the box yet. Clone it there, or give the box its git address, before pressing Start.</p>
              {/if}
            </div>

            <div class="flex min-w-0 flex-col gap-4">
              <!-- skills -->
              <div>
                <p class="micro mb-2 text-text-faint">SKILLS</p>
                <ul class="grid gap-2 sm:grid-cols-2">
                  {#each AUTOMATION_SKILLS[a.id] || [] as skill (skill.id)}
                    {@const SkillIcon = SKILL_ICONS[skill.id] || Bot}
                    <li class="flex items-center gap-3 border px-3 py-2.5 {skill.ready ? 'border-chiral/50 bg-chiral-dim/40' : 'border-hairline opacity-60'}">
                      <SkillIcon class="h-5 w-5 shrink-0 {skill.ready ? 'text-chiral' : 'text-text-faint'}" strokeWidth={1.5} />
                      <span class="min-w-0 flex-1">
                        <span class="block text-[13px] font-semibold text-white">{skill.name}</span>
                        <span class="block truncate text-[12px] text-text-dim">{skill.detail}</span>
                      </span>
                      {#if skill.ready}
                        <Check class="h-4 w-4 text-chiral" strokeWidth={2} />
                      {:else}
                        <span class="micro text-text-faint">PLANNED</span>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </div>

              <!-- launch log -->
              <div class="flex min-h-0 flex-1 flex-col">
                <p class="micro mb-2 text-text-faint">WHAT THE BOX DID</p>
                <pre
                  bind:this={logEl}
                  class="h-56 overflow-auto border border-hairline bg-black/60 p-3 font-mono text-[11px] leading-relaxed text-text-dim"
                  aria-live="polite">{a.log.length ? a.log.join('\n') : a.state === 'running' ? 'Running. It was started outside this screen, so there is no launch log here.' : 'Nothing yet. Press Start and the launch appears here line by line.'}</pre>
              </div>
            </div>
            {#if a.state === 'running' && source.cloneScreen}
              <!-- full width of the card, under the login and the log -->
              <div class="lg:col-span-2">
                <VmScreen {source} id={a.id} />
              </div>
            {/if}
          </div>
        </article>
      {/each}

      {#each PLANNED_AUTOMATIONS as planned (planned.id)}
        <article class="border border-dashed border-hairline p-5 opacity-70">
          <div class="flex items-center gap-3">
            <Bot class="h-5 w-5 text-text-faint" strokeWidth={1.5} />
            <h2 class="flex-1 text-[15px] font-bold text-white">{planned.name}</h2>
            <span class="micro text-text-faint">PLANNED</span>
          </div>
          <p class="mt-2 text-[13px] text-text-dim">{planned.description}</p>
          <p class="micro mt-3 text-text-faint">{planned.skills.join(' · ')}</p>
        </article>
      {/each}
    </div>
  {:else if store.isDemo}
    <ul class="grid gap-3 lg:grid-cols-2">
      {#each DEMO_MACHINES as machine (machine.id)}
        <li class="glass flex items-center gap-4 p-4">
          <Monitor class="h-7 w-7 shrink-0 {machine.state === 'running' ? 'text-live' : 'text-text-faint'}" strokeWidth={1.25} />
          <div class="min-w-0 flex-1">
            <p class="text-[15px] font-semibold text-white">{machine.name}</p>
            <p class="truncate text-[12px] text-text-dim">{machine.detail}</p>
            <p class="micro mt-1.5 flex flex-wrap gap-x-3 gap-y-1 text-text-faint">
              <span class="flex items-center gap-1"><Cpu class="h-3 w-3" /> {machine.cores} CORES · {machine.memoryGb} GB</span>
              {#if machine.diskBytes}<span>{fmtSize(machine.diskBytes)} DISK</span>{/if}
              {#if machine.uptime}<span class="flex items-center gap-1"><Clock class="h-3 w-3" /> {uptime(machine.uptime)}</span>{/if}
            </p>
          </div>
          <p class="micro {STATE_TONE[machine.state]}">{STATE_LABEL[machine.state]}</p>
        </li>
      {/each}
    </ul>
    <p class="mt-4 text-[12px] text-text-dim">Sample machines. Start, stop and screen controls for each one are not built yet.</p>
  {:else}
    <Notice icon={Monitor} title="MACHINES COME NEXT">
      This box can run automations today. Starting and stopping any Clonewars machine from here is the next step; the
      <a class="text-bridge underline" href="/demo">demo</a> shows the planned list.
    </Notice>
  {/if}
</SectionFrame>
