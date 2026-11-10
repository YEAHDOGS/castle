#!/usr/bin/env bash
#===============================================================================
# start.sh -- starts Castle on this computer: the box with its web app, then the
# desktop app. Works on Linux and on Windows.
#
#   Linux, Git Bash or WSL:           ./start.sh
#   PowerShell or Command Prompt:     .\start.cmd   (finds Git Bash and runs this file)
#
#   1. Builds the box daemon (castled), the web app and the desktop app. Each is built
#      when it is missing or older than its source, so a second run starts at once.
#   2. Starts the box in the background, serving the web app at
#      http://127.0.0.1:<port>.
#   3. Opens the desktop app, which connects to that same box.
#
# Needs Rust (cargo) and Node.js (npm). Nothing here needs administrator rights.
#
# The box keeps running after the desktop app is closed, so the web app stays up.
#
# The vault, the games folder, the clonewars folder and the port come from the
# desktop app's box.json, so the web app and the desktop app show the same box:
#   Windows  %APPDATA%\net.wearedogs.castle\box.json
#   Linux    ~/.config/net.wearedogs.castle/box.json
#
# Usage:
#   ./start.sh              start everything
#   ./start.sh --build      rebuild everything first, even if it looks current
#   ./start.sh --browser    also open the web app in the default browser
#   ./start.sh --no-app     web service only
#   ./start.sh --stop       stop the box this script started
#   ./start.sh --data DIR --port N    override box.json for this run
#   ./start.sh --linux      inside WSL: run a Linux box there instead of handing over to Windows
#===============================================================================
set -euo pipefail

DEFAULT_PORT=8787
START_TIMEOUT_SECONDS=20
PROBE_SECONDS=0.25
APP_ID="net.wearedogs.castle"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# WSL is Linux inside Windows: it has neither the Windows Rust toolchain nor the Windows
# desktop app. From there, hand the whole run to Windows. --linux keeps it in WSL.
if [[ "${1:-}" == --linux ]]; then
    shift
elif grep -qi microsoft /proc/version 2>/dev/null && command -v cmd.exe >/dev/null 2>&1; then
    cd /mnt/c 2>/dev/null || true # cmd.exe cannot start in a Linux-only folder
    exec cmd.exe /c "$(wslpath -w "$ROOT/start.cmd")" "$@"
fi

DAEMON_DIR="$ROOT/castled"
CLIENT_DIR="$ROOT/client"
WEB_DIR="$CLIENT_DIR/dist"

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) ON_WINDOWS=1 ;;
    *)                    ON_WINDOWS=0 ;;
esac

if (( ON_WINDOWS )); then
    EXE=".exe"
    CONFIG_DIR="$(cygpath -u "$APPDATA")/$APP_ID"
else
    EXE=""
    CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/$APP_ID"
fi
DAEMON_EXE="$DAEMON_DIR/target/release/castled$EXE"
APP_EXE="$CLIENT_DIR/src-tauri/target/release/castle$EXE"
CONFIG_FILE="$CONFIG_DIR/box.json"
PID_FILE="$CONFIG_DIR/start.pid"
LOG_FILE="$CONFIG_DIR/start-box.log"
if (( ON_WINDOWS )); then
    NOT_STARTED="The box did not start. Run it by hand to see why: $DAEMON_EXE --help"
else
    NOT_STARTED="The box did not start. See $LOG_FILE.err"
fi

say() { printf '[castle] %s\n' "$*"; }
die() { printf '[castle] %s\n' "$*" >&2; exit 1; }

# A path the box (a native program) understands: Windows form on Windows.
native() { if (( ON_WINDOWS )); then cygpath -w "$1"; else printf '%s' "$1"; fi; }

# True when the built file is missing or any of the given sources is newer than it.
is_stale() {
    local built="$1"; shift
    [[ -f "$built" ]] || return 0
    [[ -n "$(find "$@" -type f -newer "$built" -print -quit 2>/dev/null)" ]]
}

