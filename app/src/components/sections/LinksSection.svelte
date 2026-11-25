<!-- LinksSection: pages a person saved from the internet, read and kept by the box
     (castled/src/links.rs). Paste an address, or share one to Castle from a phone, and the
     box fetches the page once: its title, picture and text, its recipe when it carries
     one, and for a video the recipe its description points at. Search looks through all
     of that, so "brown sugar" finds the pie.

     The rail has All, a row per kind that has anything (Recipes, Videos, Pages), and a row
     per tag. Opening a link shows what the box kept; the original is one tap away.

     With a controller: the D-pad walks the cards, A opens one, B goes back. -->
<script>
  import { Bookmark, Search, Plus, LoaderCircle, ChevronLeft, Trash2, ExternalLink, RefreshCw, CookingPot, Play, FileText, Tag, CircleAlert, Clock, Users, Link2, ChevronDown } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { fly, fade } from 'svelte/transition';
  import { SvelteMap } from 'svelte/reactivity';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { ask } from '../../lib/ask.svelte.js';
  import { setHints } from '../../lib/input.svelte.js';
  import { fmtDate } from '../../lib/format.js';
  import { itemIn } from '../../lib/motion.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';

  const ALL = '@all';
  const KINDS = [
    { id: 'recipe', label: 'Recipes', icon: CookingPot },
    { id: 'video', label: 'Videos', icon: Play },
    { id: 'page', label: 'Pages', icon: FileText },
  ];
  const TAG = 'tag:';
  const MAX_TAG_ROWS = 10;
  const SEARCH_WAIT_MS = 300;
  const STAGGER_CAP = 20;
  const PICTURE_MARGIN = '300px';

  let cards = $state(null); // every link of this person, newest first; null until read
  let hits = $state(null); // what a search found, or null when not searching
  let problem = $state('');
  let view = $state(ALL);
  let query = $state('');
  let searching = $state(false);
  let address = $state('');
  let saving = $state(false);
  let open = $state(null); // the whole link showing in the drawer
  let opening = $state(false);
  let rereading = $state(false);
  let note = $state('');
  let tagText = $state('');
  let showText = $state(false);
  const pictures = new SvelteMap(); // link id -> picture address, or null

  const source = $derived(shell.linksSource);
  const person = $derived(shell.user.id);
  /** A link counts as a recipe when it has one, whatever kind of page carried it. */
  const kindOf = (card) => (card.hasRecipe ? 'recipe' : card.kind);
  const counts = $derived(Object.fromEntries(KINDS.map((k) => [k.id, (cards || []).filter((c) => kindOf(c) === k.id).length])));
  const tags = $derived.by(() => {
    const seen = new Map();
    for (const c of cards || []) for (const t of c.tags) seen.set(t, (seen.get(t) || 0) + 1);
    return [...seen].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, MAX_TAG_ROWS);
  });
  const railItems = $derived([
    { id: ALL, label: 'All', icon: Bookmark, count: cards?.length ?? null },
    ...KINDS.filter((k) => counts[k.id]).map((k) => ({ id: k.id, label: k.label, icon: k.icon, count: counts[k.id] })),
    ...tags.map(([t, n]) => ({ id: TAG + t, label: `#${t}`, icon: Tag, count: n })),
  ]);
  const shown = $derived.by(() => {
    const from = hits ?? cards ?? [];
    if (view === ALL) return from;
    if (view.startsWith(TAG)) return from.filter((c) => c.tags.includes(view.slice(TAG.length)));
    return from.filter((c) => kindOf(c) === view);
  });
  const subtitle = $derived(cards ? `${cards.length} ${cards.length === 1 ? 'link' : 'links'} kept on the box` : '');
  const times = $derived(open?.recipe ? [['PREP', open.recipe.prep], ['COOK', open.recipe.cook], ['TOTAL', open.recipe.total]].filter(([, value]) => value) : []);

  // ---------------------------------------------------------------- loading

  async function load(from = source, who = person) {
    problem = '';
    if (!from) {
      cards = null;
      return;
    }
    try {
      const next = await from.links(who);
      if (from === source && who === person) cards = next;
    } catch (err) {
      problem = err?.message || 'Links could not be read';
    }
  }

  // A new source or a new person starts again.
  $effect(() => {
    const from = source;
    const who = person;
    untrack(() => {
      cards = null;
      hits = null;
      query = '';
      open = null;
      view = ALL;
      pictures.clear();
      load(from, who);
    });
  });

  // Searching waits for a pause in the typing; the box does the looking.
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
        const found = await from.links(who, q);
        if (q === query.trim() && from === source && who === person) hits = found;
      } catch (err) {
        store.toast(err?.message || 'Search did not work');
      } finally {
        if (q === query.trim()) searching = false;
      }
    }, SEARCH_WAIT_MS);
    return () => clearTimeout(timer);
  });

  // A rail row that no longer exists (its last link was deleted) falls back to All.
  $effect(() => {
    if (cards && !railItems.some((item) => item.id === view)) view = ALL;
  });

  // Something was shared to Castle: save it.
  $effect(() => {
    const focus = shell.focus;
    if (focus?.section !== 'links' || !source) return;
    untrack(() => {
      shell.focus = null;
      if (focus.add) save(focus.add);
    });
  });

  $effect(() => {
    if (open) return setHints('links', [{ btn: 'A', label: 'Choose' }, { btn: 'B', label: 'Back' }], 2);
    return setHints('links', [{ btn: 'DPAD', label: 'Move' }, { btn: 'A', label: 'Open' }]);
  });

  /** Fetches a card's picture when the card comes near the screen. */
  function picture(node, card) {
    if (!card.hasPicture || pictures.has(card.id)) return;
    const from = source;
    const who = person;
    const watcher = new IntersectionObserver(
      async (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        watcher.disconnect();
        const url = await from.linkPicture(who, card.id);
        if (from === source && who === person) pictures.set(card.id, url);
      },
      { rootMargin: PICTURE_MARGIN },
    );
    watcher.observe(node);
    return { destroy: () => watcher.disconnect() };
  }

  // ---------------------------------------------------------------- changing

  async function save(text = address) {
    const wanted = text.trim();
    if (!wanted || saving || !source) return;
    saving = true;
    try {
      const link = await source.linkAdd(person, wanted);
      address = '';
      query = '';
      await load();
      show(link);
      store.toast(link.problem ? 'Saved, but the page could not be read' : link.recipe ? 'Saved, with its recipe' : 'Saved');
    } catch (err) {
      store.toast(err?.message || 'That link could not be saved');
    } finally {
      saving = false;
    }
  }

  function show(link) {
    open = link;
    note = link.note;
    tagText = link.tags.join(', ');
    showText = false;
    if (link.picture && !pictures.has(link.id)) source.linkPicture(person, link.id).then((url) => pictures.set(link.id, url));
  }

  async function openCard(card) {
    if (opening) return;
    opening = true;
    try {
      show(await source.link(person, card.id));
    } catch (err) {
      store.toast(err?.message || 'That link could not be opened');
    } finally {
      opening = false;
    }
  }

  /** Puts a changed link back into the lists without reading them all again. */
  function took(link) {
    const patch = (list) => list?.map((c) => (c.id === link.id ? { ...c, title: link.title, note: link.note, tags: link.tags, problem: link.problem, hasRecipe: !!link.recipe, hasPicture: !!link.picture, kind: link.kind, site: link.site, description: link.description.slice(0, 400) } : c)) ?? null;
    cards = patch(cards);
    hits = patch(hits);
    if (open?.id === link.id) open = link;
  }

  async function keepWords() {
    if (!open) return;
    const nextTags = tagText.split(',').map((t) => t.trim().replace(/^#/, '').toLowerCase()).filter(Boolean);
    if (note === open.note && nextTags.join(',') === open.tags.join(',')) return;
    try {
      const link = await source.linkUpdate(person, open.id, { note, tags: nextTags });
      took(link);
      tagText = link.tags.join(', ');
    } catch (err) {
      store.toast(err?.message || 'That could not be saved');
    }
  }

  async function reread() {
    if (!open || rereading) return;
    rereading = true;
    try {
      const link = await source.linkRefresh(person, open.id);
      pictures.delete(link.id);
      took(link);
      show(link);
      store.toast(link.problem ? 'The page still could not be read' : 'Read again');
    } catch (err) {
      store.toast(err?.message || 'The page could not be read');
    } finally {
      rereading = false;
    }
  }

  async function remove() {
    if (!open) return;
    if (!(await ask(`Delete “${open.title}”? The box forgets the page it kept.`, { yes: 'Delete', danger: true }))) return;
    try {
      await source.linkDelete(person, open.id);
      const id = open.id;
      open = null;
      cards = cards?.filter((c) => c.id !== id) ?? null;
      hits = hits?.filter((c) => c.id !== id) ?? null;
    } catch (err) {
      store.toast(err?.message || 'That link could not be deleted');
    }
  }

  async function close() {
    await keepWords();
    open = null;
  }

  const kindIcon = (card) => KINDS.find((k) => k.id === kindOf(card))?.icon || FileText;
  /** A saved address is only ever opened as a web page. */
  const safeHref = (url) => (/^https?:\/\//i.test(url) ? url : undefined);
  const hostOf = (url) => {
    try {
      return new URL(url).hostname.replace(/^www\./, '');
    } catch {
      return url;
    }
  };
</script>

{#snippet actions()}
  <label class="relative hidden w-56 md:block">
    {#if searching}<LoaderCircle class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 animate-spin text-text-faint" />{:else}<Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />{/if}
    <input type="search" bind:value={query} placeholder="SEARCH" class="micro h-9 w-full border border-hairline bg-white/[0.03] pl-8 pr-2 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search links" />
  </label>
{/snippet}

<SectionFrame title="Links" {subtitle} tag={store.isDemo ? 'SAMPLE DATA' : ''} items={railItems} active={view} onselect={(id) => (view = id)} {actions}>
  {#if !source}
    <Notice icon={Bookmark} title="NO BOX CONNECTED">Links are kept on your box. Connect one from Settings, or switch to Demo to look around.</Notice>
  {:else if problem}
    <Notice icon={CircleAlert} title="LINKS COULD NOT BE READ">{problem}</Notice>
  {:else}
    <div class="mx-auto max-w-6xl">
      <!-- ---------------- save a link ---------------- -->
      <form class="mb-4 flex gap-2" onsubmit={(e) => { e.preventDefault(); save(); }}>
        <label class="relative min-w-0 flex-1">
          <Link2 class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-faint" />
          <input type="text" inputmode="url" autocapitalize="off" autocomplete="off" spellcheck="false" bind:value={address} disabled={saving} placeholder="Paste a link to keep it" class="h-11 w-full border border-hairline bg-white/[0.03] pl-9 pr-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none disabled:opacity-60" aria-label="A link to save" />
        </label>
        <button type="submit" disabled={saving || !address.trim()} class="micro expo t-hover flex h-11 shrink-0 items-center gap-1.5 bg-chiral px-4 font-bold text-black hover:opacity-90 disabled:opacity-40">
          {#if saving}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />READING{:else}<Plus class="h-3.5 w-3.5" />SAVE{/if}
        </button>
      </form>
      {#if saving}<p class="micro -mt-2 mb-4 text-text-faint" aria-live="polite">THE BOX IS READING THE PAGE. THIS TAKES A FEW SECONDS.</p>{/if}

      <label class="relative mb-4 block md:hidden">
        <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
        <input type="search" bind:value={query} placeholder="SEARCH" class="micro h-10 w-full border border-hairline bg-white/[0.03] pl-8 pr-2 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Search links" />
      </label>

      {#if !cards}
        <p class="micro flex items-center gap-2 text-text-faint"><LoaderCircle class="h-3.5 w-3.5 animate-spin" />READING</p>
      {:else if !cards.length}
        <Notice icon={Bookmark} title="NOTHING SAVED YET">Paste a link above and the box reads the page and keeps it: the words, the picture, and the recipe if it has one. From a phone, share a page to Castle.</Notice>
      {:else if !shown.length}
        <Notice icon={Search} title="NOTHING MATCHES">Search looks through titles, notes, tags, ingredients and the words of every page. Try fewer words.</Notice>
      {:else}
        <ul class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {#each shown as card, i (card.id)}
            {@const Icon = kindIcon(card)}
            <li in:fly={itemIn(Math.min(i, STAGGER_CAP))}>
              <button class="nav-card expo t-hover group flex h-full w-full flex-col border border-hairline bg-white/[0.02] text-left transition-colors hover:border-white/25 hover:bg-white/[0.04]" onclick={() => openCard(card)} use:picture={card}>
                <span class="relative block aspect-[16/9] w-full overflow-hidden bg-white/[0.03]">
                  {#if pictures.get(card.id)}
                    <img src={pictures.get(card.id)} alt="" class="h-full w-full object-cover" loading="lazy" />
                  {:else}
                    <span class="grid h-full w-full place-items-center"><Icon class="h-7 w-7 text-text-faint" strokeWidth={1.25} /></span>
                  {/if}
                  <span class="micro absolute left-2 top-2 flex items-center gap-1 bg-black/70 px-1.5 py-0.5 text-white"><Icon class="h-3 w-3" />{kindOf(card).toUpperCase()}</span>
                </span>
                <span class="flex min-w-0 flex-1 flex-col px-3 py-2.5">
                  <span class="line-clamp-2 text-[14px] font-medium leading-snug text-white">{card.title}</span>
                  <span class="micro mt-1 truncate text-text-faint">{card.site.toUpperCase()} · {fmtDate(card.added).toUpperCase()}</span>
                  {#if card.problem}
                    <span class="mt-1.5 flex items-start gap-1.5 text-[12px] text-bridge"><CircleAlert class="mt-0.5 h-3.5 w-3.5 shrink-0" /><span class="line-clamp-2">Not read: {card.problem}</span></span>
                  {:else if card.snippet || card.note || card.description}
                    <span class="mt-1.5 line-clamp-2 text-[12px] leading-relaxed text-text-dim">{card.snippet || card.note || card.description}</span>
                  {/if}
                  {#if card.tags.length}
                    <span class="mt-2 flex flex-wrap gap-1">{#each card.tags as t (t)}<span class="micro border border-hairline px-1.5 py-0.5 text-text-dim">#{t.toUpperCase()}</span>{/each}</span>
                  {/if}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</SectionFrame>

<!-- ---------------- a link ---------------- -->
{#if open}
  <div class="fixed inset-0 z-40 bg-black/60" transition:fade={{ duration: 150 }} onclick={close} aria-hidden="true"></div>
  <div class="fixed inset-y-0 right-0 z-50 flex w-full max-w-xl flex-col border-l border-hairline bg-void" transition:fly={{ x: 420, duration: 240 }} role="dialog" aria-modal="true" aria-label={open.title}>
    <header class="flex items-center gap-2 border-b border-hairline py-3 pl-4 pr-16">
      <button class="grid h-9 w-9 place-items-center text-text-dim hover:text-white" onclick={close} aria-label="Close" data-nav-back><ChevronLeft class="h-5 w-5" /></button>
      <p class="micro flex-1 text-text-faint">{(open.recipe ? 'recipe' : open.kind).toUpperCase()}</p>
      <button class="grid h-9 w-9 place-items-center text-text-dim hover:text-white disabled:opacity-50" onclick={reread} disabled={rereading} aria-label="Read the page again"><RefreshCw class="h-4 w-4 {rereading ? 'animate-spin' : ''}" /></button>
      <button class="grid h-9 w-9 place-items-center text-text-dim hover:text-red-400" onclick={remove} aria-label="Delete"><Trash2 class="h-4 w-4" /></button>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto">
      {#if pictures.get(open.id)}
        <img src={pictures.get(open.id)} alt="" class="aspect-[16/9] w-full object-cover" />
      {/if}
      <div class="px-5 py-5">
        <h2 class="text-[21px] font-bold leading-tight text-white">{open.title}</h2>
        <p class="micro mt-1.5 text-text-faint">{[open.site, open.author].filter(Boolean).join(' · ').toUpperCase()} · SAVED {fmtDate(open.added).toUpperCase()}</p>
        <a href={safeHref(open.url)} target="_blank" rel="noreferrer noopener" class="micro expo t-hover mt-3 inline-flex h-9 max-w-full items-center gap-1.5 border border-hairline px-3 text-text-dim hover:text-white">
          <ExternalLink class="h-3.5 w-3.5 shrink-0" /><span class="truncate">OPEN {hostOf(open.url).toUpperCase()}</span>
        </a>

        {#if open.problem}
          <p class="mt-4 flex items-start gap-2 border border-bridge/40 px-3 py-2.5 text-[13px] leading-relaxed text-bridge">
            <CircleAlert class="mt-0.5 h-4 w-4 shrink-0" />
            <span>{open.fetched ? 'The page could not be read again, so this is the copy from before' : 'The box could not read this page'}: {open.problem}. The link is kept; the arrows above try again.</span>
          </p>
        {/if}

        {#if open.recipe}
          <section class="mt-6">
            <h3 class="micro flex items-center gap-1.5 text-chiral"><CookingPot class="h-3.5 w-3.5" />{(open.recipe.name || 'RECIPE').toUpperCase()}</h3>
            {#if open.recipe.yields || times.length}
              <p class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[13px] text-text-dim">
                {#if open.recipe.yields}<span class="flex items-center gap-1.5"><Users class="h-3.5 w-3.5 text-text-faint" />{open.recipe.yields}</span>{/if}
                {#each times as [label, value] (label)}<span class="flex items-center gap-1.5"><Clock class="h-3.5 w-3.5 text-text-faint" /><span class="micro text-text-faint">{label}</span>{value}</span>{/each}
              </p>
            {/if}
            {#if open.recipe.ingredients.length}
              <h4 class="micro mt-4 text-text-faint">INGREDIENTS</h4>
              <ul class="mt-1.5 grid gap-1">
                {#each open.recipe.ingredients as line, i (i)}<li class="border-b border-hairline-faint py-1.5 text-[14px] text-white">{line}</li>{/each}
              </ul>
            {/if}
            {#if open.recipe.steps.length}
              <h4 class="micro mt-4 text-text-faint">STEPS</h4>
              <ol class="mt-1.5 grid gap-2.5">
                {#each open.recipe.steps as step, i (i)}
                  <li class="flex gap-3 text-[14px] leading-relaxed text-text-dim"><span class="micro mt-1 w-5 shrink-0 text-chiral">{String(i + 1).padStart(2, '0')}</span><span class="min-w-0 text-white/90">{step}</span></li>
                {/each}
              </ol>
            {/if}
            {#if open.recipe.source}
              <p class="micro mt-3 text-text-faint">READ FROM <a href={safeHref(open.recipe.source)} target="_blank" rel="noreferrer noopener" class="text-chiral hover:underline">{hostOf(open.recipe.source).toUpperCase()}</a>, WHICH THE VIDEO NAMED</p>
            {/if}
          </section>
        {/if}

        <section class="mt-6">
          <h3 class="micro text-text-faint">YOUR NOTE</h3>
          <textarea bind:value={note} onblur={keepWords} rows="2" placeholder="Why you kept it" class="mt-1.5 w-full resize-y border border-hairline bg-white/[0.03] px-3 py-2 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Your note"></textarea>
          <label class="relative mt-2 block">
            <Tag class="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
            <input type="text" bind:value={tagText} onblur={keepWords} onkeydown={(e) => e.key === 'Enter' && keepWords()} autocapitalize="off" placeholder="Tags, with commas: dessert, thanksgiving" class="h-10 w-full border border-hairline bg-white/[0.03] pl-9 pr-3 text-[13px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Tags" />
          </label>
        </section>

        {#if open.description && open.kind === 'video'}
          <section class="mt-6">
            <h3 class="micro text-text-faint">DESCRIPTION</h3>
            <p class="mt-1.5 whitespace-pre-line break-words text-[13px] leading-relaxed text-text-dim">{open.description}</p>
          </section>
        {:else if open.description}
          <p class="mt-6 text-[14px] leading-relaxed text-text-dim">{open.description}</p>
        {/if}

        {#if open.related.length}
          <section class="mt-6">
            <h3 class="micro text-text-faint">LINKS IT NAMED</h3>
            <ul class="mt-1.5 grid gap-1.5">
              {#each open.related as url (url)}
                <li class="flex items-center gap-2 border border-hairline px-3 py-2">
                  <a href={safeHref(url)} target="_blank" rel="noreferrer noopener" class="min-w-0 flex-1 truncate text-[13px] text-white hover:text-chiral">{url}</a>
                  <button class="micro expo t-hover shrink-0 border border-hairline px-2 py-1 text-text-dim hover:text-white disabled:opacity-40" disabled={saving} onclick={() => save(url)}>SAVE TOO</button>
                </li>
              {/each}
            </ul>
          </section>
        {/if}

        {#if open.text && open.kind !== 'video'}
          <section class="mt-6">
            <button class="micro flex w-full items-center gap-1.5 text-text-faint hover:text-white" onclick={() => (showText = !showText)} aria-expanded={showText}>
              <ChevronDown class="h-3.5 w-3.5 transition-transform {showText ? '' : '-rotate-90'}" />WHAT THE PAGE SAID
            </button>
            {#if showText}
              <p class="mt-2 whitespace-pre-line break-words text-[14px] leading-relaxed text-text-dim" transition:fade={{ duration: 120 }}>{open.text}</p>
            {/if}
          </section>
        {/if}
      </div>
    </div>
  </div>
{/if}
