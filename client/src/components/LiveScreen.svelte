<!-- LiveScreen: taking control of a machine in a container. The box starts a VNC server and
     a web viewer inside the container behind a one-time password, on this computer's own
     loopback address; this shows that viewer. Closing it turns the server off again, so the
     machine is only reachable while someone is at it. -->
<script>
  import { onMount } from 'svelte';
  import { X, MousePointer2 } from '@lucide/svelte';

  /** source: where Clones reads from. id: the machine's key. name: what to call it. */
  let { source, id, name = 'the machine', onclose } = $props();

  let url = $state('');
  let problem = $state('');

  onMount(() => {
    let closed = false;
    source
      .cloneLive(id, true)
      .then((res) => {
        if (closed) source.cloneLive(id, false).catch(() => {});
        else url = res.url;
      })
      .catch((err) => (problem = err?.message || 'Live control could not be started'));
    return () => {
      closed = true;
      source.cloneLive(id, false).catch(() => {});
    };
  });
</script>

<svelte:document onkeydown={(e) => { if (e.key === 'Escape' && e.shiftKey && !e.defaultPrevented) { e.preventDefault(); onclose(); } }} />

<div class="fixed inset-0 z-50 flex flex-col bg-black/90 p-3" role="dialog" aria-modal="true" aria-label={`Controlling ${name}`}>
  <header class="mb-2 flex items-center gap-3">
    <MousePointer2 class="h-4 w-4 text-chiral" />
    <p class="micro flex-1 text-white">YOU ARE CONTROLLING {name.toUpperCase()} · KEYS AND CLICKS GO TO IT · SHIFT+ESC CLOSES</p>
    <button class="expo t-hover micro flex h-9 items-center gap-2 border border-hairline px-3 font-bold text-white hover:border-chiral/60" onclick={onclose}>
      <X class="h-4 w-4" /> DONE
    </button>
  </header>
  <div class="grid flex-1 place-items-center overflow-hidden border border-hairline bg-black">
    {#if url}
      <iframe src={url} title={`Live screen of ${name}`} class="block h-full w-full border-0" allow="clipboard-read; clipboard-write"></iframe>
    {:else}
      <p class="micro p-10 text-center {problem ? 'text-chiral' : 'text-text-faint'}">{problem || 'STARTING THE LIVE VIEWER IN THE CONTAINER'}</p>
    {/if}
  </div>
</div>
