#!/usr/bin/env bash
# ==============================================================================
# Castle: run penguin-autosurf (the Cart Surfer bot) in a Linux guest.
# ==============================================================================
# Payload (host data/copy): the project as penguin-autosurf.zip, or as an
# unpacked folder. Anything holding main.py + games/cart_surfer counts.
#
# Usage (root):
#   setup-penguin.sh --copy            unpack the payload, install what is missing,
#                                      open Firefox on the game and start the bot
#   setup-penguin.sh --copy --no-run   everything except starting Firefox and the bot
#   (--all / --program are accepted and ignored: start.ps1 passes them to every setup)
#
# Inside the VM:  sudo mount -o ro /dev/disk/by-label/CASTLE /mnt && sudo bash /mnt/setup-penguin.sh --copy
# From the host:  .\start.ps1 -Profile penguin          (profiles/penguin.psd1)
#            or:  .\start.ps1 debian-x11 -Copy -Setup penguin -Background
#
# Works on Arch-based guests (EndeavourOS live ISO) and Debian-based ones (the
# debian-x11 cloud target); the distro differences live in castle-lib.sh.
# The bot reads the screen and sends keys through X11, so the desktop session
# must be X11. Firefox gets its own profile next to the project, without
# first-run pages. The bot signs in with PENGUIN_USERNAME / PENGUIN_PASSWORD
# from the project's .env.
set -uo pipefail

RUN=1
while [ $# -gt 0 ]; do
  case "$1" in
    --no-run)     RUN=0 ;;
    -p|--program) shift ;;
    -h|--help)    sed -n '5,16p' "$0"; exit 0 ;;
    *) ;;
  esac
  shift
done

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
PROJECT="$COPY_DST/penguin-autosurf"

# --- Step 1: unpack the project ----------------------------------------------
echo "[+] Step 1: Unpacking penguin-autosurf into $PROJECT..."
mkdir -p "$COPY_DST" "$PROJECT"
castle_need bsdtar libarchive libarchive-tools
castle_pkg_install "${CASTLE_NEED[@]}" || exit 1
CASTLE_NEED=()
is_project_zip() { bsdtar -tf "$1" 2>/dev/null | grep -qE '(^|/)games/cart_surfer/bot\.py$'; }
SRC=""
if castle_mount_label CASTLECOPY /mnt/castle-copy; then
  # An unpacked folder wins; otherwise the newest zip that holds the project.
  dir=$(find /mnt/castle-copy -maxdepth 3 -type f -path '*/games/cart_surfer/bot.py' 2>/dev/null | head -n 1)
  if [ -n "$dir" ]; then
    SRC="${dir%/games/cart_surfer/bot.py}"
    cp -rf "$SRC/." "$PROJECT/"
  else
    while IFS= read -r z; do
      if is_project_zip "$z"; then SRC="$z"; break; fi
    done < <(find /mnt/castle-copy -maxdepth 2 -type f -iname '*.zip' -printf '%T@ %p\n' 2>/dev/null | sort -rn | cut -d' ' -f2-)
    if [ -n "$SRC" ]; then
      # A zip made from the folder itself has one top-level directory: strip it.
      strip=0
      bsdtar -tf "$SRC" | grep -qE '^games/cart_surfer/bot\.py$' || strip=1
      # Host virtualenvs and recordings are dead weight here (the guest builds its own).
      bsdtar -xf "$SRC" -C "$PROJECT" --strip-components "$strip" \
        --exclude '.venv' --exclude '.venv-linux' --exclude '.git' --exclude '__pycache__' --exclude '*.mp4'
    fi
  fi
fi
if [ -z "$SRC" ]; then
  if [ -f "$PROJECT/main.py" ]; then
    echo "[i] No project in the payload; using the copy already in $PROJECT."
  else
    echo "[-] No penguin-autosurf project found. Put penguin-autosurf.zip (or the folder) in data/copy on the host."
    exit 1
  fi
else
  echo "[+] Unpacked from $SRC"
fi
# An earlier wine-style copy may have left the whole zip here; the project is unpacked now.
find "$COPY_DST/" -maxdepth 1 -type f -iname 'penguin-autosurf*.zip' -delete 2>/dev/null || true
# Files edited on Windows may carry CRLF; bash refuses a script that does.
sed -i 's/\r$//' "$PROJECT/start.sh"
chmod +x "$PROJECT/start.sh"
chown -R "$ACTUAL_USER:" "$(readlink -f "$PROJECT")"
[ -f "$PROJECT/.env" ] && chmod 600 "$PROJECT/.env"

