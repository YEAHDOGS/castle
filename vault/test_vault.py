#!/usr/bin/env python3
"""Regression tests for the family data vault core (vault.py).

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. Run:  python3 test_vault.py
"""

import json
import os
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import vault  # noqa: E402

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                               "..", "flamethrower"))
import keyring  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-vault-test-")
    tc.addCleanup(__import__("shutil").rmtree, tmp, True)
    return tmp


def mk_users(root):
    vault.create_user(root, "dad", tier="adult")
    vault.create_user(root, "mom", tier="adult")
    vault.create_user(root, "sally", tier="child", guardian="mom")
    return root


def key_names(root):
    """Names of vaults with keys in the keyring (list_vaults returns tuples)."""
    return [n for n, _kid, _ts, _esc in keyring.list_vaults(vault.keyroot(root))]


class TestInitAndLayout(unittest.TestCase):
    def test_init_creates_layout_with_tight_perms(self):
        root = fresh_root(self)
        vault._ensure_dirs(root)
        self.assertEqual(stat.S_IMODE(os.stat(root).st_mode), 0o700)
        self.assertEqual(stat.S_IMODE(os.stat(os.path.join(root, "vaults")).st_mode), 0o700)
        for f in ("users.json", vault.ACTIVITY_FILE):
            self.assertEqual(stat.S_IMODE(os.stat(os.path.join(root, f)).st_mode), 0o600)

    def test_create_user_builds_subdirs_and_key(self):
        root = fresh_root(self)
        vault.create_user(root, "dad")
        for sub in vault.SUBDIRS:
            d = os.path.join(root, "vaults", "dad", sub)
            self.assertTrue(os.path.isdir(d), d)
            self.assertEqual(stat.S_IMODE(os.stat(d).st_mode), 0o700)
        self.assertIn("dad", key_names(root))
        self.assertEqual(vault.verify(root), [])

    def test_duplicate_user_refused(self):
        root = fresh_root(self)
        vault.create_user(root, "dad")
        with self.assertRaises(ValueError):
            vault.create_user(root, "dad")

    def test_invalid_name_refused(self):
        root = fresh_root(self)
        for bad in ("../evil", "a b", "", "x" * 65):
            with self.assertRaises(ValueError, msg=bad):
                vault.create_user(root, bad)


class TestAgeTiers(unittest.TestCase):
    def test_child_requires_guardian(self):
        root = fresh_root(self)
        with self.assertRaises(ValueError):
            vault.create_user(root, "sally", tier="child")

    def test_child_guardian_must_be_existing_adult(self):
        root = fresh_root(self)
        with self.assertRaises(ValueError):
            vault.create_user(root, "sally", tier="child", guardian="nope")

    def test_child_guardian_cannot_be_child(self):
        root = fresh_root(self)
        vault.create_user(root, "mom", tier="adult")
        vault.create_user(root, "sam", tier="child", guardian="mom")
        with self.assertRaises(ValueError):
            vault.create_user(root, "sally", tier="child", guardian="sam")

    def test_child_key_always_escrowed_adult_not(self):
        root = fresh_root(self)
        mk_users(root)
        users = vault._load_users(root)["users"]
        self.assertTrue(users["sally"]["escrowed"])
        self.assertFalse(users["dad"]["escrowed"])
        # escrow copy really exists for the child, not the adult
        self.assertTrue(os.path.exists(
            os.path.join(vault.keyroot(root), "escrow", "sally.key")))
        self.assertFalse(os.path.exists(
            os.path.join(vault.keyroot(root), "escrow", "dad.key")))

    def test_unknown_user_operations_refused(self):
        root = fresh_root(self)
        with self.assertRaises(ValueError):
            vault.add_device(root, "ghost", "phone")
        with self.assertRaises(ValueError):
            vault.add_integration(root, "ghost", "clover")


