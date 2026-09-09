#!/usr/bin/env python3
"""Regression tests for vault/backup.py list_backups (inventory lane).

Fixture-based, temp dirs only. list_backups needs no secret and writes
nothing; these tests pin down what a listing claims, what it refuses,
and that it never overstates proof (every entry carries verified=False
until backup-verify does its job).
"""

import hashlib
import json
import os
import shutil
import sys
import tempfile
import unittest

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)

import backup  # noqa: E402
import chunkseal  # noqa: E402
import vault_lock  # noqa: E402


class ListBackupsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="castle-list-")
        self.target = os.path.join(self.tmp, "backups")
        os.makedirs(self.target, mode=0o700)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_vault_format(self, name, payload=b"x" * 64):
        """A minimal castle-vault/v1 sealed file (raw keyfile path)."""
        src = os.path.join(self.tmp, "src-v")
        os.makedirs(src, mode=0o700)
        with open(os.path.join(src, "note.txt"), "wb") as f:
            f.write(payload)
        key = os.urandom(32)
        header, ct = vault_lock._seal_plaintext(src, "keyfile", key)
        vault_lock._write_locked(os.path.join(self.target, name), header, ct)
        return header

    def _write_chunks_format(self, name, payload=b"y" * 128):
        """A minimal castle-chunks/v1 sealed file."""
        secret = ("passphrase", b"pw:" + os.urandom(8))
        header, body = chunkseal.seal(payload, secret, files={
            "a.txt": {"size": len(payload), "sha256": "00" * 32}})
        with open(os.path.join(self.target, name), "wb") as f:
            f.write((json.dumps(header, sort_keys=True) + "\n").encode())
            f.write(body)
        return header

    def test_lists_both_formats(self):
        self._write_vault_format("mom-20260909T160000Z.castle")
        self._write_chunks_format("sam-20260909T160100Z.castle")
        entries = backup.list_backups(self.target)
        self.assertEqual(len(entries), 2)
        by_user = {e["user"]: e for e in entries}
        self.assertEqual(by_user["mom"]["container"], "castle-vault/v1")
        self.assertEqual(by_user["mom"]["cipher"], "aes-256-cbc")
        self.assertEqual(by_user["sam"]["container"], "castle-chunks/v1")
        for e in entries:
            self.assertFalse(e["verified"])
            self.assertIn("backup-verify", e["note"])
            self.assertGreater(e["size_bytes"], 0)
            self.assertEqual(len(e["file_sha256"]), 64)
            # the fingerprint is the real file hash
            with open(os.path.join(self.target, e["file"]), "rb") as f:
                self.assertEqual(hashlib.sha256(f.read()).hexdigest(),
                                 e["file_sha256"])

    def test_counts_are_header_claims(self):
        self._write_vault_format("mom-20260909T160000Z.castle",
                                 payload=b"z" * 100)
        (e,) = backup.list_backups(self.target)
        self.assertEqual(e["file_count"], 1)
        self.assertEqual(e["total_bytes"], 100)

    def test_kdf_normalized_from_both_header_shapes(self):
        self._write_vault_format("mom-20260909T160000Z.castle")
        self._write_chunks_format("sam-20260909T160100Z.castle")
        by_user = {e["user"]: e for e in backup.list_backups(self.target)}
        # castle-vault/v1 stores kdf as a plain string
        self.assertEqual(by_user["mom"]["kdf"], "raw-keyfile")
        # castle-chunks/v1 stores kdf as a params dict
        self.assertIn(by_user["sam"]["kdf"], ("scrypt", "pbkdf2-sha256"))

    def test_user_with_dashes_parses(self):
        self._write_chunks_format("big-bro-sam-20260909T160100Z-2.castle")
        (e,) = backup.list_backups(self.target)
        self.assertEqual(e["user"], "big-bro-sam")

    def test_skips_garbage_and_ignores_non_castle(self):
        with open(os.path.join(self.target, "junk-20260909T160000Z.castle"),
                  "wb") as f:
            f.write(b"this is not a sealed file at all\n")
        with open(os.path.join(self.target, "notes.txt"), "w") as f:
            f.write("not a backup")
        self._write_vault_format("mom-20260909T160000Z.castle")
        entries = backup.list_backups(self.target)
        self.assertEqual(len(entries), 2)  # the .txt is ignored
        junk = [e for e in entries if e["file"].startswith("junk")][0]
        self.assertIn("skipped", junk["status"])

    def test_unrecognized_format_listed_not_crashed(self):
        header = {"format": "castle-future/v9", "created": "2030-01-01T00:00:00Z"}
        with open(os.path.join(self.target, "x-20260909T160000Z.castle"),
                  "wb") as f:
            f.write((json.dumps(header) + "\n").encode())
            f.write(b"payload")
        (e,) = backup.list_backups(self.target)
        self.assertIn("unrecognized format", e["status"])
        self.assertEqual(e["container"], "castle-future/v9")

    def test_empty_target(self):
        self.assertEqual(backup.list_backups(self.target), [])

    def test_refuses_symlink_and_missing_target(self):
        link = os.path.join(self.tmp, "link")
        os.symlink(self.target, link)
        with self.assertRaises(backup.BackupError):
            backup.list_backups(link)
        with self.assertRaises(backup.BackupError):
            backup.list_backups(os.path.join(self.tmp, "nope"))
        with self.assertRaises(backup.BackupError):
            backup.list_backups("")

    def test_symlinked_entry_skipped(self):
        self._write_vault_format("mom-20260909T160000Z.castle")
        os.symlink(os.path.join(self.target, "mom-20260909T160000Z.castle"),
                   os.path.join(self.target, "evil-20260909T160000Z.castle"))
        entries = backup.list_backups(self.target)
        self.assertEqual(len(entries), 2)
        evil = [e for e in entries if e["file"].startswith("evil")][0]
        self.assertIn("skipped", evil["status"])

    def test_listing_writes_nothing(self):
        self._write_vault_format("mom-20260909T160000Z.castle")
        before = set(os.listdir(self.target))
        backup.list_backups(self.target)
        self.assertEqual(set(os.listdir(self.target)), before)


if __name__ == "__main__":
    unittest.main(verbosity=2)
