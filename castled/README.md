# castled -- the Castle box daemon

The program that runs on the Castle box. It serves one directory (the vault) over a small
HTTP API, and every Castle client talks to that API: web, Windows, Linux, Android, iOS.
No client needs Samba. Samba can still share the same directory for anyone who wants a
mapped network drive, but the apps do not depend on it.

## Run it

```powershell
cd castled
cargo run -- --data .\castle-data --web ..\client\dist
```

It prints a link ending in `#connect=<token>`. Open it and the app is connected.
`--web` is optional: without it the daemon prints the token, and you paste that into the
app's Connect screen (the server icon in the top bar).

| Flag | Default | Environment |
|---|---|---|
| `--data` vault directory | `./castle-data` | `CASTLE_DATA_DIR` |
| `--bind` address and port | `127.0.0.1:8787` | `CASTLE_BIND` |
| `--web` built Castle app to serve at `/` | none | `CASTLE_WEB_DIR` |
| `--clonewars` Castle's `clonewars` folder, to run automations | none | `CASTLE_CLONEWARS_DIR` |
| `--arcade` a games folder (`Consoles/` + `Roms/`) to show in Arcade | none | `CASTLE_ARCADE_DIR` |

## What it guarantees

- **This machine only by default.** It listens on `127.0.0.1`. Binding to another address
  makes it reachable from the network over plain HTTP, and it says so at startup.
- **Token on every call.** Everything except `GET /api/v1/hello` needs
  `Authorization: Bearer <token>`. The token is created on first start and kept in
  `<vault>/.castle/token`.
- **Answers only to this computer's names.** While it listens on `127.0.0.1`, a request
  whose `Host` is anything but `127.0.0.1`, `localhost` or `[::1]` is refused, so a web page
  elsewhere cannot aim a name of its own at the box (DNS rebinding).
- **A strict content policy on everything it serves.** Code loads only from the box, Castle
  cannot be shown inside another site's frame, and no address is sent as a referrer.
- **Nothing leaves the vault.** Names are validated, `..` and hidden names are refused, and
  symlinks are never followed.
- **No overwrites.** An upload, new folder or restore whose name is taken becomes
  `name (2)`.
- **Whole files only.** Uploads stream to a temp file and are renamed into place.
- **Trash is reversible.** Trashed items move to `<vault>/.castle/trash` until restored or
  deleted for good.

## What it does not do yet

- **No TLS.** Use it on this machine, or through a tunnel such as Tailscale that encrypts
  the link. Do not expose it to the internet as is.
- **One token, one user.** No per-person vaults, no pairing by code or QR, no revoking one
  device without changing the token for all.
- **No resumable uploads.** A dropped upload starts again from the beginning.
- **Delete is an ordinary delete.** It is not a secure erase.
- **No file sync.** It is a drive you browse, not a folder that mirrors itself.

## API (v1)

All under `/api/v1`. Ids are vault paths with `/` separators; the top folder is `""`.

| Call | Does |
|---|---|
| `GET /hello` | Name and version. No token needed. |
| `GET /list?path=` | Children of a folder. |
| `GET /tree` | Every item, plus the trash. |
| `GET /stat?path=` | One item. |
| `GET /stats` | Bytes used, and used plus free disk space as `quota`. |
| `GET /file?path=` | File contents. |
| `PUT /upload?parent=&name=` | Upload; the request body is the file. |
| `POST /mkdir` `{parent, name}` | New folder. |
| `POST /rename` `{id, name}` | Rename. `409` if the name is taken. |
| `POST /move` `{id, target}` | Move into another folder. |
| `POST /star` `{id}` | Toggle the star. |
| `POST /trash` `{id}` | Move to trash. |
| `POST /restore` `{id}` | Restore a `trash:<key>` item. |
| `POST /delete` `{id}` | Delete for good (a live item or a trash item). |

## Clones (needs `--clonewars`)

One-click automations that run in Clonewars VMs. The daemon runs only
`start.ps1 -Profile <name>` for automations compiled into it (today: `penguin`), so this
is not a way to run arbitrary commands. It works only while the daemon listens on this
machine alone.

| Call | Does |
|---|---|
| `GET /clones` | Each automation: state (`stopped`, `launching`, `running`, `failed`), saved account name, whether a password is saved, and the tail of the launch log. The password is never sent back. |
| `POST /clones/enable` | Turns Clones on in a running box. Takes no folder: the box looks for `clonewars` in `CASTLE_CLONEWARS_DIR`, then beside or above its own program and working folder. Loopback only. |
| `POST /clones/login` `{id, username, password}` | Saves the login to the project's `.env`. An empty password keeps the saved one. |
| `POST /clones/start` `{id}` | Launches the VM. If the project is not checked out next to Castle, it is cloned first from the address in `CASTLE_PENGUIN_REPO`. Refused while one is running. |
| `POST /clones/stop` `{id}` | Sends the VM a power-button shutdown over its QMP port. |

