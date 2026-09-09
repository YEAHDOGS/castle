#!/usr/bin/env python3
"""Regression tests for vault/backup_audit.py — the backup-audit lane.

Fixture-based, temp dirs only. Small synthetic archives are sealed
with the real create_backup pipeline (chunked lane for speed, one
openssl lane for container coverage); then:
  - first audit marks everything NEW and records baselines,
  - a second audit reports OK,
  - one flipped byte in the payload is detected (CORRUPT),
  - deleting an archive reports MISSING (and keeps reporting it),
  - deep verification with the secret catches tamper the baseline
    also catches, and wrong-key reads as CORRUPT,
  - refusals: symlinked/missing target, corrupt ledger, dual secrets.
"""

import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault  # noqa: E402
import backup as vault_backup  # noqa: E402
import backup_audit  # noqa: E402


def fresh_dir(tc, prefix):
    tmp = tempfile.mkdtemp(prefix=prefix)
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_root(tc):
    root = fresh_dir(tc, "castle-audit-test-root-")
    vault.create_user(root, "sam", tier="adult")
    vault.create_user(root, "mom", tier="adult")
    return root


def mk_passphrase(tc, text="test-passphrase"):
    fd, path = tempfile.mkstemp(prefix="castle-audit-pw-")
    tc.addCleanup(os.unlink, path)
    os.write(fd, text.encode())
    os.close(fd)
    os.chmod(path, 0o600)
    return path


def put(root, user, rel, data=b"data"):
    full = os.path.join(root, "vaults", user, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "wb") as f:
        f.write(data)
    return full


def seal(tc, root, user, target, pw, chunks=True):
    put(root, user, "documents/note.txt",
        b"audit fixture for %s" % user.encode())
    r = vault_backup.create_backup(root, user, target,
                                   passphrase_file=pw, confirm=user,
                                   chunks=chunks)
    return r["backup_file"]


def archive_names(target):
    return sorted(n for n in os.listdir(target) if n.endswith(".castle"))


def flip_payload_byte(path, seed=0):
    """Flip one byte in the sealed payload (after the header line).
    ``seed`` moves the flip so a second flip isn't just an undo."""
    with open(path, "rb") as f:
        data = bytearray(f.read())
    nl = data.index(b"\n")
    off = nl + 17 + seed  # inside the sealed body, never the header JSON
    data[off] ^= 0xFF
    with open(path, "wb") as f:
        f.write(data)
    return off


def ledger_entries(ledger_path):
    with open(ledger_path, encoding="utf-8") as f:
        return json.load(f)["entries"]


