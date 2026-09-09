#!/usr/bin/env python3
"""Regression tests for the flamethrower per-file crypto-shred (fileburn.py).

Fixture-based: every test gets a fresh fileburn root in a temp dir —
no test touches anything outside its own temp dir. The burn can never
leave the designated files/ / filekeys/ dirs by construction.
Run:  python3 test_fileburn.py
Exit 0 = all green, 1 = a regression.
"""

import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fileburn  # noqa: E402


def fresh():
    d = tempfile.mkdtemp(prefix="fileburn-test-")
    root = os.path.join(d, "fb")
    fileburn.init(root)
    return root


def check(name, fn):
    try:
        fn()
    except Exception as e:  # noqa: BLE001
        print("FAIL %-42s %s: %s" % (name, type(e).__name__, e))
        return False
    print("ok   %s" % name)
    return True


SECRET = b"the family recipe for kimchi: 2 tbsp gochugaru, patience"


def t_seal_unseal_round_trip():
    root = fresh()
    fileburn.seal(root, "recipe", SECRET)
    assert fileburn.unseal(root, "recipe") == SECRET
    # the bundle on disk is ciphertext, not the plaintext
    with open(os.path.join(root, "files", "recipe.sealed"), "rb") as f:
        blob = f.read()
    assert SECRET not in blob, "PLAINTEXT VISIBLE ON DISK"
    assert oct(os.stat(os.path.join(root, "files", "recipe.sealed")
                       ).st_mode & 0o777) == "0o600"


def t_key_file_tight_and_registry_holds_no_key():
    root = fresh()
    e = fileburn.seal(root, "recipe", SECRET)
    kp = os.path.join(root, "filekeys", "recipe.key")
    with open(kp, "rb") as f:
        raw_key = f.read()
    assert len(raw_key) == 32
    assert oct(os.stat(kp).st_mode & 0o777) == "0o600"
    with open(os.path.join(root, "files.json"), "rb") as f:
        reg_bytes = f.read()
    assert raw_key not in reg_bytes, "KEY MATERIAL IN REGISTRY"
    reg = json.loads(reg_bytes)
    assert reg["files"]["recipe"]["key_sha256"]  # receipt only
    assert e["key_id"] == reg["files"]["recipe"]["key_id"]


def t_duplicate_and_evil_names_refused():
    root = fresh()
    fileburn.seal(root, "recipe", SECRET)
    try:
        fileburn.seal(root, "recipe", SECRET)
    except ValueError:
        pass
    else:
        raise AssertionError("duplicate seal accepted")
    for evil in ["../escape", "a/b", "*.sealed", "", "x" * 65]:
        try:
            fileburn.seal(root, evil, SECRET)
        except ValueError:
            continue
        raise AssertionError("evil name accepted: %r" % evil)
        try:
            fileburn.burn(root, evil, confirm=evil)
        except ValueError:
            continue
        raise AssertionError("evil burn name accepted: %r" % evil)


def t_dry_run_burns_nothing_and_logs_nothing():
    root = fresh()
    fileburn.seal(root, "recipe", SECRET, escrow=True)
    r = fileburn.burn(root, "recipe")  # no confirmation
    assert r["dry_run"] is True
    assert "filekeys" in r["would_destroy"][0]
    assert "escrow" in r["would_destroy"][1]
    assert "ciphertext" in r["would_destroy"][2]
    assert r["hint"].endswith("--yes recipe to burn")
    assert os.path.exists(os.path.join(root, "filekeys", "recipe.key"))
    assert os.path.exists(os.path.join(root, "escrow", "recipe.key"))
    assert os.path.exists(os.path.join(root, "files", "recipe.sealed"))
    assert "recipe" in [n for n, _, _, _, _ in fileburn.list_files(root)]
    # dry runs are invisible to the audit log
    assert not os.path.exists(os.path.join(root, "audit.jsonl"))


def t_wrong_confirmation_burns_nothing():
    root = fresh()
    fileburn.seal(root, "recipe", SECRET)
    r = fileburn.burn(root, "recipe", confirm="y")
    assert r["dry_run"] is True
    assert os.path.exists(os.path.join(root, "filekeys", "recipe.key"))
    assert os.path.exists(os.path.join(root, "files", "recipe.sealed"))


