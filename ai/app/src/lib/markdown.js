// A small, safe Markdown renderer for chat replies. Everything is HTML-escaped first, then only a fixed set of
// tags is produced (code blocks, inline code, headings, bold/italic, lists, quotes, links to http(s) only),
// so model output can never inject markup or scripts.

const ESC = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }
const escape = (/** @type {string} */ s) => s.replace(/[&<>"']/g, (c) => ESC[c])

function inline(/** @type {string} */ s) {
  const codes = []
  // Inline code first, so nothing inside it is formatted.
  s = s.replace(/`([^`\n]+)`/g, (_, c) => `\u0000${codes.push(c) - 1}\u0000`)
  s = s
    .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*\w])\*([^*\n]+)\*(?!\w)/g, '$1<em>$2</em>')
    .replace(/(^|[^_\w])_([^_\n]+)_(?!\w)/g, '$1<em>$2</em>')
    .replace(/~~([^~\n]+)~~/g, '<del>$1</del>')
    .replace(/\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>')
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[Number(i)]}</code>`)
}

/** @param {string} md @returns {string} HTML */
export function renderMarkdown(md) {
  const lines = escape(md || '').split('\n')
  const out = []
  let i = 0
  while (i < lines.length) {
    const line = lines[i]
    const fence = /^```\s*([\w+-]*)\s*$/.exec(line)
    if (fence) {
      const body = []
      i++
      while (i < lines.length && !/^```\s*$/.test(lines[i])) body.push(lines[i++])
      i++
      out.push(`<pre data-lang="${fence[1]}"><code>${body.join('\n')}</code></pre>`)
      continue
    }
    const h = /^(#{1,4})\s+(.*)$/.exec(line)
    if (h) { out.push(`<h${h[1].length + 2}>${inline(h[2])}</h${h[1].length + 2}>`); i++; continue }
    if (/^\s*([-*_])\s*\1\s*\1[\s\1]*$/.test(line)) { out.push('<hr>'); i++; continue }
    if (/^&gt;\s?/.test(line)) {
      const q = []
      while (i < lines.length && /^&gt;\s?/.test(lines[i])) q.push(lines[i++].replace(/^&gt;\s?/, ''))
      out.push(`<blockquote>${inline(q.join('<br>'))}</blockquote>`)
      continue
    }
    const ul = /^\s*[-*+]\s+/, ol = /^\s*\d+[.)]\s+/
    if (ul.test(line) || ol.test(line)) {
      const tag = ul.test(line) ? 'ul' : 'ol'
      const re = tag === 'ul' ? ul : ol
      const items = []
      while (i < lines.length && re.test(lines[i])) items.push(`<li>${inline(lines[i++].replace(re, ''))}</li>`)
      out.push(`<${tag}>${items.join('')}</${tag}>`)
      continue
    }
    if (!line.trim()) { i++; continue }
    const para = []
    while (i < lines.length && lines[i].trim() && !/^```|^#{1,4}\s|^&gt;|^\s*[-*+]\s|^\s*\d+[.)]\s/.test(lines[i])) para.push(lines[i++])
    out.push(`<p>${inline(para.join('<br>'))}</p>`)
  }
  return out.join('')
}
