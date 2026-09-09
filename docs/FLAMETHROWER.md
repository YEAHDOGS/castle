# The Flamethrower — True Deletion on Castle

Brando's directive (2026-09-09): when something is deleted on Castle with
the flamethrower, it must be **irretrievable**. Not "moved to trash." Not
"overwritten once." Gone at the level of physics. This doc is the design.

## Why deletion is hard: the chips lie to you

To build a real deletion tool you have to understand what you're burning:

- **HDDs (spinning rust)**: the OS writes sector 100, the head writes
  sector 100. Overwrite works. One full pass of zeros defeats every
  software recovery tool; the old "35-pass Gutmann" lore is dead.
- **SSDs / NVMe / eMMC / SD cards (NAND flash)**: the OS writes sector
  100, the flash controller writes *wherever it wants* — wear leveling,
  over-provisioning (typically 7–28% invisible spare area), and the flash
  translation layer mean your overwrite almost certainly did NOT touch
  the physical cells holding the old data. **Software overwrite on flash
  is a polite fiction.** The old bytes sit in unmapped pages until the
  controller's garbage collector gets around to erasing them. Forensics
  tools read unmapped pages.
- **The only true kills on flash**: (a) the drive's own firmware erase
  (ATA Secure Erase, NVMe Format with Secure Erase / Sanitize — the
  controller erases every block including spares), or (b) crypto-shredding.

This is why the flamethrower is media-aware. It never pretends an
overwrite did something it didn't.

## Tier 1 — Crypto-shredding (the foundation)

Every vault on Castle is encrypted at rest with per-vault keys. Deleting
the key **is** deleting the data — instant, total, media-independent.
AES-256 ciphertext without the key is noise; no forensics recovers it.

- File delete with flamethrower → shred the file's data key.
- Vault delete → shred the vault master key.
- This is the default kill. It works on HDDs, SSDs, SD cards, anything.

## Tier 2 — Firmware erase (device retirement)

