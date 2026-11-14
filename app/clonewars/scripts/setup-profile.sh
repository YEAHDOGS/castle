#!/usr/bin/env bash
# ==============================================================================
# Castle: apply a launch profile and nothing else.
# ==============================================================================
# The setup that runs for `start.ps1 -Profile <name>` when the profile names no
# Setup of its own. It copies the payload (host app/data/clonewars/copy) into ~/castle-copy,
# installs the profile's packages, runs its root commands, and starts its user
# commands inside the desktop session. A profile with an app-specific setup
# (Setup = "penguin") gets the same steps from castle_profile_apply inside
# that script instead.
#
# Inside the VM:  sudo bash /mnt/castle/setup-profile.sh --copy
set -uo pipefail

if [ "$EUID" -ne 0 ]; then echo "[-] Please run this script with sudo."; exit 1; fi

for lib in "$(dirname "$0")/castle-castle-lib.sh" "$(dirname "$0")/castle-lib.sh" /mnt/castle/castle-lib.sh; do
  [ -f "$lib" ] && { . "$lib"; break; }
done
type castle_user >/dev/null 2>&1 || { echo "[-] castle-lib.sh not found next to this script."; exit 1; }

castle_wait_cloud_init
castle_user
castle_scratch_setup "$ACTUAL_USER" "$USER_HOME"
castle_live_cow
COPY_DST="$USER_HOME/castle-copy"

if castle_mount_label CASTLECOPY /mnt/castle-copy; then
  echo "[+] Copying the payload into $COPY_DST..."
  mkdir -p "$COPY_DST"
  cp -rf /mnt/castle-copy/. "$COPY_DST"/
  chown -R "$ACTUAL_USER:" "$(readlink -f "$COPY_DST")"
  chmod -R u+rw "$(readlink -f "$COPY_DST")"
  echo "[+] $(find "$COPY_DST/" -type f | wc -l) file(s) in $COPY_DST."
fi

castle_profile_apply root || exit 1

# The session part only matters when the profile asks for it.
[ -f "${CASTLE_PROFILE_FILE:-/tmp/castle-profile.env}" ] && . "${CASTLE_PROFILE_FILE:-/tmp/castle-profile.env}"
if [ "${#CASTLE_USER_COMMANDS[@]}" -gt 0 ] || [ -n "${CASTLE_SCREEN:-}" ]; then
  castle_wait_x11 || exit 1
  castle_ensure_screen
  castle_profile_apply user
fi
echo "[+] Profile applied."
exit 0
