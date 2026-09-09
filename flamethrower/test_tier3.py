#!/usr/bin/env python3
"""Regression tests for flamethrower/tier3.py — Tier-3 file shredder.

Fakes EVERYTHING: fake sysfs tree (with dev/block/<maj>:<min> symlinks),
fake st_dev resolver, fake stdin. Target files live in temp dirs; the
fake sysfs maps them onto fake disks so no test touches real hardware or
real files outside temp dirs.
Run: python3 test_tier3.py
"""

import io
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import media
import tier3


# ---------------------------------------------------------------------------
# fakes
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

    def add_disk(self, name, rotational=False, removable=False,
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
    """Per-test sandbox: fake sysfs, target dir, log dir, keyring dir."""

    def __init__(self, test):
        self.sysfs = FakeSysfs()
        self.tmp = tempfile.mkdtemp(prefix="tier3-test-")
        self.targets = os.path.join(self.tmp, "targets")
        self.logs = os.path.join(self.tmp, "logs")
        self.keyring = os.path.join(self.tmp, "keyring")
        os.makedirs(self.targets)
        os.makedirs(self.keyring)
        test.addCleanup(self.close)

    def close(self):
        self.sysfs.close()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def mkfile(self, name, data=b"x" * 5000):
        p = os.path.join(self.targets, name)
        with open(p, "wb") as f:
            f.write(data)
        return p

    def kwargs(self, disk, **kw):
        major, minor = self.sysfs.add_disk(disk, **kw.pop("disk_kw", {}))
        d = {"sysfs": self.sysfs.root,
             "stat_dev": lambda path: (major, minor),
             "keyring_root": self.keyring,
             "log_dir": self.logs,
             "no_countdown": True}
        d.update(kw)
        return d


# ---------------------------------------------------------------------------
# planning & honest labeling
# ---------------------------------------------------------------------------

class TestPlanning(unittest.TestCase):
    def setUp(self):
        self.c = Ctx(self)

    def plan_one(self, name, disk, content=b"data", disk_kw=None):
        p = self.c.mkfile(name, content)
        kw = self.c.kwargs(disk, disk_kw=disk_kw or {})
        targets = tier3.enumerate_targets([p], **{k: v for k, v in
            kw.items() if k in ("sysfs", "stat_dev", "keyring_root")})
        plans = tier3.plan_shred(targets)
        return p, targets[0], plans[0]

    def test_hdd_plan_is_a_real_kill(self):
        p, t, plan = self.plan_one("a.txt", "sda", disk_kw={"rotational": True})
        self.assertEqual(t.info.media, media.HDD)
        self.assertFalse(t.flash)
        self.assertIn("real kill", plan["method"])
        self.assertNotIn("BEST EFFORT", plan["method"])
        self.assertEqual([s["name"] for s in plan["steps"]],
                         ["overwrite-extents", "read-back-verify",
                          "rename-random", "truncate-unlink"])

    def test_sata_ssd_plan_is_best_effort(self):
        p, t, plan = self.plan_one("a.txt", "sdc",
                                   disk_kw={"rotational": False})
        self.assertEqual(t.info.media, media.SATA_SSD)
        self.assertTrue(t.flash)
        self.assertIn("BEST EFFORT", plan["method"])
        self.assertIn("crypto-shred", plan["method"])

    def test_nvme_plan_is_best_effort(self):
        p, t, plan = self.plan_one("a.txt", "nvme0n1")
        self.assertEqual(t.info.media, media.NVME)
        self.assertIn("BEST EFFORT", plan["method"])

    def test_usb_plan_is_best_effort(self):
        p, t, plan = self.plan_one("a.txt", "sdb",
                                   disk_kw={"removable": True})
        self.assertEqual(t.info.media, media.FLASH_USB)
        self.assertIn("BEST EFFORT", plan["method"])

    def test_unknown_media_assumes_flash(self):
        p, t, plan = self.plan_one("a.txt", "sdd",
                                   disk_kw={"rotational": None})
        self.assertEqual(t.info.media, media.UNKNOWN)
        self.assertTrue(t.flash)
        self.assertIn("BEST EFFORT", plan["method"])

    def test_virtual_media_is_best_effort(self):
        p, t, plan = self.plan_one("a.txt", "vda")
        self.assertEqual(t.info.media, media.VIRTUAL)
        self.assertIn("BEST EFFORT", plan["method"])

    def test_flash_target_carries_honest_label(self):
        p, t, plan = self.plan_one("a.txt", "sdc",
                                   disk_kw={"rotational": False})
        self.assertIn("best effort", t.as_dict()["honest_label"])
        self.assertIn("crypto-shred", t.as_dict()["guarantee"])

    def test_plan_is_read_only(self):
        data = b"untouched-bytes-123"
        p, t, plan = self.plan_one("a.txt", "sda",
                                   disk_kw={"rotational": True},
                                   content=data)
        with open(p, "rb") as f:
            self.assertEqual(f.read(), data)

    def test_cli_plan_is_read_only_json(self):
        p = self.c.mkfile("cli.txt", b"zzz")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        out = io.StringIO()
        old = sys.stdout
        sys.stdout = out
        try:
            rc = tier3.main(["--sysfs", kw["sysfs"], "plan", p])
        finally:
            sys.stdout = old
        # CLI uses the real st_dev: on real hardware this resolves and
        # prints JSON (rc 0); in this sandbox it must refuse cleanly (rc 1),
        # never crash and never destroy.
        self.assertIn(rc, (0, 1))
        with open(p, "rb") as f:
            self.assertEqual(f.read(), b"zzz")


# ---------------------------------------------------------------------------
# structural refusals
# ---------------------------------------------------------------------------

class TestRefusals(unittest.TestCase):
    def setUp(self):
        self.c = Ctx(self)
        self.kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        self.enum_kw = {k: v for k, v in self.kw.items()
                        if k in ("sysfs", "stat_dev", "keyring_root")}

    def test_empty_list_refused(self):
        with self.assertRaises(media.Refusal):
            tier3.enumerate_targets([], **self.enum_kw)

    def test_directory_refused(self):
        with self.assertRaises(media.Refusal):
            tier3.enumerate_targets([self.c.targets], **self.enum_kw)

    def test_nonexistent_refused(self):
        with self.assertRaises(media.Refusal):
            tier3.enumerate_targets(
                [os.path.join(self.c.targets, "nope.txt")], **self.enum_kw)

    def test_dangling_symlink_refused(self):
        link = os.path.join(self.c.targets, "dangling")
        os.symlink(os.path.join(self.c.targets, "missing.txt"), link)
        with self.assertRaises(media.Refusal):
            tier3.enumerate_targets([link], **self.enum_kw)

    def test_duplicate_inode_refused(self):
        p = self.c.mkfile("dup.txt")
        with self.assertRaises(media.Refusal):
            tier3.enumerate_targets([p, p], **self.enum_kw)

    def test_keyring_root_refused(self):
        k = os.path.join(self.c.keyring, "keys", "mom.key")
        os.makedirs(os.path.dirname(k))
        with open(k, "wb") as f:
            f.write(b"key-material")
        with self.assertRaises(media.Refusal):
            tier3.enumerate_targets([k], **self.enum_kw)

    def test_unmappable_device_refused(self):
        p = self.c.mkfile("u.txt")
        kw = dict(self.enum_kw)
        kw["stat_dev"] = lambda path: (250, 250)  # no dev/block/250:250
        with self.assertRaises(media.Refusal):
            tier3.enumerate_targets([p], **kw)

    def test_symlink_resolves_to_target(self):
        p = self.c.mkfile("real.txt", b"real-bytes")
        link = os.path.join(self.c.targets, "link.txt")
        os.symlink(p, link)
        targets = tier3.enumerate_targets([link], **self.enum_kw)
        self.assertEqual(targets[0].real, os.path.realpath(p))


# ---------------------------------------------------------------------------
# the burn ceremony
# ---------------------------------------------------------------------------

class TestBurn(unittest.TestCase):
    def setUp(self):
        self.c = Ctx(self)

    def test_dry_run_is_default_and_untouched(self):
        p = self.c.mkfile("dry.txt", b"precious")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, result = tier3.shred([p], **kw)
        self.assertEqual(rc, 2)
        with open(p, "rb") as f:
            self.assertEqual(f.read(), b"precious")
        self.assertFalse(os.path.exists(self.c.logs))
        self.assertIn("nothing destroyed", result["note"].lower())

    def test_wrong_confirmation_aborts_everything(self):
        a = self.c.mkfile("a.txt", b"aaa")
        b = self.c.mkfile("b.txt", b"bbb")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, result = tier3.shred([a, b], confirm_arg="a.txt,WRONG",
                                 dry_run=False, **kw)
        self.assertEqual(rc, 2)
        self.assertTrue(result["aborted"])
        for path, data in ((a, b"aaa"), (b, b"bbb")):
            with open(path, "rb") as f:
                self.assertEqual(f.read(), data)

    def test_correct_confirm_destroys_hdd_file(self):
        p = self.c.mkfile("gone.txt", b"top-secret-bytes" * 100)
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, result = tier3.shred([p], confirm_arg="gone.txt",
                                 dry_run=False, **kw)
        self.assertEqual(rc, 0)
        self.assertFalse(os.path.lexists(p))
        cert = result["certificates"][0]
        self.assertEqual(cert["certificate"], "flamethrower-deletion")
        self.assertEqual(cert["file"], os.path.realpath(p))
        self.assertEqual(cert["media"], media.HDD)
        self.assertIn("real kill", cert["method"])
        self.assertNotIn("BEST EFFORT", cert["method"])
        self.assertIn("sample", cert["verification"])
        self.assertNotIn("recommendation", cert)
        self.assertIn("no file contents", cert["note"])
        self.assertTrue(cert["cert_id"])

    def test_cert_contains_no_content_bytes(self):
        secret = b"super-secret-password-hunter2" * 50
        p = self.c.mkfile("s.txt", secret)
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, result = tier3.shred([p], confirm_arg="s.txt",
                                 dry_run=False, **kw)
        self.assertEqual(rc, 0)
        blob = json.dumps(result["certificates"])
        self.assertNotIn("hunter2", blob)
        logp = result["log"]
        with open(logp, encoding="utf-8") as f:
            self.assertNotIn("hunter2", f.read())

    def test_flash_burn_is_labeled_best_effort(self):
        p = self.c.mkfile("f.txt", b"flash-bytes" * 100)
        kw = self.c.kwargs("sdc", disk_kw={"rotational": False})
        rc, result = tier3.shred([p], confirm_arg="f.txt",
                                 dry_run=False, **kw)
        self.assertEqual(rc, 0)
        self.assertFalse(os.path.lexists(p))
        cert = result["certificates"][0]
        self.assertIn("BEST EFFORT", cert["method"])
        self.assertIn("does NOT claim irrecoverability", cert["verification"])
        self.assertIn("recommendation", cert)
        self.assertIn("crypto-shred", cert["recommendation"])
        self.assertIn("best effort", cert["media_note"])

    def test_non_tty_without_confirm_refuses(self):
        p = self.c.mkfile("n.txt", b"n")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        kw["stdin"] = FakeStdin([], tty=False)
        with self.assertRaises(media.Refusal):
            tier3.shred([p], dry_run=False, **kw)
        self.assertTrue(os.path.exists(p))

    def test_interactive_typing_arms(self):
        p = self.c.mkfile("yes.txt", b"y")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        kw["stdin"] = FakeStdin(["yes.txt"], tty=True)
        old = sys.stdout
        sys.stdout = io.StringIO()
        try:
            rc, result = tier3.shred([p], dry_run=False, **kw)
        finally:
            sys.stdout = old
        self.assertEqual(rc, 0)
        self.assertFalse(os.path.lexists(p))

    def test_interactive_mismatch_aborts_all(self):
        a = self.c.mkfile("one.txt", b"1")
        b = self.c.mkfile("two.txt", b"2")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        kw["stdin"] = FakeStdin(["one.txt", "WRONG"], tty=True)
        old = sys.stdout
        sys.stdout = io.StringIO()
        try:
            rc, result = tier3.shred([a, b], dry_run=False, **kw)
        finally:
            sys.stdout = old
        self.assertEqual(rc, 2)
        self.assertTrue(os.path.exists(a))
        self.assertTrue(os.path.exists(b))

    def test_confirm_order_matters(self):
        a = self.c.mkfile("a1.txt", b"a")
        b = self.c.mkfile("b1.txt", b"b")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, _ = tier3.shred([a, b], confirm_arg="b1.txt,a1.txt",
                            dry_run=False, **kw)
        self.assertEqual(rc, 2)
        self.assertTrue(os.path.exists(a))
        self.assertTrue(os.path.exists(b))

    def test_multi_file_burn_two_certs(self):
        a = self.c.mkfile("m1.txt", b"m1" * 500)
        b = self.c.mkfile("m2.txt", b"m2" * 500)
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, result = tier3.shred([a, b], confirm_arg="m1.txt,m2.txt",
                                 dry_run=False, **kw)
        self.assertEqual(rc, 0)
        self.assertEqual(len(result["certificates"]), 2)
        self.assertFalse(os.path.lexists(a))
        self.assertFalse(os.path.lexists(b))

    def test_empty_file_shreds(self):
        p = self.c.mkfile("empty.txt", b"")
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, result = tier3.shred([p], confirm_arg="empty.txt",
                                 dry_run=False, **kw)
        self.assertEqual(rc, 0)
        self.assertFalse(os.path.lexists(p))

    def test_log_file_written_0600(self):
        p = self.c.mkfile("l.txt", b"logme" * 100)
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, result = tier3.shred([p], confirm_arg="l.txt",
                                 dry_run=False, **kw)
        self.assertEqual(rc, 0)
        self.assertTrue(os.path.isfile(result["log"]))
        self.assertEqual(oct(os.stat(result["log"]).st_mode & 0o777),
                         "0o600")

    def test_shred_via_symlink_destroys_target(self):
        p = self.c.mkfile("tgt.txt", b"target-bytes" * 100)
        link = os.path.join(self.c.targets, "alias.txt")
        os.symlink(p, link)
        kw = self.c.kwargs("sda", disk_kw={"rotational": True})
        rc, _ = tier3.shred([link], confirm_arg="tgt.txt",
                            dry_run=False, **kw)
        self.assertEqual(rc, 0)
        self.assertFalse(os.path.lexists(p))

    def test_overwrite_zeroes_bytes(self):
        p = self.c.mkfile("z.txt", b"Z" * 3000)
        logged = []
        n = tier3._overwrite_file(p, 3000, logged.append)
        self.assertEqual(n, 3000)
        with open(p, "rb") as f:
            self.assertEqual(f.read(), b"\x00" * 3000)

    def test_verify_samples_pass_and_fail(self):
        p = self.c.mkfile("v.txt", b"\x00" * 5000)
        ok, checked = tier3._verify_samples(p, 5000, lambda m: None)
        self.assertTrue(ok)
        self.assertEqual(checked, tier3.VERIFY_SAMPLES)
        p2 = self.c.mkfile("v2.txt", b"\xff" * 5000)
        ok3, checked3 = tier3._verify_samples(p2, 5000, lambda m: None)
        self.assertFalse(ok3)
        self.assertEqual(checked3, tier3.VERIFY_SAMPLES)


if __name__ == "__main__":
    unittest.main(verbosity=1)
