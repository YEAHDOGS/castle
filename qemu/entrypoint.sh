#!/bin/bash
set -e

# Start websockify in the background to bridge VNC (port 5900) to WebSockets/NoVNC (port 8006)
# If QEMU starts VNC display index > 0, websockify connects to port 5900 (display :0) by default.
websockify --web /usr/share/novnc 8006 127.0.0.1:5900 >/dev/null 2>&1 &

# Forward all arguments directly to PowerShell start.ps1
# We use exec so pwsh becomes PID 1 and receives process termination signals cleanly.
exec pwsh /app/start.ps1 "$@"
