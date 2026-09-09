#!/usr/bin/env python3
"""Regression tests for email-forward receipt ingestion (receipts.py).

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. Run:  python3 test_receipts.py
"""

import json
import os
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import receipts  # noqa: E402
import vault  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-receipts-test-")
    tc.addCleanup(__import__("shutil").rmtree, tmp, True)
    return tmp


def mk_adult_with_forward(root, name="sam"):
    vault.create_user(root, name)
    vault.add_integration(root, name, "email-forward")
    return root


MAIL = b"""From: orders@coffeeshop.example.com
To: receipts@sam.castle
Subject: Your receipt from Coffee Shop
Message-ID: <abc123@coffeeshop.example.com>
Date: Tue, 08 Sep 2026 09:15:00 -0500
Content-Type: text/plain; charset="utf-8"

Thanks for your purchase!

Latte              $4.50
Muffin             $3.25

Total: $7.75
Card charged: $7.75

Keep this receipt for your records.
"""

MAIL_WITH_ATTACHMENT = (
    b"From: tickets@airline.example.com\r\n"
    b"To: receipts@sam.castle\r\n"
    b"Subject: Your e-ticket receipt\r\n"
    b"Message-ID: <tix-999@airline.example.com>\r\n"
    b"MIME-Version: 1.0\r\n"
    b'Content-Type: multipart/mixed; boundary="BOUND"\r\n'
    b"\r\n"
    b"--BOUND\r\n"
    b'Content-Type: text/plain; charset="utf-8"\r\n'
    b"\r\n"
    b"Flight AA100. Grand Total: $212.40\r\n"
    b"\r\n"
    b"--BOUND\r\n"
    b'Content-Type: application/pdf; name="eticket.pdf"\r\n'
    b"Content-Transfer-Encoding: base64\r\n"
    b'Content-Disposition: attachment; filename="eticket.pdf"\r\n'
    b"\r\n"
    b"JVBERi0xLjQK\r\n"
    b"--BOUND--\r\n"
)


class TestAddressing(unittest.TestCase):
    def test_resolve_recipient_ok(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        user, _msg = receipts.resolve_recipient(root, MAIL)
        self.assertEqual(user, "sam")

    def test_misaddressed_mail_refused(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        bad = MAIL.replace(b"receipts@sam.castle", b"sam@gmail.com")
        with self.assertRaises(ValueError):
            receipts.resolve_recipient(root, bad)

    def test_unknown_user_refused(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        bad = MAIL.replace(b"receipts@sam.castle", b"receipts@zed.castle")
        with self.assertRaises(ValueError):
            receipts.resolve_recipient(root, bad)

    def test_receipt_never_lands_in_wrong_vault(self):
        root = fresh_root(self)
        mk_adult_with_forward(root, "sam")
        mk_adult_with_forward(root, "sally")
        # mail addressed to sam, ingested by sally: refused
        with self.assertRaises(ValueError):
            receipts.ingest(root, "sally", MAIL)
        self.assertEqual(receipts.list_receipts(root, "sally"), [])
        # address says sam — resolving lands on sam
        user, _ = receipts.resolve_recipient(root, MAIL)
        self.assertEqual(user, "sam")


class TestIngestRefusals(unittest.TestCase):
    def test_requires_email_forward_integration(self):
        root = fresh_root(self)
        vault.create_user(root, "sam")  # adult, but no integration
        with self.assertRaises(ValueError):
            receipts.ingest(root, "sam", MAIL)

    def test_child_cannot_enable_surface(self):
        root = fresh_root(self)
        vault.create_user(root, "mom")
        vault.create_user(root, "sally", tier="child", guardian="mom")
        with self.assertRaises(ValueError):  # add-integration is adult-only
            vault.add_integration(root, "sally", "email-forward")
        with self.assertRaises(ValueError):
            receipts.ingest(root, "sally", MAIL)

    def test_empty_message_refused(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        with self.assertRaises(ValueError):
            receipts.ingest(root, "sam", b"   ")
        with self.assertRaises(ValueError):
            receipts.ingest(root, "sam", b"")

    def test_duplicate_forward_is_idempotent(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        r1 = receipts.ingest(root, "sam", MAIL)
        with self.assertRaises(ValueError):
            receipts.ingest(root, "sam", MAIL)
        self.assertEqual(len(receipts.list_receipts(root, "sam")), 1)
        self.assertEqual(receipts.get_receipt(root, "sam", r1["id"])["id"],
                         r1["id"])


class TestExtractionAndStorage(unittest.TestCase):
    def test_structured_fields_extracted(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        r = receipts.ingest(root, "sam", MAIL)
        self.assertEqual(r["merchant"], "Coffee Shop")
        self.assertEqual(r["total"], "7.75")
        self.assertEqual(r["total_confidence"], "high")
        self.assertEqual(r["currency"], "USD")
        self.assertTrue(r["content_hash"])
        self.assertEqual(r["message_id"],
                         "<abc123@coffeeshop.example.com>")

    def test_attachment_stored_inside_user_vault(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        r = receipts.ingest(root, "sam", MAIL_WITH_ATTACHMENT)
        self.assertEqual(r["total"], "212.40")
        self.assertEqual(len(r["attachments"]), 1)
        stored = os.path.join(root, "vaults", "sam", "receipts", r["id"],
                              r["attachments"][0]["stored_as"])
        self.assertTrue(os.path.isfile(stored), stored)
        self.assertEqual(stat.S_IMODE(os.stat(stored).st_mode), 0o600)

    def test_files_are_tight_and_original_kept(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        r = receipts.ingest(root, "sam", MAIL)
        rdir = os.path.join(root, "vaults", "sam", "receipts", r["id"])
        self.assertEqual(stat.S_IMODE(os.stat(rdir).st_mode), 0o700)
        for name in ("receipt.json", "original.eml"):
            p = os.path.join(rdir, name)
            self.assertTrue(os.path.isfile(p), p)
            self.assertEqual(stat.S_IMODE(os.stat(p).st_mode), 0o600)
        with open(os.path.join(rdir, "original.eml"), "rb") as f:
            self.assertEqual(f.read(), MAIL)

    def test_activity_log_never_carries_body_or_attachments(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        receipts.ingest(root, "sam", MAIL_WITH_ATTACHMENT)
        with open(os.path.join(root, vault.ACTIVITY_FILE),
                  encoding="utf-8") as f:
            log = f.read()
        self.assertIn("receipt.ingested", log)
        self.assertNotIn("Flight AA100", log)      # body text
        self.assertNotIn("JVBERi0xLjQK", log)      # attachment bytes

    def test_get_receipt_bad_id_refused(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        receipts.ingest(root, "sam", MAIL)
        with self.assertRaises(ValueError):
            receipts.get_receipt(root, "sam", "../users")
        with self.assertRaises(ValueError):
            receipts.get_receipt(root, "sam", "rcpt-nope")

    def test_list_receipts_round_trip(self):
        root = fresh_root(self)
        mk_adult_with_forward(root)
        receipts.ingest(root, "sam", MAIL)
        receipts.ingest(root, "sam", MAIL_WITH_ATTACHMENT)
        lst = receipts.list_receipts(root, "sam")
        self.assertEqual(len(lst), 2)
        merchants = {e["merchant"] for e in lst}
        self.assertIn("Coffee Shop", merchants)
        self.assertIn("tickets@airline.example.com", merchants)
        # each summary matches its stored record
        for e in lst:
            full = receipts.get_receipt(root, "sam", e["id"])
            self.assertEqual(full["content_hash"], e["content_hash"])


if __name__ == "__main__":
    unittest.main()