class TestDevices(unittest.TestCase):
    def test_child_device_needs_guardian_approval(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault.add_device(root, "sally", "sally-phone")          # no approver
        with self.assertRaises(ValueError):
            vault.add_device(root, "sally", "sally-phone", by="dad")  # wrong guardian
        msg = vault.add_device(root, "sally", "sally-phone", by="mom")
        self.assertIn("sally-phone", msg)
        self.assertIn("sally-phone", vault._load_users(root)["users"]["sally"]["devices"])

    def test_adult_self_approves_device(self):
        root = fresh_root(self)
        mk_users(root)
        vault.add_device(root, "dad", "dad-laptop")
        self.assertIn("dad-laptop",
                      vault._load_users(root)["users"]["dad"]["devices"])

    def test_duplicate_device_refused(self):
        root = fresh_root(self)
        mk_users(root)
        vault.add_device(root, "dad", "dad-laptop")
        with self.assertRaises(ValueError):
            vault.add_device(root, "dad", "dad-laptop")


class TestIntegrations(unittest.TestCase):
    def test_adult_can_add_integration(self):
        root = fresh_root(self)
        mk_users(root)
        vault.add_integration(root, "mom", "clover-pos")
        self.assertIn("clover-pos",
                      vault._load_users(root)["users"]["mom"]["integrations"])

    def test_child_integration_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault.add_integration(root, "sally", "clover-pos")


class TestSharingLadder(unittest.TestCase):
    def test_new_vault_is_private(self):
        root = fresh_root(self)
        mk_users(root)
        self.assertEqual(vault.show_shares(root, "dad"), [])

    def test_grant_member_family_world(self):
        root = fresh_root(self)
        mk_users(root)
        vault.grant_share(root, "mom", "dad")
        vault.grant_share(root, "mom", "family")
        vault.grant_share(root, "dad", "world")
        tos = [s["to"] for s in vault.show_shares(root, "mom")]
        self.assertEqual(tos, ["dad", "family"])

    def test_child_cannot_share_to_world(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault.grant_share(root, "sally", "world")

    def test_share_to_self_unknown_duplicate_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault.grant_share(root, "mom", "mom")
        with self.assertRaises(ValueError):
            vault.grant_share(root, "mom", "ghost")
        vault.grant_share(root, "mom", "dad")
        with self.assertRaises(ValueError):
            vault.grant_share(root, "mom", "dad")

    def test_revoke_roundtrip(self):
        root = fresh_root(self)
        mk_users(root)
        vault.grant_share(root, "mom", "family")
        vault.revoke_share(root, "mom", "family")
        self.assertEqual(vault.show_shares(root, "mom"), [])
        with self.assertRaises(ValueError):
            vault.revoke_share(root, "mom", "family")  # not a silent no-op

    def test_share_events_logged(self):
        root = fresh_root(self)
        mk_users(root)
        vault.grant_share(root, "mom", "family")
        vault.revoke_share(root, "mom", "family")
        acts = vault.activity_tail(root, 50)
        actions = [e["action"] for e in acts]
        self.assertIn("share.granted", actions)
        self.assertIn("share.revoked", actions)


class TestTrueDeletion(unittest.TestCase):
    def test_dry_run_burns_nothing(self):
        root = fresh_root(self)
        mk_users(root)
        r = vault.delete_user(root, "dad")
        self.assertTrue(r["dry_run"])
        # key files, vault dir, and registry entry all still there
        self.assertIn("dad", key_names(root))
        self.assertTrue(os.path.isdir(os.path.join(root, "vaults", "dad")))
        self.assertIn("dad", vault._load_users(root)["users"])

    def test_wrong_confirmation_burns_nothing(self):
        root = fresh_root(self)
        mk_users(root)
        r = vault.delete_user(root, "dad", confirm="mom")
        self.assertTrue(r["dry_run"])
        self.assertIn("dad", key_names(root))

    def test_typed_confirm_crypto_shreds_everything(self):
        root = fresh_root(self)
        mk_users(root)
        r = vault.delete_user(root, "sally", confirm="sally")
        self.assertFalse(r["dry_run"])
        self.assertTrue(r["cert_id"])
        # key gone from keyring AND escrow (crypto-shred = data irretrievable)
        self.assertNotIn("sally", key_names(root))
        self.assertFalse(os.path.exists(
            os.path.join(vault.keyroot(root), "escrow", "sally.key")))
        # vault dirs wiped, account retired, survivors untouched
        self.assertFalse(os.path.exists(os.path.join(root, "vaults", "sally")))
        self.assertNotIn("sally", vault._load_users(root)["users"])
        self.assertIn("dad", key_names(root))
        self.assertEqual(vault.verify(root), [])

    def test_deletion_of_unknown_user_refused(self):
        root = fresh_root(self)
        mk_users(root)
        with self.assertRaises(ValueError):
            vault.delete_user(root, "ghost", confirm="ghost")


class TestVerify(unittest.TestCase):
    def test_verify_catches_missing_subdir(self):
        root = fresh_root(self)
        mk_users(root)
        import shutil
        shutil.rmtree(os.path.join(root, "vaults", "mom", "receipts"))
        problems = vault.verify(root)
        self.assertTrue(any("mom" in p and "receipts" in p for p in problems))

    def test_verify_catches_orphan_key(self):
        root = fresh_root(self)
        mk_users(root)
        keyring.create_vault(vault.keyroot(root), "orphan")
        problems = vault.verify(root)
        self.assertTrue(any("orphan" in p for p in problems))


if __name__ == "__main__":
    unittest.main(verbosity=2)
