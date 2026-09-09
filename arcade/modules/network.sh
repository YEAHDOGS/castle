#!/usr/bin/env bash
# ==============================================================================
# Arcade Network Download Library Module (bash twin)
# ==============================================================================
# Bash twin of network.ps1.
#   Start-DownloadStream -> download_stream  (HttpClient -> curl)
#
# Contract (mirrors the PS original exactly):
#   download_stream <url> <path>   # 0 = downloaded, 1 = failed
# Logs "Starting secure download stream", the target size in MB (or
# "Unknown size"), progress lines "-> <pct> (<mb> MB)" each time another
# 10 MB lands, then "Download complete". On failure the partial file is
# removed, like the PS catch block.
# ==============================================================================

# 10 MiB progress quantum, matching the PS [Math]::Floor($TotalBytesRead/10MB)
_ARCADE_NET_QUANTUM=$((10 * 1024 * 1024))
_ARCADE_NET_UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"

# Start-DownloadStream -> download_stream
download_stream() {
  local url="$1"
  local path="$2"

  printf '  [NETWORK] Starting secure download stream from: %s\n' "$url"
  mkdir -p "$(dirname "$path")"

  # PS reads Content-Length off the response headers before streaming.
  local total
  total="$(curl -sSL -A "$_ARCADE_NET_UA" -I "$url" 2>/dev/null \
    | grep -i '^content-length:' | tail -1 | tr -d '\r' | awk '{print $2}')"
  local readable
  if [ -n "$total" ] && [ "$total" -gt 0 ] 2>/dev/null; then
    readable="$(awk -v t="$total" 'BEGIN { printf "%.2f MB", t / 1048576 }')"
  else
    readable="Unknown size"
    total=""
  fi
  printf '  [NETWORK] Target Size: %s -- Streaming payload to: %s...\n' "$readable" "$path"

  curl -sSL --fail -A "$_ARCADE_NET_UA" -o "$path" "$url" 2>/dev/null &
  local curl_pid=$!
  local last_logged=0 size cur_mb pct progress_mb

  while kill -0 "$curl_pid" 2>/dev/null; do
    sleep 0.5
    [ -f "$path" ] || continue
    size="$(stat -c%s "$path" 2>/dev/null || echo 0)"
    cur_mb=$((size / _ARCADE_NET_QUANTUM))
    if [ "$cur_mb" -gt "$last_logged" ]; then
      last_logged="$cur_mb"
      progress_mb="$(awk -v s="$size" 'BEGIN { printf "%.2f", s / 1048576 }')"
      if [ -n "$total" ]; then
        pct="$(awk -v s="$size" -v t="$total" 'BEGIN { printf "%d%%", (t > 0) ? (100 * s / t) : 0 }')"
      else
        pct="Streaming"
      fi
      printf '     -> %s (%s MB)\n' "$pct" "$progress_mb"
    fi
  done

  wait "$curl_pid"
  local rc=$?
  if [ "$rc" -eq 0 ]; then
    printf '  [OK] Download complete: %s\n' "$path"
    return 0
  fi
  printf '  [FAIL] Download failed: curl exited with code %d\n' "$rc"
  rm -f "$path"
  return 1
}
