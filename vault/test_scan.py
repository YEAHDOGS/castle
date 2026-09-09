#!/usr/bin/env python3
"""Regression tests for the scan/OCR receipt ingester (scan.py).

Fixture-based: every test gets a fresh temp vault root; nothing touches
~/.castle-vault. Run:  python3 test_scan.py
"""

import json
import os
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import scan  # noqa: E402
import receipts  # noqa: E402
import vault  # noqa: E402


def fresh_root(tc):
    tmp = tempfile.mkdtemp(prefix="castle-scan-test-")
    tc.addCleanup(__import__("shutil").rmtree, tmp, True)
    return tmp


def mk_adult_with_scan(root, name="sam"):
    vault.create_user(root, name)
    vault.add_integration(root, name, "scan")
    return root


def fake_jpg(size=2048):
    # valid JPEG magic + padding — structure doesn't matter, magic does
    return b"\xff\xd8\xff\xe0" + b"\x00" * (size - 4)


def fake_png(size=2048):
    return b"\x89PNG\r\n\x1a\n" + b"\x00" * (size - 8)


def fake_webp(size=2048):
    return b"RIFF" + b"\x00" * 4 + b"WEBP" + b"\x00" * (size - 12)


OCR = ("CORNERSHOP MARKET\n"
       "123 Main St\n"
       "Milk 2%\n"
       "Bread\n"
       "TOTAL $14.82\n"
       "Thank you!\n")


