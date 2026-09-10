#!/usr/bin/env python3
"""Regression tests for receipt metadata search (receiptsearch.py).

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. Search is read-only — tests assert it never opens
receipt bodies/attachments and writes nothing new. Run:
  python3 test_receiptsearch.py
"""

import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import receiptsearch  # noqa: E402
import receipts  # noqa: E402
import poshook  # noqa: E402
import scan  # noqa: E402
import vault  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-rsearch-test-")
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_user(root, name="sam"):
    vault.create_user(root, name)
    for integ in ("email-forward", "pos-webhook", "scan"):
        vault.add_integration(root, name, integ)


def mail(user, merchant, total_line, msgid):
    return (b"From: orders@example.com\r\n"
            b"To: receipts@%s.castle\r\n"
            b"Subject: Your receipt from %s\r\n"
            b"Message-ID: <%s@example.com>\r\n"
            b"Date: Tue, 08 Sep 2026 09:15:00 -0500\r\n"
            b"Content-Type: text/plain; charset=\"utf-8\"\r\n"
            b"\r\n"
            b"Thanks for your purchase!\r\n\r\n"
            b"%s\r\n"
            b"Keep this receipt.\r\n" % (
                user.encode(), merchant.encode(), msgid.encode(),
                total_line.encode()))


def pos_push(user, merchant, total, txn):
    return json.dumps({
        "user": user, "merchant": merchant, "total": total,
        "currency": "USD", "pos_vendor": "clover",
        "transaction_id": txn}).encode()


def fake_jpg(size=2048):
    return b"\xff\xd8\xff\xe0" + b"\x00" * (size - 4)


def seed(root, user="sam"):
    """One email ($7.75 coffee), one POS ($45.00 tire shop),
    one scan ($12.00 book store) — all in one vault."""
    mk_user(root, user)
    receipts.ingest(root, user, mail(user, "Coffee Shop", "Total: $7.75",
                                    "mail-coffee-1"))
    poshook.ingest(root, user, pos_push(user, "Tire World", "45.00",
                                       "txn-tire-1"))
    scan.ingest(root, user, fake_jpg(), "BOOK STORE\nTotal: $12.00\n",
                filename="scan1.jpg", hint_merchant="Book Store")


