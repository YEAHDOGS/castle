#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- Network Download Engine (bash twin)
# ==============================================================================
# Bash twin of clonewars/modules/network.ps1.
#   Start-NetworkStream -> download_stream  (HttpClient -> curl)
#
# Contract (mirrors the PS original):
#   download_stream <url> <path>   # 0 = downloaded, 1 = failed
# Logs "[>] Downloading:", the payload size in GB (or "Unknown size"),
# "-> <pct> (<gb> GB)" progress roughly every 50 MB, then "[OK] Download
# complete". On failure the partial file is removed. Unlike the arcade
# twin this one reports in GB and does not create the parent directory
# (the PS original doesn't either -- start.sh ensures data/ exists).
#
# Note: the PS progress predicate (bytes-read % 50MB < 128KB) also fires on
# the very first chunk; this twin logs at each 50 MB boundary instead --
# same steady-state cadence without the 0.00 GB noise line.
# ==============================================================================

# ~50 MiB progress quantum, matching the PS % 52428800 predicate
_CW_NET_QUANTUM=$((50 * 1024 * 1024))

# Start-NetworkStream -> download_stream
download_stream() {
  local url="$1"
  local path="$2"

  printf '  [>] Downloading: %s\n' "$url"

  local total
  total="$(curl -sSL -I "$url" 2>/dev/null \
    | grep -i '^content-length:' | tail -1 | tr -d '\r' | awk '{print $2}')"
  local readable
  if [ -n "$total" ] && [ "$total" -gt 0 ] 2>/dev/null; then
    readable="$(awk -v t="$total" 'BEGIN { printf "%.2f GB", t / 1073741824 }')"
  else
    readable="Unknown size"
    total=""
  fi
  printf '  [>] Payload: %s -- Streaming to disk...\n' "$readable"

  curl -sSL --fail -o "$path" "$url" 2>/dev/null &
  local curl_pid=$!
  local last_logged=0 size cur pct progress_gb

  while kill -0 "$curl_pid" 2>/dev/null; do
    sleep 0.5
    [ -f "$path" ] || continue
    size="$(stat -c%s "$path" 2>/dev/null || echo 0)"
    cur=$((size / _CW_NET_QUANTUM))
    if [ "$cur" -gt "$last_logged" ]; then
      last_logged="$cur"
      progress_gb="$(awk -v s="$size" 'BEGIN { printf "%.2f", s / 1073741824 }')"
      if [ -n "$total" ]; then
        pct="$(awk -v s="$size" -v t="$total" 'BEGIN { printf "%d%%", (t > 0) ? (100 * s / t) : 0 }')"
      else
        pct="Streaming"
      fi
      printf '     -> %s (%s GB)\n' "$pct" "$progress_gb"
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
