"""Castle's automation library: what a compiled Castle script calls, inside the machine.

A script (automation.castle) becomes main.py, which starts with ``run = castle.start()`` and
does every step inside ``with run.step(line, said):``. The run reports to the Castle box as it
goes: which line it is on, what it says, the rows it posts, the values it tracks, pictures of
the screen, and how it ended. The box keeps all of it in its database and shows it in Clones.

Only the standard library is needed here. The browser (Playwright) is loaded by ``website``,
so a script that never opens a website runs on a bare Python.
"""

import base64
import json
import os
import re
import sys
import time
import traceback
import urllib.error
import urllib.request
from contextlib import contextmanager

from .data import average, divide, drop, highest, keep, lowest, max, measure, median, min, multiply, number, sort, sum, unique  # noqa: F401

RUN_FILE = "castle-run.json"
REPORT_PATH = "/api/v1/clones/report"
REPORT_TIMEOUT = 10
REPORT_TRIES = 3
MAX_ROWS = 2000
MAX_LOG_LINE = 2000
SPOOL = os.path.expanduser("~/castle-results.ndjson")


def _now():
    return int(time.time() * 1000)


def _clean(value):
    """A value as JSON can carry it: numbers stay numbers, the rest becomes text."""
    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, float):
        return value if value == value and abs(value) != float("inf") else None
    if isinstance(value, dict):
        return {str(k): _clean(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [_clean(v) for v in value]
    return str(value)


def show(value):
    """A value as text in a sentence: 6.391 -> 6.39, 40.0 -> 40, a row -> its first fields."""
    if value is None:
        return "nothing"
    if isinstance(value, bool):
        return "yes" if value else "no"
    if isinstance(value, float):
        return str(int(value)) if value.is_integer() else f"{value:,.2f}"
    if isinstance(value, int):
        return f"{value:,}"
    if isinstance(value, dict):
        return ", ".join(f"{k} {show(v)}" for k, v in list(value.items())[:3])
    if isinstance(value, list):
        return f"{len(value)} rows"
    return str(value)


class StepFailed(Exception):
    """A step that cannot go on, with a message for the person reading the results."""


class Run:
    """One run of an automation: its values, its browser, and its line to the box."""

    def __init__(self, info):
        self.info = info
        self.values = {}
        self.web = None
        self.line = 0
        self.said = ""
        self.finished = False
        self.base = None
        self.token = info.get("token", "")
        self.shots = 0
        self.last_said = ""

    # ---- talking to the box ----

    def _send(self, body):
        """Sends one report. When the box cannot be reached it is kept in a file, so nothing
        the run found is lost, and the run goes on."""
        body = dict(body, at=_now())
        data = json.dumps(_clean(body)).encode("utf-8")
        for base in ([self.base] if self.base else self.info.get("urls", [])):
            for attempt in range(REPORT_TRIES):
                req = urllib.request.Request(base + REPORT_PATH, data=data, method="POST", headers={
                    "Content-Type": "application/json",
                    "Authorization": "Run " + self.token,
                })
                try:
                    with urllib.request.urlopen(req, timeout=REPORT_TIMEOUT) as res:
                        res.read()
                    self.base = base
                    return True
                except urllib.error.HTTPError as err:
                    if err.code < 500:
                        print(f"[castle] the box refused a report ({err.code}): {err.read()[:200].decode('utf-8', 'replace')}", flush=True)
                        return False
                except (urllib.error.URLError, OSError):
                    if self.base is None:
                        break  # try the next address
                time.sleep(1 + attempt)
        try:
            with open(SPOOL, "a", encoding="utf-8") as f:
                f.write(data.decode("utf-8") + "\n")
        except OSError:
            pass
        return False

    def say(self, text):
        text = str(text)[:MAX_LOG_LINE]
        self.last_said = text
        print(f"[castle] {text}", flush=True)
        self._send({"kind": "log", "line": self.line, "text": text})

    def waiting(self, message):
        """The run waits for a person: Clones shows it on the run's line."""
        print(f"[castle] waiting for you: {message}", flush=True)
        self._send({"kind": "step", "line": self.line, "said": f"waiting for you: {message}"[:400]})
        self._send({"kind": "log", "line": self.line, "text": f"waiting for you: {message}"[:MAX_LOG_LINE]})

    # ---- steps ----

    @contextmanager
    def step(self, line, said):
        """One line of the script. A failure in it ends the run with that line named."""
        self.line, self.said = line, said
        print(f"[castle] line {line}: {said}", flush=True)
        self._send({"kind": "step", "line": line, "said": said})
        try:
            if self.web is not None:
                self.web.check_guards()
            yield
        except SystemExit:
            raise
        except BaseException as err:  # noqa: BLE001 - every failure is reported, then the run ends
            message = str(err) if isinstance(err, StepFailed) else _explain(err)
            print(f"[castle] line {line} failed: {message}", flush=True)
            traceback.print_exc()
            self._fail_picture()
            self._send({"kind": "fail", "line": line, "said": said, "message": message})
            self.finished = True
            self._close()
            raise SystemExit(1)

    def _fail_picture(self):
        if self.web is not None:
            try:
                self.screenshot(f"line {self.line} failed")
            except Exception:  # noqa: BLE001 - the failure itself matters more than its picture
                pass

    def text(self, template):
        """Fills in {name}: a value of the script, or a sign-in field ({API_KEY})."""
        def fill(m):
            key = " ".join(m.group(1).split()).lower()
            if key in self.values:
                value = self.values[key]
                if isinstance(value, (int, float)) and not isinstance(value, bool) and re.search(r"price|cost", key):
                    return f"${value:,.2f}"
                return show(value)
            raw = m.group(1).strip()
            if raw in os.environ:
                return os.environ[raw]
            raise StepFailed(f"{{{raw}}} has no value: give the bot a {raw} in its sign-in fields")
        return re.sub(r"\{([^{}]+)\}", fill, template)

    def website(self, url):
        from .web import Browser  # Playwright is loaded only by scripts that use a website
        self.web = Browser(self)
        self.web.go(url)

    def wait(self, seconds):
        time.sleep(float(seconds))

    def post(self, dataset, value):
        if isinstance(value, list):
            rows = [r if isinstance(r, dict) else {"value": r} for r in value]
        elif isinstance(value, dict):
            rows = [value]
        else:
            rows = [{"value": value}]
        if len(rows) > MAX_ROWS:
            self.say(f"{dataset}: posting the first {MAX_ROWS} of {len(rows)} rows")
            rows = rows[:MAX_ROWS]
        self._send({"kind": "rows", "dataset": dataset, "rows": rows})
        print(f"[castle] posted {len(rows)} row(s) to {dataset}", flush=True)

    def track(self, name, value, better=""):
        n = number(value)
        if n is None:
            self.say(f"{name} has no number to track this time")
            return
        self._send({"kind": "metric", "name": name, "value": n, "better": better})

    def screenshot(self, label=""):
        if self.web is None:
            raise StepFailed("there is no browser to take a picture of yet")
        png = self.web.screenshot()
        self.shots += 1
        self._send({"kind": "shot", "label": label or f"picture {self.shots}", "png": base64.b64encode(png).decode("ascii")})

    def fail_if_empty(self, value, message):
        if value is None or value == "" or (isinstance(value, (list, dict)) and not value):
            raise StepFailed(message)

    def _close(self):
        if self.web is not None:
            try:
                self.web.close()
            except Exception:  # noqa: BLE001
                pass
            self.web = None

    def finish(self):
        """The script reached its end: the box is told, and stops the machine unless the
        automation runs again and again."""
        if self.finished:
            return
        self.finished = True
        self._close()
        self._send({"kind": "finish", "said": self.last_said})
        print("[castle] finished", flush=True)


def _explain(err):
    """A failure in words: the browser's long messages are cut to their first line."""
    text = str(err).strip().splitlines()[0] if str(err).strip() else type(err).__name__
    if "Timeout" in type(err).__name__ or "Timeout" in text:
        return "the page did not show it in time: " + text
    return text[:500]


def start(run_file=RUN_FILE):
    """The run this machine was started for, from the file Castle put next to main.py."""
    here = os.path.dirname(os.path.abspath(sys.argv[0] if sys.argv and sys.argv[0] else __file__))
    path = run_file if os.path.isabs(run_file) else os.path.join(here, run_file)
    try:
        with open(path, encoding="utf-8") as f:
            info = json.load(f)
    except (OSError, ValueError):
        print(f"[castle] {run_file} is missing: this script is meant to be started by Castle. Running without reporting.", flush=True)
        info = {"urls": [], "token": ""}
    run = Run(info)
    run._send({"kind": "hello"})
    return run


def setup_failed(message, run_file=RUN_FILE):
    """For run.sh: the machine could not get ready, so the script never started."""
    run = start(run_file)
    run.finished = True
    run._send({"kind": "fail", "line": 0, "said": "getting the machine ready", "message": message})
