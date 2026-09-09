#!/usr/bin/env python3
"""Castle backup restore — FAMILY-DATA-VAULT.md build-order step 8 (the
missing half).

``backup`` seals a user's vault into an encrypted .castle file and
*proves* it restores. ``restore`` actually does the restoring: it takes
an encrypted backup (either the openssl-backed castle-vault/v1 format or
the pure-stdlib castle-chunks/v1 container) and writes the plaintext
tree back to a destination directory.

Rules:

- dry-run is the default: ``plan`` shows what would land where, and a
  real restore needs typed confirmation — the destination directory's
  basename (same arming ritual as the rest of the repo).
- the secret is verified BEFORE anything is written: HMAC, the tarball
  hash, and every per-file SHA-256 are proved in memory first. Any
  mismatch refuses with nothing written to disk.
- the destination must not exist, or must be an empty directory. A
  restore never clobbers existing data.
- the destination cannot be the filesystem root, cannot be a symlink,
  and cannot contain (or be) the backup file itself.
- tar extraction is path-safe: symlinks/hardlinks and unsafe paths
  are refused (tar-slip protection), 0700/0600 perms are restored, and
  every extracted file is re-hashed against the sealed manifest.
- no network, no new hosts, no outbound anything.

Fixture-based tests live in test_restore.py (temp dirs only).
"""

import io
import os
import secrets
import sys
import tarfile
import time

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)

import backup as _backup  # noqa: E402  (verify helpers; never touches disk)
import chunkseal  # noqa: E402
import manifest as vault_manifest  # noqa: E402
import vault_lock  # noqa: E402


class RestoreError(Exception):
    """Anything that makes a restore untrustworthy is a hard refusal."""


def _ts():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _locked_call(fn, *args, **kwargs):
    try:
        return fn(*args, **kwargs)
    except vault_lock.VaultLockError as e:
        raise RestoreError(str(e))


def _chunk_call(fn, *args, **kwargs):
    try:
        return fn(*args, **kwargs)
    except chunkseal.ChunkSealError as e:
        raise RestoreError(str(e))
    except _backup.BackupError as e:
        # backup.py's tarball inventory check (also used here): a
        # corrupt backup is a refusal in restore-speak too
        raise RestoreError(str(e))


def _peek(backup_file):
    """Detect the container format and read its header (no secret
    needed). Returns (fmt, header)."""
    if not os.path.isfile(backup_file):
        raise RestoreError("not a file: %s" % backup_file)
    header, _ = _chunk_call(chunkseal._read_chunked, backup_file)
    fmt = header.get("format")
    if fmt not in (chunkseal.FORMAT, vault_lock.FORMAT):
        raise RestoreError(
            "not a castle backup file (unknown format id: %r)" % fmt)
    return fmt, header


def _check_dest(dest_dir):
    """Validate the restore destination. Never creates or writes
    anything — the real run does that. Returns the real path."""
    if os.path.islink(dest_dir):
        raise RestoreError(
            "restore destination is a symlink — refusing: %s" % dest_dir)
    if os.path.exists(dest_dir):
        if not os.path.isdir(dest_dir):
            raise RestoreError(
                "restore destination is not a directory: %s" % dest_dir)
        if os.listdir(dest_dir):
            raise RestoreError(
                "refusing to restore into non-empty directory: %s"
                % dest_dir)
    rp = os.path.realpath(dest_dir)
    if rp == os.sep:
        raise RestoreError("refusing: restore destination is the "
                           "filesystem root")
    return rp


def plan_restore(backup_file, dest_dir, passphrase_file=None, keyfile=None):
    """Dry-run: describe what a restore would do. Changes nothing.
    The secret is validated (0600 checks) but never used."""
    if (passphrase_file is None) == (keyfile is None):
        raise RestoreError(
            "exactly one of passphrase-file / keyfile required")
    # 0600 secret discipline, even on a dry run
    _locked_call(vault_lock._read_secret, passphrase_file, keyfile)
    fmt, header = _peek(backup_file)
    bfp = os.path.realpath(backup_file)
    rp = os.path.realpath(dest_dir)
    if rp == os.sep:
        raise RestoreError("refusing: restore destination is the "
                           "filesystem root")
    if bfp == rp or bfp.startswith(rp + os.sep):
        raise RestoreError(
            "the backup file lives inside the restore destination — "
            "refusing (it would corrupt the restored tree)")
    dest = _check_dest(dest_dir)
    files = header.get("files") or {}
    total = sum(e.get("size", 0) for e in files.values())
    count = header.get("file_count", len(files))
    base = os.path.basename(dest.rstrip(os.sep))
    return {
        "dry_run": True,
        "backup_file": bfp,
        "container": fmt,
        "cipher": header.get("cipher"),
        "mac": header.get("mac"),
        "file_count": count,
        "total_bytes": total,
        "sealed_at": header.get("created"),
        "dest_dir": dest,
        "hint": "re-run with --yes %s to restore" % base,
    }


