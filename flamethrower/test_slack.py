#!/usr/bin/env python3
"""Regression tests for the flamethrower residue wiping (slack.py).

Fakes EVERYTHING: fake sysfs tree (with dev/block/<maj>:<min> symlinks),
fake st_dev resolver, per-test log dirs. Targets live in temp dirs; the
fake sysfs maps them onto fake disks so no test touches real hardware.
The cap-bytes knob keeps free-space wipes fast in tests.
Run:  python3 test_slack.py
Exit 0 = all green, 1 = a regression.
"""

import json
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import slack  # noqa: E402


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
        major, minor = self._devs[name]
        return lambda path: (major, minor)

    def close(self):
        shutil.rmtree(self.tmp, ignore_errors=True)


class Ctx:
    """Per-test sandbox: fake sysfs, target dir, log dir."""

    def __init__(self, disk_kw=None):
        self.sysfs = FakeSysfs()
        self.sysfs.add_disk("hdd0", **(disk_kw or {"rotational": True}))
        self.stat_dev = self.sysfs.stat_dev_for("hdd0")
        self.tmp = tempfile.mkdtemp(prefix="slack-test-")
        self.log = os.path.join(self.tmp, "logs")

    def close(self):
        self.sysfs.close()
        shutil.rmtree(self.tmp, ignore_errors=True)


def kw(ctx, **over):
    d = {"sysfs": ctx.sysfs.root, "stat_dev": ctx.stat_dev,
         "log_dir": ctx.log}
    d.update(over)
    return d


def mkfile(ctx, name, data):
    p = os.path.join(ctx.tmp, name)
    with open(p, "wb") as f:
        f.write(data)
    return p


def audit_entries(ctx):
    path = os.path.join(ctx.log, "audit.jsonl")
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        return [json.loads(l) for l in f if l.strip()]


results = []


def check(name, fn):
    try:
        fn()
    except Exception as e:  # noqa: BLE001
        results.append((name, "FAIL: %r" % (e,)))
    else:
        results.append((name, "ok"))


# ---------------------------------------------------------------- slack ----

def t_slack_dry_run_touches_nothing():
    ctx = Ctx()
    try:
        data = b"x" * 100
        p = mkfile(ctx, "a.bin", data)
        rc, out = slack.wipe_slack(p, **kw(ctx))
        assert rc == 2, rc
        assert open(p, "rb").read() == data, "content changed on dry-run"
        assert os.stat(p).st_size == 100, "size changed on dry-run"
        assert audit_entries(ctx) == [], "dry-run logged"
    finally:
        ctx.close()


def t_slack_preserves_content_and_size():
    ctx = Ctx()
    try:
        data = os.urandom(100)
        p = mkfile(ctx, "b.bin", data)
        before = slack._sha256(p)
        rc, out = slack.wipe_slack(p, confirm_arg="b.bin", dry_run=False,
                                   **kw(ctx))
        assert rc == 0, out
        assert os.stat(p).st_size == 100, "size changed by slack wipe"
        assert slack._sha256(p) == before, "content changed by slack wipe"
    finally:
        ctx.close()


def t_slack_reports_pad_and_emits_cert_and_audit():
    ctx = Ctx()
    try:
        block = os.statvfs(ctx.tmp).f_bsize
        size = block - 7
        p = mkfile(ctx, "c.bin", b"y" * size)
        rc, out = slack.wipe_slack(p, confirm_arg="c.bin", dry_run=False,
                                   **kw(ctx))
        assert rc == 0, out
        cert = out["certificate"]
        assert cert["slack_bytes"] == 7, cert
        assert cert["success"] is True
        assert cert["media"] == "hdd"
        ents = audit_entries(ctx)
        assert len(ents) == 1, ents
        assert ents[0]["tier"] == "3"
        assert ents[0]["cert_id"] == cert["cert_id"]
        assert "y" * 3 not in json.dumps(ents[0]), "contents in audit log"
        assert ents[0]["fingerprint"]["sha256_after"] == slack._sha256(p)
    finally:
        ctx.close()


def t_slack_block_aligned_is_noop_success():
    ctx = Ctx()
    try:
        block = os.statvfs(ctx.tmp).f_bsize
        p = mkfile(ctx, "d.bin", b"z" * block)
        rc, out = slack.wipe_slack(p, confirm_arg="d.bin", dry_run=False,
                                   **kw(ctx))
        assert rc == 0, out
        assert out["certificate"]["slack_bytes"] == 0
        assert out["certificate"]["success"] is True
    finally:
        ctx.close()


def t_slack_refuses_symlink():
    ctx = Ctx()
    try:
        p = mkfile(ctx, "e.bin", b"q" * 10)
        link = os.path.join(ctx.tmp, "e-link.bin")
        os.symlink(p, link)
        try:
            slack.wipe_slack(link, confirm_arg="e-link.bin", dry_run=False,
                              **kw(ctx))
        except Exception as e:
            assert "symlink" in str(e).lower(), e
        else:
            raise AssertionError("symlink slack wipe not refused")
        assert open(p, "rb").read() == b"q" * 10, "target struck through!"
    finally:
        ctx.close()


def t_slack_refuses_directory():
    ctx = Ctx()
    try:
        try:
            slack.wipe_slack(ctx.tmp, confirm_arg="x", dry_run=False,
                              **kw(ctx))
        except Exception as e:
            assert "non-regular" in str(e).lower(), e
        else:
            raise AssertionError("directory slack wipe not refused")
    finally:
        ctx.close()


