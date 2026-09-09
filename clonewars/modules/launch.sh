#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- QEMU Launch Engine (bash twin)
# ==============================================================================
# Bash twin of modules/launch.ps1. The PS original carries Windows branches
# (C:\Program Files\qemu paths, WHPX accel, Windows OVMF share dir, oscdimg);
# this twin is Linux-only -- those branches are dropped, except
# new_unattended_windows_iso which keeps the PS fail-fast behavior on
# non-Windows (oscdimg.exe is Windows-only) so call sites behave identically.
#
#   running_in_docker                 # 0 = in docker
#   free_vnc_display                  # prints first free VNC display (0-99)
#   qemu_installed                    # 0 = qemu-system-x86_64 on PATH
#   new_virtual_disk <disk> [size] [base]   # 0 = fresh disk (first boot),
#                                           # 1 = disk already existed,
#                                           # 2 = fatal (base disk missing)
#   build_qemu_args <target-json> <disk> <iso> <hw-json> <first:true|false>
#                   <vnc:true|false> <display>
#                                     # prints the arg vector NUL-delimited
#   start_castle_vm <target-json> <disk> <iso> <hw-json> <first:true|false>
#                   [background] [vnc]
#
# OS families: linux (virtio disk, virtio VGA), windows (IDE disk, std VGA,
# UEFI for Win11), macos (IDE disk, std VGA, experimental note).
# ==============================================================================

QEMU_IMG="qemu-img"
QEMU_SYSTEM="qemu-system-x86_64"
_LAUNCH_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_SCRIPTS_DIR="$(dirname "$_LAUNCH_ROOT")/scripts"

# Test-RunningInDocker -> running_in_docker
running_in_docker() {
  [ -f "/.dockerenv" ] || [ -n "${DOCKER_CONTAINER:-}" ]
}

