# castled -- the Castle box daemon

The program that runs on the Castle box. It serves one directory (the vault) over a small
HTTP API, and every Castle client talks to that API: web, Windows, Linux, Android, iOS.
No client needs Samba. Samba can still share the same directory for anyone who wants a
mapped network drive, but the apps do not depend on it.

## Run it

```powershell
cd castled
cargo run -- --data ..\app\data\castle --web ..\app\dist
```

It prints a link ending in `#connect=<token>`. Open it and the app is connected.
`--web` is optional: without it the daemon prints the token, and you paste that into the
app's Connect screen (the server icon in the top bar).

| Flag | Default | Environment |
|---|---|---|
| `--data` vault directory | `../app/data/castle` (app/data/castle, git-ignored) | `CASTLE_DATA_DIR` |
| `--bind` address and port | `127.0.0.1:8787` | `CASTLE_BIND` |
| `--web` built Castle app to serve at `/` | none | `CASTLE_WEB_DIR` |
| `--clonewars` Castle's `clonewars` folder, to run automations | none | `CASTLE_CLONEWARS_DIR` |
| `--arcade` a games folder (`Consoles/` + `Roms/`) to show in Arcade | none | `CASTLE_ARCADE_DIR` |
| `--places` a folder whose subfolders the vault may be moved to from the app | none | `CASTLE_PLACES` |
| `--state` a folder outside the vault where that choice is kept (goes with `--places`) | none | `CASTLE_STATE_DIR` |

## Run it in Docker

The box as one container, serving the app at its own address. From the top of the repo:

```sh
cp docker-box.env.example .env      # then say where the drives are
docker compose up -d --build
docker compose logs castle          # prints  open: http://0.0.0.0:8787/#connect=<token>
```

Open that link from any device at home, with the computer's own address in place of
`0.0.0.0`. `.env` names the folder the box starts on (`CASTLE_VAULT`), up to two more drives
(`CASTLE_DRIVE_1=F:/:/drives/big`), a media folder for Theater and the port. Each drive under
`/drives` is a place the vault can be moved to from Settings > Storage (see Storage places
below). The container restarts with the computer.

What the container does not have: Clones and Arcade (they start programs on the computer
itself), DOGS AI, and the rest of the computer in Storage. It is plain HTTP, like any box
that listens beyond `127.0.0.1`: for the home network or a Tailscale link, never a port
forwarded from the router.

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

## Phoenix

The cleaner and the set-up of a fresh Windows PC (`src/phoenix.rs`). Phoenix is its own
project, the git submodule `app/phoenix/`. The box runs its PowerShell engine
(`scripts/engine/phoenix.ps1`) and hands the result on; it builds no PowerShell itself.
Which copy runs: `CASTLE_PHOENIX_DIR`, else `app/phoenix/`, else a `phoenix` checkout
beside Castle. The app list is the one the standalone Phoenix app keeps
(`%APPDATA%\Phoenix\phoenix.config.json`, or `PHOENIX_CONFIG`).

| Route | What it does |
| --- | --- |
| `GET /phoenix` | Whether Phoenix can run here: `{ available, reason, engine, isAdmin, android, canChange }`. A box on Linux answers `available: false` with the reason. |
| `GET /phoenix/clean?platform=&serial=` | Scans. Deletes nothing. Per rule: bytes, files, the biggest files, the folders looked in, programs to close first, `blocked: "needs-admin"`. `platform` is `windows` (default) or `android`. About 20 seconds on a full PC. |
| `POST /phoenix/clean` `{platform, serial, ids}` | Deletes what the named rules cover, and only those. An empty `ids` is refused. |
| `GET /phoenix/devices` | Android phones on USB. |
| `GET /phoenix/setup` | The app list, Phoenix's catalog, the account names and the hardening choices. |
| `PUT /phoenix/setup` `{apps, computerName, adminName, userName, harden}` | Saves any of them into the shared config. |
| `GET /phoenix/choco?q=` | Searches Chocolatey's community feed (Phoenix's catalog when the feed cannot be reached). |
| `POST /phoenix/script` | The set-up script for the saved list: Chocolatey, the apps, the hardening if chosen. |
| `POST /phoenix/answerfile` `{adminPassword, userPassword}` | A Windows answer file for the saved set-up, which runs that script at the first logon. The passwords go to the engine in a file it deletes as it reads, and into the answer file; the answer file is handed back and not kept. |

