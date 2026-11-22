<!-- AutomationEditor: makes an automation, or changes one. An automation is a recipe: the
     system it runs on (in a virtual machine, or in a container, which is far lighter), what it
     does, the rules it runs under, and the sign-in fields each of its bots fills in. What it
     does is either a Castle script, written here (Castle compiles it and brings the castle
     library), or a project of its own with tasks to run in order. On Android the tasks are
     commands for the phone's own shell, and the project's APKs are installed. -->
<script>
  import { X, Plus, Trash2, Save, Sparkles } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { INPUT, BTN, BTN_PRIMARY, LABEL, words } from '../../lib/clones-ui.js';
  import ScriptEditor from './ScriptEditor.svelte';

  /** initial: the automation being changed, or null for a new one. targets: the catalogue.
      examples: the built-in automations, to start a new one from. check: the box's script check. */
  /** draft: { name, script, fields } for a new automation started from a script in the guide. */
  let { initial, draft = null, targets, examples = [], busy, check, onsave, onclose } = $props();

  const KINDS = [
    { id: 'script', label: 'CASTLE SCRIPT', help: 'Written here, step by step. Castle turns it into a Python project, brings the castle library, and keeps what it finds in its database.' },
    { id: 'project', label: 'A PROJECT OF ITS OWN', help: 'A folder or a git repository with its own code, and the commands that start it.' },
  ];
  const BLANK_SCRIPT = 'castle 1\nwebsite example.com\n\nelement heading = h1\n\nread heading as title\nsay "The page says {title}"\npost title to castle\n';

  const automatable = $derived(targets.filter((t) => t.automatable ?? t.inject));
  const isContainer = (t) => t?.runtime === 'container';
  const ENGINES = [
    { id: 'vm', label: 'VIRTUAL MACHINE', help: 'A whole computer: its own kernel and desktop, a fixed share of memory, a window of its own.' },
    { id: 'container', label: 'CONTAINER · LIGHTER', help: 'Shares the box\'s kernel and starts only what the bot needs: up in seconds, using only the memory its programs use. Needs Docker.' },
  ];
  const EXAMPLE = {
    name: 'My AutoSurf',
    description: 'Signs in and plays on a loop. A copy of the Penguin AutoSurf recipe to change.',
    target: 'debian-x11',
    source: '../../penguin-autosurf',
    memoryGb: 4,
    cores: 8,
    screen: '1440x900',
    packages: 'python3-venv firefox-esr',
    tasks: [{ name: 'Run the bot', command: 'bash ./start.sh', root: false }],
    fields: [
      { env: 'PENGUIN_USERNAME', label: 'Username', secret: false },
      { env: 'PENGUIN_PASSWORD', label: 'Password', secret: true },
    ],
    restart: true,
    stopAfterMinutes: 0,
    temporary: false,
    kind: 'project',
    script: '',
    daily: false,
    dailyAt: '08:00',
  };
  const BLANK = { ...EXAMPLE, name: '', description: '', source: '', target: 'container-debian', cores: 2, packages: '', tasks: [{ name: '', command: '', root: false }], fields: [], restart: false, kind: 'script', script: BLANK_SCRIPT };
  const walmart = $derived(examples.find((a) => a.kind === 'script'));

  /** A new automation that starts as a copy of the Walmart example's script. */
  function fromWalmart() {
    if (!walmart) return;
    form = { ...structuredClone(BLANK), name: 'My price checker', description: 'A copy of the Walmart motor oil script to change.', target: walmart.target, memoryGb: walmart.memoryGb, cores: walmart.cores, stopAfterMinutes: walmart.stopAfterMinutes, script: walmart.script };
  }

  function formOf(a) {
    if (!a) return structuredClone(BLANK);
    return {
      name: a.name, description: a.description, target: a.target, source: a.source, memoryGb: a.memoryGb, cores: a.cores, screen: a.screen,
      packages: [...new Set([...a.apt, ...a.pacman])].join(' '),
      tasks: a.tasks.map((t) => ({ ...t })), fields: a.fields.map((f) => ({ ...f })),
      restart: a.restart, stopAfterMinutes: a.stopAfterMinutes, temporary: a.temporary,
      kind: a.kind || 'project', script: a.script || '', daily: !!a.schedule, dailyAt: a.schedule || '08:00',
    };
  }

  let form = $state(untrack(() => (draft && !initial ? { ...structuredClone(BLANK), name: draft.name, script: draft.script, fields: (draft.fields || []).map((f) => ({ ...f })) } : formOf(initial))));

  const current = $derived(targets.find((t) => t.id === form.target));
  const engine = $derived(isContainer(current) ? 'container' : 'vm');
  const choices = $derived(automatable.filter((t) => isContainer(t) === (engine === 'container')));
  const android = $derived(current?.family === 'android');
  const scripted = $derived(form.kind === 'script');
  /** A built-in keeps its kind; anything else can switch. Scripts run on Linux only. */
  const canSwitch = $derived(!initial?.builtin);
  const choicesHere = $derived(scripted ? choices.filter((t) => t.family !== 'android') : choices);

  /** Moves the automation to the other engine, on the same kind of system where there is one. */
  function setEngine(id) {
    if (id === engine) return;
    const family = current?.family || 'linux';
    const pool = automatable.filter((t) => isContainer(t) === (id === 'container') && (!scripted || t.family !== 'android'));
    const next = pool.find((t) => t.family === family) || pool[0];
    if (next) form.target = next.id;
  }

  /** Script or project. A script needs a Linux system, so an Android pick moves to Linux. */
  function pickKind(id) {
    form.kind = id;
    if (id !== 'script') return;
    if (!form.script.trim()) form.script = BLANK_SCRIPT;
    if (android) {
      const linux = automatable.find((t) => t.family === 'linux' && isContainer(t) === (engine === 'container')) || automatable.find((t) => t.family === 'linux');
      if (linux) form.target = linux.id;
    }
  }

  function save() {
    const pkgs = words(form.packages);
    onsave({
      id: initial?.id || '',
      name: form.name,
      description: form.description,
      target: form.target,
      source: form.source,
      memoryGb: Number(form.memoryGb) || 0,
      cores: Number(form.cores) || 0,
      screen: form.screen,
      apt: pkgs,
      pacman: pkgs,
      script: scripted ? form.script : '',
      schedule: form.daily ? form.dailyAt : '',
      tasks: scripted ? [] : form.tasks.filter((t) => t.command.trim()),
      fields: form.fields.filter((f) => f.env.trim()),
      restart: form.restart,
      stopAfterMinutes: Number(form.stopAfterMinutes) || 0,
      temporary: form.temporary,
    });
  }
