#!/usr/bin/env bash
# ==============================================================================
# Castle: get Python ready for the castle library, then run the automation.
# ==============================================================================
# Castle starts this in the machine's desktop session, from the project it put
# there: main.py (the compiled script), automation.castle, castle-run.json and
# this castle/ folder. The Python environment lives in ~/.castle/venv and is
# kept on the machine's disk, so only the first run installs anything.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
VENV="$HOME/.castle/venv"
WEB=$(python3 -c 'import json; print("yes" if json.load(open("castle-run.json")).get("web") else "no")' 2>/dev/null || echo no)

failed() {
  echo "[castle] $1"
  python3 -c 'import sys, castle; castle.setup_failed(sys.argv[1])' "$1" || true
  exit 1
}

command -v python3 >/dev/null || { echo "[castle] this machine has no python3"; exit 1; }
if [ ! -x "$VENV/bin/python" ]; then
  echo "[castle] making the Python environment (once per machine)"
  python3 -m venv "$VENV" || failed "python3 could not make an environment: is python3-venv installed?"
fi
if [ "$WEB" = yes ] && ! "$VENV/bin/python" -c 'import playwright' 2>/dev/null; then
  echo "[castle] installing Playwright, which drives the browser (once per machine)"
  "$VENV/bin/pip" install --disable-pip-version-check --quiet playwright || failed "Playwright could not be installed: is the machine online?"
fi
if [ "$WEB" = yes ] && ! command -v chromium >/dev/null && ! command -v chromium-browser >/dev/null && [ ! -d "$HOME/.cache/ms-playwright" ]; then
  echo "[castle] no Chromium on this system; getting Playwright's own"
  "$VENV/bin/python" -m playwright install chromium || failed "no browser: Chromium is not installed and Playwright could not fetch one"
fi

echo "[castle] running the script"
"$VENV/bin/python" main.py
code=$?
# A script that ended without saying how (a Python error outside any step) is a failure.
[ "$code" -eq 0 ] || python3 -c 'import castle; castle.setup_failed("the script stopped with a Python error; see its log on the machine (~/castle-automation.log)")' >/dev/null 2>&1 || true
exit "$code"
