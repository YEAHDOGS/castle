#!/usr/bin/env bash
# ==============================================================================
# Castle Clone Factory -- Docker entrypoint
# ==============================================================================
# Starts the noVNC web gateway (websockify bridging to QEMU's VNC display)
# and then hands off to the PowerShell VM pipeline, forwarding any CLI args
# (e.g. `docker run ... castle-vm cachyos -Vnc`).
#
# Environment overrides:
#   NOVNC_PORT  - web gateway listen port (default: 8006)
#   VNC_TARGET  - VNC endpoint noVNC bridges to (default: localhost:5900,
#                 i.e. QEMU display :0). Note: additional -Vnc instances pick
#                 free displays (:1, :2, ...) which are NOT bridged -- only
#                 display :0 is reachable through the web gateway.
# ==============================================================================
set -euo pipefail

mkdir -p /app/data

NOVNC_PORT="${NOVNC_PORT:-8006}"
VNC_TARGET="${VNC_TARGET:-localhost:5900}"

NOVNC_PROXY="/usr/share/novnc/utils/novnc_proxy"
if [ -x "$NOVNC_PROXY" ]; then
    "$NOVNC_PROXY" --listen "0.0.0.0:${NOVNC_PORT}" --vnc "${VNC_TARGET}" \
        >/var/log/novnc.log 2>&1 &
    echo "[entrypoint] noVNC gateway listening on :${NOVNC_PORT} -> ${VNC_TARGET}"
    echo "[entrypoint] open http://localhost:${NOVNC_PORT}/vnc.html in a browser"
else
    echo "[entrypoint] WARNING: novnc_proxy not found at ${NOVNC_PROXY}; web VNC gateway disabled."
fi

# Graceful shutdown: forward SIGTERM/SIGINT to children (pwsh + websockify)
trap 'kill 0' TERM INT

exec pwsh -NoProfile -ExecutionPolicy Bypass -File /app/start.ps1 "$@"
