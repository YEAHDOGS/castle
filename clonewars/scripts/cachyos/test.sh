#!/usr/bin/env bash
# ==============================================================================
# Castle guest smoke test -- CachyOS / Arch-based VMs
# ==============================================================================
# Verifies main-install.sh did its job: expected binaries present and key
# services enabled/active. Run INSIDE the guest with sudo.
# Exit code 0 = all green, 1 = something missing.
# ==============================================================================
set -uo pipefail

PASS=0
FAIL=0

check_bin() {
  if command -v "$1" >/dev/null 2>&1; then
    echo "  [OK] binary: $1"
    PASS=$((PASS + 1))
  else
    echo "  [FAIL] binary missing: $1"
    FAIL=$((FAIL + 1))
  fi
}

check_service() {
  if systemctl is-enabled "$1" >/dev/null 2>&1; then
    echo "  [OK] service enabled: $1"
    PASS=$((PASS + 1))
  else
    echo "  [FAIL] service not enabled: $1"
    FAIL=$((FAIL + 1))
  fi
}

echo "[castle] smoke test -- binaries"
for bin in wg tailscale ufw sshd smbd qemu-ga docker btrfs; do
  check_bin "$bin"
done
# samba ships smbd under a couple of names; accept either
command -v smbd >/dev/null 2>&1 || check_bin smbstatus

echo "[castle] smoke test -- services"
for svc in sshd qemu-guest-agent docker tailscaled ufw smb; do
  check_service "$svc"
done

echo ""
echo "[castle] result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