def _extract_verified(tar_bytes, dest_dir, files, files_key="files"):
    """Path-safe extract of a verified tarball, then re-hash every
    file against the sealed manifest. Refuses (nothing left half
    written where it matters) on any mismatch."""
    os.makedirs(dest_dir, mode=0o700, exist_ok=True)
    os.chmod(dest_dir, 0o700)
    with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r") as tf:
        for member in tf.getmembers():
            if member.issym() or member.islnk():
                raise RestoreError(
                    "refusing to extract symlink from backup: %s"
                    % member.name)
            if member.name.startswith("/") or \
                    ".." in member.name.split("/"):
                raise RestoreError(
                    "refusing to extract unsafe path: %s" % member.name)
        tf.extractall(dest_dir)
    # restore perms + verify every file against the sealed manifest
    seen = set()
    for root, dirs, names in os.walk(dest_dir):
        for d in dirs:
            os.chmod(os.path.join(root, d), 0o700)
        for name in names:
            full = os.path.join(root, name)
            if os.path.islink(full):
                raise RestoreError(
                    "refusing: symlink appeared during extract: %s" % full)
            os.chmod(full, 0o600)
            rel = os.path.relpath(full, dest_dir).replace(os.sep, "/")
            entry = files.get(rel)
            if entry is None:
                raise RestoreError(
                    "extracted file not in sealed manifest: %s" % rel)
            if vault_manifest.hash_file(full) != entry["sha256"]:
                raise RestoreError(
                    "hash mismatch on restored file: %s" % rel)
            seen.add(rel)
    if set(files) != seen:
        raise RestoreError(
            "restored tree does not match sealed manifest "
            "(manifest %d files, disk %d)" % (len(files), len(seen)))
    return len(seen)


def restore_backup(backup_file, dest_dir, passphrase_file=None,
                   keyfile=None, confirm=None):
    """Restore a backup file into dest_dir.

    Dry-run unless confirm == basename(dest_dir). The secret is fully
    verified (HMAC + tarball hash + per-file inventory) before any
    byte is written to the destination. Returns a dict with the plan
    or the restore certificate."""
    plan = plan_restore(backup_file, dest_dir, passphrase_file, keyfile)
    base = os.path.basename(plan["dest_dir"].rstrip(os.sep))
    if confirm != base:
        return plan
    kind, secret = _locked_call(vault_lock._read_secret,
                                passphrase_file, keyfile)
    try:
        if plan["container"] == chunkseal.FORMAT:
            header, payload = _chunk_call(chunkseal.open_file,
                                          plan["backup_file"],
                                          (kind, secret))
            # every chunk's HMAC was re-verified on open; now the
            # tarball and the per-file inventory, before any write
            _chunk_call(_backup._verify_tarball_bytes, header, payload)
            n = _extract_verified(payload, plan["dest_dir"],
                                  header.get("files") or {})
        else:
            # openssl format: unlock verifies HMAC + tarball hash, then
            # extracts path-safe with per-file manifest verification
            r = _locked_call(vault_lock.vault_unlock, plan["backup_file"],
                             plan["dest_dir"],
                             passphrase_file=passphrase_file,
                             keyfile=keyfile)
            n = r["files"]
    finally:
        del secret
    cert = {
        "cert_id": "rst-" + secrets.token_hex(6),
        "backup_file": plan["backup_file"],
        "dest_dir": plan["dest_dir"],
        "file_count": n,
        "total_bytes": plan["total_bytes"],
        "container": plan["container"],
        "restored_at": _ts(),
    }
    return {"dry_run": False, "dest_dir": plan["dest_dir"],
            "certificate": cert}


if __name__ == "__main__":
    import argparse
    import json

    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    def _secret_args(p):
        g = p.add_mutually_exclusive_group(required=True)
        g.add_argument("--passphrase-file")
        g.add_argument("--keyfile")
        return p

    p = _secret_args(sub.add_parser("restore"))
    p.add_argument("file", help="the .castle backup file to restore")
    p.add_argument("--to", dest="dest", required=True,
                   help="empty (or new) directory to restore into")
    p.add_argument("--yes", default=None,
                   help="typed confirmation: the destination's basename")

    a = ap.parse_args()
    try:
        r = restore_backup(a.file, a.dest,
                           passphrase_file=a.passphrase_file,
                           keyfile=a.keyfile, confirm=a.yes)
        if r["dry_run"]:
            print("DRY RUN — nothing written. Would restore:")
            print("  backup:    %s" % r["backup_file"])
            print("  container: %s" % r["container"])
            print("  files:     %d (%d bytes, sealed %s)"
                  % (r["file_count"], r["total_bytes"], r["sealed_at"]))
            print("  dest:      %s" % r["dest_dir"])
            print(r["hint"])
            raise SystemExit(2)
        c = r["certificate"]
        print("RESTORED %d file(s) -> %s (cert %s, all hashes verified)"
              % (c["file_count"], c["dest_dir"], c["cert_id"]))
    except RestoreError as e:
        print("refused: %s" % e)
        raise SystemExit(1)
