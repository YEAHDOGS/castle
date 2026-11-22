<!-- ScriptGuide: the Castle script language, in the app. The same pages the repository keeps
     beside the compiler (clonewars/automation): the overview, the tutorial, the whole language,
     the cookbook, machines and browsers, results, and the example scripts. Pages are listed
     down the side with the current one's sections under it; every whole script has a button
     that starts a new automation from it. -->
<script>
  import { tick } from 'svelte';
  import { BookOpen, ChevronRight } from '@lucide/svelte';
  import { guidePages, scriptFields } from '../../lib/script-guide.js';

  /** ontry({ name, script, fields }): start a new automation from a script. null: not here. */
  let { ontry = null } = $props();

  const pages = guidePages();
  let current = $state('overview');
  let top = $state(null);
  let article = $state(null);
  const page = $derived(pages.find((p) => p.id === current) || pages[0]);
  const at = $derived(pages.indexOf(page));

  async function go(id, anchor = '') {
    current = id;
    await tick();
    const el = anchor && article?.querySelector(`[id="${CSS.escape(anchor)}"]`);
    (el || top)?.scrollIntoView({ block: 'start', behavior: el ? 'smooth' : 'auto' });
  }

  /** Links between pages, anchors, and the Try buttons, for every page's HTML at once. */
  function click(e) {
    const tryButton = e.target.closest('button[data-try]');
    if (tryButton) {
      const s = page.scripts[Number(tryButton.dataset.try)];
      if (s && ontry) ontry({ name: s.name, script: s.script, fields: scriptFields(s.script) });
      return;
    }
    const link = e.target.closest('a[data-page], a[data-anchor]');
    if (!link) return;
    e.preventDefault();
    go(link.dataset.page || current, link.dataset.anchor || '');
  }
</script>

