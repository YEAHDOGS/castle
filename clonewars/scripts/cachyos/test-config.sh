#!/usr/bin/env bash
# ==============================================================================
# Castle install-config regression test -- CachyOS autoinstall
# ==============================================================================
# Guards the one credential this repo almost ships: the OS install password in
# autoinstall-cachy.yml. That value is baked into the installed system by
# cachyos-installer, so a weak/shared default committed here would be known
# to everyone with read access. This test fails if the configured password
# is missing, empty, or on the weak-default blocklist.
#
# Run from the repo root (or anywhere): ./clonewars/scripts/cachyos/test-config.sh
# Exit code 0 = config passes, 1 = weak/missing password -- fix it.
# ==============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
YML="$SCRIPT_DIR/autoinstall-cachy.yml"

FAIL=0

fail() { echo "  [FAIL] $1"; FAIL=1; }
ok()   { echo "  [OK] $1"; }

echo "[castle] install config regression test"

if [ ! -f "$YML" ]; then
  fail "config not found: $YML"
  echo "[castle] result: FAIL"
  exit 1
fi

# Extract the first `password:` value, tolerating CRLF and quoting.
PW="$(grep -m1 -E '^[[:space:]]*password:' "$YML" \
  | sed -E 's/^[[:space:]]*password:[[:space:]]*"?([^"]*)"?.*/\1/' \
  | tr -d '\r')"

if [ -z "${PW}" ]; then
  fail "install password is missing or empty in autoinstall-cachy.yml"
else
  # Weak-default blocklist (case-insensitive). A committed default here is a
  # credential leak by construction -- pick a real password before installing.
  BLOCKLIST='^(wearedogs|password|123456|12345678|qwerty|admin|administrator|cachyos|castle|changeme|letmein|welcome|test|guest)$'
  if printf '%s' "$PW" | grep -qiE "$BLOCKLIST"; then
    fail "install password is a known weak default -- set a strong, unique password in autoinstall-cachy.yml"
  else
    ok "install password is set and is not a known weak default"
  fi
fi

echo ""
if [ "$FAIL" -eq 0 ]; then
  echo "[castle] result: PASS"
else
  echo "[castle] result: FAIL"
fi
exit "$FAIL"
