#!/usr/bin/env bash
#===============================================================================
# start.sh -- starts Castle on this computer: the box with its web app, then the
# desktop app. Works on Linux and on Windows.
#
#   Linux, Git Bash or WSL:           ./start.sh
#   PowerShell or Command Prompt:     .\start.cmd   (finds Git Bash and runs this file)
#
#   1. Builds the box daemon (castled) and the web app if they are not built yet.
#   2. Starts the box in the background, serving the web app at
#      http://127.0.0.1:<port>.
#   3. Opens the desktop app, which connects to that same box.
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
#   ./start.sh --build      rebuild the daemon and the web app first
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
    APP_CANDIDATES=(
        "$CLIENT_DIR/src-tauri/target/release/castle.exe"
        "$(cygpath -u "$LOCALAPPDATA")/Castle/castle.exe"
    )
else
    EXE=""
    CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/$APP_ID"
    APP_CANDIDATES=(
        "$CLIENT_DIR/src-tauri/target/release/castle"
        "/usr/bin/castle"
    )
fi
DAEMON_EXE="$DAEMON_DIR/target/release/castled$EXE"
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

stop_pid() {
    if (( ON_WINDOWS )); then taskkill //PID "$1" //F >/dev/null; else kill "$1"; fi
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
        -h|--help) sed -n '2,29p' "$0"; exit 0 ;;
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
ARCADE="$(config_value arcade)"
CLONEWARS="$(config_value clonewars)"
URL="http://127.0.0.1:$PORT"

# ---- build what is missing ----
if (( BUILD )) || [[ ! -f "$DAEMON_EXE" ]]; then
    command -v cargo >/dev/null 2>&1 || die 'Building the box needs Rust (cargo). Install it from https://rustup.rs'
    say 'Building the box daemon (the first build takes a few minutes).'
    (cd "$DAEMON_DIR" && cargo build --release)
fi
if (( BUILD )) || [[ ! -f "$WEB_DIR/index.html" ]]; then
    command -v npm >/dev/null 2>&1 || die 'Building the web app needs Node.js (npm).'
    say 'Building the web app.'
    (cd "$CLIENT_DIR" && { [[ -d node_modules ]] || npm ci --no-audit --no-fund; } && npm run build)
fi

# ---- the box ----
read_started
if [[ -n "$STARTED_PID" ]] && (( BUILD )); then
    stop_pid "$STARTED_PID"; rm -f "$PID_FILE"; STARTED_PID=""
    sleep 0.5
fi
if [[ -n "$STARTED_PID" && "$STARTED_PORT" == "$PORT" ]]; then
    say "The box is already running (process $STARTED_PID)."
elif is_listening "$PORT"; then
    die "Something else is already listening on port $PORT. If the Castle desktop app is open, close it and run this again: on its own it starts a box without the web app."
else
    mkdir -p "$DATA"
    box_args=(--data "$(native "$DATA")" --bind "127.0.0.1:$PORT" --web "$(native "$WEB_DIR")")
    # A folder that is not there right now (an unplugged drive) is left out, so the box still starts.
    [[ -n "$ARCADE" && -d "$ARCADE" ]] && box_args+=(--arcade "$ARCADE")
    [[ -n "$CLONEWARS" && -d "$CLONEWARS" ]] && box_args+=(--clonewars "$CLONEWARS")
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
    say "Box started (process $box_pid). Vault: $DATA"
fi

TOKEN="$(tr -d '[:space:]' < "$DATA/.castle/token")"
LINK="$URL/#connect=$TOKEN"
say "Web app: $LINK"
say '         (the part after # is the key to the box; do not share the link)'
if (( BROWSER )); then open_url "$LINK"; fi

# ---- the desktop app ----
if (( ! NO_APP )); then
    app=""
    for candidate in "${APP_CANDIDATES[@]}"; do
        if [[ -x "$candidate" || -f "$candidate" ]]; then app="$candidate"; break; fi
    done
    if [[ -z "$app" ]]; then
        say 'No desktop app found. Build one with:  cd client && npm run desktop'
    elif (( ON_WINDOWS )) && tasklist //FI "IMAGENAME eq castle.exe" //NH 2>/dev/null | grep -qi '^castle\.exe'; then
        say 'The desktop app is already open.'
    elif (( ! ON_WINDOWS )) && pgrep -x castle >/dev/null 2>&1; then
        say 'The desktop app is already open.'
    else
        if (( ON_WINDOWS )); then
            cmd //c start "" "$(native "$app")"
        else
            setsid nohup "$app" >/dev/null 2>&1 < /dev/null &
        fi
        say "Desktop app opened ($app)."
    fi
fi
say 'The box keeps running in the background. Stop it with:  ./start.sh --stop'
