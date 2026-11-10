// Tiny formatters. Minimal words, mono-friendly output.

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

export function fmtSize(bytes) {
  if (bytes == null || Number.isNaN(bytes)) return '—';
  if (bytes === 0) return '0 B';
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNITS.length - 1);
  const v = bytes / Math.pow(1024, i);
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${UNITS[i]}`;
}

export function fmtDate(ts) {
  const d = new Date(ts);
  const now = Date.now();
  const day = 864e5;
  if (now - ts < day) return 'Today';
  if (now - ts < 2 * day) return 'Yesterday';
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: d.getFullYear() === new Date().getFullYear() ? undefined : 'numeric' });
}

/** Short uid for demo nodes. */
export function uid() {
  return (
    (typeof crypto !== 'undefined' && crypto.randomUUID?.()) ||
    `id-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
  );
}
