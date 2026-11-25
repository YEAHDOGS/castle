// The demo's Phoenix: a pretend PC with things to clean, a pretend phone, and an app list.
// Nothing is deleted and nothing is saved past the page. The shapes are the ones the box
// hands on from Phoenix's engine (castled/src/phoenix.rs).

const MB = 1024 ** 2;
const GB = 1024 ** 3;

const rule = (id, group, name, why, bytes, files, extra = {}) => ({
  id, group, name, why, bytes, files,
  risk: 'safe', default: true, admin: false, blocked: null, running: [], notes: [], folders: [], biggest: [],
  ...extra,
});

function pcItems() {
  return [
    rule('win.temp.user', 'Windows', 'Your temporary files', 'Programs leave working files in your Temp folder and rarely tidy up. Files touched in the last day are kept, since a running program may still want them.', 19.3 * GB, 343347, {
      folders: ['C:\\Users\\alex\\AppData\\Local\\Temp'],
      biggest: [
        { path: 'C:\\Users\\alex\\AppData\\Local\\Temp\\DockerDesktopUpdates\\Docker Desktop Installer.exe', bytes: 595 * MB },
        { path: 'C:\\Users\\alex\\AppData\\Local\\Temp\\old-payload\\project.zip', bytes: 412 * MB },
        { path: 'C:\\Users\\alex\\AppData\\Local\\Temp\\vs-setup\\vs_installer.opc', bytes: 188 * MB },
      ],
    }),
    rule('win.temp.system', 'Windows', 'Windows temporary files', 'The same thing for Windows itself and for installers. Files touched in the last day are kept.', 0, 0, { admin: true, blocked: 'needs-admin' }),
    rule('win.update.downloads', 'Windows', 'Windows Update downloads', 'Update packages that are already installed. Windows downloads them again if it ever needs one.', 2.1 * GB, 412, { admin: true, blocked: 'needs-admin' }),
    rule('win.thumbnails', 'Windows', 'Thumbnail and icon caches', 'Small previews File Explorer draws once and keeps. It draws them again as you browse.', 44 * MB, 30),
    rule('win.errors.user', 'Windows', 'Your error reports and crash dumps', 'Reports queued for Microsoft and memory dumps of programs that crashed.', 834 * MB, 10),
    rule('win.shaders', 'Windows', 'Graphics shader caches', 'Compiled shaders from games and apps. They are rebuilt the next time each one starts, which can make its first launch slower.', 10 * GB, 402),
    rule('win.recyclebin', 'Windows', 'Recycle Bin', 'Everything you have deleted and could still put back. Emptying it is final.', 18.4 * GB, 13260, { risk: 'caution', default: false }),
    rule('browser.chrome', 'Browsers', 'Chrome cache', 'Copies of pages, pictures and scripts. Your history, cookies, passwords and open tabs are not touched. Close Chrome first, or the files it has open are skipped.', 1.5 * GB, 12711, { running: ['chrome'] }),
    rule('browser.firefox', 'Browsers', 'Firefox cache', 'Copies of pages, pictures and scripts. Your history, cookies, passwords and open tabs are not touched. Close Firefox first, or the files it has open are skipped.', 367 * MB, 3021),
    rule('app.discord', 'Apps', 'Discord cache', "Pictures and videos Discord has shown you. Your account and messages live on Discord's servers.", 374 * MB, 1533),
    rule('app.spotify', 'Apps', 'Spotify cache', 'Songs Spotify has streamed, kept so they start faster. Songs you downloaded for offline use are kept in the same place and go too.', 738 * MB, 3346, { risk: 'caution', default: false }),
    rule('dev.npm', 'Developer', 'npm download cache', 'Every package npm has downloaded. The next install fetches what it needs again.', 9.4 * GB, 20781, { risk: 'caution', default: false }),
    rule('dev.cargo', 'Developer', 'Cargo download cache', 'Rust crates as downloaded and unpacked. The next build fetches what it needs again, which needs the internet.', 900 * MB, 38537, { risk: 'caution', default: false }),
  ];
}

