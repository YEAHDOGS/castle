# castled -- the Castle box daemon

The program that runs on the Castle box. It serves one directory (the vault) over a small
HTTP API, and every Castle client talks to that API: web, Windows, Linux, Android, iOS.
No client needs Samba. Samba can still share the same directory for anyone who wants a
mapped network drive, but the apps do not depend on it.

## Run it

```powershell
cd castled
cargo run -- --data .\castle-data --web ..\..\castle-app\dist
```

It prints a link ending in `#connect=<token>`. Open it and the app is connected.
`--web` is optional: without it the daemon prints the token, and you paste that into the
app's Connect screen (the server icon in the top bar).

| Flag | Default | Environment |
|---|---|---|
| `--data` vault directory | `./castle-data` | `CASTLE_DATA_DIR` |
| `--bind` address and port | `127.0.0.1:8787` | `CASTLE_BIND` |
| `--web` built Castle app to serve at `/` | none | `CASTLE_WEB_DIR` |

## What it guarantees

- **This machine only by default.** It listens on `127.0.0.1`. Binding to another address
  makes it reachable from the network over plain HTTP, and it says so at startup.
- **Token on every call.** Everything except `GET /api/v1/hello` needs
  `Authorization: Bearer <token>`. The token is created on first start and kept in
  `<vault>/.castle/token`.
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

## Tests

```powershell
cargo test
```