class BackupAuditTest(unittest.TestCase):
    def test_first_audit_marks_all_new_and_writes_ledger(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        seal(self, root, "mom", target, pw)
        report = backup_audit.audit_backups(target)
        self.assertEqual(report["failures"], 0)
        self.assertEqual([a["status"] for a in report["archives"]],
                         ["NEW", "NEW"])
        self.assertEqual(report["summary"]["new"], 2)
        ledger = os.path.join(target, "castle-audit-ledger.json")
        self.assertTrue(os.path.isfile(ledger))
        self.assertEqual(stat.S_IMODE(os.stat(ledger).st_mode), 0o600)
        entries = ledger_entries(ledger)
        self.assertEqual(set(entries), set(archive_names(target)))
        for name in archive_names(target):
            with open(os.path.join(target, name), "rb") as f:
                want = hashlib.sha256(f.read()).hexdigest()
            self.assertEqual(entries[name]["sha256"], want)

    def test_second_audit_reports_ok(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        backup_audit.audit_backups(target)  # baseline
        report = backup_audit.audit_backups(target)
        self.assertEqual(report["failures"], 0)
        self.assertEqual([a["status"] for a in report["archives"]],
                         ["OK"])
        self.assertEqual(report["summary"]["ok"], 1)

    def test_corrupt_byte_detected_without_secret(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        seal(self, root, "mom", target, pw)
        backup_audit.audit_backups(target)  # baseline
        flip_payload_byte(os.path.join(target, archive_names(target)[0]))
        report = backup_audit.audit_backups(target)
        statuses = {a["file"]: a["status"] for a in report["archives"]}
        self.assertEqual(statuses[archive_names(target)[0]], "CORRUPT")
        self.assertEqual(statuses[archive_names(target)[1]], "OK")
        self.assertEqual(report["failures"], 1)
        self.assertEqual(report["summary"]["corrupt"], 1)

    def test_corrupt_baseline_never_overwritten(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        name = archive_names(target)[0]
        backup_audit.audit_backups(target)
        ledger = os.path.join(target, "castle-audit-ledger.json")
        good = ledger_entries(ledger)[name]["sha256"]
        flip_payload_byte(os.path.join(target, name))
        backup_audit.audit_backups(target)
        # a second corruption must not become the new baseline
        flip_payload_byte(os.path.join(target, name), seed=64)
        report = backup_audit.audit_backups(target)
        self.assertEqual(report["failures"], 1)
        self.assertEqual(ledger_entries(ledger)[name]["sha256"], good)

    def test_deleted_archive_reports_missing_and_stays_missing(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        seal(self, root, "mom", target, pw)
        name = archive_names(target)[0]
        backup_audit.audit_backups(target)  # baseline
        os.unlink(os.path.join(target, name))
        for _ in range(2):
            report = backup_audit.audit_backups(target)
            statuses = {a["file"]: a["status"]
                        for a in report["archives"]}
            self.assertEqual(statuses[name], "MISSING")
            self.assertEqual(report["failures"], 1)

    def test_deep_verify_ok_with_secret(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        backup_audit.audit_backups(target)  # baseline
        report = backup_audit.audit_backups(target, passphrase_file=pw)
        self.assertEqual(report["failures"], 0)
        self.assertTrue(report["deep"])
        self.assertEqual(report["archives"][0]["status"], "OK")
        self.assertIn("bit-exact", report["archives"][0]["detail"])

    def test_deep_verify_detects_tamper_with_secret(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        name = archive_names(target)[0]
        backup_audit.audit_backups(target)  # baseline
        flip_payload_byte(os.path.join(target, name))
        report = backup_audit.audit_backups(target, passphrase_file=pw)
        statuses = {a["file"]: a["status"] for a in report["archives"]}
        self.assertEqual(statuses[name], "CORRUPT")
        self.assertEqual(report["failures"], 1)

    def test_deep_verify_rejects_tampered_first_sighting(self):
        # a corrupt archive must never become the baseline — with a
        # secret, first sightings are proved before they are recorded
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        name = archive_names(target)[0]
        flip_payload_byte(os.path.join(target, name))
        report = backup_audit.audit_backups(target, passphrase_file=pw)
        self.assertEqual(report["archives"][0]["status"], "CORRUPT")
        self.assertIn("backup-verify failed",
                      report["archives"][0]["detail"])
        self.assertEqual(report["failures"], 1)
        ledger = os.path.join(target, "castle-audit-ledger.json")
        self.assertNotIn(name, ledger_entries(ledger))

    def test_deep_verify_wrong_key_is_corrupt(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        wrong = mk_passphrase(self, "wrong-passphrase")
        seal(self, root, "sam", target, pw)
        backup_audit.audit_backups(target)  # baseline
        report = backup_audit.audit_backups(target, passphrase_file=wrong)
        self.assertEqual(report["archives"][0]["status"], "CORRUPT")
        self.assertEqual(report["failures"], 1)

    def test_openssl_container_lane(self):
        # same audit contract on the openssl-backed .castle format
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw, chunks=False)
        r1 = backup_audit.audit_backups(target)
        self.assertEqual(r1["archives"][0]["status"], "NEW")
        r2 = backup_audit.audit_backups(target, passphrase_file=pw)
        self.assertEqual(r2["archives"][0]["status"], "OK")
        flip_payload_byte(os.path.join(target, archive_names(target)[0]))
        r3 = backup_audit.audit_backups(target, passphrase_file=pw)
        self.assertEqual(r3["archives"][0]["status"], "CORRUPT")
        self.assertEqual(r3["failures"], 1)

    def test_skipped_foreign_files_do_not_fail(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        with open(os.path.join(target, "junk-20200101T000000Z.castle"),
                   "wb") as f:
            f.write(b"not a sealed backup at all")
        backup_audit.audit_backups(target)
        report = backup_audit.audit_backups(target)
        statuses = {a["file"]: a["status"] for a in report["archives"]}
        self.assertEqual(statuses["junk-20200101T000000Z.castle"],
                         "SKIPPED")
        self.assertEqual(report["failures"], 0)

    def test_custom_ledger_path(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        ledger = os.path.join(fresh_dir(self, "castle-audit-ledger-"),
                              "audit.json")
        report = backup_audit.audit_backups(target, ledger_path=ledger)
        self.assertEqual(report["ledger"], os.path.realpath(ledger))
        self.assertEqual(report["summary"]["new"], 1)

    def test_target_from_env(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        old = os.environ.get("CASTLE_BACKUP_TARGET")
        os.environ["CASTLE_BACKUP_TARGET"] = target
        self.addCleanup(lambda: os.environ.pop("CASTLE_BACKUP_TARGET", None)
                        if old is None else os.environ.update(
                            {"CASTLE_BACKUP_TARGET": old}))
        report = backup_audit.audit_backups(None)
        self.assertEqual(report["target"], os.path.realpath(target))

    def test_refusals(self):
        pw = mk_passphrase(self)
        with self.assertRaises(backup_audit.AuditError):
            backup_audit.audit_backups("/no/such/dir-anywhere")
        with self.assertRaises(backup_audit.AuditError):
            backup_audit.audit_backups(None)
        target = fresh_dir(self, "castle-audit-test-target-")
        link = os.path.join(fresh_dir(self, "castle-audit-link-"),
                            "target")
        os.symlink(target, link)
        with self.assertRaises(backup_audit.AuditError):
            backup_audit.audit_backups(link)
        ledger = os.path.join(target, "bad.json")
        with open(ledger, "w") as f:
            f.write("{not json")
        with self.assertRaises(backup_audit.AuditError):
            backup_audit.audit_backups(target, ledger_path=ledger)
        with self.assertRaises(backup_audit.AuditError):
            backup_audit.audit_backups(target, passphrase_file=pw,
                                       keyfile=pw)


class BackupAuditCliTest(unittest.TestCase):
    """Pin the CLI contract: human-readable report, exit 0 when clean,
    exit 2 on integrity failure, exit 1 on refusal."""

    def _run(self, *argv):
        return subprocess.run(
            [sys.executable, "vault.py", "backup-audit", *argv],
            cwd=_HERE, capture_output=True, text=True, timeout=120)

    def test_cli_clean_exit_zero(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        p = self._run("--target-dir", target)
        self.assertEqual(p.returncode, 0, p.stderr + p.stdout)
        self.assertIn("NEW", p.stdout)
        p = self._run("--target-dir", target)
        self.assertEqual(p.returncode, 0, p.stderr + p.stdout)
        self.assertIn("OK", p.stdout)

    def test_cli_corrupt_exit_two(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        self._run("--target-dir", target)  # baseline
        flip_payload_byte(os.path.join(target, archive_names(target)[0]))
        p = self._run("--target-dir", target)
        self.assertEqual(p.returncode, 2, p.stderr + p.stdout)
        self.assertIn("CORRUPT", p.stdout)

    def test_cli_missing_exit_two(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-audit-test-target-")
        pw = mk_passphrase(self)
        seal(self, root, "sam", target, pw)
        self._run("--target-dir", target)  # baseline
        os.unlink(os.path.join(target, archive_names(target)[0]))
        p = self._run("--target-dir", target)
        self.assertEqual(p.returncode, 2, p.stderr + p.stdout)
        self.assertIn("MISSING", p.stdout)

    def test_cli_refusal_exit_one(self):
        p = self._run("--target-dir", "/no/such/dir-anywhere")
        self.assertEqual(p.returncode, 1)
        self.assertIn("refused", p.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
