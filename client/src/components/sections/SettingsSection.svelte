<!-- SettingsSection: everything configurable, drawn from the list in lib/settings.js.
     Each row says where its value lives (this device, or the box) and whether changing
     it does something today or is saved for the feature that will read it. -->
<script>
  import { untrack } from 'svelte';
  import { Check, Copy, Info } from '@lucide/svelte';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { prefs } from '../../lib/prefs.svelte.js';
  import { SHOWCASE, refused } from '../../lib/showcase.js';
  import { SETTING_GROUPS, SETTINGS, defaultsFor } from '../../lib/settings.js';
  import SectionFrame from '../SectionFrame.svelte';
  import madeByDogs from '../../assets/made-by-dogs-sticker.webp';

  const BOX_PORT_FALLBACK = '8787';
  const TAILSCALE_RUNNING = 'Running';
  const COPIED_MS = 1600;

  let group = $state(SETTING_GROUPS[0].id);
  let boxValues = $state(defaultsFor('box'));
  let info = $state(null); // facts the box reports about itself
  let copied = $state('');

  const source = $derived(shell.boxSource);
  const current = $derived(SETTING_GROUPS.find((g) => g.id === group));
  const rows = $derived(SETTINGS.filter((s) => s.group === group));
  const port = $derived(info?.listen?.split(':').pop() || BOX_PORT_FALLBACK);
  const tailscale = $derived(info?.links?.tailscale || null);
  const serveCommand = $derived(`tailscale serve --bg ${port}`);

  const valueOf = (setting) => (setting.where === 'device' ? prefs.get(setting.id) : boxValues[setting.id]);
  const locked = (setting) => setting.where === 'box' && !source;

  async function change(setting, value) {
    if (SHOWCASE) return refused(); // the read-only demo shows the settings but keeps them as they are
    if (setting.where === 'device') {
      prefs.set(setting.id, value);
      if (setting.id === 'storage.fileView') store.setViewMode(value);
      return;
    }
    const before = boxValues[setting.id];
    boxValues[setting.id] = value;
    try {
      await source.saveSettings({ [setting.id]: value });
    } catch (err) {
      boxValues[setting.id] = before;
      store.toast(err?.message || 'That setting could not be saved');
    }
  }

  /** Keeps a typed number inside the setting's range; an empty field falls back to the default. */
  function changeNumber(setting, raw) {
    const typed = Number(raw);
    const value = raw === '' || Number.isNaN(typed) ? setting.default : Math.min(setting.max, Math.max(setting.min, Math.round(typed)));
    change(setting, value);
  }

  async function copy(text) {
    try {
      await navigator.clipboard.writeText(text);
      copied = text;
      setTimeout(() => (copied = ''), COPIED_MS);
    } catch {
      store.toast('Could not copy. Select the text instead');
    }
  }

  async function load(from) {
    boxValues = defaultsFor('box');
    info = null;
    if (!from) return;
    try {
      const [saved, facts] = await Promise.all([from.settings(), from.boxInfo()]);
      boxValues = { ...defaultsFor('box'), ...saved };
      info = facts;
    } catch (err) {
      store.toast(err?.message || 'Settings could not be read from the box');
    }
  }

  $effect(() => {
    const from = source;
    untrack(() => load(from));
  });
</script>

