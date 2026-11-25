#!/usr/bin/env bash
#===============================================================================
# start.sh -- starts all of Castle on this computer, from source, with everything on.
# Works on Linux and on Windows.
#
#   Linux, Git Bash or WSL:           ./start.sh
#   PowerShell or Command Prompt:     .\start.cmd   (finds a bash and runs this file)
#
# By default it runs LIVE, for working on Castle. It starts the box, opens the desktop
# app, and keeps both following your edits until you press Ctrl+C:
#   interface (app/src)         changes appear in the open app and browser at once
#   app shell (app/src-tauri)   the app rebuilds and reopens itself
#   box daemon (castled/src)           the box rebuilds and restarts within a few seconds
# The same live interface is in the browser at http://localhost:1420.
#
# With --release it instead builds everything once, starts it and returns: the box in
# the background serving the built web app at http://127.0.0.1:<port>, and the built
# desktop app. Nothing follows your edits in that mode.
#
# Closing the desktop app stops the box and everything it runs (for now). Without the app,
# Ctrl+C ends live mode and stops it too, and ./start.sh --stop stops a --release box.
# Clones uses the clonewars folder in this checkout and Arcade the first Games folder
# found on a drive. The vault, the port and
# those two folders can be set in the desktop app's box.json:
#   Windows  %APPDATA%\net.wearedogs.castle\box.json
#   Linux    ~/.config/net.wearedogs.castle/box.json
#
# Needs Rust (cargo) and Node.js (npm). Nothing here needs administrator rights.
#
# Usage:
#   ./start.sh              live: box, desktop app and browser interface, following edits
#   ./start.sh --no-app     live, without the desktop app (box and browser interface)
#   ./start.sh --browser    also open the interface in the default browser
#   ./start.sh --release    build everything once, start it, and return
#   ./start.sh --build      rebuild first, even if it looks current
#   ./start.sh --stop       stop the box and everything it runs
#   ./start.sh --data DIR --port N    override box.json for this run
#   ./start.sh --linux      inside WSL: stay in Linux instead of handing over to Windows
#===============================================================================
set -euo pipefail

DEFAULT_PORT=8787
START_TIMEOUT_SECONDS=20
STOP_TIMEOUT_SECONDS=60 # the box gives its VMs 20 s and its containers 20 s to stop
DEV_PORT=1420       # where the live interface is served (app/vite.config.js)
WATCH_SECONDS=2     # how often the box daemon's source is checked for changes
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
CLIENT_DIR="$ROOT/app"
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

# True when something on this computer listens on the port, over IPv4 or IPv6 (the live
# interface's server listens on IPv6 only on some machines).
is_listening() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null || (exec 3<>"/dev/tcp/::1/$1") 2>/dev/null; }

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

