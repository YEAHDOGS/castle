<!-- CommsSection: each person's texts, picture messages, calls, email and chats, kept on
     the box. Nothing here talks to a phone company or a mail server: a person exports what
     their phone or service has and hands the file to the box (Import), which reads it and
     keeps every message (castled/src/comms.rs). The same export twice adds nothing.

     The rail has All, one row per channel that has anything (texts, calls, email, Discord,
     Messenger), and Import. A conversation is one person or group on one channel; the
     person's card from Contacts gives it a name and a face, matched by number, address or
     name (lib/people.js). The thread header goes to that card, or offers to make one.

     Contacts can send someone here to show only their conversations (shell.focus).

     With a controller: the D-pad walks the list, A opens a conversation, B goes back, LB
     and RB change channel. -->
<script>
  import {
    MessagesSquare, MessageSquareText, Phone, PhoneIncoming, PhoneOutgoing, PhoneMissed, PhoneOff, Voicemail, Mail, Hash, MessageCircle,
    Upload, Search, ChevronLeft, Trash2, LoaderCircle, Paperclip, UserPlus, BookUser, X, FileDown, CircleAlert, History, Mic, Lock,
  } from '@lucide/svelte';
  import { untrack, tick } from 'svelte';
  import { fly, fade } from 'svelte/transition';
  import { SvelteMap, SvelteSet } from 'svelte/reactivity';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { ask } from '../../lib/ask.svelte.js';
  import { setHints, focusSoon } from '../../lib/input.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import { peopleIndex, isWith, initials } from '../../lib/people.js';
  import { itemIn } from '../../lib/motion.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';

  const ALL = '@all';
  const IMPORT = '@import';
  const POLL_MS = 2000;
  const SEARCH_WAIT_MS = 300;
  const STAGGER_CAP = 20;
  /** An email longer than this opens folded. */
  const FOLD_AT = 700;
  const FILE_MARGIN = '300px';
  const CHANNELS = [
    { id: 'texts', label: 'Texts', icon: MessageSquareText },
    { id: 'calls', label: 'Calls', icon: Phone },
    { id: 'email', label: 'Email', icon: Mail },
    { id: 'discord', label: 'Discord', icon: Hash },
    { id: 'facebook', label: 'Messenger', icon: MessageCircle },
  ];
  const channelOf = (id) => CHANNELS.find((c) => c.id === id) || CHANNELS[0];
  const CALL_ICONS = { incoming: PhoneIncoming, outgoing: PhoneOutgoing, missed: PhoneMissed, voicemail: Voicemail, rejected: PhoneOff, blocked: PhoneOff };
  const SOURCES = [
    {
      title: 'Android texts, picture messages and calls',
      steps: 'Install SMS Backup & Restore (free, by SyncTech). Back up Messages and Call logs to a file on the phone, then upload sms-….xml and calls-….xml here. Pictures in picture messages come too.',
    },
    { title: 'Gmail and other email', steps: 'Google Takeout → Mail gives an .mbox file; upload it, or the whole Takeout zip. Thunderbird and Apple Mail export .mbox as well, and single .eml files work.' },
    { title: 'Facebook Messenger', steps: 'Facebook → Settings → Download your information → Messages, in JSON format. Upload the zip as it is: the pictures inside come with their messages.' },
    { title: 'Discord', steps: 'DiscordChatExporter (JSON format) saves whole channels and DMs. Discord’s own data package (Privacy settings → Request data) works too, but holds only what you wrote.' },
    { title: 'Call recordings', steps: 'Upload the audio files your phone’s recorder made. A name with the number and the time (+15550102030_20261113142233.m4a) puts each one on its call.' },
  ];

  let overview = $state(null); // { conversations, imports, importing, problem }
  let contacts = $state([]);
  let problem = $state('');
  let view = $state(ALL);
  let openId = $state(null);
  let thread = $state(null); // { conversation, messages, more }
  let threadLoading = $state(false);
  let olderLoading = $state(false);
  let jumped = $state(false); // showing messages around a search hit, not the newest
  let highlight = $state(null); // the message a search hit pointed at
  let query = $state('');
  let hits = $state(null); // search results, or null when not searching
  let searching = $state(false);
  let withContact = $state(null); // the card Contacts sent us here with
  let sending = $state(null); // { name, at, of } while uploads go one by one
  let dragging = $state(false);
  let fileInput = $state(null);
  let scroller = $state(null);
  let content = $state(null);
  /** Keep the newest message in view while pictures above it load: true until the person scrolls up. */
  let stick = true;
  const files = new SvelteMap(); // attachment file name -> address, or null when it cannot be had
  const photos = new SvelteMap(); // contact id -> picture address, or null
  const unfolded = new SvelteSet(); // emails opened past their fold

  const source = $derived(shell.commsSource);
  const people = $derived(shell.contactsSource);
  const person = $derived(shell.user.id);
  const index = $derived(peopleIndex(contacts));
  const conversations = $derived((overview?.conversations || []).filter((c) => !withContact || isWith(c, withContact)));
  const counts = $derived(Object.fromEntries(CHANNELS.map((ch) => [ch.id, conversations.filter((c) => c.channel === ch.id).length])));
  const listed = $derived(view === ALL || view === IMPORT ? conversations : conversations.filter((c) => c.channel === view));
  const byId = $derived(new Map((overview?.conversations || []).map((c) => [c.id, c])));
  const railItems = $derived([
    { id: ALL, label: 'All', icon: MessagesSquare, count: conversations.length },
    ...CHANNELS.filter((ch) => counts[ch.id]).map((ch) => ({ id: ch.id, label: ch.label, icon: ch.icon, count: counts[ch.id] })),
    { id: IMPORT, label: 'Import', icon: Upload, count: overview?.importing ? '…' : null },
  ]);
  const subtitle = $derived(
    overview
      ? `${overview.conversations.length} ${overview.conversations.length === 1 ? 'conversation' : 'conversations'}${overview.importing ? ` · importing ${overview.importing.source}…` : ''}`
      : ''
  );
  const open = $derived(openId ? byId.get(openId) || thread?.conversation || null : null);
  const openPeople = $derived(open ? index.forConversation(open) : []);
  const grouped = $derived(open && (open.participants.length > 1 || open.channel === 'discord'));

  // ---------------------------------------------------------------- loading

  async function load(from = source, who = person) {
    problem = '';
    if (!from) {
      overview = null;
      return;
    }
    try {
      const next = await from.comms(who);
      if (from !== source || who !== person) return;
      overview = next;
    } catch (err) {
      problem = err?.message || 'Comms could not be read';
    }
  }

  async function loadContacts(from = people, who = person) {
    if (!from?.contacts) {
      contacts = [];
      return;
    }
    try {
      const list = await from.contacts(who);
      if (from === people && who === person) contacts = list || [];
    } catch {
      contacts = []; // no cards just means no names and faces
    }
  }

  // A new source or a new person starts again.
  $effect(() => {
    const from = source;
    const who = person;
    untrack(() => {
      files.clear();
      photos.clear();
      unfolded.clear();
      openId = null;
      thread = null;
      hits = null;
      query = '';
      overview = null;
      load(from, who);
      loadContacts(people, who);
    });
  });

  // Contacts sent someone here: show only their conversations.
  $effect(() => {
    const focus = shell.focus;
    if (focus?.section !== 'comms') return;
    untrack(() => {
      withContact = focus.contact || null;
      view = ALL;
      openId = null;
      thread = null;
      shell.focus = null;
    });
  });

  // While the box reads an import, check back until it is done.
  $effect(() => {
    if (!overview?.importing) return;
    const from = source;
    const who = person;
    const timer = setInterval(async () => {
      try {
        const next = await from.comms(who);
        if (from !== source || who !== person) return;
        const finished = !next.importing;
        overview = next;
        if (finished) {
          clearInterval(timer);
          announce(next);
          loadContacts(people, who); // a contacts file inside went to Contacts
          if (openId) openConversation(openId, { quiet: true });
        }
      } catch {
        /* the next check will say */
      }
    }, POLL_MS);
    return () => clearInterval(timer);
  });

  function announce(next) {
    if (next.problem) return store.toast(next.problem);
    const last = next.imports?.[0];
    if (!last) return;
    const r = last.report;
    const found = [plural(r.texts, 'text'), plural(r.calls, 'call'), plural(r.emails, 'email'), plural(r.chats, 'chat message'), plural(r.recordings, 'recording')].filter(Boolean);
    store.toast(found.length ? `${last.source}: ${found.join(', ')} added` : `${last.source}: nothing new${r.known ? ` (${r.known.toLocaleString()} already kept)` : ''}`);
  }

  // ---------------------------------------------------------------- a conversation

  async function openConversation(id, { at = null, quiet = false } = {}) {
    const from = source;
    const who = person;
    if (!quiet) {
      openId = id;
      thread = null;
      jumped = false;
      highlight = null;
      threadLoading = true;
    }
    try {
      let next = await from.commsThread(who, id);
      // A search hit older than the newest page: show the page that ends with it.
      if (at && next.messages.length && at < next.messages[0].at) {
        next = await from.commsThread(who, id, at + 1);
        jumped = true;
      }
      if (from !== source || who !== person || openId !== id) return;
      thread = next;
      threadLoading = false;
      await tick();
      stick = !at && !quiet;
      if (at) {
        scroller?.querySelector(`[data-message-at="${at}"]`)?.scrollIntoView({ block: 'center' });
      } else if (scroller && !quiet) {
        scroller.scrollTop = scroller.scrollHeight;
      }
    } catch (err) {
      threadLoading = false;
      if (!quiet) store.toast(err?.message || 'That conversation could not be read');
    }
  }

  // Pictures arrive after the thread is shown and push the newest messages down: follow them.
  $effect(() => {
    const box = content;
    if (!box || typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(() => {
      if (stick && scroller) scroller.scrollTop = scroller.scrollHeight;
    });
    watch.observe(box);
    return () => watch.disconnect();
  });

  /** When the person last moved the thread themselves: only their scrolling unsticks it. */
  let touched = 0;
  const touch = () => (touched = Date.now());

  function onScroll() {
    if (scroller && Date.now() - touched < 1000) stick = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 48;
  }

  async function loadOlder() {
    if (!thread?.more || olderLoading) return;
    olderLoading = true;
    stick = false;
    const keep = scroller ? scroller.scrollHeight - scroller.scrollTop : 0;
    try {
      const older = await source.commsThread(person, openId, thread.messages[0].at);
      thread = { ...thread, messages: [...older.messages, ...thread.messages], more: older.more };
      await tick();
      if (scroller) scroller.scrollTop = scroller.scrollHeight - keep;
    } catch (err) {
      store.toast(err?.message || 'Older messages could not be read');
    }
    olderLoading = false;
  }

  function closeConversation() {
    const was = openId;
    openId = null;
    thread = null;
    if (was) focusSoon(`[data-nav-key="${CSS.escape(was)}"]`);
  }

  async function forget(conv) {
    const yes = await ask(`Forget the conversation with ${conv.title}? Its ${plural(conv.count, 'message')} and their pictures are removed from the box. A later import of the same backup brings them back.`, { yes: 'Forget it', danger: true });
    if (!yes) return;
    try {
      await source.commsDelete(person, conv.id);
      closeConversation();
      await load();
      store.toast(`${conv.title} forgotten`);
    } catch (err) {
      store.toast(err?.message || 'That could not be forgotten');
    }
  }

  function openCard(conv) {
    const found = index.forConversation(conv);
    if (found.length) return shell.openWith('contacts', { contactId: found[0].id });
    const address = conv.participants[0] || '';
    const draft = { name: conv.title === address ? '' : conv.title, phones: [], emails: [] };
    if (address.includes('@')) draft.emails.push({ label: '', value: address });
    else if (/\d{5,}/.test(address.replace(/\D/g, ''))) draft.phones.push({ label: 'Mobile', value: address });
    shell.openWith('contacts', { draft });
  }

  // ---------------------------------------------------------------- search

  $effect(() => {
    const q = query.trim();
    const from = source;
    const who = person;
    if (!q || !from) {
      hits = null;
      searching = false;
      return;
    }
    searching = true;
    const timer = setTimeout(async () => {
      try {
        const found = await from.commsSearch(who, q);
        if (query.trim() === q) hits = found;
      } catch (err) {
        store.toast(err?.message || 'Search did not work');
      }
      searching = false;
    }, SEARCH_WAIT_MS);
    return () => clearTimeout(timer);
  });

  function openHit(hit) {
    if (view === IMPORT) view = ALL;
    openConversation(hit.conversation, { at: hit.message.at });
    highlight = hit.message.id;
  }

  // ---------------------------------------------------------------- import

  async function send(list) {
    const queue = [...list].filter(Boolean);
    if (!queue.length || !source) return;
    for (let i = 0; i < queue.length; i++) {
      const file = queue[i];
      sending = { name: file.name, at: i + 1, of: queue.length };
      try {
        await source.commsImport(person, file);
        await load();
        // The box reads one import at a time: wait for this one before sending the next.
        while (overview?.importing) {
          await new Promise((r) => setTimeout(r, POLL_MS));
          await load();
        }
      } catch (err) {
        store.toast(err?.message || `${file.name} could not be sent`);
        break;
      }
    }
    sending = null;
    if (fileInput) fileInput.value = '';
  }

  function onDrop(e) {
    e.preventDefault();
    dragging = false;
    send(e.dataTransfer?.files || []);
  }

  // ---------------------------------------------------------------- pictures and files

  /** Fetches an attachment when it comes near the screen. */
  function lazyFile(node, name) {
    const fetchIt = async () => {
      if (files.has(name) || !source) return;
      files.set(name, undefined);
      try {
        files.set(name, (await source.commsFile(person, name)) || null);
      } catch {
        files.set(name, null);
      }
    };
    if (typeof IntersectionObserver === 'undefined') {
      fetchIt();
      return {};
    }
    const seen = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        seen.disconnect();
        fetchIt();
      }
    }, { rootMargin: FILE_MARGIN });
    seen.observe(node);
    return { destroy: () => seen.disconnect() };
  }

  async function download(a) {
    let url = files.get(a.file);
    if (!url) {
      try {
        url = await source.commsFile(person, a.file);
        files.set(a.file, url || null);
      } catch (err) {
        return store.toast(err?.message || 'That file could not be had');
      }
    }
    if (!url) return store.toast('That file is not on the box');
    const link = document.createElement('a');
    link.href = url;
    link.download = a.name || a.file;
    link.click();
  }

  /** A person's picture from their card, once it has been fetched (see below). */
  const photoOf = (contact) => (contact?.photo ? photos.get(contact.id) || null : null);

  // Fetches the pictures of the people in the conversations on screen, once each.
  $effect(() => {
    const from = people;
    const who = person;
    const wanted = new Set();
    for (const conv of [...listed, ...(open ? [open] : [])]) for (const c of index.forConversation(conv)) if (c.photo) wanted.add(c);
    if (!from?.contactPhoto) return;
    untrack(() => {
      for (const c of wanted) {
        if (photos.has(c.id)) continue;
        photos.set(c.id, null);
        from
          .contactPhoto(who, c.id, c.photoAt)
          .then((url) => photos.set(c.id, url || null))
          .catch(() => {});
      }
    });
  });

  // ---------------------------------------------------------------- words

  function plural(n, word) {
    return n ? `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}` : '';
  }

  function duration(s) {
    if (!s) return '';
    if (s >= 3600) return `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
    if (s >= 60) return `${Math.floor(s / 60)} min ${s % 60} s`;
    return `${s} s`;
  }

  /** "14:32", "Tue", "3 Nov", "3 Nov 2024": when, as short as it can be. */
  function when(ms) {
    if (!ms) return '';
    const at = new Date(ms);
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    if (ms >= today) return at.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    if (ms >= today - 6 * 864e5) return at.toLocaleDateString(undefined, { weekday: 'short' });
    return at.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: at.getFullYear() === now.getFullYear() ? undefined : 'numeric' });
  }

  const clock = (ms) => new Date(ms).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });

  function dayOf(ms) {
    const at = new Date(ms);
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    if (ms >= today) return 'Today';
    if (ms >= today - 864e5) return 'Yesterday';
    return at.toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: at.getFullYear() === now.getFullYear() ? undefined : 'numeric' });
  }

  /** Initials from the words that have letters: none for a bare number. */
  const badge = (name) => {
    const words = String(name || '').split(/[\s·]+/).map((w) => w.replace(/^[^\p{L}]+/u, '')).filter((w) => /\p{L}/u.test(w));
    return words.length ? initials(words.join(' ')) : '';
  };

  const sameDay = (a, b) => new Date(a).toDateString() === new Date(b).toDateString();

  function callWords(m) {
    const d = m.call?.direction || 'incoming';
    const words = { incoming: 'Incoming call', outgoing: 'Outgoing call', missed: 'Missed call', voicemail: 'Voicemail', rejected: 'Declined call', blocked: 'Blocked call' };
    return words[d] || 'Call';
  }

  function pickRail(id) {
    view = id;
    if (id === IMPORT) {
      openId = null;
      thread = null;
    }
  }

  // ---------------------------------------------------------------- controller hints

  $effect(() => {
    if (openId) {
      return setHints('comms', [
        { btn: 'A', label: 'Select' },
        { btn: 'B', label: 'Back' },
      ]);
    }
    return setHints('comms', [
      { btn: 'A', label: 'Open' },
      { btn: 'LB/RB', label: 'Channel' },
      { btn: 'START', label: 'Sections' },
    ]);
  });
</script>

{#snippet avatar(conv, size = 'h-10 w-10')}
  {@const found = index.forConversation(conv)}
  {@const face = found.length === 1 ? photoOf(found[0]) : null}
  {@const Icon = channelOf(conv.channel).icon}
  <span class="relative grid {size} shrink-0 place-items-center overflow-hidden rounded-full border border-hairline bg-white/[0.04] text-[12px] font-bold text-white">
    {#if face}
      <img src={face} alt="" class="h-full w-full object-cover" draggable="false" />
    {:else}
      {#if badge(found[0]?.name || conv.title)}{badge(found[0]?.name || conv.title)}{:else}<Phone class="h-4 w-4 text-text-dim" />{/if}
    {/if}
    <span class="absolute -bottom-0.5 -right-0.5 grid h-4 w-4 place-items-center rounded-full border border-black bg-panel-solid">
      <Icon class="h-2.5 w-2.5 text-text-dim" />
    </span>
  </span>
{/snippet}

{#snippet attachmentView(a, mine)}
  {#if a.mime.startsWith('image/')}
    <div class="mt-1.5 overflow-hidden border border-hairline bg-black/30" use:lazyFile={a.file}>
      {#if files.get(a.file)}
        <button class="block" onclick={() => download(a)} title={`Save ${a.name}`}>
          <img src={files.get(a.file)} alt={a.name} class="max-h-72 w-full max-w-sm object-cover" loading="lazy" draggable="false" />
        </button>
      {:else}
        <div class="grid h-40 w-64 place-items-center text-text-faint">
          {#if files.get(a.file) === null}<span class="micro">PICTURE NOT ON THE BOX</span>{:else}<LoaderCircle class="h-4 w-4 animate-spin" />{/if}
        </div>
      {/if}
    </div>
  {:else if a.mime.startsWith('audio/')}
    <div class="mt-1.5 flex items-center gap-2" use:lazyFile={a.file}>
      <Mic class="h-3.5 w-3.5 shrink-0 text-chiral" />
      {#if files.get(a.file)}
        <audio controls preload="none" src={files.get(a.file)} class="h-8 max-w-[260px]"></audio>
      {:else}
        <span class="micro text-text-faint">{files.get(a.file) === null ? 'RECORDING NOT ON THE BOX' : 'LOADING RECORDING…'}</span>
      {/if}
    </div>
  {:else if a.mime.startsWith('video/')}
    <div class="mt-1.5" use:lazyFile={a.file}>
      <!-- svelte-ignore a11y_media_has_caption (a video sent from a phone has no captions to give) -->
      {#if files.get(a.file)}<video controls preload="metadata" src={files.get(a.file)} class="max-h-72 max-w-sm border border-hairline"></video>{/if}
    </div>
  {:else}
    <button class="micro expo t-hover mt-1.5 flex items-center gap-1.5 border px-2 py-1.5 transition-colors {mine ? 'border-black/20 hover:bg-black/10' : 'border-hairline hover:bg-white/[0.05]'}" onclick={() => download(a)}>
      <FileDown class="h-3.5 w-3.5" />
      <span class="max-w-[220px] truncate normal-case">{a.name}</span>
      <span class="opacity-60">{fmtSize(a.size)}</span>
    </button>
  {/if}
{/snippet}

{#snippet messageView(m, i, list)}
  {@const fresh = i === 0 || !sameDay(list[i - 1].at, m.at)}
  {#if fresh}
    <div class="micro my-4 text-center text-text-faint">{dayOf(m.at).toUpperCase()}</div>
  {/if}
  {#if m.kind === 'call' || m.kind === 'recording'}
    {@const Icon = m.kind === 'recording' ? Mic : CALL_ICONS[m.call?.direction] || Phone}
    <div class="my-2 flex justify-center" data-message-at={m.at}>
      <div class="flex max-w-md items-start gap-3 border px-3 py-2 {highlight === m.id ? 'border-bridge/70' : 'border-hairline'} bg-white/[0.02]">
        <Icon class="mt-0.5 h-4 w-4 shrink-0 {m.call?.direction === 'missed' || m.call?.direction === 'rejected' ? 'text-chiral' : 'text-text-dim'}" />
        <div class="min-w-0">
          <p class="text-[13px] text-white">{m.kind === 'recording' ? 'Call recording' : callWords(m)}{m.call?.seconds ? ` · ${duration(m.call.seconds)}` : ''}</p>
          <p class="micro mt-0.5 text-text-faint">{clock(m.at)}{m.kind === 'recording' && m.body ? ` · ${m.body}` : ''}</p>
          {#each m.attachments as a, ai (ai)}{@render attachmentView(a, false)}{/each}
        </div>
      </div>
    </div>
  {:else if m.kind === 'email'}
    {@const long = m.body.length > FOLD_AT && !unfolded.has(m.id)}
    <article class="my-3 border {highlight === m.id ? 'border-bridge/70' : m.me ? 'border-chiral/30' : 'border-hairline'} bg-white/[0.02] {m.me ? 'ml-6 md:ml-16' : 'mr-6 md:mr-16'}" data-message-at={m.at}>
      <header class="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 border-b border-hairline-faint px-4 py-2.5">
        <h3 class="min-w-0 flex-1 text-[14px] font-semibold text-white">{m.subject || '(no subject)'}</h3>
        <span class="micro text-text-faint">{m.me ? 'YOU' : m.from.toUpperCase()} · {clock(m.at)}</span>
      </header>
      <div class="whitespace-pre-wrap break-words px-4 py-3 text-[13px] leading-relaxed text-text-dim">{long ? `${m.body.slice(0, FOLD_AT)}…` : m.body}</div>
      {#if m.body.length > FOLD_AT}
        <button class="micro expo t-hover mx-4 mb-3 text-bridge hover:underline" onclick={() => (unfolded.has(m.id) ? unfolded.delete(m.id) : unfolded.add(m.id))}>
          {long ? 'SHOW ALL OF IT' : 'FOLD IT AGAIN'}
        </button>
      {/if}
      {#if m.attachments.length}
        <div class="flex flex-wrap gap-2 px-4 pb-3">{#each m.attachments as a, ai (ai)}{@render attachmentView(a, false)}{/each}</div>
      {/if}
    </article>
  {:else}
    <div class="my-1 flex {m.me ? 'justify-end' : 'justify-start'}" data-message-at={m.at}>
      <div class="max-w-[78%] md:max-w-[62%]">
        {#if grouped && !m.me && (i === 0 || list[i - 1].from !== m.from || fresh)}
          <p class="micro mb-0.5 ml-1 text-text-faint">{m.from}</p>
        {/if}
        <div
          class="px-3 py-2 text-[14px] leading-snug {m.me ? 'rounded-2xl rounded-br-sm bg-chiral text-black' : 'rounded-2xl rounded-bl-sm border border-hairline bg-white/[0.05] text-white'} {highlight === m.id ? 'ring-2 ring-bridge' : ''}"
          title={new Date(m.at).toLocaleString()}
        >
          {#if m.body}<p class="whitespace-pre-wrap break-words">{m.body}</p>{/if}
          {#each m.attachments as a, ai (ai)}{@render attachmentView(a, m.me)}{/each}
        </div>
        {#if i === list.length - 1 || list[i + 1].me !== m.me || !sameDay(list[i + 1].at, m.at) || list[i + 1].at - m.at > 30 * 60 * 1000}
          <p class="micro mt-0.5 {m.me ? 'mr-1 text-right' : 'ml-1'} text-text-faint">{clock(m.at)}</p>
        {/if}
      </div>
    </div>
  {/if}
{/snippet}

{#snippet actions()}
  <label class="relative hidden w-64 md:block">
    <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
    <input
      type="search"
      bind:value={query}
      placeholder="SEARCH EVERY MESSAGE"
      class="micro h-9 w-full border border-hairline bg-white/[0.03] pl-8 pr-2 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none"
      aria-label="Search every message"
    />
  </label>
{/snippet}

<SectionFrame title="Comms" {subtitle} tag={source?.isDemo || store.isDemo ? 'SAMPLE DATA' : ''} items={railItems} active={view} onselect={pickRail} {actions} bare={!!source && !!overview && view !== IMPORT}>
  {#if !source}
    <Notice icon={MessagesSquare} title="NO BOX CONNECTED">Comms keeps your texts, calls, email and chats on your Castle box. Connect to a box to see them, or switch to Demo.</Notice>
  {:else if !overview && !problem}
    <div class="grid min-h-[40dvh] place-items-center"><LoaderCircle class="h-6 w-6 animate-spin text-text-faint" /></div>
  {:else if problem}
    <Notice icon={MessagesSquare} title="COMMS IS NOT AVAILABLE">{problem}</Notice>
  {:else if view === IMPORT}
    <!-- ---------------- import ---------------- -->
    <div class="mx-auto max-w-4xl px-1 py-2">
      <section
        class="hud-corners glass border-2 border-dashed px-5 py-8 text-center transition-colors {dragging ? 'border-chiral/70 bg-chiral/5' : 'border-transparent'}"
        aria-labelledby="import-heading"
        ondragover={(e) => {
          e.preventDefault();
          dragging = true;
        }}
        ondragleave={() => (dragging = false)}
        ondrop={onDrop}
      >
        <Upload class="mx-auto h-8 w-8 text-chiral" strokeWidth={1.25} />
        <h2 id="import-heading" class="micro mt-3 text-white">BRING YOUR MESSAGES HOME</h2>
        <p class="mx-auto mt-1.5 max-w-lg text-[13px] text-text-dim">
          Drop a backup here, or pick one. Texts, call logs, email, Discord, Messenger, recordings, or a whole zip of them. A newer backup of the same phone adds only what is new.
        </p>
        <input bind:this={fileInput} type="file" multiple class="hidden" onchange={(e) => send(e.currentTarget.files)} accept=".xml,.mbox,.eml,.json,.zip,.vcf,.m4a,.mp3,.amr,.3gp,.wav,.ogg,.opus,.aac,.awb" />
        <button class="micro expo t-hover mt-4 bg-chiral px-5 py-2.5 font-bold text-black transition-opacity hover:opacity-90 disabled:opacity-50" onclick={() => fileInput?.click()} disabled={!!sending || !!overview.importing} data-autofocus>
          {sending || overview.importing ? 'WORKING…' : 'CHOOSE FILES'}
        </button>
        {#if sending || overview.importing}
          <p class="micro mt-3 flex items-center justify-center gap-2 text-bridge" in:fade>
            <LoaderCircle class="h-3.5 w-3.5 animate-spin" />
            {#if overview.importing}{overview.importing.step.toUpperCase()} · {overview.importing.source}{:else}SENDING {sending.name}{/if}
            {#if sending && sending.of > 1}<span class="text-text-faint">({sending.at} OF {sending.of})</span>{/if}
          </p>
        {/if}
        {#if overview.problem && !overview.importing}
          <p class="mt-3 flex items-center justify-center gap-1.5 text-[13px] text-chiral"><CircleAlert class="h-4 w-4" />{overview.problem}</p>
        {/if}
        <p class="micro mt-4 flex items-center justify-center gap-1.5 text-text-faint"><Lock class="h-3 w-3" />EVERYTHING STAYS ON THE BOX. NOTHING IS SENT ANYWHERE ELSE.</p>
      </section>

      <section class="mt-6" aria-labelledby="how-heading">
        <h2 id="how-heading" class="micro text-text-dim">WHERE TO GET EACH ONE</h2>
        <div class="mt-3 grid gap-3 md:grid-cols-2">
          {#each SOURCES as s (s.title)}
            <div class="border border-hairline bg-white/[0.02] px-4 py-3">
              <p class="text-[13px] font-semibold text-white">{s.title}</p>
              <p class="mt-1 text-[13px] leading-relaxed text-text-dim">{s.steps}</p>
            </div>
          {/each}
          <div class="border border-hairline-faint px-4 py-3 md:col-span-2">
            <p class="text-[13px] font-semibold text-white">Not yet</p>
            <p class="mt-1 text-[13px] leading-relaxed text-text-dim">
              iPhone texts only leave the phone through a computer tool (iMazing and the like), and Castle does not read those exports yet. The Castle app has no call recorder of its own: Android and iOS do not let apps hear calls. A phone with a built-in recorder makes files Castle can keep.
            </p>
          </div>
        </div>
      </section>

      {#if overview.imports.length}
        <section class="mt-6" aria-labelledby="history-heading">
          <h2 id="history-heading" class="micro flex items-center gap-2 text-text-dim"><History class="h-3.5 w-3.5" />IMPORTED SO FAR</h2>
          <ul class="mt-3 divide-y divide-hairline-faint border border-hairline">
            {#each overview.imports as rec, i (rec.at + rec.source)}
              {@const r = rec.report}
              {@const found = [plural(r.texts, 'text'), plural(r.calls, 'call'), plural(r.emails, 'email'), plural(r.chats, 'chat message'), plural(r.recordings, 'recording'), plural(r.contactsFiles, 'contacts file')].filter(Boolean)}
              <li class="flex flex-wrap items-baseline gap-x-4 gap-y-1 px-4 py-2.5" in:fly={itemIn(Math.min(i, STAGGER_CAP))}>
                <span class="min-w-0 flex-1 truncate text-[13px] text-white">{rec.source}</span>
                <span class="text-[12px] text-text-dim">{found.length ? found.join(' · ') : 'nothing new'}{r.known ? ` · ${r.known.toLocaleString()} already kept` : ''}</span>
                <span class="micro text-text-faint">{when(rec.at)}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>
  {:else}
    <!-- ---------------- conversations and a thread ---------------- -->
    <div class="grid h-full grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)] md:grid-cols-[minmax(280px,360px)_minmax(0,1fr)]">
      <div class="flex min-h-0 flex-col border-r border-hairline {openId ? 'hidden md:flex' : 'flex'}">
        <label class="relative block border-b border-hairline-faint p-2 md:hidden">
          <Search class="pointer-events-none absolute left-4 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
          <input type="search" bind:value={query} placeholder="SEARCH EVERY MESSAGE" class="micro h-9 w-full border border-hairline bg-white/[0.03] pl-8 pr-2 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search every message" />
        </label>
        {#if withContact}
          <div class="flex items-center gap-2 border-b border-hairline-faint px-3 py-2">
            <span class="micro text-text-faint">WITH</span>
            <button class="micro expo t-hover flex items-center gap-1.5 border border-bridge/50 px-2 py-1 text-bridge hover:bg-bridge/10" onclick={() => (withContact = null)} aria-label={`Show everyone, not only ${withContact.name}`}>
              {withContact.name.toUpperCase()} <X class="h-3 w-3" />
            </button>
          </div>
        {/if}
        <div class="min-h-0 flex-1 overflow-y-auto">
          {#if hits}
            <p class="micro px-4 pb-1 pt-3 text-text-faint">{searching ? 'SEARCHING…' : `${hits.length}${hits.length >= 200 ? '+' : ''} ${hits.length === 1 ? 'MESSAGE' : 'MESSAGES'}`}</p>
            <ul>
              {#each hits as hit, i (hit.conversation + hit.message.id)}
                {@const conv = byId.get(hit.conversation)}
                {#if conv}
                  <li in:fly={itemIn(Math.min(i, STAGGER_CAP))}>
                    <button class="nav-card expo t-hover flex w-full items-start gap-3 px-4 py-2.5 text-left transition-colors hover:bg-white/[0.04] {highlight === hit.message.id ? 'bg-white/[0.05]' : ''}" onclick={() => openHit(hit)} data-nav-key={`hit-${hit.message.id}`}>
                      {@render avatar(conv, 'h-8 w-8')}
                      <span class="min-w-0 flex-1">
                        <span class="flex items-baseline gap-2">
                          <span class="min-w-0 flex-1 truncate text-[13px] font-medium text-white">{conv.title}</span>
                          <span class="micro shrink-0 text-text-faint">{when(hit.message.at)}</span>
                        </span>
                        <span class="line-clamp-2 text-[12px] text-text-dim">{hit.message.me ? 'You: ' : ''}{hit.message.subject ? `${hit.message.subject} · ` : ''}{hit.message.body || hit.message.attachments.map((a) => a.name).join(', ')}</span>
                      </span>
                    </button>
                  </li>
                {/if}
              {:else}
                {#if !searching}<li class="px-4 py-6 text-center text-[13px] text-text-dim">Nothing says that.</li>{/if}
              {/each}
            </ul>
          {:else if !listed.length}
            <div class="px-6 py-10 text-center">
              <MessagesSquare class="mx-auto h-8 w-8 text-text-faint" strokeWidth={1.25} />
              <p class="micro mt-3 text-white">{withContact ? `NOTHING WITH ${withContact.name.toUpperCase()} YET` : 'NOTHING KEPT YET'}</p>
              <p class="mt-1.5 text-[13px] text-text-dim">Import a backup of your phone, your email or your chats, and every message lives here, on your box.</p>
              <button class="micro expo t-hover mt-4 border border-chiral/60 px-3 py-2 text-chiral hover:bg-chiral/10" onclick={() => pickRail(IMPORT)}>IMPORT</button>
            </div>
          {:else}
            <ul>
              {#each listed as conv, i (conv.id)}
                {@const found = index.forConversation(conv)}
                <li in:fly={itemIn(Math.min(i, STAGGER_CAP))}>
                  <button
                    class="nav-card expo t-hover flex w-full items-center gap-3 border-l-2 px-4 py-2.5 text-left transition-colors hover:bg-white/[0.04] {openId === conv.id ? 'border-chiral bg-white/[0.05]' : 'border-transparent'}"
                    onclick={() => openConversation(conv.id)}
                    data-nav-key={conv.id}
                    aria-current={openId === conv.id ? 'true' : undefined}
                  >
                    {@render avatar(conv)}
                    <span class="min-w-0 flex-1">
                      <span class="flex items-baseline gap-2">
                        <span class="min-w-0 flex-1 truncate text-[14px] font-medium text-white">{found.length === 1 && conv.participants.length <= 1 ? found[0].name : conv.title}</span>
                        <span class="micro shrink-0 text-text-faint">{when(conv.last)}</span>
                      </span>
                      <span class="block truncate text-[12px] text-text-dim">{conv.snippet}</span>
                    </span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </div>

      <div class="min-h-0 flex-col {openId ? 'flex' : 'hidden md:flex'}">
        {#if open}
          {@const card = openPeople[0]}
          <header class="flex shrink-0 items-center gap-3 border-b border-hairline px-3 py-2.5 md:px-5">
            <button class="micro expo t-hover grid h-8 w-8 place-items-center text-text-dim hover:text-white" onclick={closeConversation} data-nav-back aria-label="Back to the conversations">
              <ChevronLeft class="h-5 w-5" />
            </button>
            {@render avatar(open)}
            <div class="min-w-0 flex-1">
              <h2 class="truncate text-[15px] font-semibold text-white">{openPeople.length === 1 && open.participants.length <= 1 ? card.name : open.title}</h2>
              <p class="micro truncate text-text-faint">
                {channelOf(open.channel).label.toUpperCase()} · {plural(open.count, 'message').toUpperCase()}{open.participants.length ? ` · ${open.participants.join(', ')}` : ''}
              </p>
            </div>
            {#if open.participants.length <= 1 || openPeople.length}
              <button class="micro expo t-hover flex h-8 items-center gap-1.5 border border-hairline px-2.5 text-text-dim transition-colors hover:text-white" onclick={() => openCard(open)} title={card ? `Open ${card.name}'s card in Contacts` : 'Make a card for them in Contacts'}>
                {#if card}<BookUser class="h-3.5 w-3.5" /><span class="hidden lg:inline">OPEN CARD</span>{:else}<UserPlus class="h-3.5 w-3.5" /><span class="hidden lg:inline">ADD TO CONTACTS</span>{/if}
              </button>
            {/if}
            <button class="micro expo t-hover grid h-8 w-8 place-items-center border border-hairline text-text-dim transition-colors hover:border-chiral/60 hover:text-chiral" onclick={() => forget(open)} aria-label={`Forget the conversation with ${open.title}`} title="Forget this conversation">
              <Trash2 class="h-3.5 w-3.5" />
            </button>
          </header>
          <div bind:this={scroller} onscroll={onScroll} onwheel={touch} ontouchmove={touch} onkeydown={touch} onpointerdown={touch} class="min-h-0 flex-1 overflow-y-auto px-3 py-3 md:px-6">
            {#if threadLoading || !thread}
              <div class="grid h-full place-items-center"><LoaderCircle class="h-5 w-5 animate-spin text-text-faint" /></div>
            {:else}
              <div bind:this={content}>
              {#if thread.more}
                <div class="flex justify-center py-2">
                  <button class="micro expo t-hover flex items-center gap-1.5 border border-hairline px-3 py-1.5 text-text-dim hover:text-white disabled:opacity-50" onclick={loadOlder} disabled={olderLoading}>
                    {#if olderLoading}<LoaderCircle class="h-3 w-3 animate-spin" />{/if}LOAD OLDER
                  </button>
                </div>
              {:else}
                <p class="micro pt-2 text-center text-text-faint">THE BEGINNING</p>
              {/if}
              {#each thread.messages as m, i (m.id)}
                {@render messageView(m, i, thread.messages)}
              {/each}
              {#if jumped}
                <div class="sticky bottom-2 flex justify-center pt-3">
                  <button class="micro expo t-hover border border-bridge/60 bg-black/80 px-3 py-1.5 text-bridge hover:bg-bridge/10" onclick={() => openConversation(open.id)}>BACK TO THE NEWEST</button>
                </div>
              {/if}
              </div>
            {/if}
          </div>
          {#if open.channel !== 'calls'}
            <p class="micro shrink-0 truncate border-t border-hairline-faint px-5 py-2 text-center text-text-faint">KEPT FROM {open.channel === 'email' ? 'YOUR MAIL' : open.channel === 'texts' ? 'YOUR PHONE' : channelOf(open.channel).label.toUpperCase()} · REPLY THERE</p>
          {/if}
        {:else}
          <div class="wiregrid grid h-full place-items-center p-6 text-center">
            <div>
              <Paperclip class="mx-auto h-7 w-7 text-text-faint" strokeWidth={1.25} />
              <p class="micro mt-3 text-text-dim">PICK A CONVERSATION</p>
              <p class="mt-1 max-w-xs text-[13px] text-text-faint">Every text, call, email and chat you have brought home, in one place.</p>
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</SectionFrame>
