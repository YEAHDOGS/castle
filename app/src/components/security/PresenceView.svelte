<!-- PresenceView: Security > Presence. The radar nodes on the walls (see gatehouse/ and
     castled/src/presence.rs): each node's motion-tracker sweep, the rooms they watch,
     occupied or empty, what happened lately, and adding a node, which shows its own key
     once. Presence is not identity: it says something is there, not who. -->
<script>
  import { untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import { Radar, Plus, KeyRound, Trash2, Copy, Check, PenLine, DoorOpen, DoorClosed, CircleHelp, Wifi, WifiOff } from '@lucide/svelte';
  import { ask } from '../../lib/ask.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { itemIn } from '../../lib/motion.js';
  import RadarScope from './RadarScope.svelte';
  import WipNote from '../WipNote.svelte';

  let { source } = $props();

  const POLL_MS = 700;
  const EVENTS_SHOWN = 12;
  const SENSORS = [
    { id: 'ld2450', label: 'LD2450 (tracks positions)' },
    { id: 'ld2410', label: 'LD2410 (here or not)' },
  ];
  const EVENT_WORDS = {
    entered: (e) => `Someone in ${e.room}`,
    left: (e) => `${e.room} empty`,
    offline: (e) => `${e.nodeName} stopped reporting`,
    online: (e) => `${e.nodeName} is reporting`,
  };

  let overview = $state(null);
  let problem = $state('');
  let draft = $state({ name: '', room: '', sensor: 'ld2450' });
  let adding = $state(false);
  let shownKey = $state(null); // { name, key } just after a node is added or given a new key
  let copied = $state(false);
  let editing = $state(null); // { id, name, room }
  let busy = $state('');

  const nodes = $derived(overview?.nodes || []);
  const rooms = $derived(overview?.rooms || []);
  const online = $derived(nodes.filter((n) => n.online).length);
  const occupied = $derived(rooms.filter((r) => r.occupied).length);
  const moving = $derived(nodes.reduce((sum, n) => sum + (n.targets?.length || 0), 0));
  const knownRooms = $derived([...new Set(nodes.map((n) => n.room))]);

  function ago(at, now = overview?.now ?? Date.now()) {
    if (!at) return 'never';
    const s = Math.max(0, Math.round((now - at) / 1000));
    if (s < 3) return 'now';
    if (s < 60) return `${s} s ago`;
    if (s < 3600) return `${Math.round(s / 60)} min ago`;
    if (s < 86400) return `${Math.round(s / 3600)} h ago`;
    return new Date(at).toLocaleDateString(undefined, { dateStyle: 'medium' });
  }
  const metres = (t) => `${(Math.hypot(t.x, t.y) / 1000).toFixed(1)} m`;
  const status = (n) => (n.online ? { label: 'REPORTING', tone: 'text-live', icon: Wifi } : n.lastSeen ? { label: 'OFFLINE', tone: 'text-chiral', icon: WifiOff } : { label: 'WAITING FOR IT', tone: 'text-text-dim', icon: CircleHelp });

  async function load(from) {
    try {
      const next = await from.presence();
      if (from !== source) return;
      overview = next;
      problem = '';
    } catch (err) {
      problem = err?.message || 'Presence could not be read';
    }
  }

  $effect(() => {
    const from = source;
    if (!from) return;
    untrack(() => {
      overview = null;
      load(from);
    });
    const timer = setInterval(() => {
      if (typeof document !== 'undefined' && document.hidden) return;
      load(from);
    }, POLL_MS);
    return () => clearInterval(timer);
  });

  async function addNode() {
    adding = true;
    try {
      const { node, key } = await source.presenceNodeAdd({ name: draft.name, room: draft.room, sensor: draft.sensor });
      shownKey = { name: node.name, key };
      copied = false;
      draft = { name: '', room: draft.room, sensor: draft.sensor };
      await load(source);
    } catch (err) {
      store.toast(err?.message || 'The node was not added');
    } finally {
      adding = false;
    }
  }

  async function newKey(node) {
    const question = `Give ${node.name} a new key? Its old key stops working at once, so the node goes quiet until the new key is in its config.h.`;
    if (!(await ask(question, { yes: 'New key', danger: true }))) return;
    busy = node.id;
    try {
      const { key } = await source.presenceNodeRekey(node.id);
      shownKey = { name: node.name, key };
      copied = false;
    } catch (err) {
      store.toast(err?.message || 'No new key was made');
    } finally {
      busy = '';
    }
  }

  async function removeNode(node) {
    if (!(await ask(`Remove ${node.name}? Its key stops working, and it disappears from the rooms.`, { yes: 'Remove', danger: true }))) return;
    busy = node.id;
    try {
      await source.presenceNodeRemove(node.id);
      await load(source);
      store.toast(`${node.name} removed`);
    } catch (err) {
      store.toast(err?.message || 'The node was not removed');
    } finally {
      busy = '';
    }
  }

  async function saveEdit() {
    const { id, name, room } = editing;
    busy = id;
    try {
      await source.presenceNodeUpdate({ id, name, room });
      editing = null;
      await load(source);
    } catch (err) {
      store.toast(err?.message || 'The change was not saved');
    } finally {
      busy = '';
    }
  }

  async function copyKey() {
    try {
      await navigator.clipboard.writeText(shownKey.key);
      copied = true;
    } catch {
      store.toast('Copy it by hand: the clipboard is blocked here');
    }
  }
</script>

{#snippet stat(label, value, tone)}
  <div class="glass p-4">
    <p class="micro text-text-faint">{label}</p>
    <p class="mt-2 text-[34px] font-black leading-none tabular-nums {tone}">{value}</p>
  </div>
{/snippet}

<WipNote title="Presence radar">
  The radar nodes have not run on real hardware yet. The firmware is written for an ESP32
  with an LD2450 sensor, but no node has been built and mounted on a wall. The sweep here
  shows sample data.
</WipNote>

{#if problem && !overview}
  <div class="glass p-4 text-[13px] text-text-dim">{problem}</div>
{:else if !overview}
  <div class="glass p-4 text-[13px] text-text-dim">Reading the nodes…</div>
{:else}
  <div class="grid gap-3 sm:grid-cols-3">
    {@render stat('ROOMS OCCUPIED', `${occupied}/${rooms.length}`, occupied ? 'text-chiral' : 'text-white')}
    {@render stat('NODES REPORTING', `${online}/${nodes.length}`, online < nodes.length ? 'text-chiral' : 'text-live')}
    {@render stat('MOVING NOW', moving, moving ? 'text-chiral' : 'text-white')}
  </div>

  {#if shownKey}
    <div in:fly={itemIn(0)} class="hud-corners glass mt-3 p-4">
      <p class="micro flex items-center gap-2 text-chiral"><KeyRound class="h-4 w-4" />THE KEY FOR {shownKey.name.toUpperCase()}: SHOWN ONCE</p>
      <p class="mt-1.5 max-w-3xl text-[13px] leading-relaxed text-text-dim">
        Put it in the node's <span class="font-mono text-white">config.h</span> as <span class="font-mono text-white">NODE_KEY</span>, then flash the node
        (see gatehouse/README.md). The box keeps only a fingerprint of it, so it cannot show it again. This key lets the node report what it sees and
        nothing else.
      </p>
      <div class="mt-3 flex flex-wrap items-center gap-2">
        <code class="min-w-0 flex-1 select-all break-all border border-hairline bg-black px-3 py-2 font-mono text-[12px] text-white">{shownKey.key}</code>
        <button class="micro expo t-hover flex h-9 items-center gap-1.5 border border-hairline px-3 text-text-dim hover:text-white" onclick={copyKey}>
          {#if copied}<Check class="h-3.5 w-3.5 text-live" />COPIED{:else}<Copy class="h-3.5 w-3.5" />COPY{/if}
        </button>
        <button class="micro expo t-hover h-9 bg-chiral px-4 font-bold text-black hover:opacity-90" onclick={() => (shownKey = null)}>DONE</button>
      </div>
    </div>
  {/if}

  {#if nodes.length}
    <ul class="mt-3 grid gap-3 md:grid-cols-2 2xl:grid-cols-3">
      {#each nodes as node, i (node.id)}
        {@const look = status(node)}
        <li in:fly={itemIn(i)} class="flex flex-col border bg-white/[0.02] {node.present ? 'border-chiral/50' : 'border-hairline'}">
          <div class="flex items-start gap-3 px-4 pt-3">
            <div class="min-w-0 flex-1">
              {#if editing?.id === node.id}
                <div class="flex gap-2">
                  <input bind:value={editing.name} class="micro h-8 min-w-0 flex-1 border border-hairline bg-white/[0.03] px-2 normal-case text-white focus:border-chiral/60 focus:outline-none" aria-label="Node name" />
                  <input bind:value={editing.room} list="presence-rooms" class="micro h-8 min-w-0 flex-1 border border-hairline bg-white/[0.03] px-2 normal-case text-white focus:border-chiral/60 focus:outline-none" aria-label="Room" />
                </div>
              {:else}
                <h3 class="truncate text-[15px] font-bold text-white">{node.name}</h3>
                <p class="micro mt-0.5 truncate text-text-faint">{node.room.toUpperCase()} · {node.sensor.toUpperCase()}</p>
              {/if}
            </div>
            <span class="micro flex shrink-0 items-center gap-1.5 {look.tone}"><look.icon class="h-3.5 w-3.5" />{look.label}</span>
          </div>

          {#if node.sensor === 'ld2450'}
            <div class="mx-4 mt-2 aspect-[2/1.15]">
              <RadarScope targets={node.targets} online={node.online} label={node.name} />
            </div>
          {:else}
            <div class="mx-4 mt-2 grid aspect-[2/1.15] place-items-center border border-hairline-faint">
              <div class="text-center">
                <span class="mx-auto block h-10 w-10 rounded-full {node.present ? 'bg-chiral shadow-[0_0_30px_rgba(255,106,0,0.6)]' : 'border border-hairline'}"></span>
                <p class="micro mt-3 {node.present ? 'text-chiral' : 'text-text-faint'}">{node.online ? (node.present ? 'SOMEONE HERE' : 'NOBODY') : 'NOT REPORTING'}</p>
              </div>
            </div>
          {/if}

          <div class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 px-4 text-[12px] text-text-dim">
            {#if node.online && node.targets.length}
              <span class="text-chiral">{node.targets.length} seen · {node.targets.map(metres).join(', ')}</span>
            {:else if node.online}
              <span>{node.present ? 'Someone here' : 'Nothing moving'}</span>
            {/if}
            <span class="text-text-faint">Last report {ago(node.lastSeen)}</span>
          </div>

          <div class="mt-3 flex gap-1 border-t border-hairline-faint px-2 py-2">
            {#if editing?.id === node.id}
              <button class="micro expo t-hover h-8 bg-chiral px-3 font-bold text-black disabled:opacity-50" onclick={saveEdit} disabled={busy === node.id}>SAVE</button>
              <button class="micro expo t-hover h-8 px-3 text-text-dim hover:text-white" onclick={() => (editing = null)}>CANCEL</button>
            {:else}
              <button class="micro expo t-hover flex h-8 items-center gap-1.5 px-2.5 text-text-dim hover:text-white" onclick={() => (editing = { id: node.id, name: node.name, room: node.room })}><PenLine class="h-3.5 w-3.5" />RENAME</button>
              <button class="micro expo t-hover flex h-8 items-center gap-1.5 px-2.5 text-text-dim hover:text-white disabled:opacity-50" onclick={() => newKey(node)} disabled={busy === node.id}><KeyRound class="h-3.5 w-3.5" />NEW KEY</button>
              <button class="micro expo t-hover ml-auto flex h-8 items-center gap-1.5 px-2.5 text-text-dim hover:text-chiral disabled:opacity-50" onclick={() => removeNode(node)} disabled={busy === node.id}><Trash2 class="h-3.5 w-3.5" />REMOVE</button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {:else}
    <div class="glass mt-3 flex gap-3 p-4">
      <Radar class="mt-0.5 h-5 w-5 shrink-0 text-bridge" />
      <p class="text-[13px] leading-relaxed text-text-dim">
        No nodes yet. A node is an ESP32 and a $10 radar sensor by a doorway or in a room; it tells the box when something moves, through
        wood and drywall, with no camera. Add one below, then flash it with the sketch in gatehouse/.
      </p>
    </div>
  {/if}

  {#if rooms.length}
    <div class="mt-3 grid gap-3 lg:grid-cols-2">
      <section class="glass p-4" aria-labelledby="rooms-heading">
        <h2 id="rooms-heading" class="micro text-text-faint">ROOMS</h2>
        <ul class="mt-2">
          {#each rooms as room (room.name)}
            <li class="flex items-center gap-3 border-t border-hairline-faint py-2 first:border-t-0">
              {#if !room.watched}
                <CircleHelp class="h-4 w-4 shrink-0 text-text-faint" />
              {:else if room.occupied}
                <DoorOpen class="h-4 w-4 shrink-0 text-chiral" />
              {:else}
                <DoorClosed class="h-4 w-4 shrink-0 text-text-dim" />
              {/if}
              <span class="min-w-0 flex-1 truncate text-[14px] text-white">{room.name}</span>
              <span class="micro {room.watched ? (room.occupied ? 'text-chiral' : 'text-text-dim') : 'text-text-faint'}">
                {room.watched ? (room.occupied ? 'OCCUPIED' : 'EMPTY') : 'NO NODE REPORTING'}
              </span>
              <span class="micro w-24 text-right text-text-faint">{room.since ? ago(room.since).toUpperCase() : ''}</span>
            </li>
          {/each}
        </ul>
      </section>
      <section class="glass p-4" aria-labelledby="events-heading">
        <h2 id="events-heading" class="micro text-text-faint">LATELY</h2>
        <ul class="mt-2">
          {#each overview.events.slice(0, EVENTS_SHOWN) as event (`${event.at}:${event.kind}:${event.room}:${event.node}`)}
            <li class="flex items-baseline gap-3 border-t border-hairline-faint py-1.5 text-[13px] first:border-t-0">
              <span class="min-w-0 flex-1 truncate {event.kind === 'entered' ? 'text-chiral' : event.kind === 'offline' ? 'text-text-dim' : 'text-white'}">{(EVENT_WORDS[event.kind] || ((e) => e.kind))(event)}</span>
              <span class="micro shrink-0 text-text-faint">{ago(event.at).toUpperCase()}</span>
            </li>
          {:else}
            <li class="py-2 text-[13px] text-text-dim">Nothing yet since the box started.</li>
          {/each}
        </ul>
      </section>
    </div>
  {/if}

  <section class="hud-corners glass mt-3 px-5 py-5" aria-labelledby="add-node-heading">
    <h2 id="add-node-heading" class="micro flex items-center gap-2 text-chiral"><Plus class="h-4 w-4" />ADD A NODE</h2>
    <p class="mt-1.5 max-w-3xl text-[13px] text-text-dim">
      The box makes the node its own key, shown once. The box also has to listen on the home network for a node to reach it (see gatehouse/README.md).
    </p>
    <form class="mt-4 grid gap-2 md:grid-cols-[1fr_1fr_auto_auto]" onsubmit={(e) => (e.preventDefault(), addNode())}>
      <input bind:value={draft.name} placeholder="NAME, e.g. FRONT DOOR" class="micro h-10 min-w-0 border border-hairline bg-white/[0.03] px-3 normal-case text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Node name" />
      <input bind:value={draft.room} list="presence-rooms" placeholder="ROOM, e.g. HALL" class="micro h-10 min-w-0 border border-hairline bg-white/[0.03] px-3 normal-case text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Room" />
      <select bind:value={draft.sensor} class="micro h-10 border border-hairline bg-black px-2 text-white" aria-label="Sensor">
        {#each SENSORS as s (s.id)}<option value={s.id}>{s.label.toUpperCase()}</option>{/each}
      </select>
      <button type="submit" class="micro expo t-hover h-10 bg-chiral px-4 font-bold text-black transition-opacity hover:opacity-90 disabled:opacity-50" disabled={adding || !draft.name.trim() || !draft.room.trim()}>
        {adding ? 'ADDING…' : 'ADD'}
      </button>
    </form>
    <datalist id="presence-rooms">
      {#each knownRooms as room (room)}<option value={room}></option>{/each}
    </datalist>
  </section>

  <p class="mt-4 max-w-3xl text-[12px] leading-relaxed text-text-dim">
    Presence is not identity: a node says something alive is there, not who, and a big dog looks like a person. Fans, curtains and pets show up
    until the sensor is tuned. Radar passes wood and drywall but not much brick or concrete, so this is an indoor sense, not a fence. Readings
    are kept in memory only; a restart forgets where everyone was.
  </p>
{/if}
