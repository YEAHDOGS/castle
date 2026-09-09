#!/usr/bin/env python3
"""Regression tests for the flamethrower key lifecycle.

Fixture-based: every test gets a fresh keyring in a temp dir.
Run:  python3 test_keyring.py
Exit 0 = all green, 1 = a regression.
"""

import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import keyring  # noqa: E402


def fresh():
    d = tempfile.mkdtemp(prefix="flamethrower-test-")
    keyring.init(os.path.join(d, "kr"))
    return os.path.join(d, "kr")


def check(name, fn):
    try:
        fn()
    except Exception as e:  # noqa: BLE001
        print("FAIL %-42s %s: %s" % (name, type(e).__name__, e))
        return False
    print("ok   %s" % name)
    return True


def t_create_lists_vault():
    root = fresh()
    keyring.create_vault(root, "mom")
    names = [n for n, _, _, _ in keyring.list_vaults(root)]
    assert names == ["mom"], names
    kp = os.path.join(root, "keys", "mom.key")
    assert os.path.exists(kp)
    assert oct(os.stat(kp).st_mode & 0o777) == "0o600"


def t_duplicate_rejected():
    root = fresh()
    keyring.create_vault(root, "sam")
    try:
        keyring.create_vault(root, "sam")
    except ValueError:
        return
    raise AssertionError("duplicate vault name accepted")


def t_evil_names_rejected():
    root = fresh()
    for evil in ["../escape", "*.key", "a/b", "", "x" * 65]:
        try:
            keyring.create_vault(root, evil)
        except ValueError:
            continue
        raise AssertionError("evil name accepted: %r" % evil)


def t_dry_run_burns_nothing():
    root = fresh()
    keyring.create_vault(root, "sally", escrow=True)
    kp = os.path.join(root, "keys", "sally.key")
    r = keyring.destroy_vault(root, "sally")  # no confirmation
    assert r["dry_run"] is True
    assert os.path.exists(kp), "dry run destroyed the key!"
    assert os.path.exists(os.path.join(root, "escrow", "sally.key"))
    assert "sally" in [n for n, _, _, _ in keyring.list_vaults(root)]


def t_wrong_confirmation_burns_nothing():
    root = fresh()
    keyring.create_vault(root, "dad")
    r = keyring.destroy_vault(root, "dad", confirm="y")
    assert r["dry_run"] is True
    assert os.path.exists(os.path.join(root, "keys", "dad.key"))


def t_real_burn_kills_all_copies_and_issues_cert():
    root = fresh()
    keyring.create_vault(root, "mom", escrow=True)
    kp = os.path.join(root, "keys", "mom.key")
    with open(kp, "rb") as f:
        raw_key = f.read()
    assert len(raw_key) == 32
    r = keyring.destroy_vault(root, "mom", confirm="mom")
    assert r["dry_run"] is False
    assert not os.path.exists(kp), "keyring copy survived"
    assert not os.path.exists(os.path.join(root, "escrow", "mom.key")), \
        "escrow copy survived"
    assert [c["copy"] for c in r["kills"]] == ["keyring", "escrow"]
    assert all(c["unlinked"] for c in r["kills"])
    assert "mom" not in [n for n, _, _, _ in keyring.list_vaults(root)]
    # certificate exists, verifies, and contains no key material
    v = keyring.verify_cert(r["certificate"])
    assert v["valid"], v
    with open(r["certificate"], "rb") as f:
        cert_bytes = f.read()
    assert raw_key not in cert_bytes, "KEY MATERIAL LEAKED INTO CERTIFICATE"
    cert = json.loads(cert_bytes)
    assert cert["vault"] == "mom" and len(cert["copies_destroyed"]) == 2


def t_burn_without_escrow_kills_single_copy():
    root = fresh()
    keyring.create_vault(root, "sam")
    r = keyring.destroy_vault(root, "sam", confirm="sam")
    assert [c["copy"] for c in r["kills"]] == ["keyring"]
    assert keyring.verify_cert(r["certificate"])["valid"]


def t_unknown_vault_rejected():
    root = fresh()
    try:
        keyring.destroy_vault(root, "ghost", confirm="ghost")
    except ValueError:
        return
    raise AssertionError("destroyed a vault that never existed")


def t_master_burn_needs_typed_confirmation():
    root = fresh()
    keyring.create_vault(root, "a")
    keyring.create_vault(root, "b", escrow=True)
    r = keyring.destroy_master(root)
    assert r["dry_run"] is True
    assert len(keyring.list_vaults(root)) == 2
    r = keyring.destroy_master(root, confirm="DESTROY-ALL")
    assert r["dry_run"] is False
    assert len(r["burned"]) == 2
    assert keyring.list_vaults(root) == []
    for b in r["burned"]:
        assert keyring.verify_cert(b["certificate"])["valid"]


def t_tampered_cert_fails_verification():
    root = fresh()
    keyring.create_vault(root, "mom")
    r = keyring.destroy_vault(root, "mom", confirm="mom")
    with open(r["certificate"], encoding="utf-8") as f:
        cert = json.load(f)
    del cert["verification"]  # tamper: drop a required field
    bad = r["certificate"] + ".bad"
    with open(bad, "w", encoding="utf-8") as f:
        json.dump(cert, f)
    v = keyring.verify_cert(bad)
    assert v["valid"] is False and "verification" in v["missing_fields"]


def t_media_detection_returns_sane_label():
    assert keyring.detect_media(tempfile.gettempdir()) in ("hdd", "ssd", "unknown")


TESTS = [
    ("create lists vault, key file 0600", t_create_lists_vault),
    ("duplicate vault rejected", t_duplicate_rejected),
    ("evil names rejected", t_evil_names_rejected),
    ("dry-run burns nothing", t_dry_run_burns_nothing),
    ("wrong typed confirmation burns nothing", t_wrong_confirmation_burns_nothing),
    ("real burn kills keyring+escrow, issues valid cert, no key leak",
     t_real_burn_kills_all_copies_and_issues_cert),
    ("burn without escrow kills single copy", t_burn_without_escrow_kills_single_copy),
    ("unknown vault rejected", t_unknown_vault_rejected),
    ("master burn needs DESTROY-ALL", t_master_burn_needs_typed_confirmation),
    ("tampered cert fails verification", t_tampered_cert_fails_verification),
    ("media detection returns sane label", t_media_detection_returns_sane_label),
]


def main():
    ok = all(check(n, fn) for n, fn in TESTS)
    print("%d/%d green" % (sum(1 for _ in TESTS) if ok else 0, len(TESTS)) if ok
          else "REGRESSIONS PRESENT")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