# --- Step 2: system packages (only what is missing) ---------------------------
castle_profile_apply root || exit 1
castle_need python3 python python3
castle_need firefox firefox firefox-esr
castle_need xdotool xdotool xdotool
castle_need xset xorg-xset x11-xserver-utils
castle_need xrandr xorg-xrandr x11-xserver-utils
castle_need gcc gcc build-essential           # pynput's evdev builds when PyPI has no wheel
case "$(castle_pm)" in
  pacman)
    python3 -c 'import ensurepip' 2>/dev/null || CASTLE_NEED+=(python-pip)
    python3 -c 'import tkinter' 2>/dev/null || CASTLE_NEED+=(tk)           # the bot's on-screen status box
    [ -e /usr/include/linux/input.h ] || CASTLE_NEED+=(linux-api-headers)
    ldconfig -p | grep -q libXtst.so.6 || CASTLE_NEED+=(libxtst)
    ldconfig -p | grep -q libGL.so.1 || CASTLE_NEED+=(libglvnd) ;;
  apt)
    python3 -c 'import ensurepip' 2>/dev/null || CASTLE_NEED+=(python3-venv)
    python3 -c 'import tkinter' 2>/dev/null || CASTLE_NEED+=(python3-tk)
    [ -e "$(python3 -c 'import sysconfig; print(sysconfig.get_paths()["include"])' 2>/dev/null)/Python.h" ] || CASTLE_NEED+=(python3-dev)
    ldconfig -p | grep -q libXtst.so.6 || CASTLE_NEED+=(libxtst6)
    ldconfig -p | grep -q libGL.so.1 || CASTLE_NEED+=(libgl1) ;;