# The first Games folder with Consoles and Roms inside, on any drive or mounted disk.
find_games_folder() {
    local dir
    if (( ON_WINDOWS )); then
        for dir in /{c..z}/Games; do
            if [[ -d "$dir/Consoles" && -d "$dir/Roms" ]]; then printf '%s' "$dir"; return 0; fi
        done
    else
        for dir in /media/*/Games /media/*/*/Games /run/media/*/*/Games /mnt/*/Games; do
            if [[ -d "$dir/Consoles" && -d "$dir/Roms" ]]; then printf '%s' "$dir"; return 0; fi
        done
    fi
    return 0
}

# True in a terminal that was opened "as administrator" (Windows) or as root (Linux).
is_admin() {
    if (( ON_WINDOWS )); then net session >/dev/null 2>&1; else [[ "$(id -u)" == 0 ]]; fi
}

app_is_open() {
    if (( ON_WINDOWS )); then
        tasklist //FI "IMAGENAME eq castle.exe" //NH 2>/dev/null | grep -qi '^castle\.exe'
    else
        pgrep -x castle >/dev/null 2>&1
    fi
}

is_listening() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

# One value from box.json, or nothing. Needs node; without it the defaults apply.
config_value() {
    [[ -f "$CONFIG_FILE" ]] && command -v node >/dev/null 2>&1 || return 0
    node -e '
        const fs = require("fs");
        try {
            const value = JSON.parse(fs.readFileSync(process.argv[1], "utf8"))[process.argv[2]];
            if (value !== undefined && value !== null) process.stdout.write(String(value));
        } catch { /* unreadable config: the defaults apply */ }
    ' "$(native "$CONFIG_FILE")" "$1"
}

is_castled() {
    local pid="$1"
    if (( ON_WINDOWS )); then
        tasklist //FI "PID eq $pid" //NH 2>/dev/null | grep -qi '^castled'
    else
        [[ "$(cat "/proc/$pid/comm" 2>/dev/null)" == castled ]]
    fi
}

# Stops a box. If Windows refuses, the box was started from an administrator terminal
# and only one of those can stop it: say so instead of failing without a reason.
stop_pid() {
    if (( ON_WINDOWS )); then
        taskkill //PID "$1" //F >/dev/null 2>&1 && return 0
        if is_admin; then die "Could not stop the box (process $1)."; fi
        die "Windows refused to stop the box (process $1). It was started from an administrator terminal, so stop it from one: right-click your terminal, Run as administrator, then run this again."
    else
        kill "$1" 2>/dev/null && return 0
        die "Could not stop the box (process $1). It belongs to another account; try again with sudo."
    fi
}

# Sets STARTED_PID and STARTED_PORT when the box this script started is still running.
STARTED_PID=""
STARTED_PORT=""
read_started() {
    STARTED_PID=""; STARTED_PORT=""
    [[ -f "$PID_FILE" ]] || return 0
    local pid port
    read -r pid port < "$PID_FILE" || true
    if [[ "$pid" =~ ^[0-9]+$ ]] && is_castled "$pid"; then
        STARTED_PID="$pid"; STARTED_PORT="$port"
    else
        rm -f "$PID_FILE"
    fi
}

# Starts the box detached from this terminal and prints its process id.
launch_box() {
    if (( ON_WINDOWS )); then
        # Start-Process gives a real Windows process with no console window. It must not
        # redirect the box's output: a redirected child inherits this script's own output
        # and whatever is reading it would wait for the box to exit. So no log on Windows.
        local quoted="" arg
        for arg in "$@"; do quoted+="'\"${arg//\'/\'\'}\"',"; done
        powershell -NoProfile -NonInteractive -Command "
            (Start-Process -FilePath '$(native "$DAEMON_EXE")' -ArgumentList ${quoted%,} -WindowStyle Hidden -PassThru).Id" | tr -d '\r'
    else
        setsid nohup "$DAEMON_EXE" "$@" > "$LOG_FILE" 2> "$LOG_FILE.err" < /dev/null &
        echo $!
    fi
}

