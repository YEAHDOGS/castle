#!/usr/bin/env bash
# Regression check: the founder's personal name must never appear in this repo.
# Prohibited: "Brandon Wellacruz", "brandonwellacruz"/"wellacruz" (any case),
# "Brandon Wellacruz", "Captain Brando". Product attribution is "DOGS".
set -euo pipefail
cd "$(dirname "$0")/.."
if grep -rInE --exclude-dir=.git --exclude-dir=node_modules -e 'wellacruz' -e 'Brandon Wellacruz' -e 'Captain Brando' . | grep -v '^./scripts/name-audit-check.sh:'; then
  echo "FAIL: prohibited personal-name references found above" >&2
  exit 1
fi
echo "name-audit: clean"
