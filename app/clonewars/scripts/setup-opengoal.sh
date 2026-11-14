#!/usr/bin/env bash
# ==============================================================================
# Castle: OpenGOAL (Jak and Daxter PC port) on an Arch-based guest, from scratch.
# ==============================================================================
# Usage (root):
#   setup-opengoal.sh --copy [--all]       copy the CASTLECOPY share, install OpenGOAL, build the game
#                                          from the disc, launch it in the desktop session
#   setup-opengoal.sh --copy --program X   same; X is ignored (kept for the shared flag vocabulary)
#
# Inputs, all optional but you need a game disc from somewhere:
#   ~/castle-copy/opengoal-linux-*.tar.gz  the official Linux release (app/data/clonewars/copy on the host);
#                                          without it the latest release is fetched from GitHub
#   /dev/sr1..                             game ISOs the host attached as extra CD-ROMs (any *.iso in
#                                          app/data/clonewars/copy is attached this way -- too big for the FAT share)
#   ~/castle-copy/*.iso                    a small ISO that did fit the share
#
# From the host:  .\start.ps1 endeavouros -c -Setup opengoal
# Inside the VM:  sudo mount -o ro /dev/disk/by-label/CASTLE /mnt && sudo bash /mnt/setup-opengoal.sh --copy --all
set -euo pipefail

COPY=0
RUN_ALL=0
GAME="${CASTLE_OPENGOAL_GAME:-jak1}"
while [ $# -gt 0 ]; do
  case "$1" in
    -c|--copy)    COPY=1 ;;
    -a|--all)     RUN_ALL=1 ;;
    -p|--program) shift ;;   # accepted for symmetry with setup-wine.sh, unused
    --game)       shift; GAME="$1" ;;
    -h|--help)    sed -n '5,18p' "$0"; exit 0 ;;
    *)            ;;
  esac
  shift
done

if [ "$EUID" -ne 0 ]; then
  echo "[-] Please run this script with sudo."
  exit 1
fi

# --- Desktop user ---------------------------------------------------------------
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
OG_DIR="$USER_HOME/opengoal"
echo "[+] Desktop user: $ACTUAL_USER ($USER_HOME)"

# Shared helpers (next to this script, uploaded by the host or on the CASTLE share).
for lib in "$(dirname "$0")/castle-castle-lib.sh" "$(dirname "$0")/castle-lib.sh" /mnt/castle/castle-lib.sh; do
  [ -f "$lib" ] && { . "$lib"; break; }
done
type castle_scratch_setup >/dev/null 2>&1 && castle_scratch_setup "$ACTUAL_USER" "$USER_HOME"

# Live ISO: the root overlay is RAM. The extracted disc (~1.4 GB), decompiler
# output and compiled game (~1.5 GB) all land there, so give it room.
if mountpoint -q /run/archiso/cowspace 2>/dev/null; then
  mount -o remount,size=14G /run/archiso/cowspace 2>/dev/null && echo "[+] Live session: cow space raised to 14G." || true
fi

mount_label() { # <label> <mountpoint>
  local dev="/dev/disk/by-label/$1" mnt="$2" i
  mountpoint -q "$mnt" && return 0
  for i in 1 2 3 4 5 6 7 8 9 10; do [ -e "$dev" ] && break; sleep 2; done
  [ -e "$dev" ] || return 1
  mkdir -p "$mnt" && mount -o ro "$dev" "$mnt"
}

# --- Step 0: payload share ----------------------------------------------------
if [ "$COPY" = 1 ]; then
  echo "[+] Step 0: Copying the Castle copy share into $COPY_DST..."
  if mount_label CASTLECOPY /mnt/castle-copy; then
    mkdir -p "$COPY_DST"
    cp -rvn /mnt/castle-copy/. "$COPY_DST"/ | sed 's/^/    /'   # -n: keep files an earlier script already copied
    chown -R "$ACTUAL_USER:" "$(readlink -f "$COPY_DST")"; chmod -R u+rw "$(readlink -f "$COPY_DST")"   # real folder: it may be a symlink onto the scratch disk
  else
    echo "[-] No CASTLECOPY share attached. Continuing without it."
  fi
fi