open_url() {
    if (( ON_WINDOWS )); then cmd //c start "" "$1"; else xdg-open "$1" >/dev/null 2>&1 & fi
}

# ---- options ----
BUILD=0; BROWSER=0; NO_APP=0; STOP=0; DATA=""; PORT=""
while (( $# > 0 )); do
    case "$1" in
        --build)   BUILD=1; shift ;;
        --browser) BROWSER=1; shift ;;
        --no-app)  NO_APP=1; shift ;;
        --stop)    STOP=1; shift ;;
        --data)    DATA="${2:?--data needs a folder}"; shift 2 ;;
        --port)    PORT="${2:?--port needs a number}"; shift 2 ;;
        -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
        *) die "Unknown option '$1' (see --help)" ;;
    esac
done

mkdir -p "$CONFIG_DIR"

if (( STOP )); then
    read_started
    [[ -n "$STARTED_PID" ]] || { say 'No box started by this script is running.'; exit 0; }
    stop_pid "$STARTED_PID"
    rm -f "$PID_FILE"
    say "Stopped the box (process $STARTED_PID)."
    exit 0
fi

# ---- settings shared with the desktop app ----
[[ -n "$PORT" ]] || PORT="$(config_value port)"
[[ -n "$PORT" ]] || PORT="$DEFAULT_PORT"
[[ "$PORT" =~ ^[0-9]+$ ]] || die "The port must be a number, not '$PORT'."
[[ -n "$DATA" ]] || DATA="$(config_value data)"
[[ -n "$DATA" ]] || DATA="$HOME/Castle"
# Everything is on unless there is nothing to turn on: Clones uses the clonewars folder in
# this checkout, and Arcade uses the first Games folder (Consoles + Roms) found on a drive.
CLONEWARS="$(config_value clonewars)"
[[ -n "$CLONEWARS" ]] || CLONEWARS="$ROOT/clonewars"
ARCADE="$(config_value arcade)"
[[ -n "$ARCADE" ]] || ARCADE="$(find_games_folder)"
URL="http://127.0.0.1:$PORT"

# ---- build what is missing or out of date ----
need_cargo() { command -v cargo >/dev/null 2>&1 || die 'Building Castle needs Rust (cargo). Install it from https://rustup.rs'; }
need_npm() { command -v npm >/dev/null 2>&1 || die 'Building Castle needs Node.js (npm). Install it from https://nodejs.org'; }
install_packages() { [[ -d "$CLIENT_DIR/node_modules" ]] || (cd "$CLIENT_DIR" && npm ci --no-audit --no-fund); }

if is_admin; then
    say 'Note: this terminal has administrator rights. Castle does not need them, and a box started here can only be stopped from an administrator terminal.'
fi

read_started
DAEMON_SOURCES=("$DAEMON_DIR/src" "$DAEMON_DIR/Cargo.toml" "$ROOT/security/audits")
WEB_SOURCES=("$CLIENT_DIR/src" "$CLIENT_DIR/index.html" "$CLIENT_DIR/package.json" "$CLIENT_DIR/public")
APP_SOURCES=("${WEB_SOURCES[@]}" "$CLIENT_DIR/src-tauri/src" "$CLIENT_DIR/src-tauri/Cargo.toml" "$CLIENT_DIR/src-tauri/tauri.conf.json")

