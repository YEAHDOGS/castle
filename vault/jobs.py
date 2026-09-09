#!/usr/bin/env python3
"""Castle scheduled backup jobs — FAMILY-DATA-VAULT.md step 8 (scheduled lane).

The step-8 backup primitive is one-shot; a family data home needs
UNATTENDED backups: nightly sealed backups of every vault to the
backup target (Castle's 10TB drive, once mounted). This module is the
scheduler between the cron entry and the backup primitive:

  manifest (JSON, data only) -> validator -> due-check -> runner -> retention

Manifest shape (``backup-jobs.json``):

    {"version": 1, "jobs": [
      {"name": "nightly-mom",
       "user": "mom",
       "target_dir": "/mnt/castle-backups",
       "passphrase_env": "CASTLE_PASSPHRASE_MOM",
       "schedule": {"kind": "interval", "hours": 24},
       "retention": {"keep_last": 14, "max_age_days": 60},
       "enabled": true, "chunks": false}
    ]}

Secrets are the hard rule: the manifest carries env var NAMES only,
never values. The runner reads the passphrase from the environment at
run time, materializes it into a 0600 temp file for the backup call,
then crypto-shreds the temp file. A manifest that embeds a passphrase,
token, or key value is REFUSED at validate time — silently accepting
would teach the operator to commit secrets to git.

``keyfile_env`` is the twin: the env var names a PATH to a 0600
32-byte keyfile (the secret itself is never in the environment or the
manifest — same DNA as secretstore: values live in files, names live
in config).

Trust model (same DNA as the vault core and the flamethrower):
  - Dry-run is the default everywhere: ``run_due(execute=False)``
    enumerates which jobs are due and what they'd do, writing nothing.
    Real runs need the explicit ``execute=True`` flag (the CLI gate is
    ``--execute``). Backups only WRITE new timestamped files — they
    never delete, never overwrite — so an unattended runner that only
    ever appends can't destroy the family archive.
  - Retention pruning is the only destructive half: deleting a backup
    needs TYPED confirmation (the job name), and prune refuses when the
    job has no retention policy — "no policy" never means "delete
    everything".
  - Refusals, never guesses: unknown user, duplicate job names, bad
    env var names, embedded secret values, invalid schedules, insane
    retention (keep_last < 1), a target that is/contains/is-inside the
    vault, a missing secret env var (loud failure, never a silent
    skipped backup — a silently-skipped backup is how families lose
    data), a symlink as target dir.
  - State file (``<jobs>.state``, 0600) tracks last_run per job —
    metadata only, never secrets. The activity log gets one record per
    executed backup (metadata only).

Schedule kinds: {"kind": "interval", "hours": N} (N >= 1) or
{"kind": "daily", "at": "HH:MM"} (UTC wall-clock). First-ever run of a
job is always due — a job that has never run has never backed up.

Retention policy: delete a backup if it is older than ``max_age_days``
OR beyond the newest ``keep_last``. The newest ``keep_last`` backups
always survive; anything ancient dies even if the count is low. Deletion is the
flamethrower overwrite+unlink primitive (these are encrypted blobs,
but a kill is a kill).

Stdlib only. No network, no new hosts, no installs.
"""

import calendar
import json
import os
import re
import stat
import sys
import time

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault as vault_core  # noqa: E402
import backup as vault_backup  # noqa: E402

sys.path.insert(0, os.path.join(_HERE, "..", "flamethrower"))
import keyring  # noqa: E402

JOB_FORMAT_VERSION = 1
ENV_NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
DAILY_AT_RE = re.compile(r"^([01][0-9]|2[0-3]):([0-5][0-9])$")
# any key that looks like it carries a secret VALUE, not a *_env name
SECRETISH_RE = re.compile(r"(passphrase|secret|token|password|private.?key)",
                          re.IGNORECASE)


class JobError(Exception):
    """Anything that makes a backup job untrustworthy is a hard refusal."""


# ---------------------------------------------------------------------------
# manifest loading + validation
# ---------------------------------------------------------------------------

def _valid_env_name(name):
    return isinstance(name, str) and ENV_NAME_RE.match(name) is not None


def _vault_dir(root, user):
    return os.path.realpath(os.path.join(root, "vaults", user))