# --- Step 1: find the game disc -------------------------------------------------
# PS2 discs carry SYSTEM.CNF at the root; that is how a mounted disc is recognised.
GAME_SRC=""
GAME_IS_FOLDER=0
shopt -s nullglob nocaseglob
for dev in /dev/sr[1-9]; do
  mnt="/mnt/castle-cd${dev#/dev/sr}"
  mkdir -p "$mnt"
  mountpoint -q "$mnt" || mount -o ro "$dev" "$mnt" 2>/dev/null || { rmdir "$mnt" 2>/dev/null; continue; }
  if [ -e "$mnt/SYSTEM.CNF" ] || [ -e "$mnt/system.cnf" ]; then
    echo "[+] Game disc: $dev mounted at $mnt ($(ls "$mnt" | wc -l) entries)"
    # The extractor writes buildinfo.json into the folder it is given, and a
    # CD-ROM is read-only, so hand it a writable overlay of the disc instead
    # (overlayfs: reads come from the disc, the few writes land in /tmp).
    up="/tmp/castle-game-upper"; wk="/tmp/castle-game-work"; ov="/mnt/castle-game"
    mkdir -p "$up" "$wk" "$ov"
    if mountpoint -q "$ov" || mount -t overlay overlay -o "lowerdir=$mnt,upperdir=$up,workdir=$wk" "$ov"; then
      chown "$ACTUAL_USER:" "$up" "$ov" 2>/dev/null || true
      GAME_SRC="$ov"
    else
      GAME_SRC="$mnt"
    fi
    GAME_IS_FOLDER=1
    break
  fi
