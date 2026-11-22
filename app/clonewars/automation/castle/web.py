"""The browser a Castle script drives, through Playwright.

It is a real Chromium with a window, on the machine's own screen, so the screen view in
Clones shows what it does. Its profile (cookies, the site's own memory of this browser) is
kept in ~/.castle/browser, so a site sees the same browser come back day after day.

Selectors are the browser's own (CSS, or Playwright's text= and xpath=), plus four plain
forms: text "Search", label "Search", placeholder "Search Walmart" and title "Search".
"""

import os
import re
import shutil
import subprocess
import sys
import time
from urllib.parse import urljoin

from playwright.sync_api import TimeoutError as PageTimeout
from playwright.sync_api import sync_playwright

WAIT_MS = 30_000
# How long "fail if <element> is shown" looks before it decides the page is fine.
SHOWN_WAIT_MS = 2_500
TYPE_DELAY_MS = 45
# "pause whenever": how long a person has to answer what the page asks, and how often to look.
PAUSE_MAX_S = 10 * 60
PAUSE_LOOK_S = 2
# While waiting for something to show, the guards are looked at this often.
LOOK_MS = 1_000
SCROLL_PAUSE = 0.6
SCROLL_MAX = 60
# CASTLE_BROWSER_PROFILE and CASTLE_HEADLESS=1 are for trying a script outside a machine.
PROFILE = os.environ.get("CASTLE_BROWSER_PROFILE") or os.path.expanduser("~/.castle/browser")
HEADLESS_AGENT = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36"
_FRIENDLY = re.compile(r'^(text|label|placeholder|title)\s+"(.*)"$', re.IGNORECASE | re.DOTALL)


def locate(root, selector):
    """A locator for a selector inside a page or an item of a list."""
    m = _FRIENDLY.match(selector.strip())
    if m:
        kind, value = m.group(1).lower(), m.group(2)
        return {
            "text": root.get_by_text,
            "label": root.get_by_label,
            "placeholder": root.get_by_placeholder,
            "title": root.get_by_title,
        }[kind](value)
    return root.locator(selector)


def _system_chromium():
    for name in ("chromium", "chromium-browser", "google-chrome", "google-chrome-stable"):
        path = shutil.which(name)
        if path:
            return path
    return None


def _in_container():
    return os.path.exists("/.dockerenv") or os.environ.get("CASTLE_CONTAINER") == "1"


