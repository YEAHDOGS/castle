#!/usr/bin/env bash
# Regression check: the founder's personal name must never appear in this repo.
# Prohibited: "the user", "user"/"user" (any case),
# "the user", "the founder". Product attribution is "DOGS".
set -euo pipefail
cd "$(dirname "$0")/.."
if grep -rInE --exclude-dir=.git --exclude-dir=node_modules -e 'user' -e 'the user' -e 'the founder' . | grep -v '^./scripts/name-audit-check.sh:'; then
  echo "FAIL: prohibited personal-name references found above" >&2
  exit 1
fi
echo "name-audit: clean"