def _check_target_sane(target, vdir):
    """Structural target checks the validator can do without the backup
    primitive. The full check (per-run, via backup._check_target) still
    applies at execution time — this is the early-warning layer."""
    if not isinstance(target, str) or not target:
        raise JobError("job target_dir must be a non-empty string")
    if os.path.islink(target):
        raise JobError("job target_dir is a symlink — refusing: %s"
                       % target)
    t = os.path.realpath(target)
    if t == os.sep:
        raise JobError("refusing: job target_dir is the filesystem root")
    if t == vdir:
        raise JobError("job target_dir IS the vault dir — refusing: %s"
                       % target)
    if t.startswith(vdir + os.sep):
        raise JobError("job target_dir lives inside the vault being "
                       "backed up — refusing: %s" % target)
    if vdir.startswith(t + os.sep):
        raise JobError("job target_dir contains the vault being backed "
                       "up — refusing: %s" % target)


def _validate_schedule(sched):
    if not isinstance(sched, dict):
        raise JobError("job schedule must be an object")
    kind = sched.get("kind")
    if kind == "interval":
        hours = sched.get("hours")
        if not isinstance(hours, (int, float)) or hours < 1:
            raise JobError("interval schedule needs hours >= 1, got %r"
                           % hours)
        if hours > 24 * 365:
            raise JobError("interval schedule absurdly long: %r hours"
                           % hours)
        return {"kind": "interval", "hours": hours}
    if kind == "daily":
        at = sched.get("at")
        if not isinstance(at, str) or not DAILY_AT_RE.match(at):
            raise JobError("daily schedule needs at=HH:MM (00:00-23:59), "
                           "got %r" % at)
        return {"kind": "daily", "at": at}
    raise JobError("unknown schedule kind: %r (want 'interval' or 'daily')"
                   % kind)


def _validate_retention(ret):
    if ret is None:
        return None
    if not isinstance(ret, dict):
        raise JobError("job retention must be an object")
    keep = ret.get("keep_last")
    age = ret.get("max_age_days")
    if keep is None and age is None:
        raise JobError("job retention sets neither keep_last nor "
                       "max_age_days — an empty retention policy is a lie")
    if keep is not None and \
            (not isinstance(keep, int) or isinstance(keep, bool)
             or keep < 1):
        raise JobError("retention keep_last must be an integer >= 1, "
                       "got %r" % keep)
    if age is not None and \
            (not isinstance(age, (int, float)) or isinstance(age, bool)
             or age < 1):
        raise JobError("retention max_age_days must be >= 1, got %r" % age)
    return {"keep_last": keep, "max_age_days": age}


def _validate_job(root, raw):
    """Validate one raw job dict into its canonical form. Refusals are
    JobError; every refusal is a specific, loud sentence."""
    if not isinstance(raw, dict):
        raise JobError("job must be an object, got %r" % type(raw).__name__)
    name = raw.get("name")
    if not isinstance(name, str) or not keyring._valid_name(name):
        raise JobError("bad job name: %r (alnum plus -_. only, max 64 "
                       "chars)" % name)

    # the anti-secret guard: no key may LOOK like it carries a secret
    # value unless it's the *_env name of it. This runs before anything
    # else touches the values, so a "passphrase": "hunter2" manifest
    # dies at the door and never becomes a habit.
    for key in raw:
        if key in ("passphrase_env", "keyfile_env"):
            continue
        if SECRETISH_RE.search(str(key)):
            raise JobError("job %r: field %r looks like an embedded "
                           "secret VALUE — refusing. Secrets live in the "
                           "environment (passphrase_env/keyfile_env), "
                           "never in the manifest." % (name, key))

    # exactly one secret source, named not valued
    sources = [k for k in ("passphrase_env", "keyfile_env") if raw.get(k)]
    if len(sources) != 1:
        raise JobError("job %r: need exactly one of passphrase_env / "
                       "keyfile_env, got %d" % (name, len(sources)))
    src = sources[0]
    if not _valid_env_name(raw[src]):
        raise JobError("job %r: %s is not a valid env var name: %r"
                       % (name, src, raw[src]))

    user = raw.get("user")
    vault_core._ensure_dirs(root)
    users = vault_core._load_users(root)
    try:
        vault_core._require_user(users, user)
    except Exception as e:
        raise JobError("job %r: %s" % (name, e))

    target = raw.get("target_dir")
    _check_target_sane(target, _vault_dir(root, user))
    target_real = os.path.realpath(target)

    schedule = _validate_schedule(raw.get("schedule"))
    retention = _validate_retention(raw.get("retention"))
    enabled = raw.get("enabled", True)
    if not isinstance(enabled, bool):
        raise JobError("job %r: enabled must be true/false, got %r"
                       % (name, enabled))
    chunks = raw.get("chunks", False)
    if not isinstance(chunks, bool):
        raise JobError("job %r: chunks must be true/false, got %r"
                       % (name, chunks))

    return {"name": name, "user": user, "target_dir": target_real,
            "secret_kind": ("passphrase" if src == "passphrase_env"
                            else "keyfile"),
            "secret_env": raw[src], "schedule": schedule,
            "retention": retention, "enabled": enabled, "chunks": chunks}


