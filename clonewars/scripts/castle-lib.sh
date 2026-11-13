#!/usr/bin/env bash
# ==============================================================================
# Castle: helpers shared by the setup-*.sh guest scripts (sourced, not run).
# ==============================================================================

# castle_scratch_setup <user> <home>
#   The host attaches a sparse qcow2 as a virtio disk with serial
#   "castlescratch" (start.ps1 -Copy). On a live ISO everything written lands
#   in RAM, so the payload copy, the Wine prefix and OpenGOAL's build all move
#   onto that disk instead, and they survive reboots. The disk is only ever
#   touched when it carries that serial, so no other disk can be formatted by
#   mistake. Without the disk this is a no-op and everything stays in RAM.
castle_scratch_setup() {
  local user="$1" home="$2" dev="/dev/disk/by-id/virtio-castlescratch" mnt="/mnt/castle-scratch" i d
  for i in 1 2 3 4 5; do [ -e "$dev" ] && break; sleep 1; done
  if [ ! -e "$dev" ]; then
    echo "[i] No scratch disk attached; files stay on the guest's own storage (RAM on a live ISO)."
    return 0
  fi
  if ! blkid -o value -s TYPE "$dev" 2>/dev/null | grep -q .; then
    echo "[+] Scratch disk is blank -- formatting it (ext4, label CASTLESCRATCH)..."
    mkfs.ext4 -q -L CASTLESCRATCH -E lazy_itable_init=1,lazy_journal_init=1 "$dev"
  fi
  mkdir -p "$mnt"
  mountpoint -q "$mnt" || mount -o noatime,discard "$dev" "$mnt" || { echo "[-] Could not mount the scratch disk; working in RAM."; return 0; }
  # Heavy folders live on the scratch disk; the home directory only holds links.
  for d in castle-copy opengoal .wine; do
    mkdir -p "$mnt/$d"
    if [ -e "$home/$d" ] && [ ! -L "$home/$d" ]; then
      # Something already there from an earlier RAM-only run: fold it in.
      cp -a "$home/$d/." "$mnt/$d/" 2>/dev/null && rm -rf "$home/$d"
    fi
    ln -sfn "$mnt/$d" "$home/$d"
  done
  chown -R "$user:" "$mnt" 2>/dev/null || true
  echo "[+] Scratch disk: $dev on $mnt ($(df -h "$mnt" | awk 'NR==2 {print $4}') free) -> ~/castle-copy, ~/opengoal, ~/.wine"
}

# ==============================================================================
# Shared building blocks for setup-*.sh (Arch-based and Debian-based guests)
# ==============================================================================

# castle_wait_cloud_init
#   Cloud-image guests install their base packages with cloud-init on first
#   boot; the guest agent answers before that is done. Wait for it (showing its
#   log) so a setup script never fights it for the package manager.
castle_wait_cloud_init() {
  command -v cloud-init >/dev/null 2>&1 || return 0
  local state tail_pid=""
  state=$(cloud-init status 2>/dev/null | awk '/^status:/ { print $2 }')
  [ "$state" = running ] || [ "$state" = "not" ] || return 0
  echo "[+] Waiting for cloud-init to finish the first-boot install..."
  if [ -f /var/log/cloud-init-output.log ]; then
    tail -n 0 -F /var/log/cloud-init-output.log 2>/dev/null | sed -u 's/^/    | /' &
    tail_pid=$!
  fi
  cloud-init status --wait >/dev/null 2>&1 || true
  if [ -n "$tail_pid" ]; then pkill -P "$tail_pid" 2>/dev/null; kill "$tail_pid" 2>/dev/null; pkill -f 'tail -n 0 -F /var/log/cloud-init-output.log' 2>/dev/null; fi
  echo "[+] cloud-init: $(cloud-init status 2>/dev/null | awk '/^status:/ { print $2 }')"
}

# In a Castle container (container.ps1) there is no systemd, no cloud-init and
# no scratch disk: CASTLE_CONTAINER=1 is set, the X server is :0, the desktop
# user is "castle", and every helper below falls back to what is there.

# castle_user  -> sets ACTUAL_USER, USER_HOME, USER_UID (the desktop user)
castle_user() {
  ACTUAL_USER="${SUDO_USER:-}"
  if [ -z "$ACTUAL_USER" ] || [ "$ACTUAL_USER" = "root" ]; then
    ACTUAL_USER=$(loginctl list-users --no-legend 2>/dev/null | awk '$1 >= 1000 { print $2; exit }' || true)
  fi
  if [ -z "$ACTUAL_USER" ]; then
    ACTUAL_USER=$(awk -F: '$3 >= 1000 && $3 < 65534 { print $1; exit }' /etc/passwd || true)
  fi
  : "${ACTUAL_USER:=liveuser}"
  USER_HOME=$(getent passwd "$ACTUAL_USER" | cut -d: -f6)
  USER_UID=$(id -u "$ACTUAL_USER")
  echo "[+] Desktop user: $ACTUAL_USER ($USER_HOME)"
}

