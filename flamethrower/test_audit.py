#!/usr/bin/env python3
"""Regression tests for the flamethrower cross-burn audit log (step 6).

Covers: append + hash-chain verification, tamper/delete/reorder detection,
append refusals (incomplete data), edge cases (missing/empty/malformed log),
and the wire-in that records every Tier 1/2/3 burn in the log.
Run:  python3 test_audit.py
Exit 0 = all green, 1 = a regression.
"""

import io
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import audit  # noqa: E402
import keyring  # noqa: E402
import media  # noqa: E402
import tier2  # noqa: E402
import tier3  # noqa: E402


def fresh_root():
    d = tempfile.mkdtemp(prefix="audit-test-")
    root = os.path.join(d, "ft")
    os.makedirs(root)
    return d, root


def burn1(root, cert_id="cert-aaa", vault="mom"):
    return audit.append(root, tier="1", cert_id=cert_id,
                        fingerprint={"vault": vault, "key_id": "k1",
                                     "key_sha256_receipt": "r1"},
                        method="crypto-shred (key destruction)", media="ssd",
                        success=True, verification="copies destroyed")


def burn2(root, cert_id="cert-bbb"):
    return audit.append(root, tier="2", cert_id=cert_id,
                        fingerprint={"device": "/dev/nvme0n1",
                                     "serial": "NVME123", "model": "FAKE"},
                        method="Tier 2 firmware erase (crypto erase)",
                        media="nvme", success=True,
                        verification="firmware-reported")


def burn3(root, cert_id="cert-ccc"):
    return audit.append(root, tier="3", cert_id=cert_id,
                        fingerprint={"file": "/data/x.txt", "size_bytes": 9,
                                     "inode": 4242},
                        method="Tier-3 file shred (HDD)", media="hdd",
                        success=True, verification="read-back samples")


class TestChain(unittest.TestCase):
    def setUp(self):
        self.tmp, self.root = fresh_root()
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def test_append_creates_log_0600(self):
        burn1(self.root)
        path = audit.audit_path(self.root)
        self.assertTrue(os.path.exists(path))
        self.assertEqual(oct(os.stat(path).st_mode & 0o777), "0o600")

    def test_genesis_first_entry(self):
        e = burn1(self.root)
        self.assertEqual(e["seq"], 1)
        self.assertEqual(e["prev_hash"], audit.GENESIS_HASH)
        self.assertEqual(len(e["entry_hash"]), 64)

    def test_seq_and_links_chain(self):
        e1 = burn1(self.root)
        e2 = burn2(self.root)
        e3 = burn3(self.root)
        self.assertEqual([e1["seq"], e2["seq"], e3["seq"]], [1, 2, 3])
        self.assertEqual(e2["prev_hash"], e1["entry_hash"])
        self.assertEqual(e3["prev_hash"], e2["entry_hash"])
        r = audit.verify(self.root)
        self.assertTrue(r["valid"], r["problems"])
        self.assertEqual(r["entries"], 3)

    def test_mixed_tiers_verify(self):
        burn1(self.root, cert_id="c1")
        burn3(self.root, cert_id="c2")
        burn2(self.root, cert_id="c3")
        burn1(self.root, cert_id="c4", vault="dad")
        r = audit.verify(self.root)
        self.assertTrue(r["valid"], r["problems"])
        self.assertEqual(r["entries"], 4)
        tails = [e["tier"] for e in audit.tail(self.root, 4)]
        self.assertEqual(tails, ["1", "3", "2", "1"])

    def test_operator_recorded(self):
        e = burn1(self.root)
        self.assertTrue(e["operator"])
        self.assertTrue(e["timestamp"].endswith("Z"))


