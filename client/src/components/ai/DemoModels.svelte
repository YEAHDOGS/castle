<!-- DemoModels: the demo's AI page. Sample models to pick from; on a box, AI is DOGS AI
     itself (see AiSection). -->
<script>
  import { BrainCircuit, Check, Download, Cpu } from '@lucide/svelte';
  import { fly } from 'svelte/transition';
  import { fmtSize } from '../../lib/format.js';
  import { itemIn } from '../../lib/motion.js';
  import { DEMO_MODELS, DEMO_AI_HARDWARE } from '../../lib/demo-shell.js';
  import SectionFrame from '../SectionFrame.svelte';

  /** actions: the header's right side, from AiSection. */
  let { actions = null } = $props();

  const ALL = 'all';
  const THOUSAND = 1000;
  const FULL = 100;
  const FAMILIES = [...new Set(DEMO_MODELS.map((m) => m.family))];
  const VIEWS = [{ id: ALL, label: 'All models', icon: BrainCircuit }, ...FAMILIES.map((f) => ({ id: f, label: f }))];
  const STATE_LABEL = { loaded: 'LOADED', ready: 'ON DISK', available: 'NOT DOWNLOADED' };

  let family = $state(ALL);
  let loadedId = $state(DEMO_MODELS.find((m) => m.state === 'loaded')?.id || null);

  const shown = $derived(family === ALL ? DEMO_MODELS : DEMO_MODELS.filter((m) => m.family === family));
  const loaded = $derived(DEMO_MODELS.find((m) => m.id === loadedId) || null);
  const railItems = $derived(VIEWS.map((v) => ({ ...v, count: v.id === ALL ? DEMO_MODELS.length : DEMO_MODELS.filter((m) => m.family === v.id).length })));
  const vramPercent = (DEMO_AI_HARDWARE.vramUsed / DEMO_AI_HARDWARE.vramTotal) * FULL;
  const ramPercent = (DEMO_AI_HARDWARE.ramUsed / DEMO_AI_HARDWARE.ramTotal) * FULL;

  const stateOf = (model) => (model.id === loadedId ? 'loaded' : model.state === 'loaded' ? 'ready' : model.state);
  const contextLabel = (tokens) => (tokens ? `${Math.round(tokens / THOUSAND)}K CONTEXT` : 'AUDIO IN');
</script>

<SectionFrame
  title="AI"
  subtitle={loaded ? `${loaded.name} is loaded` : 'No model loaded'}
  tag="SAMPLE DATA"
  items={railItems}
  active={family}
  onselect={(id) => (family = id)}
  {actions}
>
  <!-- what the box has to run models on -->
  <div class="glass mb-5 grid gap-4 p-4 sm:grid-cols-2">
    {#each [{ label: DEMO_AI_HARDWARE.gpu, used: DEMO_AI_HARDWARE.vramUsed, total: DEMO_AI_HARDWARE.vramTotal, percent: vramPercent }, { label: 'MEMORY', used: DEMO_AI_HARDWARE.ramUsed, total: DEMO_AI_HARDWARE.ramTotal, percent: ramPercent }] as meter (meter.label)}
      <div>
        <div class="micro mb-2 flex items-center justify-between text-text-faint">
          <span class="flex items-center gap-1.5"><Cpu class="h-3 w-3" /> {meter.label}</span>
          <span class="text-text-dim">{fmtSize(meter.used)} / {fmtSize(meter.total)}</span>
        </div>
        <div class="h-[3px] overflow-hidden rounded-full bg-white/[0.07]">
          <div class="h-full rounded-full" style="width: {meter.percent}%; background: linear-gradient(90deg, var(--color-chiral), var(--color-bridge));"></div>
        </div>
      </div>
    {/each}
  </div>

  <ul class="grid gap-3 lg:grid-cols-2 2xl:grid-cols-3">
    {#each shown as model, i (model.id)}
      {@const state = stateOf(model)}
      <li in:fly={itemIn(i)} class="flex flex-col border bg-white/[0.02] p-4 {state === 'loaded' ? 'border-chiral/60' : 'border-hairline'}">
        <div class="flex items-start gap-3">
          <BrainCircuit class="mt-0.5 h-6 w-6 shrink-0 {state === 'loaded' ? 'text-chiral' : 'text-text-faint'}" strokeWidth={1.25} />
          <div class="min-w-0 flex-1">
            <h2 class="text-[15px] font-bold text-white">{model.name}</h2>
            <p class="micro mt-0.5 text-text-faint">{model.family} · {model.params} · {model.quant}</p>
          </div>
          <span class="micro shrink-0 {state === 'loaded' ? 'text-live' : 'text-text-faint'}">{STATE_LABEL[state]}</span>
        </div>
        <p class="mt-3 flex-1 text-[13px] text-text-dim">{model.note}</p>
        <div class="mt-4 flex items-center gap-3">
          <p class="micro flex-1 text-text-faint">{fmtSize(model.size)} · {contextLabel(model.context)}</p>
          {#if state === 'loaded'}
            <span class="micro flex items-center gap-1.5 text-chiral"><Check class="h-3.5 w-3.5" strokeWidth={2} /> IN USE</span>
          {:else if state === 'ready'}
            <button
              class="expo t-hover micro h-8 border border-hairline px-3 font-bold text-white transition-colors hover:border-chiral/60"
              onclick={() => (loadedId = model.id)}
            >
              LOAD
            </button>
          {:else}
            <span class="micro flex items-center gap-1.5 text-text-faint"><Download class="h-3.5 w-3.5" /> NOT ON THE BOX</span>
          {/if}
        </div>
      </li>
    {/each}
  </ul>

  <p class="mt-5 text-[12px] leading-relaxed text-text-dim">
    These models are sample data: the box does not report its own yet, and Load only changes this screen. Chat is not part of
    this build.
  </p>
</SectionFrame>
