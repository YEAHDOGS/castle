#!/usr/bin/env bash
# Regression check: the founder's personal name must never appear in this repo.
# Personal-name needles redacted per 2026-09-13 privacy order.
# Re-seed from private notes to re-arm this guard. Product attribution is "DOGS".
set -euo pipefail
cd "$(dirname "$0")/.."
# Tracked and untracked files, but not what git ignores (app/data's disks, build output,
# node_modules) or other checkouts' worktrees under .claude/.
if git grep --untracked -nIiE -e 'REDACTED-NAME-1' -e 'REDACTED-NAME-2' -e 'REDACTED-NAME-3' -- . ':!.claude' ':!scripts/name-audit-check.sh'; then
  echo "FAIL: prohibited personal-name references found above" >&2
  exit 1
fi
echo "name-audit: clean"
