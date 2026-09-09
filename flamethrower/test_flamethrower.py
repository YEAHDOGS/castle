#!/usr/bin/env python3
"""Regression tests for flamethrower/flamethrower.py — the unified
true-deletion CLI (FLAMETHROWER.md build-order step 9).

Fakes EVERYTHING: fake sysfs tree (with dev/block/<maj>:<min> symlinks),
fake st_dev resolver, fake stdin. Target files live in temp dirs; the
fake sysfs maps them onto fake disks so no test touches real hardware,
and no test shreds a real file outside temp dirs.
Run: python3 test_flamethrower.py
"""

import io
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import audit
import flamethrower
import media


# ---------------------------------------------------------------------------
# fakes (same shape as test_tier3.py — the tests must stand alone)
# ---------------------------------------------------------------------------

class FakeSysfs:
    """Fake /sys with block disks and dev/block/<maj>:<min> symlinks."""

    def __init__(self):
        self.tmp = tempfile.mkdtemp(prefix="fake-sys-")
        self.root = os.path.join(self.tmp, "sys")
        os.makedirs(os.path.join(self.root, "block"))
        os.makedirs(os.path.join(self.root, "dev", "block"))
        self._devs = {}
        self._next_minor = 1

    def add_disk(self, name, rotational=True, removable=False,
                 serial="FAKESERIAL1", model="FAKE"):
        d = os.path.join(self.root, "block", name)
        os.makedirs(os.path.join(d, "queue"))
        os.makedirs(os.path.join(d, "device"))
        if rotational is not None:
            with open(os.path.join(d, "queue", "rotational"), "w") as f:
                f.write("1" if rotational else "0")
        with open(os.path.join(d, "removable"), "w") as f:
            f.write("1" if removable else "0")
        with open(os.path.join(d, "device", "model"), "w") as f:
            f.write(model)
        with open(os.path.join(d, "device", "serial"), "w") as f:
            f.write(serial)
        major, minor = 8, self._next_minor
        self._next_minor += 1
        os.symlink("../../block/%s" % name,
                   os.path.join(self.root, "dev", "block",
                                "%d:%d" % (major, minor)))
        self._devs[name] = (major, minor)
        return major, minor

    def stat_dev_for(self, name):
        """A stat_dev callable mapping every path to this disk's dev."""
        major, minor = self._devs[name]
        return lambda path: (major, minor)

    def close(self):
        shutil.rmtree(self.tmp, ignore_errors=True)


class FakeStdin:
    def __init__(self, lines, tty=True):
        self.lines = list(lines)
        self._tty = tty

    def isatty(self):
        return self._tty

    def readline(self):
        return self.lines.pop(0) + "\n" if self.lines else ""


class Ctx:
    """Per-test sandbox: fake sysfs (HDD disk), target dir, log dir."""

    def __init__(self, test):
        self.sysfs = FakeSysfs()
        self.sysfs.add_disk("sdhdd", rotational=True)
        self.tmp = tempfile.mkdtemp(prefix="flamethrower-test-")
        self.targets = os.path.join(self.tmp, "targets")
        self.logs = os.path.join(self.tmp, "logs")
        self.keyring = os.path.join(self.tmp, "keyring")
        os.makedirs(self.targets)
        test.addCleanup(shutil.rmtree, self.tmp, True)
        test.addCleanup(self.sysfs.close)

    def target(self, name="secret.txt", data=b"SUPER-SECRET-BYTES-" * 64):
        path = os.path.join(self.targets, name)
        with open(path, "wb") as f:
            f.write(data)
        return path, data

    def kwargs(self):
        return dict(sysfs=self.sysfs.root,
                    stat_dev=self.sysfs.stat_dev_for("sdhdd"),
                    stdin=FakeStdin([], tty=False),
                    keyring_root=self.keyring,
                    log_dir=self.logs,
                    no_countdown=True)


# ---------------------------------------------------------------------------
# tests
# ---------------------------------------------------------------------------

