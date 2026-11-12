// Shared looks and labels for the Clones screens.

export const STATE_LABEL = { stopped: 'STOPPED', launching: 'STARTING', running: 'RUNNING', failed: 'FAILED' };
export const STATE_TONE = { stopped: 'text-text-faint', launching: 'text-chiral', running: 'text-live', failed: 'text-chiral' };

export const INPUT =
  'h-10 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none';
export const AREA =
  'w-full border border-hairline bg-white/[0.03] px-3 py-2 font-mono text-[12px] leading-relaxed text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none';
export const BTN =
  'expo t-hover micro flex h-10 items-center justify-center gap-2 border border-hairline px-3 font-bold text-white transition-colors hover:border-chiral/60 disabled:opacity-40';
export const BTN_PRIMARY =
  'expo t-hover hud-corners micro flex h-11 items-center justify-center gap-2 bg-chiral px-5 font-bold text-black transition-all hover:brightness-110 active:scale-[0.98] disabled:opacity-40';
export const BTN_STOP =
  'expo t-hover micro flex h-10 items-center justify-center gap-2 border border-chiral/70 px-3 font-bold text-chiral transition-colors hover:bg-chiral-dim disabled:opacity-40';
export const LABEL = 'micro mb-1 block text-text-faint';

export const FAMILIES = [
  { id: '', label: 'All' },
  { id: 'linux', label: 'Linux' },
  { id: 'windows', label: 'Windows' },
  { id: 'android', label: 'Android' },
  { id: 'macos', label: 'macOS' },
];

/** Non-empty trimmed lines of a text box. */
export function lines(text) {
  return String(text || '').split('\n').map((line) => line.trim()).filter(Boolean);
}

/** Words of a text box: package names separated by spaces, commas or new lines. */
export function words(text) {
  return String(text || '').split(/[\s,]+/).filter(Boolean);
}

/** "12.3 s" or "3 m 05 s". */
export function fmtSeconds(seconds) {
  if (seconds == null) return '-';
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  return `${Math.floor(seconds / 60)} m ${String(Math.round(seconds % 60)).padStart(2, '0')} s`;
}
