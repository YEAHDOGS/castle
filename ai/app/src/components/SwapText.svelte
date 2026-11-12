<script>
  // Text that slides/fades when its value changes. The old and new strings share one grid cell, so the swap
  // never shifts the surrounding layout.
  import { textIn, textOut } from '../lib/motion.js'

  /** @type {{ text: string, class?: string }} */
  let { text, class: cls = '' } = $props()
</script>

<span class="inline-grid {cls}">
  {#key text}
    <span class="col-start-1 row-start-1 whitespace-nowrap" in:textIn out:textOut>{text}</span>
  {/key}
</span>
