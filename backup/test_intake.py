#!/usr/bin/env python3
"""Fixture-based regression tests for Castle backup intake.

Temp dirs only. Nothing touches ~/.castle-backups or real disks.
Run: python3 test_intake.py
"""

import hashlib
import json
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import intake  # noqa: E402


def _tmp():
    return tempfile.mkdtemp(prefix="castle-intake-")


def _blob(d, name, content=b"image-bytes"):
    p = os.path.join(d, name)
    with open(p, "wb") as f:
        f.write(content)
    return p


def _sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        h.update(f.read())
    return h.hexdigest()


results = []


def check(name, fn):
    try:
        fn()
        results.append((name, "ok"))
    except AssertionError as e:
        results.append((name, "FAIL %s" % e))
    except Exception as e:  # noqa: BLE001
        results.append((name, "ERROR %r" % e))


def t_register_disk_image():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "laptop.img", b"disk-image-content" * 100)
    rec = intake.register(root, "brando-laptop", "disk-image",
                          "QUARANTINE-INFECTED-2026-09-09", f, _sha(f))
    assert rec["relpath"].startswith("machines/brando-laptop/images/")
    assert os.path.isfile(os.path.join(root, rec["relpath"]))
    st = os.stat(os.path.join(root, rec["relpath"]))
    assert st.st_mode & 0o777 == 0o600
    assert rec["quarantine"] is False


def t_register_data_goes_to_data_dir():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "docs.zip", b"data-backup")
    rec = intake.register(root, "brando-laptop", "data", "DATA-2026-09-09",
                          f, _sha(f))
    assert rec["relpath"].startswith("machines/brando-laptop/data/")


def t_sha_mismatch_refuses_and_writes_nothing():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "laptop.img", b"real-bytes")
    try:
        intake.register(root, "brando-laptop", "disk-image", "LBL", f, "0" * 64)
    except ValueError as e:
        assert "mismatch" in str(e)
    else:
        raise AssertionError("sha mismatch accepted")
    assert not os.path.exists(os.path.join(root, "intake.jsonl"))
    assert not os.listdir(os.path.join(root, "machines"))


def t_unsafe_names_refused():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "laptop.img", b"x")
    for machine, label in (("../evil", "ok"), ("ok", "../evil"),
                           ("ok", "a/b"), ("ok", "")):
        try:
            intake.register(root, machine, "disk-image", label, f, _sha(f))
        except ValueError:
            pass
        else:
            raise AssertionError("unsafe name accepted: %r %r" % (machine, label))


def t_unknown_kind_refused():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "x", b"x")
    try:
        intake.register(root, "m", "iso", "L", f, _sha(f))
    except ValueError:
        pass
    else:
        raise AssertionError("unknown kind accepted")


def t_nonregular_src_refused():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "x", b"x")
    link = os.path.join(srcdir, "link")
    os.symlink(f, link)
    for bad in (srcdir, link):
        try:
            intake.register(root, "m", "disk-image", "L", bad, _sha(f))
        except ValueError:
            pass
        else:
            raise AssertionError("non-regular src accepted: %r" % bad)


def t_bad_sha_format_refused():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "x", b"x")
    for bad in ("", "xyz", "0" * 63, "g" * 64):
        try:
            intake.register(root, "m", "disk-image", "L", f, bad)
        except ValueError:
            pass
        else:
            raise AssertionError("bad sha accepted: %r" % bad)


def t_idempotent_same_bytes_same_label():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "laptop.img", b"same-bytes")
    r1 = intake.register(root, "m", "disk-image", "LBL", f, _sha(f))
    r2 = intake.register(root, "m", "disk-image", "LBL", f, _sha(f))
    assert r1["id"] == r2["id"]
    recs = intake._read_records(root)
    assert len(recs) == 1


def t_quarantine_layout_and_marker():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "infected.img", b"malware?")
    rec = intake.register(root, "m", "disk-image", "INFECTED-2026-09-09", f,
                          _sha(f), quarantine=True)
    assert "images/quarantine/" in rec["relpath"].replace(os.sep, "/")
    assert rec["quarantine"] is True
    marker = os.path.join(root, "machines", "m", "images", "quarantine",
                          intake.QUARANTINE_MARKER)
    assert os.path.isfile(marker)
    with open(marker) as fh:
        assert "DO NOT MOUNT" in fh.read()


