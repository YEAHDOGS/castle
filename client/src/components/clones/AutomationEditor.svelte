<!-- AutomationEditor: makes an automation, or changes one. An automation is a recipe: the
     system it runs on, the project it brings, what to install, the tasks to run in order,
     the rules it runs under, and the sign-in fields each of its bots fills in. -->
<script>
  import { X, Plus, Trash2, Save, Sparkles } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { INPUT, BTN, BTN_PRIMARY, LABEL, words } from '../../lib/clones-ui.js';

  /** initial: the automation being changed, or null for a new one. targets: the catalogue. */
  let { initial, targets, busy, onsave, onclose } = $props();

  const scriptable = $derived(targets.filter((t) => t.inject));
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
  };
  const BLANK = { ...EXAMPLE, name: '', description: '', source: '', cores: 4, packages: '', tasks: [{ name: '', command: '', root: false }], fields: [], restart: false };

  function formOf(a) {
    if (!a) return structuredClone(BLANK);
    return {
      name: a.name, description: a.description, target: a.target, source: a.source, memoryGb: a.memoryGb, cores: a.cores, screen: a.screen,
      packages: [...new Set([...a.apt, ...a.pacman])].join(' '),
      tasks: a.tasks.map((t) => ({ ...t })), fields: a.fields.map((f) => ({ ...f })),
      restart: a.restart, stopAfterMinutes: a.stopAfterMinutes, temporary: a.temporary,
    };
  }

  let form = $state(untrack(() => formOf(initial)));

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
      tasks: form.tasks.filter((t) => t.command.trim()),
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
        <button type="button" class={BTN} onclick={() => (form = structuredClone(EXAMPLE))}><Sparkles class="h-3.5 w-3.5" /> START FROM THE PENGUIN EXAMPLE</button>
      {/if}
      <button type="button" class={BTN} onclick={onclose} aria-label="Close"><X class="h-4 w-4" /></button>
    </header>

    <div class="grid gap-3 sm:grid-cols-2">
      <label class="block"><span class={LABEL}>NAME</span><input bind:value={form.name} spellcheck="false" class={INPUT} /></label>
      <label class="block">
        <span class={LABEL}>RUNS ON</span>
        <select bind:value={form.target} class={INPUT}>
          {#each scriptable as t (t.id)}<option value={t.id} class="bg-black">{t.name}</option>{/each}
        </select>
      </label>
      <label class="block sm:col-span-2"><span class={LABEL}>WHAT IT DOES</span><input bind:value={form.description} class={INPUT} /></label>
    </div>

    <div>
      <label class="block">
        <span class={LABEL}>PROJECT IT BRINGS</span>
        <input bind:value={form.source} spellcheck="false" placeholder="https://github.com/you/your-bot   or   C:\Projects\your-bot" class={INPUT} />
      </label>
      <p class="mt-1 text-[12px] leading-relaxed text-text-dim">
        A folder on this box, or an https address on GitHub, GitLab, Codeberg or Bitbucket. Other addresses are refused. The project is unpacked in the machine and the tasks run inside it.
      </p>
    </div>

    <label class="block">
      <span class={LABEL}>PACKAGES TO INSTALL</span>
      <input bind:value={form.packages} spellcheck="false" placeholder="python3-venv firefox-esr" class={INPUT} />
    </label>

    <div>
      <p class={LABEL}>TASKS, RUN IN THIS ORDER</p>
      <ul class="space-y-2">
        {#each form.tasks as task, i (i)}
          <li class="grid grid-cols-[minmax(0,10rem)_minmax(0,1fr)_auto_auto] items-center gap-2">
            <input bind:value={task.name} placeholder="what it is" class={INPUT} />
            <input bind:value={task.command} spellcheck="false" placeholder="python main.py" class="{INPUT} font-mono text-[12px]" />
            <label class="micro flex items-center gap-1.5 text-text-faint" title="Run as root before the desktop starts, instead of as the user inside it">
              <input type="checkbox" bind:checked={task.root} class="accent-chiral" /> ROOT
            </label>
            <button type="button" class={BTN} aria-label="Remove task" onclick={() => form.tasks.splice(i, 1)}><Trash2 class="h-3.5 w-3.5" /></button>
          </li>
        {/each}
      </ul>
      <button type="button" class="{BTN} mt-2" onclick={() => form.tasks.push({ name: '', command: '', root: false })}><Plus class="h-3.5 w-3.5" /> ADD A TASK</button>
      {#if initial?.builtin}
        <p class="mt-1 text-[12px] text-text-dim">The built-in example has its own setup; tasks here run after it.</p>
      {/if}
    </div>

    <div>
      <p class={LABEL}>RULES</p>
      <div class="space-y-2 text-[13px] text-text-dim">
        <label class="flex items-center gap-2"><input type="checkbox" bind:checked={form.restart} class="accent-chiral" /> When the tasks finish, run them again</label>
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
        Each field becomes a line of the project's .env and a variable the tasks can read. A bot that leaves one empty uses the project's own .env.
      </p>
    </div>

    <div class="grid grid-cols-3 gap-3">
      <label class="block"><span class={LABEL}>MEMORY (GB)</span><input bind:value={form.memoryGb} type="number" min="1" max="256" class={INPUT} /></label>
      <label class="block"><span class={LABEL}>CORES</span><input bind:value={form.cores} type="number" min="1" max="128" class={INPUT} /></label>
      <label class="block"><span class={LABEL}>SCREEN</span><input bind:value={form.screen} spellcheck="false" placeholder="1440x900" class={INPUT} /></label>
    </div>

    <button type="submit" disabled={busy || !form.name.trim()} class="{BTN_PRIMARY} w-full"><Save class="h-4 w-4" /> {initial ? 'SAVE CHANGES' : 'CREATE AUTOMATION'}</button>
  </form>
</div>
