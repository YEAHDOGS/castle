<!-- AiSection: AI is DOGS AI. The box runs DOGS AI's server (its code is in this repository
     under ai/), and this section shows DOGS AI's own app, exactly as it is, in the space below
     Castle's header: create images, video and audio, chat, characters, the model library.

     The app comes from the dogs server's own address, so it keeps its own look and its own
     settings. On a box reached over the network, the dogs server needs its key: the header
     shows it, for DOGS AI's Settings. The demo cannot run models and shows sample ones. -->
<script>
  import { BrainCircuit, Power, ExternalLink, KeyRound, Check, LoaderCircle, RotateCw } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { store } from '../../lib/store.svelte.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';
  import DemoModels from '../ai/DemoModels.svelte';

  const POLL_MS = 5000;

  let status = $state(null);
  let problem = $state('');
  let busy = $state(false);
  let copied = $state(false);
  let frameKey = $state(0); // bumped to reload the app

  const source = $derived(store.isDemo ? null : store.boxUrl ? store.adapter : null);
  // The dogs server runs where the box does: same machine name, its own port.
  const appUrl = $derived(status?.running && store.boxUrl ? `${new URL(store.boxUrl).protocol}//${new URL(store.boxUrl).hostname}:${status.port}/` : null);

  async function load(from = source) {
    if (!from) return;
    try {
      const next = await from.aiStatus();
      if (from !== source) return;
      status = next;
      problem = '';
    } catch (err) {
      problem = err?.message || 'The box did not say how AI is';
    }
  }

  $effect(() => {
    const from = source;
    untrack(() => {
      status = null;
      load(from);
    });
    if (!from) return;
    const timer = setInterval(() => load(from), POLL_MS);
    return () => clearInterval(timer);
  });

  async function start() {
    busy = true;
    try {
      status = await source.aiStart();
      if (!status.running) store.toast('DOGS AI is starting. It can take a few seconds.');
    } catch (err) {
      store.toast(err?.message || 'DOGS AI did not start');
    } finally {
      busy = false;
    }
  }

  async function stop() {
    busy = true;
    try {
      status = await source.aiStop();
    } catch (err) {
      store.toast(err?.message || 'DOGS AI did not stop');
    } finally {
      busy = false;
    }
  }

  async function copyKey() {
    try {
      await navigator.clipboard.writeText(status.key);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      store.toast('Copy it by hand: the clipboard is blocked here');
    }
  }
</script>

{#snippet actions()}
  {#if status?.key}
    <button class="micro expo t-hover hidden h-9 items-center gap-1.5 border border-hairline px-2.5 text-text-dim hover:text-white md:flex" onclick={copyKey} title="DOGS AI's key, for its Settings when this box is reached over the network">
      {#if copied}<Check class="h-3.5 w-3.5 text-live" />{:else}<KeyRound class="h-3.5 w-3.5" />{/if}KEY
    </button>
  {/if}
  {#if appUrl}
    <button class="expo t-hover grid h-9 w-9 place-items-center border border-hairline text-text-dim hover:text-white" onclick={() => frameKey++} aria-label="Reload DOGS AI" title="Reload">
      <RotateCw class="h-3.5 w-3.5" />
    </button>
    <a class="expo t-hover grid h-9 w-9 place-items-center border border-hairline text-text-dim hover:text-white" href={appUrl} target="_blank" rel="noopener noreferrer" aria-label="Open DOGS AI in its own window" title="Open in its own window">
      <ExternalLink class="h-3.5 w-3.5" />
    </a>
    {#if status.ours}
      <button class="micro expo t-hover hidden h-9 items-center gap-1.5 border border-hairline px-2.5 text-text-dim hover:text-white disabled:opacity-50 sm:flex" onclick={stop} disabled={busy}>
        <Power class="h-3.5 w-3.5" />STOP
      </button>
    {/if}
  {/if}
{/snippet}

{#if store.isDemo}
  <DemoModels />
{:else}
  <SectionFrame
    title="AI"
    subtitle={appUrl ? `DOGS AI · ${status.installed ? 'ready' : 'runtime not installed'}` : 'DOGS AI'}
    bare={!!appUrl}
    {actions}
  >
    {#if !source}
      <Notice icon={BrainCircuit} title="NO BOX CONNECTED">AI is DOGS AI, running on your Castle box. Connect to a box to use it, or switch to Demo.</Notice>
    {:else if problem && !status}
      <Notice icon={BrainCircuit} title="AI IS NOT AVAILABLE">{problem}</Notice>
    {:else if !status}
      <div class="grid min-h-[40dvh] place-items-center"><LoaderCircle class="h-6 w-6 animate-spin text-text-faint" /></div>
    {:else if appUrl}
      {#key frameKey}
        <iframe src={appUrl} title="DOGS AI" class="block h-full w-full border-0 bg-black" allow="clipboard-read; clipboard-write; fullscreen; autoplay"></iframe>
      {/key}
    {:else if !status.dir}
      <Notice icon={BrainCircuit} title="DOGS AI WAS NOT FOUND">
        Its code is the <span class="text-white">ai</span> folder of the Castle checkout. Set CASTLE_AI_DIR on the box to use a copy somewhere else.
      </Notice>
    {:else if !status.node}
      <Notice icon={BrainCircuit} title="NODE.JS IS NEEDED">DOGS AI runs on Node.js 22 or newer. Install it on the box from nodejs.org, then start it here.</Notice>
    {:else if !status.built}
      <Notice icon={BrainCircuit} title="DOGS AI'S APP IS NOT BUILT">
        Build it once on the box: <code class="text-white">npm --prefix app/ai/app install</code> then <code class="text-white">npm --prefix ai/app run build</code>. Castle's start script does this for you.
      </Notice>
    {:else}
      <Notice icon={BrainCircuit} title="DOGS AI IS NOT RUNNING">
        From {status.dir}.
        <button class="micro mx-auto mt-4 flex h-10 items-center gap-2 bg-chiral px-5 font-bold text-black hover:brightness-110 disabled:opacity-50" onclick={start} disabled={busy}>
          <Power class="h-4 w-4" />{busy ? 'STARTING…' : 'START DOGS AI'}
        </button>
        {#if status.log}<pre class="micro mt-4 max-h-40 overflow-auto whitespace-pre-wrap border border-hairline bg-black/50 p-2 text-left normal-case tracking-normal text-text-faint">{status.log.slice(-1200)}</pre>{/if}
      </Notice>
    {/if}
  </SectionFrame>
{/if}
