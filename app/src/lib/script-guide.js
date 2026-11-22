// The Castle script guide, as the app shows it: the language's own pages
// (clonewars/automation/README.md and docs/*.md) and its example scripts, read at build time
// so the app and the repository say the same thing. Each page is rendered once to HTML with
// heading anchors, links between pages turned into in-app jumps, and every whole script
// (a ```castle block) numbered so the page can offer to try it. The Markdown reader is
// Castle's own (markdown.js): the guide needs no library.
import { markdown, esc } from './markdown.js';

const ROOT = 'app/clonewars/automation';
const REPO = 'https://github.com/YEAHDOGS/castle/blob/master/';

const files = import.meta.glob(['../../clonewars/automation/README.md', '../../clonewars/automation/docs/*.md'], { query: '?raw', import: 'default', eager: true });
const exampleFiles = import.meta.glob('../../clonewars/automation/examples/*.castle', { query: '?raw', import: 'default', eager: true });

/** The pages, in reading order: id, label, and where the page lives in the repository. */
const ORDER = [
  { id: 'overview', label: 'Overview', path: `${ROOT}/README.md` },
  { id: 'tutorial', label: 'Your first script', path: `${ROOT}/docs/tutorial.md` },
  { id: 'language', label: 'The language', path: `${ROOT}/docs/language.md` },
  { id: 'cookbook', label: 'Cookbook', path: `${ROOT}/docs/cookbook.md` },
  { id: 'machines', label: 'Machines & browsers', path: `${ROOT}/docs/machines.md` },
  { id: 'results', label: 'Results & daily update', path: `${ROOT}/docs/results.md` },
  { id: 'examples', label: 'Examples', path: `${ROOT}/examples` },
];

const source = (path) => files[`../../clonewars/${path.slice('app/clonewars/'.length)}`] || '';

/** "# Walmart motor oil prices. Searches..." -> "Walmart motor oil prices". */
export function scriptName(script, fallback = 'My Castle script') {
  const comment = script.split('\n').map((l) => l.trim()).find((l) => l.startsWith('#'));
  const name = comment?.replace(/^#+\s*/, '').split(/[:.]\s/)[0].replace(/[.:]$/, '').trim();
  return name && name.length <= 60 ? name : fallback;
}

/** The bot fields a script asks for ({NAME} in capitals), ready for the automation's editor. */
export function scriptFields(script) {
  const seen = new Set();
  for (const m of script.matchAll(/\{([A-Z][A-Z0-9_]*)\}/g)) seen.add(m[1]);
  return [...seen].map((env) => ({
    env,
    label: env.toLowerCase().split('_').join(' ').replace(/^./, (c) => c.toUpperCase()),
    secret: /PASSWORD|SECRET|TOKEN|KEY|PIN|CODE/.test(env),
  }));
}

function examplesMarkdown() {
  const parts = ['# Examples', '', 'Whole scripts to read, run and change. Every box has the Walmart one as a built-in automation; the other two are the tutorial\'s. Press **Try this script** to start a new automation from one.', ''];
  const entries = Object.entries(exampleFiles).sort(([a], [b]) => (a.includes('walmart') ? -1 : b.includes('walmart') ? 1 : a.localeCompare(b)));
  for (const [file, text] of entries) {
    parts.push(`## ${scriptName(text, file.split('/').pop().replace('.castle', ''))}`, '', '```castle', text.trimEnd(), '```', '');
  }
  return parts.join('\n');
}

/** GitHub's anchor for a heading, so links written for the repository work here too. */
function slug(text) {
  return text
    .replace(/<[^>]+>/g, '')
    .replace(/&[a-z]+;|&#\d+;/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\- _]/gu, '')
    .trim()
    .replace(/ /g, '-');
}

function resolve(dir, rel) {
  const parts = dir.split('/');
  for (const seg of rel.split('/')) {
    if (seg === '..') parts.pop();
    else if (seg && seg !== '.') parts.push(seg);
  }
  return parts.join('/');
}

function render(page) {
  const dir = page.path.endsWith('.md') ? page.path.split('/').slice(0, -1).join('/') : page.path;
  const scripts = [];
  const toc = [];
  const used = new Map();
  let lastHeading = '';
  const hooks = {
    heading({ depth, inner }) {
      let id = slug(inner);
      const n = used.get(id) || 0;
      used.set(id, n + 1);
      if (n) id = `${id}-${n}`;
      const plain = inner.replace(/<[^>]+>/g, '').replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&#39;/g, "'").replace(/&amp;/g, '&');
      if (depth === 2) toc.push({ id, text: plain });
      if (depth > 1) lastHeading = plain;
      return `<h${depth} id="${id}">${inner}</h${depth}>`;
    },
    code({ text, lang }) {
      if (lang === 'castle') {
        const i = scripts.push({ script: text + '\n', name: scriptName(text, page.id === 'overview' ? 'My Castle script' : lastHeading || 'My Castle script') }) - 1;
        return `<div class="guide-script"><div class="guide-script-bar"><span>CASTLE SCRIPT</span><button type="button" data-try="${i}">TRY THIS SCRIPT</button></div><pre><code>${esc(text)}</code></pre></div>`;
      }
      return `<pre class="guide-code"${lang ? ` data-lang="${esc(lang)}"` : ''}><code>${esc(text)}</code></pre>`;
    },
    link({ href, title, inner }) {
      const t = title ? ` title="${esc(title)}"` : '';
      if (/^https?:\/\//.test(href)) return `<a href="${esc(href)}"${t} target="_blank" rel="noreferrer noopener">${inner}</a>`;
      if (href.startsWith('#')) return `<a href="${esc(href)}" data-anchor="${esc(href.slice(1))}"${t}>${inner}</a>`;
      const [path, anchor = ''] = href.split('#');
      const target = resolve(dir, path).replace(/\/$/, '');
      const known = ORDER.find((p) => p.path === target || (p.id === 'examples' && target.startsWith(`${ROOT}/examples`)));
      if (known) return `<a href="#${esc(anchor)}" data-page="${known.id}" data-anchor="${esc(anchor)}"${t}>${inner}</a>`;
      return `<a href="${esc(REPO + target)}"${t} target="_blank" rel="noreferrer noopener">${inner}</a>`;
    },
    table({ header, rows }) {
      // Wide tables scroll inside the page rather than pushing it sideways.
      const head = header.map((c) => `<th>${c}</th>`).join('');
      const body = rows.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('');
      return `<div class="guide-table"><table><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table></div>`;
    },
  };
  const text = page.id === 'examples' ? examplesMarkdown() : source(page.path);
  const html = markdown(text, hooks);
  const title = (text.match(/^# (.+)$/m)?.[1] || page.label).replace(/`/g, '');
  return { ...page, html, scripts, toc, title };
}

let cache = null;
/** Every page of the guide, rendered (once). */
export function guidePages() {
  cache ??= ORDER.map(render);
  return cache;
}
