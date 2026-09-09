#!/usr/bin/env python3
"""Regression tests for flamethrower/media.py — Tier 2/3 device honesty.

Builds a FAKE sysfs tree in a temp dir; no test touches real hardware.
Run: python3 test_media.py
"""

import io
import json
import os
import shutil
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import media


class FakeSysfs:
    """Constructs a fake /sys tree with whole-disk entries."""

    def __init__(self):
        self.tmp = tempfile.mkdtemp(prefix="fake-sys-")
        self.root = os.path.join(self.tmp, "sys")
        os.makedirs(os.path.join(self.root, "block"))

    def add_disk(self, name, rotational=None, removable=None,
                 model="FAKE", serial="FAKE123", usb=False):
        d = os.path.join(self.root, "block", name)
        if usb:
            # block/<disk> is a symlink whose target passes through a usb* dir
            devdir = os.path.join(self.root, "devices", "pci0000", "usb1",
                                  "1-1", "host0", "target0:0:0")
            os.makedirs(os.path.join(devdir, "queue"))
            os.makedirs(os.path.join(devdir, "device"))
            os.symlink(devdir, d)
            d = devdir
        else:
            os.makedirs(os.path.join(d, "queue"))
            os.makedirs(os.path.join(d, "device"))
        if rotational is not None:
            with open(os.path.join(d, "queue", "rotational"), "w") as f:
                f.write("1" if rotational else "0")
        if removable is not None:
            with open(os.path.join(d, "removable"), "w") as f:
                f.write("1" if removable else "0")
        with open(os.path.join(d, "device", "model"), "w") as f:
            f.write(model)
        with open(os.path.join(d, "device", "serial"), "w") as f:
            f.write(serial)

    def close(self):
        shutil.rmtree(self.tmp, ignore_errors=True)


class TestClassify(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fs = FakeSysfs()
        cls.fs.add_disk("sda", rotational=True, removable=False)
        cls.fs.add_disk("sdb", rotational=False, removable=False)
        cls.fs.add_disk("nvme0n1", rotational=False, removable=False)
        cls.fs.add_disk("sdc", rotational=False, removable=True, usb=True)
        cls.fs.add_disk("mmcblk0", rotational=False, removable=True)
        cls.fs.add_disk("loop0", rotational=False, removable=False)

    @classmethod
    def tearDownClass(cls):
        cls.fs.close()

    def c(self, name):
        return media.classify(name, self.fs.root)

    def test_hdd_from_rotational(self):
        self.assertEqual(self.c("sda").media, media.HDD)

    def test_sata_ssd_from_non_rotational(self):
        self.assertEqual(self.c("sdb").media, media.SATA_SSD)

    def test_nvme(self):
        info = self.c("nvme0n1")
        self.assertEqual(info.media, media.NVME)
        self.assertEqual(info.transport, "nvme")

    def test_usb_stick_is_flash(self):
        info = self.c("sdc")
        self.assertEqual(info.media, media.FLASH_USB)
        self.assertTrue(info.removable)

    def test_mmc_is_flash(self):
        self.assertEqual(self.c("mmcblk0").media, media.FLASH_USB)

    def test_loop_is_virtual(self):
        self.assertEqual(self.c("loop0").media, media.VIRTUAL)

    def test_unknown_name(self):
        self.assertEqual(self.c("sdz").media, media.UNKNOWN)

    def test_partition_resolves_to_disk(self):
        self.assertEqual(self.c("sda1").media, media.HDD)
        self.assertEqual(self.c("/dev/nvme0n1p2").media, media.NVME)
        self.assertEqual(self.c("mmcblk0p1").media, media.FLASH_USB)

    def test_model_serial_surfaced(self):
        info = self.c("sda")
        self.assertEqual(info.model, "FAKE")
        self.assertEqual(info.serial, "FAKE123")


class TestKills(unittest.TestCase):
    def test_recommended_kill_names_real_routine(self):
        self.assertIn("nvme format --ses=2", media.recommended_kill(media.NVME))
        self.assertIn("ATA Secure Erase", media.recommended_kill(media.SATA_SSD))
        self.assertIn("nwipe", media.recommended_kill(media.HDD))
        self.assertIn("crypto-shred", media.recommended_kill(media.FLASH_USB))

    def test_overwrite_kill_allowed_only_on_hdd(self):
        ok = media.DeviceInfo("sda", media.HDD, "sata", False, True)
        self.assertTrue(media.assert_overwrite_kill_ok(ok))
        for m in (media.SATA_SSD, media.NVME, media.FLASH_USB,
                  media.VIRTUAL, media.UNKNOWN):
            bad = media.DeviceInfo("x", m, "unknown", False, False)
            with self.assertRaises(media.Refusal):
                media.assert_overwrite_kill_ok(bad)

    def test_honest_label_flash_admits_best_effort(self):
        for m in (media.SATA_SSD, media.NVME, media.FLASH_USB, media.UNKNOWN):
            label = media.honest_label(m)
            self.assertIn("best effort", label)
            self.assertIn("crypto-shred", label)
        self.assertIn("sector 100", media.honest_label(media.HDD))


class TestCLI(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fs = FakeSysfs()
        cls.fs.add_disk("sda", rotational=True, removable=False)
        cls.fs.add_disk("nvme0n1", rotational=False, removable=False)

    @classmethod
    def tearDownClass(cls):
        cls.fs.close()

    def test_identify_json(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            rc = media.main(["--sysfs", self.fs.root, "identify",
                             "/dev/sda", "nvme0n1"])
        self.assertEqual(rc, 0)
        out = json.loads(buf.getvalue())
        self.assertEqual(out[0]["media"], "hdd")
        self.assertEqual(out[1]["media"], "nvme")

    def test_kill_plan_refuses_flash(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            media.main(["--sysfs", self.fs.root, "kill-plan", "nvme0n1"])
        out = json.loads(buf.getvalue())
        self.assertFalse(out[0]["overwrite_kill_allowed"])
        self.assertIn("refusing", out[0]["refusal"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
