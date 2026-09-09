#!/usr/bin/env bash
# Dogs uptime pinger — ZERO tokens. No AI, no agent, just curl.
# Install on YOUR machine (not mine):
#   1. Save this file, then: chmod +x uptime-ping.sh
#   2. crontab -e, add:  * * * * * /path/to/uptime-ping.sh
# Appends one line per site per minute to ~/uptime.log
set -u
URLS=(
  "https://icecream.wearedogs.net"
  "https://yeahdogs.github.io/tower/"
  "https://yeahdogs.github.io/forge/"
  "https://wearedogs.net"
)
LOG="${UPTIME_LOG:-$HOME/uptime.log}"
for url in "${URLS[@]}"; do
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 15 "$url" 2>/dev/null || echo "FAIL")
  printf '%s %s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$url" "$code" >> "$LOG"
done
