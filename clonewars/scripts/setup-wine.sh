#!/usr/bin/env bash
# ==============================================================================
# Castle: install Wine on an Arch-based guest and run Windows programs.
# ==============================================================================
# Usage (root):
#   setup-wine.sh                          install Wine only
#   setup-wine.sh /path/to/app.exe         install Wine, then run that program
#   setup-wine.sh --copy                   also copy the CASTLECOPY share (host data/copy) into ~/castle-copy
#   setup-wine.sh --copy --all             ...and run every .exe that was copied
#   setup-wine.sh --copy --program NAME    ...and run NAME (a file inside ~/castle-copy, or a path)
#
# Inside the VM:   sudo mount -o ro /dev/disk/by-label/CASTLE /mnt && sudo bash /mnt/setup-wine.sh --copy --all
# From the host:   .\start.ps1 endeavouros -Copy    (runs this through the QEMU guest agent, no login needed)
#
# When the guest agent runs it there is no SUDO_USER and no display in the
# environment, so the desktop user is discovered from logind and programs are
# started inside that user's own systemd session, which already knows about
# the display. Written for the EndeavourOS live ISO (user: liveuser); works the
# same on an installed EndeavourOS/Arch with qemu-guest-agent.
set -euo pipefail

COPY=0
RUN_ALL=0
PROGRAMS=()
while [ $# -gt 0 ]; do
  case "$1" in
    -c|--copy)    COPY=1 ;;
    -a|--all)     RUN_ALL=1 ;;
    -p|--program) shift; [ $# -gt 0 ] || { echo "[-] --program needs a value"; exit 2; }; PROGRAMS+=("$1") ;;
    -h|--help)    sed -n '5,13p' "$0"; exit 0 ;;
    *)            PROGRAMS+=("$1") ;;
  esac
  shift
done

if [ "$EUID" -ne 0 ]; then
  echo "[-] Please run this script with sudo: sudo $0 [--copy] [--all] [--program app.exe] [path_to_exe]"
  exit 1
fi

# --- Which user gets the Wine prefix and the windows -------------------------
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
COPY_DST="$USER_HOME/castle-copy"
echo "[+] Desktop user: $ACTUAL_USER ($USER_HOME)"

# Shared helpers (next to this script, uploaded by the host or on the CASTLE share).
for lib in "$(dirname "$0")/castle-castle-lib.sh" "$(dirname "$0")/castle-lib.sh" /mnt/castle/castle-lib.sh; do
  [ -f "$lib" ] && { . "$lib"; break; }
done
type castle_scratch_setup >/dev/null 2>&1 && castle_scratch_setup "$ACTUAL_USER" "$USER_HOME"

# --- Live ISO: the root overlay lives in RAM and archiso caps it at 256M by
# default, which is far too small for Wine and its 32-bit half. Grow it; RAM is
# only used as files are written.
if mountpoint -q /run/archiso/cowspace 2>/dev/null; then
  mount -o remount,size=8G /run/archiso/cowspace 2>/dev/null && echo "[+] Live session: cow space raised to 8G." || true
fi

mount_label() { # <label> <mountpoint>  -> 0 when mounted
  local dev="/dev/disk/by-label/$1" mnt="$2" i
  mountpoint -q "$mnt" && return 0
  for i in 1 2 3 4 5 6 7 8 9 10; do   # udev may still be settling on a fresh boot
    [ -e "$dev" ] && break
    sleep 2
  done
  [ -e "$dev" ] || return 1
  mkdir -p "$mnt"
  mount -o ro "$dev" "$mnt"
}

# --- Step 0: copy the payload share -----------------------------------------
if [ "$COPY" = 1 ]; then
  echo "[+] Step 0: Copying the Castle copy share into $COPY_DST..."
  if mount_label CASTLECOPY /mnt/castle-copy; then
    mkdir -p "$COPY_DST"
    # -n: files already copied by an earlier script in the same session stay put.
    cp -rvn /mnt/castle-copy/. "$COPY_DST"/ | sed 's/^/    /'
    # ~/castle-copy may be a symlink onto the scratch disk: chown/find must
    # work on the real folder (neither follows a symlink given as argument).
    chown -R "$ACTUAL_USER:" "$(readlink -f "$COPY_DST")"
    chmod -R u+rw "$(readlink -f "$COPY_DST")"
    echo "[+] $(find "$COPY_DST/" -type f | wc -l) file(s) in $COPY_DST."
  else
    echo "[-] No CASTLECOPY share attached (the host adds it with start.ps1 -Copy). Nothing copied."
  fi
