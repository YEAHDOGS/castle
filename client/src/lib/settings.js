// Every setting Castle has, in one list. The Settings section draws itself from this, so
// adding a setting is adding one entry here.
//
// where: 'device' settings live in this browser or app and are different on every device.
//        'box' settings are saved on the Castle box and shared by every device.
// live:  true when changing it does something today. Settings without it are saved now
//        and wait for the feature that will read them; the screen says so on each one.

import { Cable, Server, Users, HardDrive, Gamepad2, Clapperboard, Boxes, BrainCircuit, Globe, Palette, RefreshCw } from '@lucide/svelte';

export const SETTING_GROUPS = [
  { id: 'connections', label: 'Connections', icon: Cable, blurb: 'How your devices reach this box when you are away from home.' },
  { id: 'box', label: 'This box', icon: Server, blurb: 'What this Castle is called and how it was started.' },
  { id: 'people', label: 'People and sign-in', icon: Users, blurb: 'Who can use the box and how they unlock it.' },
  { id: 'storage', label: 'Storage', icon: HardDrive, blurb: 'How files, trash and old versions are kept.' },
  { id: 'arcade', label: 'Arcade', icon: Gamepad2, blurb: 'How the game library looks and how games start.' },
  { id: 'theater', label: 'Theater', icon: Clapperboard, blurb: 'Where film, show and album details come from, and subtitles.' },
  { id: 'clones', label: 'Clones', icon: Boxes, blurb: 'Defaults for virtual machines and automations.' },
  { id: 'ai', label: 'AI', icon: BrainCircuit, blurb: 'Where models run and how long they stay loaded.' },
  { id: 'network', label: 'Network', icon: Globe, blurb: 'DNS, ad blocking and how the connection is measured.' },
  { id: 'look', label: 'Look and feel', icon: Palette, blurb: 'How Castle behaves on this device.' },
  { id: 'updates', label: 'Updates and backup', icon: RefreshCw, blurb: 'Keeping the box current and its data safe.' },
];

const choice = (value, label) => ({ value, label });

