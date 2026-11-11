<!-- PeopleList: everyone on this box, the current person ticked. Picking one switches to
     them; a locked person asks for their PIN in place first. Used in the left rail on Home
     and in the people menu elsewhere. -->
<script>
  import { Lock, Check } from '@lucide/svelte';
  import { slide } from 'svelte/transition';
  import { shell } from '../lib/shell.svelte.js';
  import { store } from '../lib/store.svelte.js';
  import { DEMO_PIN } from '../lib/demo-shell.js';
  import { DUR, expoOut, reducedMotion } from '../lib/motion.js';
  import Avatar from './Avatar.svelte';

  /** ondone: called after a switch, or when the current person is picked again. */
  let { ondone = () => {} } = $props();

  const PIN_LENGTH = 4;

  let askingFor = $state(null); // id of the locked person whose PIN is being typed
  let pin = $state('');
  let error = $state('');

  const reveal = () => ({ duration: reducedMotion() ? 0 : DUR.UI, easing: expoOut });

  function reset() {
    askingFor = null;
    pin = '';
    error = '';
  }

  async function pick(person) {
    if (person.id === shell.user.id) {
      reset();
      return ondone();
    }
    if (person.locked) {
      // Picking the same locked person again closes their PIN box.
      askingFor = askingFor === person.id ? null : person.id;
      pin = '';
      error = '';
      return;
    }
    await shell.switchUser(person.id);
    reset();
    ondone();
  }

  async function submitPin(e) {
    e.preventDefault();
    error = (await shell.switchUser(askingFor, pin)) || '';
    if (error) {
      pin = '';
      return;
    }
    reset();
    ondone();
  }
</script>

<ul>
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
        <div transition:slide={reveal()}>
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
        </div>
      {/if}
    </li>
  {/each}
</ul>

<p class="mt-2 px-2 text-[12px] leading-relaxed text-text-dim">
  {#if store.isDemo}
    Demo people. Each has their own storage; locked ones open with PIN {DEMO_PIN}.
  {:else}
    This box has one person so far. Separate people with their own storage and sign-in are being tried in Demo first.
  {/if}
</p>
