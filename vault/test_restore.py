#!/usr/bin/env python3
"""Regression tests for vault/restore.py — FAMILY-DATA-VAULT.md step 8
(backup restore lane).

Fixture-based, temp dirs only. Every test seals a throwaway vault into
a throwaway backup target and restores into a throwaway destination;
nothing touches real data.
"""

import hashlib
import io
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault  # noqa: E402
import backup as vault_backup  # noqa: E402
import chunkseal  # noqa: E402
import restore as vault_restore  # noqa: E402
import vault_lock  # noqa: E402


def fresh_dir(tc, prefix):
    tmp = tempfile.mkdtemp(prefix=prefix)
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_root(tc):
    root = fresh_dir(tc, "castle-restore-test-root-")
    vault.create_user(root, "sam", tier="adult")
    return root


def mk_passphrase(tc, text="test-passphrase"):
    fd, path = tempfile.mkstemp(prefix="castle-restore-pw-")
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


def sha_tree(top):
    """{relpath: sha256} for every regular file under top."""
    out = {}
    for r, _d, files in os.walk(top):
        for name in files:
            full = os.path.join(r, name)
            h = hashlib.sha256()
            with open(full, "rb") as f:
                for chunk in iter(lambda: f.read(65536), b""):
                    h.update(chunk)
            out[os.path.relpath(full, top).replace(os.sep, "/")] = \
                h.hexdigest()
    return out


def make_backup(tc, root, target, pw, chunks=False):
    put(root, "sam", "receipts/r1.txt", b"receipt one")
    put(root, "sam", "documents/notes/n.txt", b"hello " * 1000)
    put(root, "sam", "photos/deep/a/b/c.bin", bytes(range(256)) * 40)
    r = vault_backup.create_backup(root, "sam", target,
                                   passphrase_file=pw,
                                   confirm="sam", chunks=chunks)
    assert not r["dry_run"]
    return r["backup_file"]


def dest_for(tc, prefix="castle-restore-dest-"):
    """A unique destination path that does not exist yet (restores
    create it). Cleaned up after the test."""
    tmp = tempfile.mkdtemp(prefix=prefix)
    shutil.rmtree(tmp)
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