fi

echo "[+] Step 1: Checking and enabling [multilib] in /etc/pacman.conf..."
# Uncomment [multilib] and the Include line directly beneath it
sed -i '/^#\[multilib\]/{s/^#//;n;s/^#//}' /etc/pacman.conf

# The live ISO ships /etc/pacman.d/mirrorlist with every Server commented out
# (its Welcome app expects a "Update Mirrors" click first), so pacman fails
# with "no servers configured for repository". Enable the Worldwide block.
MIRRORLIST=/etc/pacman.d/mirrorlist
# grep -c prints the count AND exits 1 when it is 0, so never "|| echo 0" it.
ACTIVE_MIRRORS=$(grep -c '^Server' "$MIRRORLIST" 2>/dev/null || true)
if [ "${ACTIVE_MIRRORS:-0}" = 0 ]; then
  echo "[+] Step 1b: No active Arch mirror -- enabling the Worldwide mirrors..."
  sed -i '/^## Worldwide/,/^$/ s/^#Server/Server/' "$MIRRORLIST"
  ACTIVE_MIRRORS=$(grep -c '^Server' "$MIRRORLIST" 2>/dev/null || true)
  if [ "${ACTIVE_MIRRORS:-0}" = 0 ]; then
    printf '\n## Castle\nServer = https://geo.mirror.pkgbuild.com/$repo/os/$arch\nServer = https://mirror.rackspace.com/archlinux/$repo/os/$arch\n' >> "$MIRRORLIST"
  fi
  grep '^Server' "$MIRRORLIST" | sed 's/^/    /'
fi

echo "[+] Step 2: Refreshing package databases..."
pacman -Sy --noconfirm

echo "[+] Step 3: Installing Wine, Gecko, Mono, and Winetricks..."
pacman -S --needed --noconfirm wine wine-gecko wine-mono winetricks

echo "[+] Wine installed successfully: $(wine --version)"

# --- Step 4: which programs -------------------------------------------------
if [ "$RUN_ALL" = 1 ] && [ -d "$COPY_DST" ]; then
  # Every .exe and .msi that came in (installers included: they run and the
  # user clicks through them like on Windows). Uninstallers are skipped.
  while IFS= read -r f; do PROGRAMS+=("$f"); done < <(find "$COPY_DST/" -maxdepth 3 -type f \( -iname '*.exe' -o -iname '*.msi' \) ! -iname 'unins*' | sort)
fi

resolve_program() { # name-or-path -> absolute path, or empty
  local p="$1" hit
  if [ -f "$p" ]; then readlink -f "$p"; return; fi
  if [ -f "$COPY_DST/$p" ]; then readlink -f "$COPY_DST/$p"; return; fi
  hit=$(find "$COPY_DST/" -maxdepth 3 -type f -iname "$p" 2>/dev/null | head -n 1 || true)
  [ -n "$hit" ] && readlink -f "$hit"
  return 0
}

