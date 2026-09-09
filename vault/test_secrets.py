#!/usr/bin/env python3
"""Regression tests for the Castle secrets store (vault/secretstore.py).

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. All secret values are obvious fakes — real key material
never appears in fixtures, logs, or assertions. Run: python3 test_secrets.py
"""

import io
import json
import os
import shutil
import stat
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import vault  # noqa: E402
import secretstore as vault_secrets  # noqa: E402

FAKE_A = b"FAKE-TEST-API-KEY-ALPHA-0001"
FAKE_B = b"FAKE-TEST-API-KEY-BRAVO-0002"


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-secrets-test-")
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_users(root):
    vault.create_user(root, "dad", tier="adult")
    vault.create_user(root, "mom", tier="adult")
    vault.create_user(root, "sally", tier="child", guardian="mom")
    return root


def secret_file_path(root, user, name):
    return os.path.join(root, "vaults", user, "secrets", name + ".secret")


def activity_text(root):
    with open(os.path.join(root, vault.ACTIVITY_FILE),
              encoding="utf-8") as f:
        return f.read()


class _FakeStdin:
    """sys.stdin stand-in: read_secret_input only uses .buffer.read()."""
    def __init__(self, data=b""):
        self.buffer = io.BytesIO(data)


class _FakeStdout:
    """sys.stdout stand-in: get-secret only uses .buffer.write()."""
    def __init__(self):
        self.buffer = io.BytesIO()


class TestAddGetRoundtrip(unittest.TestCase):
    def test_add_then_get_roundtrips(self):
        root = fresh_root(self)
        mk_users(root)
        msg = vault_secrets.add_secret(root, "dad", "alpaca-key", FAKE_A)
        self.assertIn("alpaca-key", msg)
        self.assertEqual(vault_secrets.get_secret(root, "dad", "alpaca-key"),
                         FAKE_A)

    def test_ciphertext_on_disk_is_not_plaintext(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "alpaca-key", FAKE_A)
        with open(secret_file_path(root, "dad", "alpaca-key"), "rb") as f:
            raw = f.read()
        self.assertNotIn(FAKE_A, raw)
        # header is a single JSON line, then raw ciphertext
        header_raw, sep, ct = raw.partition(b"\n")
        self.assertTrue(sep)
        header = json.loads(header_raw.decode("utf-8"))
        self.assertEqual(header["format"], "castle-secret/v1")
        self.assertEqual(header["cipher"], "aes-256-cbc")
        self.assertEqual(header["mac"], "hmac-sha256")
        self.assertTrue(len(ct) > 0)

    def test_tight_perms_on_everything(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "alpaca-key", FAKE_A)
        sdir = os.path.join(root, "vaults", "dad", "secrets")
        self.assertEqual(stat.S_IMODE(os.stat(sdir).st_mode), 0o700)
        for fn in ("alpaca-key.secret", "index.json"):
            full = os.path.join(sdir, fn)
            self.assertEqual(stat.S_IMODE(os.stat(full).st_mode), 0o600, fn)

    def test_index_holds_names_not_values(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "alpaca-key", FAKE_A)
        with open(os.path.join(root, "vaults", "dad", "secrets",
                               "index.json"), "rb") as f:
            raw = f.read()
        self.assertNotIn(FAKE_A, raw)
        idx = json.loads(raw.decode("utf-8"))
        self.assertIn("alpaca-key", idx["secrets"])
        self.assertIn("created", idx["secrets"]["alpaca-key"])

    def test_per_user_keys_are_isolated(self):
        # same secret name for two users; corrupting one's ciphertext
        # must not touch the other's (different data keys)
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        vault_secrets.add_secret(root, "mom", "tok", FAKE_B)
        self.assertEqual(vault_secrets.get_secret(root, "dad", "tok"),
                         FAKE_A)
        self.assertEqual(vault_secrets.get_secret(root, "mom", "tok"),
                         FAKE_B)
        # flip a byte in dad's ciphertext -> dad fails closed, mom fine
        path = secret_file_path(root, "dad", "tok")
        with open(path, "rb") as f:
            raw = bytearray(f.read())
        raw[-1] ^= 0x01
        with open(path, "wb") as f:
            f.write(bytes(raw))
        with self.assertRaises(ValueError):
            vault_secrets.get_secret(root, "dad", "tok")
        self.assertEqual(vault_secrets.get_secret(root, "mom", "tok"),
                         FAKE_B)


