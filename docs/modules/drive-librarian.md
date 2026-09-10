# Drive Librarian — every removable drive Castle has ever seen

**Drive Librarian.** Brando's problem (2026-09-09): ~100 flash drives and
spare hard drives floating around, no idea what's on which, when it was last
plugged in, or whether its contents still exist anywhere. The Librarian gives
every removable drive a permanent identity, logs every sighting, and keeps an
immutable on-Castle backup — no file ever written to the drive itself.

Two jobs, one module:
1. **Record keeping** — tag each drive, know it apart from every other drive,
   and always know when Castle last saw it.
2. **Backup** — when a known drive is plugged in, refresh its on-Castle backup
   automatically. When it's unplugged, the backup and the record remain.

## Identity without writing to the drive

Yes — there is a serial number. Every USB mass-storage device reports one
(`iSerial`), and Linux exposes it through udev. Nothing is ever written to
the drive; identity is read from the hardware and the existing filesystem
metadata:

- `ID_SERIAL_SHORT` — the USB device serial (usually unique per unit).
- `ID_VENDOR_ID` / `ID_MODEL_ID` / `ID_VENDOR` / `ID_MODEL` — who made it.
- Exact byte capacity — two "16GB" drives are rarely the same byte count.
- Partition-table GUID + filesystem UUIDs (`ID_FS_UUID`) — already on the
  drive, read-only.

**Fingerprint** = `usb:<vendor>:<model>:<serial>` + capacity + FS UUIDs,
stored at first sighting. A udev rule pins it: that fingerprint is that
physical drive, forever.

**Honest caveat:** bargain-bin flash drives sometimes ship with blank or
duplicated serials (`0123456789ABCDEF` is a classic). For those, the Librarian
falls back to a composite fingerprint — serial + exact capacity + partition
layout hash — which is still unique in practice across a personal collection,
still without writing anything.

## Last-seen tracking (always, even without backup)

udev `add`/`remove` rules call the Librarian handler on every plug/unplug:

- **Plug in** → identify by fingerprint, record sighting
  (`first_seen`, `last_seen`, capacity, mount point), then start backup.
- **Unplug** → record the departure. The drive's row keeps its tag, its full
  sighting history, and its latest backup.

The sighting is logged even if the backup is skipped, fails, or the drive is
unknown. "When did I last see the blue SanDisk?" is always answerable.

Unknown drives (never fingerprinted) are **not** auto-backed-up or auto-
mounted — they're logged as sightings and flagged "untagged" in the
dashboard until Brando tags or enrolls them. See security notes below.

## Backup on plug-in

For enrolled drives, plug-in triggers:

1. Mount read-only-ish (`noexec,nodev,nosuid` — the drive is untrusted
   media until proven otherwise).
2. `rsync` the contents into a new timestamped snapshot under the Castle
   backup root: `drives/<fingerprint>/<ISO-timestamp>/`.
3. Write a **hash manifest** (`manifest.sha256` — SHA-256 of every file) and
   verify it immediately.
4. Flip the snapshot read-only (`chattr +i` / read-only bind). Snapshots are
   **immutable**: later backups never modify an old snapshot, they add a new
   one. Retention is per-drive and configurable (default: keep the last 5
   snapshots, always keep the first).
5. Record the backup in the drive's row: snapshot path, file count, bytes,
   manifest hash, duration.

Deduplication across snapshots follows the same content-addressed thinking
as the rest of Castle's backup story — identical files are stored once
(see `docs/REDUNDANCY.md` for the storage layer this sits on).

## Tagging

- **User tags** in the Librarian DB: `tax-2019`, `mom-photos`, `spare-blank`,
  whatever Brando wants. Multiple tags per drive, searchable.
- **Auto hints** (read-only scan at backup time): volume label, top-level
  directory names, total files. Shown as suggestions, never as truth — the
  user's tag is the source of record.

## Online vs. offline drives

A drive has a **role**, set per drive:

- **`archive`** (default) — the drive lives in a drawer. Castle keeps its
  record, its sighting history, and its latest immutable backup. Dashboard
  shows it as OFFLINE with "last seen" front and center.
- **`pool`** — the drive stays plugged into Castle and joins the expandable
  storage pool (live capacity for file storage / cloud hosting). Pool drives
  are still fingerprinted and sighting-logged, but they're storage, not
  backup sources.

A drive can change roles any time. Nothing about pooling weakens the
immutability rule: backups are snapshots, never live mirrors.

## Dashboard — the shelf

One view: every drive Castle has ever seen. Tag, vendor/model, capacity,
role (archive/pool), **last seen** (with "3 days ago" style relative time),
last backup status, snapshot count. Filter by tag, by online/offline, by
"not seen in 6 months" (the "do I still own this?" audit).

## Security model

Removable media is an attack vector (malicious USB devices are a real
thing), so the Librarian is paranoid by default:

- Unknown drives are never auto-mounted with exec permissions and never
  auto-backed-up. Sighting logged, dashboard flag raised, Brando decides.
- Known drives mount `noexec,nodev,nosuid`. Backup reads files; it never
  executes anything from the drive.
- The fingerprint DB and manifests live on Castle's redundant storage, not
  on the drives — losing a drive never loses its record.
- Snapshot immutability is enforced at the filesystem level, not by
  convention. A compromised backup job can't rewrite history.

## Out of scope (for now)

- Full-disk imaging of flash drives (file-level backup covers the use case;
  imaging 100 drives is a different project).
- Health monitoring per drive (SMART doesn't exist on most flash drives;
  sighting + manifest re-verification is the integrity story).
- Write-blocking hardware — nice for forensics, overkill here since we
  never write to the drives anyway.
