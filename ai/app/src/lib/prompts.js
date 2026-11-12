// Prompt categories (ids match server/src/prompts.mjs) and how specific a prompt is, judged by its length.
import { Sun, Users, Landmark, ShieldAlert, Mountain, Palette, Film, AudioLines, Tag } from '@lucide/svelte'

export const CATEGORIES = [
  { id: 'lighting', label: 'Lighting & texture', icon: Sun, color: 'text-amber-300', tint: 'bg-amber-400/15 border-amber-400/30' },
  { id: 'people', label: 'People', icon: Users, color: 'text-pink-300', tint: 'bg-pink-400/15 border-pink-400/30' },
  { id: 'history', label: 'Political & historical', icon: Landmark, color: 'text-sky-300', tint: 'bg-sky-400/15 border-sky-400/30' },
  { id: 'boundary', label: 'Boundary probing', icon: ShieldAlert, color: 'text-red-400', tint: 'bg-red-500/15 border-red-500/30' },
  { id: 'scene', label: 'Scenes & places', icon: Mountain, color: 'text-emerald-300', tint: 'bg-emerald-400/15 border-emerald-400/30' },
  { id: 'style', label: 'Style & art', icon: Palette, color: 'text-violet-300', tint: 'bg-violet-400/15 border-violet-400/30' },
  { id: 'motion', label: 'Video & motion', icon: Film, color: 'text-cyan-300', tint: 'bg-cyan-400/15 border-cyan-400/30' },
  { id: 'sound', label: 'Music & sound', icon: AudioLines, color: 'text-lime-300', tint: 'bg-lime-400/15 border-lime-400/30' },
  { id: 'other', label: 'Uncategorized', icon: Tag, color: 'text-neutral-400', tint: 'bg-white/5 border-white/10' },
]
const OTHER = CATEGORIES[CATEGORIES.length - 1]
export const categoryOf = (/** @type {string} */ id) => CATEGORIES.find((c) => c.id === id) || OTHER

const LOOSE_MAX_WORDS = 15
const SPECIFIC_MAX_WORDS = 60
export const SPECIFICITY = [
  { id: 'loose', label: 'Loose', about: `under ${LOOSE_MAX_WORDS} words` },
  { id: 'specific', label: 'Specific', about: `${LOOSE_MAX_WORDS}–${SPECIFIC_MAX_WORDS - 1} words` },
  { id: 'dense', label: 'Dense', about: `${SPECIFIC_MAX_WORDS}+ words` },
]

export function specificityOf(/** @type {string} */ text) {
  const words = text.trim().split(/\s+/).length
  return SPECIFICITY[words < LOOSE_MAX_WORDS ? 0 : words < SPECIFIC_MAX_WORDS ? 1 : 2]
}

const MINUTE = 60_000, HOUR = 60 * MINUTE, DAY = 24 * HOUR
/** "just now", "5m ago", "3h ago", "4d ago", then the date. */
export function ago(/** @type {number} */ t) {
  const d = Date.now() - t
  if (d < MINUTE) return 'just now'
  if (d < HOUR) return `${Math.floor(d / MINUTE)}m ago`
  if (d < DAY) return `${Math.floor(d / HOUR)}h ago`
  if (d < 30 * DAY) return `${Math.floor(d / DAY)}d ago`
  return new Date(t).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })
}
