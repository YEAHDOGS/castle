#!/usr/bin/env python3
"""Regression tests for flamethrower/tier2.py — Tier-2 firmware erase.

Fakes EVERYTHING: fake sysfs tree, fake hdparm/nvme output, fake mounts
table, fake command runner, fake stdin. No test touches real hardware,
runs no destructive commands, and never writes outside temp dirs.
Run: python3 test_tier2.py
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
import tier2


# ---------------------------------------------------------------------------
# fakes
# ---------------------------------------------------------------------------

class FakeSysfs:
    """Fake /sys tree with whole-disk entries (serial files included)."""

    def __init__(self):
        self.tmp = tempfile.mkdtemp(prefix="fake-sys-")
        self.root = os.path.join(self.tmp, "sys")
        os.makedirs(os.path.join(self.root, "block"))

    def add_disk(self, name, rotational=False, removable=False,
                 serial="FAKESERIAL1", model="FAKE"):
        d = os.path.join(self.root, "block", name)
        os.makedirs(os.path.join(d, "queue"))
        os.makedirs(os.path.join(d, "device"))
        with open(os.path.join(d, "queue", "rotational"), "w") as f:
            f.write("1" if rotational else "0")
        with open(os.path.join(d, "removable"), "w") as f:
            f.write("1" if removable else "0")
        with open(os.path.join(d, "device", "model"), "w") as f:
            f.write(model)
        with open(os.path.join(d, "device", "serial"), "w") as f:
            f.write(serial)

    def close(self):
        shutil.rmtree(self.tmp, ignore_errors=True)


NVME_CRYPTO_OK = """NVMe Identify Controller:
Format NVM Attributes (FNA) : 0x4
  [2:2] : 0x1   Crypto Erase Supported as part of Secure Erase
  [1:1] : 0     Crypto Erase Applies to All Namespaces
Sanitize Commands (SANSZ) : 0
"""

NVME_SANITIZE_ONLY = """NVMe Identify Controller:
Format NVM Attributes (FNA) : 0x0
Sanitize Commands (SANSZ) : 1
  [2:2] : 0x1   Block Erase Sanitize Operation Supported
"""

NVME_NOTHING = """NVMe Identify Controller:
Format NVM Attributes (FNA) : 0x0
Sanitize Commands (SANSZ) : 0
"""

HDPARM_ENHANCED = """ATA device, with non-removable media
Security:
    Master password revision code = 65534
        supported
        not enabled
        not locked
    not frozen
    2min for SECURITY ERASE UNIT. 8min for ENHANCED SECURITY ERASE UNIT.
"""

HDPARM_NORMAL = """ATA device, with non-removable media
Security:
    Master password revision code = 65534
        supported
        not enabled
        not locked
    not frozen
    2min for SECURITY ERASE UNIT.
"""

HDPARM_FROZEN = """ATA device, with non-removable media
Security:
    Master password revision code = 65534
        supported
        not enabled
        not locked
        frozen
    2min for SECURITY ERASE UNIT.
"""

HDPARM_UNSUPPORTED = """ATA device, with non-removable media
Security:
    not supported
