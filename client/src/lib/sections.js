// The sections of Castle, in wheel order. Each one is its own interface.

import { House, HardDrive, BrainCircuit, Gamepad2, Boxes, Globe, ShieldCheck, Settings } from '@lucide/svelte';

export const SECTIONS = [
  { id: 'home', label: 'Home', tagline: 'The box at a glance', icon: House },
  { id: 'storage', label: 'Storage', tagline: 'Files, photos and backups', icon: HardDrive },
  { id: 'ai', label: 'AI', tagline: 'Local models and chat', icon: BrainCircuit },
  { id: 'arcade', label: 'Arcade', tagline: 'Your game library', icon: Gamepad2 },
  { id: 'clones', label: 'Clones', tagline: 'Virtual machines and automations', icon: Boxes },
  { id: 'network', label: 'Network', tagline: 'Live traffic, speed and your provider', icon: Globe },
  { id: 'security', label: 'Security', tagline: 'Checks, the activity record and audits', icon: ShieldCheck },
  { id: 'settings', label: 'Settings', tagline: 'Connections and everything configurable', icon: Settings },
];

export const DEFAULT_SECTION = 'home';

export const sectionById = (id) => SECTIONS.find((s) => s.id === id) || SECTIONS[0];