class RestorePlanTest(unittest.TestCase):
    def test_dry_run_default_writes_nothing(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        dest = dest_for(self)
        plan = vault_restore.restore_backup(bfile, dest,
                                            passphrase_file=pw)
        self.assertTrue(plan["dry_run"])
        self.assertFalse(os.path.exists(dest))
        self.assertEqual(plan["file_count"], 3)
        self.assertIn("hint", plan)

    def test_wrong_confirm_stays_dry_run(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        dest = dest_for(self)
        r = vault_restore.restore_backup(bfile, dest,
                                         passphrase_file=pw,
                                         confirm="definitely-not-the-basename")
        self.assertTrue(r["dry_run"])
        self.assertFalse(os.path.exists(dest))

    def test_nonempty_dest_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        dest = fresh_dir(self, "castle-restore-dest-")
        with open(os.path.join(dest, "existing.txt"), "w") as f:
            f.write("precious")
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.restore_backup(
                bfile, dest, passphrase_file=pw,
                confirm=os.path.basename(dest))

    def test_backup_inside_dest_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        # restoring into the backup's own directory is a corruption trap
        with self.assertRaises(vault_restore.RestoreError) as ctx:
            vault_restore.plan_restore(bfile, target, passphrase_file=pw)
        self.assertIn("inside the restore destination", str(ctx.exception))

    def test_symlink_dest_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        real = fresh_dir(self, "castle-restore-real-")
        link = os.path.join(tempfile.gettempdir(),
                            "castle-restore-link-%d" % os.getpid())
        os.symlink(real, link)
        self.addCleanup(os.unlink, link)
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.plan_restore(bfile, link, passphrase_file=pw)

    def test_root_dest_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.plan_restore(bfile, "/", passphrase_file=pw)

    def test_missing_backup_refused(self):
        root = mk_root(self)
        pw = mk_passphrase(self)
        dest = dest_for(self)
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.plan_restore(os.path.join(tempfile.gettempdir(),
                                                    "no-such-backup.castle"),
                                       dest, passphrase_file=pw)

    def test_secret_required_and_0600(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        dest = dest_for(self)
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.plan_restore(bfile, dest)  # no secret at all
        loose = dest + "-pw"
        with open(loose, "w") as f:
            f.write("x")
        self.addCleanup(os.unlink, loose)
        os.chmod(loose, 0o644)  # world-readable -> refused
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.plan_restore(bfile, dest, passphrase_file=loose)


class RestoreRoundTripTest(unittest.TestCase):
    def _roundtrip(self, chunks):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw, chunks=chunks)
        dest = dest_for(self)
        base = os.path.basename(dest)
        r = vault_restore.restore_backup(bfile, dest,
                                         passphrase_file=pw,
                                         confirm=base)
        self.assertFalse(r["dry_run"])
        cert = r["certificate"]
        self.assertEqual(cert["file_count"], 3)
        # byte-exact: every file matches the original vault tree
        vdir = os.path.join(root, "vaults", "sam")
        self.assertEqual(sha_tree(vdir), sha_tree(dest))
        # perms restored: 0700 dirs, 0600 files
        for r_, dirs, files in os.walk(dest):
            for d in dirs:
                self.assertEqual(stat.S_IMODE(os.stat(
                    os.path.join(r_, d)).st_mode), 0o700)
            for name in files:
                self.assertEqual(stat.S_IMODE(os.stat(
                    os.path.join(r_, name)).st_mode), 0o600)
        return cert

    def test_roundtrip_openssl_format(self):
        cert = self._roundtrip(chunks=False)
        self.assertEqual(cert["container"], "castle-vault/v1")

    def test_roundtrip_chunked_format(self):
        cert = self._roundtrip(chunks=True)
        self.assertEqual(cert["container"], chunkseal.FORMAT)

    def test_restore_into_empty_dir_ok(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        dest = fresh_dir(self, "castle-restore-dest-")  # exists, empty
        r = vault_restore.restore_backup(bfile, dest,
                                         passphrase_file=pw,
                                         confirm=os.path.basename(dest))
        self.assertFalse(r["dry_run"])
        self.assertEqual(r["certificate"]["file_count"], 3)

    def test_wrong_passphrase_refused_nothing_written(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        bad = mk_passphrase(self, text="wrong-passphrase")
        dest = dest_for(self)
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.restore_backup(bfile, dest,
                                         passphrase_file=bad,
                                         confirm=os.path.basename(dest))
        # openssl unlock may have created the dir, but no bytes landed
        self.assertEqual(sha_tree(dest) if os.path.exists(dest) else {}, {})

    def test_tampered_backup_refused_nothing_restored(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw)
        evil = bfile + ".tampered"
        self.addCleanup(os.unlink, evil)
        with open(bfile, "rb") as f:
            raw = bytearray(f.read())
        raw[-10] ^= 0xFF  # flip a ciphertext byte -> HMAC must fail
        with open(evil, "wb") as f:
            f.write(raw)
        dest = dest_for(self)
        with self.assertRaises(vault_restore.RestoreError):
            vault_restore.restore_backup(evil, dest,
                                         passphrase_file=pw,
                                         confirm=os.path.basename(dest))
        self.assertEqual(sha_tree(dest) if os.path.exists(dest) else {}, {})

    def test_chunked_symlink_member_refused(self):
        # hand-craft a chunked backup whose tarball hides a symlink:
        # the extractor must refuse before the link lands on disk.
        pw = mk_passphrase(self)
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w") as tf:
            ti = tarfile.TarInfo("evil-link")
            ti.type = tarfile.SYMTYPE
            ti.linkname = "/etc/passwd"
            tf.addfile(ti)
            ti2 = tarfile.TarInfo("ok.txt")
            data = b"fine"
            ti2.size = len(data)
            tf.addfile(ti2, io.BytesIO(data))
        files = {"ok.txt": {"size": 4, "sha256":
                            hashlib.sha256(b"fine").hexdigest()},
                 "evil-link": {"size": 0, "sha256":
                               hashlib.sha256(b"").hexdigest()}}
        kind, secret = vault_lock._read_secret(pw, None)
        header, body = chunkseal.seal(buf.getvalue(), (kind, secret),
                                      files=files)
        bfile = os.path.join(tempfile.gettempdir(),
                             "castle-restore-evil-%d.castle" % os.getpid())
        self.addCleanup(os.unlink, bfile)
        chunkseal._write_chunked(bfile, header, body)
        dest = dest_for(self)
        with self.assertRaises(vault_restore.RestoreError) as ctx:
            vault_restore.restore_backup(bfile, dest,
                                         passphrase_file=pw,
                                         confirm=os.path.basename(dest))
        # refused either by the tarball inventory scan ("non-regular
        # member") or by the extractor's symlink guard — either way the
        # link must not land on disk and nothing is restored
        msg = str(ctx.exception)
        self.assertTrue("non-regular" in msg or "symlink" in msg, msg)
        self.assertFalse(os.path.islink(os.path.join(dest, "evil-link")))
        self.assertFalse(os.path.exists(os.path.join(dest, "ok.txt")))


class RestoreCLITest(unittest.TestCase):
    def test_cli_dry_run_exit2_then_real_exit0(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-restore-test-target-")
        pw = mk_passphrase(self)
        bfile = make_backup(self, root, target, pw, chunks=True)
        dest = dest_for(self)
        base = os.path.basename(dest)
        dry = subprocess.run(
            [sys.executable, os.path.join(_HERE, "vault.py"), "restore",
             bfile, "--to", dest, "--passphrase-file", pw],
            capture_output=True, text=True, timeout=60)
        self.assertEqual(dry.returncode, 2)
        self.assertIn("DRY RUN", dry.stdout)
        self.assertFalse(os.path.exists(dest))
        real = subprocess.run(
            [sys.executable, os.path.join(_HERE, "vault.py"), "restore",
             bfile, "--to", dest, "--passphrase-file", pw,
             "--yes", base],
            capture_output=True, text=True, timeout=60)
        self.assertEqual(real.returncode, 0, real.stderr)
        self.assertIn("RESTORED 3 file(s)", real.stdout)
        vdir = os.path.join(root, "vaults", "sam")
        self.assertEqual(sha_tree(vdir), sha_tree(dest))


if __name__ == "__main__":
    unittest.main()
