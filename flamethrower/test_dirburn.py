#!/usr/bin/env python3
"""Regression tests for the flamethrower whole-directory burn (dirburn.py).

Fixture-based: every test gets a fresh fileburn root AND a fresh target
dir in temp dirs — no test touches anything outside its own temp dirs.
Run:  python3 test_dirburn.py
Exit 0 = all green, 1 = a regression.
"""

import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dirburn  # noqa: E402
import fileburn  # noqa: E402


def fresh():
    d = tempfile.mkdtemp(prefix="dirburn-test-")
    root = os.path.join(d, "fb")
    fileburn.init(root)
    target = os.path.join(d, "victim")
    os.makedirs(target)
    return root, target


def plant(target, relpath, data=b"secret-data"):
    p = os.path.join(target, relpath)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "wb") as f:
        f.write(data)
    return p


def check(name, fn):
    try:
        fn()
    except Exception as e:  # noqa: BLE001
        print("FAIL %-42s %s: %s" % (name, type(e).__name__, e))
        return False
    print("ok   %s" % name)
    return True


SECRET = b"never-write-this-plainly"


def t_dry_run_burns_nothing_and_writes_no_manifest():
    root, target = fresh()
    plant(target, "a.txt", SECRET)
    plant(target, "sub/b.txt", SECRET)
    before = os.listdir(os.path.join(root, "manifests")) \
        if os.path.exists(os.path.join(root, "manifests")) else []
    r = dirburn.burn_directory(root, target)
    assert r["dry_run"] is True
    assert len(r["would_burn_files"]) == 2
    assert os.path.isfile(os.path.join(target, "a.txt")), "DRY RUN BURNED A FILE"
    assert os.path.isdir(target)
    assert os.listdir(os.path.join(root, "manifests")) == before, \
        "DRY RUN WROTE A MANIFEST"
    assert not os.path.exists(os.path.join(root, "audit.jsonl")), \
        "DRY RUN TOUCHED THE AUDIT LOG"


def t_wrong_confirmation_burns_nothing():
    root, target = fresh()
    plant(target, "a.txt", SECRET)
    r = dirburn.burn_directory(root, target, confirm="not-the-name")
    assert r["dry_run"] is True
    assert os.path.isfile(os.path.join(target, "a.txt")), \
        "WRONG CONFIRMATION BURNED A FILE"


def t_real_burn_kills_all_files_manifest_valid_verify_passes():
    root, target = fresh()
    plant(target, "a.txt", SECRET)
    plant(target, "sub/b.txt", SECRET)
    plant(target, "sub/deep/c.txt", SECRET)
    r = dirburn.burn_directory(root, target,
                               confirm=os.path.basename(target))
    assert r["dry_run"] is False
    assert r["files_burned"] == 3
    assert r["verified"] is True
    # every original is gone, target root gone with its emptied subdirs
    assert not os.path.lexists(target), "TARGET ROOT SURVIVED"
    # manifest shape: path, bytes, sha256-before, timestamp — hashes only
    with open(r["manifest"], encoding="utf-8") as f:
        m = json.load(f)
    assert m["manifest_version"] == 1
    assert m["target"] == os.path.realpath(target)
    assert m["burn_id"] == r["burn_id"]
    assert len(m["files"]) == 3
    for e in m["files"]:
        assert all(k in e for k in
                   ("path", "bytes", "sha256_before", "sealed_name",
                    "cert_id", "burned_at", "status"))
        assert e["status"] == "burned"
        assert e["bytes"] == len(SECRET)
        assert e["burned_at"].endswith("Z")
    with open(r["manifest"], "rb") as f:
        mbytes = f.read()
    assert SECRET not in mbytes, "PLAINTEXT IN MANIFEST"
    # one audit record per burned file
    with open(os.path.join(root, "audit.jsonl"), encoding="utf-8") as f:
        assert len(f.read().strip().split("\n")) == 3
    # verify_manifest agrees the burn is clean
    v = dirburn.verify_manifest(root, r["manifest"])
    assert v["valid"] is True and v["survivors"] == []


def t_verify_pass_detects_a_surviving_file():
    root, target = fresh()
    a = plant(target, "a.txt", SECRET)
    plant(target, "b.txt", SECRET)
    r = dirburn.burn_directory(root, target,
                               confirm=os.path.basename(target))
    # plant a survivor: restore one of the manifest-listed paths
    with open(r["manifest"], encoding="utf-8") as f:
        m = json.load(f)
    os.makedirs(os.path.dirname(m["files"][0]["path"]), exist_ok=True)
    with open(m["files"][0]["path"], "wb") as f:
        f.write(b"i survived")
    v = dirburn.verify_manifest(root, r["manifest"])
    assert v["valid"] is False, "VERIFY PASSED WITH A SURVIVOR"
    assert v["survivors"] == [m["files"][0]["path"]], \
        "SURVIVOR PATH NOT NAMED LOUDLY"