# Process ids of the programs called $1 (castle.exe, castle) that run from inside this
# checkout, so what an earlier ./start.sh left behind can be told apart from anything else
# (an installed Castle app, say).
our_programs() {
    if (( ON_WINDOWS )); then
        powershell -NoProfile -NonInteractive -Command "
            \$root = '$(native "$ROOT")'
            Get-CimInstance Win32_Process -Filter \"Name='$1'\" |
                Where-Object { \$_.ExecutablePath -and \$_.ExecutablePath.StartsWith(\$root, [StringComparison]::OrdinalIgnoreCase) } |
                ForEach-Object { \$_.ProcessId }" | tr -d '\r'
    else
        local pid
        for pid in $(pgrep -x "$1" 2>/dev/null); do
            [[ "$(readlink -f "/proc/$pid/exe" 2>/dev/null)" == "$ROOT"/* ]] && echo "$pid"
        done
    fi
    return 0
}

# The process id listening on a port, when it was started from this checkout (its program
# or its command line is inside it). Nothing for anything else.
our_listener() {
    if (( ON_WINDOWS )); then
        powershell -NoProfile -NonInteractive -Command "
            \$root = '$(native "$ROOT")'
            \$c = Get-NetTCPConnection -State Listen -LocalPort $1 -ErrorAction SilentlyContinue | Select-Object -First 1
            if (\$c) {
                \$p = Get-CimInstance Win32_Process -Filter \"ProcessId=\$(\$c.OwningProcess)\"
                if (\$p -and (\"\$(\$p.ExecutablePath)\".StartsWith(\$root, [StringComparison]::OrdinalIgnoreCase) -or
                              \"\$(\$p.CommandLine)\".IndexOf(\$root, [StringComparison]::OrdinalIgnoreCase) -ge 0)) { \$p.ProcessId }
            }" | tr -d '\r'
    else
        local pid
        pid="$(ss -ltnpH "sport = :$1" 2>/dev/null | grep -o 'pid=[0-9]*' | head -n 1 | cut -d= -f2)"
        [[ -n "$pid" ]] || return 0
        if [[ "$(readlink -f "/proc/$pid/exe" 2>/dev/null)" == "$ROOT"/* ]] || tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | grep -qF "$ROOT"; then
            echo "$pid"
        fi
    fi
    return 0
}

# Ends a process and everything it started.
stop_tree() {
    if (( ON_WINDOWS )); then
        taskkill //PID "$1" //T //F >/dev/null 2>&1 || true
    else
        pkill -TERM -P "$1" 2>/dev/null || true
        kill "$1" 2>/dev/null || true
    fi
}

# Sets STARTED_PID and STARTED_PORT when the box this script started is still running.
# A box from this checkout that listens on the port without a record (the record was lost,
# or it was started by hand or by the dev app) is taken over, so it is stopped and
# restarted like one started here rather than reported as "something else".
STARTED_PID=""
STARTED_PORT=""
read_started() {
    STARTED_PID=""; STARTED_PORT=""
    if [[ -f "$PID_FILE" ]]; then
        local pid port
        read -r pid port < "$PID_FILE" || true
        if [[ "$pid" =~ ^[0-9]+$ ]] && is_castled "$pid"; then
            STARTED_PID="$pid"; STARTED_PORT="$port"
            return 0
        fi
        rm -f "$PID_FILE"
    fi
    local found
    found="$(our_listener "$PORT")"
    if [[ "$found" =~ ^[0-9]+$ ]] && is_castled "$found"; then
        STARTED_PID="$found"; STARTED_PORT="$PORT"
        printf '%s %s\n' "$found" "$PORT" > "$PID_FILE"
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
BUILD=0; BROWSER=0; NO_APP=0; STOP=0; RELEASE=0; DATA=""; PORT=""
while (( $# > 0 )); do
    case "$1" in
        --release) RELEASE=1; shift ;;
        --build)   BUILD=1; shift ;;
        --browser) BROWSER=1; shift ;;
        --no-app)  NO_APP=1; shift ;;
        --stop)    STOP=1; shift ;;
        --data)    DATA="${2:?--data needs a folder}"; shift 2 ;;
        --port)    PORT="${2:?--port needs a number}"; shift 2 ;;
        -h|--help) sed -n '2,39p' "$0"; exit 0 ;;
        *) die "Unknown option '$1' (see --help)" ;;
    esac
done

mkdir -p "$CONFIG_DIR"

# ---- settings shared with the desktop app ----
[[ -n "$PORT" ]] || PORT="$(config_value port)"
[[ -n "$PORT" ]] || PORT="$DEFAULT_PORT"
[[ "$PORT" =~ ^[0-9]+$ ]] || die "The port must be a number, not '$PORT'."
[[ -n "$DATA" ]] || DATA="$(config_value data)"
[[ -n "$DATA" ]] || DATA="$HOME/Castle"
# Everything is on unless there is nothing to turn on: Clones uses the clonewars folder in
# this checkout, and Arcade uses the first Games folder (Consoles + Roms) found on a drive.
CLONEWARS="$(config_value clonewars)"
[[ -n "$CLONEWARS" ]] || CLONEWARS="$ROOT/app/clonewars"
ARCADE_SET="$(config_value arcade)" # looked for afresh each time the box starts when unset
URL="http://127.0.0.1:$PORT"
DEV_URL="http://localhost:$DEV_PORT"

# Asks the box to stop (POST /api/v1/box/stop). It stops what it started first (Clones'
# machines, DOGS AI and ComfyUI), then itself; this waits for that. A box from this
# checkout that does not answer, or does not stop in time, is ended.
stop_box_and_all() {
    read_started
    local token pid="$STARTED_PID" waited=0
    token="$(tr -d '[:space:]' < "$DATA/.castle/token" 2>/dev/null || true)"
    if [[ -n "$token" ]] && is_listening "$PORT"; then
        say 'Stopping the box and everything it runs (machines, DOGS AI).'
        if (( ON_WINDOWS )); then
            powershell -NoProfile -NonInteractive -Command "Invoke-RestMethod -Method Post -Uri '$URL/api/v1/box/stop' -Headers @{ Authorization = 'Bearer $token' } -TimeoutSec 10 | Out-Null" >/dev/null 2>&1 || true
        else
            curl -fsS -m 10 -X POST -H "Authorization: Bearer $token" "$URL/api/v1/box/stop" >/dev/null 2>&1 || true
        fi
    fi
    # The box stops listening first and stops its machines after, so wait for the process.
    while { is_listening "$PORT" || { [[ -n "$pid" ]] && is_castled "$pid"; }; } && (( waited < STOP_TIMEOUT_SECONDS * 4 )); do
        sleep 0.25
        waited=$(( waited + 1 ))
    done
    if [[ -n "$pid" ]] && is_castled "$pid"; then stop_pid "$pid"; fi
    rm -f "$PID_FILE"
}

if (( STOP )); then
    read_started
    if [[ -z "$STARTED_PID" ]] && ! is_listening "$PORT"; then say 'No box is running.'; exit 0; fi
    stop_box_and_all
    say 'Stopped the box and everything it ran.'
    exit 0
fi

need_cargo() { command -v cargo >/dev/null 2>&1 || die 'Building Castle needs Rust (cargo). Install it from https://rustup.rs'; }
need_npm() { command -v npm >/dev/null 2>&1 || die 'Building Castle needs Node.js (npm). Install it from https://nodejs.org'; }
install_packages() { [[ -d "$CLIENT_DIR/node_modules" ]] || (cd "$CLIENT_DIR" && npm ci --no-audit --no-fund); }

# DOGS AI's app (app/ai/app), which the box shows in its AI section through the dogs server.
# app/ai is the dogs-ai project as a git submodule: fetched here when the checkout does not
# have it yet. Built once when it is missing (or with --build). A failure leaves AI off;
# Castle still starts.
AI_APP_DIR="$ROOT/app/ai/app"
build_ai_app() {
    if [[ ! -f "$ROOT/app/ai/server/bin/dogs.mjs" ]]; then
        say 'Fetching DOGS AI (app/ai, the dogs-ai project).'
        git -C "$ROOT" submodule update --init app/ai || { say 'DOGS AI could not be fetched, so AI stays off.'; return 0; }
    fi
    [[ -f "$AI_APP_DIR/package.json" ]] || return 0
    if [[ -f "$AI_APP_DIR/dist/index.html" ]] && (( ! BUILD )); then return 0; fi
    say 'Building DOGS AI (app/ai/app).'
    if [[ ! -d "$AI_APP_DIR/node_modules" ]]; then
        (cd "$AI_APP_DIR" && npm ci --no-audit --no-fund) || { say 'DOGS AI did not install, so AI stays off.'; return 0; }
    fi
    (cd "$AI_APP_DIR" && npm run build) || say 'DOGS AI did not build, so AI stays off.'
}

# Phoenix (app/phoenix), which the box runs for its Phoenix section: the cleaner and the
# set-up of a fresh Windows PC. It is the phoenix project as a git submodule, fetched here
# when the checkout does not have it yet. Only Windows can run it (its engine is Windows
# PowerShell). A failure leaves the section saying so; Castle still starts.
fetch_phoenix() {
    (( ON_WINDOWS )) || return 0
    [[ -f "$ROOT/app/phoenix/scripts/engine/phoenix.ps1" ]] && return 0
    say 'Fetching Phoenix (app/phoenix, the phoenix project).'
    git -C "$ROOT" submodule update --init app/phoenix || say 'Phoenix could not be fetched, so its section stays off.'
}

DAEMON_SOURCES=("$DAEMON_DIR/src" "$DAEMON_DIR/Cargo.toml" "$DAEMON_DIR/audits")
WEB_SOURCES=("$CLIENT_DIR/src" "$CLIENT_DIR/index.html" "$CLIENT_DIR/package.json" "$CLIENT_DIR/public")
APP_SOURCES=("${WEB_SOURCES[@]}" "$CLIENT_DIR/src-tauri/src" "$CLIENT_DIR/src-tauri/Cargo.toml" "$CLIENT_DIR/src-tauri/tauri.conf.json")

# Stops the box this script started, if one is running.
stop_started_box() {
    read_started
    [[ -n "$STARTED_PID" ]] || return 0
    stop_pid "$STARTED_PID"; rm -f "$PID_FILE"; STARTED_PID=""
    sleep 0.5
}

# Builds the daemon. Windows will not overwrite a program that is running, so the box
# stops first. Returns 1 if the build fails; the old program is then still in place.
build_daemon() {
    stop_started_box
    if (cd "$DAEMON_DIR" && cargo build --release); then
        touch "$DAEMON_EXE" # cargo leaves the file alone when nothing changed; mark it checked
        return 0
    fi
    # Mark the old program checked too, so a broken edit is not rebuilt every two seconds.
    [[ -f "$DAEMON_EXE" ]] && touch "$DAEMON_EXE"
    return 1
}

# Starts the box with everything on, unless it is already running. Returns 1 on failure.
start_box() {
    read_started
    if [[ -n "$STARTED_PID" && "$STARTED_PORT" == "$PORT" ]]; then
        say "The box is already running (process $STARTED_PID)."
        return 0
    fi
    if is_listening "$PORT"; then
        say "Something else is already listening on port $PORT. If a Castle desktop app is open, close it and run this again."
        return 1
    fi
    mkdir -p "$DATA"
    local box_args=(--data "$(native "$DATA")" --bind "127.0.0.1:$PORT")
    [[ -f "$WEB_DIR/index.html" ]] && box_args+=(--web "$(native "$WEB_DIR")")
    # A folder that is not there right now (an unplugged drive) is left out, so the box still starts.
    if [[ -n "$CLONEWARS" && -d "$CLONEWARS" ]]; then box_args+=(--clonewars "$(native "$CLONEWARS")"); else say 'Clones: off (no clonewars folder).'; fi
    # A drive plugged in since the last start is picked up when the box restarts.
    local ARCADE="$ARCADE_SET"
    [[ -n "$ARCADE" ]] || ARCADE="$(find_games_folder)"
    if [[ -n "$ARCADE" && -d "$ARCADE" ]]; then
        box_args+=(--arcade "$(native "$ARCADE")")
    else
        say 'Arcade: off (no Games folder found on any drive; plug it in and press Find my games in the app).'
    fi
    local box_pid waited=0
    box_pid="$(launch_box "${box_args[@]}")"
    [[ "$box_pid" =~ ^[0-9]+$ ]] || { say "$NOT_STARTED"; return 1; }
    until is_listening "$PORT"; do
        if ! is_castled "$box_pid" || (( waited >= START_TIMEOUT_SECONDS * 4 )); then
            say "$NOT_STARTED"
            return 1
        fi
        sleep "$PROBE_SECONDS"
        waited=$(( waited + 1 ))
    done
    printf '%s %s\n' "$box_pid" "$PORT" > "$PID_FILE"
    say "Box started (process $box_pid). Vault: $(native "$DATA")"
}

if is_admin; then
    say 'Note: this terminal has administrator rights. Castle does not need them, and a box started here can only be stopped from an administrator terminal.'
fi

# ---- the box daemon: needed either way ----
if (( BUILD )) || is_stale "$DAEMON_EXE" "${DAEMON_SOURCES[@]}"; then
    need_cargo
    say 'Building the box daemon (the first build takes a few minutes).'
    build_daemon || die 'The box daemon did not build.'
fi

#===============================================================================
# Built mode (--release): build everything once, start it, and return.
#===============================================================================
if (( RELEASE )); then
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

    build_ai_app

    fetch_phoenix
    start_box || exit 1
    TOKEN="$(tr -d '[:space:]' < "$DATA/.castle/token")"
    LINK="$URL/#connect=$TOKEN"
    say "Web app: $LINK"
    say '         (the part after # is the key to the box; do not share the link)'
    if (( BROWSER )); then open_url "$LINK"; fi

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
    say 'Closing the desktop app stops the box and everything it runs. Without the app:  ./start.sh --stop'
    exit 0
fi

#===============================================================================
# Live mode (the default): everything runs from source and follows your edits.
#   interface (app/src)         changes appear in the open app and browser at once
#   app shell (app/src-tauri)   the app rebuilds and reopens itself
#   box daemon (castled/src)           the box rebuilds and restarts within a few seconds
# This stays in the foreground. Closing the app or Ctrl+C ends it all: the app, the watching,
# the live interface, and the box with everything it runs.
#===============================================================================
need_cargo; need_npm
install_packages
build_ai_app
fetch_phoenix
# An earlier ./start.sh (still running, or closed while Tauri left its children behind, as it
# often does on Windows) may hold the dev app and the live interface's port. Those are
# ours: close them so this run takes their place. Anything else is left alone.
LEFTOVER_APPS="$(our_programs "castle$EXE")"
if [[ -n "$LEFTOVER_APPS" ]]; then
    say 'Closing the Castle app an earlier ./start.sh left open, so this one can take its place.'
    for pid in $LEFTOVER_APPS; do stop_tree "$pid"; done
    sleep 1
fi
if app_is_open; then
    die 'A Castle desktop app (not one from this checkout) is open. Close it and run this again, so the live one can take its place.'
fi
if is_listening "$DEV_PORT"; then
    LEFTOVER_DEV="$(our_listener "$DEV_PORT")"
    [[ -n "$LEFTOVER_DEV" ]] || die "Something else is using port $DEV_PORT, which the live interface needs."
    say "Stopping the live interface an earlier ./start.sh left on port $DEV_PORT."
    stop_tree "$LEFTOVER_DEV"
    for _ in 1 2 3 4 5 6 7 8 9 10; do is_listening "$DEV_PORT" || break; sleep 0.5; done
    is_listening "$DEV_PORT" && die "Port $DEV_PORT is still in use after stopping the old live interface."
fi

start_box || exit 1
TOKEN="$(tr -d '[:space:]' < "$DATA/.castle/token")"
LINK="$DEV_URL/#connect=$TOKEN&box=$URL"
say "Web app (live): $LINK"
say '                (the part after # is the key to the box; do not share the link)'

# The box follows its own source: checked every couple of seconds, rebuilt and restarted on a change.
# Only the newest ./start.sh watches: an older one still running steps aside instead of
# rebuilding the same box at the same time.
SESSION_FILE="$CONFIG_DIR/start.session"
SESSION="$$-$RANDOM-$(date +%s)"
printf '%s\n' "$SESSION" > "$SESSION_FILE"
watch_daemon() {
    while sleep "$WATCH_SECONDS"; do
        if [[ "$(cat "$SESSION_FILE" 2>/dev/null)" != "$SESSION" ]]; then
            say 'A newer ./start.sh is watching the box now; this one stopped watching.'
            return 0
        fi
        is_stale "$DAEMON_EXE" "${DAEMON_SOURCES[@]}" || continue
        say 'The box daemon changed. Rebuilding and restarting it.'
        build_daemon || say 'The box daemon did not build. The previous one is starting again; fix the error above and save.'
        start_box || true
    done
}
watch_daemon &
WATCHER=$!
# When live mode ends, nothing of it keeps running: the app (Ctrl+C leaves it open), the box
# and what it started, and the live interface, which Tauri can leave behind on Windows.
end_live() {
    kill "$WATCHER" 2>/dev/null || true
    local pid
    for pid in $(our_programs "castle$EXE"); do stop_tree "$pid"; done
    stop_box_and_all
    pid="$(our_listener "$DEV_PORT")"
    [[ -n "$pid" ]] && stop_tree "$pid"
    say 'Live mode ended. The app, the box and everything it ran have stopped.'
}
trap end_live EXIT

if (( BROWSER )); then
    # Give the live interface a moment to come up before the browser asks for it.
    ( until is_listening "$DEV_PORT"; do sleep "$PROBE_SECONDS"; done; open_url "$LINK" ) &
fi

cd "$CLIENT_DIR"
if (( NO_APP )); then
    say 'Starting the live interface. Ctrl+C to end.'
    npm run dev
else
    # The desktop app bundles the daemon; daemon.js puts it where the app's build looks for it.
    node scripts/daemon.js >/dev/null || die 'Could not prepare the daemon for the desktop app.'
    say 'Starting the live desktop app (the first build takes several minutes). Ctrl+C to end.'
    npx tauri dev
fi
