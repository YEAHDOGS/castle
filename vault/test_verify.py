#!/usr/bin/env python3
"""Regression tests for vault/verify.py. Temp dirs only."""

import json
import os
import sys
import tempfile
import unittest
from io import StringIO

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import manifest  # noqa: E402
import verify  # noqa: E402


def _setup():
    """A vault tree plus a manifest written for it. Returns (dir, mpath)."""
    d = tempfile.mkdtemp(prefix="castle-verify-")
    with open(os.path.join(d, "keep.txt"), "w") as f:
        f.write("keep me")
    with open(os.path.join(d, "change.txt"), "w") as f:
        f.write("original bytes")
    os.makedirs(os.path.join(d, "sub"))
    with open(os.path.join(d, "sub", "gone.txt"), "w") as f:
        f.write("doomed")
    mpath = os.path.join(tempfile.mkdtemp(prefix="castle-m-"), "m.json")
    manifest.write_manifest(d, mpath)
    return d, mpath


class TestAudit(unittest.TestCase):
    def test_round_trip_clean(self):
        d, mpath = _setup()
        doc = manifest.load_manifest(mpath)
        r = verify.audit(d, doc)
        self.assertTrue(r["clean"])
        self.assertEqual(r["added"], [])
        self.assertEqual(r["removed"], [])
        self.assertEqual(r["modified"], [])
        self.assertEqual(set(r["unchanged"]),
                         {"keep.txt", "change.txt", "sub/gone.txt"})

    def test_modified_file_detected(self):
        d, mpath = _setup()
        with open(os.path.join(d, "change.txt"), "w") as f:
            f.write("tampered bytes, same name")
        r = verify.audit(d, manifest.load_manifest(mpath))
        self.assertFalse(r["clean"])
        self.assertEqual(r["modified"], ["change.txt"])
        self.assertEqual(r["added"], [])
        self.assertEqual(r["removed"], [])

    def test_same_size_different_bytes_detected(self):
        # size alone can't hide tampering: same length, different bytes
        d, mpath = _setup()
        with open(os.path.join(d, "change.txt"), "wb") as f:
            f.write(b"ORIGINAL BYTES")  # 14 bytes, same as original
        r = verify.audit(d, manifest.load_manifest(mpath))
        self.assertEqual(r["modified"], ["change.txt"])

    def test_added_and_removed_detected(self):
        d, mpath = _setup()
        os.unlink(os.path.join(d, "sub", "gone.txt"))
        with open(os.path.join(d, "new.txt"), "w") as f:
            f.write("brand new")
        r = verify.audit(d, manifest.load_manifest(mpath))
        self.assertFalse(r["clean"])
        self.assertEqual(r["added"], ["new.txt"])
        self.assertEqual(r["removed"], ["sub/gone.txt"])
        self.assertEqual(r["modified"], [])

    def test_mtime_touch_alone_is_not_modified(self):
        d, mpath = _setup()
        p = os.path.join(d, "keep.txt")
        os.utime(p, (1_000_000_000, 1_000_000_000))
        r = verify.audit(d, manifest.load_manifest(mpath))
        self.assertTrue(r["clean"])

    def test_new_symlink_reported_as_skipped_not_added(self):
        d, mpath = _setup()
        os.symlink(os.path.join(d, "keep.txt"),
                   os.path.join(d, "new-link"))
        r = verify.audit(d, manifest.load_manifest(mpath))
        self.assertFalse(r["clean"])
        self.assertEqual(r["added"], [])
        self.assertEqual(r["skipped_new"], ["new-link"])

    def test_corrupt_manifest_is_a_refusal_not_a_clean_bill(self):
        d, _mpath = _setup()
        bad = os.path.join(tempfile.mkdtemp(prefix="castle-m-"), "bad.json")
        with open(bad, "w") as f:
            f.write("{corrupt")
        with self.assertRaises(manifest.ManifestError):
            verify.audit(d, manifest.load_manifest(bad))

    def test_human_readable_diff(self):
        d, mpath = _setup()
        with open(os.path.join(d, "change.txt"), "w") as f:
            f.write("tampered")
        with open(os.path.join(d, "new.txt"), "w") as f:
            f.write("new")
        os.unlink(os.path.join(d, "sub", "gone.txt"))
        r = verify.audit(d, manifest.load_manifest(mpath))
        out = verify.format_result(r)
        self.assertIn("+ new.txt", out)
        self.assertIn("- sub/gone.txt", out)
        self.assertIn("~ change.txt", out)

    def test_clean_report_says_clean(self):
        d, mpath = _setup()
        r = verify.audit(d, manifest.load_manifest(mpath))
        self.assertIn("clean", verify.format_result(r))


class TestVerifyCli(unittest.TestCase):
    def _run(self, *argv):
        old = sys.stdout
        sys.stdout = buf = StringIO()
        try:
            code = verify.main(list(argv))
        finally:
            sys.stdout = old
        return code, buf.getvalue()

    def test_cli_exit_0_when_clean(self):
        d, mpath = _setup()
        code, out = self._run(d, "--manifest", mpath)
        self.assertEqual(code, 0)
        self.assertIn("clean", out)

    def test_cli_exit_nonzero_with_diff_when_dirty(self):
        d, mpath = _setup()
        with open(os.path.join(d, "change.txt"), "w") as f:
            f.write("tampered")
        code, out = self._run(d, "--manifest", mpath)
        self.assertEqual(code, 2)
        self.assertIn("~ change.txt", out)

    def test_cli_corrupt_manifest_errors(self):
        d, _mpath = _setup()
        bad = os.path.join(tempfile.mkdtemp(prefix="castle-m-"), "bad.json")
        with open(bad, "w") as f:
            f.write("junk")
        old = sys.stderr
        sys.stderr = StringIO()
        try:
            code = verify.main([d, "--manifest", bad])
        finally:
            sys.stderr = old
        self.assertEqual(code, 1)

    def test_cli_json_output(self):
        d, mpath = _setup()
        code, out = self._run(d, "--manifest", mpath, "--json")
        self.assertEqual(code, 0)
        parsed = json.loads(out)
        self.assertTrue(parsed["clean"])


if __name__ == "__main__":
    unittest.main()
