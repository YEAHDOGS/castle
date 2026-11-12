const KB = 1024
const MB = 1024 ** 2
const GB = 1024 ** 3

/** Human size: "5.4 GB", "320 MB", "98 KB", "512 B"; "?" when unknown. */
export function size(/** @type {number|null|undefined} */ b) {
  if (b == null) return '?'
  if (b >= GB) return `${(b / GB).toFixed(1)} GB`
  if (b >= MB) return `${(b / MB).toFixed(b >= 100 * MB ? 0 : 1)} MB`
  if (b >= KB) return `${Math.round(b / KB)} KB`
  return `${b} B`
}

/** Compact count: 1.2M, 34k. */
export const count = (/** @type {number} */ n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : String(n))
