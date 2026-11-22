// A small Markdown reader for Castle's own pages (the Castle script guide), so the app needs
// no Markdown library. It reads the part of Markdown those pages use, and nothing more:
//
//   # headings              paragraphs              > quotes
//   - and 1. lists (nested, with code or paragraphs inside an item)
//   | tables |              ```fenced code```       `code`, **strong**, *em*, [links](href)
//
// Raw HTML in the source is shown as text, never run. The page decides how headings, code
// blocks, links and tables look through `hooks` (see script-guide.js); without them they are
// plain HTML.

export const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

const FENCE = /^(\s*)```\s*([\w-]*)\s*$/;
const HEADING = /^(#{1,6})\s+(.+?)\s*#*\s*$/;
const ITEM = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
const TABLE_RULE = /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/;

/** Inline text: code spans first (nothing inside them is read), then links, strong and em. */
export function inline(text, hooks = {}) {
  const held = [];
  const hold = (html) => `\u0000${held.push(html) - 1}\u0000`;
  let s = text.replace(/(`+)([\s\S]*?[^`])\1(?!`)/g, (_, _ticks, code) => hold(`<code>${esc(code.trim() || code)}</code>`));
  s = s.replace(/\\([\\`*_{}[\]()#+\-.!|>])/g, (_, c) => hold(esc(c)));
  s = esc(s);
  s = s.replace(/\[([^\]]*)\]\(([^)\s]+)(?:\s+&quot;([^"]*?)&quot;)?\)/g, (_, label, href, title) => {
    const inner = emphasis(label);
    const url = href.replace(/&amp;/g, '&');
    return hold(hooks.link ? hooks.link({ href: url, title: title || '', inner }) : `<a href="${esc(url)}">${inner}</a>`);
  });
  s = emphasis(s);
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => held[Number(i)]).replace(/\u0000(\d+)\u0000/g, (_, i) => held[Number(i)]);
}

function emphasis(s) {
  return s.replace(/\*\*(?=\S)([\s\S]*?\S)\*\*/g, '<strong>$1</strong>').replace(/(^|[^*\w])\*(?=\S)([^*]*?\S)\*(?![*\w])/g, '$1<em>$2</em>');
}

/** A table row's cells. A | inside `code` belongs to the cell. */
function cells(line) {
  let row = line.trim();
  if (row.startsWith('|')) row = row.slice(1);
  if (row.endsWith('|') && !row.endsWith('\\|')) row = row.slice(0, -1);
  const out = [];
  let cell = '';
  let ticks = false;
  for (let i = 0; i < row.length; i++) {
    const c = row[i];
    if (c === '\\' && row[i + 1] === '|') {
      cell += '|';
      i++;
    } else if (c === '`') {
      ticks = !ticks;
      cell += c;
    } else if (c === '|' && !ticks) {
      out.push(cell.trim());
      cell = '';
    } else {
      cell += c;
    }
  }
  out.push(cell.trim());
  return out;
}

const indentOf = (line) => line.length - line.trimStart().length;
const blank = (line) => line.trim() === '';

