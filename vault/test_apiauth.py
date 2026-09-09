#!/usr/bin/env python3
"""Regression tests for the Castle API-token auth (vault/apiauth.py).

Wires the DOGS Token Authority (tokens/tokens.py) into the vault
ingestion trust boundary: scoped, short-lived, revocable bearer tokens
for the pos-webhook / email-forward / scan ingestion purposes.

Fixture-based: every test gets a fresh temp vault root (with its own
tokenkeys keyring); nothing touches ~/.castle-vault or ~/.castle-tokens.
Run:  python3 test_apiauth.py
"""

import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import apiauth  # noqa: E402
import poshook  # noqa: E402
import receipts  # noqa: E402
import scan  # noqa: E402
import vault  # noqa: E402
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                               "..", "tokens"))
import tokens as _tokens  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-apiauth-test-")
    tc.addCleanup(__import__("shutil").rmtree, tmp, True)
    return tmp


def mk_adult_with(root, name, purpose):
    vault.create_user(root, name)
    vault.add_integration(root, name, purpose)
    return root


def push(user="sam", txn="txn-001", **kw):
    body = {"user": user, "pos_vendor": "clover",
            "transaction_id": txn, "merchant": "Coffee Shop",
            "total": "7.75", "currency": "USD",
            "occurred_at": "2026-09-09T08:30:00Z",
            "items": [{"name": "Latte", "price": "4.50"},
                      {"name": "Muffin", "price": "3.25"}]}
    body.update(kw)
    return json.dumps(body).encode("utf-8")


MAIL = (b"From: orders@coffeeshop.example.com\r\n"
        b"To: receipts@sam.castle\r\n"
        b"Subject: Your receipt\r\n"
        b"Message-ID: <apiauth-1@coffeeshop.example.com>\r\n"
        b"Content-Type: text/plain; charset=\"utf-8\"\r\n"
        b"\r\nTotal: $7.75\r\n")

OCR = "CORNERSHOP MARKET\nTOTAL $14.82\n"


def fake_jpg(size=2048):
    return b"\xff\xd8\xff\xe0" + b"\x00" * (size - 4)