"""


class ProcResult:
    def __init__(self, rc, out):
        self.returncode = rc
        self.stdout = out
        self.stderr = ""


class FakeRunner:
    """Maps argv-prefix strings to canned (rc, stdout); records all calls."""

    def __init__(self):
        self.stubs = {}   # prefix tuple -> (rc, stdout)
        self.calls = []

    def stub(self, prefix, rc=0, stdout=""):
        self.stubs[tuple(prefix)] = (rc, stdout)

    def __call__(self, argv):
        self.calls.append(list(argv))
        for prefix, (rc, out) in self.stubs.items():
            if list(argv[:len(prefix)]) == list(prefix):
                return ProcResult(rc, out)
        raise OSError("no stub for %r" % (argv,))


class FakeStdin(io.StringIO):
    def isatty(self):
        return True


class NonTtyStdin(io.StringIO):
    def isatty(self):
        return False


# ---------------------------------------------------------------------------
# tests
# ---------------------------------------------------------------------------

class Base(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fs = FakeSysfs()
        cls.fs.add_disk("nvme0n1", serial="NVME123")
        cls.fs.add_disk("sda", rotational=True, serial="HDD123")
        cls.fs.add_disk("sdb", serial="SSD123")
        cls.fs.add_disk("sdc", removable=True, serial="USB123")
        cls.fs.add_disk("loop0", serial="LOOP123")
        cls.tmp = tempfile.mkdtemp(prefix="tier2-test-")
        cls.mounts = os.path.join(cls.tmp, "mounts")
        with open(cls.mounts, "w") as f:
            f.write("")  # nothing mounted
        cls.logdir = os.path.join(cls.tmp, "logs")

    @classmethod
    def tearDownClass(cls):
        cls.fs.close()
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def info(self, name):
        return media.classify(name, self.fs.root)

    def nvme_runner(self, stdout):
        r = FakeRunner()
        r.stub(["nvme", "id-ctrl", "-H"], stdout=stdout)
        r.stub(["nvme", "format"], stdout="format ok")
        r.stub(["nvme", "sanitize"], stdout="sanitize ok")
        return r


class TestProbes(Base):
    def test_probe_nvme_crypto(self):
        ok, san, _ = tier2.probe_nvme("/dev/nvme0n1",
                                      self.nvme_runner(NVME_CRYPTO_OK))
        self.assertTrue(ok)
        self.assertFalse(san)

    def test_probe_nvme_sanitize_only(self):
        ok, san, _ = tier2.probe_nvme("/dev/nvme0n1",
                                      self.nvme_runner(NVME_SANITIZE_ONLY))
        self.assertFalse(ok)
        self.assertTrue(san)

    def test_probe_nvme_nothing(self):
        ok, san, _ = tier2.probe_nvme("/dev/nvme0n1",
                                      self.nvme_runner(NVME_NOTHING))
        self.assertFalse(ok)
        self.assertFalse(san)

    def test_probe_nvme_missing_tool(self):
        def boom(argv):
            raise FileNotFoundError("nvme")
        ok, san, detail = tier2.probe_nvme("/dev/nvme0n1", boom)
        self.assertFalse(ok)
        self.assertFalse(san)
        self.assertIn("not available", detail)

    def test_probe_ata_enhanced(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_ENHANCED)
        self.assertEqual(tier2.probe_ata("/dev/sda", r), "enhanced")

    def test_probe_ata_normal(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_NORMAL)
        self.assertEqual(tier2.probe_ata("/dev/sda", r), "normal")

    def test_probe_ata_frozen(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_FROZEN)
        self.assertEqual(tier2.probe_ata("/dev/sda", r), "frozen")

    def test_probe_ata_unsupported(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_UNSUPPORTED)
        self.assertEqual(tier2.probe_ata("/dev/sda", r), "unsupported")

    def test_probe_ata_missing_tool(self):
        def boom(argv):
            raise FileNotFoundError("hdparm")
        self.assertEqual(tier2.probe_ata("/dev/sda", boom), "unavailable")


class TestPlans(Base):
    def test_nvme_plan_is_crypto_erase_ses2(self):
        steps = tier2.plan_erase(self.info("nvme0n1"),
                                 self.nvme_runner(NVME_CRYPTO_OK))
        self.assertEqual(len(steps), 1)
        self.assertIn("nvme", steps[0].argv[0])
        self.assertIn("format", steps[0].argv)
        self.assertIn("--ses=2", steps[0].argv)  # crypto erase, not user-erase
        self.assertIn("--force", steps[0].argv)

    def test_nvme_sanitize_fallback_plan(self):
        steps = tier2.plan_erase(self.info("nvme0n1"),
                                 self.nvme_runner(NVME_SANITIZE_ONLY))
        self.assertEqual(len(steps), 1)
        self.assertIn("sanitize", steps[0].argv)
        self.assertIn("-a", steps[0].argv)

    def test_nvme_no_firmware_erase_refuses_no_overwrite_offered(self):
        with self.assertRaises(media.Refusal) as cm:
            tier2.plan_erase(self.info("nvme0n1"),
                             self.nvme_runner(NVME_NOTHING))
        msg = str(cm.exception).lower()
        self.assertIn("crypto-shred", msg)
        # the message may EXPLAIN why overwrite fails, but must never offer
        # it as the kill (no nwipe/dd-style fallback blessing)
        self.assertNotIn("nwipe", msg)
        self.assertNotIn("fallback", msg)

    def test_sata_ssd_enhanced_plan(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_ENHANCED)
        r.stub(["hdparm", "--user-master"], stdout="")
        steps = tier2.plan_erase(self.info("sdb"), r)
        self.assertEqual(len(steps), 2)
        self.assertIn("--security-erase-enhanced", steps[1].argv)
        # the throwaway ATA password is planned, not printed as a kill
        self.assertIn("--security-set-pass", steps[0].argv)

    def test_sata_ssd_normal_plan(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_NORMAL)
        steps = tier2.plan_erase(self.info("sdb"), r)
        self.assertIn("--security-erase", steps[1].argv)
        self.assertNotIn("--security-erase-enhanced", steps[1].argv)

    def test_sata_ssd_frozen_refuses_with_remedy(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_FROZEN)
        with self.assertRaises(media.Refusal) as cm:
            tier2.plan_erase(self.info("sdb"), r)
        self.assertIn("FROZEN", str(cm.exception))
        self.assertIn("Suspend", str(cm.exception))

    def test_sata_ssd_unsupported_refuses(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_UNSUPPORTED)
        with self.assertRaises(media.Refusal) as cm:
            tier2.plan_erase(self.info("sdb"), r)
        self.assertIn("crypto-shred", str(cm.exception).lower())

    def test_sata_ssd_hdparm_missing_refuses(self):
        def boom(argv):
            raise FileNotFoundError("hdparm")
        with self.assertRaises(media.Refusal):
            tier2.plan_erase(self.info("sdb"), boom)

    def test_hdd_plan_is_overwrite(self):
        steps = tier2.plan_erase(self.info("sda"), FakeRunner())
        self.assertEqual(len(steps), 1)
        self.assertEqual(steps[0].argv[0], "__builtin_zero_fill__")
        self.assertIn("single-pass", steps[0].note)

    def test_usb_flash_refuses_crypto_shred_only(self):
        with self.assertRaises(media.Refusal) as cm:
            tier2.plan_erase(self.info("sdc"), FakeRunner())
        self.assertIn("crypto-shred", str(cm.exception).lower())

    def test_virtual_refuses(self):
        with self.assertRaises(media.Refusal):
            tier2.plan_erase(self.info("loop0"), FakeRunner())

    def test_unknown_refuses_never_assumes_hdd(self):
        info = media.DeviceInfo("sdz", media.UNKNOWN, "unknown",
                                False, False, serial="X")
        with self.assertRaises(media.Refusal) as cm:
            tier2.plan_erase(info, FakeRunner())
        self.assertIn("Never assume HDD", str(cm.exception))


class TestGuards(Base):
    def test_dry_run_runs_no_destructive_commands(self):
        r = self.nvme_runner(NVME_CRYPTO_OK)
        rc, result = tier2.erase("nvme0n1", dry_run=True, runner=r,
                                sysfs=self.fs.root, mounts_path=self.mounts)
        self.assertEqual(rc, 2)
        self.assertIn("plan", result)
        destructive = [c for c in r.calls
                       if "format" in c or "sanitize" in c]
        self.assertEqual(destructive, [])  # probes only

    def test_mounted_partition_refuses(self):
        with open(self.mounts, "w") as f:
            f.write("/dev/sdb1 /mnt/data ext4 rw 0 0\n")
        self.addCleanup(lambda: open(self.mounts, "w").write(""))
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_NORMAL)
        with self.assertRaises(media.Refusal) as cm:
            tier2.assert_erasable(self.info("sdb"), self.mounts)
        self.assertIn("mounted", str(cm.exception).lower())

    def test_root_device_mounted_refuses(self):
        with open(self.mounts, "w") as f:
            f.write("/dev/sdb2 / ext4 rw 0 0\n")
        self.addCleanup(lambda: open(self.mounts, "w").write(""))
        with self.assertRaises(media.Refusal):
            tier2.assert_erasable(self.info("sdb"), self.mounts)

    def test_unknown_serial_refuses(self):
        info = media.DeviceInfo("sda", media.HDD, "sata", False, True,
                                serial="unknown")
        with self.assertRaises(media.Refusal):
            tier2.assert_erasable(info, self.mounts)

    def test_confirm_serial_exact_match(self):
        self.assertTrue(tier2.confirm_serial("ABC123", provided="ABC123"))

    def test_confirm_serial_erase_prefix(self):
        # interactive path via fake TTY stdin
        s = FakeStdin("ERASE ABC123\n")
        self.assertTrue(tier2.confirm_serial("ABC123", stdin=s))
        s2 = FakeStdin("ABC123\n")
        self.assertTrue(tier2.confirm_serial("ABC123", stdin=s2))

    def test_confirm_serial_mismatch(self):
        self.assertFalse(tier2.confirm_serial("ABC123", provided="nope"))

    def test_confirm_serial_refuses_piped_stdin(self):
        with self.assertRaises(media.Refusal):
            tier2.confirm_serial("ABC123", stdin=NonTtyStdin("ABC123\n"))

    def test_serial_change_mid_run_aborts(self):
        r = self.nvme_runner(NVME_CRYPTO_OK)
        orig = tier2._rescan_serial
        tier2._rescan_serial = lambda info, sysfs: "DIFFERENT"
        self.addCleanup(lambda: setattr(tier2, "_rescan_serial", orig))
        rc, result = tier2.erase("nvme0n1", confirm_serial_arg="NVME123",
                                dry_run=False, runner=r, sysfs=self.fs.root,
                                mounts_path=self.mounts,
                                log_dir=self.logdir, no_countdown=True,
                                require_root=False)
        self.assertEqual(rc, 1)
        self.assertIn("changed", result["note"])
        formats = [c for c in r.calls if "format" in c]
        self.assertEqual(formats, [])  # never executed

    def test_wrong_serial_aborts_without_running(self):
        r = self.nvme_runner(NVME_CRYPTO_OK)
        rc, result = tier2.erase("nvme0n1", confirm_serial_arg="WRONG",
                                dry_run=False, runner=r, sysfs=self.fs.root,
                                mounts_path=self.mounts,
                                log_dir=self.logdir, no_countdown=True,
                                require_root=False)
        self.assertEqual(rc, 2)
        self.assertTrue(result["aborted"])
        formats = [c for c in r.calls if "format" in c]
        self.assertEqual(formats, [])


class TestBurn(Base):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()

    def burn_nvme(self, runner, serial="NVME123", **kw):
        args = dict(confirm_serial_arg=serial, dry_run=False, runner=runner,
                    sysfs=self.fs.root, mounts_path=self.mounts,
                    log_dir=os.path.join(self.tmp, "burn-%s" % serial),
                    no_countdown=True, require_root=False)
        args.update(kw)
        return tier2.erase("nvme0n1", **args)

    def test_successful_burn_emits_certificate(self):
        r = self.nvme_runner(NVME_CRYPTO_OK)
        rc, cert = self.burn_nvme(r)
        self.assertEqual(rc, 0)
        self.assertEqual(cert["certificate"], "flamethrower-deletion")
        self.assertEqual(cert["device"], "/dev/nvme0n1")
        self.assertEqual(cert["serial"], "NVME123")
        self.assertEqual(cert["media"], "nvme")
        self.assertTrue(cert["success"])
        self.assertIn("firmware", cert["verification"])
        self.assertIn("cert_id", cert)
        self.assertTrue(os.path.exists(
            os.path.join(self.tmp, "burn-NVME123", "certificates",
                         cert["cert_id"] + ".json")))
        # the log records THAT it burned; cert carries no key material
        self.assertNotIn("flamethrower", json.dumps(cert).lower()
                         .replace("flamethrower-deletion", ""))
        formats = [c for c in r.calls if "format" in c]
        self.assertEqual(len(formats), 1)
        self.assertIn("--ses=2", formats[0])

    def test_failed_step_marks_partial_erase(self):
        r = self.nvme_runner(NVME_CRYPTO_OK)
        r.stubs[("nvme", "format")] = (1, "device busy")
        rc, cert = self.burn_nvme(r)
        self.assertEqual(rc, 1)
        self.assertFalse(cert["success"])
        self.assertIn("PARTIAL", cert["warning"])

    def test_sata_burn_runs_setpass_then_erase(self):
        r = FakeRunner()
        r.stub(["hdparm", "-I"], stdout=HDPARM_ENHANCED)
        r.stub(["hdparm", "--user-master"], stdout="")
        args = dict(confirm_serial_arg="SSD123", dry_run=False, runner=r,
                    sysfs=self.fs.root, mounts_path=self.mounts,
                    log_dir=os.path.join(self.tmp, "burn-SSD123"),
                    no_countdown=True, require_root=False)
        rc, cert = tier2.erase("sdb", **args)
        self.assertEqual(rc, 0)
        erases = [c for c in r.calls if "--security-erase-enhanced" in c]
        self.assertEqual(len(erases), 1)
        # the ATA password travels only inside argv, never to stdout/log text
        self.assertNotIn("flamethrower", open(
            os.path.join(self.tmp, "burn-SSD123", "logs",
                         os.listdir(os.path.join(self.tmp, "burn-SSD123",
                                                 "logs"))[0])).read()
                         .replace("flamethrower-deletion", ""))


class TestHddOverwrite(Base):
    def test_zero_fill_and_readback(self):
        path = os.path.join(tempfile.mkdtemp(prefix="hdd-fake-"), "disk.img")
        self.addCleanup(shutil.rmtree, os.path.dirname(path),
                        ignore_errors=True)
        size = 4 * 1024 * 1024  # 4 MiB fake disk
        with open(path, "wb") as f:
            f.write(os.urandom(size))
        msgs = []
        tier2.zero_fill(path, size, msgs.append)
        ok, checked = tier2.readback_verify(path, size, msgs.append)
        self.assertTrue(ok)
        self.assertEqual(checked, tier2.VERIFY_SAMPLES)
        with open(path, "rb") as f:
            self.assertEqual(f.read(), b"\x00" * size)

    def test_readback_catches_nonzero(self):
        path = os.path.join(tempfile.mkdtemp(prefix="hdd-fake-"), "disk.img")
        self.addCleanup(shutil.rmtree, os.path.dirname(path),
                        ignore_errors=True)
        size = 2 * 1024 * 1024
        with open(path, "wb") as f:
            f.write(os.urandom(size))
        ok, _ = tier2.readback_verify(path, size, lambda m: None)
        self.assertFalse(ok)  # random data is not zeros

    def test_run_step_builtin(self):
        path = os.path.join(tempfile.mkdtemp(prefix="hdd-fake-"), "disk.img")
        self.addCleanup(shutil.rmtree, os.path.dirname(path),
                        ignore_errors=True)
        size = 2 * 1024 * 1024
        with open(path, "wb") as f:
            f.write(os.urandom(size))
        step = tier2.Step("hdd-single-pass-overwrite",
                          ["__builtin_zero_fill__", "fake-dev"])
        rc, note = tier2._run_step(step, path, lambda m: None)
        self.assertEqual(rc, 0)
        self.assertIn("zero", note)


class TestCLI(Base):
    def test_plan_json(self):
        r = self.nvme_runner(NVME_CRYPTO_OK)
        # plan() via CLI uses the default runner; just check JSON shape
        # through plan_erase + the CLI's json path is covered below
        buf = io.StringIO()
        from contextlib import redirect_stdout
        orig_run = tier2._run
        tier2._run = r
        self.addCleanup(lambda: setattr(tier2, "_run", orig_run))
        # plan_erase calls probes with default runner inside main(); patch it
        with redirect_stdout(buf):
            rc = tier2.main(["--sysfs", self.fs.root, "plan", "nvme0n1"])
        self.assertEqual(rc, 0)
        out = json.loads(buf.getvalue())
        self.assertEqual(out[0]["media"], "nvme")
        self.assertEqual(len(out[0]["plan"]), 1)

    def test_erase_dry_run_default_exits_2(self):
        buf = io.StringIO()
        from contextlib import redirect_stdout
        r = self.nvme_runner(NVME_CRYPTO_OK)
        orig_run = tier2._run
        tier2._run = r
        self.addCleanup(lambda: setattr(tier2, "_run", orig_run))
        with redirect_stdout(buf):
            rc = tier2.main(["--sysfs", self.fs.root, "--mounts", self.mounts,
                             "erase", "nvme0n1"])
        self.assertEqual(rc, 2)
        self.assertIn("DRY-RUN", buf.getvalue())


if __name__ == "__main__":
    unittest.main(verbosity=2)