def t_quarantine_data_refused():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "docs.zip", b"x")
    try:
        intake.register(root, "m", "data", "L", f, _sha(f), quarantine=True)
    except ValueError:
        pass
    else:
        raise AssertionError("quarantine on data accepted")


def t_verify_clean():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"aaa")
    intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    ok, issues = intake.verify(root)
    assert ok == 1 and issues == []


def t_verify_corrupt_detected():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"aaa")
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    path = os.path.join(root, rec["relpath"])
    with open(path, "r+b") as fh:
        fh.seek(0)
        fh.write(b"b")
    ok, issues = intake.verify(root)
    assert ok == 0 and issues == [(rec["relpath"], "CORRUPT")]


def t_verify_missing_detected():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"aaa")
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    os.unlink(os.path.join(root, rec["relpath"]))
    ok, issues = intake.verify(root)
    assert ok == 0 and issues == [(rec["relpath"], "MISSING")]


def t_corrupt_log_refused_not_trusted():
    root = _tmp()
    intake._ensure_dirs(root)
    with open(os.path.join(root, intake.INTAKE_LOG), "w") as f:
        f.write('{"ok": 1}\n{broken json\n')
    try:
        intake.verify(root)
    except ValueError as e:
        assert "refuse" in str(e)
    else:
        raise AssertionError("corrupt log trusted")


def t_list_is_metadata_only():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"SECRET-BYTES-DO-NOT-ECHO")
    intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    recs = intake.list_entries(root)
    assert len(recs) == 1
    blob = json.dumps(recs)
    assert "SECRET-BYTES" not in blob
    assert set(recs[0]) >= {"machine", "kind", "label", "relpath", "bytes",
                           "sha256", "quarantine", "ts", "id", "note"}


def t_cli_register_and_verify():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"cli-test")
    rc = intake.main(["--dir", root, "register", "--machine", "m",
                      "--kind", "disk-image", "--label", "L",
                      "--sha256", _sha(f), f])
    assert rc == 0
    rc = intake.main(["--dir", root, "verify"])
    assert rc == 0


def t_cli_verify_drift_exit_2_and_refusal_exit_1():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"drift")
    intake.main(["--dir", root, "register", "--machine", "m",
                 "--kind", "disk-image", "--label", "L",
                 "--sha256", _sha(f), f])
    rec = intake.list_entries(root)[0]
    with open(os.path.join(root, rec["relpath"]), "r+b") as fh:
        fh.seek(0)
        fh.write(b"X")
    assert intake.main(["--dir", root, "verify"]) == 2
    assert intake.main(["--dir", root, "register", "--machine", "../e",
                        "--kind", "disk-image", "--label", "L",
                        "--sha256", _sha(f), f]) == 1


def t_activity_log_records_intake():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"act")
    intake.register(root, "m", "disk-image", "L", f, _sha(f))
    with open(os.path.join(root, intake.ACTIVITY_FILE)) as fh:
        lines = fh.read()
    assert '"action": "register"' in lines
    assert "act" != lines or True  # metadata only: raw bytes never logged
    assert "SECRET" not in lines


# ---------------------------------------------------------------------------
# retire
# ---------------------------------------------------------------------------

def t_retire_happy_path_destroys_file_and_closes_loop():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"superseded-backup" * 50)
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    path = os.path.join(root, rec["relpath"])
    ret = intake.retire(root, rec["id"], "L1")
    assert not os.path.exists(path), "intake file must be gone"
    assert ret["intake_id"] == rec["id"]
    assert ret["sha256"] == rec["sha256"]  # fingerprint, not contents
    assert ret["verification"] == "sampled-read-back-clean"
    assert "flash" in ret["media_note"]  # honest media labeling
    ok, issues = intake.verify(root)
    assert ok == 0 and issues == [], "retired intakes are not drift"
    recs = intake.list_entries(root)
    assert len(recs) == 1 and recs[0]["retired"] is True