class Browser:
    def __init__(self, run):
        self.run = run
        self.pw = sync_playwright().start()
        args = ["--no-first-run", "--no-default-browser-check", "--start-maximized", "--disable-blink-features=AutomationControlled"]
        if _in_container():
            # A container has no user namespaces for Chromium's own sandbox; Docker's is the sandbox.
            args.append("--no-sandbox")
        os.makedirs(PROFILE, exist_ok=True)
        options = dict(headless=False, args=args, no_viewport=True, locale="en-US")
        if os.environ.get("CASTLE_HEADLESS") == "1":
            # No window: a desktop-sized page, and a browser that does not call itself headless.
            options.update(headless=True, no_viewport=False, viewport={"width": 1440, "height": 900}, user_agent=HEADLESS_AGENT)
        exe = _system_chromium()
        try:
            self.context = self.pw.chromium.launch_persistent_context(PROFILE, executable_path=exe, **options)
        except Exception as err:  # noqa: BLE001 - fall back to Playwright's own Chromium
            if exe is None:
                raise
            run.say(f"the system Chromium did not start ({str(err).splitlines()[0][:120]}); getting Playwright's own")
            subprocess.run([sys.executable, "-m", "playwright", "install", "chromium"], check=True)
            self.context = self.pw.chromium.launch_persistent_context(PROFILE, **options)
        self.context.set_default_timeout(WAIT_MS)
        self.page = self.context.pages[0] if self.context.pages else self.context.new_page()
        # (selector, message, pause): pages the script watches for from the step that named them.
        self.guards = []

    def go(self, url):
        self.page.goto(url, wait_until="domcontentloaded")
        self._settle()

    def _settle(self):
        try:
            self.page.wait_for_load_state("load", timeout=15_000)
        except Exception:  # noqa: BLE001 - a page that never says "loaded" can still be used
            pass

    def guard(self, selector, message, pause):
        """From now on, before every step and while waiting: when the page shows this, the run
        pauses for a person to deal with it (pause), or ends (not pause)."""
        self.guards.append((selector, message, pause))
        self.check_guards()

    def _shown(self, selector):
        try:
            return locate(self.page, selector).first.is_visible()
        except Exception:  # noqa: BLE001 - a page in the middle of changing shows nothing yet
            return False

    def check_guards(self):
        """True when the run paused for a person and the page is clear again."""
        from . import StepFailed

        for selector, message, pause in self.guards:
            if not self._shown(selector):
                continue
            if not pause:
                raise StepFailed(message)
            self.run.waiting(message)
            deadline = time.time() + PAUSE_MAX_S
            while time.time() < deadline:
                time.sleep(PAUSE_LOOK_S)
                if not self._shown(selector):
                    self.run.say("the page is clear again; going on")
                    self._settle()
                    return True
            raise StepFailed(f"{message} Nobody answered it within {PAUSE_MAX_S // 60} minutes.")
        return False

    def _one(self, selector):
        """The first match, once it shows. The guards are looked at while waiting, and a pause
        for a person starts the wait again."""
        loc = locate(self.page, selector).first
        deadline = time.time() + WAIT_MS / 1000
        while True:
            if self.check_guards():
                deadline = time.time() + WAIT_MS / 1000
            try:
                loc.wait_for(state="visible", timeout=LOOK_MS if self.guards else WAIT_MS)
                return loc
            except PageTimeout:
                if not self.guards or time.time() >= deadline:
                    raise

    def type(self, selector, text):
        box = self._one(selector)
        box.click()
        box.fill("")
        box.press_sequentially(text, delay=TYPE_DELAY_MS)

    def click(self, selector):
        self._one(selector).click()
        self._settle()

    def press(self, key, selector=None):
        if selector:
            self._one(selector).press(key)
        else:
            self.page.keyboard.press(key)
        self._settle()

    def wait_for(self, selector):
        self._one(selector)

    def fail_if_shown(self, selector, message):
        """Ends the run when the page shows something the script cannot go on from."""
        from . import StepFailed

        loc = locate(self.page, selector).first
        try:
            loc.wait_for(state="visible", timeout=SHOWN_WAIT_MS)
        except Exception:  # noqa: BLE001 - not shown: the run goes on
            return
        raise StepFailed(message)

    def scroll_down(self):
        self.page.mouse.wheel(0, 900)
        time.sleep(SCROLL_PAUSE)

    def scroll_to_bottom(self):
        """Scrolls until the page stops growing, so listings that load on scroll are there."""
        still = 0
        last = -1
        for _ in range(SCROLL_MAX):
            self.page.mouse.wheel(0, 1200)
            time.sleep(SCROLL_PAUSE)
            height = self.page.evaluate("() => document.documentElement.scrollHeight")
            at_end = self.page.evaluate("() => window.scrollY + window.innerHeight >= document.documentElement.scrollHeight - 4")
            still = still + 1 if (height == last and at_end) else 0
            last = height
            if still >= 3:
                break

    def read_text(self, selector):
        return " ".join(self._one(selector).inner_text().split())

    def read_list(self, selector, fields):
        """One row per item: each field is the text (or attribute) of the first match inside it."""
        rows = []
        for item in locate(self.page, selector).all():
            row = {}
            for name, sel, attr in fields:
                target = locate(item, sel).first if sel else item
                if sel and target.count() == 0:
                    row[name] = None
                    continue
                if attr:
                    value = target.get_attribute(attr)
                    if value and attr in ("href", "src"):
                        value = urljoin(self.page.url, value)
                else:
                    # The text as shown; text kept for screen readers only is hidden, so then
                    # the text as written.
                    value = " ".join(target.inner_text().split()) or " ".join((target.text_content() or "").split())
                row[name] = value if value != "" else None
            if any(v is not None for v in row.values()):
                rows.append(row)
        return rows

    def screenshot(self):
        return self.page.screenshot(type="png")

    def close(self):
        try:
            self.context.close()
        finally:
            self.pw.stop()