class BurnTests(unittest.TestCase):

    def test_dry_run_is_default_and_never_touches_bytes(self):
        ctx = Ctx(self)
        path, data = ctx.target()
        rc, result = flamethrower.burn([path], dry_run=True, **ctx.kwargs())
        self.assertEqual(rc, 2)
        self.assertIn("targets", result)
        # the plan enumerates the target honestly
        self.assertEqual(result["targets"][0]["file"],
                         os.path.realpath(path))
        with open(path, "rb") as f:
            self.assertEqual(f.read(), data, "dry-run modified bytes!")

    def test_real_burn_requires_i_understand(self):
        ctx = Ctx(self)
        path, data = ctx.target()
        # correct typed confirmation but no --i-understand -> still dry-run
        rc, result = flamethrower.burn(
            [path], confirm_arg="secret.txt", i_understand=False,
            dry_run=False, **ctx.kwargs())
        self.assertEqual(rc, 2)
        self.assertTrue(result.get("aborted"))
        self.assertIn("--i-understand", result["note"])
        with open(path, "rb") as f:
            self.assertEqual(f.read(), data,
                             "burn ran without --i-understand!")

    def test_confirmation_mismatch_aborts_and_spares_bytes(self):
        ctx = Ctx(self)
        path, data = ctx.target()
        rc, result = flamethrower.burn(
            [path], confirm_arg="wrong-name.txt", i_understand=True,
            dry_run=False, **ctx.kwargs())
        self.assertEqual(rc, 2)
        self.assertTrue(result.get("aborted"))
        with open(path, "rb") as f:
            self.assertEqual(f.read(), data,
                             "burn ran on mismatched confirmation!")

    def test_no_confirm_on_non_tty_refuses(self):
        ctx = Ctx(self)
        path, data = ctx.target()
        rc, result = flamethrower.burn(
            [path], confirm_arg=None, i_understand=True, dry_run=False,
            **ctx.kwargs())
        self.assertEqual(rc, 1)  # tier3 refusal: must type on a real console
        self.assertTrue(result.get("aborted"))
        with open(path, "rb") as f:
            self.assertEqual(f.read(), data)

    def test_real_burn_destroys_file_and_writes_audit_record(self):
        ctx = Ctx(self)
        path, data = ctx.target()
        rc, result = flamethrower.burn(
            [path], confirm_arg="secret.txt", i_understand=True,
            dry_run=False, **ctx.kwargs())
        self.assertEqual(rc, 0)
        self.assertFalse(os.path.lexists(path), "target survived the burn")
        self.assertEqual(len(result["certificates"]), 1)
        cert = result["certificates"][0]
        self.assertEqual(cert["overwrite_passes"], 3)
        # one audit record per certificate, appended to <root>/audit.jsonl
        audit_file = os.path.join(ctx.logs, audit.AUDIT_FILE)
        self.assertTrue(os.path.isfile(audit_file))
        with open(audit_file, encoding="utf-8") as f:
            entries = [json.loads(line) for line in f if line.strip()]
        self.assertEqual(len(entries), 1)
        entry = entries[0]
        self.assertEqual(entry["tier"], "3")
        self.assertEqual(entry["cert_id"], cert["cert_id"])
        self.assertEqual(entry["fingerprint"]["file"], os.path.realpath(path))
        self.assertTrue(entry["success"])
        # the hash-chained log verifies clean
        report = audit.verify(ctx.logs)
        self.assertTrue(report["valid"], "audit chain broken: %s" % report)

    def test_multipass_overwrite_performs_every_pass(self):
        seen = []
        log = lambda msg: seen.append(msg)
        path = tempfile.NamedTemporaryFile(delete=False).name
        self.addCleanup(os.unlink, path)
        with open(path, "wb") as f:
            f.write(os.urandom(4096))
        flamethrower.overwrite_multipass(
            path, 4096, passes=3, log=log,
            rng_bytes=lambda n: b"\xab" * n)
        passes = [m for m in seen if m.strip().startswith("pass ")]
        self.assertEqual(len(passes), 3)
        self.assertIn("CSPRNG", passes[0])
        self.assertIn("CSPRNG", passes[1])
        self.assertIn("zeros", passes[2])
        # final pass is zeros
        with open(path, "rb") as f:
            self.assertEqual(f.read(), b"\x00" * 4096)

    def test_overwrite_requires_at_least_one_pass(self):
        path = tempfile.NamedTemporaryFile(delete=False).name
        self.addCleanup(os.unlink, path)
        with self.assertRaises(ValueError):
            flamethrower.overwrite_multipass(path, 8, passes=0, log=None)

    def test_cli_dry_run_lists_targets_without_destroying(self):
        # read-only against the REAL sysfs: proves the CLI path never
        # arms on plain `burn <file>` — no fake hardware involved.
        # On tmpfs the media mapping itself refuses (rc 1); on block
        # storage the dry-run lists the plan (rc 2). Both are safe.
        tmp = tempfile.mkdtemp(prefix="flamethrower-cli-")
        self.addCleanup(shutil.rmtree, tmp, True)
        path = os.path.join(tmp, "note.txt")
        data = b"hello"
        with open(path, "wb") as f:
            f.write(data)
        rc = flamethrower.main(["burn", path])
        self.assertIn(rc, (1, 2))
        with open(path, "rb") as f:
            self.assertEqual(f.read(), data)

    def test_cli_wrong_confirm_aborts(self):
        tmp = tempfile.mkdtemp(prefix="flamethrower-cli-")
        self.addCleanup(shutil.rmtree, tmp, True)
        path = os.path.join(tmp, "note.txt")
        data = b"hello"
        with open(path, "wb") as f:
            f.write(data)
        rc = flamethrower.main(
            ["burn", path, "--i-understand", "--confirm", "nope.txt",
             "--no-countdown"])
        self.assertIn(rc, (1, 2))  # abort, or media refusal before arming
        with open(path, "rb") as f:
            self.assertEqual(f.read(), data)


if __name__ == "__main__":
    unittest.main(verbosity=2)
