#!/usr/bin/env python3
"""Regression tests for vault/manifest.py. Temp dirs only."""

import json
import os
import stat
import sys
import tempfile
import unittest

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import manifest  # noqa: E402


def _tree():
    d = tempfile.mkdtemp(prefix="castle-manifest-")
    os.makedirs(os.path.join(d, "photos", "2024"))
    with open(os.path.join(d, "notes.txt"), "w") as f:
        f.write("hello vault")
    with open(os.path.join(d, "photos", "2024", "beach.jpg"), "wb") as f:
        f.write(b"\xff\xd8\xff" + b"junk-bytes" * 100)
    return d


class TestBuildManifest(unittest.TestCase):
    def test_build_lists_files_with_paths_sizes_hashes_mtimes(self):
        d = _tree()
        doc = manifest.build_manifest(d)
        self.assertEqual(doc["format"], "castle-vault-manifest/1")
        by_path = {e["path"]: e for e in doc["entries"]}
        self.assertEqual(set(by_path), {"notes.txt", "photos/2024/beach.jpg"})
        for e in doc["entries"]:
            self.assertTrue(isinstance(e["size"], int) and e["size"] > 0)
            self.assertEqual(len(e["sha256"]), 64)
            self.assertTrue(isinstance(e["mtime_ns"], int))
        # hash of notes.txt must equal sha256 of its bytes
        import hashlib
        self.assertEqual(by_path["notes.txt"]["sha256"],
                         hashlib.sha256(b"hello vault").hexdigest())

    def test_build_is_deterministic(self):
        d = _tree()
        self.assertEqual(manifest.build_manifest(d), manifest.build_manifest(d))

    def test_empty_dir_builds_empty_manifest(self):
        d = tempfile.mkdtemp(prefix="castle-manifest-")
        doc = manifest.build_manifest(d)
        self.assertEqual(doc["entries"], [])

    def test_symlinks_are_skipped_never_followed(self):
        d = _tree()
        os.symlink(os.path.join(d, "notes.txt"),
                   os.path.join(d, "link-to-notes"))
        outside = tempfile.mkdtemp(prefix="castle-outside-")
        with open(os.path.join(outside, "evil.txt"), "w") as f:
            f.write("not in the vault")
        os.symlink(outside, os.path.join(d, "photos", "linkdir"))
        doc = manifest.build_manifest(d)
        by_path = {e["path"]: e for e in doc["entries"]}
        self.assertNotIn("link-to-notes", by_path)
        self.assertNotIn("photos/linkdir", by_path)
        self.assertNotIn("photos/linkdir/evil.txt", by_path)
        skipped_paths = {s["path"] for s in doc["skipped"]}
        self.assertIn("link-to-notes", skipped_paths)

    def test_symlinked_root_is_refused(self):
        d = _tree()
        link = d + "-link"
        os.symlink(d, link)
        with self.assertRaises(manifest.ManifestError):
            manifest.build_manifest(link)

    def test_nonexistent_and_file_targets_refused(self):
        d = _tree()
        with self.assertRaises(manifest.ManifestError):
            manifest.build_manifest(os.path.join(d, "nope"))
        with self.assertRaises(manifest.ManifestError):
            manifest.build_manifest(os.path.join(d, "notes.txt"))

    def test_filesystem_root_refused(self):
        with self.assertRaises(manifest.ManifestError):
            manifest.build_manifest("/")


class TestWriteLoadManifest(unittest.TestCase):
    def test_round_trip(self):
        d = _tree()
        mpath = os.path.join(tempfile.mkdtemp(prefix="castle-m-"), "m.json")
        doc = manifest.write_manifest(d, mpath)
        loaded = manifest.load_manifest(mpath)
        self.assertEqual(loaded["entries"], doc["entries"])
        self.assertEqual(loaded["format"], "castle-vault-manifest/1")

    def test_manifest_is_0600(self):
        d = _tree()
        mpath = os.path.join(tempfile.mkdtemp(prefix="castle-m-"), "m.json")
        manifest.write_manifest(d, mpath)
        mode = stat.S_IMODE(os.stat(mpath).st_mode)
        self.assertEqual(mode, 0o600)

    def test_manifest_inside_target_tree_refused(self):
        d = _tree()
        with self.assertRaises(manifest.ManifestError):
            manifest.write_manifest(d, os.path.join(d, "manifest.json"))
        with self.assertRaises(manifest.ManifestError):
            manifest.write_manifest(d, os.path.join(d, "photos", "m.json"))

    def test_missing_file_refused(self):
        with self.assertRaises(manifest.ManifestError):
            manifest.load_manifest(os.path.join(
                tempfile.mkdtemp(prefix="castle-m-"), "nope.json"))

    def test_garbage_json_refused(self):
        mpath = os.path.join(tempfile.mkdtemp(prefix="castle-m-"), "m.json")
        with open(mpath, "w") as f:
            f.write("this is not json{{{")
        with self.assertRaises(manifest.ManifestError):
            manifest.load_manifest(mpath)

    def test_wrong_shape_refused(self):
        bad_docs = [
            [1, 2, 3],
            {"format": "wrong/9", "entries": []},
            {"format": "castle-vault-manifest/1"},  # no entries
            {"format": "castle-vault-manifest/1", "entries": {}},
            {"format": "castle-vault-manifest/1", "entries": [
                {"path": "a.txt", "size": -1, "sha256": "x" * 64}]},
            {"format": "castle-vault-manifest/1", "entries": [
                {"path": "a.txt", "size": 1, "sha256": "nothex" + "0" * 58}]},
            {"format": "castle-vault-manifest/1", "entries": [
                {"path": "a.txt", "size": 1, "sha256": "0" * 64},
                {"path": "a.txt", "size": 1, "sha256": "0" * 64}]},  # dup
            {"format": "castle-vault-manifest/1", "entries": [
                {"path": "../escape.txt", "size": 1, "sha256": "0" * 64}]},
            {"format": "castle-vault-manifest/1", "entries": [
                {"path": "/absolute.txt", "size": 1, "sha256": "0" * 64}]},
        ]
        for i, bad in enumerate(bad_docs):
            mpath = os.path.join(tempfile.mkdtemp(prefix="castle-m-"), "m.json")
            with open(mpath, "w") as f:
                json.dump(bad, f)
            with self.assertRaises(manifest.ManifestError, msg="doc %d" % i):
                manifest.load_manifest(mpath)


if __name__ == "__main__":
    unittest.main()
