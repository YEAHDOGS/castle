<script>
  import ImageInput from './ImageInput.svelte'

  /** @type {{ name: string, spec: { type: string, label?: string, min?: number, max?: number, step?: number, options?: string[] }, value: any }} */
  let { name, spec, value = $bindable() } = $props()

  const INPUT = 'w-full well rounded-lg px-3 py-2 text-sm 2xl:text-lg outline-none'
  const label = $derived(spec.label || name.replace(/_/g, ' '))
</script>

{#if spec.type === 'image'}
  <ImageInput {label} bind:value />
{:else}
  <label class="flex flex-col gap-1">
    <span class="text-[11px] 2xl:text-sm uppercase tracking-wider text-muted">{label}</span>
    {#if spec.type === 'text'}
      <textarea class="{INPUT} resize-y" rows="2" bind:value></textarea>
    {:else if spec.type === 'select'}
      <select class={INPUT} bind:value>
        {#each spec.options || [] as o (o)}<option value={o}>{o}</option>{/each}
      </select>
    {:else}
      <input class={INPUT} type="number" min={spec.min} max={spec.max} step={spec.step} bind:value />
    {/if}
  </label>
{/if}