Looking is allowed from any device. Deleting, saving and making an answer file work only
from the computer the box runs on, like turning Clones on.

What a cleaning rule may touch is decided by Phoenix, not by the caller: the routes take
rule ids, never paths. See `app/phoenix/docs/CLEANER.md`.

## Links

Pages a person saved from the internet. The box fetches the page once and keeps what it
says, so the link can be searched and read from the box whether or not the page still
exists. As with Contacts and Comms, `person` defaults to `owner`.

| Call | Does |
|---|---|
| `GET /links?person=&q=` | A person's links, newest first, without their text. With `q`, only those holding every word of it (title, site, note, tags, description, ingredients, steps, the page's text), with the words around the match as `snippet`. |
| `GET /links/item?person=&id=` | One link with everything kept: its text, its recipe, the addresses a video's description named. |
| `GET /links/picture?person=&id=` | The link's picture, copied to the box when it was saved. |
| `POST /links/add` `{person, url, note}` | Saves an address. `url` may be the words a phone's Share sends, with the address in them. The page is read before the call answers. The same address twice is one link. |
| `POST /links/refresh` `{person, id}` | Reads the page again. If it has gone, the saved copy stays and the reason is added. |
| `POST /links/update` `{person, id, title, note, tags}` | Changes what the person wrote on it. Any of the three. |
| `POST /links/delete` `{person, id}` | Deletes the link and its picture. |

**What is read.** The title, publisher, author, description and picture a page declares
(Open Graph and the ordinary `<meta>` tags), its text without menus, scripts and forms, and
its recipe when it carries one as schema.org `Recipe` data (what recipe sites publish for
search engines): servings, times, ingredients and steps. A YouTube address is kept with its
full description; up to four addresses in that description are fetched once, and the first
that holds a recipe gives the video its recipe, marked with where it came from.

**On disk.** `<vault>/.castle/links/<person>/<id>.json`, one file per link holding all of
the above, and `<id>.jpg` (or `.png`, `.webp`, `.gif`) beside it. No index and no database:
search reads the files into memory once per start.

**Limits.** A page is fetched as `Castle/<version>`, with no cookies and no script run. A
site that refuses that (some answer 403 to anything that is not a browser) is not worked
around: the link is saved with the reason. Pages built entirely by script have little text
to keep. Pages over 4 MB and pictures over 5 MB are not kept.

**It does not read the home network.** Every name is looked up before it is fetched, and an
address on this computer, a private network, a Tailscale range or a link-local range is
refused, at the first address and at every redirect. The connection goes to the addresses
that were checked.

## A cloud bucket

Storage can show one bucket beside the vault: storage rented from a provider that speaks
the S3 API (Cloudflare R2, Backblaze B2, MinIO, Amazon S3). The box is given the bucket's
address and a pair of keys once, in Storage > Bucket, and keeps them in
`<vault>/.castle/bucket.json`. From then on the box makes every call to the provider
(`src/bucket.rs`); the app talks only to the box, and no call returns the secret.

An item's id is its whole name in the bucket. A folder's id ends with a slash
(`music/2026/`), and the top of the bucket is `""`.