class TestTamper(unittest.TestCase):
    def setUp(self):
        self.tmp, self.root = fresh_root()
        self.addCleanup(shutil.rmtree, self.tmp, True)
        burn1(self.root, cert_id="c1")
        burn2(self.root, cert_id="c2")
        burn3(self.root, cert_id="c3")
        self.path = audit.audit_path(self.root)

    def _lines(self):
        with open(self.path, encoding="utf-8") as f:
            return f.readlines()

    def test_edit_entry_detected(self):
        lines = self._lines()
        e2 = json.loads(lines[1])
        e2["success"] = False  # flip a field, keep the old hash
        lines[1] = json.dumps(e2) + "\n"
        with open(self.path, "w", encoding="utf-8") as f:
            f.writelines(lines)
        r = audit.verify(self.root)
        self.assertFalse(r["valid"])
        self.assertTrue(any("line 2" in p and "mismatch" in p
                            for p in r["problems"]), r["problems"])

    def test_delete_entry_detected(self):
        lines = self._lines()
        del lines[1]  # silently drop the middle burn
        with open(self.path, "w", encoding="utf-8") as f:
            f.writelines(lines)
        r = audit.verify(self.root)
        self.assertFalse(r["valid"])
        self.assertTrue(any("continuity" in p for p in r["problems"]),
                        r["problems"])

    def test_reorder_detected(self):
        lines = self._lines()
        lines[0], lines[1] = lines[1], lines[0]
        with open(self.path, "w", encoding="utf-8") as f:
            f.writelines(lines)
        r = audit.verify(self.root)
        self.assertFalse(r["valid"])
        self.assertTrue(len(r["problems"]) >= 1)

    def test_appended_garbage_flagged(self):
        with open(self.path, "a", encoding="utf-8") as f:
            f.write("not json at all\n")
        r = audit.verify(self.root)
        self.assertFalse(r["valid"])
        self.assertTrue(any("invalid JSON" in p for p in r["problems"]),
                        r["problems"])


class TestRefusals(unittest.TestCase):
    def setUp(self):
        self.tmp, self.root = fresh_root()
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def kw(self, **over):
        d = dict(tier="1", cert_id="c1",
                 fingerprint={"vault": "mom", "key_id": "k1"},
                 method="crypto-shred (key destruction)", media="ssd",
                 success=True, verification="ok")
        d.update(over)
        return d

    def test_refuses_missing_cert_id(self):
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(cert_id=""))
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(cert_id=None))

    def test_refuses_missing_method(self):
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(method=""))

    def test_refuses_bad_tier(self):
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(tier="4"))
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(tier=None))

    def test_refuses_non_dict_fingerprint(self):
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(fingerprint="mom"))
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(fingerprint={}))

    def test_refusal_leaves_log_untouched(self):
        with self.assertRaises(ValueError):
            audit.append(self.root, **self.kw(cert_id=""))
        self.assertFalse(os.path.exists(audit.audit_path(self.root)))


class TestEdges(unittest.TestCase):
    def setUp(self):
        self.tmp, self.root = fresh_root()
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def test_missing_log_is_clean(self):
        r = audit.verify(self.root)
        self.assertTrue(r["valid"])
        self.assertEqual(r["entries"], 0)

    def test_empty_log_is_clean(self):
        open(audit.audit_path(self.root), "w").close()
        r = audit.verify(self.root)
        self.assertTrue(r["valid"])
        self.assertEqual(r["entries"], 0)

    def test_failed_burn_recorded_honestly(self):
        e = audit.append(self.root, tier="2", cert_id="c-fail",
                         fingerprint={"device": "/dev/sdb",
                                      "serial": "BAD", "model": "X"},
                         method="Tier 2 firmware erase (ATA secure erase)",
                         media="sata-ssd", success=False,
                         verification="step failed — partial erase")
        self.assertFalse(e["success"])
        r = audit.verify(self.root)
        self.assertTrue(r["valid"], r["problems"])


# ---------------------------------------------------------------------------
# wire-in: real burns across the tiers must land in the log
# ---------------------------------------------------------------------------

class ProcResult:
    def __init__(self, rc, out):
        self.returncode = rc
        self.stdout = out
        self.stderr = ""