done
if [ -z "$GAME_SRC" ]; then
  for iso in "$COPY_DST"/*.iso; do GAME_SRC="$iso"; echo "[+] Game ISO: $iso"; break; done
fi
if [ -z "$GAME_SRC" ]; then
  echo "[-] No game disc found: put the Jak and Daxter .iso in app/data/clonewars/copy on the host (it is attached as a CD-ROM)."
  exit 2
fi

# --- Step 2: OpenGOAL release ---------------------------------------------------
echo "[+] Step 2: Installing OpenGOAL into $OG_DIR..."
mkdir -p "$OG_DIR"
TARBALL=""
for t in "$COPY_DST"/opengoal-linux-*.tar.gz; do TARBALL="$t"; done
if [ -z "$TARBALL" ]; then
  echo "[+] No release tarball in the payload; fetching the latest Linux release from GitHub..."
  URL=$(curl -fsSL https://api.github.com/repos/open-goal/jak-project/releases/latest | grep -o 'https://[^"]*opengoal-linux-[^"]*\.tar\.gz' | head -n 1)
  [ -n "$URL" ] || { echo "[-] Could not resolve the OpenGOAL release URL."; exit 3; }
  TARBALL="$COPY_DST/$(basename "$URL")"
  mkdir -p "$COPY_DST"
  curl -fL --progress-bar -o "$TARBALL" "$URL"
fi
if [ ! -x "$OG_DIR/gk" ]; then
  tar -xzf "$TARBALL" -C "$OG_DIR"
  chmod +x "$OG_DIR"/gk "$OG_DIR"/goalc "$OG_DIR"/extractor
fi
chown -R "$ACTUAL_USER:" "$(readlink -f "$OG_DIR")"   # real folder (symlink onto the scratch disk)
echo "[+] OpenGOAL: $(basename "$TARBALL") -> $OG_DIR"

# The release binaries only need zlib beyond glibc; make sure it is there.
ldconfig -p | grep -q 'libz\.so\.1' || pacman -S --needed --noconfirm zlib >/dev/null 2>&1 || true

# --- Step 3: extract, decompile, compile (as the desktop user) -----------------
PROJ="$OG_DIR/data"
run_og() { # runs one OpenGOAL tool as the user, from its folder
  sudo -u "$ACTUAL_USER" env HOME="$USER_HOME" bash -c "cd '$OG_DIR' && $*"
}
if [ -f "$PROJ/out/$GAME/fr3/.castle-built" ]; then
  echo "[+] Step 3: Game already built, skipping extraction."
else
  echo "[+] Step 3a: Extracting the disc (copies the game files into $PROJ/iso_data/$GAME)..."
  rm -rf "$PROJ/iso_data/_temp"   # a previous aborted attempt leaves 1.4 GB here (RAM on the live ISO)
  if [ "$GAME_IS_FOLDER" = 1 ]; then
    run_og "./extractor --extract --validate --proj-path '$PROJ' --game '$GAME' --folder '$GAME_SRC'"
  else
    run_og "./extractor --extract --validate --proj-path '$PROJ' --game '$GAME' '$GAME_SRC'"
  fi
  # Every extractor step wants the game-files-path positional, even after extraction.
  FOLDER_FLAG=""; [ "$GAME_IS_FOLDER" = 1 ] && FOLDER_FLAG="--folder"
  echo "[+] Step 3b: Decompiling (this is the slow part; ~2-5 min with a dozen cores)..."
  run_og "./extractor --decompile --proj-path '$PROJ' --game '$GAME' $FOLDER_FLAG '$GAME_SRC'"
  echo "[+] Step 3c: Compiling the game..."
  run_og "./extractor --compile --proj-path '$PROJ' --game '$GAME' $FOLDER_FLAG '$GAME_SRC'"
  touch "$PROJ/out/$GAME/fr3/.castle-built" 2>/dev/null || true
fi

# --- Step 4: launch in the desktop session --------------------------------------
wait_for_session() {
  local i
  for i in $(seq 1 120); do
    if [ -S "/run/user/$USER_UID/bus" ] && loginctl list-sessions --no-legend 2>/dev/null | awk '{ print $3 }' | grep -qx "$ACTUAL_USER"; then return 0; fi
    [ "$i" = 1 ] && echo "[+] Waiting for $ACTUAL_USER's desktop session..."
    sleep 5
  done
  return 1
}
wait_for_session || { echo "[-] No desktop session; run it yourself:  cd $OG_DIR && ./gk -v --proj-path data --game $GAME -- -boot -fakeiso"; exit 1; }

# What OpenGL the session actually has (virgl = host GPU, llvmpipe = software).
if command -v glxinfo >/dev/null 2>&1; then
  sudo -u "$ACTUAL_USER" env DISPLAY=:0 glxinfo -B 2>/dev/null | grep -E 'OpenGL (renderer|version)' | sed 's/^/    /' || true
fi

# The live session starts with its sink at 40%; the VM's HDA card is the only
# output, so run it at full volume (the host mixer is the real volume knob).
if command -v wpctl >/dev/null 2>&1; then
  sudo -u "$ACTUAL_USER" XDG_RUNTIME_DIR="/run/user/$USER_UID" wpctl set-volume @DEFAULT_AUDIO_SINK@ 1.0 2>/dev/null || true
  sudo -u "$ACTUAL_USER" XDG_RUNTIME_DIR="/run/user/$USER_UID" wpctl set-mute @DEFAULT_AUDIO_SINK@ 0 2>/dev/null || true
  echo "[+] Audio: $(sudo -u "$ACTUAL_USER" XDG_RUNTIME_DIR="/run/user/$USER_UID" wpctl get-volume @DEFAULT_AUDIO_SINK@ 2>/dev/null || echo 'no sink')"
fi

echo "[+] Step 4: Launching $GAME..."
# No GPU in the VM: Mesa's llvmpipe renders in software (OpenGL 4.5). Let it
# use every core the VM has.
LP_THREADS=$(nproc)
unit="castle-opengoal-$(date +%s%N)"
if ! systemd-run --machine="$ACTUAL_USER@" --user --quiet --collect --unit="$unit" \
     --working-directory="$OG_DIR" --setenv=LP_NUM_THREADS="$LP_THREADS" --setenv=MESA_GL_VERSION_OVERRIDE=4.5 \
     "$OG_DIR/gk" -v --proj-path "$PROJ" --game "$GAME" -- -boot -fakeiso 2>/dev/null; then
  sudo -u "$ACTUAL_USER" env DISPLAY="${DISPLAY:-:0}" XDG_RUNTIME_DIR="/run/user/$USER_UID" HOME="$USER_HOME" LP_NUM_THREADS="$LP_THREADS" \
    bash -c "cd '$OG_DIR' && nohup ./gk -v --proj-path '$PROJ' --game '$GAME' -- -boot -fakeiso >/tmp/castle-gk.log 2>&1 &"
fi
echo "[+] Done. OpenGOAL lives in $OG_DIR (game data under $PROJ/out/$GAME)."
