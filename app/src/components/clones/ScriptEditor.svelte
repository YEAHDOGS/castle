<!-- ScriptEditor: where a Castle script is written. Line numbers down the side, problems from
     the box under it as you type (the box compiles the script, so what it says is what will
     run), the steps the language knows, and the Python the script becomes. -->
<script>
  import { untrack } from 'svelte';
  import { CircleAlert, CircleCheck, BookOpen, Code } from '@lucide/svelte';
  import { BTN, LABEL } from '../../lib/clones-ui.js';

  /** value: the script (bindable). check(script) resolves the box's verdict. */
  let { value = $bindable(''), check } = $props();

  const CHECK_AFTER_MS = 450;
  const LINE_PX = 20;
  /** The steps, as the language reference shows them (clonewars/automation/README.md). */
  const STEPS = [
    ['castle 1', 'The language version, on the first line.'],
    ['website walmart.com', 'Opens a browser on the site. Every web step needs it first.'],
    ['element search box = input[name="q"]', 'A name for something on the page. Selectors are CSS, or text "Search", label "Search", placeholder "…", title "…".'],
    ['list results = .item', 'A name for a repeating thing, with its fields indented under it.'],
    ['  field price = .price', 'One field of each item: its text, or an attribute with @href.'],
    ['go to walmart.com/cart', 'Opens another page.'],
    ['type "motor oil" into search box', 'Types text. {NAME} puts in a sign-in field, {name} a value.'],
    ['click search button', 'Clicks an element.'],
    ['press enter', 'Presses a key, or press enter in search box.'],
    ['wait for results  /  wait 5 seconds', 'Waits until it shows, or for a while.'],
    ['scroll down  /  scroll to the bottom', 'Scrolls, so things that load on scroll appear.'],
    ['read results', 'Reads every item of a list into rows.'],
    ['read title as page title', 'Reads one element\'s text into a value.'],
    ['keep results where title has "0W-20"', 'Also: matches "pattern", is empty, is not empty, is below 10, is above 10. drop is the opposite.'],
    ['keep results unique by id', 'The first of each, so a listing shown twice counts once.'],
    ['measure results title in quarts as quarts', 'Reads an amount from text: quarts, gallons, liters, milliliters, fluid ounces, pounds, ounces, grams, kilograms, items.'],
    ['divide results price by quarts as price per quart', 'Works out a new field. multiply too; by a field or a number.'],
    ['sort results by price lowest first', 'Or highest first.'],
    ['average results price as average price', 'Also sum, min, max, median. lowest and highest give the whole row.'],
    ['count results as listings found', 'How many rows.'],
    ['post results to castle', 'Saves rows or a value in Castle\'s database. as "name" picks the dataset.'],
    ['track average price lower is better', 'Keeps a number day after day; Castle says how today compares.'],
    ['say "found {listings found}"', 'Writes a line to the run\'s log.'],
    ['screenshot "the results"', 'Keeps a picture of the page with the run.'],
    ['fail if results is empty "why"', 'Ends the run when a list or value is empty, or: fail if robot check is shown.'],
    ['pause whenever robot check is shown "what to do"', 'From here on: when the page shows it, the run waits for you to deal with it (take control), then goes on. fail whenever ends the run instead.'],
    ['python:', 'Indented Python under it, for what the language cannot say yet.'],
  ];

  let problems = $state([]);
  let python = $state('');
  let state = $state('checking'); // checking | ok | problems | offline
  let showSteps = $state(false);
  let showPython = $state(false);
  let area = $state(null);
  let gutter = $state(null);
  let timer;

  const lineCount = $derived(Math.max(1, value.split('\n').length));
  const badLines = $derived(new Set(problems.map((p) => p.line)));

  async function verify(text) {
    state = 'checking';
    try {
      const verdict = await check(text);
      if (text !== value) return; // typed on since
      problems = verdict.problems || [];
      python = verdict.python || '';
      state = verdict.ok ? 'ok' : 'problems';
    } catch {
      state = 'offline';
    }
  }

  $effect(() => {
    const text = value;
    clearTimeout(timer);
    timer = setTimeout(() => untrack(() => verify(text)), CHECK_AFTER_MS);
    return () => clearTimeout(timer);
  });

  /** Puts the caret at the start of a line and brings it into view. */
  function goTo(line) {
    if (!area) return;
    const lines = value.split('\n');
    const at = lines.slice(0, line - 1).reduce((n, l) => n + l.length + 1, 0);
    area.focus();
    area.setSelectionRange(at, at + (lines[line - 1] || '').length);
    area.scrollTop = Math.max(0, (line - 4) * LINE_PX);
  }

  /** Tab indents, the way a field goes under its list. */
  function keydown(e) {
    if (e.key !== 'Tab' || e.shiftKey) return;
    e.preventDefault();
    const { selectionStart: a, selectionEnd: b } = e.currentTarget;
    value = value.slice(0, a) + '  ' + value.slice(b);
    requestAnimationFrame(() => area?.setSelectionRange(a + 2, a + 2));
  }