class TestImageValidation(unittest.TestCase):
    def test_jpg_png_webp_accepted(self):
        self.assertEqual(scan.detect_image_kind(fake_jpg()), "jpg")
        self.assertEqual(scan.detect_image_kind(fake_png()), "png")
        self.assertEqual(scan.detect_image_kind(fake_webp()), "webp")

    def test_text_file_refused_as_image(self):
        self.assertIsNone(scan.detect_image_kind(b"hello world" * 200))

    def test_riff_without_webp_refused(self):
        self.assertIsNone(scan.detect_image_kind(b"RIFF" + b"\x00" * 2048))

    def test_ingest_refuses_non_image_bytes(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", b"definitely not an image" * 200, OCR)

    def test_ingest_refuses_tiny_stub(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", b"\xff\xd8\xff" + b"\x00" * 100, OCR)

    def test_ingest_refuses_empty_image(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", b"", OCR)


class TestOcrAndExtraction(unittest.TestCase):
    def test_happy_path(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        r = scan.ingest(root, "sam", fake_jpg(), OCR)
        self.assertEqual(r["source"], "scan")
        self.assertEqual(r["total"], "14.82")
        self.assertEqual(r["total_confidence"], "high")
        self.assertEqual(r["merchant"], "CORNERSHOP MARKET")
        self.assertEqual(r["currency"], "USD")

    def test_empty_ocr_text_refused(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_jpg(), "")
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_jpg(), "   \n  ")

    def test_no_total_means_unknown_not_zero(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        r = scan.ingest(root, "sam", fake_png(),
                        "SOME STORE\nhello there\n")
        self.assertIsNone(r["total"])
        self.assertEqual(r["total_confidence"], "none")

    def test_hint_merchant_takes_precedence(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        r = scan.ingest(root, "sam", fake_jpg(), OCR,
                        hint_merchant="CornerShop (stated)")
        self.assertEqual(r["merchant"], "CornerShop (stated)")
        self.assertEqual(r["merchant_confidence"], "high")

    def test_ocr_text_capped(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_jpg(), "x" * 200_000)


class TestAddressingAndTrust(unittest.TestCase):
    def test_unknown_user_refused(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        with self.assertRaises(ValueError):
            scan.ingest(root, "zed", fake_jpg(), OCR)

    def test_scan_never_lands_in_wrong_vault(self):
        root = fresh_root(self)
        mk_adult_with_scan(root, "sam")
        mk_adult_with_scan(root, "sally")
        scan.ingest(root, "sam", fake_jpg(), OCR)
        self.assertEqual(len(receipts.list_receipts(root, "sam")), 1)
        self.assertEqual(receipts.list_receipts(root, "sally"), [])
        with self.assertRaises(ValueError):
            scan.ingest(root, "zed", fake_jpg(), OCR)  # unknown: refused
        self.assertEqual(len(receipts.list_receipts(root, "sam")), 1)

    def test_requires_scan_integration(self):
        root = fresh_root(self)
        vault.create_user(root, "sam")  # adult, but no integration
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_jpg(), OCR)

    def test_child_cannot_enable_surface(self):
        root = fresh_root(self)
        vault.create_user(root, "mom")
        vault.create_user(root, "sally", tier="child", guardian="mom")
        with self.assertRaises(ValueError):  # add-integration is adult-only
            vault.add_integration(root, "sally", "scan")
        with self.assertRaises(ValueError):
            scan.ingest(root, "sally", fake_jpg(), OCR)

    def test_duplicate_scan_refused(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        scan.ingest(root, "sam", fake_jpg(), OCR)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_jpg(), OCR)
        self.assertEqual(len(receipts.list_receipts(root, "sam")), 1)

    def test_retake_with_different_text_is_new_record(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        scan.ingest(root, "sam", fake_jpg(), OCR)
        scan.ingest(root, "sam", fake_jpg(), OCR + "\nRESCAN\n")
        self.assertEqual(len(receipts.list_receipts(root, "sam")), 2)


class TestStorageSafety(unittest.TestCase):
    def test_stored_image_is_0600(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        r = scan.ingest(root, "sam", fake_jpg(), OCR)
        img = os.path.join(receipts._receipt_dir(root, "sam"),
                           r["id"], r["image_filename"])
        self.assertEqual(stat.S_IMODE(os.stat(img).st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(
            os.stat(os.path.dirname(img)).st_mode), 0o700)

    def test_path_traversal_filename_refused(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        for bad in ("../../evil.jpg", "..\\evil.jpg", "/etc/scan.jpg",
                    "sub/dir.jpg"):
            with self.assertRaises(ValueError, msg=bad):
                scan.ingest(root, "sam", fake_jpg(), OCR, filename=bad)

    def test_extension_kind_mismatch_refused(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_png(), OCR, filename="scan.jpg")

    def test_canary_no_directory_escape(self):
        # the canary: even an adversarial filename can never write
        # outside (or inside) the vault — the refusal happens before
        # any directory or file is created, so the whole tree is
        # byte-identical after the attempt.
        root = fresh_root(self)
        mk_adult_with_scan(root)
        before = []
        for dirpath, _dn, fn in os.walk(root):
            before.append((dirpath, sorted(fn)))
        before.sort()
        with self.assertRaises(ValueError):
            scan.ingest(root, "sam", fake_jpg(), OCR,
                        filename="../../../tmp/x.jpg")
        after = []
        for dirpath, _dn, fn in os.walk(root):
            after.append((dirpath, sorted(fn)))
        after.sort()
        self.assertEqual(before, after)

    def test_source_visible_in_shared_index(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        vault.add_integration(root, "sam", "pos-webhook")
        scan.ingest(root, "sam", fake_jpg(), OCR)
        rows = receipts.list_receipts(root, "sam")
        self.assertEqual(rows[0]["source"], "scan")

    def test_activity_log_carries_metadata_only(self):
        root = fresh_root(self)
        mk_adult_with_scan(root)
        img = fake_jpg()
        scan.ingest(root, "sam", img, OCR)
        log = open(os.path.join(root, "activity.jsonl")).read()
        self.assertIn("receipt.scan", log)
        self.assertIn("CORNERSHOP MARKET", log)
        self.assertNotIn("Thank you!", log)     # no OCR body in the log
        self.assertNotIn(img[:20].decode("latin1").split("\x00")[0], log)


if __name__ == "__main__":
    unittest.main()