class TestRefusals(unittest.TestCase):
    def test_child_refused_everywhere(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault_secrets.add_secret(root, "sally", "tok", FAKE_A)
        with self.assertRaises(ValueError):
            vault_secrets.get_secret(root, "sally", "tok")
        with self.assertRaises(ValueError):
            vault_secrets.list_secrets(root, "sally")
        with self.assertRaises(ValueError):
            vault_secrets.rotate_secret(root, "sally", "tok", FAKE_A)
        with self.assertRaises(ValueError):
            vault_secrets.delete_secret(root, "sally", "tok", confirm="tok")

    def test_unknown_user_refused(self):
        root = fresh_root(self)
        mk_users(root)
        for op in (lambda: vault_secrets.add_secret(root, "ghost", "t",
                                                    FAKE_A),
                   lambda: vault_secrets.get_secret(root, "ghost", "t"),
                   lambda: vault_secrets.list_secrets(root, "ghost")):
            with self.assertRaises(ValueError):
                op()

    def test_empty_secret_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault_secrets.add_secret(root, "dad", "tok", b"")
        # empty via stdin path
        with mock.patch.object(sys, "stdin", _FakeStdin(b"")):
            with self.assertRaises(ValueError):
                vault_secrets.read_secret_input()

    def test_oversize_secret_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault_secrets.add_secret(root, "dad", "tok",
                                     b"x" * (vault_secrets.MAX_SECRET_BYTES
                                             + 1))

    def test_duplicate_add_refused(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        with self.assertRaises(ValueError):
            vault_secrets.add_secret(root, "dad", "tok", FAKE_B)

    def test_traversal_names_refused(self):
        root = fresh_root(self)
        mk_users(root)
        for bad in ("../evil", "a/b", "..", "", "x" * 65, "a b"):
            with self.assertRaises(ValueError, msg=bad):
                vault_secrets.add_secret(root, "dad", bad, FAKE_A)
            with self.assertRaises(ValueError, msg=bad):
                vault_secrets.get_secret(root, "dad", bad)

    def test_unknown_secret_name_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault_secrets.get_secret(root, "dad", "nope")
        with self.assertRaises(ValueError):
            vault_secrets.rotate_secret(root, "dad", "nope", FAKE_A)
        with self.assertRaises(ValueError):
            vault_secrets.delete_secret(root, "dad", "nope", confirm="nope")

    def test_secret_file_must_be_0600_not_symlink(self):
        root = fresh_root(self)
        mk_users(root)
        loose = os.path.join(root, "loose.key")
        with open(loose, "wb") as f:
            f.write(FAKE_A)
        os.chmod(loose, 0o644)
        with self.assertRaises(ValueError):
            vault_secrets.read_secret_input(loose)
        link = os.path.join(root, "link.key")
        tight = os.path.join(root, "tight.key")
        with open(tight, "wb") as f:
            f.write(FAKE_A)
        os.chmod(tight, 0o600)
        os.symlink(tight, link)
        with self.assertRaises(ValueError):
            vault_secrets.read_secret_input(link)
        with self.assertRaises(ValueError):
            vault_secrets.read_secret_input(os.path.join(root, "missing.key"))
        # the good path works
        self.assertEqual(vault_secrets.read_secret_input(tight), FAKE_A)


class TestGetListLogging(unittest.TestCase):
    def test_add_rotate_delete_logged_by_name_only(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        vault_secrets.rotate_secret(root, "dad", "tok", FAKE_B)
        vault_secrets.delete_secret(root, "dad", "tok", confirm="tok")
        text = activity_text(root)
        for action in ("secret.added", "secret.rotated", "secret.deleted"):
            self.assertIn(action, text)
        self.assertIn("tok", text)
        # key material never touches the log
        self.assertNotIn(FAKE_A.decode(), text)
        self.assertNotIn(FAKE_B.decode(), text)

    def test_get_logs_nothing(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        before = activity_text(root)
        vault_secrets.get_secret(root, "dad", "tok")
        vault_secrets.list_secrets(root, "dad")
        self.assertEqual(activity_text(root), before)

    def test_list_returns_names_only(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "alpha", FAKE_A)
        vault_secrets.add_secret(root, "dad", "beta", FAKE_B)
        names = vault_secrets.list_secrets(root, "dad")
        self.assertEqual(names, ["alpha", "beta"])
        self.assertNotIn(FAKE_A, names)
        self.assertNotIn(FAKE_B, names)


class TestRotate(unittest.TestCase):
    def test_rotate_replaces_value(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        old_ct = open(secret_file_path(root, "dad", "tok"), "rb").read()
        vault_secrets.rotate_secret(root, "dad", "tok", FAKE_B)
        self.assertEqual(vault_secrets.get_secret(root, "dad", "tok"),
                         FAKE_B)
        new_ct = open(secret_file_path(root, "dad", "tok"), "rb").read()
        self.assertNotEqual(old_ct, new_ct)  # fresh IV, fresh ciphertext
        idx = vault_secrets._load_index(root, "dad")
        self.assertGreaterEqual(idx["secrets"]["tok"]["rotated"],
                                idx["secrets"]["tok"]["created"])

    def test_rotate_unknown_or_empty_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault_secrets.rotate_secret(root, "dad", "nope", FAKE_A)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        with self.assertRaises(ValueError):
            vault_secrets.rotate_secret(root, "dad", "tok", b"")


class TestDelete(unittest.TestCase):
    def test_dry_run_lists_target_burns_nothing(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        r = vault_secrets.delete_secret(root, "dad", "tok")
        self.assertTrue(r["dry_run"])
        self.assertTrue(os.path.exists(
            secret_file_path(root, "dad", "tok")))
        self.assertIn("tok", vault_secrets._load_index(root, "dad")["secrets"])

    def test_wrong_confirmation_burns_nothing(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        r = vault_secrets.delete_secret(root, "dad", "tok", confirm="mom")
        self.assertTrue(r["dry_run"])
        self.assertTrue(os.path.exists(
            secret_file_path(root, "dad", "tok")))

    def test_typed_confirm_crypto_shreds(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        vault_secrets.add_secret(root, "dad", "other", FAKE_B)
        r = vault_secrets.delete_secret(root, "dad", "tok", confirm="tok")
        self.assertFalse(r["dry_run"])
        self.assertFalse(os.path.exists(
            secret_file_path(root, "dad", "tok")))
        idx = vault_secrets._load_index(root, "dad")["secrets"]
        self.assertNotIn("tok", idx)
        self.assertIn("other", idx)  # survivors untouched
        # and it's really gone
        with self.assertRaises(ValueError):
            vault_secrets.get_secret(root, "dad", "tok")
        text = activity_text(root)
        self.assertIn("secret.deleted", text)
        self.assertNotIn(FAKE_A.decode(), text)


class TestVaultIntegration(unittest.TestCase):
    def test_verify_green_with_secrets(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        self.assertEqual(vault.verify(root), [])

    def test_verify_catches_loose_secret_perms(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        os.chmod(secret_file_path(root, "dad", "tok"), 0o644)
        problems = vault.verify(root)
        self.assertTrue(any("tok.secret" in p for p in problems))

    def test_delete_user_shreds_secrets(self):
        root = fresh_root(self)
        mk_users(root)
        vault_secrets.add_secret(root, "dad", "tok", FAKE_A)
        r = vault.delete_user(root, "dad", confirm="dad")
        self.assertFalse(r["dry_run"])
        self.assertFalse(os.path.exists(
            os.path.join(root, "vaults", "dad")))
        self.assertTrue(any("shredded" in t for t in r["kills"]))
        # mom's vault (and her lack of secrets) untouched
        self.assertIn("mom", vault._load_users(root)["users"])
        self.assertEqual(vault.verify(root), [])

    def test_cli_add_get_list_delete_roundtrip(self):
        root = fresh_root(self)
        mk_users(root)
        with mock.patch.object(sys, "stdin", _FakeStdin(FAKE_A)):
            self.assertEqual(
                vault.main(["--dir", root, "add-secret", "dad", "cli-tok"]),
                0)
        fake_out = _FakeStdout()
        with mock.patch.object(sys, "stdout", fake_out):
            self.assertEqual(
                vault.main(["--dir", root, "get-secret", "dad", "cli-tok"]),
                0)
        self.assertEqual(fake_out.buffer.getvalue(), FAKE_A)
        # list via CLI
        out = io.StringIO()
        with mock.patch("sys.stdout", out):
            vault.main(["--dir", root, "list-secrets", "dad"])
        self.assertIn("cli-tok", out.getvalue())
        # delete dry-run exits 2, real burn exits 0
        self.assertEqual(
            vault.main(["--dir", root, "delete-secret", "dad", "cli-tok"]),
            2)
        self.assertEqual(
            vault.main(["--dir", root, "delete-secret", "dad", "cli-tok",
                        "--yes", "cli-tok"]),
            0)
        self.assertEqual(vault_secrets.list_secrets(root, "dad"), [])

    def test_cli_child_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with mock.patch.object(sys, "stdin", _FakeStdin(FAKE_A)):
            self.assertEqual(
                vault.main(["--dir", root, "add-secret", "sally", "tok"]),
                1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