"Running" means a QEMU process is using that automation's disk in this clonewars folder,
whoever started it.

## Arcade (needs `--arcade`)

A read-only view of a games folder laid out like this:

```text
<arcade>/Consoles/RetroArch-Win64/   retroarch.exe, cores/, playlists/*.lpl, thumbnails/
<arcade>/Consoles/PCSX2/             pcsx2-qt.exe
<arcade>/Consoles/PPSSPP/            PPSSPPWindows64.exe
<arcade>/Roms/<maker>/<system>/      the games
```

RetroArch's playlists are the library and its thumbnails folder is the box art, read
where they are. PlayStation 2 and PSP have no playlist, so their `Roms` folders are
listed directly and open in PCSX2 and PPSSPP. Nothing is written to the folder.

| Call | Does |
|---|---|
| `GET /arcade` | Systems found, with game counts, art counts and sizes. |
| `GET /arcade/games?system=` | The games of one system. |
| `GET /arcade/art?system=&game=&kind=` | A game's `boxart` or `snap` PNG. |
| `POST /arcade/enable` | Turns Arcade on in a running box. Takes no folder: the box looks in `CASTLE_ARCADE_DIR`, then for `Games` at the top of each drive or mounted disk. Loopback only. |
| `POST /arcade/play` `{system, game}` | Opens the game in its emulator on this machine. Loopback only. |

## Network

What the box can see of its own connection. It is not the router yet, so this is the
traffic of the machine it runs on, not of every device in the house.

| Call | Does |
|---|---|
| `GET /network` | Byte counters for each network adapter, plus this machine's address, gateway and DNS servers. The app turns two readings into a rate. |
| `POST /network/speedtest` | Times a 50 MB download and a 10 MB upload against Cloudflare's speed-test service, and a response time. Takes 10 to 30 seconds. |
| `GET /network/provider` | Asks ipinfo.io who the internet provider is. That service sees the public address. |

The speed test and the provider lookup are the only calls that reach the internet, and
they run only when asked for.

## Settings and box facts

| Call | Does |
|---|---|
| `GET /settings` | The settings the app has saved on the box. |
| `PUT /settings` `{name: value, ...}` | Merges settings in. Names are dotted (`storage.trashDays`); values are a switch, a number or short text. |
| `GET /box` | Version, listen address, whether Clones and Arcade are on, and whether Tailscale or WireGuard is installed (with Tailscale's state). |

The box stores settings; it does not act on them yet. The games folder and the
Clonewars folder are deliberately not settings: they are chosen when the box starts, or found by the box itself when a section is turned on later, so a
web page cannot point the box at a new folder of programs to run.

Reading Tailscale's state uses `tailscale status --json`, whose layout Tailscale does not
document as stable. It has not been tried against an installed Tailscale.

## Security

| Call | Does |
|---|---|
| `GET /security` | The posture checks, the state of the activity record, refused calls in the last 24 hours, and when the token was made. |
| `GET /security/log?limit=N` | The newest entries of the activity record, newest first. |
| `POST /security/verify` | Re-reads the whole record and checks every entry follows from the one before. |
| `POST /security/token/rotate` | Replaces the box token and returns the new one. Every device holding the old one is refused from then on. |
| `GET /security/audits` | The audits this box can run, and whether it can run them at all. |
| `POST /security/audits/run` `{id}` | Runs one audit and returns its table. |

**The activity record.** Every call that changes something is written to
`.castle/security/activity.log` after it finishes: what it was, its status, and the file
or item it named. Bodies are never stored whole, so a saved login's password does not land
there. Each entry carries the SHA-256 of the entry before it, so an edited or removed
entry breaks the chain from that point and `verify` says where. Limits: someone with
access to the disk can rewrite the whole file or cut entries off the end, and nothing
notices until the newest hash is also held somewhere else. That is not built. The file
grows without limit. Reads are not recorded. Calls with a wrong token are counted every
time and written at most once a minute.

**The checks** say what the box can tell about itself: whether it listens beyond this
machine, whether a Tailscale link is up, whether the record is intact, refused calls,
the token file's permissions (Unix only), and whether the vault's disk is encrypted
(BitLocker on Windows, a `crypt` layer under the mount on Linux). Three more are listed
as not built: accounts, Castle's own file encryption, and a copy of the record elsewhere.

**Audits** are the bash scripts in [`../security/audits`](../security/audits), compiled
into the binary and fed to `bash` on standard input, so there is no script on disk to
swap. They only read. They need a Linux box; elsewhere the list is returned with
`available: false`. Without root some columns are blank (process names behind open
ports, other accounts' crontabs). `bash security/audits/test.sh` runs them all and checks
the shape of their output.

## Tests

```powershell
cargo test
```
