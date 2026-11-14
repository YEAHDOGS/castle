<!-- Notice: a centred message for a section that has nothing to show yet, with one way forward. -->
<script>
  let { icon: Icon, title, children } = $props();
</script>

<div class="wiregrid grid min-h-[50dvh] place-items-center p-6">
  <div class="hud-corners-faint hud-corners max-w-md bg-panel-solid/80 p-6 text-center">
    <Icon class="mx-auto h-8 w-8 text-text-faint" strokeWidth={1.25} />
    <p class="micro mt-3 text-white">{title}</p>
    <div class="mt-2 text-[13px] leading-relaxed text-text-dim">{@render children()}</div>
  </div>
</div>
