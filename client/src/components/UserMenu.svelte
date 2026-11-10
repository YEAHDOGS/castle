<!-- UserMenu: who is using Castle. Opens from the Castle mark, top left. Each person has
     their own storage and arcade; a locked person asks for their PIN before switching. -->
<script>
  import { Lock, Check, X } from '@lucide/svelte';
  import { scale, fade } from 'svelte/transition';
  import { shell } from '../lib/shell.svelte.js';
  import { store } from '../lib/store.svelte.js';
  import { DEMO_PIN } from '../lib/demo-shell.js';
  import { pop, reducedMotion } from '../lib/motion.js';
  import Avatar from './Avatar.svelte';

  const PIN_LENGTH = 4;

  let askingFor = $state(null); // id of the locked person whose PIN is being typed
  let pin = $state('');
  let error = $state('');

  function reset() {
    askingFor = null;
    pin = '';
    error = '';
  }

  function close() {
    shell.userMenuOpen = false;
    reset();
  }

  async function pick(person) {
    if (person.id === shell.user.id) return close();
    if (person.locked) {
      askingFor = person.id;
      pin = '';
      error = '';
      return;
    }
    await shell.switchUser(person.id);
    reset();
  }

  async function submitPin(e) {
    e.preventDefault();
    error = (await shell.switchUser(askingFor, pin)) || '';
    if (error) pin = '';
    else reset();
  }
</script>

{#if shell.userMenuOpen}
  <div class="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-label="People">
    <div
      transition:fade={{ duration: reducedMotion() ? 0 : 150 }}
      class="absolute inset-0 bg-black/50"
      onclick={close}
      onkeydown={(e) => { if (e.key === 'Escape') close(); }}
      role="button"
      tabindex="-1"
      aria-label="Close"
    ></div>
    <div
      transition:scale={pop()}
      class="hud-corners glass absolute left-3 top-3 w-[min(20rem,calc(100vw-1.5rem))] md:left-4 md:top-16"
      style="transform-origin: top left"
    >
      <div class="flex items-center gap-3 border-b border-hairline px-4 py-3">
        <p class="micro flex-1 text-text-faint">WHO IS USING CASTLE</p>
        <button class="expo t-hover p-1 text-text-dim transition-colors hover:text-white" onclick={close} aria-label="Close">
          <X class="h-4 w-4" strokeWidth={1.75} />
        </button>
      </div>

      <ul class="p-2">
        {#each shell.users as person (person.id)}
          {@const current = person.id === shell.user.id}
          <li>
            <button
              class="expo t-hover flex w-full items-center gap-3 px-2 py-2 text-left transition-colors hover:bg-white/[0.05] {current ? 'bg-white/[0.04]' : ''}"
              onclick={() => pick(person)}
              aria-current={current ? 'true' : undefined}
            >
              <Avatar {person} />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-[14px] font-semibold text-white">{person.name}</span>
                <span class="micro block text-text-faint">{person.role}</span>
              </span>
              {#if current}
                <Check class="h-4 w-4 text-chiral" strokeWidth={2} />
              {:else if person.locked}
                <Lock class="h-3.5 w-3.5 text-text-faint" strokeWidth={1.75} />
              {/if}
            </button>
            {#if askingFor === person.id}
              <form class="flex items-center gap-2 px-2 pb-2 pt-1" onsubmit={submitPin}>
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  bind:value={pin}
                  type="password"
                  inputmode="numeric"
                  autocomplete="off"
                  maxlength={PIN_LENGTH}
                  autofocus
                  placeholder="PIN"
                  aria-label={`PIN for ${person.name}`}
                  class="micro h-9 min-w-0 flex-1 border border-hairline bg-white/[0.03] px-3 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none"
                />
                <button type="submit" class="micro h-9 bg-chiral px-3 font-bold text-black">UNLOCK</button>
              </form>
              {#if error}
                <p class="px-2 pb-2 text-[12px] text-chiral" role="alert">{error}</p>
              {/if}
            {/if}
          </li>
        {/each}
      </ul>

      <p class="border-t border-hairline-faint px-4 py-3 text-[12px] leading-relaxed text-text-dim">
        {#if store.isDemo}
          Demo people. Each has their own storage; locked ones open with PIN {DEMO_PIN}.
        {:else}
          This box has one person so far. Separate people with their own storage, arcade and sign-in are being tried in the
          <a class="text-bridge underline" href="/demo">demo</a> first.
        {/if}
      </p>
    </div>
  </div>
{/if}
