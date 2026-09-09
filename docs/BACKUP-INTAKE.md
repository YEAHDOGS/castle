# Castle Backup Intake — the Phoenix backup target contract

Answers Phoenix's open founder question (EMERGENCY-RUNBOOK.md, Appendix C,
Q2): *"Where exactly is Castle's 10TB target — SMB share name/path, and
which credentials?"* This doc defines Castle's side of the contract; the
physical disk → share mapping is still Brando's call (marked below).

## The contract (Castle's side)

**Intake root** (on-Castle path): `~/.castle-backups/`
(the operator can point the CLI anywhere with `--dir`)

**Layout:**

```
.castle-backups/
  intake.jsonl          # append-only manifest: one record per intake
  activity.jsonl        # who registered what, metadata only
  machines/
    <machine-name>/     # e.g. brando-laptop, brando-desktop
      images/           # disk images (Rescuezilla/Veeam etc.)
        quarantine/     # INFECTED images — see rules below
      data/             # data backups (docs, photos, configs)
```

**Manifest record** (per intake, SHA-256 verified at write time):

| field | meaning |
|---|---|
| `machine` | which machine backed up |
| `kind` | `disk-image` or `data` |
| `label` | human label, e.g. `QUARANTINE-INFECTED-2026-09-09` |
| `relpath` | path inside the intake root |
| `bytes` | size |
| `sha256` | verified digest — `verify` re-hashes to catch bit-rot/tampering |
| `quarantine` | true → never mount/boot/open on a daily driver |
| `note` | operator note (metadata only) |

**CLI** (`backup/intake.py`, stdlib only, no network):

```bash
# stage a disk image, quarantine it, refuse on sha mismatch
python3 intake.py register --machine brando-laptop --kind disk-image \
  --label QUARANTINE-INFECTED-2026-09-09 --sha256 <64-hex> \
  --quarantine /path/to/rescuezilla-image.img

# data backup
python3 intake.py register --machine brando-laptop --kind data \
  --label DATA-2026-09-09 --sha256 <64-hex> /path/to/backup.zip

# integrity check (exit 2 on drift)
python3 intake.py verify
python3 intake.py verify --machine brando-laptop

# retire a superseded backup (typed confirmation, honest destruction)
python3 intake.py retire --id <16-hex-id> --confirm-label <EXACT-LABEL>
#   — 2x CSPRNG overwrite + zero pass (fsync'd), read-back sampled,
#   then rename→truncate→unlink; appends a "retirement" record with the
#   file's sha256 fingerprint (never contents) so the manifest stays a
#   true history. verify skips retired intakes; list marks them RETIRED.
#   Refuses: unknown id, already retired, label mismatch, missing file,
#   sha drift (never destroy data you can't verify), and quarantined
#   images without --release-quarantine (forensics evidence).

# manifest (metadata only)
python3 intake.py list
```

**Quarantine rules (non-negotiable):**

1. Infected images go to `images/quarantine/` with a
   `QUARANTINE-DO-NOT-MOUNT.txt` marker.
2. Castle tooling NEVER mounts, boots, or opens a quarantined image.
3. Forensics happens on an isolated setup, never the daily driver.
4. Registration is refused unless the claimed SHA-256 matches the bytes
   on disk — a quarantined image with an unverifiable hash is rejected
   at the door.

**Security posture of the intake:**

- Names strictly validated (alnum + `-_.`), destinations can never
  escape the intake root, symlinks refused as sources.
- All dirs 0700, files 0600. Logs carry metadata only — never file bytes.
- Corrupt intake logs are refused, never trusted (`verify` fails loud).

## What Phoenix's runbook can now say

Phase 0, Step 0.7: after imaging the infected drive to direct-attached
USB, copy the image to Castle and register it:

```bash
ssh castle 'python3 ~/castle/backup/intake.py register \
  --machine brando-laptop --kind disk-image \
  --label QUARANTINE-INFECTED-$(date +%F) \
  --sha256 $(sha256sum image.img | cut -d" " -f1) \
  --quarantine image.img'
```

Phase 4 (scheduled Veeam Agent full backups): target the Castle intake
per the transport below, then register each backup file.

## Transport — still needs Brando's answer ❓

| question | status |
|---|---|
| Physical target: which device/volume backs `~/.castle-backups` (the 10TB drive)? | **needs Brando** |
| SMB share name exposing it to the LAN (Veeam needs this in Step 4.4) | **needs Brando** |
| Credentials for the share / SSH user for the intake CLI | **needs Brando** |
| Veeam Agent Free job definition once the share exists | open Phoenix-side work |

Proposed (only a proposal until Brando says so): SMB share `castle-backups`
→ the 10TB drive, `--dir` default moved there on the Castle box, share
auth via a dedicated backup user (never the admin/root account), Veeam
writes to the share and a Castle-side cron registers + verifies each
new backup file into the manifest.

## Tests

`backup/test_intake.py` — 28 fixture-based regression tests (temp dirs
only): layout, refusals (sha mismatch, unsafe names, symlinks, bad kinds),
idempotency, quarantine layout + marker, verify clean/corrupt/missing,
corrupt-log refusal, metadata-only logs, CLI exit codes, plus 10 retire
tests (typed-confirmation refusals, double-retire, corrupt/missing-file
refusals, quarantine release-gate, verify-skip/list-flag, metadata-only
retirement records).
