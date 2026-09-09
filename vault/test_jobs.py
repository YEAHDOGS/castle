#!/usr/bin/env python3
"""Regression tests for vault/jobs.py — scheduled backup jobs
(FAMILY-DATA-VAULT.md step 8, scheduled lane).

Fixture-based, temp dirs only. Secrets are injected via the ``env=``
argument (a test-local dict) — never through the real environment, and
never written into a manifest.
"""

import calendar
import json
import os
import shutil
import sys
import tempfile
import time
import unittest

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault  # noqa: E402
import jobs as vault_jobs  # noqa: E402
import backup as vault_backup  # noqa: E402


def fresh_dir(tc, prefix):
    tmp = tempfile.mkdtemp(prefix=prefix)
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_root(tc):
    root = fresh_dir(tc, "castle-jobs-test-root-")
    vault.create_user(root, "sam", tier="adult")
    vault.create_user(root, "sally", tier="child", guardian="sam")
    return root


def put(root, user, rel, data=b"data"):
    full = os.path.join(root, "vaults", user, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "wb") as f:
        f.write(data)
    return full


def mk_manifest(tc, target, jobs):
    path = os.path.join(fresh_dir(tc, "castle-jobs-test-manifest-"),
                        "backup-jobs.json")
    doc = {"version": 1, "jobs": jobs}
    with open(path, "w", encoding="utf-8") as f:
        json.dump(doc, f)
    return path


def job(name="nightly-sam", user="sam", target=None, env="CASTLE_PW_SAM",
        schedule=None, retention=None, enabled=True, chunks=False,
        **extra):
    j = {"name": name, "user": user, "target_dir": target or "/nonexistent",
         "passphrase_env": env,
         "schedule": schedule or {"kind": "interval", "hours": 24},
         "enabled": enabled, "chunks": chunks}
    if retention is not None:
        j["retention"] = retention
    j.update(extra)
    return j


class ValidateTest(unittest.TestCase):
    def test_good_manifest_loads(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target, [job(target=target)])
        jobs = vault_jobs.load_jobs(root, p)
        self.assertEqual(len(jobs), 1)
        self.assertEqual(jobs[0]["secret_env"], "CASTLE_PW_SAM")
        self.assertEqual(jobs[0]["target_dir"], os.path.realpath(target))

    def test_embedded_passphrase_value_is_refusal(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        j = job(target=target)
        j["passphrase"] = "hunter2"  # the sin this module exists to kill
        p = mk_manifest(self, target, [j])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_missing_jobs_file_is_refusal(self):
        root = mk_root(self)
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, "/nonexistent/jobs.json")

    def test_unknown_user_is_refusal(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target, [job(target=target, user="ghost")])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_duplicate_names_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target,
                        [job(target=target), job(target=target)])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_bad_env_name_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target, [job(target=target, env="9lives")])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_zero_secret_sources_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        j = job(target=target)
        del j["passphrase_env"]
        p = mk_manifest(self, target, [j])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_two_secret_sources_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        j = job(target=target, keyfile_env="CASTLE_KEY_SAM")
        p = mk_manifest(self, target, [j])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_bad_schedule_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        for bad in ({"kind": "interval", "hours": 0},
                    {"kind": "daily", "at": "25:00"},
                    {"kind": "daily", "at": "noon"},
                    {"kind": "weekly"}):
            p = mk_manifest(self, target,
                            [job(target=target, schedule=bad)])
            with self.assertRaises(vault_jobs.JobError):
                vault_jobs.load_jobs(root, p)

    def test_insane_retention_refused(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target,
                        [job(target=target,
                             retention={"keep_last": 0})])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)
        p = mk_manifest(self, target,
                        [job(target=target,
                             retention={"max_age_days": -3})])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_target_inside_vault_refused(self):
        root = mk_root(self)
        inside = os.path.join(root, "vaults", "sam", "evil-target")
        os.makedirs(inside)
        p = mk_manifest(self, inside, [job(target=inside)])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_symlink_target_refused(self):
        root = mk_root(self)
        real = fresh_dir(self, "castle-jobs-test-target-")
        link = os.path.join(fresh_dir(self, "castle-jobs-test-linkdir-"),
                            "target")
        os.symlink(real, link)
        p = mk_manifest(self, real, [job(target=link)])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.load_jobs(root, p)

    def test_manifest_never_holds_secret_values(self):
        # the anti-leak canary: even the TEST manifest bytes must not
        # contain the passphrase value — the value only ever exists
        # in the env dict.
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target, [job(target=target)])
        with open(p, "rb") as f:
            blob = f.read()
        self.assertNotIn(b"s3cret-passphrase", blob)
        self.assertNotIn(b"CASTLE_PW_SAM\x00", blob)