esac
if [ ${#CASTLE_NEED[@]} -gt 0 ]; then
  echo "[+] Step 2: Installing ${CASTLE_NEED[*]}..."
  castle_pkg_install "${CASTLE_NEED[@]}" || { echo "[-] Could not install: ${CASTLE_NEED[*]}"; exit 1; }
else
  echo "[+] Step 2: System packages already present."
fi
# Debian names the browser firefox-esr; the bot and its launcher say firefox.
if ! command -v firefox >/dev/null 2>&1 && command -v firefox-esr >/dev/null 2>&1; then
  ln -sf "$(command -v firefox-esr)" /usr/local/bin/firefox
fi

# --- Step 3: Python environment (start.sh builds .venv-linux) -----------------
echo "[+] Step 3: Python packages (first run downloads them)..."
castle_as_user bash -c "cd ${PROJECT@Q} && ./start.sh --help >/dev/null" || { echo "[-] Python dependency install failed."; exit 1; }
echo "[+] Python environment ready: $("$PROJECT/.venv-linux/bin/python" --version 2>&1)"

[ "$RUN" = 1 ] || { echo "[+] Done (--no-run). Start it with: cd $PROJECT && ./start.sh play --start"; exit 0; }

# --- Step 4: desktop session --------------------------------------------------
if ! castle_wait_x11; then
  echo "    Everything is installed. Log in to an X11 session, then: cd $PROJECT && ./start.sh play --start"
  exit 1
fi
echo "[+] Step 4: X11 session on $DISPLAY."
# KDE's shell resets the screen mode while it starts; let it come up first.
if command -v plasmashell >/dev/null 2>&1; then
  for i in $(seq 1 60); do pgrep -u "$ACTUAL_USER" -x plasmashell >/dev/null && break; sleep 2; done
fi
castle_ensure_screen
# The bot stops seeing the game behind a lock screen or a blanked display
# (it sits idle through its own logged-out breaks).
if command -v kwriteconfig6 >/dev/null 2>&1; then
  castle_on_x kwriteconfig6 --file kscreenlockerrc --group Daemon --key Autolock false 2>/dev/null || true
  castle_on_x kwriteconfig6 --file kscreenlockerrc --group Daemon --key LockOnResume false 2>/dev/null || true
  castle_on_x qdbus6 org.freedesktop.ScreenSaver /ScreenSaver configure >/dev/null 2>&1 || true
fi
castle_on_x xset s off -dpms 2>/dev/null || true
# The EndeavourOS live ISO's Welcome window would sit over the browser.
pkill -u "$ACTUAL_USER" -f eos-welcome 2>/dev/null || true
pkill -u "$ACTUAL_USER" -x yad 2>/dev/null || true
castle_profile_apply user

# --- Step 5: Firefox on the game ----------------------------------------------
SITE=$("$PROJECT/.venv-linux/bin/python" - "$PROJECT" <<'PY' 2>/dev/null || true
import sys
sys.path.insert(0, sys.argv[1])
from games.cart_surfer import settings
cfg = settings.load()
print(cfg["sites"][cfg["session"]["site"]])
PY
)
: "${SITE:=https://play.cpjourney.net}"
# Its own profile next to the project (it survives reboots with the disk).
FF_PROFILE="$COPY_DST/penguin-firefox"
mkdir -p "$FF_PROFILE"
cat > "$FF_PROFILE/user.js" <<'JS'
// Castle: written by setup-penguin.sh on every run.
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("startup.homepage_welcome_url", "");
user_pref("startup.homepage_welcome_url.additional", "");
user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);
user_pref("datareporting.policy.dataSubmissionEnabled", false);
user_pref("termsofuse.bypassNotification", true);
user_pref("browser.sessionstore.resume_from_crash", false);
user_pref("browser.startup.couldRestoreSession.count", 2);   // no "open previous tabs" bar after a hard power-off
user_pref("browser.tabs.closeWindowWithLastTab", false);     // the bot's Ctrl+W before a break keeps the window
user_pref("browser.startup.page", 0);
user_pref("browser.tabs.warnOnClose", false);
user_pref("browser.warnOnQuit", false);
user_pref("browser.translations.automaticallyPopup", false);
user_pref("signon.rememberSignons", false);
JS
chown -R "$ACTUAL_USER:" "$(readlink -f "$FF_PROFILE")"

firefox_window() { castle_on_x xdotool search --onlyvisible --class firefox 2>/dev/null | tail -n 1; }
# An earlier run's bot: ask it to stop (it lets go of its keys), then make sure
# of both. A key left pressed repeats into every window, the login form included.
if pkill -TERM -u "$ACTUAL_USER" -f 'main.py play' 2>/dev/null; then
  for i in 1 2 3 4 5 6; do pgrep -u "$ACTUAL_USER" -f 'main.py play' >/dev/null || break; sleep 1; done
  pkill -KILL -u "$ACTUAL_USER" -f 'main.py play' 2>/dev/null || true
fi
pkill -u "$ACTUAL_USER" -f 'xterm -hold -e bash -c BROWSER=' 2>/dev/null || true   # and its terminal
castle_on_x xdotool keyup space Left Right Up Down Return Control_L Alt_L Shift_L 2>/dev/null || true
if [ -n "$(firefox_window)" ] && pgrep -u "$ACTUAL_USER" -f "firefox.*$FF_PROFILE" >/dev/null; then
  echo "[+] Step 5: Firefox is already open on the game."
else
  echo "[+] Step 5: Opening Firefox on $SITE..."
  castle_in_session penguin-firefox "$USER_HOME" firefox --profile "$FF_PROFILE" --new-instance "$SITE"
fi
WIN=""
for i in $(seq 1 60); do WIN=$(firefox_window); [ -n "$WIN" ] && break; sleep 2; done
[ -n "$WIN" ] || { echo "[-] Firefox did not open a window."; exit 1; }
# Maximized, not fullscreen: in fullscreen the page stretches the game over the
# whole screen and the bot's status box has nowhere to sit but on the game,
# which hides the game from the bot. Maximized, the box covers the toolbar.
castle_ensure_screen
castle_on_x xdotool windowstate --remove FULLSCREEN "$WIN" 2>/dev/null || true
castle_on_x xdotool windowstate --add MAXIMIZED_VERT --add MAXIMIZED_HORZ "$WIN" 2>/dev/null \
  || castle_on_x xdotool windowmove "$WIN" 0 0 windowsize "$WIN" 100% 95% 2>/dev/null || true

# --- Step 6: the bot ----------------------------------------------------------
# In a terminal so its log is on screen; Firefox goes back in front of it,
# because the bot reads the game from the screen. --start: no F8 needed.
LOG="$PROJECT/logs/castle-run.log"
castle_on_x mkdir -p "$PROJECT/logs"
: > "$LOG"; chown "$ACTUAL_USER:" "$LOG"
# After a logged-out break the bot opens the site again: same profile, same window.
BROWSER_CMD="firefox --profile ${FF_PROFILE@Q} %s"
RUN_CMD="BROWSER=${BROWSER_CMD@Q} ./start.sh play --start 2>&1 | tee ${LOG@Q}"
echo "[+] Step 6: Starting the bot (log: $LOG)..."
if command -v konsole >/dev/null 2>&1; then
  castle_in_session penguin-bot "$PROJECT" konsole --hold -e bash -c "$RUN_CMD"
elif command -v xterm >/dev/null 2>&1; then
  castle_in_session penguin-bot "$PROJECT" xterm -hold -e bash -c "$RUN_CMD"
else
  castle_in_session penguin-bot "$PROJECT" bash -c "$RUN_CMD"
fi
sleep 4
castle_on_x xdotool windowactivate "$WIN" 2>/dev/null || true

# Report what it does until the first game starts (4 minutes at most), then leave it running.
for i in $(seq 1 48); do
  sleep 5
  grep -qE 'cart game [0-9]+ started|STOPPED|Error|Traceback' "$LOG" 2>/dev/null && break
done
sed 's/^/    /' "$LOG" 2>/dev/null | tail -n 25
if grep -qE 'STOPPED|Traceback|^Error' "$LOG" 2>/dev/null; then
  echo "[-] The bot stopped; see the log above."
  exit 1
fi
if ! grep -qE '^running \(game at' "$LOG" 2>/dev/null; then
  echo "[-] The bot has not found the game on screen yet (it keeps looking). Is the game page showing in Firefox?"
  exit 1
fi
if grep -qE 'cart game [0-9]+ started' "$LOG" 2>/dev/null; then
  echo "[+] The bot is playing Cart Surfer in the VM. F8 (or Ctrl+Alt+S) pauses, F9 (or Ctrl+Alt+Q) quits."
else
  echo "[+] The bot is running in the VM and on its way to the Mine. F8 (or Ctrl+Alt+S) pauses, F9 (or Ctrl+Alt+Q) quits."
fi
echo "    Again later: cd $PROJECT && ./start.sh play --start"
exit 0
