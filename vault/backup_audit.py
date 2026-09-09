#!/usr/bin/env python3
"""Sealed-backup integrity verification — FAMILY-DATA-VAULT.md step 8 audit lane.

``backup-audit`` reads the backup-list inventory of a backup target
(local dir or mounted share — the 10TB SMB mount path + credentials
are still pending from Brando; pass the *mount point* as
``--target-dir``, or set ``CASTLE_BACKUP_TARGET``), recomputes each
sealed archive's SHA-256, and reports per archive:

    OK      fingerprint matches the ledger baseline (and, when a
            secret is given, the archive re-proves bit-exact via
            backup-verify)
    NEW     present but no ledger baseline yet (first sighting —
            information, not failure)
    CORRUPT fingerprint differs from the ledger baseline, OR deep
            proof with the secret failed (tamper / wrong key)
    MISSING ledger has a baseline but the file is gone from the target

The ledger (``castle-audit-ledger.json`` in the target dir, or
``--ledger FILE``) is metadata only — filenames, SHA-256s, sizes,
timestamps; never secrets, never archive bytes — and is written 0600
like every other Castle artifact. Baselines are recorded for OK and
NEW archives; a CORRUPT baseline is never overwritten (corruption
must not become the new normal); MISSING records are kept so the
file stays reported until the operator re-baselines it.

Exit codes (CLI): 0 = all OK/NEW, 1 = refusal (bad target, corrupt
ledger, secret-arg error), 2 = any CORRUPT or MISSING archive.

No outbound network, no secrets logged, read-only against the
archives themselves.
"""

import hashlib
import json
import os
import time

_LEDGER_FORMAT = "castle-audit-ledger/v1"
_DEFAULT_LEDGER_NAME = "castle-audit-ledger.json"
_TARGET_ENV = "CASTLE_BACKUP_TARGET"

STATUS_OK = "OK"
STATUS_NEW = "NEW"
STATUS_CORRUPT = "CORRUPT"
STATUS_MISSING = "MISSING"
STATUS_SKIPPED = "SKIPPED"


class AuditError(Exception):
    """Refusal — bad input, bad ledger, or a broken secret contract."""


def _utc_now():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1048576), b""):
            h.update(chunk)
    return h.hexdigest()


def _load_ledger(ledger_path):
    """Read and validate the audit ledger. Returns {} when the file
    does not exist yet (first run). Corrupt ledgers are refused, never
    trusted."""
    if not os.path.exists(ledger_path):
        return {}
    if os.path.islink(ledger_path) or not os.path.isfile(ledger_path):
        raise AuditError("ledger is not a regular file: %s" % ledger_path)
    try:
        with open(ledger_path, "rb") as f:
            doc = json.loads(f.read().decode("utf-8"))
    except (ValueError, UnicodeDecodeError):
        raise AuditError("ledger is not valid JSON: %s" % ledger_path)
    if not isinstance(doc, dict) or doc.get("format") != _LEDGER_FORMAT:
        raise AuditError("ledger has unknown format id: %s" % ledger_path)
    entries = doc.get("entries")
    if not isinstance(entries, dict):
        raise AuditError("ledger has no entries map: %s" % ledger_path)
    clean = {}
    for name, rec in entries.items():
        if (isinstance(rec, dict)
                and isinstance(rec.get("sha256"), str)
                and len(rec["sha256"]) == 64):
            clean[name] = rec
    return clean


def _save_ledger(ledger_path, baseline, target_dir):
    doc = {
        "format": _LEDGER_FORMAT,
        "target": os.path.realpath(target_dir),
        "updated": _utc_now(),
        "note": ("SHA-256 baselines of sealed .castle archives. "
                 "Metadata only — no secrets, no archive bytes."),
        "entries": baseline,
    }
    d = os.path.dirname(os.path.abspath(ledger_path)) or "."
    if not os.path.isdir(d):
        raise AuditError("ledger directory does not exist: %s" % d)
    tmp = ledger_path + ".tmp-%d" % os.getpid()
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=2, sort_keys=True)
        f.write("\n")
    os.chmod(tmp, 0o600)
    os.replace(tmp, ledger_path)


def _deep_prove(path, passphrase_file, keyfile):
    """Re-prove one sealed archive bit-exact with the secret.
    Returns (ok: bool, detail: str)."""
    import backup as _bk
    try:
        r = _bk.verify_backup(path, passphrase_file=passphrase_file,
                              keyfile=keyfile)
    except _bk.BackupError as e:
        return False, "backup-verify failed: %s" % e
    return True, ("backup-verify bit-exact: %d file(s), %d bytes"
                  % (r["file_count"], r["total_bytes"]))