export const SETTINGS = [
  // ---- connections ----
  { id: 'remote.method', group: 'connections', where: 'box', type: 'choice', default: 'off', label: 'Remote access', help: 'Which private link your devices use to reach the box from outside the house.', options: [choice('off', 'Off'), choice('tailscale', 'Tailscale'), choice('wireguard', 'WireGuard')] },
  { id: 'remote.breakGlass', group: 'connections', where: 'box', type: 'toggle', default: false, label: 'Keep WireGuard as a backup', help: 'A second way in, used only if Tailscale is unreachable.' },
  { id: 'remote.lanDiscovery', group: 'connections', where: 'box', type: 'toggle', default: true, label: 'Announce on the home network', help: 'Lets apps on the same Wi-Fi find the box without typing its address.' },

  // ---- this box ----
  { id: 'box.name', group: 'box', where: 'box', type: 'text', default: 'Castle', label: 'Box name', help: 'Shown to every device that connects.' },
  { id: 'box.timezone', group: 'box', where: 'box', type: 'text', default: '', label: 'Time zone', help: 'For schedules and timestamps. Blank uses the box\'s own clock.', placeholder: 'e.g. America/New_York' },

  // ---- people ----
  { id: 'people.pinLength', group: 'people', where: 'box', type: 'choice', default: 4, label: 'PIN length', help: 'How many digits each person\'s PIN has.', options: [choice(4, '4'), choice(6, '6')] },
  { id: 'people.lockMinutes', group: 'people', where: 'box', type: 'number', default: 15, min: 0, max: 720, unit: 'min', label: 'Lock after', help: 'Minutes of no activity before a person must unlock again. 0 never locks.' },
  { id: 'people.guests', group: 'people', where: 'box', type: 'toggle', default: true, label: 'Allow guests', help: 'A guest sees only what has been shared with everyone.' },
  { id: 'people.childApproval', group: 'people', where: 'box', type: 'toggle', default: true, label: 'Adult approves a child\'s new device', help: 'A child\'s account cannot add a phone or laptop on its own.' },

  // ---- storage ----
  { id: 'storage.trashDays', group: 'storage', where: 'box', type: 'number', default: 30, min: 0, max: 365, unit: 'days', label: 'Empty trash after', help: 'Days a trashed file is kept before it is deleted for good. 0 keeps it until you empty it.' },
  { id: 'storage.versions', group: 'storage', where: 'box', type: 'number', default: 10, min: 0, max: 100, unit: 'versions', label: 'Keep old versions', help: 'How many earlier versions of a changed file to keep. 0 keeps none.' },
  { id: 'storage.quotaGb', group: 'storage', where: 'box', type: 'number', default: 0, min: 0, max: 100000, unit: 'GB', label: 'Space per person', help: 'The most each person may store. 0 means no limit.' },
  { id: 'storage.confirmDelete', group: 'storage', where: 'device', type: 'toggle', default: true, live: true, label: 'Ask before deleting for good', help: 'Shows a confirmation before a file is removed from the trash permanently.' },
  { id: 'storage.showHidden', group: 'storage', where: 'device', type: 'toggle', default: false, live: true, label: 'Show hidden files on the computer', help: 'Files the system hides (and names starting with a dot) appear, faded, when you browse This computer.' },
  { id: 'storage.fileView', group: 'storage', where: 'device', type: 'choice', default: 'grid', live: true, label: 'Show files as', help: 'The layout Storage opens in.', options: [choice('grid', 'Grid'), choice('list', 'List')] },

  // ---- arcade ----
  { id: 'arcade.cardSize', group: 'arcade', where: 'device', type: 'choice', default: 'medium', live: true, label: 'Box art size', help: 'How large each game appears in the library.', options: [choice('small', 'Small'), choice('medium', 'Medium'), choice('large', 'Large')] },
  { id: 'arcade.regionTags', group: 'arcade', where: 'device', type: 'toggle', default: true, live: true, label: 'Show region and language', help: 'The small line under a title, such as USA or En,Fr.' },
  { id: 'arcade.descriptions', group: 'arcade', where: 'device', type: 'toggle', default: true, live: true, label: 'Look up game descriptions', help: 'Opening a game\'s card asks Wikipedia what the game is. The box sends the title once and keeps the answer.' },
  { id: 'arcade.fullscreen', group: 'arcade', where: 'box', type: 'toggle', default: true, label: 'Start games full screen', help: 'Applies to every emulator the box starts.' },
  { id: 'arcade.savesPerPerson', group: 'arcade', where: 'box', type: 'toggle', default: true, label: 'Separate saves per person', help: 'Each person gets their own save files and save states.' },
  { id: 'arcade.saveHistory', group: 'arcade', where: 'box', type: 'toggle', default: true, label: 'Keep a history of saves', help: 'Every save is kept as a version you can go back to.' },

  // ---- theater ----
  { id: 'theater.lookups', group: 'theater', where: 'device', type: 'toggle', default: true, live: true, label: 'Look up details', help: 'Opening a film, show or album asks open sources (Wikipedia and Wikidata, TVmaze, MusicBrainz) for its summary, cast and poster. Only the title is sent, once.' },
  { id: 'theater.autoplay', group: 'theater', where: 'device', type: 'toggle', default: true, live: true, label: 'Play the next one', help: 'When an episode or a song ends, the next starts.' },
  { id: 'theater.opensubtitlesKey', group: 'theater', where: 'box', type: 'text', default: '', live: true, label: 'OpenSubtitles API key', help: 'Lets the box find subtitles for a video. The key is free at opensubtitles.com; it is kept on the box.', placeholder: 'paste the key' },

  // ---- clones ----
  { id: 'clones.pollSeconds', group: 'clones', where: 'device', type: 'choice', default: 2, live: true, label: 'Refresh status every', help: 'How often this screen asks the box what an automation is doing.', options: [choice(1, '1 s'), choice(2, '2 s'), choice(5, '5 s')] },
  { id: 'clones.memoryGb', group: 'clones', where: 'box', type: 'number', default: 4, min: 1, max: 256, unit: 'GB', label: 'Memory for a new machine', help: 'The default when a machine\'s own profile does not say.' },
  { id: 'clones.cores', group: 'clones', where: 'box', type: 'number', default: 4, min: 1, max: 128, unit: 'cores', label: 'Processor cores for a new machine', help: 'The default when a machine\'s own profile does not say.' },
  { id: 'clones.autostart', group: 'clones', where: 'box', type: 'toggle', default: false, label: 'Restart automations when the box starts', help: 'Brings back whatever was running before a restart.' },

  // ---- ai ----
  { id: 'ai.serverUrl', group: 'ai', where: 'box', type: 'text', default: 'http://127.0.0.1:1234', label: 'Model server address', help: 'The local server that runs the models.' },
  { id: 'ai.keepLoadedMinutes', group: 'ai', where: 'box', type: 'number', default: 30, min: 0, max: 1440, unit: 'min', label: 'Unload an idle model after', help: 'Frees memory when nobody is chatting. 0 keeps it loaded.' },
  { id: 'ai.chatHistory', group: 'ai', where: 'box', type: 'toggle', default: true, label: 'Keep chat history', help: 'Chats are stored on the box, per person.' },

  // ---- network ----
  { id: 'network.adBlock', group: 'network', where: 'box', type: 'toggle', default: false, label: 'Block ads and trackers', help: 'Answers known ad and tracking domains with nothing, for every device using the box for DNS.' },
  { id: 'network.upstreamDns', group: 'network', where: 'box', type: 'choice', default: 'provider', label: 'Look up names with', help: 'Where the box sends DNS questions it cannot answer itself.', options: [choice('provider', 'Your provider'), choice('cloudflare', 'Cloudflare'), choice('quad9', 'Quad9')] },
  { id: 'network.speedTestDaily', group: 'network', where: 'box', type: 'toggle', default: false, label: 'Run a speed test every day', help: 'Builds a record of what the connection actually delivers.' },
  { id: 'network.livePollSeconds', group: 'network', where: 'device', type: 'choice', default: 2, live: true, label: 'Update live traffic every', help: 'How often the Network screen re-reads the box\'s counters.', options: [choice(1, '1 s'), choice(2, '2 s'), choice(5, '5 s')] },

  // ---- look and feel ----
  { id: 'look.motion', group: 'look', where: 'device', type: 'choice', default: 'full', live: true, label: 'Motion', help: 'Reduced turns off animations and transitions.', options: [choice('full', 'Full'), choice('reduced', 'Reduced')] },
  { id: 'look.startSection', group: 'look', where: 'device', type: 'choice', default: 'last', live: true, label: 'Open Castle in', help: 'The section shown when the app starts.', options: [choice('last', 'Where I left off'), choice('storage', 'Storage'), choice('ai', 'AI'), choice('arcade', 'Arcade'), choice('theater', 'Theater'), choice('comms', 'Comms'), choice('contacts', 'Contacts'), choice('clones', 'Clones'), choice('network', 'Network')] },
  { id: 'look.wheelInvert', group: 'look', where: 'device', type: 'toggle', default: false, live: true, label: 'Reverse the wheel', help: 'Turns the section wheel the other way when you scroll.' },

  // ---- updates and backup ----
  { id: 'updates.auto', group: 'updates', where: 'box', type: 'toggle', default: false, label: 'Install updates automatically', help: 'The box updates itself overnight.' },
  { id: 'updates.channel', group: 'updates', where: 'box', type: 'choice', default: 'stable', label: 'Update channel', help: 'Preview gets changes sooner and may be rougher.', options: [choice('stable', 'Stable'), choice('preview', 'Preview')] },
  { id: 'backup.schedule', group: 'updates', where: 'box', type: 'choice', default: 'off', label: 'Back up the vault', help: 'How often a sealed copy of everything is made.', options: [choice('off', 'Off'), choice('daily', 'Daily'), choice('weekly', 'Weekly')] },
  { id: 'backup.keep', group: 'updates', where: 'box', type: 'number', default: 7, min: 1, max: 365, unit: 'copies', label: 'Backups to keep', help: 'Older copies are removed once there are more than this.' },
];

export const settingById = (id) => SETTINGS.find((s) => s.id === id);

/** The default value of every setting kept in `where`, keyed by id. */
export function defaultsFor(where) {
  return Object.fromEntries(SETTINGS.filter((s) => s.where === where).map((s) => [s.id, s.default]));
}