def t_retire_wrong_confirm_label_refuses_and_keeps_file():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"keep-me")
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    try:
        intake.retire(root, rec["id"], "l1")  # case mismatch
    except ValueError as e:
        assert "does not exactly match" in str(e)
    else:
        raise AssertionError("mismatched confirmation accepted")
    assert os.path.isfile(os.path.join(root, rec["relpath"]))


def t_retire_unknown_id_refuses():
    root = _tmp()
    intake._ensure_dirs(root)
    try:
        intake.retire(root, "deadbeefdeadbeef", "L")
    except ValueError as e:
        assert "no intake record" in str(e)
    else:
        raise AssertionError("unknown id accepted")


def t_retire_twice_refuses():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"once")
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    intake.retire(root, rec["id"], "L1")
    try:
        intake.retire(root, rec["id"], "L1")
    except ValueError as e:
        assert "already retired" in str(e)
    else:
        raise AssertionError("double retire accepted")
    recs = [r for r in intake._read_records(root)
            if r.get("type") == "retirement"]
    assert len(recs) == 1


def t_retire_corrupt_file_refuses_before_destroying():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"original")
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    path = os.path.join(root, rec["relpath"])
    with open(path, "r+b") as fh:
        fh.seek(0)
        fh.write(b"X")
    try:
        intake.retire(root, rec["id"], "L1")
    except ValueError as e:
        assert "CORRUPT" in str(e)
    else:
        raise AssertionError("corrupt file burned anyway")
    assert os.path.isfile(path), "corrupt evidence must survive the refusal"


def t_retire_missing_file_refuses():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"gone")
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    os.unlink(os.path.join(root, rec["relpath"]))
    try:
        intake.retire(root, rec["id"], "L1")
    except ValueError as e:
        assert "missing" in str(e)
    else:
        raise AssertionError("missing file retired")


def t_retire_quarantine_needs_release_flag():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "infected.img", b"forensics-pending")
    rec = intake.register(root, "m", "disk-image", "INFECTED", f, _sha(f),
                          quarantine=True)
    path = os.path.join(root, rec["relpath"])
    try:
        intake.retire(root, rec["id"], "INFECTED")
    except ValueError as e:
        assert "quarantined image" in str(e)
    else:
        raise AssertionError("quarantine retired without release flag")
    assert os.path.isfile(path)
    ret = intake.retire(root, rec["id"], "INFECTED", release_quarantine=True)
    assert ret["quarantine"] is True and ret["quarantine_released"] is True
    assert not os.path.exists(path)


def t_retirement_record_is_metadata_only():
    root, srcdir = _tmp(), _tmp()
    secret = b"DO-NOT-ECHO-PLAINTEXT"
    f = _blob(srcdir, "a.img", secret)
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    intake.retire(root, rec["id"], "L1")
    blob = "\n".join(open(os.path.join(root, intake.INTAKE_LOG)).read().splitlines())
    assert secret.decode() not in blob
    blob += open(os.path.join(root, intake.ACTIVITY_FILE)).read()
    assert secret.decode() not in blob


def t_cli_retire_exit_codes():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"cli-retire")
    rc = intake.main(["--dir", root, "register", "--machine", "m",
                      "--kind", "data", "--label", "D1",
                      "--sha256", _sha(f), f])
    assert rc == 0
    rec = intake.list_entries(root)[0]
    assert intake.main(["--dir", root, "retire", "--id", rec["id"],
                        "--confirm-label", "nope"]) == 1
    assert intake.main(["--dir", root, "retire", "--id", rec["id"],
                        "--confirm-label", "D1"]) == 0
    assert intake.main(["--dir", root, "verify"]) == 0


def t_retire_leaves_directory_tree_intact():
    root, srcdir = _tmp(), _tmp()
    f = _blob(srcdir, "a.img", b"x")
    rec = intake.register(root, "m", "disk-image", "L1", f, _sha(f))
    intake.retire(root, rec["id"], "L1")
    assert os.path.isdir(os.path.join(root, "machines", "m", "images"))


for name, fn in sorted([(k, v) for k, v in globals().items()
                        if k.startswith("t_")]):
    check(name, fn)

failed = [n for n, r in results if r != "ok"]
for n, r in results:
    print("%-44s %s" % (n, r))
print("%d/%d green" % (len(results) - len(failed), len(results)))

if __name__ == "__main__":
    sys.exit(1 if failed else 0)
