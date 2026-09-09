#!/usr/bin/env python3
"""Regression tests for the POS webhook receiver (poshook.py).

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. Run:  python3 test_poshook.py
"""

import json
import os
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import poshook  # noqa: E402
import receipts  # noqa: E402
import vault  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-poshook-test-")
    tc.addCleanup(__import__("shutil").rmtree, tmp, True)
    return tmp


def mk_adult_with_pos(root, name="sam"):
    vault.create_user(root, name)
    vault.add_integration(root, name, "pos-webhook")
    return root


def push(user="sam", **kw):
    body = {"user": user, "pos_vendor": "clover",
            "transaction_id": "txn-001", "merchant": "Coffee Shop",
            "total": "7.75", "currency": "USD",
            "occurred_at": "2026-09-09T08:30:00Z",
            "items": [{"name": "Latte", "price": "4.50"},
                      {"name": "Muffin", "price": "3.25"}]}
    body.update(kw)
    return json.dumps(body).encode("utf-8")


class TestAddressing(unittest.TestCase):
    def test_resolve_recipient_ok(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        user, payload = poshook.resolve_recipient(root, push())
        self.assertEqual(user, "sam")
        self.assertEqual(payload["merchant"], "Coffee Shop")

    def test_non_json_refused(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        with self.assertRaises(ValueError):
            poshook.resolve_recipient(root, b"not json at all")
        with self.assertRaises(ValueError):
            poshook.resolve_recipient(root, b"[1,2,3]")

    def test_missing_user_field_refused(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        with self.assertRaises(ValueError):
            poshook.resolve_recipient(root, push(user=None))

    def test_unknown_user_refused(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        with self.assertRaises(ValueError):
            poshook.resolve_recipient(root, push(user="zed"))

    def test_push_never_lands_in_wrong_vault(self):
        root = fresh_root(self)
        mk_adult_with_pos(root, "sam")
        mk_adult_with_pos(root, "sally")
        # push addressed to sam, ingested as sally: refused
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sally", push())
        self.assertEqual(receipts.list_receipts(root, "sally"), [])
        self.assertEqual(receipts.list_receipts(root, "sam"), [])


class TestIngestRefusals(unittest.TestCase):
    def test_requires_pos_webhook_integration(self):
        root = fresh_root(self)
        vault.create_user(root, "sam")  # adult, but no integration
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push())

    def test_child_cannot_enable_surface(self):
        root = fresh_root(self)
        vault.create_user(root, "mom")
        vault.create_user(root, "sally", tier="child", guardian="mom")
        with self.assertRaises(ValueError):  # add-integration is adult-only
            vault.add_integration(root, "sally", "pos-webhook")
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sally", push(user="sally"))

    def test_empty_payload_refused(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", b"")
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", b"   ")

    def test_incomplete_record_refused_not_guessed(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        body = json.loads(push().decode())
        del body["merchant"]
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", json.dumps(body).encode())
        body = json.loads(push().decode())
        del body["total"]
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", json.dumps(body).encode())
        self.assertEqual(receipts.list_receipts(root, "sam"), [])

    def test_float_total_refused(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        # a float is a lossy guess at money — refused, never rounded
        for bad in (7.75, "7.750", "$7.75", "7,75", "7"):
            body = json.loads(push().decode())
            body["total"] = bad
            with self.assertRaises(ValueError):
                poshook.ingest(root, "sam", json.dumps(body).encode())

    def test_bad_currency_refused(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        body = json.loads(push().decode())
        body["currency"] = "usd"
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", json.dumps(body).encode())

    def test_duplicate_push_is_idempotent(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        r1 = poshook.ingest(root, "sam", push())
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", push())  # same bytes
        # same transaction_id under different bytes: still refused
        body = json.loads(push().decode())
        body["occurred_at"] = "2026-09-09T08:31:00Z"
        with self.assertRaises(ValueError):
            poshook.ingest(root, "sam", json.dumps(body).encode())
        self.assertEqual(len(receipts.list_receipts(root, "sam")), 1)
        self.assertEqual(
            receipts.get_receipt(root, "sam", r1["id"])["id"], r1["id"])

    def test_optional_fields_may_be_absent(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        body = {"user": "sam", "merchant": "Corner Deli", "total": "12.00"}
        r = poshook.ingest(root, "sam", json.dumps(body).encode())
        self.assertEqual(r["currency"], "USD")       # default kept
        self.assertIsNone(r["transaction_id"])
        self.assertIsNone(r["pos_vendor"])


class TestStorage(unittest.TestCase):
    def test_record_and_raw_kept_tight(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        r = poshook.ingest(root, "sam", push())
        self.assertEqual(r["source"], "pos-webhook")
        self.assertEqual(r["merchant_confidence"], "high")
        self.assertEqual(r["total_confidence"], "high")
        self.assertEqual(r["total"], "7.75")
        self.assertEqual(r["transaction_id"], "txn-001")
        self.assertEqual(r["pos_vendor"], "clover")
        self.assertEqual(len(r["items"]), 2)
        rdir = os.path.join(root, "vaults", "sam", "receipts", r["id"])
        self.assertEqual(stat.S_IMODE(os.stat(rdir).st_mode), 0o700)
        for name in ("receipt.json", "raw.json"):
            p = os.path.join(rdir, name)
            self.assertTrue(os.path.isfile(p), p)
            self.assertEqual(stat.S_IMODE(os.stat(p).st_mode), 0o600)
        # raw.json is the verbatim canonical push
        with open(os.path.join(rdir, "raw.json"), encoding="utf-8") as f:
            raw = json.loads(f.read())
        self.assertEqual(raw["transaction_id"], "txn-001")

    def test_pos_and_email_receipts_share_the_index(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        vault.add_integration(root, "sam", "email-forward")
        poshook.ingest(root, "sam", push())
        lst = receipts.list_receipts(root, "sam")
        self.assertEqual(len(lst), 1)
        self.assertEqual(lst[0]["source"], "pos-webhook")

    def test_activity_log_metadata_only(self):
        root = fresh_root(self)
        mk_adult_with_pos(root)
        poshook.ingest(root, "sam", push())
        with open(os.path.join(root, vault.ACTIVITY_FILE),
                  encoding="utf-8") as f:
            log = f.read()
        self.assertIn("receipt.pos", log)
        self.assertIn("Coffee Shop", log)
        self.assertIn("txn-001", log)
        self.assertNotIn('"items"', log)          # no payload echo
        self.assertNotIn("Latte", log)            # no item detail


if __name__ == "__main__":
    unittest.main()
