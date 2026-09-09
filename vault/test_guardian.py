#!/usr/bin/env python3
"""Regression tests for guardian controls (vault/guardian.py).

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. Run:  python3 test_guardian.py
"""

import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import guardian  # noqa: E402
import vault  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-guardian-test-")
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_family(root):
    """mom (guardian) + sam (child of mom) + dad (unrelated adult)."""
    vault.create_user(root, "mom", tier="adult")
    vault.create_user(root, "dad", tier="adult")
    vault.create_user(root, "sam", tier="child", guardian="mom")
    vault.add_device(root, "sam", "sam-phone", by="mom")
    vault.add_device(root, "mom", "mom-laptop")


class TestRevokeDevice(unittest.TestCase):
    def test_guardian_revokes_child_device(self):
        r = fresh_root(self)
        mk_family(r)
        msg = guardian.revoke_device(r, "sam", "sam-phone", by="mom")
        self.assertIn("revoked", msg)
        users = vault._load_users(r)
        self.assertEqual(users["users"]["sam"]["devices"], [])

    def test_non_guardian_cannot_revoke_child_device(self):
        r = fresh_root(self)
        mk_family(r)
        with self.assertRaises(ValueError):
            guardian.revoke_device(r, "sam", "sam-phone", by="dad")
        users = vault._load_users(r)
        self.assertIn("sam-phone", users["users"]["sam"]["devices"])

    def test_child_cannot_self_revoke_device(self):
        # The kid reports the loss; the guardian executes. If sam could
        # revoke his own devices, device approval would be theater.
        r = fresh_root(self)
        mk_family(r)
        with self.assertRaises(ValueError):
            guardian.revoke_device(r, "sam", "sam-phone", by="sam")

    def test_adult_revokes_own_device(self):
        r = fresh_root(self)
        mk_family(r)
        guardian.revoke_device(r, "mom", "mom-laptop")
        users = vault._load_users(r)
        self.assertEqual(users["users"]["mom"]["devices"], [])

    def test_guardian_cannot_touch_adult_devices(self):
        # No backdoor: mom's guardianship over sam gives her nothing on dad.
        r = fresh_root(self)
        mk_family(r)
        vault.add_device(r, "dad", "dad-phone")
        with self.assertRaises(ValueError) as ctx:
            guardian.revoke_device(r, "dad", "dad-phone", by="mom")
        self.assertIn("own devices", str(ctx.exception))
        users = vault._load_users(r)
        self.assertIn("dad-phone", users["users"]["dad"]["devices"])

    def test_revoking_unknown_device_is_refused(self):
        r = fresh_root(self)
        mk_family(r)
        with self.assertRaises(ValueError):
            guardian.revoke_device(r, "sam", "nope-phone", by="mom")

    def test_revocation_is_logged(self):
        r = fresh_root(self)
        mk_family(r)
        guardian.revoke_device(r, "sam", "sam-phone", by="mom")
        acts = [e["action"] for e in vault.activity_tail(r)]
        self.assertIn("device.revoked", acts)


class TestGuardianRevokeShare(unittest.TestCase):
    def test_guardian_revokes_child_share(self):
        r = fresh_root(self)
        mk_family(r)
        vault.grant_share(r, "sam", "dad")
        msg = guardian.guardian_revoke_share(r, "sam", "dad", "mom")
        self.assertIn("revoked", msg)
        self.assertEqual(vault.show_shares(r, "sam"), [])

    def test_non_guardian_refused(self):
        r = fresh_root(self)
        mk_family(r)
        vault.grant_share(r, "sam", "dad")
        with self.assertRaises(ValueError):
            guardian.guardian_revoke_share(r, "sam", "dad", "dad")
        self.assertEqual(len(vault.show_shares(r, "sam")), 1)

    def test_adult_shares_are_immune(self):
        # Dad shares something; mom is NOT his guardian, so she gets
        # nothing — not even on the kid. Vision: no family-admin backdoor.
        r = fresh_root(self)
        mk_family(r)
        vault.grant_share(r, "dad", "mom")
        with self.assertRaises(ValueError):
            guardian.guardian_revoke_share(r, "dad", "mom", "mom")
        self.assertEqual(len(vault.show_shares(r, "dad")), 1)

    def test_phantom_revoke_is_refused(self):
        r = fresh_root(self)
        mk_family(r)
        with self.assertRaises(ValueError):
            guardian.guardian_revoke_share(r, "sam", "family", "mom")

    def test_override_is_logged_as_guardian(self):
        r = fresh_root(self)
        mk_family(r)
        vault.grant_share(r, "sam", "dad")
        guardian.guardian_revoke_share(r, "sam", "dad", "mom")
        acts = [e["action"] for e in vault.activity_tail(r)]
        self.assertIn("share.revoked.by_guardian", acts)


class TestGraduate(unittest.TestCase):
    def test_graduation_flips_tier_clears_guardian_keeps_data(self):
        r = fresh_root(self)
        mk_family(r)
        vault.grant_share(r, "sam", "family")
        msg = guardian.graduate(r, "sam", by="mom")
        self.assertIn("graduated", msg)
        users = vault._load_users(r)
        rec = users["users"]["sam"]
        self.assertEqual(rec["tier"], "adult")
        self.assertIsNone(rec["guardian"])
        self.assertIn("sam-phone", rec["devices"])  # data comes with him
        self.assertEqual(len(rec["shares"]), 1)
        self.assertTrue(rec["escrowed"])  # family recovery keeps working
        self.assertEqual(vault.verify(r), [])  # registry still consistent

    def test_graduate_unlocks_adult_surface(self):
        # Before graduation: world shares and integrations are refused.
        # After: allowed, because the tier is real.
        r = fresh_root(self)
        mk_family(r)
        with self.assertRaises(ValueError):
            vault.grant_share(r, "sam", "world")
        guardian.graduate(r, "sam", by="mom")
        vault.grant_share(r, "sam", "world")
        vault.add_integration(r, "sam", "email-forward")
        self.assertEqual(len(vault.show_shares(r, "sam")), 1)

    def test_graduation_needs_the_guardian(self):
        r = fresh_root(self)
        mk_family(r)
        with self.assertRaises(ValueError):
            guardian.graduate(r, "sam", by="dad")
        users = vault._load_users(r)
        self.assertEqual(users["users"]["sam"]["tier"], "child")

    def test_adult_cannot_graduate(self):
        r = fresh_root(self)
        mk_family(r)
        with self.assertRaises(ValueError):
            guardian.graduate(r, "mom", by="mom")

    def test_graduation_is_logged(self):
        r = fresh_root(self)
        mk_family(r)
        guardian.graduate(r, "sam", by="mom")
        acts = [e["action"] for e in vault.activity_tail(r)]
        self.assertIn("user.graduated", acts)


class TestGuardianReview(unittest.TestCase):
    def test_review_shows_child_activity_only(self):
        r = fresh_root(self)
        mk_family(r)
        vault.grant_share(r, "dad", "mom")  # noise: not sam's
        vault.grant_share(r, "sam", "family")
        hits = guardian.guardian_review(r, "sam")
        self.assertTrue(all(
            e["actor"] == "sam" or "sam" in str(e.get("detail", ""))
            for e in hits))
        self.assertTrue(any("share.granted" in e["action"] for e in hits))


if __name__ == "__main__":
    unittest.main(verbosity=2)
