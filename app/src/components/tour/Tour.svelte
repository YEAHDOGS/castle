<!-- Tour: the first-run tutorial of the demo. The carousel first (one to three big cards,
     once), then the comic-book blurbs over the interface, until each has been put away. The
     state, and what was seen before, is tour.svelte.js; what the blurbs say is blurbs.js.
     Renders nothing while the tour is off, so the everyday app never pays for it. -->
<script>
  import { tour } from './tour.svelte.js';
  import Carousel from './Carousel.svelte';
  import Blurbs from './Blurbs.svelte';
</script>

{#if tour.on}
  {#if tour.carousel}
    <Carousel />
  {:else}
    <Blurbs />
  {/if}
{/if}