# castle_live_cow  -> a live ISO's root overlay is RAM, capped small by default
castle_live_cow() {
  if mountpoint -q /run/archiso/cowspace 2>/dev/null; then
    mount -o remount,size=8G /run/archiso/cowspace 2>/dev/null && echo "[+] Live session: cow space raised to 8G." || true
  fi
}

# castle_mount_label <label> <mountpoint>  -> 0 when mounted (read-only)
castle_mount_label() {
  local dev="/dev/disk/by-label/$1" mnt="$2" i
  mountpoint -q "$mnt" && return 0
  for i in 1 2 3 4 5 6 7 8 9 10; do [ -e "$dev" ] && break; sleep 2; done
  [ -e "$dev" ] || return 1
  mkdir -p "$mnt" && mount -o ro "$dev" "$mnt"
}

# castle_pm  -> pacman | apt | (empty)
castle_pm() {
  if command -v pacman >/dev/null 2>&1; then echo pacman
  elif command -v apt-get >/dev/null 2>&1; then echo apt
  fi
}

# castle_pkg_install <package>...   (names of the guest's own package manager)
castle_pkg_install() {
  [ $# -gt 0 ] || return 0
  case "$(castle_pm)" in
    pacman)
      # The live ISO ships a mirrorlist with every server commented out.
      local list=/etc/pacman.d/mirrorlist active
      active=$(grep -c '^Server' "$list" 2>/dev/null || true)
      if [ "${active:-0}" = 0 ]; then
        sed -i '/^## Worldwide/,/^$/ s/^#Server/Server/' "$list"
        active=$(grep -c '^Server' "$list" 2>/dev/null || true)
        [ "${active:-0}" = 0 ] && printf '\n## Castle\nServer = https://geo.mirror.pkgbuild.com/$repo/os/$arch\nServer = https://mirror.rackspace.com/archlinux/$repo/os/$arch\n' >> "$list"
      fi
      pacman -Sy --needed --noconfirm "$@" ;;
    apt)
      local i
      for i in $(seq 1 60); do   # another install (cloud-init, unattended upgrades) may hold the lock
        fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1 || break
        [ "$i" = 1 ] && echo "[i] Waiting for another package install to finish..."
        sleep 5
      done
      DEBIAN_FRONTEND=noninteractive apt-get update -q &&
        DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends "$@" ;;
    *) echo "[-] No supported package manager (pacman, apt) in this guest."; return 1 ;;
  esac
}

# castle_need <command> <pacman-package> <apt-package>
#   Queues the right package name in CASTLE_NEED when the command is missing.
#   Finish with: castle_pkg_install "${CASTLE_NEED[@]}"
CASTLE_NEED=()
castle_need() {
  command -v "$1" >/dev/null 2>&1 && return 0
  case "$(castle_pm)" in
    pacman) [ -n "$2" ] && CASTLE_NEED+=("$2") ;;
    apt)    [ -n "$3" ] && CASTLE_NEED+=("$3") ;;
  esac
  return 0
}

# castle_as_user <cmd...>  -> as the desktop user, outside the session (no display)
castle_as_user() { sudo -u "$ACTUAL_USER" env HOME="$USER_HOME" XDG_RUNTIME_DIR="/run/user/$USER_UID" "$@"; }

# castle_on_x <cmd...>  -> as the desktop user with the display (after castle_wait_x11)
castle_on_x() { sudo -u "$ACTUAL_USER" env HOME="$USER_HOME" XDG_RUNTIME_DIR="/run/user/$USER_UID" DISPLAY="$DISPLAY" XAUTHORITY="$XAUTHORITY" "$@"; }

# castle_wait_x11  -> 0 with DISPLAY/XAUTHORITY exported once the user's X11
#   desktop is up; 1 when there is none (or it is Wayland) after 10 minutes.
castle_wait_x11() {
  local i env x auth
  for i in $(seq 1 120); do
    env=$(castle_as_user systemctl --user show-environment 2>/dev/null || true)
    x=$(printf '%s\n' "$env" | sed -n 's/^DISPLAY=//p' | head -n 1)
    auth=$(printf '%s\n' "$env" | sed -n 's/^XAUTHORITY=//p' | head -n 1)
    if printf '%s\n' "$env" | grep -qE '^(WAYLAND_DISPLAY=|XDG_SESSION_TYPE=wayland)'; then
      echo "[-] The desktop session is Wayland; this setup needs X11."
      return 1
    fi
    # A bare window manager session may not publish DISPLAY to systemd: the
    # first X server's socket is display :0.
    if [ -z "$x" ] && [ -S /tmp/.X11-unix/X0 ]; then x=":0"; fi
    if [ -n "$x" ]; then
      export DISPLAY="$x" XAUTHORITY="${auth:-$USER_HOME/.Xauthority}"
      if castle_on_x xrandr >/dev/null 2>&1; then return 0; fi
    fi
    [ "$i" = 1 ] && echo "[+] Waiting for $ACTUAL_USER's desktop session..."
    sleep 5
  done
  echo "[-] No X11 desktop session for $ACTUAL_USER after 10 minutes."
  return 1
}

