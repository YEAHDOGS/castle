<!-- AskDialog: the yes-or-no questions Castle asks (lib/ask.svelte.js). Cancel takes focus
     first, so a stray Enter or A never deletes anything. Escape or B is Cancel. -->
<script>
  import { scale, fade } from 'svelte/transition';
  import { asking } from '../lib/ask.svelte.js';
  import { modalIn, reducedMotion } from '../lib/motion.js';

  const q = $derived(asking.current);

  // Capturing, so Escape answers the question and does not also close what is under it.
  function onKey(e) {
    if (!q || e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    q.answer(false);
  }
</script>

<svelte:window onkeydowncapture={onKey} />

{#if q}
  <div class="fixed inset-0 z-[90] grid place-items-center p-4" role="alertdialog" aria-modal="true" aria-label="Question" aria-describedby="ask-question">
    <div class="absolute inset-0 bg-black/70" transition:fade={{ duration: reducedMotion() ? 0 : 150 }} role="presentation" onclick={() => q.answer(false)}></div>
    <div transition:scale={modalIn()} class="hud-corners glass relative z-10 w-full max-w-md px-5 py-5">
      <p id="ask-question" class="text-[14px] leading-relaxed text-white">{q.question}</p>
      <div class="mt-5 flex justify-end gap-2">
        <button data-autofocus class="micro expo t-hover h-10 border border-hairline px-4 font-bold text-white hover:border-white/30" onclick={() => q.answer(false)}>
          {q.no.toUpperCase()}
        </button>
        <button
          class="micro expo t-hover h-10 px-4 font-bold transition-all hover:brightness-110 {q.danger ? 'bg-red-500 text-black' : 'bg-chiral text-black'}"
          onclick={() => q.answer(true)}
        >
          {q.yes.toUpperCase()}
        </button>
      </div>
    </div>
  </div>
{/if}