def t_real_burn_kills_key_ciphertext_cert_and_audit():
    root = fresh()
    e = fileburn.seal(root, "recipe", SECRET, escrow=True)
    kp = os.path.join(root, "filekeys", "recipe.key")
    with open(kp, "rb") as f:
        raw_key = f.read()
    r = fileburn.burn(root, "recipe", confirm="recipe")
    assert r["dry_run"] is False
    assert not os.path.exists(kp), "keyring copy survived"
    assert not os.path.exists(os.path.join(root, "escrow", "recipe.key")), \
        "escrow copy survived"
    assert not os.path.exists(
        os.path.join(root, "files", "recipe.sealed")), "ciphertext survived"
    assert [c["copy"] for c in r["kills"]] == ["filekeys", "escrow"]
    assert all(c["unlinked"] for c in r["kills"])
    assert r["ciphertext_bytes_overwritten"] > 0
    assert "recipe" not in [n for n, _, _, _, _ in fileburn.list_files(root)]
    # the key is gone: unseal is now impossible
    try:
        fileburn.unseal(root, "recipe")
    except ValueError:
        pass
    else:
        raise AssertionError("unseal succeeded after key destruction")
    # certificate verifies, carries no key material and no file contents
    v = fileburn.verify_cert(r["certificate"])
    assert v["valid"], v
    with open(r["certificate"], "rb") as f:
        cert_bytes = f.read()
    assert raw_key not in cert_bytes, "KEY MATERIAL LEAKED INTO CERTIFICATE"
    assert SECRET not in cert_bytes, "FILE CONTENTS LEAKED INTO CERTIFICATE"
    cert = json.loads(cert_bytes)
    assert cert["file"] == "recipe"
    assert cert["key_id"] == e["key_id"]
    # one tier-1 audit record, fingerprint only (never contents)
    with open(os.path.join(root, "audit.jsonl"), encoding="utf-8") as f:
        lines = [json.loads(l) for l in f if l.strip()]
    assert len(lines) == 1, lines
    assert lines[0]["tier"] == "1" and lines[0]["cert_id"] == r["cert_id"]
    assert lines[0]["fingerprint"]["key_id"] == e["key_id"]
    assert SECRET.decode() not in json.dumps(lines[0]), \
        "FILE CONTENTS LEAKED INTO AUDIT LOG"


def t_burn_without_escrow_kills_single_key_copy():
    root = fresh()
    fileburn.seal(root, "note", b"buy milk")
    r = fileburn.burn(root, "note", confirm="note")
    assert [c["copy"] for c in r["kills"]] == ["filekeys"]
    assert fileburn.verify_cert(r["certificate"])["valid"]


def t_unknown_file_refused():
    root = fresh()
    try:
        fileburn.burn(root, "ghost", confirm="ghost")
    except ValueError:
        return
    raise AssertionError("burned a file that was never sealed")


def t_burn_never_escapes_designated_dirs():
    root = fresh()
    # a canary file outside files/ must survive even a hostile burn attempt
    canary = os.path.join(root, "canary.txt")
    with open(canary, "w") as f:
        f.write("do not touch")
    fileburn.seal(root, "recipe", SECRET)
    fileburn.burn(root, "recipe", confirm="recipe")
    with open(canary) as f:
        assert f.read() == "do not touch", "BURN TOUCHED OUTSIDE files/"


def t_tampered_cert_fails_verification():
    root = fresh()
    fileburn.seal(root, "recipe", SECRET)
    r = fileburn.burn(root, "recipe", confirm="recipe")
    with open(r["certificate"], encoding="utf-8") as f:
        cert = json.load(f)
    cert["copies_destroyed"][0]["unlinked"] = False  # tamper
    bad = r["certificate"] + ".bad"
    with open(bad, "w", encoding="utf-8") as f:
        json.dump(cert, f)
    v = fileburn.verify_cert(bad)
    assert v["valid"] is False


def t_media_detection_returns_sane_label():
    assert fileburn.detect_media(tempfile.gettempdir()) in (
        "hdd", "ssd", "unknown")


TESTS = [
    ("seal/unseal round-trip, ciphertext only on disk", t_seal_unseal_round_trip),
    ("key file 0600, registry holds receipt not key",
     t_key_file_tight_and_registry_holds_no_key),
    ("duplicates and evil names refused", t_duplicate_and_evil_names_refused),
    ("dry-run burns nothing, logs nothing", t_dry_run_burns_nothing_and_logs_nothing),
    ("wrong typed confirmation burns nothing", t_wrong_confirmation_burns_nothing),
    ("real burn kills key+escrow+ciphertext, cert+audit clean",
     t_real_burn_kills_key_ciphertext_cert_and_audit),
    ("burn without escrow kills single key copy",
     t_burn_without_escrow_kills_single_key_copy),
    ("unknown file refused", t_unknown_file_refused),
    ("burn never escapes the designated dirs",
     t_burn_never_escapes_designated_dirs),
    ("tampered cert fails verification", t_tampered_cert_fails_verification),
    ("media detection returns sane label", t_media_detection_returns_sane_label),
]


def main():
    ok = all(check(n, fn) for n, fn in TESTS)
    print("%d/%d green" % (len(TESTS) if ok else 0, len(TESTS)) if ok
          else "REGRESSIONS PRESENT")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