def load_jobs(root, jobs_file):
    """Load + validate a manifest. Returns the canonical job list.
    Unknown top-level version, non-list jobs, or duplicate names are
    refusals — a manifest that can't be read exactly is a rumor."""
    if os.path.islink(jobs_file):
        raise JobError("jobs file is a symlink — refusing: %s" % jobs_file)
    try:
        with open(jobs_file, encoding="utf-8") as f:
            doc = json.load(f)
    except FileNotFoundError:
        raise JobError("jobs file not found: %s" % jobs_file)
    except ValueError as e:
        raise JobError("jobs file is not valid JSON: %s" % e)
    if not isinstance(doc, dict) or doc.get("version") != JOB_FORMAT_VERSION:
        raise JobError("jobs file must be a JSON object with version=%d"
                       % JOB_FORMAT_VERSION)
    raw_jobs = doc.get("jobs")
    if not isinstance(raw_jobs, list) or not raw_jobs:
        raise JobError("jobs file needs a non-empty 'jobs' list")
    jobs = [_validate_job(root, r) for r in raw_jobs]
    names = [j["name"] for j in jobs]
    if len(set(names)) != len(names):
        raise JobError("duplicate job names in manifest: %s"
                       % sorted(n for n in names if names.count(n) > 1))
    return jobs


# ---------------------------------------------------------------------------
# due-ness (UTC everywhere — wall-clock games need timezones, UTC doesn't)
# ---------------------------------------------------------------------------

def _daily_epoch_utc(at, now):
    """Epoch of today's HH:MM in UTC at-or-before ``now``."""
    hh, mm = int(at[:2]), int(at[3:])
    t = time.gmtime(now)
    return calendar.timegm((t.tm_year, t.tm_mon, t.tm_mday, hh, mm, 0,
                            0, 0, 0))


def is_due(job, last_run, now=None):
    """True when the job should run. A job that has never run (last_run
    None) is always due — no backup has ever happened."""
    now = time.time() if now is None else now
    sched = job["schedule"]
    if last_run is None:
        return True
    if sched["kind"] == "interval":
        return (now - last_run) >= sched["hours"] * 3600
    scheduled = _daily_epoch_utc(sched["at"], now)
    return now >= scheduled and last_run < scheduled


# ---------------------------------------------------------------------------
# state (metadata only — never secrets)
# ---------------------------------------------------------------------------

def _state_path(jobs_file):
    return jobs_file + ".state"


def _load_state(jobs_file):
    path = _state_path(jobs_file)
    if not os.path.exists(path):
        return {}
    try:
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
    except (ValueError, OSError):
        raise JobError("jobs state file is corrupt — refusing to guess: "
                       "%s" % path)
    if not isinstance(doc, dict):
        raise JobError("jobs state file is corrupt — refusing: %s" % path)
    return doc


def _save_state(jobs_file, state):
    path = _state_path(jobs_file)
    tmp = path + ".tmp.%d" % os.getpid()
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(state, f, indent=2, sort_keys=True)
        f.write("\n")
        f.flush()
        os.fsync(f.fileno())
    os.chmod(tmp, 0o600)
    os.replace(tmp, path)


# ---------------------------------------------------------------------------
# secret resolution — env var NAME from the manifest, value at run time
# ---------------------------------------------------------------------------

