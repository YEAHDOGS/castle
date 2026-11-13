<!-- ContactsSection: the signed-in person's own address book, kept on the box.
     The rail has Everyone, Starred, Birthdays (the next ones first), Duplicates (cards that
     look like one person, with Merge), and Backup: a phone's, Google's or iCloud's .vcf in,
     the whole book out as a .vcf to put back onto an Android phone or an iPhone, and every
     earlier copy of the book (kept before each import, merge, delete or restore) to go back to.

     A card has the person's numbers, addresses, birthday, a bio, their picture, the Theater
     items they are in (home videos with dad), and their conversations in Comms.

     Each person's book is their own: the app asks for the signed-in person's (see
     castled/src/contacts.rs for what that does and does not promise yet). -->
<script>
  import { BookUser, Search, Star, Cake, Users, ArchiveRestore, Plus, X, Phone, Mail, MapPin, Link as LinkIcon, PenLine, Trash2, Merge, Upload, Download, History, LoaderCircle, MessagesSquare, Clapperboard, Camera, Play, Check, ChevronLeft, Tag, Building2 } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { fly, fade, scale } from 'svelte/transition';
  import { SvelteMap } from 'svelte/reactivity';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { ask } from '../../lib/ask.svelte.js';
  import { PAD, setHints } from '../../lib/input.svelte.js';
  import { modalIn, itemIn } from '../../lib/motion.js';
  import { fmtStamp } from '../../lib/computer.js';
  import { isWith, initials } from '../../lib/people.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';
  import TheaterPlayer from '../TheaterPlayer.svelte';

  const EVERYONE = 'everyone';
  const STARRED = 'starred';
  const BIRTHDAYS = 'birthdays';
  const DUPLICATES = 'duplicates';
  const BACKUP = 'backup';
  const LABELS = { phones: ['Mobile', 'Home', 'Work', 'Main', 'Fax', 'Other'], emails: ['Home', 'Work', 'Other'], addresses: ['Home', 'Work', 'Other'], urls: ['Home', 'Work', 'Other'] };
  const DAY = 24 * 60 * 60 * 1000;
  const STAGGER_CAP = 24;
  const MEDIA_PICK_MAX = 60;

  let book = $state([]);
  let loading = $state(true);
  let problem = $state('');
  let view = $state(EVERYONE);
  let query = $state('');
  let open = $state(null); // the card being looked at
  let editing = $state(null); // a draft card in the editor
  let saving = $state(false);
  let dups = $state([]);
  let notSame = $state(new Set()); // duplicate groups waved off this visit
  let copies = $state([]);
  let importing = $state(false);
  let conversations = $state([]); // from Comms, to show each person's
  let theaterItems = $state([]); // to show the items a card links
  let picking = $state(false); // choosing Theater items for a card
  let pickQuery = $state('');
  let queue = $state([]);
  let playAt = $state(0);
  const photos = new SvelteMap(); // `${id}:${photoAt}` -> picture address, or null
  const thumbs = new SvelteMap(); // theater item id -> picture address, or null
  let fileInput = $state(null);
  let photoInput = $state(null);

  const source = $derived(shell.contactsSource);
  const person = $derived(shell.user.id);
  const theater = $derived(shell.theaterSource);
  const comms = $derived(shell.commsSource);

  const matches = (c, q) => {
    if (!q) return true;
    const hay = [c.name, c.nickname, c.given, c.family, c.org, c.title, c.notes, ...c.groups, ...c.phones.map((p) => p.value), ...c.emails.map((e) => e.value)].join(' ').toLowerCase();
    const digits = q.replace(/\D/g, '');
    return q.toLowerCase().split(/\s+/).every((w) => hay.includes(w)) || (digits.length >= 3 && c.phones.some((p) => p.value.replace(/\D/g, '').includes(digits)));
  };

  const shown = $derived.by(() => {
    const q = query.trim();
    let list = book.filter((c) => matches(c, q));
    if (view === STARRED) list = list.filter((c) => c.starred);
    if (view === BIRTHDAYS) list = list.filter((c) => c.birthday).sort((a, b) => untilBirthday(a.birthday) - untilBirthday(b.birthday));
    return list;
  });
  const groupsByLetter = $derived.by(() => {
    const out = [];
    for (const c of shown) {
      const letter = /^[a-z]/i.test(c.name) ? c.name[0].toUpperCase() : '#';
      if (out.at(-1)?.letter !== letter) out.push({ letter, cards: [] });
      out.at(-1).cards.push(c);
    }
    return out;
  });
  const dupGroups = $derived(dups.filter((d) => !notSame.has(d.ids.join(','))).map((d) => ({ ...d, cards: d.ids.map((id) => book.find((c) => c.id === id)).filter(Boolean) })).filter((d) => d.cards.length > 1));
  const soon = $derived(book.filter((c) => c.birthday && untilBirthday(c.birthday) <= 14).length);
  const railItems = $derived([
    { id: EVERYONE, label: 'Everyone', icon: Users, count: book.length },
    { id: STARRED, label: 'Starred', icon: Star, count: book.filter((c) => c.starred).length },
    { id: BIRTHDAYS, label: 'Birthdays', icon: Cake, count: soon || undefined },
    { id: DUPLICATES, label: 'Duplicates', icon: Merge, count: dupGroups.length },
    { id: BACKUP, label: 'Backup', icon: ArchiveRestore },
  ]);
  const subtitle = $derived(source ? `${book.length} ${book.length === 1 ? 'person' : 'people'}${dupGroups.length ? ` · ${dupGroups.length} to tidy` : ''}` : '');
  const openConvos = $derived(open ? conversations.filter((c) => isWith(c, open)).sort((a, b) => b.last - a.last) : []);
  const openMedia = $derived(open ? open.media.map((id) => theaterItems.find((i) => i.id === id)).filter(Boolean) : []);
  // Theater items tagged with the person's name, offered for their card.
  const suggested = $derived.by(() => {
    if (!open) return [];
    const names = [open.name, open.nickname, open.given].filter(Boolean).map((n) => n.toLowerCase());
    return theaterItems.filter((i) => !open.media.includes(i.id) && (i.tags || []).some((t) => names.includes(String(t).toLowerCase()))).slice(0, 8);
  });
  const pickable = $derived.by(() => {
    const q = pickQuery.trim().toLowerCase();
    return theaterItems.filter((i) => ['video', 'photo', 'movie', 'episode'].includes(i.kind) && (!q || `${i.title} ${i.folder || ''} ${(i.tags || []).join(' ')}`.toLowerCase().includes(q))).slice(0, MEDIA_PICK_MAX);
  });

  // ---------------------------------------------------------------- reading

  async function load(from = source, who = person) {
    loading = true;
    problem = '';
    if (!from) {
      book = [];
      loading = false;
      return;
    }
    try {
      const [cards, found] = await Promise.all([from.contacts(who), from.contactDuplicates(who)]);
      if (from !== source || who !== person) return;
      book = cards;
      dups = found;
      if (open) open = cards.find((c) => c.id === open.id) || null;
    } catch (err) {
      problem = err?.message || 'Contacts could not be read';
    }
    loading = false;
  }

  async function loadSide(who) {
    // Comms and Theater are extras on a card: when they are not there, the card still works.
    try {
      conversations = comms ? (await comms.comms(who)).conversations : [];
    } catch {
      conversations = [];
    }
    try {
      theaterItems = theater ? await theater.theaterItems() : [];
    } catch {
      theaterItems = [];
    }
  }

  $effect(() => {
    const from = source;
    const who = person;
    untrack(() => {
      open = null;
      editing = null;
      photos.clear();
      load(from, who).then(() => takeFocus());
      loadSide(who);
    });
  });

  // Another section sent the person here: open a card, or start one from what it knew.
  function takeFocus() {
    const f = shell.focus;
    if (f?.section !== 'contacts') return;
    shell.focus = null;
    if (f.contactId) {
      const c = book.find((x) => x.id === f.contactId);
      if (c) open = c;
    } else if (f.draft) {
      startEdit({ ...blank(), ...f.draft });
    }
  }

  $effect(() => {
    if (view === BACKUP && source) {
      source.contactHistory(person).then((list) => (copies = list)).catch(() => (copies = []));
    }
  });

  function photo(node, c) {
    const key = `${c.id}:${c.photoAt}`;
    if (!c.photo || photos.has(key)) return {};
    photos.set(key, null);
    source?.contactPhoto(person, c.id, c.photoAt).then((url) => photos.set(key, url));
    return {};
  }

  function thumb(node, item) {
    if (thumbs.has(item.id)) return {};
    thumbs.set(item.id, null);
    theater?.theaterArt(item.id, 'thumb').then((url) => thumbs.set(item.id, url)).catch(() => {});
    return {};
  }

  // ---------------------------------------------------------------- birthdays

  /** Days until the next birthday, 0 for today. */
  function untilBirthday(b) {
    const m = /^(?:\d{4}|-)-(\d{2})-(\d{2})$/.exec(b || '');
    if (!m) return 999;
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    let next = new Date(today.getFullYear(), Number(m[1]) - 1, Number(m[2]));
    if (next < today) next = new Date(today.getFullYear() + 1, Number(m[1]) - 1, Number(m[2]));
    return Math.round((next - today) / DAY);
  }

  function birthdayText(b) {
    const m = /^(\d{4}|-)-(\d{2})-(\d{2})$/.exec(b || '');
    if (!m) return '';
    const date = new Date(2000, Number(m[2]) - 1, Number(m[3])).toLocaleDateString(undefined, { month: 'long', day: 'numeric' });
    const days = untilBirthday(b);
    const when = days === 0 ? 'today' : days === 1 ? 'tomorrow' : `in ${days} days`;
    if (m[1] === '-') return `${date} · ${when}`;
    const turning = new Date().getFullYear() - Number(m[1]) + (days > 0 && new Date(Date.now() + days * DAY).getFullYear() > new Date().getFullYear() ? 1 : 0);
    return `${date}, ${m[1]} · turns ${turning} ${when}`;
  }

  // ---------------------------------------------------------------- changing

  const blank = () => ({ id: '', name: '', given: '', family: '', nickname: '', phones: [], emails: [], addresses: [], urls: [], org: '', title: '', birthday: '', notes: '', groups: [], media: [], starred: false });

  function startEdit(c) {
    const draft = JSON.parse(JSON.stringify(c));
    for (const k of ['phones', 'emails']) if (!draft[k].length) draft[k].push({ label: k === 'phones' ? 'Mobile' : 'Home', value: '' });
    draft.groupsText = draft.groups.join(', ');
    editing = draft;
  }

  async function save() {
    saving = true;
    try {
      const { groupsText, ...draft } = editing;
      draft.groups = groupsText.split(',').map((g) => g.trim()).filter(Boolean);
      const kept = await source.contactSave(person, draft);
      editing = null;
      await load();
      open = book.find((c) => c.id === kept.id) || kept;
      store.toast(draft.id ? 'Saved' : `${kept.name} added`);
    } catch (err) {
      store.toast(err?.message || 'That could not be saved');
    }
    saving = false;
  }

  async function quietSave(c, changes) {
    try {
      const kept = await source.contactSave(person, { ...c, ...changes });
      book = book.map((x) => (x.id === kept.id ? kept : x));
      if (open?.id === kept.id) open = kept;
    } catch (err) {
      store.toast(err?.message || 'That could not be saved');
    }
  }

  async function remove(c) {
    if (!(await ask(`Delete ${c.name}? A copy of the book is kept first, so Backup can bring them back.`, { yes: 'Delete', danger: true }))) return;
    try {
      await source.contactsDelete(person, [c.id]);
      open = null;
      await load();
      store.toast(`${c.name} deleted`);
    } catch (err) {
      store.toast(err?.message || 'That could not be deleted');
    }
  }

  async function merge(group) {
    const [keep, ...rest] = group.cards;
    if (!(await ask(`Merge ${rest.map((c) => c.name).join(', ')} into ${keep.name}? Every number, address and note is kept.`, { yes: 'Merge' }))) return;
    try {
      const merged = await source.contactsMerge(person, group.cards.map((c) => c.id));
      await load();
      store.toast(`Merged into ${merged.name}`);
    } catch (err) {
      store.toast(err?.message || 'Those could not be merged');
    }
  }

  async function setPhoto(e) {
    const file = e.currentTarget.files?.[0];
    e.currentTarget.value = '';
    if (!file || !open) return;
    try {
      const kept = await source.contactPhotoSet(person, open.id, file);
      book = book.map((x) => (x.id === kept.id ? kept : x));
      open = kept;
    } catch (err) {
      store.toast(err?.message || 'That picture could not be used');
    }
  }

  async function importFiles(files) {
    if (!files?.length || !source) return;
    importing = true;
    for (const file of files) {
      try {
        const r = await source.contactsImport(person, file);
        store.toast(`${file.name}: ${r.added} added${r.skipped ? `, ${r.skipped} already here` : ''}${r.empty ? `, ${r.empty} empty skipped` : ''}`);
      } catch (err) {
        store.toast(`${file.name}: ${err?.message || 'could not be read'}`);
      }
    }
    importing = false;
    await load();
    copies = await source.contactHistory(person).catch(() => copies);
  }

  async function exportBook(ids = null) {
    try {
      const blob = await source.contactsExport(person, ids);
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = ids?.length === 1 ? `${book.find((c) => c.id === ids[0])?.name || 'contact'}.vcf` : `contacts-${new Date().toISOString().slice(0, 10)}.vcf`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
    } catch (err) {
      store.toast(err?.message || 'The book could not be written out');
    }
  }

  async function restore(copy) {
    if (!(await ask(`Put back the book from ${fmtStamp(copy.at)} (${copy.count} people)? The book as it is now is kept too.`, { yes: 'Restore' }))) return;
    try {
      const r = await source.contactsRestore(person, copy.at);
      await load();
      copies = await source.contactHistory(person);
      store.toast(`Restored: ${r.count} people`);
    } catch (err) {
      store.toast(err?.message || 'That copy could not be restored');
    }
  }

  function linkMedia(item) {
    if (!open || open.media.includes(item.id)) return;
    quietSave(open, { media: [...open.media, item.id] });
  }

  function unlinkMedia(item) {
    quietSave(open, { media: open.media.filter((id) => id !== item.id) });
  }

  function pickRail(id) {
    view = id;
    open = null;
  }

  function onDrop(e) {
    e.preventDefault();
    importFiles([...(e.dataTransfer?.files || [])].filter((f) => /\.vcf$|\.vcard$/i.test(f.name)));
  }

  // ---------------------------------------------------------------- controller

  $effect(() => {
    if (editing) return setHints('contacts', [{ btn: 'A', label: 'Choose' }, { btn: 'B', label: 'Cancel' }], 2);
    if (open) return setHints('contacts', [{ btn: 'A', label: 'Choose' }, { btn: 'B', label: 'Back' }], 2);
    return setHints('contacts', [{ btn: 'DPAD', label: 'Move' }, { btn: 'A', label: 'Open' }]);
  });

  function onKey(e) {
    if (e.key === 'Escape' || e.key === 'GoBack') {
      if (picking) picking = false;
      else if (editing) editing = null;
      else if (open) open = null;
      else return;
      e.preventDefault();
    }
  }

  const fieldIcon = { phones: Phone, emails: Mail, addresses: MapPin, urls: LinkIcon };
  const hrefFor = (kind, value) => (kind === 'phones' ? `tel:${value.replace(/[^\d+]/g, '')}` : kind === 'emails' ? `mailto:${value}` : kind === 'urls' ? (/^https?:\/\//i.test(value) ? value : `https://${value}`) : null);
  const ago = (t) => {
    const days = Math.floor((Date.now() - t) / DAY);
    return days <= 0 ? 'today' : days === 1 ? 'yesterday' : days < 30 ? `${days} days ago` : fmtStamp(t);
  };
</script>

<svelte:window onkeydown={onKey} />

{#snippet face(c, size = 'h-11 w-11 text-[14px]')}
  <span class="relative grid shrink-0 place-items-center overflow-hidden rounded-full border border-hairline bg-white/[0.05] font-bold text-text-dim {size}" use:photo={c}>
    {#if photos.get(`${c.id}:${c.photoAt}`)}
      <img src={photos.get(`${c.id}:${c.photoAt}`)} alt="" class="h-full w-full object-cover" draggable="false" />
    {:else}
      {initials(c.name)}
    {/if}
  </span>
{/snippet}

{#snippet cardButton(c, i)}
  <li in:fly={itemIn(Math.min(i, STAGGER_CAP))}>
    <button class="nav-card expo t-hover group flex w-full items-center gap-3 border border-hairline bg-white/[0.02] px-3 py-2.5 text-left transition-colors hover:border-white/25 hover:bg-white/[0.04]" onclick={() => (open = c)}>
      {@render face(c)}
      <span class="min-w-0 flex-1">
        <span class="flex items-center gap-1.5">
          <span class="truncate text-[14px] font-medium text-white">{c.name}</span>
          {#if c.starred}<Star class="h-3 w-3 shrink-0 fill-bridge text-bridge" />{/if}
        </span>
        <span class="micro mt-0.5 block truncate text-text-faint">
          {#if view === BIRTHDAYS}{birthdayText(c.birthday)}{:else}{c.phones[0]?.value || c.emails[0]?.value || c.org || '—'}{/if}
        </span>
      </span>
      {#if view !== BIRTHDAYS && c.birthday && untilBirthday(c.birthday) <= 14}
        <Cake class="h-4 w-4 shrink-0 text-bridge" aria-label="Birthday soon" />
      {/if}
    </button>
  </li>
{/snippet}

{#snippet actions()}
  <label class="relative hidden w-56 md:block">
    <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
    <input type="search" bind:value={query} placeholder="SEARCH NAMES, NUMBERS, NOTES" class="micro h-9 w-full border border-hairline bg-white/[0.03] pl-8 pr-2 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search contacts" />
  </label>
  {#if source && !problem}
    <button class="micro expo t-hover flex h-9 items-center gap-1.5 bg-chiral px-3 font-bold text-black transition-opacity hover:opacity-90" onclick={() => startEdit(blank())}>
      <Plus class="h-3.5 w-3.5" /><span class="hidden lg:inline">NEW</span>
    </button>
  {/if}
{/snippet}

<SectionFrame title="Contacts" {subtitle} tag={source?.isDemo ? 'SAMPLE DATA' : ''} items={railItems} active={view} onselect={pickRail} {actions}>
  {#if !source}
    <Notice icon={BookUser} title="NO BOX CONNECTED">Contacts keeps everyone you know on your Castle box, with their pictures, birthdays and the home videos they are in. Connect to a box, or switch to Demo.</Notice>
  {:else if loading && !book.length && !problem}
    <div class="grid min-h-[40dvh] place-items-center"><LoaderCircle class="h-6 w-6 animate-spin text-text-faint" /></div>
  {:else if problem}
    <Notice icon={BookUser} title="CONTACTS ARE NOT AVAILABLE">{problem}</Notice>
  {:else if view === BACKUP}
    <!-- ---------------- backup ---------------- -->
    <div class="mx-auto grid max-w-4xl gap-4" role="region" aria-label="Back up and restore" ondragover={(e) => e.preventDefault()} ondrop={onDrop}>
      <section class="hud-corners glass px-5 py-5">
        <h2 class="micro flex items-center gap-2 text-chiral"><Upload class="h-4 w-4" />BRING CONTACTS IN</h2>
        <p class="mt-1.5 text-[13px] text-text-dim">A vCard file (.vcf) from your phone, Google or iCloud: every card, with its picture. A card already here is skipped. Drop files here, or:</p>
        <div class="mt-3 flex flex-wrap items-center gap-2">
          <button class="micro expo t-hover flex h-10 items-center gap-1.5 bg-chiral px-4 font-bold text-black hover:opacity-90 disabled:opacity-50" onclick={() => fileInput.click()} disabled={importing}>
            {#if importing}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />READING…{:else}<Upload class="h-3.5 w-3.5" />CHOOSE .VCF FILES{/if}
          </button>
          <input bind:this={fileInput} type="file" accept=".vcf,.vcard,text/vcard,text/x-vcard" multiple class="hidden" onchange={(e) => { importFiles([...e.currentTarget.files]); e.currentTarget.value = ''; }} />
        </div>
        <ul class="mt-4 grid gap-2 text-[13px] text-text-dim md:grid-cols-3">
          <li><span class="micro block text-white">ANDROID</span>Contacts › Fix &amp; manage › Export to file.</li>
          <li><span class="micro block text-white">IPHONE</span>iCloud.com › Contacts › select all › Export vCard.</li>
          <li><span class="micro block text-white">GOOGLE</span>contacts.google.com › Export › vCard.</li>
        </ul>
      </section>

      <section class="hud-corners glass px-5 py-5">
        <h2 class="micro flex items-center gap-2 text-chiral"><Download class="h-4 w-4" />PUT THEM BACK ON A PHONE</h2>
        <p class="mt-1.5 text-[13px] text-text-dim">One .vcf with everyone in it, pictures included. Open it on the phone and every card is added.</p>
        <div class="mt-3 flex flex-wrap gap-2">
          <button class="micro expo t-hover flex h-10 items-center gap-1.5 border border-hairline px-4 text-white hover:bg-white/[0.06] disabled:opacity-50" onclick={() => exportBook()} disabled={!book.length}>
            <Download class="h-3.5 w-3.5" />DOWNLOAD ALL {book.length}
          </button>
        </div>
        <ul class="mt-4 grid gap-2 text-[13px] text-text-dim md:grid-cols-2">
          <li><span class="micro block text-white">ANDROID</span>Open the file (Files, or from an email), then Import into the phone’s Contacts.</li>
          <li><span class="micro block text-white">IPHONE</span>AirDrop, email or Files: tap the file, then Add All Contacts. Or iCloud.com › Contacts › Import.</li>
        </ul>
      </section>

      <section class="hud-corners glass px-5 py-5">
        <h2 class="micro flex items-center gap-2 text-chiral"><History class="h-4 w-4" />EARLIER COPIES</h2>
        <p class="mt-1.5 text-[13px] text-text-dim">The box keeps the book as it was before every import, merge, delete and restore. Putting one back keeps today’s too.</p>
        {#if copies.length}
          <ul class="mt-3 divide-y divide-hairline-faint border border-hairline">
            {#each copies as copy (copy.at)}
              <li class="flex items-center gap-3 px-3 py-2.5">
                <div class="min-w-0 flex-1">
                  <p class="truncate text-[13px] text-white">{copy.reason}</p>
                  <p class="micro mt-0.5 text-text-faint">{fmtStamp(copy.at)} · {copy.count} {copy.count === 1 ? 'person' : 'people'}</p>
                </div>
                <button class="micro expo t-hover border border-hairline px-2.5 py-1.5 text-text-dim hover:text-white" onclick={() => restore(copy)}>RESTORE</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="micro mt-3 text-text-faint">NONE YET: THE FIRST ONE IS KEPT AT THE FIRST CHANGE.</p>
        {/if}
      </section>
    </div>
  {:else if view === DUPLICATES}
    <!-- ---------------- duplicates ---------------- -->
    <div class="mx-auto max-w-4xl">
      {#if !dupGroups.length}
        <Notice icon={Check} title="NO DUPLICATES">Nobody here looks like two people. Cards that share a number (however it is written), an email address or a name show up here.</Notice>
      {:else}
        <p class="mb-3 text-[13px] text-text-dim">These look like the same person. Merging keeps the first card and adds everything from the others; nothing is lost, and Backup keeps the book from before.</p>
        <ul class="grid gap-3">
          {#each dupGroups as group, gi (group.ids.join(','))}
            <li class="hud-corners glass px-4 py-4" in:fly={itemIn(gi)} out:scale={{ duration: 180, start: 0.97 }}>
              <p class="micro text-bridge">{group.reasons.join(' · ').toUpperCase()}</p>
              <div class="mt-3 grid gap-2 sm:grid-cols-2">
                {#each group.cards as c, i (c.id)}
                  <button class="flex items-start gap-3 border px-3 py-2.5 text-left transition-colors hover:bg-white/[0.04] {i === 0 ? 'border-chiral/50' : 'border-hairline'}" onclick={() => (open = c)}>
                    {@render face(c)}
                    <span class="min-w-0 flex-1 text-[12px] text-text-dim">
                      <span class="block truncate text-[14px] font-medium text-white">{c.name}{#if i === 0}<span class="micro ml-1.5 text-chiral">KEEPS</span>{/if}</span>
                      {#each [...c.phones, ...c.emails] as f (f.value)}<span class="block truncate">{f.value}</span>{/each}
                      <span class="micro mt-1 block text-text-faint">FROM {c.source.toUpperCase()}</span>
                    </span>
                  </button>
                {/each}
              </div>
              <div class="mt-3 flex gap-2">
                <button class="micro expo t-hover flex h-9 items-center gap-1.5 bg-chiral px-3 font-bold text-black hover:opacity-90" onclick={() => merge(group)}><Merge class="h-3.5 w-3.5" />MERGE</button>
                <button class="micro expo t-hover h-9 border border-hairline px-3 text-text-dim hover:text-white" onclick={() => (notSame = new Set([...notSame, group.ids.join(',')]))}>NOT THE SAME</button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {:else if !book.length}
    <Notice icon={BookUser} title="NOBODY HERE YET">Bring in your phone’s contacts from Backup, or add someone with New. They stay on this box.</Notice>
  {:else if !shown.length}
    <Notice icon={Search} title="NO ONE MATCHES">{view === STARRED ? 'Star someone from their card and they show here.' : view === BIRTHDAYS ? 'Nobody has a birthday on their card yet.' : 'Try fewer words, or part of a number.'}</Notice>
  {:else}
    <!-- ---------------- the book ---------------- -->
    <label class="relative mb-3 block md:hidden">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
      <input type="search" bind:value={query} placeholder="SEARCH" class="micro h-10 w-full border border-hairline bg-white/[0.03] pl-8 pr-2 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search contacts" />
    </label>
    {#if view === BIRTHDAYS}
      <ul class="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {#each shown as c, i (c.id)}{@render cardButton(c, i)}{/each}
      </ul>
    {:else}
      {#each groupsByLetter as g (g.letter)}
        <h2 class="micro mb-2 mt-4 text-text-faint first:mt-0">{g.letter}</h2>
        <ul class="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {#each g.cards as c, i (c.id)}{@render cardButton(c, i)}{/each}
        </ul>
      {/each}
    {/if}
  {/if}
</SectionFrame>

<!-- ---------------- a card ---------------- -->
{#if open && !editing}
  <div class="fixed inset-0 z-40 bg-black/60" transition:fade={{ duration: 150 }} onclick={() => (open = null)} aria-hidden="true"></div>
  <div class="fixed inset-y-0 right-0 z-50 flex w-full max-w-lg flex-col border-l border-hairline bg-void" transition:fly={{ x: 420, duration: 240 }} role="dialog" aria-modal="true" aria-label={open.name}>
    <header class="flex items-center gap-2 border-b border-hairline py-3 pl-4 pr-16">
      <button class="grid h-9 w-9 place-items-center text-text-dim hover:text-white" onclick={() => (open = null)} aria-label="Close card" data-nav-back><ChevronLeft class="h-5 w-5" /></button>
      <p class="micro flex-1 text-text-faint">CONTACT</p>
      <button class="grid h-9 w-9 place-items-center hover:text-white {open.starred ? 'text-bridge' : 'text-text-dim'}" onclick={() => quietSave(open, { starred: !open.starred })} aria-label={open.starred ? 'Unstar' : 'Star'} aria-pressed={open.starred}>
        <Star class="h-4 w-4 {open.starred ? 'fill-bridge' : ''}" />
      </button>
      <button class="grid h-9 w-9 place-items-center text-text-dim hover:text-white" onclick={() => startEdit(open)} aria-label="Edit"><PenLine class="h-4 w-4" /></button>
      <button class="grid h-9 w-9 place-items-center text-text-dim hover:text-white" onclick={() => exportBook([open.id])} aria-label="Download as a .vcf"><Download class="h-4 w-4" /></button>
      <button class="grid h-9 w-9 place-items-center text-text-dim hover:text-red-400" onclick={() => remove(open)} aria-label="Delete"><Trash2 class="h-4 w-4" /></button>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-5">
      <div class="flex items-center gap-4">
        <button class="group relative" onclick={() => photoInput.click()} aria-label="Change picture">
          {@render face(open, 'h-20 w-20 text-[24px]')}
          <span class="absolute inset-0 grid place-items-center rounded-full bg-black/50 opacity-0 transition-opacity group-hover:opacity-100"><Camera class="h-5 w-5 text-white" /></span>
        </button>
        <input bind:this={photoInput} type="file" accept="image/jpeg,image/png,image/webp,image/gif" class="hidden" onchange={setPhoto} />
        <div class="min-w-0">
          <h2 class="text-[22px] font-bold leading-tight text-white">{open.name}</h2>
          {#if open.nickname || (open.given && open.name !== `${open.given} ${open.family}`.trim())}
            <p class="text-[13px] text-text-dim">{[open.given, open.family].filter(Boolean).join(' ')}{open.nickname ? ` · “${open.nickname}”` : ''}</p>
          {/if}
          {#if open.org || open.title}<p class="mt-0.5 flex items-center gap-1.5 text-[13px] text-text-dim"><Building2 class="h-3.5 w-3.5" />{[open.title, open.org].filter(Boolean).join(', ')}</p>{/if}
        </div>
      </div>

      {#if open.birthday}
        <p class="mt-4 flex items-center gap-2 text-[13px] {untilBirthday(open.birthday) <= 14 ? 'text-bridge' : 'text-text-dim'}"><Cake class="h-4 w-4" />{birthdayText(open.birthday)}</p>
      {/if}

      <dl class="mt-4 grid gap-1">
        {#each ['phones', 'emails', 'addresses', 'urls'] as kind (kind)}
          {#each open[kind] as f (f.value)}
            {@const Icon = fieldIcon[kind]}
            {@const href = hrefFor(kind, f.value)}
            <div class="flex items-center gap-3 border-b border-hairline-faint py-2">
              <Icon class="h-4 w-4 shrink-0 text-text-faint" />
              <dd class="min-w-0 flex-1 break-words text-[14px] text-white">
                {#if href}<a {href} class="hover:text-chiral" target={kind === 'urls' ? '_blank' : undefined} rel="noreferrer">{f.value}</a>{:else}{f.value}{/if}
              </dd>
              {#if f.label}<dt class="micro shrink-0 text-text-faint">{f.label.toUpperCase()}</dt>{/if}
            </div>
          {/each}
        {/each}
      </dl>

      {#if open.groups.length}
        <p class="mt-3 flex flex-wrap items-center gap-1.5"><Tag class="h-3.5 w-3.5 text-text-faint" />{#each open.groups as g (g)}<span class="micro border border-hairline px-1.5 py-0.5 text-text-dim">{g.toUpperCase()}</span>{/each}</p>
      {/if}

      {#if open.notes}
        <section class="mt-5">
          <h3 class="micro text-text-faint">ABOUT</h3>
          <p class="mt-1.5 whitespace-pre-line text-[14px] leading-relaxed text-text-dim">{open.notes}</p>
        </section>
      {/if}

      <section class="mt-6">
        <div class="flex items-center justify-between">
          <h3 class="micro flex items-center gap-1.5 text-text-faint"><MessagesSquare class="h-3.5 w-3.5" />IN COMMS</h3>
          {#if openConvos.length}
            <button class="micro text-chiral hover:underline" onclick={() => shell.openWith('comms', { contact: $state.snapshot(open) })}>OPEN ALL</button>
          {/if}
        </div>
        {#if openConvos.length}
          <ul class="mt-2 grid gap-1.5">
            {#each openConvos.slice(0, 5) as conv (conv.id)}
              <li>
                <button class="flex w-full items-center gap-3 border border-hairline px-3 py-2 text-left hover:bg-white/[0.04]" onclick={() => shell.openWith('comms', { contact: $state.snapshot(open), conversationId: conv.id })}>
                  <span class="micro w-16 shrink-0 text-chiral">{conv.channel.toUpperCase()}</span>
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-[13px] text-white">{conv.snippet || conv.title}</span>
                    <span class="micro block text-text-faint">{conv.count} · LAST {ago(conv.last).toUpperCase()}</span>
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="mt-2 text-[13px] text-text-faint">No texts, calls or email with {open.name} on the box yet. Import them in Comms.</p>
        {/if}
      </section>

      <section class="mt-6">
        <div class="flex items-center justify-between">
          <h3 class="micro flex items-center gap-1.5 text-text-faint"><Clapperboard class="h-3.5 w-3.5" />IN THEATER</h3>
          {#if theater}<button class="micro text-chiral hover:underline" onclick={() => { pickQuery = ''; picking = true; }}>+ ADD</button>{/if}
        </div>
        {#if openMedia.length}
          <ul class="mt-2 grid grid-cols-3 gap-2">
            {#each openMedia as item, i (item.id)}
              <li class="group relative">
                <button class="block w-full" onclick={() => { queue = openMedia; playAt = i; }} aria-label={`Play ${item.title}`} use:thumb={item}>
                  <span class="relative block aspect-video overflow-hidden border border-hairline bg-white/[0.03]">
                    {#if thumbs.get(item.id)}<img src={thumbs.get(item.id)} alt="" class="h-full w-full object-cover" />{:else}<Clapperboard class="absolute inset-0 m-auto h-5 w-5 text-text-faint" />{/if}
                    <span class="absolute inset-0 grid place-items-center bg-black/30 opacity-0 transition-opacity group-hover:opacity-100"><Play class="h-5 w-5 text-white" /></span>
                  </span>
                  <span class="micro mt-1 block truncate text-left text-text-dim">{item.title}</span>
                </button>
                <button class="absolute right-1 top-1 grid h-6 w-6 place-items-center bg-black/70 text-text-dim opacity-0 transition-opacity hover:text-white group-hover:opacity-100 focus:opacity-100" onclick={() => unlinkMedia(item)} aria-label={`Take ${item.title} off ${open.name}'s card`}><X class="h-3.5 w-3.5" /></button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="mt-2 text-[13px] text-text-faint">Put the home videos and photos {open.name} is in on their card.</p>
        {/if}
        {#if suggested.length}
          <p class="micro mt-3 text-text-faint">TAGGED WITH THEIR NAME</p>
          <div class="mt-1.5 flex flex-wrap gap-1.5">
            {#each suggested as item (item.id)}
              <button class="micro border border-bridge/40 px-2 py-1 text-bridge hover:bg-bridge/10" onclick={() => linkMedia(item)}>+ {item.title.toUpperCase()}</button>
            {/each}
          </div>
        {/if}
      </section>

      <p class="micro mt-8 text-text-faint">FROM {open.source.toUpperCase()} · ADDED {fmtStamp(open.added).toUpperCase()}</p>
    </div>
  </div>
{/if}

<!-- ---------------- choosing Theater items ---------------- -->
{#if picking && open}
  <div class="fixed inset-0 z-[60] grid place-items-center bg-black/70 p-4" transition:fade={{ duration: 150 }}>
    <div class="flex max-h-[80dvh] w-full max-w-2xl flex-col border border-hairline bg-void" in:scale={modalIn()} role="dialog" aria-modal="true" aria-label="Add from Theater">
      <header class="flex items-center gap-3 border-b border-hairline px-4 py-3">
        <p class="micro flex-1 text-white">ADD TO {open.name.toUpperCase()}’S CARD</p>
        <button class="grid h-8 w-8 place-items-center text-text-dim hover:text-white" onclick={() => (picking = false)} aria-label="Close" data-nav-back><X class="h-4 w-4" /></button>
      </header>
      <div class="border-b border-hairline px-4 py-3">
        <input type="search" bind:value={pickQuery} placeholder="SEARCH THEATER" class="micro h-9 w-full border border-hairline bg-white/[0.03] px-3 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search Theater" />
      </div>
      <ul class="grid min-h-0 flex-1 grid-cols-2 gap-2 overflow-y-auto p-4 sm:grid-cols-3">
        {#each pickable as item (item.id)}
          {@const linked = open.media.includes(item.id)}
          <li>
            <button class="block w-full text-left disabled:opacity-50" onclick={() => linkMedia(item)} disabled={linked} use:thumb={item}>
              <span class="relative block aspect-video overflow-hidden border {linked ? 'border-chiral/60' : 'border-hairline'} bg-white/[0.03]">
                {#if thumbs.get(item.id)}<img src={thumbs.get(item.id)} alt="" class="h-full w-full object-cover" />{:else}<Clapperboard class="absolute inset-0 m-auto h-5 w-5 text-text-faint" />{/if}
                {#if linked}<Check class="absolute right-1 top-1 h-4 w-4 text-chiral" />{/if}
              </span>
              <span class="micro mt-1 block truncate text-text-dim">{item.title}</span>
            </button>
          </li>
        {:else}
          <li class="col-span-full py-8 text-center text-[13px] text-text-faint">Nothing in Theater matches. Home videos and photos come from Theater’s libraries.</li>
        {/each}
      </ul>
    </div>
  </div>
{/if}

<!-- ---------------- editing ---------------- -->
{#if editing}
  <div class="fixed inset-0 z-[60] grid place-items-center bg-black/70 p-4" transition:fade={{ duration: 150 }}>
    <div class="w-full max-w-xl" in:scale={modalIn()} role="dialog" aria-modal="true" aria-label={editing.id ? `Edit ${editing.name}` : 'New contact'}>
    <form class="flex max-h-[90dvh] w-full flex-col border border-hairline bg-void" onsubmit={(e) => { e.preventDefault(); save(); }}>
      <header class="flex items-center gap-3 border-b border-hairline px-4 py-3">
        <p class="micro flex-1 text-white">{editing.id ? `EDIT ${editing.name.toUpperCase()}` : 'NEW CONTACT'}</p>
        <button type="button" class="grid h-8 w-8 place-items-center text-text-dim hover:text-white" onclick={() => (editing = null)} aria-label="Cancel" data-nav-back><X class="h-4 w-4" /></button>
      </header>
      <div class="grid min-h-0 flex-1 gap-3 overflow-y-auto px-4 py-4">
        {#snippet text(label, key, placeholder = '', type = 'text')}
          <label class="grid gap-1">
            <span class="micro text-text-faint">{label}</span>
            <input {type} bind:value={editing[key]} {placeholder} class="h-10 border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" />
          </label>
        {/snippet}
        {@render text('NAME AS SHOWN', 'name', 'Dad')}
        <div class="grid grid-cols-2 gap-3">
          {@render text('FIRST', 'given')}
          {@render text('LAST', 'family')}
        </div>
        <div class="grid grid-cols-2 gap-3">
          {@render text('NICKNAME', 'nickname')}
          <label class="grid gap-1">
            <span class="micro text-text-faint">BIRTHDAY</span>
            <input bind:value={editing.birthday} placeholder="1961-04-02, or --04-02" class="h-10 border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" />
          </label>
        </div>
        <div class="grid grid-cols-2 gap-3">
          {@render text('COMPANY', 'org')}
          {@render text('JOB TITLE', 'title')}
        </div>
        {#each [['phones', 'PHONE NUMBERS', 'tel'], ['emails', 'EMAIL', 'email'], ['addresses', 'ADDRESSES', 'text'], ['urls', 'WEBSITES', 'url']] as [kind, label, type] (kind)}
          <fieldset class="grid gap-1.5">
            <legend class="micro mb-1 text-text-faint">{label}</legend>
            {#each editing[kind] as f, i (i)}
              <div class="flex gap-2">
                <input list={`labels-${kind}`} bind:value={f.label} placeholder="Label" class="h-10 w-28 shrink-0 border border-hairline bg-white/[0.03] px-2 text-[13px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Label" />
                <input {type} bind:value={f.value} class="h-10 min-w-0 flex-1 border border-hairline bg-white/[0.03] px-3 text-[14px] text-white focus:border-chiral/60 focus:outline-none" aria-label={label.toLowerCase()} />
                <button type="button" class="grid h-10 w-10 shrink-0 place-items-center text-text-faint hover:text-white" onclick={() => editing[kind].splice(i, 1)} aria-label="Remove"><X class="h-4 w-4" /></button>
              </div>
            {/each}
            <datalist id={`labels-${kind}`}>{#each LABELS[kind] as l (l)}<option value={l}></option>{/each}</datalist>
            <button type="button" class="micro justify-self-start text-chiral hover:underline" onclick={() => editing[kind].push({ label: LABELS[kind][0], value: '' })}>+ ADD</button>
          </fieldset>
        {/each}
        <label class="grid gap-1">
          <span class="micro text-text-faint">GROUPS</span>
          <input bind:value={editing.groupsText} placeholder="Family, Friends" class="h-10 border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" />
        </label>
        <label class="grid gap-1">
          <span class="micro text-text-faint">ABOUT THEM</span>
          <textarea bind:value={editing.notes} rows="4" placeholder="Anything worth remembering" class="border border-hairline bg-white/[0.03] px-3 py-2 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none"></textarea>
        </label>
      </div>
      <footer class="flex justify-end gap-2 border-t border-hairline px-4 py-3">
        <button type="button" class="micro expo t-hover h-10 border border-hairline px-4 text-text-dim hover:text-white" onclick={() => (editing = null)}>CANCEL</button>
        <button type="submit" class="micro expo t-hover h-10 bg-chiral px-5 font-bold text-black hover:opacity-90 disabled:opacity-50" disabled={saving}>{saving ? 'SAVING…' : 'SAVE'}</button>
      </footer>
    </form>
    </div>
  </div>
{/if}

{#if queue.length}
  <TheaterPlayer {queue} bind:at={playAt} source={theater} onclose={() => (queue = [])} />
{/if}