| Call | Does |
|---|---|
| `GET /bucket` | `{connected, endpoint, bucket, publicUrl, keyId, quota}`. `keyId` is the last four characters of the key id. |
| `POST /bucket/connect` `{endpoint, bucket, accessKeyId, secretAccessKey, region, publicUrl, quotaGb}` | Tries the keys with one listing and keeps them only if the provider accepts them. `endpoint` must be `https://` with nothing after the name (plain `http://` only for a provider on this computer). `region` defaults to `auto`. `publicUrl` is where the bucket's files are public, if they are; `quotaGb` is how much it may hold, for the meter. |
| `POST /bucket/disconnect` | Forgets the keys. Nothing in the bucket changes. |
| `GET /bucket/usage` | `{used, objects, quota, complete}`, added up from a full listing and remembered for ten minutes, or until the box next changes the bucket. |
| `GET /bucket/list?path=` | What is directly in a folder. Each item: `{id, name, kind, parentId, mime, size, modified, url}`; `url` is the file's public address when the bucket has one. |
| `GET /bucket/stat?path=` | One item. |
| `GET /bucket/file?path=` | A file's bytes, passed through from the provider. Honours `Range`. |
| `PUT /bucket/upload?parent=&name=` | Upload; the request body is the file, passed on as it arrives. Needs `Content-Length`. A taken name becomes `name (2)`. |
| `POST /bucket/mkdir` `{parent, name}` | New folder: an empty object whose name ends in a slash, so it shows while empty. |
| `POST /bucket/rename` `{path, name}` | Rename. `409` if the name is taken. |
| `POST /bucket/move` `{path, target}` | Move into another folder. |
| `POST /bucket/delete` `{path}` | Delete a file, or a folder and everything in it. |
| `PUT /bucket/write?path=` | Replaces a small text file (up to 1 MB), or makes it. The one call that overwrites: it is for a file the app edits in place, such as a catalog. |

- **A bucket cannot rename.** A rename or move copies each file to its new name and removes
  the old ones only once every copy is there. A folder holding more than 1,000 files is
  refused, to be done in smaller pieces.
- **There is no trash.** A delete is for good, and the app always asks first.
- **Anyone with the box token can use the bucket**, like everything else on the box. The
  keys are as safe as the vault's private folder.

### A catalog in the bucket

A website can read which songs it has from one small file in the bucket instead of from
its own code, so a new song goes live by uploading it. Castle shows a Catalog button in
Storage > Bucket when the bucket holds `music/catalog.json` with `"kind": "dogs-music"`
(`app/src/lib/catalog.js` has the format). Adding a song there uploads its files into the
folders the catalog names and saves the catalog with `PUT /bucket/write`. The bucket needs
a public address for this, since the catalog lists each file by its link.

## Storage places (needs `--places` and `--state`)

| Call | Does |
|---|---|
| `GET /storage/places` | `{current, canChoose, missing, places: [{id, name, path, free, total, current, hasVault}]}`. The first place is the vault the box was started with (`id` `""`); the rest are the folders under `--places`, each holding its vault in a folder named `Castle`. `missing` names a chosen place that was not there at start. |
| `POST /storage/place` `{id}` | Makes that place the vault. A new vault gets a copy of the token and the settings, so devices stay connected. The choice is written to `<state>/vault`, and the box stops so that what runs it starts it again there. |

Files are not moved. The vault that was in use stays where it is, whole, and choosing its
place again opens it. A place can only be named by a folder directly under `--places`, so
this cannot point the box at any other folder. It stops the box on purpose: use it only
where something restarts the box (the Docker image does). When the chosen drive is not
there at start, the box says so and opens the vault it was started with, rather than
making an empty vault where the drive should have been.

## Clones (needs `--clonewars`)

One-click automations that run in Clonewars VMs. The daemon runs only
`start.ps1 -Profile <name>` for automations compiled into it (today: `penguin`), so this
is not a way to run arbitrary commands. It works only while the daemon listens on this
machine alone.

| Call | Does |
|---|---|
| `GET /clones` | Each automation: state (`stopped`, `launching`, `running`, `failed`), saved account name, whether a password is saved, and the tail of the launch log. The password is never sent back. |
| `POST /clones/enable` | Turns Clones on in a running box. Takes no folder: the box looks for `clonewars` in `CASTLE_CLONEWARS_DIR`, then beside or above its own program and working folder. Loopback only. |
| `GET /clones/screen?path=<id>` | One PNG of a running automation's VM screen, taken through the VM's control port (QMP `screendump`). It only looks; nothing is sent into the machine. Needs QEMU 7.1 or newer. |
| `POST /clones/login` `{id, username, password}` | Saves the login to the project's `.env`. An empty password keeps the saved one. |
| `POST /clones/start` `{id}` | Launches the VM. If the project is not checked out next to Castle, it is cloned first from the address in `CASTLE_PENGUIN_REPO`. Refused while one is running. |
| `POST /clones/stop` `{id}` | Sends the VM a power-button shutdown over its QMP port. |
| `POST /clones/automation/save` `{..., script, schedule}` | `script` is a Castle script (`app/clonewars/automation/README.md`); it is compiled on save and refused with its first problem and line. `schedule` is `HH:MM`: the first bot starts every day at that time on the box's clock. |
| `POST /clones/script/check` `{script}` | Compiles a script without running it: `{ok, problems: [{line, message}], python, web, datasets, tracked, fields}`. |
| `GET /clones/results?path=<id>` | An automation's runs (status, the line it is on or failed at, its pictures), the rows each dataset holds after its newest post, its tracked values over 180 days, the newest run's log, and the update in words. |
| `GET /clones/results/shot?path=<run>/<n>` | One picture a run took, as PNG. |
| `POST /clones/report` | **No box token.** A script reporting from inside its machine, with `Authorization: Run <secret>`: the secret Castle wrote into that run's `castle-run.json`, kept only as a hash. It opens that one run's results until the run ends, and nothing else. The machine reaches the box as `host.docker.internal` (container) or `10.0.2.2` (QEMU), so this one route skips the check that the request names this computer. |