def _resolve_secret(job):
    """Return (passphrase_file, keyfile) for the job's secret source.

    passphrase_env: the env var's VALUE is the passphrase — it is
    materialized into a 0600 temp file the caller must shred after use.
    keyfile_env: the env var's value is the PATH to a 0600 keyfile —
    the path is validated, the file itself is never read here.
    A missing/empty env var is a loud refusal, never a silent skip."""
    env_name = job["secret_env"]
    value = os.environ.get(env_name)
    if not value:
        raise JobError("job %r: env var %s is unset or empty — refusing "
                       "to run a backup whose secret can't be resolved "
                       "(a silently-skipped backup is how families lose "
                       "data)" % (job["name"], env_name))
    if job["secret_kind"] == "keyfile":
        path = value
        if os.path.islink(path) or not os.path.isfile(path):
            raise JobError("job %r: keyfile_env %s points at %r — not "
                           "a real file" % (job["name"], env_name, path))
        mode = stat.S_IMODE(os.stat(path).st_mode)
        if mode != 0o600:
            raise JobError("job %r: keyfile %s is mode %04o, must be "
                           "exactly 0600" % (job["name"], path, mode))
        if os.path.getsize(path) != 32:
            raise JobError("job %r: keyfile %s is not 32 bytes"
                           % (job["name"], path))
        return None, path
    # passphrase: value lives in the env, so hand the backup primitive
    # the 0600 file it demands. The caller owns the shred.
    import tempfile
    fd, tmp = tempfile.mkstemp(prefix="castle-job-pw-")
    try:
        os.write(fd, value.encode("utf-8"))
        os.close(fd)
        fd = None
        os.chmod(tmp, 0o600)
    finally:
        if fd is not None:
            os.close(fd)
    return tmp, None


# ---------------------------------------------------------------------------
# runner
# ---------------------------------------------------------------------------

def run_due(root, jobs_file, execute=False, now=None, env=None):
    """Run every enabled due job. Dry-run is the default: plans are
    returned and NOTHING is written (not even state). ``execute=True``
    performs the backups and records last_run in the state file.

    ``env`` is an optional dict of env vars for tests; real runs use
    os.environ. Returns {"dry_run": ..., "ran": [...], "skipped": [...],
    "plans": [...]} — plans carry the backup primitive's dry-run plan
    per due job."""
    now = time.time() if now is None else now
    jobs = load_jobs(root, jobs_file)
    state = _load_state(jobs_file)
    result = {"dry_run": not execute, "ran": [], "skipped": [], "plans": []}
    old_env = None
    if env is not None:
        old_env = dict(os.environ)
        os.environ.clear()
        os.environ.update(env)
    try:
        for job in jobs:
            if not job["enabled"]:
                result["skipped"].append(
                    {"name": job["name"], "reason": "disabled"})
                continue
            last = state.get(job["name"], {}).get("last_run")
            if not is_due(job, last, now):
                result["skipped"].append(
                    {"name": job["name"], "reason": "not due",
                     "last_run": last})
                continue
            pw_file, keyfile = _resolve_secret(job)
            try:
                plan = vault_backup.plan_backup(
                    root, job["user"], job["target_dir"],
                    passphrase_file=pw_file, keyfile=keyfile)
            except vault_backup.BackupError as e:
                raise JobError("job %r: plan refused: %s"
                               % (job["name"], e))
            finally:
                if pw_file and os.path.exists(pw_file):
                    keyring._overwrite_and_unlink(pw_file)
            entry = {"name": job["name"], "user": job["user"],
                     "target_dir": job["target_dir"],
                     "file_count": plan["file_count"],
                     "total_bytes": plan["total_bytes"],
                     "would_write": plan["would_write"]}
            result["plans"].append(entry)
            if not execute:
                continue
            # real run: resolve the secret again (fresh temp file — the
            # plan pass shredded its own), back up with the user's own
            # typed-confirmation equivalent (the --execute flag plus the
            # explicit manifest is the operator's signature), then
            # verify and record.
            pw_file, keyfile = _resolve_secret(job)
            try:
                r = vault_backup.create_backup(
                    root, job["user"], job["target_dir"],
                    passphrase_file=pw_file, keyfile=keyfile,
                    confirm=job["user"], chunks=job["chunks"])
            except vault_backup.BackupError as e:
                raise JobError("job %r: backup failed: %s"
                               % (job["name"], e))
            finally:
                if pw_file and os.path.exists(pw_file):
                    keyring._overwrite_and_unlink(pw_file)
            cert = r["certificate"]
            state[job["name"]] = {"last_run": now,
                                  "last_file": r["backup_file"],
                                  "last_cert": cert["cert_id"]}
            _save_state(jobs_file, state)
            vault_core._activity(root, job["user"], "backup.job-ran",
                                 "job=%s user=%s file=%s cert=%s" % (
                                     job["name"], job["user"],
                                     os.path.basename(r["backup_file"]),
                                     cert["cert_id"]))
            result["ran"].append({"name": job["name"],
                                 "file": r["backup_file"],
                                 "cert": cert["cert_id"]})
    finally:
        if old_env is not None:
            os.environ.clear()
            os.environ.update(old_env)
    return result


