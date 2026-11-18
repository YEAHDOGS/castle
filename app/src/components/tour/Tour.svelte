<!-- Tour: the tutorial of the section showing, as a stepper (Steps.svelte): one pop-up at a
     time, four at most. This follows the section, so that a first visit to one starts its
     tutorial; the state, and what was seen before, is tour.svelte.js, and the words are
     steps.js. Renders nothing while the tour is off, so the everyday app never pays for it. -->
<script>
  import { untrack } from 'svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { tour } from './tour.svelte.js';
  import Steps from './Steps.svelte';

  // Only the section is watched: what arrive() reads of the tour must not run this again.
  $effect(() => {
    const section = shell.section;
    untrack(() => tour.arrive(section));
  });
</script>

{#if tour.on}
  <Steps />
{/if}