# Get-FreeVncDisplay -> free_vnc_display
# PS probes .NET's active TCP listeners; here we probe 127.0.0.1 via /dev/tcp.
free_vnc_display() {
  local display port
  for ((display = 0; display < 100; display++)); do
    port=$((5900 + display))
    if ! (echo > "/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
      printf '%d\n' "$display"
      return 0
    fi
  done
  printf '0\n'
}

# Test-QemuInstalled -> qemu_installed
qemu_installed() {
  if command -v "$QEMU_SYSTEM" >/dev/null 2>&1; then
    return 0
  fi
  printf '  [FAIL] QEMU not found. Please install QEMU using your package manager.\n'
  printf '     Example: apt-get install -y qemu-system-x86 qemu-utils\n'
  return 1
}

# New-VirtualDisk -> new_virtual_disk
# rc 0: fresh disk created (first boot); rc 1: disk already existed;
# rc 2: fatal (backing base disk missing) -- mirrors PS `Exit 1`.
new_virtual_disk() {
  local disk_path="$1"
  local disk_size="${2:-40G}"
  local base_disk="${3:-}"

  if [ -f "$disk_path" ]; then
    local disk_bytes disk_gb has_backing=0
    disk_bytes="$(stat -c%s "$disk_path")"
    disk_gb="$(awk -v b="$disk_bytes" 'BEGIN { printf "%.2f", b / 1073741824 }')"

    # A qcow2 created but never installed to holds only metadata (~200 KB);
    # treat it as a first boot unless it is a linked clone (has backing file).
    if [ "$disk_bytes" -lt 4194304 ]; then
      local backing
      backing="$("$QEMU_IMG" info --output=json "$disk_path" 2>/dev/null \
        | jq -r '."backing-filename" // empty' 2>/dev/null)"
      [ -n "$backing" ] && has_backing=1
    else
      has_backing=1  # large enough: skip the (possibly slow) qemu-img probe
    fi

    if [ "$disk_bytes" -lt 4194304 ] && [ "$has_backing" -eq 0 ]; then
      printf '  [DISK] Virtual disk exists but is blank: %s\n' "$disk_path"
      printf '  [DISK] Nothing installed yet -- booting the installer from CD-ROM.\n'
      return 0
    fi
    printf '  [DISK] Virtual disk exists: %s (%s GB on disk)\n' "$disk_path" "$disk_gb"
    return 1
  fi

  mkdir -p "$(dirname "$disk_path")"

  if [ -n "$base_disk" ]; then
    if [ ! -f "$base_disk" ]; then
      printf '  [FAIL] Base backing disk not found: %s\n' "$base_disk"
      return 2
    fi
    local abs_base
    abs_base="$(cd "$(dirname "$base_disk")" && pwd)/$(basename "$base_disk")"
    printf '  [DISK] Creating linked clone backed by base: %s\n' "$abs_base"
    "$QEMU_IMG" create -f qcow2 -b "$abs_base" -F qcow2 "$disk_path" >/dev/null
  else
    printf '  [DISK] Creating %s virtual disk: %s\n' "$disk_size" "$disk_path"
    "$QEMU_IMG" create -f qcow2 "$disk_path" "$disk_size" >/dev/null
  fi
  return 0
}

# Build-QemuArgs -> build_qemu_args
# Prints the QEMU argument vector NUL-delimited (paths may contain spaces).
build_qemu_args() {
  local target="$1" disk_path="$2" iso_path="$3" hardware="$4"
  local first_boot="$5" vnc="$6" vnc_display="${7:-0}"

  local os_family target_id firmware
  os_family="$(printf '%s' "$target" | jq -r '.OsFamily // "linux"')"
  target_id="$(printf '%s' "$target" | jq -r '.Id')"
  firmware="$(printf '%s' "$target" | jq -r '.Firmware // empty')"

  # Boot order: CD-ROM first for fresh installs, disk first for existing VMs.
  local boot_order="cd"
  [ "$first_boot" = "true" ] && boot_order="dc"

  local disk_if="virtio" vga_type="virtio"
  case "$os_family" in
    windows|macos) disk_if="ide"; vga_type="std" ;;
  esac

  local cpu_profile cpu_cores memory
  cpu_profile="$(printf '%s' "$hardware" | jq -r '.cpu_profile')"
  cpu_cores="$(printf '%s' "$hardware" | jq -r '.cpu_cores')"
  memory="$(printf '%s' "$hardware" | jq -r '.memory')"

  local -a args=(
    -accel kvm
    -accel tcg
    -cpu "$cpu_profile"
    -smp "$cpu_cores"
    -m "$memory"
    -drive "file=\"$disk_path\",if=$disk_if,format=qcow2"
    -drive "file=\"$iso_path\",media=cdrom,readonly=on"
    -boot "order=$boot_order"
    -vga "$vga_type"
    -usb
    -device usb-tablet
  )

  # Attach the /scripts folder as a Virtual FAT drive (skip for android).
  # USB bus, not IDE: ide-hd refuses a read-only backing node, and USB keeps
  # the share out of the hard-disk enumeration.
  if [ -d "$_SCRIPTS_DIR" ] && [ "$target_id" != "android" ]; then
    args+=(
      -drive "file=fat:ro:\"$_SCRIPTS_DIR\",format=raw,if=none,id=castlescripts,readonly=on"
      -device "usb-storage,drive=castlescripts"
    )
  fi

  if [ "$vnc" = "true" ]; then
    local vnc_bind="127.0.0.1"
    running_in_docker && vnc_bind="0.0.0.0"
    args+=(-vnc "$vnc_bind:$vnc_display")
  else
    args+=(-display gtk)
  fi

  # UEFI firmware (Windows 11, modern targets). Debian/Ubuntu ship OVMF in
  # /usr/share/OVMF via the 'ovmf' package.
  if [ "$firmware" = "uefi" ]; then
    local ovmf_path="" candidate
    for candidate in \
        "/usr/share/OVMF/OVMF_CODE.fd" \
        "/usr/share/OVMF/OVMF_CODE_4M.fd" \
        "/usr/share/edk2/x86_64/OVMF_CODE.fd" \
        "/usr/share/qemu/OVMF.fd"; do
      if [ -f "$candidate" ]; then ovmf_path="$candidate"; break; fi
    done
    if [ -n "$ovmf_path" ]; then
      args+=(-bios "$ovmf_path")
      printf '  [*] Firmware: UEFI (%s)\n' "$ovmf_path" >&2
    else
      printf "  [?] UEFI firmware (OVMF) not found -- falling back to BIOS.\n" >&2
      printf "     Windows 11 requires UEFI. Install OVMF in: /usr/share/OVMF/  (install the 'ovmf' package)\n" >&2
    fi
  fi

  if [ "$os_family" = "macos" ]; then
    printf '  [?] macOS support is experimental. Additional setup may be required.\n' >&2
  fi

  printf '%s\0' "${args[@]}"
}

