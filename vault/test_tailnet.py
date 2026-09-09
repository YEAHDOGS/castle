#!/usr/bin/env python3
"""Regression tests for tailnet onboarding + hosted DNS per service
(vault/tailnet.py) — FAMILY-DATA-VAULT.md build-order step 4.

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. No network calls are made by anything under test.
Run:  python3 test_tailnet.py
"""

import json
import os
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import vault  # noqa: E402
import tailnet  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-tailnet-test-")
    tc.addCleanup(__import__("shutil").rmtree, tmp, True)
    return tmp


def mk_family(root):
    vault.create_user(root, "dad", tier="adult")
    vault.create_user(root, "mom", tier="adult")
    vault.create_user(root, "sally", tier="child", guardian="mom")
    vault.add_device(root, "dad", "thinkpad")
    vault.add_device(root, "sally", "phone", by="mom")
    return root


def read_state(root):
    with open(os.path.join(root, "tailnet.json"), encoding="utf-8") as f:
        return json.load(f)


class OnboardTest(unittest.TestCase):
    def test_unknown_user_refused(self):
        root = mk_family(fresh_root(self))
        with self.assertRaises(ValueError):
            tailnet.onboard(root, "uncle", "laptop")

    def test_unregistered_device_refused(self):
        root = mk_family(fresh_root(self))
        with self.assertRaises(ValueError):
            tailnet.onboard(root, "dad", "never-registered")

    def test_bad_device_name_refused(self):
        root = mk_family(fresh_root(self))
        vault.add_device(root, "dad", "goodname")
        with self.assertRaises(ValueError):
            tailnet.onboard(root, "dad", "../evil")
        with self.assertRaises(ValueError):
            tailnet.onboard(root, "dad", "")

    def test_onboard_pending_and_runbook(self):
        root = mk_family(fresh_root(self))
        r = tailnet.onboard(root, "dad", "thinkpad")
        self.assertEqual(r["state"], "pending")
        self.assertIn("tailscale up", r["runbook"])
        self.assertIn("--authkey <PASTE-AUTH-KEY>", r["runbook"])
        doc = read_state(root)
        self.assertEqual(doc["devices"]["dad/thinkpad"]["state"], "pending")
        self.assertIsNone(doc["devices"]["dad/thinkpad"]["ip"])

    def test_no_auth_key_material_stored(self):
        """The auth key lives only in the operator's paste buffer."""
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        blob = open(os.path.join(root, "tailnet.json")).read()
        self.assertNotIn("authkey", blob.lower())
        self.assertNotIn("tskey", blob.lower())
        log = open(os.path.join(root, "activity.jsonl")).read()
        self.assertNotIn("authkey", log.lower())

    def test_double_onboard_refused(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        with self.assertRaises(ValueError):
            tailnet.onboard(root, "dad", "thinkpad")

    def test_child_device_onboards_after_guardian_registration(self):
        root = mk_family(fresh_root(self))
        r = tailnet.onboard(root, "sally", "phone", by="mom")
        self.assertEqual(r["state"], "pending")

    def test_tailnet_state_is_0600(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        mode = stat.S_IMODE(os.stat(os.path.join(root, "tailnet.json")).st_mode)
        self.assertEqual(mode, 0o600)

    def test_corrupt_state_refused_not_migrated(self):
        root = mk_family(fresh_root(self))
        with open(os.path.join(root, "tailnet.json"), "w") as f:
            f.write('{"version": 1, "devices": [1,2]}')
        with self.assertRaises(ValueError):
            tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")


class ClaimTest(unittest.TestCase):
    def test_claim_before_onboard_refused(self):
        root = mk_family(fresh_root(self))
        with self.assertRaises(ValueError):
            tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")

    def test_non_tailnet_ips_refused(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        for bad in ("192.168.1.5", "10.0.0.9", "8.8.8.8", "127.0.0.1",
                    "::1", "not-an-ip", ""):
            with self.assertRaises(ValueError, msg=bad):
                tailnet.claim(root, "dad", "thinkpad", bad)

    def test_claim_happy_path(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        r = tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")
        self.assertEqual(r["state"], "active")
        self.assertEqual(r["ip"], "100.80.1.2")

    def test_double_claim_refused(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")
        with self.assertRaises(ValueError):
            tailnet.claim(root, "dad", "thinkpad", "100.80.1.3")

    def test_claim_unregistered_device_refused(self):
        root = mk_family(fresh_root(self))
        with self.assertRaises(ValueError):
            tailnet.claim(root, "dad", "ghost", "100.80.1.2")


class StatusDropTest(unittest.TestCase):
    def test_status_lists_pending_and_active(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        tailnet.onboard(root, "sally", "phone", by="mom")
        tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")
        rows = {(r["user"], r["device"]): r for r in tailnet.status(root)}
        self.assertEqual(rows[("dad", "thinkpad")]["state"], "active")
        self.assertEqual(rows[("sally", "phone")]["state"], "pending")
        self.assertIsNone(rows[("sally", "phone")]["ip"])

    def test_status_excludes_revoked_device(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")
        import guardian  # noqa: E402
        guardian.revoke_device(root, "dad", "thinkpad", by="dad")
        self.assertEqual(tailnet.status(root), [])
        # and claiming a revoked device is refused outright
        with self.assertRaises(ValueError):
            tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")

    def test_drop_unknown_refused(self):
        root = mk_family(fresh_root(self))
        with self.assertRaises(ValueError):
            tailnet.drop(root, "dad", "thinkpad")

    def test_drop_purges_record(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        tailnet.drop(root, "dad", "thinkpad")
        self.assertNotIn("dad/thinkpad", read_state(root)["devices"])
        # can re-onboard cleanly afterwards
        tailnet.onboard(root, "dad", "thinkpad")
        self.assertEqual(read_state(root)["devices"]["dad/thinkpad"]["state"],
                         "pending")


class DnsTest(unittest.TestCase):
    def test_dns_fragment_has_both_services(self):
        root = mk_family(fresh_root(self))
        out = os.path.join(root, "dns.conf")
        r = tailnet.dns_refresh(root, "100.91.2.3", out=out)
        body = open(out).read()
        self.assertIn("address=/vault.castle/100.91.2.3", body)
        self.assertIn("address=/receipts.castle/100.91.2.3", body)
        self.assertEqual(r["castle_ip"], "100.91.2.3")
        mode = stat.S_IMODE(os.stat(out).st_mode)
        self.assertEqual(mode, 0o600)

    def test_dns_default_path(self):
        root = mk_family(fresh_root(self))
        r = tailnet.dns_refresh(root, "100.91.2.3")
        self.assertTrue(r["out"].endswith(
            os.path.join("dnsmasq.d", "10-castle.conf")))
        self.assertTrue(os.path.exists(r["out"]))

    def test_dns_refuses_non_tailnet_castle_ip(self):
        root = mk_family(fresh_root(self))
        for bad in ("192.168.1.1", "127.0.0.1", "203.0.113.9", "nope"):
            with self.assertRaises(ValueError, msg=bad):
                tailnet.dns_refresh(root, bad)

    def test_dns_dir_is_0700(self):
        root = mk_family(fresh_root(self))
        tailnet.dns_refresh(root, "100.91.2.3")
        mode = stat.S_IMODE(
            os.stat(os.path.join(root, "dnsmasq.d")).st_mode)
        self.assertEqual(mode, 0o700)


class ActivityLogTest(unittest.TestCase):
    def test_events_are_metadata_only(self):
        root = mk_family(fresh_root(self))
        tailnet.onboard(root, "dad", "thinkpad")
        tailnet.claim(root, "dad", "thinkpad", "100.80.1.2")
        tailnet.dns_refresh(root, "100.91.2.3")
        actions = [json.loads(l)["action"]
                   for l in open(os.path.join(root, "activity.jsonl"))]
        self.assertIn("tailnet.onboarded", actions)
        self.assertIn("tailnet.claimed", actions)
        self.assertIn("tailnet.dns", actions)


if __name__ == "__main__":
    unittest.main(verbosity=2)