if (( BUILD )) || is_stale "$DAEMON_EXE" "${DAEMON_SOURCES[@]}"; then
    need_cargo
    # Windows will not overwrite a program that is running, so the old box stops first.
    if [[ -n "$STARTED_PID" ]]; then
        say 'Stopping the running box so it can be rebuilt.'
        stop_pid "$STARTED_PID"; rm -f "$PID_FILE"; STARTED_PID=""
        sleep 0.5
    fi
    say 'Building the box daemon (the first build takes a few minutes).'
    (cd "$DAEMON_DIR" && cargo build --release) || die 'The box daemon did not build.'
    touch "$DAEMON_EXE" # cargo leaves the file alone when nothing changed; mark it checked
fi
if (( BUILD )) || is_stale "$WEB_DIR/index.html" "${WEB_SOURCES[@]}"; then
    need_npm
    say 'Building the web app.'
    install_packages
    (cd "$CLIENT_DIR" && npm run build) || die 'The web app did not build.'
fi
if (( ! NO_APP )) && { (( BUILD )) || is_stale "$APP_EXE" "${APP_SOURCES[@]}"; }; then
    if app_is_open; then
        say 'The desktop app is open, so it was not rebuilt. Close it and run this again to get the newest one.'
    else
        need_cargo; need_npm
        say 'Building the desktop app (the first build takes several minutes).'
        install_packages
        # The app bundles the daemon; daemon.js puts the one just built where the build looks for it.
        (cd "$CLIENT_DIR" && node scripts/daemon.js && npx tauri build --no-bundle) || die 'The desktop app did not build.'
        [[ -f "$APP_EXE" ]] || die "The desktop app built, but $APP_EXE is missing."
        touch "$APP_EXE"
    fi
fi

# ---- the box ----
if [[ -n "$STARTED_PID" && "$STARTED_PORT" == "$PORT" ]]; then
    say "The box is already running (process $STARTED_PID)."
elif is_listening "$PORT"; then
    die "Something else is already listening on port $PORT. If the Castle desktop app is open, close it and run this again: on its own it starts a box without the web app."
else
    mkdir -p "$DATA"
    box_args=(--data "$(native "$DATA")" --bind "127.0.0.1:$PORT" --web "$(native "$WEB_DIR")")
    # A folder that is not there right now (an unplugged drive) is left out, so the box still starts.
    if [[ -n "$CLONEWARS" && -d "$CLONEWARS" ]]; then box_args+=(--clonewars "$(native "$CLONEWARS")"); else say 'Clones: off (no clonewars folder).'; fi
    if [[ -n "$ARCADE" && -d "$ARCADE" ]]; then
        box_args+=(--arcade "$(native "$ARCADE")")
    else
        say 'Arcade: off (no Games folder found on any drive; plug it in and press Find my games in the app).'
    fi
    box_pid="$(launch_box "${box_args[@]}")"
    [[ "$box_pid" =~ ^[0-9]+$ ]] || die "$NOT_STARTED"
    waited=0
    until is_listening "$PORT"; do
        if ! is_castled "$box_pid" || (( waited >= START_TIMEOUT_SECONDS * 4 )); then
            die "$NOT_STARTED"
        fi
        sleep "$PROBE_SECONDS"
        waited=$(( waited + 1 ))
    done
    printf '%s %s\n' "$box_pid" "$PORT" > "$PID_FILE"
    say "Box started (process $box_pid). Vault: $(native "$DATA")"
fi

TOKEN="$(tr -d '[:space:]' < "$DATA/.castle/token")"
LINK="$URL/#connect=$TOKEN"
say "Web app: $LINK"
say '         (the part after # is the key to the box; do not share the link)'
if (( BROWSER )); then open_url "$LINK"; fi

# ---- the desktop app ----
if (( ! NO_APP )); then
    if app_is_open; then
        say 'The desktop app is already open.'
    else
        if (( ON_WINDOWS )); then
            cmd //c start "" "$(native "$APP_EXE")"
        else
            setsid nohup "$APP_EXE" >/dev/null 2>&1 < /dev/null &
        fi
        say 'Desktop app opened.'
    fi
fi
say 'The box keeps running in the background. Stop it with:  ./start.sh --stop'
