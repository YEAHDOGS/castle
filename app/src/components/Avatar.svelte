<!-- Avatar: a person's initial in their colour. -->
<script>
  let { person, size = 'md' } = $props();

  const TONES = {
    chiral: 'border-chiral/60 bg-chiral-dim text-chiral',
    bridge: 'border-bridge/60 bg-bridge-dim text-bridge',
    violet: 'border-violet/60 bg-violet/15 text-violet',
    faint: 'border-hairline bg-white/[0.05] text-text-dim',
  };
  const SIZES = { sm: 'h-7 w-7 text-[11px]', md: 'h-8 w-8 text-[12px]' };
</script>

<span class="grid shrink-0 place-items-center border font-bold {SIZES[size]} {TONES[person.tone] || TONES.faint}" aria-hidden="true">
  {person.name.slice(0, 1).toUpperCase()}
</span>