When a whole drive leaves the Castle (sold, RMA'd, dead):

- **NVMe**: `nvme format --ses=2` (crypto erase) or Sanitize.
- **SATA SSD**: ATA Secure Erase (firmware-level).
- **HDD**: single-pass overwrite + verify (nwipe-style).
- Refuse to "overwrite" an SSD and call it done — same rule as the
  Phoenix nuke tool. The tool knows the media type and picks the real
  kill or refuses.

## Tier 3 — File shredding (spinning rust only, honest labeling)

On HDDs: overwrite file extents, rename, truncate, verify by read-back
sampling. On flash: the tool says plainly "best effort — use
crypto-shred for guarantees" and offers to re-encrypt-then-shred-key
instead. Never lies about what it did.

## Verification

Every burn returns a certificate: what was deleted, which method, media
type, verification result (read-back sample / firmware confirmation /
key destruction receipt). A deletion you can't verify is a rumor.

## Safety: the most dangerous tool gets the strongest interlocks

The flamethrower can destroy what backups can't restore. So:

- Explicit target enumeration — the tool shows you *exactly* what dies.
- Typed confirmation (the name of the vault / the disk serial, not "y").
- Dry-run is the default. The real run is the exception.
- No recursive globs, no "everything older than X" without a list-first.
- Abort window before firmware-level kills.

Dangerous is fine. Accidental is not. Same DNA as the Phoenix nuke
interlocks.

## Build order

1. Per-vault encryption at rest (makes Tier 1 possible at all).
2. Key lifecycle: generation, escrow (family recovery!), destruction
   ceremony with certificate. ✅ done
   (`flamethrower/keyring.py`: `create-vault` generates a 256-bit data
   key (CSPRNG) with an optional escrow copy; `destroy-vault` burns every
   copy and issues a deletion certificate + tier-1 audit record. Family
   recovery closes the loop: `recover-vault NAME [--yes NAME]` restores
   a lost live key from its escrow copy — refused if no usable escrow
   exists, refused if the live key is still present (recovery would fork
   the key), escrow bytes SHA-256-checked against the recorded receipt
   before the write and the installed copy verified after (foreign
   keys fail closed), escrow copy preserved, dry-run default, typed
   confirmation, recovery certificate + one tier-1 audit record, hashes
   only — never key material. 19 regression tests green, temp dirs only.)
3. Media detection (HDD vs SATA SSD vs NVMe vs removable flash).
4. Tier 2 firmware-erase routines (shared with Phoenix tooling). ✅ done
5. Tier 3 file shredder with honest media labeling. ✅ done
   (`flamethrower/tier3.py`: HDD overwrite→verify→rename→truncate→unlink;
   flash runs the same sequence labeled BEST EFFORT with a Tier-1
   crypto-shred pointer; typed basename confirmation; dry-run default;
   deletion certificates; 32 regression tests)
6. Deletion certificates + an audit log (ironic but necessary: the log
   records *that* something was burned, never its contents). ✅ done
   (`flamethrower/audit.py`: append-only JSONL audit log at
   `<root>/audit.jsonl` with a SHA-256 hash chain — each entry links the
   previous entry's hash, so edits, deletions, and reordering are all
   detectable by `audit verify`; every Tier 1/2/3 burn appends one entry
   per certificate — timestamp, operator, tier, cert id, fingerprint
   (vault/key receipt, disk serial, file path — never key material or file
   contents), method, media, verification result, success flag.
   Incomplete records are refused, never guessed. Dry runs log nothing.
   22 regression tests; 118 green total across the module.)
7. Per-file crypto-shred (the first true-deletion primitive at file
   granularity). ✅ done
   (`flamethrower/fileburn.py`: `seal` a file with a fresh random 256-bit
   per-file key — ciphertext (SHA-256 counter-mode stream cipher,
   prototype; production path = AES-256-GCM) lives in a designated
   `files/` dir, keys in `filekeys/` (+ optional escrow) — then `burn`
   destroys every key copy (CSPRNG overwrite x3, fsync, unlink) AND
   overwrites + unlinks the ciphertext. Dry-run is the default, real
   burns need typed confirmation (the file name), names are strictly
   validated so a burn can never leave the designated dirs, every burn
   issues a deletion certificate + one tier-1 audit record (hashes only —
   never key material or file contents), post-burn `unseal` is impossible.
   11 regression tests, temp dirs only.)
8. Whole-directory burn with manifest + verify pass. ✅ done
   (`flamethrower/dirburn.py`: `burn_directory` runs the step-7 seal+burn
   primitive on every regular file in a target dir, then overwrites +
   unlinks the originals, writes a JSON manifest (path, bytes,
   sha256-before, timestamp — hashes only, never contents) to
   `<root>/manifests/<burn_id>.json`, removes the emptied dir tree, and
   runs a verify pass that fails LOUDLY with the path of any surviving
   file. Dry-run is the default; real burns need typed confirmation (the
   target's basename). The target must be a real directory — not a
   symlink, not `/`, not the fileburn root, not containing it, not
   inside it. Symlinks, non-regular files, hardlink duplicates, and
   paths resolving outside the target abort the whole burn before
   anything dies (canary-tested). 8 regression tests, temp dirs only.)
9. Unified true-deletion CLI (`flamethrower/flamethrower.py`). One
   entrypoint: `plan` shows what would die; `burn` is dry-run by
   default (exit 2, nothing touched). Real deletion needs BOTH an
   explicit `--i-understand` admission AND the typed basename of every
   target (`--confirm` or interactive TTY) — a mismatch on either side
   aborts with nothing destroyed. Multi-pass overwrite for files
   (CSPRNG x (n-1), final zero pass, fsync'd every pass, read-back
   sample verification of the final pass, then rename → truncate →
   unlink), flash targets labeled BEST EFFORT with the Tier-1 pointer.
   Every real deletion emits a deletion certificate AND appends one
   audit record per certificate to `<root>/audit.jsonl` (cert id, file
   path, size, method, media, verification — never contents). 9
   fixture-based regression tests, temp dirs only. ✅ done
   (`flamethrower/flamethrower.py`: wraps tier3.py with the mission's
   arming ritual — dry-run default, `--i-understand` + typed basename
   per target, CSPRNG multi-pass + final zero pass with read-back
   verification, rename→truncate→unlink, certificates in
   `<root>/certificates/` + one tier-3 audit record per cert. 9 fixture
   tests; 116 green across the flamethrower module.)
10. Residue wiping: slack space + free space (Tier-3-adjacent). ✅ done
    (`flamethrower/slack.py`: `wipe-slack` extends a file to the next
    filesystem block boundary with CSPRNG bytes, fsyncs, truncates back
    to the exact original size and fsyncs — contents and size preserved,
    only the slack tail destroyed; `wipe-free` fills every free cluster
    under a directory with CSPRNG filler files to ENOSPC, fsyncs each,
    unlinks them all, fsyncs the directory. Media is detected, never
    assumed — unmappable media refuses; flash targets are labeled BEST
    EFFORT with the Tier-1 crypto-shred pointer, never blessed as a
    kill. Dry-run is the default; real wipes need typed basename
    confirmation. Symlinks, non-regular files, and the filesystem root
    are refused. Every wipe issues a deletion certificate + one tier-3
    audit record (hashes only — never contents). 15 fixture-based
    regression tests, temp dirs only.)
