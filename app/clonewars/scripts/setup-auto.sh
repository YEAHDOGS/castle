#!/usr/bin/env bash
# ==============================================================================
# Castle: run whatever the payload calls for.
# ==============================================================================
# The default for `start.ps1 -Copy`. Looks at what came in (the CASTLECOPY
# share + any extra CD-ROMs) and runs the matching setup scripts in turn:
#   *.exe / *.msi anywhere in the payload  -> setup-wine.sh   (Wine, then run each of them)
#   a PS2 disc (SYSTEM.CNF) on a CD-ROM     -> setup-opengoal.sh (build + launch Jak and Daxter)
# Flags are passed straight through (--copy, --all, --program X).
#
# Inside the VM: sudo mount -o ro /dev/disk/by-label/CASTLE /mnt && sudo bash /mnt/setup-auto.sh --copy --all
set -uo pipefail

if [ "$EUID" -ne 0 ]; then echo "[-] Please run this script with sudo."; exit 1; fi

HERE=$(cd "$(dirname "$0")" && pwd)
sibling() { # <name>  -> path of setup-<name>.sh next to this script (uploaded or on the share)
  local n
  for n in "$HERE/castle-setup-$1.sh" "$HERE/setup-$1.sh" "/mnt/castle/setup-$1.sh"; do
    [ -f "$n" ] && { echo "$n"; return 0; }
  done
  return 1
}

# --- Look at the payload ----------------------------------------------------
mount_label() { local dev="/dev/disk/by-label/$1" mnt="$2" i; mountpoint -q "$mnt" && return 0; for i in 1 2 3 4 5 6 7 8 9 10; do [ -e "$dev" ] && break; sleep 2; done; [ -e "$dev" ] || return 1; mkdir -p "$mnt" && mount -o ro "$dev" "$mnt"; }
HAVE_WINDOWS_APPS=0
HAVE_DISC=0
if mount_label CASTLECOPY /mnt/castle-copy; then
  if find /mnt/castle-copy -maxdepth 3 -type f \( -iname '*.exe' -o -iname '*.msi' \) | grep -q .; then HAVE_WINDOWS_APPS=1; fi
fi
shopt -s nullglob
for dev in /dev/sr[1-9]; do
  mnt="/mnt/castle-cd${dev#/dev/sr}"; mkdir -p "$mnt"
  mountpoint -q "$mnt" || mount -o ro "$dev" "$mnt" 2>/dev/null || { rmdir "$mnt" 2>/dev/null; continue; }
  if [ -e "$mnt/SYSTEM.CNF" ] || [ -e "$mnt/system.cnf" ]; then HAVE_DISC=1; fi
done
echo "[+] Payload: Windows apps=$HAVE_WINDOWS_APPS, game disc=$HAVE_DISC"

RC=0
if [ "$HAVE_WINDOWS_APPS" = 1 ]; then
  s=$(sibling wine) && { echo "[+] === setup-wine.sh $* ==="; bash "$s" "$@" || RC=$?; } || echo "[-] setup-wine.sh not found"
fi
if [ "$HAVE_DISC" = 1 ]; then
  s=$(sibling opengoal) && { echo "[+] === setup-opengoal.sh $* ==="; bash "$s" "$@" || RC=$?; } || echo "[-] setup-opengoal.sh not found"
fi
if [ "$HAVE_WINDOWS_APPS" = 0 ] && [ "$HAVE_DISC" = 0 ]; then
  echo "[i] Nothing to set up: no .exe/.msi in the payload and no game disc. Copying the payload only."
  s=$(sibling wine) && bash "$s" --copy || RC=$?
fi
exit $RC
