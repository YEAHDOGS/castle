[![Download Android APK](https://img.shields.io/badge/Download-Android_APK-3DDC84?style=for-the-badge&logo=android)](https://github.com/YEAHDOGS/castle/releases/latest/download/castle-android-latest.apk)

# Castle — Personal Cloud

Your drive. Your box. No login wall — it opens straight into your files.

Svelte 5 + Vite + Tailwind frontend in a Tauri v2 shell. One codebase ships to **web** and to **Windows and Linux desktop**; the desktop app runs a box of its own. Android and iOS builds exist, but phones are getting a separate app. Made by DOGS — https://wearedogs.net

## Run it (web)

```bash
npm install
npm run dev        # http://localhost:1420
```

The web build is a fully working demo: same UI, same shortcuts, files persist to `localStorage`.

## Desktop app (Windows, Linux)

The desktop app is the interface plus a box: it carries the `castled` daemon inside it,
starts it when the window opens, connects to it without asking for a token, and stops it
when the window closes. A box already listening on the port is used as it is and left running.

```bash
npm install
npm run desktop    # builds castled from ../castle, then the installer
```

The installer lands in `src-tauri/target/release/bundle/nsis/`. It needs a Rust toolchain.
For a dev window with hot reload: `npm run daemon` once, then `npm run tauri dev`.

Files live in `Castle` in the home folder unless `box.json` in the app's config folder
(`%APPDATA%\net.wearedogs.castle` on Windows, `~/.config/net.wearedogs.castle` on Linux)
says otherwise. Every field is optional:

```json
{
  "data": "D:/Castle",
  "arcade": "E:/Games",
  "clonewars": "C:/path/to/castle/clonewars",
  "port": 8787
}
```

`arcade` and `clonewars` turn those two sections on. A folder that is missing when the app
starts (an unplugged drive) is skipped, and that section stays off until the next start.
The box writes its output to `box.log` in the same folder.

## Keyboard-first

Everything is mouse-optional:

| Keys | Does |
|---|---|
| `Ctrl/⌘ + Shift + N` (or `Ctrl/⌘ + N`) | New folder, name already being edited |
| `F2` | Rename focused file/folder inline (`Enter` commits, `Esc` cancels) |
| `Arrows` (`+ Shift` extends) | Move through the grid spatially / rows in list view |
| `Enter` | Open folder / preview file |
| `Backspace` | Up one folder |
| `Delete` | Move selection to Trash |
| `Ctrl/⌘ + A` | Select all |
| `A–Z` | Type-ahead: jumps to the first file matching what you type |
| `Esc` | Close menu → preview → cancel rename → clear selection |

Double-click opens. Right-click gets the context menu. Drag files in from anywhere to upload.

## The look

Death Stranding HUD fused with wearedogs.net motion: near-black void, chiral orange (`#ff6a00`) as the voice, bridge cyan (`#00d7ff`) for data and the focus ring, corner-bracket wireframe panels, mono microcopy, scanline restraint.

Motion law: **expo-out everywhere** (`cubic-bezier(0.16,1,0.3,1)`), 150ms hovers / 250ms UI / 320ms moments, transform + opacity only, 60fps on a mid-range phone. `prefers-reduced-motion` stills everything. See `src/lib/motion.js`.

## Builds (GitHub Actions)

| Workflow | Trigger | Output |
|---|---|---|
| `web-demo.yml` | push to `main` | `dist/` artifact; deploys to Cloudflare Pages **only if** `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ACCOUNT_ID` secrets exist |
| `android.yml` | push to `main` / manual | debug APK artifact — sideload it |
| `ios.yml` | manual only | **unsigned** IPA artifact |
| `build-apps.yml` | manual only, with a checkbox per platform | Windows installer, Linux AppImage and .deb, and optionally the APK and IPA, all on one GitHub release tagged `v<version>` |

Mobile targets (`src-tauri/gen`) are generated inside CI and gitignored — no local Android SDK / Xcode needed.

### Sideload the APK on Android

1. Actions tab → latest **Android APK** run → download `castle-android-apk`.
2. Copy it to your phone (USB, Drive, whatever).
3. Open it on the phone, allow **Install unknown apps** when asked, tap Install.

### iOS signing note

The CI IPA is **unsigned** — install it with [AltStore](https://altstore.io), SideStore, or Sideloadly using your own Apple ID. A signed, TestFlight-grade build needs an Apple Developer account + certificate/provisioning secrets wired into `ios.yml` — that's a later step, flagged on the task board.

### Cloudflare Pages (web demo hosting)

`web-demo.yml` deploys automatically once two repo secrets exist (**Settings → Secrets → Actions**):

- `CLOUDFLARE_API_TOKEN` — API token with Pages:Edit (dash.cloudflare.com → My Profile → API Tokens)
- `CLOUDFLARE_ACCOUNT_ID` — shown on the Pages dashboard

Without them the workflow still uploads the `dist/` artifact every push.

## Project layout

```
src/
  App.svelte            shell: sidebar + topbar + browser + overlays
  lib/
    adapter.js          StorageAdapter contract + LocalStorageAdapter (demo backend)
    store.js            DriveStore — all state + actions (Svelte 5 runes)
    keys.js             keyboard shortcuts + type-ahead
    motion.js           the motion language (expo-out, durations, reduced-motion)
    format.js           size/date formatting
  components/           Sidebar, TopBar, Breadcrumbs, FileBrowser, NewMenu,
                        ContextMenu, PreviewModal, Toasts
src-tauri/              Tauri v2 shell (identifier: net.wearedogs.castle)
.github/workflows/      web-demo, android, ios
```

## Roadmap

- **v1 (this):** shortcuts, inline rename, aesthetic shell, motion language, demo backend.
- **Next:** native filesystem adapter via Tauri (`@tauri-apps/plugin-fs`) talking to the real Castle box; real upload/download bytes; share links; themes (yes, including a joke one).

## License

Private. All rights reserved — DOGS.