# castle_in_session <unit> <workdir> <cmd...>
#   Starts a long-running program inside the desktop user's own systemd
#   session (it outlives this script). Falls back to a detached sudo.
castle_in_session() {
  local unit="$1" dir="$2"; shift 2
  if systemd-run --machine="$ACTUAL_USER@" --user --quiet --collect --unit="$unit-$(date +%s%N)" \
       --working-directory="$dir" --setenv=DISPLAY="${DISPLAY:-:0}" --setenv=XAUTHORITY="${XAUTHORITY:-$USER_HOME/.Xauthority}" \
       --setenv=PYTHONUNBUFFERED=1 "$@" 2>/dev/null; then
    return 0
  fi
  [ -n "${CASTLE_CONTAINER:-}" ] || echo "[i] systemd-run --user unavailable, starting it with sudo -u $ACTUAL_USER"
  castle_on_x env PYTHONUNBUFFERED=1 bash -c "cd ${dir@Q} && setsid nohup ${*@Q} >/dev/null 2>&1 &"
}

# castle_ensure_screen [WxH]
#   A VM window starts at 640x480, and a desktop that is still starting up
#   puts that mode back once after it was changed. Sets the wanted mode (the
#   argument, CASTLE_SCREEN, or the first big enough one that works) and only
#   returns once it has held for a few seconds.
castle_ensure_screen() {
  local want="${1:-${CASTLE_SCREEN:-}}" out mode steady=0 i now
  # A container's screen (Xvfb) is made at the right size and nothing resets it.
  [ -n "${CASTLE_CONTAINER:-}" ] && return 0
  out=$(castle_on_x xrandr 2>/dev/null | awk '/ connected/ { print $1; exit }')
  [ -n "$out" ] || return 0
  for i in $(seq 1 30); do
    now=$(castle_on_x xrandr 2>/dev/null | sed -n 's/.*current \([0-9]*\) x \([0-9]*\).*/\1x\2/p' | head -n 1)
    if { [ -n "$want" ] && [ "$now" = "$want" ]; } || { [ -z "$want" ] && [ "${now#*x}" -ge 800 ] 2>/dev/null; }; then
      steady=$((steady + 1))
      [ "$steady" -ge 4 ] && return 0
    else
      steady=0
      for mode in $want 1440x900 1280x960 1280x1024 1920x1080; do
        castle_on_x xrandr --output "$out" --mode "$mode" 2>/dev/null && { echo "[+] Screen set to $mode."; break; }
      done
    fi
    sleep 2
  done
  echo "[i] The screen is still $now."
}

# castle_profile_apply [root|user]
#   A launch profile (start.ps1 -Profile) arrives as /tmp/castle-profile.env:
#   package lists per package manager, commands to run as root, and commands
#   to run as the desktop user inside the X11 session. "root" installs the
#   packages and runs the root commands; "user" starts the user commands (call
#   it once the session is up). Without a profile both do nothing.
CASTLE_PKGS_PACMAN=(); CASTLE_PKGS_APT=(); CASTLE_ROOT_COMMANDS=(); CASTLE_USER_COMMANDS=()
castle_profile_apply() {
  local phase="${1:-root}" file="${CASTLE_PROFILE_FILE:-/tmp/castle-profile.env}" c
  [ -f "$file" ] || return 0
  # shellcheck disable=SC1090
  . "$file"
  if [ "$phase" = root ]; then
    case "$(castle_pm)" in
      pacman) if [ "${#CASTLE_PKGS_PACMAN[@]}" -gt 0 ]; then echo "[+] Profile packages: ${CASTLE_PKGS_PACMAN[*]}"; castle_pkg_install "${CASTLE_PKGS_PACMAN[@]}" || return 1; fi ;;
      apt)    if [ "${#CASTLE_PKGS_APT[@]}" -gt 0 ]; then echo "[+] Profile packages: ${CASTLE_PKGS_APT[*]}"; castle_pkg_install "${CASTLE_PKGS_APT[@]}" || return 1; fi ;;
    esac
    for c in "${CASTLE_ROOT_COMMANDS[@]}"; do
      echo "[+] Profile (root): $c"
      bash -c "$c" || { echo "[-] Profile command failed: $c"; return 1; }
    done
  else
    for c in "${CASTLE_USER_COMMANDS[@]}"; do
      echo "[+] Profile (user session): $c"
      castle_in_session castle-profile "$USER_HOME" bash -lc "$c"
    done
  fi
  return 0
}