A script run's results are in `<vault>/.castle/clones/results.db` (SQLite: runs, rows,
tracked values, log lines, the daily schedule) and its pictures in `shots/` beside it.

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

### Save data

| Call | Does |
|---|---|
| `GET /arcade/saves` | Every save file on the games drive, newest first: its core, kind (`battery` for the game's own save, `state` for a save state), the game it belongs to, and how many versions are recorded. |
| `POST /arcade/saves/record` | Records every save that changed since it was last recorded. Reads the drive; writes only to the vault. |
| `GET /arcade/saves/history?path=<save id>` | Every recorded version of one save, newest first. |
| `POST /arcade/saves/restore` `{id, hash}` | Puts a recorded version back in place of the save file. Loopback only. |

Saves are read from RetroArch's `saves/<core>/` and `states/<core>/` folders and from
`Saves/RetroArch/` on the drive. A save belongs to a game when its name, without the
extension, is the game file's name. GameCube memory-card files are named by game code, so
they are matched by the code read from each GameCube game file. Versions live in `<vault>/.castle/saves`: each
distinct file once under `objects/<sha256>`, and `journal.jsonl` listing every version in
order. Restoring is the only call that writes to the games drive. It replaces that one
save file, and records the file as it was first, so a restore can be undone. Recording is
not automatic yet: it happens when the Saves view is opened or Record now is pressed.

## Network

What the box can see of its own connection. It is not the router yet, so this is the
traffic of the machine it runs on, not of every device in the house.

| Call | Does |
|---|---|
| `GET /network` | Byte counters for each network adapter, plus this machine's address, gateway and DNS servers. The app turns two readings into a rate. |
| `POST /network/speedtest` | Times a 50 MB download and a 10 MB upload against Cloudflare's speed-test service, and a response time. Takes 10 to 30 seconds. |
| `GET /network/provider` | Asks ipinfo.io who the internet provider is. That service sees the public address. |

The speed test and the provider lookup here reach the internet, and they run only when
asked for. (So do saving a link, and looking up a game, film or album.)

## Settings and box facts

| Call | Does |
|---|---|
| `GET /settings` | The settings the app has saved on the box. |
| `PUT /settings` `{name: value, ...}` | Merges settings in. Names are dotted (`storage.trashDays`); values are a switch, a number or short text. |
| `GET /system` | Vital signs of the machine the box runs on: processor use since the last call, memory, the vault disk's free and total space, uptime, host name and OS. |
| `GET /box` | Version, listen address, whether Clones and Arcade are on, and whether Tailscale or WireGuard is installed (with Tailscale's state). |
| `POST /box/stop` | Stops the box and everything it started: Clones' machines (VMs get 20 s to shut down, then are switched off), containers and emulators, then DOGS AI and its ComfyUI. Only a box that listens on this computer alone. The desktop app sends it when it closes; Ctrl+C does the same. |

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

**Audits** are the bash scripts in [`audits/`](audits), compiled
into the binary and fed to `bash` on standard input, so there is no script on disk to
swap. They only read. They need a Linux box; elsewhere the list is returned with
`available: false`. Without root some columns are blank (process names behind open
ports, other accounts' crontabs). `bash castled/audits/test.sh` runs them all and checks
the shape of their output.

## Tests

```powershell
cargo test
```
