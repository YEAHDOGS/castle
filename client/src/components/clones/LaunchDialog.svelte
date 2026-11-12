<!-- LaunchDialog: what happens after an operating system's tile is clicked. Pick a disk that
     exists and it starts at once; or make a new machine that keeps its disk; or a temporary
     one whose data is gone when its window closes. Scriptable systems can also be given
     packages, commands and files on the way in. -->
<script>
  import { X, Play, HardDrive, Plus, Flame } from '@lucide/svelte';
  import { fmtSize } from '../../lib/format.js';
  import { prefs } from '../../lib/prefs.svelte.js';
  import { STATE_LABEL, STATE_TONE, INPUT, AREA, BTN, BTN_PRIMARY, LABEL, lines, words } from '../../lib/clones-ui.js';

  /** machines: the ones that already exist for this target. onstart(body) launches. */
  let { target, machines, busy, onstart, onclose } = $props();

  const single = $derived(!!target.runtime); // Docker and emulator targets have one machine
  let mode = $state(''); // '' (choosing) | new | temporary
  let name = $state('');
  let memoryGb = $state(prefs.get('clones.memoryGb'));
  let cores = $state(prefs.get('clones.cores'));
  let fresh = $state(false);
  let packages = $state('');
  let rootCommands = $state('');
  let userCommands = $state('');
  let paths = $state('');

  function start(extra) {
    const pkgs = words(packages);
    onstart({
      target: target.id,
      memoryGb: Number(memoryGb) || 0,
      cores: Number(cores) || 0,
      apt: pkgs,
      pacman: pkgs,
      rootCommands: lines(rootCommands),
      userCommands: lines(userCommands),
      paths: lines(paths),
      ...extra,
    });
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="fixed inset-0 z-50 grid place-items-center bg-black/75 p-4" role="dialog" aria-modal="true" aria-label="Start {target.name}">
  <div class="hud-corners max-h-[92dvh] w-full max-w-2xl overflow-auto bg-panel-solid p-5">
    <header class="mb-4 flex items-start gap-3">
      <div class="min-w-0 flex-1">
        <h2 class="text-[18px] font-bold text-white">{target.name}</h2>
        <p class="text-[13px] text-text-dim">{target.description}</p>
      </div>
      <button class={BTN} onclick={onclose} aria-label="Close"><X class="h-4 w-4" /></button>
    </header>

    {#if !mode}
      {#if machines.length}
        <p class="micro mb-2 text-text-faint">USE A DISK THAT EXISTS</p>
        <ul class="mb-5 space-y-2">
          {#each machines as m (m.id)}
            {@const live = m.state === 'running' || m.state === 'launching'}
            <li>
              <button
                disabled={busy || live}
                onclick={() => start({ mode: 'existing', machine: m.id, memoryGb: 0, cores: 0 })}
                class="expo t-hover flex w-full items-center gap-3 border border-hairline p-3 text-left transition-colors hover:border-chiral/60 disabled:opacity-60"
              >
                <HardDrive class="h-5 w-5 shrink-0 text-chiral" strokeWidth={1.5} />
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-[14px] font-semibold text-white">{m.name}</span>
                  <span class="micro text-text-faint">{m.kind === 'main' ? 'MAIN DISK' : 'OWN DISK'}{m.diskBytes ? ` · ${fmtSize(m.diskBytes)}` : ''}</span>
                </span>
                {#if live}
                  <span class="micro {STATE_TONE[m.state]}">{STATE_LABEL[m.state]}</span>
                {:else}
                  <Play class="h-4 w-4 fill-current text-chiral" />
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <p class="micro mb-2 text-text-faint">{machines.length ? 'OR MAKE ONE' : 'MAKE ONE'}</p>
      <div class="grid gap-2 sm:grid-cols-2">
        {#if single}
          {#if target.apk}
            <label class="block sm:col-span-2">
              <span class={LABEL}>APKS TO INSTALL, AND OTHER FILES OR FOLDERS, ONE PATH PER LINE (OPTIONAL)</span>
              <textarea bind:value={paths} rows="3" spellcheck="false" placeholder="C:/Downloads/my-app.apk" class={AREA}></textarea>
              <span class="mt-1 block text-[12px] text-text-dim">Every .apk is installed and opened once Android has started. Other files go to Download/castle-copy on the device.</span>
            </label>
          {/if}
          <button disabled={busy} class="{BTN_PRIMARY} sm:col-span-2" onclick={() => start({ mode: 'existing', machine: target.id })}>
            <Play class="h-4 w-4 fill-black" /> START IT
          </button>
        {:else}
          <button class="expo t-hover border border-hairline p-4 text-left transition-colors hover:border-chiral/60" onclick={() => (mode = 'new')}>
            <Plus class="h-5 w-5 text-chiral" strokeWidth={1.5} />
            <span class="mt-2 block text-[14px] font-semibold text-white">New machine</span>
            <span class="block text-[12px] leading-relaxed text-text-dim">Its own disk. Everything you do in it is kept.</span>
          </button>
          <button class="expo t-hover border border-hairline p-4 text-left transition-colors hover:border-chiral/60" onclick={() => (mode = 'temporary')}>
            <Flame class="h-5 w-5 text-chiral" strokeWidth={1.5} />
            <span class="mt-2 block text-[14px] font-semibold text-white">Temporary</span>
            <span class="block text-[12px] leading-relaxed text-text-dim">Nothing is kept. All data is wiped when its window closes.</span>
          </button>
        {/if}
      </div>
      {#if !target.built && !single}
        <p class="mt-3 text-[12px] leading-relaxed text-text-dim">
          This system has not been built on this box yet, so the first start downloads and installs it. After that, new and temporary machines start from the built disk in seconds.
        </p>
      {/if}
    {:else}
      <form class="space-y-4" onsubmit={(e) => { e.preventDefault(); start({ mode, name, fresh }); }}>
        <p class="micro text-chiral">{mode === 'new' ? 'NEW MACHINE: KEEPS ITS DISK' : 'TEMPORARY: WIPED WHEN IT CLOSES'}</p>
        {#if mode === 'new'}
          <label class="block">
            <span class={LABEL}>NAME</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input bind:value={name} autofocus spellcheck="false" placeholder="test bench" class={INPUT} />
          </label>
        {/if}
        <div class="grid grid-cols-2 gap-3">
          <label class="block"><span class={LABEL}>MEMORY (GB)</span><input bind:value={memoryGb} type="number" min="1" max="256" class={INPUT} /></label>
          <label class="block"><span class={LABEL}>PROCESSOR CORES</span><input bind:value={cores} type="number" min="1" max="128" class={INPUT} /></label>
        </div>
        {#if mode === 'new' && target.built}
          <label class="flex items-center gap-2 text-[13px] text-text-dim">
            <input type="checkbox" bind:checked={fresh} class="accent-chiral" />
            Install from scratch instead of starting from the built disk (slower)
          </label>
        {/if}

        {#if target.inject}
          <p class="micro border-t border-hairline pt-4 text-text-faint">PUT THINGS IN IT (ALL OPTIONAL)</p>
          <label class="block">
            <span class={LABEL}>PACKAGES TO INSTALL</span>
            <input bind:value={packages} spellcheck="false" placeholder="git python3 htop" class={INPUT} />
          </label>
          <label class="block">
            <span class={LABEL}>FILES AND FOLDERS ON THIS BOX, ONE PATH PER LINE</span>
            <textarea bind:value={paths} rows="2" spellcheck="false" placeholder={'C:\\Downloads\\installer.run\nC:\\Projects\\my-app'} class={AREA}></textarea>
            <span class="mt-1 block text-[12px] text-text-dim">They arrive in ~/castle-copy. A folder arrives as a zip.</span>
          </label>
          <label class="block">
            <span class={LABEL}>COMMANDS AS ROOT, ONE PER LINE</span>
            <textarea bind:value={rootCommands} rows="2" spellcheck="false" placeholder="timedatectl set-timezone UTC" class={AREA}></textarea>
          </label>
          <label class="block">
            <span class={LABEL}>COMMANDS IN THE DESKTOP SESSION, ONE PER LINE</span>
            <textarea bind:value={userCommands} rows="2" spellcheck="false" placeholder="bash ~/castle-copy/installer.run" class={AREA}></textarea>
          </label>
        {:else}
          <p class="border-t border-hairline pt-4 text-[12px] leading-relaxed text-text-dim">
            Packages, scripts and files can be put into systems marked SCRIPTABLE. This one starts as it is.
          </p>
        {/if}

        <div class="flex gap-2">
          <button type="button" class={BTN} onclick={() => (mode = '')}>BACK</button>
          <button type="submit" disabled={busy || (mode === 'new' && !name.trim())} class="{BTN_PRIMARY} flex-1">
            <Play class="h-4 w-4 fill-black" /> {mode === 'new' ? 'CREATE AND START' : 'START TEMPORARY'}
          </button>
        </div>
      </form>
    {/if}
  </div>
</div>