/** Lines, as blocks of HTML. */
function blocks(lines, hooks) {
  const out = [];
  let i = 0;
  const startsBlock = (line, next) => FENCE.test(line) || HEADING.test(line) || /^\s*>/.test(line) || ITEM.test(line) || (line.trim().startsWith('|') && next !== undefined && TABLE_RULE.test(next));

  while (i < lines.length) {
    const line = lines[i];
    if (blank(line)) {
      i++;
      continue;
    }

    let m = line.match(FENCE);
    if (m) {
      const pad = m[1].length;
      const lang = m[2];
      const body = [];
      i++;
      while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) {
        body.push(lines[i].slice(Math.min(pad, indentOf(lines[i]))));
        i++;
      }
      i++; // the closing fence
      const text = body.join('\n');
      out.push(hooks.code ? hooks.code({ text, lang }) : `<pre><code>${esc(text)}</code></pre>`);
      continue;
    }

    m = line.match(HEADING);
    if (m) {
      const depth = m[1].length;
      const inner = inline(m[2], hooks);
      out.push(hooks.heading ? hooks.heading({ depth, inner }) : `<h${depth}>${inner}</h${depth}>`);
      i++;
      continue;
    }

    if (line.trim().startsWith('|') && i + 1 < lines.length && TABLE_RULE.test(lines[i + 1])) {
      const header = cells(line).map((c) => inline(c, hooks));
      const rows = [];
      i += 2;
      while (i < lines.length && lines[i].trim().startsWith('|')) {
        rows.push(cells(lines[i]).map((c) => inline(c, hooks)));
        i++;
      }
      out.push(hooks.table ? hooks.table({ header, rows }) : `<table><thead><tr>${header.map((h) => `<th>${h}</th>`).join('')}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</tbody></table>`);
      continue;
    }

    if (/^\s*>/.test(line)) {
      const body = [];
      while (i < lines.length && /^\s*>/.test(lines[i])) {
        body.push(lines[i].replace(/^\s*>\s?/, ''));
        i++;
      }
      out.push(`<blockquote>${blocks(body, hooks)}</blockquote>`);
      continue;
    }

    m = line.match(ITEM);
    if (m) {
      const base = m[1].length;
      const ordered = /\d/.test(m[2]);
      const items = [];
      let loose = false;
      while (i < lines.length) {
        const im = lines[i].match(ITEM);
        if (!im || im[1].length !== base || /\d/.test(im[2]) !== ordered) break;
        // Where the item's text starts: the indent its later lines line up with.
        const content = base + im[2].length + lines[i].slice(base + im[2].length).match(/^\s*/)[0].length;
        const body = [im[3]];
        i++;
        // An item goes on while lines are indented past its marker, across single blank lines.
        while (i < lines.length) {
          if (blank(lines[i])) {
            const next = lines.slice(i + 1).find((l) => !blank(l));
            if (next !== undefined && indentOf(next) > base && !(ITEM.test(next) && indentOf(next) === base)) {
              body.push('');
              i++;
              continue;
            }
            break;
          }
          if (indentOf(lines[i]) > base) {
            body.push(lines[i].slice(Math.min(content, indentOf(lines[i]))));
            i++;
            continue;
          }
          // A line that is not indented carries the paragraph on (lazy), unless it starts something.
          if (!startsBlock(lines[i], lines[i + 1]) && !blank(body[body.length - 1])) {
            body.push(lines[i].trim());
            i++;
            continue;
          }
          break;
        }
        if (i < lines.length && blank(lines[i])) {
          const next = lines.slice(i).find((l) => !blank(l));
          const nm = next?.match(ITEM);
          if (nm && nm[1].length === base && /\d/.test(nm[2]) === ordered) {
            loose = true;
            while (blank(lines[i])) i++;
          }
        }
        if (body.includes('')) loose = true;
        items.push(body);
      }
      const html = items.map((body) => {
        const inner = blocks(body, hooks);
        // A tight list's items are their text, not paragraphs.
        return `<li>${loose ? inner : inner.replace(/^<p>([\s\S]*?)<\/p>/, '$1')}</li>`;
      });
      const start = ordered ? Number(m[2].replace(/\D/g, '')) : 1;
      out.push(ordered ? `<ol${start !== 1 ? ` start="${start}"` : ''}>${html.join('')}</ol>` : `<ul>${html.join('')}</ul>`);
      continue;
    }

    const para = [];
    while (i < lines.length && !blank(lines[i]) && !(para.length && startsBlock(lines[i], lines[i + 1]))) {
      para.push(lines[i].trim());
      i++;
    }
    out.push(`<p>${inline(para.join('\n'), hooks)}</p>`);
  }
  return out.join('\n');
}

/** Markdown to HTML. hooks: { heading, code, link, table } to shape those parts. */
export function markdown(text, hooks = {}) {
  return blocks(text.replace(/\r\n?/g, '\n').split('\n'), hooks);
}
