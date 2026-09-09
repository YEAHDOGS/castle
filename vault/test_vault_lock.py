#!/usr/bin/env python3
"""Regression tests for vault/vault_lock.py (FAMILY-DATA-VAULT step 7).

Temp dirs only. Secret fixtures are 0600; no real key material anywhere.
"""

import hashlib
import json
import os
import stat
import sys
import tempfile
import unittest

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault_lock as vl  # noqa: E402
import manifest as vault_manifest  # noqa: E402


def _tree():
    d = tempfile.mkdtemp(prefix="castle-lock-")
    os.makedirs(os.path.join(d, "photos"))
    with open(os.path.join(d, "notes.txt"), "w") as f:
        f.write("family secrets stay home")
    with open(os.path.join(d, "photos", "a.bin"), "wb") as f:
        f.write(os.urandom(4096))
    with open(os.path.join(d, "empty.txt"), "w") as f:
        pass
    return d


def _secret_file(content, mode=0o600):
    fd, p = tempfile.mkstemp(prefix="castle-secret-")
    with os.fdopen(fd, "wb") as f:
        f.write(content)
    os.chmod(p, mode)
    return p


def _tree_hashes(d):
    return {e["path"]: e["sha256"]
            for e in vault_manifest.build_manifest(d)["entries"]}


class TestCipherSelection(unittest.TestCase):
    def test_selects_cbc_hmac_when_gcm_unavailable(self):
        # openssl enc on this machine refuses AEAD — the module must
        # fail closed to AES-256-CBC + HMAC-SHA256, never silently
        # downgrade to something weaker.
        self.assertEqual(vl.select_cipher(),
                         ("aes-256-cbc", "hmac-sha256"))

    def test_header_records_cipher_suite(self):
        d = _tree()
        pf = _secret_file(b"correct horse")
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        h = vl.vault_init(d, out, passphrase_file=pf)
        self.assertEqual(h["cipher"], "aes-256-cbc")
        self.assertEqual(h["mac"], "hmac-sha256")


class TestSecretHygiene(unittest.TestCase):
    def test_keyfile_777_refused_before_crypto(self):
        d = _tree()
        kf = _secret_file(os.urandom(32), mode=0o777)
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_init(d, out, keyfile=kf)
        self.assertFalse(os.path.exists(out))

    def test_passphrase_file_644_refused(self):
        d = _tree()
        pf = _secret_file(b"nope", mode=0o644)
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_init(d, out, passphrase_file=pf)

    def test_keyfile_wrong_size_refused(self):
        d = _tree()
        kf = _secret_file(b"too short")
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_init(d, out, keyfile=kf)

    def test_neither_secret_refused(self):
        d = _tree()
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_init(d, out)

    def test_both_secrets_refused(self):
        d = _tree()
        pf = _secret_file(b"a")
        kf = _secret_file(os.urandom(32))
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_init(d, out, passphrase_file=pf, keyfile=kf)

    def test_secret_inside_tree_refused(self):
        d = _tree()
        pf = _secret_file(b"inside job")
        inner = os.path.join(d, "pw.txt")
        os.rename(pf, inner)
        os.chmod(inner, 0o600)
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_init(d, out, passphrase_file=inner)

    def test_locked_file_is_0600(self):
        d = _tree()
        pf = _secret_file(b"pw")
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        vl.vault_init(d, out, passphrase_file=pf)
        self.assertEqual(stat.S_IMODE(os.stat(out).st_mode), 0o600)

    def test_symlink_in_tree_refused(self):
        d = _tree()
        os.symlink(os.path.join(d, "notes.txt"),
                   os.path.join(d, "evil-link"))
        pf = _secret_file(b"pw")
        out = os.path.join(tempfile.mkdtemp(), "v.castle")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_init(d, out, passphrase_file=pf)


class TestRoundTrip(unittest.TestCase):
    def _roundtrip(self, **secrets):
        d = _tree()
        before = _tree_hashes(d)
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        vl.vault_init(d, locked, **secrets)
        restored = os.path.join(tmp, "restored")
        r = vl.vault_unlock(locked, restored, **secrets)
        self.assertTrue(r["verified"])
        self.assertEqual(_tree_hashes(restored), before)
        return locked

    def test_roundtrip_passphrase_byte_exact(self):
        pf = _secret_file(b"s3cr3t-passphrase")
        self._roundtrip(passphrase_file=pf)

    def test_roundtrip_keyfile_byte_exact(self):
        kf = _secret_file(os.urandom(32))
        self._roundtrip(keyfile=kf)

    def test_wrong_passphrase_fails_closed(self):
        d = _tree()
        pf = _secret_file(b"right")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        vl.vault_init(d, locked, passphrase_file=pf)
        wrong = _secret_file(b"wrong")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_unlock(locked, os.path.join(tmp, "r2"),
                            passphrase_file=wrong)

    def test_wrong_keyfile_fails_closed(self):
        d = _tree()
        kf = _secret_file(os.urandom(32))
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        vl.vault_init(d, locked, keyfile=kf)
        other = _secret_file(os.urandom(32))
        with self.assertRaises(vl.VaultLockError):
            vl.vault_unlock(locked, os.path.join(tmp, "r2"), keyfile=other)

    def test_keyfile_cannot_open_passphrase_vault(self):
        d = _tree()
        pf = _secret_file(b"pw")
        kf = _secret_file(os.urandom(32))
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        vl.vault_init(d, locked, passphrase_file=pf)
        with self.assertRaises(vl.VaultLockError):
            vl.vault_unlock(locked, os.path.join(tmp, "r2"), keyfile=kf)

    def test_unlock_into_nonempty_dir_refused(self):
        d = _tree()
        pf = _secret_file(b"pw")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        vl.vault_init(d, locked, passphrase_file=pf)
        busy = os.path.join(tmp, "busy")
        os.makedirs(busy)
        with open(os.path.join(busy, "x"), "w") as f:
            f.write("occupied")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_unlock(locked, busy, passphrase_file=pf)