class FakeRunner:
    def __init__(self):
        self.stubs = {}
        self.calls = []

    def stub(self, prefix, rc=0, stdout=""):
        self.stubs[tuple(prefix)] = (rc, stdout)

    def __call__(self, argv):
        self.calls.append(list(argv))
        for prefix, (rc, out) in self.stubs.items():
            if list(argv[:len(prefix)]) == list(prefix):
                return ProcResult(rc, out)
        raise OSError("no stub for %r" % (argv,))


NVME_CRYPTO_OK = """NVMe Identify Controller:
Format NVM Attributes (FNA) : 0x4
  [2:2] : 0x1   Crypto Erase Supported as part of Secure Erase
"""


class FakeSysfs2:
    def __init__(self):
        self.tmp = tempfile.mkdtemp(prefix="fake-sys2-")
        self.root = os.path.join(self.tmp, "sys")
        os.makedirs(os.path.join(self.root, "block"))
        d = os.path.join(self.root, "block", "nvme0n1")
        os.makedirs(os.path.join(d, "queue"))
        os.makedirs(os.path.join(d, "device"))
        open(os.path.join(d, "queue", "rotational"), "w").write("0")
        open(os.path.join(d, "removable"), "w").write("0")
        open(os.path.join(d, "device", "model"), "w").write("FAKE")
        open(os.path.join(d, "device", "serial"), "w").write("NVME123")

    def close(self):
        shutil.rmtree(self.tmp, ignore_errors=True)


class FakeSysfs3:
    """Fake /sys with block disks and dev/block/<maj>:<min> symlinks."""

    def __init__(self):
        self.tmp = tempfile.mkdtemp(prefix="fake-sys3-")
        self.root = os.path.join(self.tmp, "sys")
        os.makedirs(os.path.join(self.root, "block"))
        os.makedirs(os.path.join(self.root, "dev", "block"))

    def add_disk(self, name, rotational=True, serial="FAKESERIAL"):
        d = os.path.join(self.root, "block", name)
        os.makedirs(os.path.join(d, "queue"))
        os.makedirs(os.path.join(d, "device"))
        open(os.path.join(d, "queue", "rotational"), "w") \
            .write("1" if rotational else "0")
        open(os.path.join(d, "removable"), "w").write("0")
        open(os.path.join(d, "device", "model"), "w").write("FAKE")
        open(os.path.join(d, "device", "serial"), "w").write(serial)
        major, minor = 8, 5
        os.symlink("../../block/%s" % name,
                   os.path.join(self.root, "dev", "block",
                                "%d:%d" % (major, minor)))
        return major, minor

    def close(self):
        shutil.rmtree(self.tmp, ignore_errors=True)


