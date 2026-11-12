// This computer: the drives and folders of the machine the box runs on, beyond the vault.
//
// Storage shows them with the same browser as My Drive. An item there has an id of
// "host:" followed by its whole path ("host:C:\Users"), so every part of the app can tell
// it from a vault file, and the adapters send it to the box's /host calls.

export const HOST_PREFIX = 'host:';
/** The top of This computer: the list of drives. */
export const HOST_ROOT = HOST_PREFIX;
export const COMPUTER_NAME = 'This computer';

export const isHostId = (id) => typeof id === 'string' && id.startsWith(HOST_PREFIX);
export const hostPath = (id) => (isHostId(id) ? id.slice(HOST_PREFIX.length) : '');
export const hostId = (path) => HOST_PREFIX + path;

const isWindowsPath = (path) => /^[A-Za-z]:/.test(path);

/** "C:\Users\alex" -> ["C:\", "C:\Users", "C:\Users\alex"]; "/home/alex" -> ["/", "/home", "/home/alex"]. */
function steps(path) {
  if (!path) return [];
  if (isWindowsPath(path)) {
    const parts = path.split(/[\\/]+/).filter(Boolean);
    const drive = `${parts[0].toUpperCase()}\\`;
    const out = [drive];
    for (const part of parts.slice(1)) out.push(out.length === 1 ? drive + part : `${out[out.length - 1]}\\${part}`);
    return out;
  }
  const out = ['/'];
  for (const part of path.split('/').filter(Boolean)) out.push(`${out.length === 1 ? '' : out[out.length - 1]}/${part}`);
  return out;
}

const stepName = (step) => {
  if (/^[A-Za-z]:\\$/.test(step)) return step.slice(0, 2);
  if (step === '/') return '/';
  return step.split(/[\\/]/).pop();
};

/** The breadcrumbs from This computer down to `path`. */
export function hostTrail(path) {
  return [{ id: HOST_ROOT, name: COMPUTER_NAME }, ...steps(path).map((step) => ({ id: hostId(step), name: stepName(step) }))];
}

/** The folder `path` is in, or '' when it is a drive (whose parent is This computer). */
export function hostParent(path) {
  const all = steps(path);
  return all.length > 1 ? all[all.length - 2] : '';
}

/** What a drive is, for its icon and its place on the page. */
export const isRemovableDrive = (drive) => !!drive?.removable;

/** A time as a date and a clock: "Today 14:32:05", "Yesterday 09:10:44", "3 Nov 2026, 18:02". */
export function fmtStamp(ms) {
  if (!ms) return '—';
  const at = new Date(ms);
  const clock = at.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const today = new Date();
  const startOfToday = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  if (ms >= startOfToday) return `Today ${clock}`;
  if (ms >= startOfToday - 864e5) return `Yesterday ${clock}`;
  const date = at.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
  return `${date}, ${clock}`;
}

/** "for 3 h 12 min", "for 4 days": how long ago `ms` was, roughly. */
export function fmtSince(ms, now = Date.now()) {
  const mins = Math.max(0, Math.floor((now - ms) / 6e4));
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins} min`;
  const hours = Math.floor(mins / 60);
  if (hours < 48) return `${hours} h ${mins % 60} min`;
  return `${Math.floor(hours / 24)} days`;
}