def audit_backups(target_dir, passphrase_file=None, keyfile=None,
                  ledger_path=None):
    """Run a sealed-backup integrity audit of ``target_dir``.

    Returns a report dict::

        {"target": ..., "ledger": ..., "archives": [ ... ],
         "summary": {"ok": n, "new": n, "corrupt": n, "missing": n},
         "failures": n}

    Each archive record has ``file``, ``status``, ``sha256`` (when the
    file is present), and ``detail``. ``failures`` is the CORRUPT +
    MISSING count — the CLI maps it to exit 2. Raises AuditError on
    refusals (bad target dir, bad ledger, bad secret args).
    """
    if not target_dir:
        target_dir = os.environ.get(_TARGET_ENV)
    if not isinstance(target_dir, str) or not target_dir:
        raise AuditError("target dir required (--target-dir or %s)"
                         % _TARGET_ENV)
    if os.path.islink(target_dir):
        raise AuditError("backup target is a symlink — refusing")
    if not os.path.isdir(target_dir):
        raise AuditError("backup target is not a directory: %s"
                         % target_dir)
    if passphrase_file is not None and keyfile is not None:
        raise AuditError("passphrase-file and keyfile are mutually "
                         "exclusive")
    deep = passphrase_file is not None or keyfile is not None
    if ledger_path is None:
        ledger_path = os.path.join(target_dir, _DEFAULT_LEDGER_NAME)
    elif os.path.islink(ledger_path):
        raise AuditError("ledger is a symlink — refusing")

    import backup as _bk
    inventory = _bk.list_backups(target_dir)  # raises BackupError on refusal

    baseline = _load_ledger(ledger_path)
    now = _utc_now()
    archives = []
    corrupt = missing = ok = new = 0
    new_baseline = dict(baseline)

    present = set()
    for entry in inventory:
        name = entry.get("file")
        if entry.get("status"):  # skipped / unrecognized — not an archive
            archives.append({"file": name, "status": STATUS_SKIPPED,
                             "sha256": None,
                             "detail": entry["status"]})
            continue
        present.add(name)
        path = os.path.join(target_dir, name)
        try:
            digest = _sha256_file(path)
        except OSError as e:
            corrupt += 1
            archives.append({"file": name, "status": STATUS_CORRUPT,
                             "sha256": None,
                             "detail": "unreadable: %s" % e})
            continue
        rec = baseline.get(name)
        if rec is None:
            # Deep lane: never trust a first sighting blindly — prove it
            # with the secret before recording it as the baseline.
            if deep:
                good, deep_detail = _deep_prove(path, passphrase_file,
                                               keyfile)
                if not good:
                    corrupt += 1
                    archives.append({"file": name, "status": STATUS_CORRUPT,
                                     "sha256": digest,
                                     "detail": deep_detail})
                    continue
            new += 1
            status, detail = STATUS_NEW, (
                "first sighting — baseline recorded (%d bytes)"
                % entry.get("size_bytes", 0))
            new_baseline[name] = {"sha256": digest,
                                  "size_bytes": entry.get("size_bytes"),
                                  "first_seen": now, "last_ok": now}
        elif rec["sha256"] != digest:
            corrupt += 1
            status = STATUS_CORRUPT
            detail = ("sha256 mismatch: baseline %s…, now %s…"
                      % (rec["sha256"][:12], digest[:12]))
            archives.append({"file": name, "status": status,
                             "sha256": digest, "detail": detail})
            continue  # never overwrite a CORRUPT baseline
        else:
            if deep:
                good, deep_detail = _deep_prove(path, passphrase_file,
                                               keyfile)
                if not good:
                    corrupt += 1
                    archives.append({"file": name, "status": STATUS_CORRUPT,
                                     "sha256": digest,
                                     "detail": deep_detail})
                    continue
                detail = "sha256 match; " + deep_detail
            else:
                detail = "sha256 matches ledger baseline"
            ok += 1
            status = STATUS_OK
            new_baseline[name] = dict(rec)
            new_baseline[name]["last_ok"] = now
            new_baseline[name]["size_bytes"] = entry.get("size_bytes")
        archives.append({"file": name, "status": status,
                         "sha256": digest, "detail": detail})

    for name in sorted(set(baseline) - present):
        missing += 1
        archives.append({"file": name, "status": STATUS_MISSING,
                         "sha256": baseline[name]["sha256"],
                         "detail": "in ledger, file gone from target"})

    _save_ledger(ledger_path, new_baseline, target_dir)
    failures = corrupt + missing
    return {
        "target": os.path.realpath(target_dir),
        "ledger": os.path.realpath(ledger_path),
        "deep": deep,
        "archives": sorted(archives, key=lambda a: a["file"]),
        "summary": {"ok": ok, "new": new,
                    "corrupt": corrupt, "missing": missing,
                    "failures": failures},
        "failures": failures,
    }


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--target-dir", default=os.environ.get(_TARGET_ENV),
                    help="backup target directory (or %s)" % _TARGET_ENV)
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--passphrase-file")
    g.add_argument("--keyfile")
    ap.add_argument("--ledger",
                    help="audit ledger path (default: "
                         "<target>/%s)" % _DEFAULT_LEDGER_NAME)
    args = ap.parse_args()
    try:
        report = audit_backups(args.target_dir,
                               passphrase_file=args.passphrase_file,
                               keyfile=args.keyfile,
                               ledger_path=args.ledger)
    except AuditError as e:
        print("refused: %s" % e)
        raise SystemExit(1)
    for a in report["archives"]:
        print("%-8s %s (%s)" % (a["status"], a["file"], a["detail"]))
    s = report["summary"]
    print("audit %s: ok=%d new=%d corrupt=%d missing=%d%s" % (
        report["target"], s["ok"], s["new"], s["corrupt"], s["missing"],
        ", deep-verify ON" if report["deep"] else ""))
    raise SystemExit(2 if report["failures"] else 0)