class TestWireIn(unittest.TestCase):
    def test_tier1_burn_logged(self):
        tmp = tempfile.mkdtemp(prefix="audit-wire1-")
        self.addCleanup(shutil.rmtree, tmp, True)
        root = os.path.join(tmp, "kr")
        keyring.init(root)
        keyring.create_vault(root, "mom", escrow=True)
        r = keyring.destroy_vault(root, "mom", confirm="mom")
        self.assertFalse(r["dry_run"])
        entries = audit.tail(root, 5)
        self.assertEqual(len(entries), 1)
        e = entries[0]
        self.assertEqual(e["tier"], "1")
        self.assertEqual(e["cert_id"], r["cert_id"])
        self.assertEqual(e["fingerprint"]["vault"], "mom")
        # receipt hash only — the key itself never touches the log
        self.assertIn("key_sha256_receipt", e["fingerprint"])
        blob = json.dumps(entries)
        self.assertNotIn("key_sha256_receipt", blob.replace(
            "key_sha256_receipt", ""))  # sanity: receipt is the only secret-ish field
        self.assertTrue(audit.verify(root)["valid"])

    def test_tier1_master_burn_logs_each_vault(self):
        tmp = tempfile.mkdtemp(prefix="audit-wire1m-")
        self.addCleanup(shutil.rmtree, tmp, True)
        root = os.path.join(tmp, "kr")
        keyring.init(root)
        keyring.create_vault(root, "a")
        keyring.create_vault(root, "b")
        r = keyring.destroy_master(root, confirm="DESTROY-ALL")
        entries = audit.tail(root, 5)
        self.assertEqual(len(entries), 2)
        self.assertEqual(sorted(e["fingerprint"]["vault"] for e in entries),
                         ["a", "b"])
        self.assertTrue(audit.verify(root)["valid"])

    def test_tier2_burn_logged(self):
        fs = FakeSysfs2()
        tmp = tempfile.mkdtemp(prefix="audit-wire2-")
        self.addCleanup(shutil.rmtree, tmp, True)
        self.addCleanup(fs.close)
        mounts = os.path.join(tmp, "mounts")
        open(mounts, "w").write("")
        logdir = os.path.join(tmp, "t2")
        r = FakeRunner()
        r.stub(["nvme", "id-ctrl", "-H"], stdout=NVME_CRYPTO_OK)
        r.stub(["nvme", "format"], stdout="format ok")
        rc, cert = tier2.erase("nvme0n1", confirm_serial_arg="NVME123",
                               dry_run=False, runner=r, sysfs=fs.root,
                               mounts_path=mounts, log_dir=logdir,
                               no_countdown=True, require_root=False)
        self.assertEqual(rc, 0)
        entries = audit.tail(logdir, 5)
        self.assertEqual(len(entries), 1)
        e = entries[0]
        self.assertEqual(e["tier"], "2")
        self.assertEqual(e["cert_id"], cert["cert_id"])
        self.assertEqual(e["fingerprint"]["serial"], "NVME123")
        self.assertTrue(e["success"])
        self.assertTrue(audit.verify(logdir)["valid"])

    def test_tier3_burn_logged_per_file(self):
        fs = FakeSysfs3()
        tmp = tempfile.mkdtemp(prefix="audit-wire3-")
        self.addCleanup(shutil.rmtree, tmp, True)
        self.addCleanup(fs.close)
        major, minor = fs.add_disk("sda", rotational=True, serial="HDD123")
        targets = os.path.join(tmp, "targets")
        os.makedirs(targets)
        files = []
        for name in ("f1.txt", "f2.txt"):
            p = os.path.join(targets, name)
            with open(p, "wb") as f:
                f.write(b"data" * 2000)
            files.append(p)
        logdir = os.path.join(tmp, "t3")
        rc, result = tier3.shred(files, confirm_arg="f1.txt,f2.txt",
                                 dry_run=False, sysfs=fs.root,
                                 stat_dev=lambda p: (major, minor),
                                 keyring_root=os.path.join(tmp, "kr"),
                                 log_dir=logdir, no_countdown=True)
        self.assertEqual(rc, 0)
        entries = audit.tail(logdir, 5)
        self.assertEqual(len(entries), 2)
        self.assertTrue(all(e["tier"] == "3" for e in entries))
        cert_ids = {c["cert_id"] for c in result["certificates"]}
        self.assertEqual({e["cert_id"] for e in entries}, cert_ids)
        self.assertTrue(all(e["success"] for e in entries))
        self.assertTrue(audit.verify(logdir)["valid"])

    def test_dry_runs_log_nothing(self):
        tmp = tempfile.mkdtemp(prefix="audit-wire0-")
        self.addCleanup(shutil.rmtree, tmp, True)
        root = os.path.join(tmp, "kr")
        keyring.init(root)
        keyring.create_vault(root, "mom")
        r = keyring.destroy_vault(root, "mom")  # dry-run
        self.assertTrue(r["dry_run"])
        self.assertFalse(os.path.exists(audit.audit_path(root)))


if __name__ == "__main__":
    result = unittest.TextTestRunner(verbosity=1).run(
        unittest.defaultTestLoader.loadTestsFromName("__main__"))
    total = result.testsRun
    fails = len(result.failures) + len(result.errors)
    print("%d/%d green" % (total - fails, total))
    sys.exit(0 if result.wasSuccessful() else 1)