class DueTest(unittest.TestCase):
    def test_never_run_is_always_due(self):
        j = job(schedule={"kind": "interval", "hours": 24})
        j = dict(j, schedule={"kind": "interval", "hours": 24})
        jv = {"schedule": {"kind": "interval", "hours": 24}}
        self.assertTrue(vault_jobs.is_due(jv, None, now=1_000_000))

    def test_interval_due_logic(self):
        jv = {"schedule": {"kind": "interval", "hours": 24}}
        now = 1_000_000
        self.assertTrue(vault_jobs.is_due(jv, now - 25 * 3600, now))
        self.assertFalse(vault_jobs.is_due(jv, now - 1 * 3600, now))

    def test_daily_due_logic(self):
        # daily at 02:00 UTC; now = 2026-09-09 06:00 UTC
        jv = {"schedule": {"kind": "daily", "at": "02:00"}}
        now = calendar.timegm((2026, 9, 9, 6, 0, 0, 0, 0, 0))
        today_0200 = calendar.timegm((2026, 9, 9, 2, 0, 0, 0, 0, 0))
        self.assertTrue(vault_jobs.is_due(jv, today_0200 - 3600, now))
        self.assertFalse(vault_jobs.is_due(jv, today_0200 + 60, now))
        # before the daily time, not due even with an old last_run
        early = calendar.timegm((2026, 9, 9, 1, 0, 0, 0, 0, 0))
        self.assertFalse(vault_jobs.is_due(jv, today_0200 - 86400, early))

    def test_disabled_job_never_runs(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        put(root, "sam", "receipts/r.txt", b"x")
        p = mk_manifest(self, target,
                        [job(target=target, enabled=False)])
        r = vault_jobs.run_due(root, p, execute=True,
                               env={"CASTLE_PW_SAM": "s3cret-passphrase"})
        self.assertEqual(r["ran"], [])
        self.assertEqual(r["skipped"][0]["reason"], "disabled")
        self.assertEqual(os.listdir(target), [])


class RunnerTest(unittest.TestCase):
    def test_dry_run_default_writes_nothing(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        put(root, "sam", "receipts/r.txt", b"receipt")
        p = mk_manifest(self, target, [job(target=target)])
        r = vault_jobs.run_due(root, p, execute=False,
                               env={"CASTLE_PW_SAM": "s3cret-passphrase"})
        self.assertTrue(r["dry_run"])
        self.assertEqual(r["ran"], [])
        self.assertEqual(len(r["plans"]), 1)
        self.assertEqual(os.listdir(target), [])
        self.assertFalse(os.path.exists(p + ".state"))

    def test_execute_backs_up_and_records_state(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        put(root, "sam", "receipts/r.txt", b"receipt")
        p = mk_manifest(self, target, [job(target=target)])
        r = vault_jobs.run_due(root, p, execute=True,
                               env={"CASTLE_PW_SAM": "s3cret-passphrase"})
        self.assertFalse(r["dry_run"])
        self.assertEqual(len(r["ran"]), 1)
        files = [f for f in os.listdir(target) if f.endswith(".castle")]
        self.assertEqual(len(files), 1)
        self.assertTrue(os.path.exists(p + ".state"))
        mode = os.stat(p + ".state").st_mode & 0o777
        self.assertEqual(mode, 0o600)

    def test_second_run_not_due_writes_nothing(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        put(root, "sam", "receipts/r.txt", b"receipt")
        env = {"CASTLE_PW_SAM": "s3cret-passphrase"}
        p = mk_manifest(self, target, [job(target=target)])
        vault_jobs.run_due(root, p, execute=True, env=env)
        r = vault_jobs.run_due(root, p, execute=True, env=env)
        self.assertEqual(r["ran"], [])
        self.assertEqual(r["skipped"][0]["reason"], "not due")
        files = [f for f in os.listdir(target) if f.endswith(".castle")]
        self.assertEqual(len(files), 1)

    def test_missing_env_var_is_loud_refusal_not_silent_skip(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        put(root, "sam", "receipts/r.txt", b"receipt")
        p = mk_manifest(self, target, [job(target=target)])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.run_due(root, p, execute=True, env={})
        self.assertEqual(os.listdir(target), [])

    def test_executed_backup_verifies_bit_exact(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        put(root, "sam", "receipts/r.txt", b"receipt")
        env = {"CASTLE_PW_SAM": "s3cret-passphrase"}
        p = mk_manifest(self, target, [job(target=target)])
        r = vault_jobs.run_due(root, p, execute=True, env=env)
        pw = os.path.join(fresh_dir(self, "castle-jobs-test-pw-"), "pw")
        with open(pw, "w") as f:
            f.write("s3cret-passphrase")
        os.chmod(pw, 0o600)
        vr = vault_backup.verify_backup(r["ran"][0]["file"],
                                        passphrase_file=pw)
        self.assertGreater(vr["file_count"], 0)
        self.assertEqual(vr["backup_file"], r["ran"][0]["file"])


class PruneTest(unittest.TestCase):
    def _seed(self, tc, n):
        root = mk_root(tc)
        target = fresh_dir(tc, "castle-jobs-test-target-")
        put(root, "sam", "receipts/r.txt", b"receipt")
        env = {"CASTLE_PW_SAM": "s3cret-passphrase"}
        p = mk_manifest(tc, target,
                        [job(target=target,
                             retention={"keep_last": 2})])
        for i in range(n):
            vault_jobs.run_due(root, p, execute=True, env=env,
                               now=1_000_000 + i * 25 * 3600)
            state_path = p + ".state"
            if os.path.exists(state_path):
                os.unlink(state_path)  # force-due for seeding only
        return root, target, p

    def test_prune_plan_is_dry_run(self):
        root, target, p = self._seed(self, 3)
        plan = vault_jobs.prune_plan(root, p, "nightly-sam")
        self.assertTrue(plan["dry_run"])
        self.assertEqual(plan["kept"], 2)
        self.assertEqual(len(plan["would_delete"]), 1)
        self.assertEqual(len([f for f in os.listdir(target)
                              if f.endswith(".castle")]), 3)

    def test_prune_needs_typed_confirmation(self):
        root, target, p = self._seed(self, 3)
        r = vault_jobs.prune(root, p, "nightly-sam", confirm="wrong")
        self.assertTrue(r["dry_run"])
        self.assertEqual(len([f for f in os.listdir(target)
                              if f.endswith(".castle")]), 3)

    def test_prune_deletes_oldest_keeps_newest(self):
        root, target, p = self._seed(self, 3)
        before = sorted(f for f in os.listdir(target)
                        if f.endswith(".castle"))
        r = vault_jobs.prune(root, p, "nightly-sam",
                             confirm="nightly-sam")
        self.assertFalse(r["dry_run"])
        self.assertEqual(len(r["deleted"]), 1)
        after = sorted(f for f in os.listdir(target)
                       if f.endswith(".castle"))
        self.assertEqual(after, before[1:])  # oldest died, newest survived

    def test_prune_without_policy_is_refusal(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target, [job(target=target)])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.prune(root, p, "nightly-sam", confirm="nightly-sam")

    def test_prune_unknown_job_is_refusal(self):
        root = mk_root(self)
        target = fresh_dir(self, "castle-jobs-test-target-")
        p = mk_manifest(self, target,
                        [job(target=target,
                             retention={"keep_last": 2})])
        with self.assertRaises(vault_jobs.JobError):
            vault_jobs.prune(root, p, "nope", confirm="nope")


if __name__ == "__main__":
    unittest.main()