<div bind:this={top} class="grid scroll-mt-24 gap-5 lg:grid-cols-[14rem_minmax(0,1fr)]">
  <nav class="lg:sticky lg:top-4 lg:self-start" aria-label="Castle script guide">
    <p class="micro mb-2 flex items-center gap-2 text-text-faint"><BookOpen class="h-3.5 w-3.5" /> CASTLE SCRIPT</p>
    <ul class="flex flex-wrap gap-1 lg:block lg:space-y-0.5">
      {#each pages as p (p.id)}
        <li>
          <button
            type="button"
            class="w-full px-2 py-1.5 text-left text-[13px] {p.id === current ? 'border-l-2 border-chiral bg-chiral-dim text-white' : 'border-l-2 border-transparent text-text-dim hover:text-white'}"
            aria-current={p.id === current ? 'page' : undefined}
            onclick={() => go(p.id)}>{p.label}</button
          >
          {#if p.id === current && p.toc.length > 1}
            <ul class="mb-2 mt-1 hidden space-y-0.5 border-l border-hairline lg:block">
              {#each p.toc as h (h.id)}
                <li><button type="button" class="block w-full truncate py-0.5 pl-3 text-left text-[12px] text-text-faint hover:text-white" title={h.text} onclick={() => go(p.id, h.id)}>{h.text}</button></li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  </nav>

  <div class="min-w-0">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <article bind:this={article} class="guide hud-corners glass min-w-0 p-5 sm:p-7 {ontry ? '' : 'no-try'}" onclick={click}>
      {@html page.html}
    </article>
    <div class="mt-4 flex flex-wrap gap-2">
      {#if at > 0}
        <button type="button" class="micro flex-1 border border-hairline px-3 py-3 text-left text-text-dim hover:text-white" onclick={() => go(pages[at - 1].id)}>← {pages[at - 1].label.toUpperCase()}</button>
      {/if}
      {#if at < pages.length - 1}
        <button type="button" class="micro flex flex-1 items-center justify-end gap-1 border border-hairline px-3 py-3 text-right text-text-dim hover:text-white" onclick={() => go(pages[at + 1].id)}>{pages[at + 1].label.toUpperCase()} <ChevronRight class="h-3.5 w-3.5" /></button>
      {/if}
    </div>
  </div>
</div>

<style>
  .guide {
    color: var(--color-text-dim);
    font-size: 14px;
    line-height: 1.7;
  }
  .guide :global(h1) {
    color: #fff;
    font-size: 26px;
    font-weight: 700;
    line-height: 1.25;
    margin: 0 0 1rem;
  }
  .guide :global(h2) {
    color: #fff;
    font-size: 19px;
    font-weight: 700;
    margin: 2.25rem 0 0.75rem;
    padding-top: 1rem;
    border-top: 1px solid var(--color-hairline);
    scroll-margin-top: 1.5rem;
  }
  .guide :global(h3) {
    color: #fff;
    font-size: 15px;
    font-weight: 600;
    margin: 1.75rem 0 0.5rem;
    scroll-margin-top: 1.5rem;
  }
  .guide :global(h4) {
    color: #fff;
    font-size: 14px;
    font-weight: 600;
    margin: 1.5rem 0 0.4rem;
    scroll-margin-top: 1.5rem;
  }
  .guide :global(h4 code) {
    color: var(--color-chiral);
  }
  .guide :global(p),
  .guide :global(ul),
  .guide :global(ol),
  .guide :global(blockquote) {
    margin: 0 0 0.9rem;
  }
  .guide :global(ul) {
    list-style: disc;
    padding-left: 1.25rem;
  }
  .guide :global(ol) {
    list-style: decimal;
    padding-left: 1.4rem;
  }
  .guide :global(li) {
    margin: 0.2rem 0;
  }
  .guide :global(li > p) {
    margin: 0;
  }
  .guide :global(strong) {
    color: #fff;
    font-weight: 600;
  }
  .guide :global(a) {
    color: var(--color-bridge);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .guide :global(code) {
    font-family: var(--font-mono2);
    font-size: 12.5px;
    color: #fff;
    background: rgba(255, 255, 255, 0.06);
    padding: 0.05rem 0.3rem;
  }
  .guide :global(pre) {
    margin: 0 0 1rem;
    overflow-x: auto;
    border: 1px solid var(--color-hairline);
    background: rgba(0, 0, 0, 0.5);
    padding: 0.85rem 1rem;
    line-height: 1.6;
  }
  .guide :global(pre code) {
    background: none;
    padding: 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.85);
    white-space: pre;
  }
  .guide :global(blockquote) {
    border-left: 2px solid var(--color-chiral);
    background: var(--color-chiral-dim);
    padding: 0.6rem 0.9rem;
    color: rgba(255, 255, 255, 0.8);
  }
  .guide :global(blockquote p) {
    margin: 0;
  }
  .guide :global(.guide-table) {
    margin: 0 0 1rem;
    overflow-x: auto;
    border: 1px solid var(--color-hairline);
  }
  .guide :global(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  .guide :global(th) {
    font-family: var(--font-mono2);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    font-weight: 400;
    color: var(--color-text-faint);
    text-align: left;
    padding: 0.5rem 0.75rem;
    background: rgba(255, 255, 255, 0.03);
  }
  .guide :global(td) {
    padding: 0.5rem 0.75rem;
    border-top: 1px solid var(--color-hairline);
    vertical-align: top;
  }
  .guide :global(.guide-script) {
    margin: 0 0 1rem;
    border: 1px solid rgba(255, 106, 0, 0.35);
  }
  .guide :global(.guide-script pre) {
    margin: 0;
    border: 0;
  }
  .guide :global(.guide-script-bar) {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.35rem 0.5rem 0.35rem 1rem;
    border-bottom: 1px solid rgba(255, 106, 0, 0.25);
    background: var(--color-chiral-dim);
    font-family: var(--font-mono2);
    font-size: 10px;
    letter-spacing: 0.14em;
    color: var(--color-chiral);
  }
  .guide :global(.guide-script-bar span) {
    flex: 1;
  }
  .guide :global(.guide-script-bar button) {
    border: 1px solid rgba(255, 106, 0, 0.6);
    padding: 0.3rem 0.6rem;
    color: #fff;
    letter-spacing: 0.14em;
    cursor: pointer;
  }
  .guide :global(.guide-script-bar button:hover) {
    background: rgba(255, 106, 0, 0.25);
  }
  .guide.no-try :global(.guide-script-bar button) {
    display: none;
  }
</style>
