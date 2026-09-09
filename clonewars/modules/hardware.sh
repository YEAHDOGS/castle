#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- Host Hardware Detection (bash twin)
# ==============================================================================
# Bash twin of modules/hardware.ps1. The PS original has two branches: Linux
# via /proc and Windows via CIM/WMI. Per the conversion plan the Windows CIM
# branch is dropped -- this twin is Linux-only and keeps the /proc branch.
#
#   get_host_hardware_profile   # prints the profile as JSON:
#     { cpu_cores, memory ("8G"), cpu_profile, host_cores,
#       total_ram_gb, cpu_vendor }
#
# Semantics preserved from the PS original:
#   cpu_cores  = max(2, host_cores - 2)
#   memory     = 50% of total RAM, clamped to [4, max_ram] GB, where max_ram
#                defaults to 8 and honors $CASTLE_VM_MAX_RAM_GB (>= 4)
#   cpu_profile: *Intel* -> Haswell-v4, *Advanced Micro Devices* -> EPYC-v4,
#                else "max"
# ==============================================================================

# .NET [Math]::Round uses banker's rounding (half to even); replicate it so
# odd-GB hosts allocate exactly what the PS version would.
_round_half_even() {
  awk -v x="$1" 'BEGIN {
    f = int(x); d = x - f
    if (d < 0.5) r = f
    else if (d > 0.5) r = f + 1
    else r = (f % 2 == 0) ? f : f + 1
    printf "%d", r
  }'
}
# Get-HostHardwareProfile -> get_host_hardware_profile
get_host_hardware_profile() {
  # 1. CPU cores -- logical processor count, minus 2 headroom, minimum 2
  local host_cores cpu_cores
  host_cores="$(nproc 2>/dev/null || grep -c '^processor' /proc/cpuinfo 2>/dev/null || echo 2)"
  cpu_cores=$((host_cores - 2))
  [ "$cpu_cores" -lt 2 ] && cpu_cores=2

  # 2. Total RAM (GB) via /proc/meminfo, CPU vendor via /proc/cpuinfo
  local total_ram_gb=16 cpu_vendor="Unknown" mem_kb vendor_line
  if [ -r /proc/meminfo ]; then
    mem_kb="$(grep -m1 '^MemTotal:' /proc/meminfo | grep -o '[0-9]*')"
    if [ -n "$mem_kb" ]; then
      # PS: [Math]::Round(kb/1024/1024)
      total_ram_gb="$(_round_half_even "$(awk -v kb="$mem_kb" 'BEGIN { printf "%.6f", kb / 1048576 }')")"
    fi
  fi
  if [ -r /proc/cpuinfo ]; then
    vendor_line="$(grep -m1 'vendor_id' /proc/cpuinfo)"
    if [[ "$vendor_line" =~ vendor_id[[:space:]]*:[[:space:]]*(.+) ]]; then
      cpu_vendor="$(printf '%s' "${BASH_REMATCH[1]}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    fi
  fi

  # RAM allocation: 50% of total, clamped between 4 GB and the ceiling.
  local max_ram_gb=8
  if [ -n "${CASTLE_VM_MAX_RAM_GB:-}" ] && [[ "$CASTLE_VM_MAX_RAM_GB" =~ ^[0-9]+$ ]]; then
    max_ram_gb="$CASTLE_VM_MAX_RAM_GB"
    [ "$max_ram_gb" -lt 4 ] && max_ram_gb=4
  fi
  local half_ram
  half_ram="$(_round_half_even "$(awk -v t="$total_ram_gb" 'BEGIN { printf "%.6f", t / 2 }')")"
  [ "$half_ram" -lt 4 ] && half_ram=4
  local memory_gb="$half_ram"
  [ "$memory_gb" -gt "$max_ram_gb" ] && memory_gb="$max_ram_gb"

  # CPU profile vendor selection (PS switch -Wildcard)
  local cpu_profile="max"
  case "$cpu_vendor" in
    *Intel*) cpu_profile="Haswell-v4" ;;
    *"Advanced Micro Devices"*) cpu_profile="EPYC-v4" ;;
  esac

  jq -n \
    --argjson cpu_cores "$cpu_cores" \
    --arg memory "${memory_gb}G" \
    --arg cpu_profile "$cpu_profile" \
    --argjson host_cores "$host_cores" \
    --argjson total_ram_gb "$total_ram_gb" \
    --arg cpu_vendor "$cpu_vendor" \
    '{cpu_cores:$cpu_cores, memory:$memory, cpu_profile:$cpu_profile,
      host_cores:$host_cores, total_ram_gb:$total_ram_gb, cpu_vendor:$cpu_vendor}'
}
