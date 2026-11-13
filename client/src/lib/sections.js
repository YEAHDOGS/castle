// The sections of Castle, in wheel order. Each one is its own interface.
// `key` is the letter that opens it while the dial is open (underlined on the dial). Each
// is a letter of the section's name, and no two share one. The digits 1 to 9 also work, in
// wheel order.

import { House, HardDrive, BrainCircuit, Gamepad2, Clapperboard, MessagesSquare, BookUser, Boxes, Globe, ShieldCheck, Settings } from '@lucide/svelte';

export const SECTIONS = [
  { id: 'home', key: 'h', label: 'Home', tagline: 'The box at a glance', icon: House },
  { id: 'storage', key: 's', label: 'Storage', tagline: 'Files, photos and backups', icon: HardDrive },
  { id: 'ai', key: 'i', label: 'AI', tagline: 'DOGS AI: images, video, audio and chat', icon: BrainCircuit },
  { id: 'arcade', key: 'a', label: 'Arcade', tagline: 'Your game library', icon: Gamepad2 },
  { id: 'theater', key: 't', label: 'Theater', tagline: 'Films, shows, music and photos', icon: Clapperboard },
  { id: 'comms', key: 'm', label: 'Comms', tagline: 'Texts, calls, email and chats, kept', icon: MessagesSquare },
  { id: 'contacts', key: 'o', label: 'Contacts', tagline: 'Everyone you know, backed up', icon: BookUser },
  { id: 'clones', key: 'c', label: 'Clones', tagline: 'Virtual machines and automations', icon: Boxes },
  { id: 'network', key: 'n', label: 'Network', tagline: 'Live traffic, speed and your provider', icon: Globe },
  { id: 'security', key: 'r', label: 'Security', tagline: 'Checks, the activity record and audits', icon: ShieldCheck },
  { id: 'settings', key: 'g', label: 'Settings', tagline: 'Connections and everything configurable', icon: Settings },
];

export const DEFAULT_SECTION = 'home';

export const sectionById = (id) => SECTIONS.find((s) => s.id === id) || SECTIONS[0];

/** Extra keys for a section, beyond its letter. Comma is Settings, as in many apps. */
const ALIASES = { ',': 'settings' };

/** The section a key opens on the dial, or null. */
export function sectionForKey(key) {
  if (!key || key.length !== 1) return null;
  const k = key.toLowerCase();
  if (ALIASES[k]) return sectionById(ALIASES[k]);
  if (k >= '1' && k <= '9') return SECTIONS[Number(k) - 1] || null;
  return SECTIONS.find((s) => s.key === k) || null;
}