# ---------------------------------------------------------------------------
# retention pruning
# ---------------------------------------------------------------------------

def prune_plan(root, jobs_file, job_name):
    """What a retention prune would delete. Dry-run by nature — nothing
    is ever written. Refuses when the job has no retention policy
    (no policy never means 'delete everything') or the job is unknown."""
    jobs = {j["name"]: j for j in load_jobs(root, jobs_file)}
    job = jobs.get(job_name)
    if job is None:
        raise JobError("unknown job: %s" % job_name)
    if not job["retention"]:
        raise JobError("job %r has no retention policy — refusing to "
                       "delete anything (set keep_last or max_age_days "
                       "explicitly)" % job_name)
    try:
        entries = vault_backup.list_backups(job["target_dir"])
    except vault_backup.BackupError as e:
        raise JobError("job %r: cannot inventory target: %s"
                       % (job_name, e))
    keep_last = job["retention"]["keep_last"]
    max_age = job["retention"]["max_age_days"]
    mine = []
    for e in entries:
        if e.get("user") != job["user"] or not e.get("sealed_at"):
            continue
        try:
            sealed = calendar.timegm(
                time.strptime(e["sealed_at"], "%Y-%m-%dT%H:%M:%SZ"))
        except (ValueError, TypeError):
            continue  # unverifiable age: never delete what we can't date
        mine.append((sealed, e))
    mine.sort()  # oldest first
    now = time.time()
    victims = []
    for i, (sealed, e) in enumerate(mine):
        age_days = (now - sealed) / 86400.0
        too_old = max_age is not None and age_days > max_age
        beyond_keep = keep_last is not None and \
            i < len(mine) - keep_last
        if too_old or beyond_keep:
            victims.append({"file": e["file"], "sealed_at": e["sealed_at"],
                            "age_days": round(age_days, 1),
                            "reason": ("older than %s days"
                                       % max_age if too_old else
                                       "beyond keep_last=%s" % keep_last)})
    return {"dry_run": True, "job": job_name,
            "kept": len(mine) - len(victims),
            "would_delete": victims,
            "hint": ("re-run with --yes %s to prune" % job_name)}


def prune(root, jobs_file, job_name, confirm=None):
    """Enforce a job's retention policy. Dry-run by default; the real
    prune needs the job NAME typed back. Returns the dry-run plan or
    the kill report."""
    plan = prune_plan(root, jobs_file, job_name)
    if confirm != job_name:
        return plan
    if not plan["would_delete"]:
        return {"dry_run": False, "job": job_name, "deleted": [],
                "note": "nothing to prune"}
    jobs = {j["name"]: j for j in load_jobs(root, jobs_file)}
    target = jobs[job_name]["target_dir"]
    deleted = []
    for v in plan["would_delete"]:
        path = os.path.realpath(os.path.join(target, v["file"]))
        t = os.path.realpath(target)
        if path == t or not path.startswith(t + os.sep):
            raise JobError("prune target escaped the backup dir — "
                           "aborting: %s" % v["file"])
        if os.path.islink(path) or not os.path.isfile(path):
            raise JobError("prune victim not a regular file — aborting: "
                           "%s" % v["file"])
        keyring._overwrite_and_unlink(path)
        deleted.append(v["file"])
    vault_core._activity(root, jobs[job_name]["user"], "backup.job-pruned",
                         "job=%s deleted=%d files=%s" % (
                             job_name, len(deleted), ",".join(deleted)))
    return {"dry_run": False, "job": job_name, "deleted": deleted}