if [ ${#PROGRAMS[@]} -eq 0 ]; then
  echo "[+] Setup complete. You can now run any .exe file using:"
  echo "    wine /path/to/app.exe"
  [ -d "$COPY_DST" ] && echo "    (your copied files are in $COPY_DST)"
  exit 0
fi

# --- Step 5: run the programs in the desktop session ------------------------
wait_for_session() {
  local i
  for i in $(seq 1 120); do   # up to 10 minutes
    if [ -S "/run/user/$USER_UID/bus" ] && loginctl list-sessions --no-legend 2>/dev/null | awk '{ print $3 }' | grep -qx "$ACTUAL_USER"; then
      return 0
    fi
    [ "$i" = 1 ] && echo "[+] Waiting for $ACTUAL_USER's desktop session..."
    sleep 5
  done
  return 1
}

# run_in_session [--wait] <workdir> <cmd...>
#   Runs a command inside the desktop user's own systemd session (which
#   carries DISPLAY / WAYLAND_DISPLAY / DBUS from the login). With --wait it
#   blocks until the command exits and returns its status. Falls back to
#   sudo + the first X display when the user manager is unreachable.
run_in_session() {
  local wait_flag="" dir unit
  if [ "$1" = "--wait" ]; then wait_flag="--wait"; shift; fi
  dir="$1"; shift
  unit="castle-wine-$(date +%s%N)"
  if systemd-run --machine="$ACTUAL_USER@" --user --quiet --collect $wait_flag \
       --unit="$unit" --working-directory="$dir" --setenv=WINEDEBUG=-all "$@" 2>/dev/null; then
    return 0
  fi
  local rc=$?
  # systemd-run --wait reports the command's own failure with the same code path;
  # only fall back when the user manager itself is unreachable.
  if [ -z "$wait_flag" ] || ! systemctl --user -M "$ACTUAL_USER@" is-system-running >/dev/null 2>&1; then
    echo "[i] systemd-run --user unavailable, falling back to sudo -u $ACTUAL_USER"
    if [ -n "$wait_flag" ]; then
      sudo -u "$ACTUAL_USER" env DISPLAY="${DISPLAY:-:0}" XDG_RUNTIME_DIR="/run/user/$USER_UID" WINEDEBUG=-all \
        bash -c "cd ${dir@Q} && ${*@Q}"
    else
      sudo -u "$ACTUAL_USER" env DISPLAY="${DISPLAY:-:0}" XDG_RUNTIME_DIR="/run/user/$USER_UID" WINEDEBUG=-all \
        bash -c "cd ${dir@Q} && nohup ${*@Q} >/dev/null 2>&1 &"
    fi
    return $?
  fi
  return $rc
}

# installer_kind <file> -> inno | nsis | installshield | msi | app
#   The big installer families each embed their name in the binary.
installer_kind() {
  case "${1,,}" in *.msi) echo msi; return ;; esac
  if grep -qa "Inno Setup" "$1"; then echo inno
  elif grep -qa "Nullsoft" "$1"; then echo nsis
  elif grep -qa "InstallShield" "$1"; then echo installshield
  else echo app; fi
}

# install_and_launch <file>
#   Plain programs just start. Installers run silently (their own quiet
#   switches), and once they finish the Start Menu shortcut they created is
#   launched, so an installer in the payload ends with the installed app on
#   screen. If the silent run fails, the installer's wizard is shown instead.
install_and_launch() {
  local exe="$1" dir kind marker lnk
  dir=$(dirname "$exe")
  kind=$(installer_kind "$exe")
  local -a cmd
  case "$kind" in
    app)           echo "[+] Launching $exe as $ACTUAL_USER..."; run_in_session "$dir" wine "$exe"; return ;;
    inno)          cmd=(wine "$exe" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /NOCANCEL) ;;
    nsis)          cmd=(wine "$exe" /S) ;;
    installshield) cmd=(wine "$exe" /s '/v"/qn"') ;;
    msi)           cmd=(wine msiexec /i "$exe" /qn) ;;
  esac
  marker=$(mktemp /tmp/castle-install.XXXXXX)
  echo "[+] Installing $(basename "$exe") silently ($kind installer)..."
  if run_in_session --wait "$dir" "${cmd[@]}"; then
    # Wine mirrors Windows shortcuts under drive_c; take the newest non-uninstall one.
    lnk=$(find "$USER_HOME/.wine/drive_c/" -newer "$marker" -iname '*.lnk' -ipath '*start menu*' \
            ! -iname '*uninstall*' ! -iname '*remove*' ! -iname '*readme*' ! -iname '*website*' ! -iname '*help*' 2>/dev/null | head -n 1)
    if [ -n "$lnk" ]; then
      echo "[+] Installed. Launching its shortcut: $(basename "$lnk" .lnk)"
      run_in_session "$USER_HOME" wine start /unix "$lnk"
    else
      echo "[i] Installed, but it left no Start Menu shortcut; find it under ~/.wine/drive_c."
    fi
  else
    echo "[i] Silent install did not work; running the installer with its wizard."
    run_in_session "$dir" wine "$exe"
  fi
  rm -f "$marker"
}

if ! wait_for_session; then
  echo "[-] No desktop session for $ACTUAL_USER after 10 minutes; programs not started."
  echo "    Run them yourself:  cd $COPY_DST && wine <app>.exe"
  exit 1
fi

FAILED=0
for p in "${PROGRAMS[@]}"; do
  target=$(resolve_program "$p")
  if [ -z "$target" ]; then
    echo "[-] File not found: $p"
    FAILED=1
    continue
  fi
  install_and_launch "$target"
done
echo "[+] Done. Wine prefix: $USER_HOME/.wine  |  copied files: $COPY_DST"
exit $FAILED