class TestMint(unittest.TestCase):
    def test_mint_roundtrip_claims(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        tok = apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        self.assertTrue(tok.startswith("dogs1."))
        claims = apiauth.verify_api_token(root, "sam", "pos-webhook", tok)
        self.assertEqual(claims["iss"], "castle-vault")
        self.assertEqual(claims["aud"], "castle-vault")
        self.assertEqual(claims["sub"], "sam")
        self.assertEqual(claims["purpose"], "pos-webhook")

    def test_mint_unknown_user_refused(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        with self.assertRaises(ValueError):
            apiauth.mint_api_token(root, "zed", "pos-webhook")

    def test_mint_child_refused(self):
        root = fresh_root(self)
        vault.create_user(root, "dad")
        vault.create_user(root, "kid", tier="child", guardian="dad")
        with self.assertRaises(ValueError):
            apiauth.mint_api_token(root, "kid", "pos-webhook")

    def test_mint_bad_purpose_refused(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        with self.assertRaises(ValueError):
            apiauth.mint_api_token(root, "sam", "nuke-everything")

    def test_mint_bad_ttl_refused(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        for bad in (0, 30, 86401, -5):
            with self.assertRaises(ValueError):
                apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=bad)

    def test_token_never_lands_in_activity_log(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        tok = apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        with open(os.path.join(root, "activity.jsonl")) as f:
            log = f.read()
        self.assertNotIn(tok, log)
        self.assertIn("mint-api-token", log)

    def test_keyring_perms(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        kd = os.path.join(root, "tokenkeys")
        self.assertEqual(os.stat(kd).st_mode & 0o777, 0o700)
        for name in os.listdir(os.path.join(kd, "keys")):
            self.assertEqual(
                os.stat(os.path.join(kd, "keys", name)).st_mode & 0o777,
                0o600)


class TestEnforcement(unittest.TestCase):
    def test_off_by_default_ingest_needs_no_token(self):
        # regression safety: nothing changes until the operator opts in
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        self.assertEqual(apiauth.api_auth_status(root), {})
        r = poshook.ingest(root, "sam", push())
        self.assertTrue(r["id"].startswith("rcpt-"))

    def test_require_on_refuses_missing_token(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        with self.assertRaises(ValueError) as cm:
            poshook.ingest(root, "sam", push())
        self.assertIn("requires an api token", str(cm.exception))

    def test_require_on_accepts_valid_token(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        tok = apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        r = poshook.ingest(root, "sam", push(), api_token=tok)
        self.assertEqual(r["merchant"], "Coffee Shop")

    def test_require_off_restores_old_behavior(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        apiauth.set_api_auth(root, "pos-webhook", False)
        r = poshook.ingest(root, "sam", push())
        self.assertTrue(r["id"].startswith("rcpt-"))

    def test_wrong_user_token_refused(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        mk_adult_with(root, "sally", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        tok = apiauth.mint_api_token(root, "sally", "pos-webhook", ttl=600)
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push(), api_token=tok)
        self.assertEqual(receipts.list_receipts(root, "sam"), [])

    def test_wrong_purpose_token_refused(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        tok = apiauth.mint_api_token(root, "sam", "scan", ttl=600)
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push(), api_token=tok)

    def test_foreign_audience_token_refused(self):
        # a token minted for another audience never verifies on castle-vault
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        foreign = _tokens.mint(d=apiauth._keyring_dir(root),
                               iss="castle-vault", aud="somewhere-else",
                               sub="sam", purpose="pos-webhook", ttl=600)
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push(), api_token=foreign)

    def test_tampered_token_refused(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        tok = apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        parts = tok.split(".")
        ct = parts[2]
        parts[2] = ("A" if ct[0] != "A" else "B") + ct[1:]
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push(), api_token=".".join(parts))

    def test_revoked_token_refused(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        tok = apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        apiauth.revoke_api_token(root, tok, reason="leaked")
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push(), api_token=tok)

    def test_removed_integration_voids_token(self):
        # removing the integration kills the surface's tokens implicitly
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        apiauth.set_api_auth(root, "pos-webhook", True)
        tok = apiauth.mint_api_token(root, "sam", "pos-webhook", ttl=600)
        users = vault._load_users(root)
        users["users"]["sam"]["integrations"].remove("pos-webhook")
        vault._save_users(root, users)
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push(), api_token=tok)

    def test_email_forward_enforcement(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "email-forward")
        apiauth.set_api_auth(root, "email-forward", True)
        with self.assertRaises(ValueError):
            receipts.ingest(root, "sam", MAIL)
        tok = apiauth.mint_api_token(root, "sam", "email-forward", ttl=600)
        r = receipts.ingest(root, "sam", MAIL, api_token=tok)
        self.assertTrue(r["id"].startswith("rcpt-"))

    def test_scan_enforcement(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "scan")
        apiauth.set_api_auth(root, "scan", True)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_jpg(), OCR)
        tok = apiauth.mint_api_token(root, "sam", "scan", ttl=600)
        r = scan.ingest(root, "sam", fake_jpg(), OCR, api_token=tok)
        self.assertEqual(r["total"], "14.82")


class TestCLI(unittest.TestCase):
    def test_api_auth_cli_on_off(self):
        root = fresh_root(self)
        mk_adult_with(root, "sam", "pos-webhook")
        self.assertEqual(
            vault.main(["--dir", root, "api-auth", "--purpose", "pos-webhook",
                        "--on"]), 0)
        self.assertTrue(apiauth.api_auth_status(root)["pos-webhook"])
        self.assertEqual(
            vault.main(["--dir", root, "api-auth", "--purpose", "pos-webhook",
                        "--off"]), 0)
        self.assertFalse(apiauth.api_auth_status(root)["pos-webhook"])

    def test_api_auth_cli_needs_exactly_one_flag(self):
        root = fresh_root(self)
        self.assertEqual(
            vault.main(["--dir", root, "api-auth", "--purpose", "pos-webhook"]),
            1)
        self.assertEqual(
            vault.main(["--dir", root, "api-auth", "--purpose", "pos-webhook",
                        "--on", "--off"]), 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