def t_slack_wrong_confirmation_aborts():
    ctx = Ctx()
    try:
        data = b"w" * 50
        p = mkfile(ctx, "f.bin", data)
        rc, out = slack.wipe_slack(p, confirm_arg="WRONG", dry_run=False,
                                   **kw(ctx))
        assert rc == 2, out
        assert out.get("aborted") is True
        assert open(p, "rb").read() == data
        assert audit_entries(ctx) == []
    finally:
        ctx.close()


def t_slack_flash_labeled_best_effort():
    ctx = Ctx(disk_kw={"rotational": False})  # fake SATA SSD
    try:
        p = mkfile(ctx, "g.bin", b"v" * 13)
        rc, out = slack.wipe_slack(p, confirm_arg="g.bin", dry_run=False,
                                   **kw(ctx))
        assert rc == 0, out
        assert out["certificate"]["media"] != "hdd"
        assert "BEST EFFORT" in out["certificate"]["verification"], out
        assert "flash" in out["certificate"]["media_note"].lower()
    finally:
        ctx.close()


def t_slack_unmappable_media_refuses():
    ctx = Ctx()
    try:
        p = mkfile(ctx, "h.bin", b"u" * 9)
        try:
            slack.wipe_slack(p, confirm_arg="h.bin", dry_run=False,
                              **kw(ctx, stat_dev=lambda path: (250, 250)))
        except Exception as e:
            assert "unknown" in str(e).lower() or "refus" in str(e).lower(), e
        else:
            raise AssertionError("unmappable media not refused")
    finally:
        ctx.close()


# ---------------------------------------------------------------- free -----

def t_free_dry_run_touches_nothing():
    ctx = Ctx()
    try:
        d = os.path.join(ctx.tmp, "target")
        os.makedirs(d)
        rc, out = slack.wipe_free(d, **kw(ctx))
        assert rc == 2, rc
        assert os.listdir(d) == [], os.listdir(d)
        assert audit_entries(ctx) == []
    finally:
        ctx.close()


def t_free_wipe_completes_and_cleans_up():
    ctx = Ctx()
    try:
        d = os.path.join(ctx.tmp, "target")
        os.makedirs(d)
        keep = os.path.join(d, "keep.txt")
        open(keep, "w").write("do not touch")
        rc, out = slack.wipe_free(d, confirm_arg="target", dry_run=False,
                                  cap_bytes=8 * 1024 * 1024, **kw(ctx))
        assert rc == 0, out
        cert = out["certificate"]
        assert cert["kind"] == "wipe-free"
        assert cert["success"] is True
        leftovers = [f for f in os.listdir(d)
                     if f.startswith(slack.FILL_PREFIX)]
        assert leftovers == [], leftovers
        assert open(keep).read() == "do not touch", "user file harmed!"
        ents = audit_entries(ctx)
        assert len(ents) == 1 and ents[0]["tier"] == "3", ents
        assert ents[0]["cert_id"] == cert["cert_id"]
    finally:
        ctx.close()


def t_free_wrong_confirmation_aborts():
    ctx = Ctx()
    try:
        d = os.path.join(ctx.tmp, "target")
        os.makedirs(d)
        rc, out = slack.wipe_free(d, confirm_arg="WRONG", dry_run=False,
                                  **kw(ctx))
        assert rc == 2 and out.get("aborted") is True, out
        assert audit_entries(ctx) == []
    finally:
        ctx.close()


def t_free_refuses_root_and_nondir():
    ctx = Ctx()
    try:
        for bad in ("/", os.path.join(ctx.tmp, "nope.txt")):
            if bad != "/":
                open(bad, "w").write("x")
            try:
                slack.wipe_free(bad, confirm_arg="x", dry_run=False,
                                **kw(ctx))
            except Exception as e:
                assert "refus" in str(e).lower(), e
            else:
                raise AssertionError("not refused: %s" % bad)
    finally:
        ctx.close()


def t_free_flash_labeled_best_effort():
    ctx = Ctx(disk_kw={"rotational": False})
    try:
        d = os.path.join(ctx.tmp, "target")
        os.makedirs(d)
        rc, out = slack.wipe_free(d, confirm_arg="target", dry_run=False,
                                  cap_bytes=1024 * 1024, **kw(ctx))
        assert rc == 0, out
        assert "BEST EFFORT" in out["certificate"]["verification"], out
    finally:
        ctx.close()


def t_cli_dry_run_exit_codes():
    # CLI uses the real st_dev: on real hardware this resolves (rc 0/2);
    # in this sandbox it refuses cleanly (rc 1). Never crashes, never
    # destroys — real burns are exercised through the function API above.
    ctx = Ctx()
    try:
        p = mkfile(ctx, "cli.bin", b"a" * 17)
        d = os.path.join(ctx.tmp, "d")
        os.makedirs(d)
        rc = slack.main(["--sysfs", ctx.sysfs.root, "--log-dir", ctx.log,
                         "wipe-slack", p])
        assert rc in (0, 1, 2), rc
        rc = slack.main(["--sysfs", ctx.sysfs.root, "--log-dir", ctx.log,
                         "wipe-free", d])
        assert rc in (0, 1, 2), rc
        assert open(p, "rb").read() == b"a" * 17, "CLI destroyed something"
    finally:
        ctx.close()


for name, fn in sorted([(k, v) for k, v in globals().items()
                        if k.startswith("t_")]):
    check(name, fn)

failed = [n for n, r in results if r != "ok"]
for n, r in results:
    print("%-42s %s" % (n, r))
print("%d/%d green" % (len(results) - len(failed), len(results)))
sys.exit(1 if failed else 0)
