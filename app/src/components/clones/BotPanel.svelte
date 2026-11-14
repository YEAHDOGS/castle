<!-- BotPanel: one bot of an automation. Its sign-in, its start and stop, what the box did,
     and its machine's screen while it runs. A value that comes from the project's own .env
     is never shown: the panel only says it is being used. -->
<script>
  import { Play, Square, Save, Trash2, ScrollText } from '@lucide/svelte';
  import VmScreen from '../VmScreen.svelte';
  import { STATE_LABEL, STATE_TONE, INPUT, BTN, BTN_PRIMARY, BTN_STOP, LABEL } from '../../lib/clones-ui.js';

  /** onact(call, doneMessage) runs a box call and resolves true when it worked. */
  let { source, automation, bot, busy, onact } = $props();

  const CONFIRM_MS = 4000;
  const CLEAR = ' '; // a single space tells the box to forget a saved value

  let edits = $state({}); // env -> what was typed
  let replacing = $state({}); // env -> true: typing over a value the project's .env supplies
  let name = $state('');
  let showLog = $state(false);
  let confirming = $state(false);
  let logEl = $state(null);

  const live = $derived(bot.state === 'running' || bot.state === 'launching');
  const dirty = $derived(Object.values(edits).some(Boolean) || (!!name.trim() && name.trim() !== bot.name));
  const valueOf = (env) => bot.values.find((v) => v.env === env) || { from: 'missing', value: '' };

  async function saveBot(values = edits) {
    const ok = await onact(() => source.cloneBotSave(automation.id, bot.id, name, values), 'Saved');
    if (!ok) return;
    edits = {};
    replacing = {};
    name = '';
  }

  function remove() {
    if (!confirming) {
      confirming = true;
      setTimeout(() => (confirming = false), CONFIRM_MS);
      return;
    }
    onact(() => source.cloneBotDelete(automation.id, bot.id), 'Bot removed');
  }

  // Keep the newest log line in view while a launch is scrolling past.
  $effect(() => {
    bot.log;
    if (logEl) logEl.scrollTop = logEl.scrollHeight;
  });
</script>

<section class="border border-hairline bg-black/30 p-4">
  <header class="mb-3 flex flex-wrap items-center gap-3">
    <input
      value={name || bot.name}
      oninput={(e) => (name = e.currentTarget.value)}
      aria-label="Bot name"
      spellcheck="false"
      class="min-w-0 flex-1 border-b border-transparent bg-transparent text-[15px] font-bold text-white hover:border-hairline focus:border-chiral/60 focus:outline-none"
    />
    {#if bot.engine === 'container'}<span class="micro text-text-faint">CONTAINER</span>{:else if bot.engine === 'emulator'}<span class="micro text-text-faint">OWN PHONE</span>{:else if bot.ownDisk}<span class="micro text-text-faint">OWN DISK</span>{/if}
    <p class="micro flex items-center gap-2 {STATE_TONE[bot.state]}" role="status">
      <span class="h-2 w-2 rounded-full bg-current {bot.state === 'launching' ? 'animate-pulse' : ''}"></span>
      {STATE_LABEL[bot.state] || bot.state}
    </p>
  </header>

  <form class="space-y-3" onsubmit={(e) => { e.preventDefault(); saveBot(); }}>
    {#each automation.fields as field (field.env)}
      {@const v = valueOf(field.env)}
      {#if v.from === 'env' && !replacing[field.env]}
        <div class="flex items-center gap-3">
          <p class="flex-1 text-[13px] text-live">Using {field.label.toLowerCase()} in env</p>
          <button type="button" class="micro text-text-faint hover:text-white" onclick={() => (replacing[field.env] = true)}>USE ANOTHER</button>
        </div>
      {:else}
        <label class="block">
          <span class="{LABEL} flex items-center gap-2">
            <span class="flex-1">{field.label.toUpperCase()}</span>
            {#if v.from === 'saved'}
              <button type="button" class="hover:text-white" onclick={() => saveBot({ [field.env]: CLEAR })}>FORGET</button>
            {/if}
          </span>
          <input
            bind:value={edits[field.env]}
            type={field.secret ? 'password' : 'text'}
            autocomplete={field.secret ? 'new-password' : 'off'}
            spellcheck="false"
            placeholder={v.from === 'saved' ? (field.secret ? 'saved. type to replace it' : v.value) : 'not set yet'}
            class={INPUT}
          />
        </label>
      {/if}
    {/each}
    {#if dirty}
      <button type="submit" disabled={busy} class="{BTN} w-full"><Save class="h-3.5 w-3.5" strokeWidth={1.75} /> SAVE</button>
    {/if}
  </form>

  <div class="mt-3 flex gap-2">
    {#if live}
      <button disabled={busy} class="{BTN_STOP} flex-1" onclick={() => onact(() => source.cloneStop(automation.id, bot.id), bot.engine === 'container' ? 'Stopping the container' : 'Shutting the machine down')}>
        <Square class="h-3.5 w-3.5 fill-current" strokeWidth={2} /> STOP
      </button>
    {:else}
      <button disabled={busy || !bot.ready || dirty} class="{BTN_PRIMARY} flex-1" onclick={() => onact(() => source.cloneStart(automation.id, bot.id), bot.engine === 'container' ? 'Starting the container' : 'Starting the machine')}>
        <Play class="h-4 w-4 fill-black" strokeWidth={2} /> START
      </button>
    {/if}
    <button class={BTN} aria-pressed={showLog} title="What the box did" onclick={() => (showLog = !showLog)}><ScrollText class="h-4 w-4" /></button>
    {#if bot.id !== 'main' && !live}
      <button disabled={busy} class={BTN} title="Remove this bot and its disk" onclick={remove}>
        <Trash2 class="h-4 w-4" />{#if confirming}REMOVE IT AND ITS DISK?{/if}
      </button>
    {/if}
  </div>
  {#if !live && !bot.ready}
    <p class="mt-2 text-[12px] text-text-dim">Fill in what is missing and save to enable Start.</p>
  {/if}

  {#if showLog || bot.state === 'launching' || bot.state === 'failed'}
    <pre bind:this={logEl} class="mt-3 h-44 overflow-auto border border-hairline bg-black/60 p-3 font-mono text-[11px] leading-relaxed text-text-dim" aria-live="polite">{bot.log.length ? bot.log.join('\n') : 'Nothing yet. Press Start and the launch appears here line by line.'}</pre>
  {/if}
  {#if bot.state === 'running' && source.cloneScreen}
    <VmScreen {source} id={bot.key} controllable={bot.controllable} name={bot.name} />
  {/if}
</section>