</script>

<div>
  <div class="flex items-center gap-2">
    <span class="{LABEL} flex-1">THE SCRIPT</span>
    <p class="micro mb-1 flex items-center gap-1.5 {state === 'ok' ? 'text-live' : state === 'problems' ? 'text-chiral' : 'text-text-faint'}" role="status">
      {#if state === 'ok'}<CircleCheck class="h-3.5 w-3.5" /> READY TO RUN
      {:else if state === 'problems'}<CircleAlert class="h-3.5 w-3.5" /> {problems.length} PROBLEM{problems.length === 1 ? '' : 'S'}
      {:else if state === 'offline'}THE BOX DID NOT ANSWER
      {:else}CHECKING{/if}
    </p>
  </div>
  <div class="flex h-[22rem] overflow-hidden border border-hairline bg-black/40 focus-within:border-chiral/60">
    <pre bind:this={gutter} aria-hidden="true" class="w-11 shrink-0 select-none overflow-hidden border-r border-hairline py-2 pr-2 text-right font-mono text-[12px] leading-[20px] text-text-faint">{#each { length: lineCount } as _, i (i)}<span class={badLines.has(i + 1) ? 'text-chiral' : ''}>{i + 1}</span>
{/each}</pre>
    <textarea
      bind:this={area}
      bind:value
      onscroll={(e) => gutter && (gutter.scrollTop = e.currentTarget.scrollTop)}
      onkeydown={keydown}
      spellcheck="false"
      autocapitalize="off"
      autocomplete="off"
      wrap="off"
      aria-label="Castle script"
      class="min-w-0 flex-1 resize-none bg-transparent px-3 py-2 font-mono text-[12px] leading-[20px] text-white focus:outline-none"
    ></textarea>
  </div>

  {#if problems.length}
    <ul class="mt-2 max-h-36 space-y-1 overflow-auto">
      {#each problems as p, i (i)}
        <li>
          <button type="button" class="flex w-full gap-3 text-left text-[12px] leading-relaxed text-text-dim hover:text-white" onclick={() => goTo(p.line)}>
            <span class="micro w-14 shrink-0 pt-0.5 text-chiral">LINE {p.line}</span><span>{p.message}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="mt-2 flex flex-wrap gap-2">
    <button type="button" class={BTN} aria-pressed={showSteps} onclick={() => (showSteps = !showSteps)}><BookOpen class="h-3.5 w-3.5" /> STEPS</button>
    <button type="button" class={BTN} aria-pressed={showPython} disabled={!python} onclick={() => (showPython = !showPython)}><Code class="h-3.5 w-3.5" /> THE PYTHON IT BECOMES</button>
  </div>
  {#if showSteps}
    <dl class="mt-3 grid gap-x-4 gap-y-2 border border-hairline bg-black/30 p-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]">
      {#each STEPS as [example, meaning] (example)}
        <dt class="font-mono text-[12px] text-white">{example}</dt>
        <dd class="text-[12px] leading-relaxed text-text-dim">{meaning}</dd>
      {/each}
    </dl>
    <p class="mt-1 text-[12px] text-text-dim">The whole language, a tutorial and recipes are under <span class="text-white">Clones → Castle script</span>.</p>
  {/if}
  {#if showPython && python}
    <pre class="mt-3 max-h-72 overflow-auto border border-hairline bg-black/60 p-3 font-mono text-[11px] leading-relaxed text-text-dim">{python}</pre>
    <p class="mt-1 text-[12px] text-text-dim">Castle writes this again from the script on every start, with the castle library beside it. Change the script, not the Python.</p>
  {/if}
</div>