class TestTamper(unittest.TestCase):
    def _locked(self):
        d = _tree()
        pf = _secret_file(b"pw")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        vl.vault_init(d, locked, passphrase_file=pf)
        return locked, pf, tmp

    def _flip_byte(self, locked, offset_from_end=10):
        with open(locked, "rb") as f:
            raw = bytearray(f.read())
        raw[-offset_from_end] ^= 0x01
        with open(locked, "wb") as f:
            f.write(bytes(raw))

    def test_tampered_ciphertext_refused(self):
        locked, pf, tmp = self._locked()
        self._flip_byte(locked)
        with self.assertRaises(vl.VaultLockError) as c:
            vl.vault_unlock(locked, os.path.join(tmp, "r"),
                            passphrase_file=pf)
        self.assertIn("HMAC", str(c.exception))

    def test_tampered_header_hmac_refused(self):
        locked, pf, tmp = self._locked()
        with open(locked, "rb") as f:
            raw = f.read()
        header_raw, _, ct = raw.partition(b"\n")
        header = json.loads(header_raw.decode())
        header["hmac"] = "00" * 32
        with open(locked, "wb") as f:
            f.write((json.dumps(header, sort_keys=True) + "\n").encode())
            f.write(ct)
        with self.assertRaises(vl.VaultLockError):
            vl.vault_unlock(locked, os.path.join(tmp, "r"),
                            passphrase_file=pf)

    def test_garbage_file_refused(self):
        tmp = tempfile.mkdtemp()
        bad = os.path.join(tmp, "bad.castle")
        with open(bad, "wb") as f:
            f.write(b"definitely not a vault")
        pf = _secret_file(b"pw")
        with self.assertRaises(vl.VaultLockError):
            vl.vault_unlock(bad, os.path.join(tmp, "r"),
                            passphrase_file=pf)


class TestLock(unittest.TestCase):
    def test_lock_dry_run_is_default(self):
        d = _tree()
        pf = _secret_file(b"pw")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        r = vl.vault_lock(d, locked, passphrase_file=pf)
        self.assertTrue(r["dry_run"])
        # plaintext fully intact, sealed copy exists
        self.assertTrue(os.path.isdir(d))
        self.assertEqual(len(_tree_hashes(d)), 3)
        self.assertTrue(os.path.isfile(locked))

    def test_lock_with_confirmation_burns_plaintext(self):
        d = _tree()
        before = _tree_hashes(d)
        pf = _secret_file(b"pw")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        base = os.path.basename(d)
        r = vl.vault_lock(d, locked, passphrase_file=pf, confirm=base,
                          burn_root=os.path.join(tmp, "burnstate"))
        self.assertFalse(r["dry_run"])
        self.assertFalse(os.path.exists(d))  # plaintext is gone
        # ...but the sealed copy restores it byte-exact
        restored = os.path.join(tmp, "restored")
        vl.vault_unlock(locked, restored, passphrase_file=pf)
        self.assertEqual(_tree_hashes(restored), before)

    def test_lock_wrong_confirmation_stays_dry(self):
        d = _tree()
        pf = _secret_file(b"pw")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        r = vl.vault_lock(d, locked, passphrase_file=pf, confirm="nope")
        self.assertTrue(r["dry_run"])
        self.assertTrue(os.path.isdir(d))


class TestCLI(unittest.TestCase):
    @staticmethod
    def _cli_main(argv):
        import importlib.util
        path = os.path.join(_HERE, "vault.py")
        spec = importlib.util.spec_from_file_location("castle_vault_cli",
                                                      path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod.main(argv)

    def test_cli_init_unlock_roundtrip(self):
        d = _tree()
        before = _tree_hashes(d)
        pf = _secret_file(b"cli-pw")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        rc = self._cli_main(["vault-init", d, "--out", locked,
                             "--passphrase-file", pf])
        self.assertEqual(rc, 0)
        restored = os.path.join(tmp, "restored")
        rc = self._cli_main(["vault-unlock", locked, "--out", restored,
                             "--passphrase-file", pf])
        self.assertEqual(rc, 0)
        self.assertEqual(_tree_hashes(restored), before)

    def test_cli_lock_dry_run_exit_2(self):
        d = _tree()
        pf = _secret_file(b"cli-pw")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        rc = self._cli_main(["vault-lock", d, "--out", locked,
                             "--passphrase-file", pf])
        self.assertEqual(rc, 2)
        self.assertTrue(os.path.isdir(d))

    def test_cli_wrong_passphrase_exit_1(self):
        d = _tree()
        pf = _secret_file(b"right")
        wrong = _secret_file(b"wrong")
        tmp = tempfile.mkdtemp()
        locked = os.path.join(tmp, "v.castle")
        self.assertEqual(self._cli_main(
            ["vault-init", d, "--out", locked,
             "--passphrase-file", pf]), 0)
        rc = self._cli_main(["vault-unlock", locked,
                             "--out", os.path.join(tmp, "r"),
                             "--passphrase-file", wrong])
        self.assertEqual(rc, 1)


if __name__ == "__main__":
    unittest.main()