class TestUnfiltered(unittest.TestCase):
    def test_returns_all_three(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam")
        self.assertEqual(len(got), 3)
        self.assertEqual(sorted(e["merchant"] for e in got),
                         ["Book Store", "Coffee Shop", "Tire World"])

    def test_metadata_only_never_bodies(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam")
        for e in got:
            blob = json.dumps(e)
            self.assertNotIn("purchase", blob)
            self.assertNotIn("Keep this receipt", blob)
            self.assertNotIn("Latte", blob)

    def test_unknown_user_refused(self):
        root = fresh_root(self)
        seed(root)
        with self.assertRaises(ValueError):
            receiptsearch.search(root, "nobody")

    def test_empty_vault_empty_list(self):
        root = fresh_root(self)
        mk_user(root)
        self.assertEqual(receiptsearch.search(root, "sam"), [])


class TestMerchantFilter(unittest.TestCase):
    def test_substring_case_insensitive(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam", merchant="tire")
        self.assertEqual([e["merchant"] for e in got], ["Tire World"])
        got = receiptsearch.search(root, "sam", merchant="BOOK")
        self.assertEqual([e["merchant"] for e in got], ["Book Store"])
        got = receiptsearch.search(root, "sam", merchant="shop")
        self.assertEqual([e["merchant"] for e in got], ["Coffee Shop"])

    def test_no_match_empty(self):
        root = fresh_root(self)
        seed(root)
        self.assertEqual(receiptsearch.search(root, "sam", merchant="zzz"), [])


class TestTotalFilters(unittest.TestCase):
    def test_min_total(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam", min_total="10.00")
        self.assertEqual(sorted(e["merchant"] for e in got),
                         ["Book Store", "Tire World"])

    def test_max_total(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam", max_total="12.00")
        self.assertEqual(sorted(e["merchant"] for e in got),
                         ["Book Store", "Coffee Shop"])

    def test_range(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam", min_total="10.00",
                                   max_total="20.00")
        self.assertEqual([e["merchant"] for e in got], ["Book Store"])

    def test_exact_decimal_string_not_float(self):
        root = fresh_root(self)
        seed(root)
        # float total must be rejected at the filter, not rounded
        with self.assertRaises(ValueError):
            receiptsearch.search(root, "sam", min_total=7.75)

    def test_garbage_total_refused(self):
        root = fresh_root(self)
        seed(root)
        with self.assertRaises(ValueError):
            receiptsearch.search(root, "sam", max_total="lots")

    def test_min_gt_max_refused(self):
        root = fresh_root(self)
        seed(root)
        with self.assertRaises(ValueError):
            receiptsearch.search(root, "sam", min_total="50", max_total="5")

    def test_unknown_total_never_matches_total_filter(self):
        root = fresh_root(self)
        mk_user(root)
        # email with no parseable total -> total None in index
        receipts.ingest(root, "sam",
                        mail("sam", "Mystery Mart", "thanks for visiting",
                             "mail-mystery-1"))
        rows = [e for e in receiptsearch.search(root, "sam")
                if e["merchant"] == "Mystery Mart"]
        self.assertEqual(len(rows), 1)
        self.assertIsNone(rows[0]["total"])
        self.assertEqual(receiptsearch.search(root, "sam", min_total="0"), [])
        self.assertEqual(receiptsearch.search(root, "sam", max_total="999"), [])


class TestDateFilters(unittest.TestCase):
    def test_since_until(self):
        root = fresh_root(self)
        seed(root)
        # everything ingested today; wide window matches, narrow misses
        got = receiptsearch.search(root, "sam", since="2026-01-01",
                                   until="2026-12-31")
        self.assertEqual(len(got), 3)
        got = receiptsearch.search(root, "sam", since="2027-01-01")
        self.assertEqual(got, [])
        got = receiptsearch.search(root, "sam", until="2025-12-31")
        self.assertEqual(got, [])

    def test_bad_date_refused(self):
        root = fresh_root(self)
        seed(root)
        with self.assertRaises(ValueError):
            receiptsearch.search(root, "sam", since="2026/09/01")
        with self.assertRaises(ValueError):
            receiptsearch.search(root, "sam", since="2027-01-01",
                                 until="2026-01-01")


class TestSourceFilter(unittest.TestCase):
    def test_source_exact(self):
        root = fresh_root(self)
        seed(root)
        self.assertEqual(
            [e["merchant"] for e in receiptsearch.search(root, "sam",
                                                        source="pos-webhook")],
            ["Tire World"])
        self.assertEqual(
            [e["merchant"] for e in receiptsearch.search(root, "sam",
                                                        source="scan")],
            ["Book Store"])

    def test_legacy_email_normalizes_to_email(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam", source="email")
        self.assertEqual([e["merchant"] for e in got], ["Coffee Shop"])

    def test_combined_filters_and(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam", merchant="shop",
                                   max_total="10.00", source="email")
        self.assertEqual([e["merchant"] for e in got], ["Coffee Shop"])
        got = receiptsearch.search(root, "sam", merchant="shop",
                                   min_total="10.00", source="email")
        self.assertEqual(got, [])


class TestLimitsAndReadonly(unittest.TestCase):
    def test_limit(self):
        root = fresh_root(self)
        seed(root)
        got = receiptsearch.search(root, "sam", limit=2)
        self.assertEqual(len(got), 2)
        with self.assertRaises(ValueError):
            receiptsearch.search(root, "sam", limit=0)

    def test_search_writes_nothing(self):
        root = fresh_root(self)
        seed(root)
        before = set()
        for dp, _dn, fn in os.walk(root):
            for f in fn:
                before.add(os.path.join(dp, f))
        receiptsearch.search(root, "sam", merchant="shop")
        after = set()
        for dp, _dn, fn in os.walk(root):
            for f in fn:
                after.add(os.path.join(dp, f))
        self.assertEqual(before, after)

    def test_corrupt_index_line_skipped(self):
        root = fresh_root(self)
        seed(root)
        p = receipts._index_path(root, "sam")
        with open(p, "a", encoding="utf-8") as f:
            f.write("THIS IS NOT JSON\n")
        got = receiptsearch.search(root, "sam")
        self.assertEqual(len(got), 3)

    def test_sally_cannot_search_sams_vault(self):
        root = fresh_root(self)
        seed(root, "sam")
        mk_user(root, "sally")
        self.assertEqual(receiptsearch.search(root, "sally"), [])


class TestCLI(unittest.TestCase):
    def test_search_json(self):
        root = fresh_root(self)
        seed(root)
        out = json.loads(receiptsearch.search_json(root, "sam",
                                                   merchant="tire"))
        self.assertEqual([e["merchant"] for e in out], ["Tire World"])


if __name__ == "__main__":
    unittest.main()