function phoneItems() {
  return [
    rule('android.app-caches', 'Apps', 'App caches, every app', 'What apps keep to load faster: pictures, video segments, web pages. Android clears all of them at once; without root it cannot do one app at a time. Sign-ins and app data stay.', 6.2 * GB, 148, {
      biggest: [
        { path: 'com.google.android.youtube', bytes: 1.4 * GB },
        { path: 'com.instagram.android', bytes: 980 * MB },
        { path: 'com.spotify.music', bytes: 760 * MB },
        { path: 'com.android.chrome', bytes: 512 * MB },
      ],
    }),
    rule('android.thumbnails', 'Storage', 'Gallery thumbnails', 'Small previews of your photos and videos. The gallery draws them again. The photos themselves are not touched.', 412 * MB, 9120, { folders: ['/sdcard/DCIM/.thumbnails'] }),
    rule('android.trashed', 'Storage', 'Photos and files already in the trash', 'Things you deleted in the gallery or the Files app, which Android keeps for 30 days. Removing them here is final.', 1.1 * GB, 86, { risk: 'caution', default: false }),
    rule('android.apk-downloads', 'Storage', 'App installers in Downloads', 'The .apk files left in Downloads after you installed an app from them. The installed apps stay.', 340 * MB, 4, { risk: 'caution', default: false }),
  ];
}

const CATALOG = [
  { id: '7zip', name: '7zip', category: 'Utilities', description: 'zip, unzip, wow!', default: true },
  { id: 'powertoys', name: 'powertoys', category: 'Utilities', description: 'The Windows power tools Microsoft forgot to include', default: true },
  { id: 'powershell-core', name: 'PowerShell', category: 'Utilities', description: 'The current PowerShell, beside the one Windows ships', default: true },
  { id: 'vscode', name: 'VS Code', category: 'Development', description: 'The editor', default: true },
  { id: 'git', name: 'Git', category: 'Development', description: 'Version control', default: true },
  { id: 'firefox', name: 'Firefox', category: 'Browsers', description: 'A browser', default: true },
  { id: 'vlc', name: 'VLC', category: 'Media', description: 'Plays everything', default: true },
  { id: 'steam', name: 'Steam', category: 'Games', description: 'Games', default: false },
];

const FEED = [
  { id: 'powertoys', title: 'Microsoft PowerToys', version: '0.96.1', downloads: 2143772, summary: 'A set of utilities for power users to tune and streamline their Windows experience.', source: 'online' },
  { id: 'powershell-core', title: 'PowerShell', version: '7.5.4', downloads: 12903381, summary: 'PowerShell is a cross-platform automation and configuration tool.', source: 'online' },
  { id: 'obs-studio', title: 'OBS Studio', version: '32.0.2', downloads: 3841001, summary: 'Free and open source software for video recording and live streaming.', source: 'online' },
  { id: 'everything', title: 'Everything', version: '1.4.1.1028', downloads: 2618844, summary: 'Locate files and folders by name instantly.', source: 'online' },
  { id: 'notepadplusplus', title: 'Notepad++', version: '8.8.7', downloads: 31200450, summary: 'A free source code editor and Notepad replacement.', source: 'online' },
];

export class DemoPhoenix {
  apps = ['7zip', 'powertoys', 'powershell-core', 'vscode', 'git', 'firefox', 'vlc'];
  harden = { enabled: false, chosen: {} };
  names = { computerName: 'THE-MACHINE', adminName: 'BigMan', userName: 'Main' };

  async phoenixStatus() {
    return { available: true, engine: '2.0.0', isAdmin: false, os: 'Microsoft Windows 11 Pro', android: true, canChange: true };
  }

