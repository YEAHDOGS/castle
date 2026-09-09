#!/usr/bin/env python3
"""Regression tests for vault/backup.py — FAMILY-DATA-VAULT.md step 8.

Fixture-based, temp dirs only. Every test runs against a throwaway
vault root and a throwaway backup target; nothing touches the real
filesystem layout.
"""

import hashlib
import io
import json
import os
import shutil
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault  # noqa: E402
import backup as vault_backup  # noqa: E402


def fresh_dir(tc, prefix):
    tmp = tempfile.mkdtemp(prefix=prefix)
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_root(tc):
    root = fresh_dir(tc, "castle-backup-test-root-")
    vault.create_user(root, "sam", tier="adult")
    vault.create_user(root, "sally", tier="child", guardian="sam")
    return root


def mk_passphrase(tc, text="test-passphrase"):
    fd, path = tempfile.mkstemp(prefix="castle-backup-pw-")
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


def activity_lines(root):
    with open(os.path.join(root, vault.ACTIVITY_FILE),
              encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


class BackupPlanTest(unittest.TestCase):
    def test_dry_run_default_writes_nothing(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        put(root, "sam", "receipts/r1.txt", b"receipt one")
        pw = mk_passphrase(self)
        plan = vault_backup.create_backup(root, "sam", target,
                                           passphrase_file=pw)
        self.assertTrue(plan["dry_run"])
        self.assertEqual(os.listdir(target), [])
        self.assertEqual(plan["file_count"], 1)
        self.assertIn("would_write", plan)

    def test_unknown_user_is_refusal(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(root, "nobody", target,
                                        passphrase_file=mk_passphrase(self))

    def test_target_inside_vault_refused(self):
        root = mk_root(self)
        pw = mk_passphrase(self)
        inside = os.path.join(root, "vaults", "sam", "backups-here")
        os.makedirs(inside)
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(root, "sam", inside,
                                        passphrase_file=pw)

    def test_target_containing_vault_refused(self):
        root = mk_root(self)
        pw = mk_passphrase(self)
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(root, "sam", root,
                                        passphrase_file=pw)

    def test_missing_target_refused(self):
        root = mk_root(self)
        pw = mk_passphrase(self)
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(
                root, "sam",
                os.path.join(root, "no-such-target"),
                passphrase_file=pw)

    def test_secret_inside_vault_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        pw = put(root, "sam", "pw.txt", b"key-material-adjacent")
        os.chmod(pw, 0o600)
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(root, "sam", target,
                                        passphrase_file=pw)

    def test_secret_inside_target_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        pw = os.path.join(target, "pw.txt")
        with open(pw, "wb") as f:
            f.write(b"passphrase")
        os.chmod(pw, 0o600)
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(root, "sam", target,
                                        passphrase_file=pw)

    def test_both_secrets_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(root, "sam", target,
                                        passphrase_file=mk_passphrase(self),
                                        keyfile=mk_passphrase(self))

    def test_no_secret_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.create_backup(root, "sam", target)


class BackupWriteTest(unittest.TestCase):
    def _backed_up(self, root=None, user="sam", kind="passphrase"):
        root = root or mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        put(root, user, "receipts/r1.txt", b"receipt one")
        put(root, user, "documents/d1.txt", b"doc one")
        kw = {}
        if kind == "passphrase":
            kw["passphrase_file"] = mk_passphrase(self)
        else:
            fd, kf = tempfile.mkstemp(prefix="castle-backup-key-")
            self.addCleanup(os.unlink, kf)
            os.write(fd, os.urandom(32))
            os.close(fd)
            os.chmod(kf, 0o600)
            kw["keyfile"] = kf
        r = vault_backup.create_backup(root, user, target, confirm=user,
                                        **kw)
        self.assertFalse(r["dry_run"])
        return root, target, r, kw

    def test_backup_writes_encrypted_file_and_cert(self):
        root, target, r, kw = self._backed_up()
        files = os.listdir(target)
        self.assertEqual(len(files), 1)
        self.assertTrue(files[0].endswith(".castle"))
        self.assertTrue(files[0].startswith("sam-"))
        out = os.path.join(target, files[0])
        self.assertEqual(stat.S_IMODE(os.stat(out).st_mode), 0o600)
        cert = r["certificate"]
        self.assertEqual(cert["file_count"], 2)
        self.assertTrue(cert["cert_id"].startswith("bkp-"))
        # the backup is NOT plaintext: the receipt bytes appear nowhere
        with open(out, "rb") as f:
            blob = f.read()
        self.assertNotIn(b"receipt one", blob)
        with open(out, "rb") as f:
            self.assertEqual(
                hashlib.sha256(f.read()).hexdigest(),
                cert["file_sha256"])

    def test_backup_never_burns_live_vault(self):
        root, target, r, kw = self._backed_up()
        live = os.path.join(root, "vaults", "sam", "receipts", "r1.txt")
        with open(live, "rb") as f:
            self.assertEqual(f.read(), b"receipt one")

    def test_backup_is_activity_logged_metadata_only(self):
        root, target, r, kw = self._backed_up()
        acts = [a for a in activity_lines(root) if a["action"] == "backup"]
        self.assertEqual(len(acts), 1)
        detail = acts[0]["detail"]
        self.assertNotIn("receipt one", detail)
        self.assertIn(r["certificate"]["cert_id"], detail)

    def test_second_backup_never_overwrites_first(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        put(root, "sam", "receipts/r1.txt", b"v1")
        pw = mk_passphrase(self)
        r1 = vault_backup.create_backup(root, "sam", target,
                                         passphrase_file=pw, confirm="sam")
        put(root, "sam", "receipts/r2.txt", b"v2")
        r2 = vault_backup.create_backup(root, "sam", target,
                                         passphrase_file=pw, confirm="sam")
        self.assertNotEqual(r1["backup_file"], r2["backup_file"])
        self.assertEqual(len(os.listdir(target)), 2)
        with open(r1["backup_file"], "rb") as f:
            self.assertNotIn(b"v2", f.read())

    def test_keyfile_mode_works(self):
        root, target, r, kw = self._backed_up(kind="keyfile")
        self.assertEqual(r["certificate"]["file_count"], 2)
        rep = vault_backup.verify_backup(r["backup_file"], **kw)
        self.assertTrue(rep["ok"])

    def test_child_user_can_be_backed_up(self):
        root, target, r, kw = self._backed_up(user="sally")
        self.assertTrue(r["certificate"]["cert_id"].startswith("bkp-"))

    def test_typed_confirmation_mismatch_is_dry_run(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        put(root, "sam", "receipts/r1.txt", b"x")
        r = vault_backup.create_backup(root, "sam", target,
                                        passphrase_file=mk_passphrase(self),
                                        confirm="SAM")
        self.assertTrue(r["dry_run"])
        self.assertEqual(os.listdir(target), [])


class VerifyBackupTest(unittest.TestCase):
    def _sealed(self, passphrase="verify-pass"):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        put(root, "sam", "receipts/r1.txt", b"receipt one")
        pw = mk_passphrase(self, passphrase)
        r = vault_backup.create_backup(root, "sam", target,
                                        passphrase_file=pw, confirm="sam")
        return r["backup_file"], pw

    def test_verify_proves_bit_exact_restore(self):
        path, pw = self._sealed()
        rep = vault_backup.verify_backup(path, passphrase_file=pw)
        self.assertTrue(rep["ok"])
        self.assertEqual(rep["file_count"], 1)
        self.assertEqual(rep["total_bytes"], len(b"receipt one"))

    def test_verify_wrong_passphrase_fails_closed(self):
        path, pw = self._sealed()
        bad = mk_passphrase(self, "wrong-passphrase")
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.verify_backup(path, passphrase_file=bad)

    def test_verify_tampered_file_fails_closed(self):
        path, pw = self._sealed()
        with open(path, "r+b") as f:
            f.seek(0, 2)
            size = f.tell()
            f.seek(size - 10)
            b = f.read(1)
            f.seek(size - 10)
            f.write(bytes([b[0] ^ 0xFF]))
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.verify_backup(path, passphrase_file=pw)

    def test_verify_not_a_castle_file_refused(self):
        path, pw = self._sealed()
        junk = path + ".junk"
        shutil.copy(path, junk)
        with open(junk, "wb") as f:
            f.write(b"not a vault file at all")
        with self.assertRaises(vault_backup.BackupError):
            vault_backup.verify_backup(junk, passphrase_file=pw)

    def test_restore_roundtrip_matches_live_bytes(self):
        # full loop: backup, then unlock into a fresh dir — the bytes
        # that come back must equal the bytes that went in
        import vault_lock as vl
        path, pw = self._sealed()
        outdir = fresh_dir(self, "castle-backup-test-restore-")
        r = vl.vault_unlock(path, os.path.join(outdir, "restored"),
                            passphrase_file=pw)
        self.assertEqual(r["files"], 1)
        with open(os.path.join(outdir, "restored", "receipts",
                               "r1.txt"), "rb") as f:
            self.assertEqual(f.read(), b"receipt one")


class BackupCLITest(unittest.TestCase):
    def test_cli_dry_run_exit_2_writes_nothing(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        put(root, "sam", "receipts/r1.txt", b"x")
        pw = mk_passphrase(self)
        buf = io.StringIO()
        with redirect_stdout(buf):
            code = vault.main(["--dir", root, "backup", "sam",
                               "--target-dir", target,
                               "--passphrase-file", pw])
        self.assertEqual(code, 2)
        self.assertEqual(os.listdir(target), [])
        self.assertIn("DRY RUN", buf.getvalue())

    def test_cli_backup_then_verify(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-backup-test-target-")
        put(root, "sam", "receipts/r1.txt", b"x")
        pw = mk_passphrase(self)
        with redirect_stdout(io.StringIO()):
            code = vault.main(["--dir", root, "backup", "sam",
                               "--target-dir", target,
                               "--passphrase-file", pw, "--yes", "sam"])
        self.assertEqual(code, 0)
        files = os.listdir(target)
        self.assertEqual(len(files), 1)
        buf = io.StringIO()
        with redirect_stdout(buf):
            code = vault.main(["--dir", root, "backup-verify",
                               os.path.join(target, files[0]),
                               "--passphrase-file", pw])
        self.assertEqual(code, 0)
        self.assertIn("VERIFIED", buf.getvalue())


if __name__ == "__main__":
    unittest.main()