</script>

<svelte:document onkeydown={(e) => { if (e.key === 'Escape' && !e.defaultPrevented) { e.preventDefault(); onclose(); } }} />

<div class="fixed inset-0 z-50 grid place-items-center bg-black/75 p-4" role="dialog" aria-modal="true" aria-label="Automation">
  <form class="hud-corners max-h-[92dvh] w-full max-w-3xl space-y-5 overflow-auto bg-panel-solid p-5" onsubmit={(e) => { e.preventDefault(); save(); }}>
    <header class="flex items-center gap-3">
      <h2 class="flex-1 text-[18px] font-bold text-white">{initial ? `Change ${initial.name}` : 'New automation'}</h2>
      {#if !initial}
        {#if walmart}<button type="button" class={BTN} onclick={fromWalmart}><Sparkles class="h-3.5 w-3.5" /> WALMART EXAMPLE</button>{/if}
        <button type="button" class={BTN} onclick={() => (form = structuredClone(EXAMPLE))}><Sparkles class="h-3.5 w-3.5" /> PENGUIN EXAMPLE</button>
      {/if}
      <button type="button" class={BTN} onclick={onclose} aria-label="Close"><X class="h-4 w-4" /></button>
    </header>

    <div class="grid gap-3 sm:grid-cols-2">
      <label class="block"><span class={LABEL}>NAME</span><input bind:value={form.name} spellcheck="false" class={INPUT} /></label>
      <label class="block">
        <span class={LABEL}>RUNS ON</span>
        <select bind:value={form.target} class={INPUT}>
          {#each choicesHere as t (t.id)}<option value={t.id} class="bg-black">{t.name}</option>{/each}
        </select>
      </label>
      <label class="block sm:col-span-2"><span class={LABEL}>WHAT IT DOES</span><input bind:value={form.description} class={INPUT} /></label>
    </div>

    {#if canSwitch}
      <div>
        <p class={LABEL}>WHAT IT RUNS</p>
        <div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label="What it runs">
          {#each KINDS as k (k.id)}
            <button type="button" role="radio" aria-checked={form.kind === k.id} class="{BTN} {form.kind === k.id ? '!border-chiral/70 !text-chiral' : ''}" onclick={() => pickKind(k.id)}>{k.label}</button>
          {/each}
        </div>
        <p class="mt-1 text-[12px] leading-relaxed text-text-dim">{KINDS.find((k) => k.id === form.kind)?.help}</p>
      </div>
    {/if}

    <div>
      <p class={LABEL}>RUNS IN</p>
      <div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Runs in">
        {#each ENGINES as e (e.id)}
          <button
            type="button"
            role="radio"
            aria-checked={engine === e.id}
            disabled={!automatable.some((t) => isContainer(t) === (e.id === 'container'))}
            class="{BTN} {engine === e.id ? '!border-chiral/70 !text-chiral' : ''}"
            onclick={() => setEngine(e.id)}
          >{e.label}</button>
        {/each}
      </div>
      <p class="mt-1 text-[12px] leading-relaxed text-text-dim">{ENGINES.find((e) => e.id === engine)?.help}</p>
    </div>

    {#if scripted}
      <ScriptEditor bind:value={form.script} {check} />
    {:else}
    <div>
      <label class="block">
        <span class={LABEL}>PROJECT IT BRINGS</span>
        <input bind:value={form.source} spellcheck="false" placeholder="https://github.com/you/your-bot   or   C:\Projects\your-bot" class={INPUT} />
      </label>
      <p class="mt-1 text-[12px] leading-relaxed text-text-dim">
        A folder on this box, or an https address on GitHub, GitLab, Codeberg or Bitbucket. Other addresses are refused.
        {#if android}Every .apk in it is installed on the phone and opened; its other files go to Download/castle-copy, where the tasks start.{:else}The project is unpacked in the machine and the tasks run inside it.{/if}
      </p>
    </div>

    {/if}

    {#if !android}
      <label class="block">
        <span class={LABEL}>{scripted ? 'MORE PACKAGES TO INSTALL (A WEBSITE SCRIPT ALREADY GETS PYTHON AND CHROMIUM)' : 'PACKAGES TO INSTALL'}</span>
        <input bind:value={form.packages} spellcheck="false" placeholder={scripted ? 'none needed' : 'python3-venv firefox-esr'} class={INPUT} />
      </label>
    {/if}

    {#if !scripted}

    <div>
      <p class={LABEL}>{android ? 'TASKS ON THE PHONE, RUN IN THIS ORDER' : 'TASKS, RUN IN THIS ORDER'}</p>
      <ul class="space-y-2">
        {#each form.tasks as task, i (i)}
          <li class="grid grid-cols-[minmax(0,10rem)_minmax(0,1fr)_auto_auto] items-center gap-2">
            <input bind:value={task.name} placeholder="what it is" class={INPUT} />
            <input bind:value={task.command} spellcheck="false" placeholder={android ? 'am start -n com.example.app/.MainActivity' : 'python main.py'} class="{INPUT} font-mono text-[12px]" />
            <label class="micro flex items-center gap-1.5 text-text-faint" title={android ? 'Run as root (su), on images that allow it' : 'Run as root before the desktop starts, instead of as the user inside it'}>
              <input type="checkbox" bind:checked={task.root} class="accent-chiral" /> ROOT
            </label>
            <button type="button" class={BTN} aria-label="Remove task" onclick={() => form.tasks.splice(i, 1)}><Trash2 class="h-3.5 w-3.5" /></button>
          </li>
        {/each}
      </ul>
      <button type="button" class="{BTN} mt-2" onclick={() => form.tasks.push({ name: '', command: '', root: false })}><Plus class="h-3.5 w-3.5" /> ADD A TASK</button>
      {#if android}
        <p class="mt-1 text-[12px] leading-relaxed text-text-dim">
          Commands for the phone's own shell, as with adb shell: <span class="font-mono">am start</span>, <span class="font-mono">input tap 540 1200</span>, <span class="font-mono">input text hello</span>, <span class="font-mono">sleep 5</span>. ROOT runs one with su, which the container's Android image allows and the Play Store emulator does not.
        </p>
      {/if}
      {#if initial?.builtin}
        <p class="mt-1 text-[12px] text-text-dim">The built-in example has its own setup; tasks here run after it.</p>
      {/if}
    </div>
    {/if}

    <div>
      <p class={LABEL}>RULES</p>
      <div class="space-y-2 text-[13px] text-text-dim">
        <label class="flex flex-wrap items-center gap-2">
          <input type="checkbox" bind:checked={form.daily} class="accent-chiral" /> Run every day at
          <input bind:value={form.dailyAt} type="time" disabled={!form.daily} class="{INPUT} !w-32" aria-label="Time of day" />
          <span class="text-[12px]">on the box's clock</span>
        </label>
        <label class="flex items-center gap-2"><input type="checkbox" bind:checked={form.restart} class="accent-chiral" /> {scripted ? 'When the script finishes, run it again (otherwise its machine stops when it is done)' : 'When the tasks finish, run them again'}</label>
        <label class="flex items-center gap-2"><input type="checkbox" bind:checked={form.temporary} class="accent-chiral" /> Temporary: every run starts clean and nothing it writes is kept</label>
        <label class="flex items-center gap-2">
          Shut the machine down after
          <input bind:value={form.stopAfterMinutes} type="number" min="0" max="100000" class="{INPUT} !w-24" /> minutes (0 means never)
        </label>
      </div>
    </div>

    <div>
      <p class={LABEL}>WHAT EACH BOT FILLS IN</p>
      <ul class="space-y-2">
        {#each form.fields as field, i (i)}
          <li class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto_auto] items-center gap-2">
            <input bind:value={field.env} spellcheck="false" placeholder="API_KEY" class="{INPUT} font-mono text-[12px]" />
            <input bind:value={field.label} placeholder="what to call it" class={INPUT} />
            <label class="micro flex items-center gap-1.5 text-text-faint"><input type="checkbox" bind:checked={field.secret} class="accent-chiral" /> SECRET</label>
            <button type="button" class={BTN} aria-label="Remove field" onclick={() => form.fields.splice(i, 1)}><Trash2 class="h-3.5 w-3.5" /></button>
          </li>
        {/each}
      </ul>
      <button type="button" class="{BTN} mt-2" onclick={() => form.fields.push({ env: '', label: '', secret: false })}><Plus class="h-3.5 w-3.5" /> ADD A FIELD</button>
      <p class="mt-1 text-[12px] leading-relaxed text-text-dim">
        {#if android}Each field becomes a variable the phone's tasks can read ($API_KEY).{:else}Each field becomes a line of the project's .env and a variable the tasks can read.{/if} A bot that leaves one empty uses the project's own .env.
      </p>
    </div>

    <div class="grid grid-cols-3 gap-3">
      <label class="block"><span class={LABEL}>MEMORY (GB)</span><input bind:value={form.memoryGb} type="number" min="1" max="256" class={INPUT} /></label>
      <label class="block"><span class={LABEL}>CORES</span><input bind:value={form.cores} type="number" min="1" max="128" class={INPUT} /></label>
      {#if !android}
        <label class="block"><span class={LABEL}>SCREEN</span><input bind:value={form.screen} spellcheck="false" placeholder="1440x900" class={INPUT} /></label>
      {/if}
    </div>

    <button type="submit" disabled={busy || !form.name.trim()} class="{BTN_PRIMARY} w-full"><Save class="h-4 w-4" /> {initial ? 'SAVE CHANGES' : 'CREATE AUTOMATION'}</button>
  </form>
</div>