{#snippet command(text)}
  <button
    class="expo t-hover group mt-2 flex w-full items-center gap-3 border border-hairline bg-black/60 px-3 py-2 text-left font-mono text-[12px] text-bridge transition-colors hover:border-bridge/50"
    onclick={() => copy(text)}
    aria-label={`Copy the command: ${text}`}
  >
    <span class="min-w-0 flex-1 truncate">{text}</span>
    {#if copied === text}
      <Check class="h-3.5 w-3.5 shrink-0 text-live" strokeWidth={2} />
    {:else}
      <Copy class="h-3.5 w-3.5 shrink-0 text-text-faint group-hover:text-white" strokeWidth={1.75} />
    {/if}
  </button>
{/snippet}

{#snippet fact(label, value)}
  <div>
    <dt class="micro text-text-faint">{label}</dt>
    <dd class="mt-0.5 break-all text-[13px] text-white">{value}</dd>
  </div>
{/snippet}

<SectionFrame
  title="Settings"
  subtitle={current?.blurb}
  tag={store.isDemo ? 'SAMPLE DATA' : ''}
  items={SETTING_GROUPS}
  active={group}
  onselect={(id) => (group = id)}
>
  <div class="mx-auto max-w-3xl">
    {#if group === 'connections'}
      <section class="glass mb-4 p-5">
        <h2 class="text-[15px] font-bold text-white">Where the box answers</h2>
        {#if info}
          <p class="mt-2 text-[13px] leading-relaxed text-text-dim">
            {#if info.loopback}
              On <span class="text-white">{info.listen}</span>, which is this machine only. Nothing else on your network or the internet can reach it.
              To use it from your phone or away from home, add one of the private links below.
            {:else}
              On <span class="text-white">{info.listen}</span>, which other machines on the network can reach, without encryption. Use it only on a network you trust.
            {/if}
          </p>
        {:else}
          <p class="mt-2 text-[13px] text-text-dim">Connect to a box to see how it is reached.</p>
        {/if}
      </section>

      <section class="glass mb-4 p-5">
        <div class="flex flex-wrap items-center gap-3">
          <h2 class="flex-1 text-[15px] font-bold text-white">Tailscale</h2>
          {#if tailscale}
            <p class="micro flex items-center gap-2 {tailscale.state === TAILSCALE_RUNNING ? 'text-live' : 'text-text-faint'}" role="status">
              <span class="h-2 w-2 rounded-full bg-current"></span>
              {!tailscale.installed ? 'NOT INSTALLED ON THE BOX' : tailscale.state === TAILSCALE_RUNNING ? 'CONNECTED' : tailscale.state ? tailscale.state.toUpperCase() : 'INSTALLED'}
            </p>
          {/if}
        </div>
        <p class="mt-2 text-[13px] leading-relaxed text-text-dim">
          Tailscale gives each of your devices a private address that only your other devices can reach, wherever they are. Nothing is
          opened on your router. It is free for personal use and built on WireGuard.
        </p>
        {#if tailscale?.state === TAILSCALE_RUNNING}
          <dl class="mt-4 grid gap-3 sm:grid-cols-2">
            {@render fact('THIS BOX IS', tailscale.name || 'unnamed')}
            {@render fact('PRIVATE ADDRESS', tailscale.addresses.join(', ') || 'unknown')}
          </dl>
        {/if}
        <ol class="mt-4 space-y-4 text-[13px] leading-relaxed text-text-dim">
          <li>
            <span class="text-white">1. Put Tailscale on the box's machine.</span> Install it from tailscale.com, then sign in. Signing in creates your private network.
            {@render command('tailscale up')}
          </li>
          <li>
            <span class="text-white">2. Put Tailscale on your phone and laptop</span> and sign in with the same account. They can now see the box's machine.
          </li>
          <li>
            <span class="text-white">3. Share Castle to those devices.</span> Run this on the box's machine and open the https address it prints. The first time, Tailscale asks you to turn on HTTPS certificates for your account.
            {@render command(serveCommand)}
          </li>
        </ol>
        <p class="mt-4 flex gap-2 text-[12px] leading-relaxed text-text-dim">
          <Info class="mt-0.5 h-3.5 w-3.5 shrink-0 text-text-faint" strokeWidth={1.75} />
          <span>Castle does not run these for you yet. Once shared, any of your Tailscale devices that has the box token can use everything, including Clones and Arcade's Play.</span>
        </p>
      </section>

      <section class="glass mb-4 p-5">
        <div class="flex flex-wrap items-center gap-3">
          <h2 class="flex-1 text-[15px] font-bold text-white">WireGuard</h2>
          {#if info}
            <p class="micro text-text-faint">{info.links.wireguardInstalled ? 'INSTALLED ON THE BOX' : 'NOT INSTALLED ON THE BOX'}</p>
          {/if}
        </div>
        <p class="mt-2 text-[13px] leading-relaxed text-text-dim">
          The same encryption without Tailscale's account and coordination. You set it up by hand: each device gets a key pair, the box
          lists every device as a peer, and your router forwards one port to the box. It suits a backup way in. Castle does not write
          that configuration for you yet.
        </p>
      </section>
    {:else if group === 'box' && info}
      <section class="glass mb-4 p-5">
        <h2 class="text-[15px] font-bold text-white">How this box was started</h2>
        <dl class="mt-3 grid gap-3 sm:grid-cols-2">
          {@render fact('VERSION', info.version)}
          {@render fact('ANSWERS ON', info.listen)}
          {@render fact('CLONES', info.clones ? 'On: a Clonewars folder was given' : 'Off: start with --clonewars')}
          {@render fact('ARCADE', info.arcade ? 'On: a games folder was given' : 'Off: start with --arcade')}
        </dl>
        <p class="mt-3 text-[12px] leading-relaxed text-text-dim">
          These are chosen when the box starts, not from this screen: a web page should not be able to point the box at a new folder of programs to run.
        </p>
      </section>
    {/if}

    <ul class="space-y-3">
      {#each rows as setting (setting.id)}
        {@const value = valueOf(setting)}
        {@const off = locked(setting)}
        <li class="glass p-4 {off ? 'opacity-50' : ''}">
          <div class="flex flex-wrap items-start gap-x-4 gap-y-3">
            <div class="min-w-[12rem] flex-1">
              <p class="text-[14px] font-semibold text-white">{setting.label}</p>
              <p class="mt-0.5 text-[12px] leading-relaxed text-text-dim">{setting.help}</p>
              <p class="micro mt-2 flex flex-wrap gap-x-3 gap-y-1">
                <span class={setting.where === 'device' ? 'text-bridge' : 'text-chiral'}>{setting.where === 'device' ? 'THIS DEVICE' : 'ON THE BOX'}</span>
                {#if !setting.live}<span class="text-text-faint">SAVED · NOT ACTIVE YET</span>{/if}
              </p>
            </div>

            {#if setting.type === 'toggle'}
              <button
                role="switch"
                aria-checked={!!value}
                aria-label={setting.label}
                disabled={off}
                onclick={() => change(setting, !value)}
                class="expo t-hover relative h-7 w-12 shrink-0 border transition-colors {value ? 'border-chiral bg-chiral-dim' : 'border-hairline bg-white/[0.03]'}"
              >
                <span class="expo t-ui absolute top-1/2 h-4 w-4 -translate-y-1/2 transition-all {value ? 'left-[26px] bg-chiral' : 'left-1 bg-text-faint'}"></span>
              </button>
            {:else if setting.type === 'choice'}
              <div class="flex shrink-0 flex-wrap border border-hairline" role="radiogroup" aria-label={setting.label}>
                {#each setting.options as option (option.value)}
                  <button
                    role="radio"
                    aria-checked={value === option.value}
                    disabled={off}
                    onclick={() => change(setting, option.value)}
                    class="expo t-hover micro px-3 py-2 transition-colors {value === option.value ? 'bg-chiral font-bold text-black' : 'text-text-dim hover:text-white'}"
                  >
                    {option.label}
                  </button>
                {/each}
              </div>
            {:else if setting.type === 'number'}
              <label class="flex shrink-0 items-center gap-2">
                <input
                  type="number"
                  min={setting.min}
                  max={setting.max}
                  {value}
                  disabled={off}
                  aria-label={setting.label}
                  onchange={(e) => (SHOWCASE ? ((e.currentTarget.value = value ?? ""), refused()) : changeNumber(setting, e.currentTarget.value))}
                  class="h-9 w-24 border border-hairline bg-white/[0.03] px-2 text-right text-[14px] tabular-nums text-white focus:border-chiral/60 focus:outline-none"
                />
                <span class="micro w-14 text-text-faint">{setting.unit}</span>
              </label>
            {:else}
              <input
                type="text"
                {value}
                disabled={off}
                placeholder={setting.placeholder || ''}
                aria-label={setting.label}
                autocomplete="off"
                spellcheck="false"
                onchange={(e) => (SHOWCASE ? ((e.currentTarget.value = value ?? ""), refused()) : change(setting, e.currentTarget.value.trim()))}
                class="h-9 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none sm:w-64"
              />
            {/if}
          </div>
        </li>
      {/each}
    </ul>

    {#if !source && rows.some((s) => s.where === 'box')}
      <p class="mt-4 text-[12px] text-text-dim">
        Settings marked ON THE BOX need a box. Connect to one from Storage, or try them in the <a class="text-bridge underline" href="/demo">demo</a>.
      </p>
    {/if}

    <!-- room under the last setting, so the sticker never covers it -->
    <div class="h-24" aria-hidden="true"></div>
  </div>
</SectionFrame>

<!-- The sticker is pinned to the bottom right corner and stays there while the page scrolls. -->
<a class="expo t-hover fixed bottom-4 right-4 z-10 opacity-90 transition-opacity hover:opacity-100" href="https://wearedogs.net" target="_blank" rel="noreferrer">
  <img src={madeByDogs} alt="This product was made by DOGS" width="360" height="119" class="h-auto w-[200px]" />
</a>