# Start-CastleVm -> start_castle_vm
start_castle_vm() {
  local target="$1" disk_path="$2" iso_path="$3" hardware="$4"
  local first_boot="$5" background="${6:-false}" vnc="${7:-false}"

  local os_family name boot_mode disk_if
  os_family="$(printf '%s' "$target" | jq -r '.OsFamily // "linux"')"
  name="$(printf '%s' "$target" | jq -r '.Name')"
  boot_mode="Disk (existing)"
  [ "$first_boot" = "true" ] && boot_mode="CD-ROM (install)"
  disk_if="virtio"
  [[ "$os_family" == "windows" || "$os_family" == "macos" ]] && disk_if="ide"

  local vnc_display=0 vnc_bind="127.0.0.1"
  if [ "$vnc" = "true" ]; then
    vnc_display="$(free_vnc_display)"
    running_in_docker && vnc_bind="0.0.0.0"
  fi

  printf '\n  [BOOT] Launching VM\n'
  printf '  -------------------------------------------\n'
  printf '  * Target:    %s\n' "$name"
  printf '  * OS Family: %s\n' "$os_family"
  printf '  * Boot:      %s\n' "$boot_mode"
  printf '  * CPU:       %s cores -- %s\n' \
    "$(printf '%s' "$hardware" | jq -r '.cpu_cores')" \
    "$(printf '%s' "$hardware" | jq -r '.cpu_profile')"
  printf '  * RAM:       %s\n' "$(printf '%s' "$hardware" | jq -r '.memory')"
  printf '  * Disk:      %s interface\n' "$disk_if"
  if [ "$vnc" = "true" ]; then
    printf '  * VNC:       Listening on %s:%d (Display :%d)\n' \
      "$vnc_bind" "$((5900 + vnc_display))" "$vnc_display"
  fi

  local -a qemu_args
  mapfile -d '' qemu_args < <(build_qemu_args "$target" "$disk_path" "$iso_path" \
    "$hardware" "$first_boot" "$vnc" "$vnc_display")

  if [ "$background" = "true" ]; then
    printf '  [BOOT] Launching VM in the background (asynchronous)...\n'
    "$QEMU_SYSTEM" "${qemu_args[@]}" >/dev/null 2>&1 &
    printf '  [BOOT] Background VM Process started (PID: %d).\n' "$!"
  else
    printf '\n  Castle VM booting...\n'
    if [ "$vnc" = "true" ]; then
      printf '  VNC mode active. Connect a VNC client to display.\n'
    else
      printf '  Close the QEMU window to shut down.\n'
    fi
    printf '\n'
    "$QEMU_SYSTEM" "${qemu_args[@]}"
  fi
}

# New-UnattendedWindowsIso -> new_unattended_windows_iso
# oscdimg.exe is Windows-only: the PS version fails fast on non-Windows
# before doing anything else, and so does this twin.
new_unattended_windows_iso() {
  printf '  [FAIL] Unattended Windows ISO generation requires Windows (oscdimg.exe).\n'
  printf '     Generate the ISO on your Windows host, then copy it into clonewars/data/.\n'
  return 1
}