def t_canary_escape_attempts_blocked_and_canary_survives():
    root, target = fresh()
    plant(target, "a.txt", SECRET)
    canary_dir = tempfile.mkdtemp(prefix="dirburn-canary-")
    canary = os.path.join(canary_dir, "do-not-touch.txt")
    with open(canary, "w") as f:
        f.write("canary")
    # symlink inside the target pointing outside the target
    os.symlink(canary, os.path.join(target, "evil-link"))
    try:
        dirburn.burn_directory(root, target,
                               confirm=os.path.basename(target))
    except ValueError:
        pass
    else:
        raise AssertionError("symlink inside target did not abort the burn")
    with open(canary) as f:
        assert f.read() == "canary", "BURN TOUCHED THE CANARY"
    assert os.path.isfile(os.path.join(target, "a.txt")), \
        "BURN PARTIALLY RAN BEFORE THE SYMLINK REFUSAL"
    # symlinked dirs are refused too
    os.unlink(os.path.join(target, "evil-link"))
    os.symlink(canary_dir, os.path.join(target, "evil-dir"))
    try:
        dirburn.burn_directory(root, target,
                               confirm=os.path.basename(target))
    except ValueError:
        pass
    else:
        raise AssertionError("symlinked dir inside target not refused")
    with open(canary) as f:
        assert f.read() == "canary", "BURN TOUCHED THE CANARY"
    # the target itself being a symlink is refused outright
    link_target = os.path.join(os.path.dirname(target), "victim-link")
    os.symlink(target, link_target)
    try:
        dirburn.burn_directory(root, link_target,
                               confirm="victim-link")
    except ValueError:
        pass
    else:
        raise AssertionError("symlink target not refused")


def t_refused_targets():
    root, target = fresh()
    plant(target, "a.txt", SECRET)
    for bad in ("/", root, os.path.join(root, "files")):
        try:
            dirburn.burn_directory(root, bad, confirm="x")
        except ValueError:
            pass
        else:
            raise AssertionError("refused target accepted: %s" % bad)
    # a target that CONTAINS the fileburn root is suicide — refused
    outside = tempfile.mkdtemp(prefix="dirburn-outer-")
    newroot = os.path.join(outside, "fb")
    fileburn.init(newroot)
    try:
        dirburn.burn_directory(newroot, outside, confirm="x")
    except ValueError:
        pass
    else:
        raise AssertionError("target containing the root not refused")
    assert os.path.isfile(os.path.join(target, "a.txt")), \
        "refused burn touched the fixture"


def t_empty_dir_burns_cleanly():
    root, target = fresh()
    r = dirburn.burn_directory(root, target,
                               confirm=os.path.basename(target))
    assert r["dry_run"] is False
    assert r["files_burned"] == 0
    assert r["verified"] is True
    with open(r["manifest"], encoding="utf-8") as f:
        m = json.load(f)
    assert m["files"] == []
    assert dirburn.verify_manifest(root, r["manifest"])["valid"] is True


def t_hardlink_duplicate_and_nonregular_refused():
    root, target = fresh()
    a = plant(target, "a.txt", SECRET)
    os.link(a, os.path.join(target, "a-hardlink.txt"))
    try:
        dirburn.burn_directory(root, target,
                               confirm=os.path.basename(target))
    except ValueError:
        pass
    else:
        raise AssertionError("hardlink duplicate not refused")
    assert os.path.isfile(a), "REFUSED BURN TOUCHED THE FIXTURE"


TESTS = [
    ("dry-run burns nothing, writes no manifest, logs nothing",
     t_dry_run_burns_nothing_and_writes_no_manifest),
    ("wrong typed confirmation burns nothing",
     t_wrong_confirmation_burns_nothing),
    ("real burn: 3 files killed, manifest valid, verify passes",
     t_real_burn_kills_all_files_manifest_valid_verify_passes),
    ("verify pass detects a surviving file, names it",
     t_verify_pass_detects_a_surviving_file),
    ("canary escape attempts blocked (symlink file/dir/target)",
     t_canary_escape_attempts_blocked_and_canary_survives),
    ("refused targets: /, root itself, target containing root",
     t_refused_targets),
    ("empty dir burns cleanly", t_empty_dir_burns_cleanly),
    ("hardlink duplicates refused", t_hardlink_duplicate_and_nonregular_refused),
]


def main():
    ok = all(check(n, fn) for n, fn in TESTS)
    print("%d/%d green" % (len(TESTS) if ok else 0, len(TESTS)) if ok
          else "REGRESSIONS PRESENT")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
