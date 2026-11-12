// Turns the server's model scan and Ollama list into Library tiles, and works out what deleting one removes.

/**
 * @typedef {{ key: string, kind: string, name: string, sub: string, size: number, status: string,
 *   location?: string, ctx?: { max: number|null, configured: number|null, loaded: number|null },
 *   loaded?: any, shares?: string[], deleteIds?: string[], deletePaths?: string[], freed?: number, llm?: string }} Tile
 */

/** Files used by more than one model (e.g. a shared text encoder) are kept when deleting just one model. */
function fileUseCounts(models) {
  const uses = new Map()
  for (const m of models) for (const f of Object.values(m.files || {})) if (f) uses.set(f.id, (uses.get(f.id) || 0) + 1)
  return uses
}

/**
 * @param {any} catalog  GET /api/models
 * @param {any} llm      GET /api/llm
 * @returns {Tile[]}
 */
export function buildTiles(catalog, llm) {
  const models = catalog.models.filter((/** @type {any} */ m) => !m.custom)
  const uses = fileUseCounts(models)
  /** @type {Tile[]} */
  const tiles = models.map((/** @type {any} */ m) => {
    const files = Object.values(m.files).filter(Boolean)
    const own = files.filter((/** @type {any} */ f) => uses.get(f.id) === 1 && !f.broken)
    return {
      key: m.id, kind: m.kind, name: m.label, sub: m.files.model?.name || '', status: m.status,
      size: files.reduce((s, /** @type {any} */ f) => s + (f.size || 0), 0),
      location: m.files.model?.location,
      deleteIds: own.map((/** @type {any} */ f) => f.id),
      deletePaths: own.flatMap((/** @type {any} */ f) => [...new Set([f.path, f.seenAt, ...(f.alsoAt || [])])]),
      freed: own.reduce((s, /** @type {any} */ f) => s + (f.size || 0), 0),
    }
  })
  for (const m of llm.models || []) {
    tiles.push({
      key: `llm:${m.name}`, kind: 'llm', name: m.name.replace(/:latest$/, ''), sub: [m.parameters, m.quantization].filter(Boolean).join(' · '),
      size: m.size, status: m.loaded ? 'loaded' : 'ready', ctx: m.context, loaded: m.loaded, shares: m.sharesWeightsWith, llm: m.name,
      freed: m.sharesWeightsWith?.length ? 0 : m.size,
    })
  }
  for (const f of catalog.unrecognized || []) {
    if (f.broken) continue
    tiles.push({
      key: `file:${f.id}`, kind: 'file', name: f.name, sub: f.repo || f.location, size: f.size, status: 'unused',
      location: f.location, deleteIds: [f.id], deletePaths: [...new Set([f.path, f.seenAt, ...(f.alsoAt || [])])], freed: f.size,
    })
  }
  return tiles
}

/** Storage bar segments: model files per location, plus Ollama's store. */
export function storageSegments(catalog, llm) {
  const segs = Object.entries(catalog.storage?.byLocation || {}).map(([label, bytes]) => ({ label, bytes: Number(bytes) }))
  if (llm.disk?.bytes) segs.push({ label: 'ollama', bytes: llm.disk.bytes })
  return segs.sort((a, b) => b.bytes - a.bytes)
}

/** "32k" style token counts. */
export const tokens = (/** @type {number|null|undefined} */ n) => (n == null ? '—' : n >= 1024 ? `${Math.round(n / 1024)}k` : String(n))