  async phoenixDevices() {
    return { devices: [{ serial: 'R5CT30XXXXX', state: 'device', model: 'Old phone' }] };
  }

  async phoenixScan(platform = 'windows') {
    const items = platform === 'android' ? phoneItems() : pcItems();
    return { platform, serial: platform === 'android' ? 'R5CT30XXXXX' : null, isAdmin: false, freeBytes: platform === 'android' ? 9.8 * GB : 70.7 * GB, totalBytes: items.reduce((sum, i) => sum + i.bytes, 0), items };
  }

  async phoenixClean(platform, serial, ids) {
    const items = (platform === 'android' ? phoneItems() : pcItems()).filter((i) => ids.includes(i.id) && !i.blocked);
    const freed = items.reduce((sum, i) => sum + i.bytes, 0);
    return { platform, freedBytes: freed, files: items.reduce((sum, i) => sum + i.files, 0), failed: 0, rules: items.map((i) => ({ id: i.id, freedBytes: i.bytes, files: i.files, failed: 0 })) };
  }

  async phoenixSetup() {
    return {
      apps: [...this.apps],
      catalog: CATALOG,
      ...this.names,
      saved: true,
      harden: {
        ...this.harden,
        name: 'SimeonOnSecurity: Windows Optimize, Harden, Debloat',
        description: 'A third-party script that applies the DoD STIG baselines and its author\'s own hardening to Windows 10 and 11. Phoenix runs it as it is, at one pinned commit, with only the parts you tick.',
        homepage: 'https://github.com/simeononsecurity/Windows-Optimize-Harden-Debloat',
        commit: '5370d7f3d2d7b3499401845b545c171e674dab0e',
        options: [
          { name: 'disabletelemetry', group: 'Privacy', label: 'Disable telemetry', why: 'Turns off data collection and telemetry.', default: true, risk: 'safe' },
          { name: 'privacy', group: 'Privacy', label: 'Privacy settings', why: "Its author's privacy changes.", default: true, risk: 'safe' },
          { name: 'defenderhardening', group: 'Hardening', label: 'Defender hardening', why: 'Hardens Windows Defender beyond the STIG.', default: true, risk: 'safe' },
          { name: 'smbhardening', group: 'Hardening', label: 'SMB hardening', why: 'Hardens file-sharing client and server settings. Old NAS boxes and printers may stop connecting.', default: true, risk: 'caution' },
          { name: 'windows', group: 'DoD STIG baselines', label: 'Windows 10/11', why: 'The Windows desktop STIGs. The strictest part: it can lock out sign-in options, remote desktop and features you use.', default: false, risk: 'danger' },
        ],
      },
    };
  }

  async phoenixSaveSetup(body) {
    if (body.apps) this.apps = [...body.apps];
    if (body.harden) this.harden = { enabled: !!body.harden.enabled, chosen: { ...body.harden.chosen } };
    for (const key of Object.keys(this.names)) if (typeof body[key] === 'string') this.names[key] = body[key];
    return this.phoenixSetup();
  }

  async phoenixChoco(q) {
    const words = q.toLowerCase().trim();
    return FEED.filter((p) => !words || p.id.includes(words) || p.title.toLowerCase().includes(words) || p.summary.toLowerCase().includes(words));
  }

  async phoenixScript() {
    const list = this.apps.map((a) => `'${a}'`).join(', ');
    return { script: `# phoenix-firstboot v1\r\n# A sample: the real script installs Chocolatey, then each app below.\r\n$apps = @(${list})\r\n`, apps: [...this.apps], harden: [] };
  }

  async phoenixAnswerFile() {
    return { xml: '<?xml version="1.0" encoding="utf-8"?>\r\n<!-- A sample: the demo makes no answer file. -->\r\n<unattend xmlns="urn:schemas-microsoft-com:unattend" />\r\n', accounts: [this.names.adminName, this.names.userName].filter(Boolean), apps: [...this.apps], warnings: [] };
  }
}
