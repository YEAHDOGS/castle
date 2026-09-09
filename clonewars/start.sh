#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- One-command virtual machine pipeline (bash twin)
# ==============================================================================
# Bash twin of start.ps1. Downloads, cryptographically verifies, provisions,
# and boots QEMU virtual machines from the clonewars target registry.
#
# Usage:
#   ./start.sh [target-id] [--instance NAME] [--background] [--base-disk PATH]
#              [--vnc] [--delete-disk] [--delete-iso] [--purge]
#   ./start.sh list
#
# The Windows-only GUI (gui.ps1) is intentionally not ported.
# ==============================================================================

set -u

_START_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_MODULE_ROOT="$_START_ROOT/modules"
_DATA_DIR="$_START_ROOT/data"
mkdir -p "$_DATA_DIR"

# --- module loader (mirrors the PS dot-sourcing order) ---
# shellcheck disable=SC1090,SC1091
source "$_MODULE_ROOT/targets.sh"
source "$_MODULE_ROOT/network.sh"
source "$_MODULE_ROOT/verify.sh"
source "$_MODULE_ROOT/hardware.sh"
source "$_MODULE_ROOT/launch.sh"
source "$_MODULE_ROOT/resolvers.sh"

# --- CLI parsing (mirrors the PS param block) ---
TARGET="" INSTANCE="" BASE_DISK=""
BACKGROUND=false VNC=false DELETE_DISK=false DELETE_ISO=false PURGE=false

usage() {
  cat <<'EOF'
  Castle VM -- One-command virtual machine pipeline.

  Usage:
    ./start.sh <target-id>     Directly boot/install a specific target.
    ./start.sh list             Show static list of available targets.
    ./start.sh                 Start the interactive prompting loop.

  Options:
    --instance NAME   Use a named VM instance disk (data/instances/NAME.qcow2)
    --background      Launch QEMU detached in the background
    --base-disk PATH  Use PATH as the backing base disk for provisioning
    --vnc             Expose the VM console over VNC (127.0.0.1)
    --delete-disk     Delete the target's virtual disk and exit
    --delete-iso      Delete the target's cached ISO and exit
    --purge           Full purge: disk, ISO, and pinned trust hash, then exit
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --instance)   INSTANCE="${2:?--instance needs a value}"; shift 2 ;;
    --background) BACKGROUND=true; shift ;;
    --base-disk)  BASE_DISK="${2:?--base-disk needs a value}"; shift 2 ;;
    --vnc)        VNC=true; shift ;;
    --delete-disk) DELETE_DISK=true; shift ;;
    --delete-iso)  DELETE_ISO=true; shift ;;
    --purge)       PURGE=true; shift ;;
    -h|--help)     usage; exit 0 ;;
    --*) printf '  [FAIL] Unknown option: %s\n' "$1" >&2; usage >&2; exit 1 ;;
    *)  [ -z "$TARGET" ] && TARGET="$1"; shift ;;
  esac
done

# --- cache-pattern helpers (mirror the PS File/FileTemplate/Id logic) ---
_iso_glob() { # <target-json> -> filename glob for cached ISO(s)
  local t="$1" id file tpl
  id="$(printf '%s' "$t" | jq -r '.Id')"
  file="$(printf '%s' "$t" | jq -r '.File // empty')"
  tpl="$(printf '%s' "$t" | jq -r '.FileTemplate // empty')"
  if [ -n "$file" ]; then printf '%s' "$file"
  elif [ -n "$tpl" ]; then printf '%s' "${tpl//\$v/*}"
  else printf '%s.iso' "$id"; fi
}

_disk_glob() { # <target-json> -> filename glob for provisioned disk(s)
  local t="$1" id file tpl pat
  id="$(printf '%s' "$t" | jq -r '.Id')"
  file="$(printf '%s' "$t" | jq -r '.File // empty')"
  tpl="$(printf '%s' "$t" | jq -r '.FileTemplate // empty')"
  if [ -n "$file" ]; then pat="$file"
  elif [ -n "$tpl" ]; then pat="${tpl//\$v/*}"
  else pat="$id.iso"; fi
  printf '%s' "$pat" | sed -E 's/\.(iso|img\.gz|zip)$/.qcow2/'
}


# glob a cache pattern, keeping only paths that actually exist
# (nullglob alone keeps literal non-glob patterns for missing files)
_glob_existing() { # <dir> <pattern> -> existing matches, one per line
  local d="$1" pat="$2" f
  shopt -s nullglob
  local hits=( "$d"/$pat )
  shopt -u nullglob
  for f in "${hits[@]}"; do
    [ -e "$f" ] && printf '%s\n' "$f"
  done
}

_gb1() { awk -v b="$1" 'BEGIN { printf "%.1f", b / 1073741824 }'; }
_gb2() { awk -v b="$1" 'BEGIN { printf "%.2f", b / 1073741824 }'; }

# --- resolve a target id to exactly one target JSON (PS exact-then-fuzzy) ---
# prints the target JSON on stdout; rc 1 = unknown, rc 2 = ambiguous
_resolve_single_target() {
  local input="$1" matches count
  matches="$(cw_find_targets "$input")"
  count="$(printf '%s' "$matches" | grep -c . || true)"
  if [ "$count" -eq 0 ]; then
    printf "\n  [FAIL] Unknown target: '%s'\n" "$1" >&2
    return 1
  fi
  if [ "$count" -gt 1 ]; then
    printf "\n  [?] Ambiguous -- '%s' matches multiple entries:\n" "$1" >&2
    printf '%s\n' "$matches" | jq -r '     " * \(.Id) -- \(.Name)"' >&2
    return 2
  fi
  printf '%s\n' "$matches"
}

# --- purge pinned trust keys for a resolved target (PS key patterns) ---
_purge_trust_keys() { # <target-json>
  local t="$1" trust_file="$_DATA_DIR/.castle_trust.json"
  [ -f "$trust_file" ] || return 0
  local file id
  file="$(printf '%s' "$t" | jq -r '.File // empty')"
  id="$(printf '%s' "$t" | jq -r '.Id')"
  local store
  store="$(get_pinned_trust_store)"
  store="$(printf '%s' "$store" | jq -c --arg f "$file" --arg idiso "$id.iso" \
    --arg rx "^${id}-.+\\.(iso|zip|img\\.gz)$" \
    'with_entries(select(.key != $f and .key != $idiso and ((.key | test($rx)) | not)))')"
  save_pinned_trust_store "$store"
}

# ==============================================================================
# DIRECT COMMAND LINE CLEANUP / REMOVAL
# ==============================================================================
if [ "$DELETE_DISK" = true ] || [ "$DELETE_ISO" = true ] || [ "$PURGE" = true ]; then
  if [ -z "$TARGET" ]; then
    printf '  [FAIL] Target name is required for deletion.\n'
    exit 1
  fi
  MATCHED="$(_resolve_single_target "$TARGET")" || exit 1

  # NOTE: PS resolves dynamic versions here (may prompt); keep the behavior.
  RESOLVED="$(resolve_target_version "$MATCHED")"

  if [ -n "$INSTANCE" ]; then
    DISK_FILE="$_DATA_DIR/instances/$INSTANCE.qcow2"
  else
    DISK_FILE="$_DATA_DIR/$(_disk_glob "$RESOLVED")"
  fi
  ISO_FILE="$_DATA_DIR/$(printf '%s' "$RESOLVED" | jq -r '.File // empty')"
  IS_FOLDER="$(printf '%s' "$RESOLVED" | jq -r '.IsFolder // false')"

  if [ "$DELETE_DISK" = true ] || [ "$PURGE" = true ]; then
    if [ -e "$DISK_FILE" ]; then
      printf '  [CLEANUP] Deleting virtual disk: %s\n' "$DISK_FILE"
      rm -f "$DISK_FILE"
    else
      printf '  [CLEANUP] Virtual disk does not exist: %s\n' "$DISK_FILE"
    fi
  fi

  _delete_cached_iso() { # <matched-json> <resolved-json>
    local t="$1" r="$2" pat f rfile
    rfile="$(printf '%s' "$r" | jq -r '.File // empty')"
    if [ -n "$rfile" ] && [ -e "$_DATA_DIR/$rfile" ]; then
      printf '  [CLEANUP] Deleting cached ISO: %s\n' "$_DATA_DIR/$rfile"
      if [ "$IS_FOLDER" = "true" ]; then rm -rf "$_DATA_DIR/$rfile"; else rm -f "$_DATA_DIR/$rfile"; fi
      return 0
    fi
    # Fall back to the wildcard pattern from the ORIGINAL target entry
    pat="$(_iso_glob "$t")"
    mapfile -t hits < <(_glob_existing "$_DATA_DIR" "$pat")
    if [ "${#hits[@]}" -gt 0 ]; then
      for f in "${hits[@]}"; do
        printf '  [CLEANUP] Deleting cached ISO: %s\n' "$f"
        if [ "$IS_FOLDER" = "true" ]; then rm -rf "$f"; else rm -f "$f"; fi
      done
    else
      printf '  [CLEANUP] Cached ISO does not exist: %s\n' "$_DATA_DIR/$rfile"
    fi
  }

  if [ "$DELETE_ISO" = true ] || [ "$PURGE" = true ]; then
    _delete_cached_iso "$MATCHED" "$RESOLVED"
  fi

  if [ "$PURGE" = true ]; then
    before="$(get_pinned_trust_store | jq 'keys | length')"
    _purge_trust_keys "$RESOLVED"
    after="$(get_pinned_trust_store | jq 'keys | length')"
    if [ "$after" -lt "$before" ]; then
      printf '  [CLEANUP] Removed pinned trust hash(es).\n'
    fi
  fi

  printf '  [CLEANUP] Direct cleanup completed successfully.\n'
  exit 0
fi

# ==============================================================================
# INTERACTIVE CLI HELPERS
# ==============================================================================
_show_menu() {
  printf '\n   === Castle VM -- Target Registry ===\n'
  printf '   ================================================================\n\n'
  local i=1 n entry
  n="$(cw_target_count)"
  while [ "$i" -le "$n" ]; do
    entry="$(cw_target_json $((i - 1)))"
    local id name iso_pat disk_pat iso_icon="[   ]" disk_icon="      " size_str=""
    id="$(printf '%s' "$entry" | jq -r '.Id')"
    name="$(printf '%s' "$entry" | jq -r '.Name')"
    iso_pat="$(_iso_glob "$entry")"; disk_pat="$(_disk_glob "$entry")"
    mapfile -t isos < <(_glob_existing "$_DATA_DIR" "$iso_pat")
    mapfile -t disks < <(_glob_existing "$_DATA_DIR" "$disk_pat")
    if [ "${#isos[@]}" -gt 0 ]; then
      iso_icon="[ISO]"
      if [ -d "${isos[0]}" ]; then
        size_str="($(_gb1 "$(du -sb "${isos[0]}" | cut -f1)") GB)"
      else
        size_str="($(_gb1 "$(stat -c%s "${isos[0]}")") GB)"
      fi
    fi
    [ "${#disks[@]}" -gt 0 ] && disk_icon="[DISK]"
    printf '   [%2d] %s %s  %-18s %s %s\n' "$i" "$iso_icon" "$disk_icon" "$id" "$name" "$size_str"
    i=$((i + 1))
  done
  printf '\n   [ISO] = ISO cached  [DISK] = VM disk exists  [   ] = Not downloaded\n'
  printf '   Usage: Select a number, enter a Target ID, type '\''help'\'' or '\''exit'\''.\n\n'
}

_show_target_details() { # <target-json>
  local t="$1"
  local name id desc osf disk_size iso_status disk_status
  name="$(printf '%s' "$t" | jq -r '.Name')"
  id="$(printf '%s' "$t" | jq -r '.Id')"
  desc="$(printf '%s' "$t" | jq -r '.Description // ""')"
  osf="$(printf '%s' "$t" | jq -r '.OsFamily // ""')"
  shopt -s nullglob
  mapfile -t isos < <(_glob_existing "$_DATA_DIR" "$(_iso_glob "$t")")
  mapfile -t disks < <(_glob_existing "$_DATA_DIR" "$(_disk_glob "$t")")
  shopt -u nullglob
  if [ "${#isos[@]}" -gt 0 ]; then
    iso_status="Cached ($(_gb2 "$(stat -c%s "${isos[0]}" 2>/dev/null || du -sb "${isos[0]}" | cut -f1)") GB) at $(basename "${isos[0]}")"
  else
    iso_status="Not downloaded (will fetch automatically)"
  fi
  if [ "$(printf '%s' "$t" | jq -r '.DownloadOnly // false')" = "true" ]; then
    disk_size="N/A"; disk_status="N/A (download-only firmware target)"
  else
    disk_size="$(printf '%s' "$t" | jq -r '.DiskSize // "40G"')"
    if [ "${#disks[@]}" -gt 0 ]; then
      disk_status="Provisioned ($(_gb2 "$(stat -c%s "${disks[0]}")") GB) at $(basename "${disks[0]}")"
    else
      disk_status="Not provisioned (will create new $disk_size disk)"
    fi
  fi
  printf '\n   Target Details: %s\n' "$name"
  printf '   ================================================================\n'
  printf '   * Target ID:     %s\n' "$id"
  printf '   * Description:   %s\n' "$desc"
  printf '   * OS Family:     %s\n' "$osf"
  printf '   * Disk Size:     %s\n' "$disk_size"
  printf '   * ISO Cache:     %s\n' "$iso_status"
  printf '   * Virtual Disk:  %s\n' "$disk_status"
  printf '   ================================================================\n\n'
}

_show_help() {
  cat <<'EOF'

   === Castle VM -- CLI Documentation ===
   ================================================================
   Castle VM is a modular virtual machine pipeline using QEMU.
   It handles automatic ISO download, cryptographic verification,
   local caching, trust tracking (Trust Pinning), disk provisioning, and boot.

   COMMAND LINE USAGE:
     ./start.sh <target-id>     Directly boot/install a specific target.
     ./start.sh list            Show static list of available targets.
     ./start.sh                 Start this interactive prompting loop.

   SECURITY & TRUST PINNING:
     * Remote Verification: Checks remote SHA hashes and GPG signatures.
     * Local Trust Store: After first success, hashes are saved in
       data/.castle_trust.json. Future boots re-verify against the local
       trust store to protect against bit rot or offline tampering.

   DIRECTORY STRUCTURE:
     * start.sh                  Unified CLI entry point.
     * data/                     Downloaded ISOs, virtual disks, and trust store.
     * modules/                  Modular helper scripts for network, targets,
                                 verification, hardware, and launch.
     * scripts/                  Autoinstall configurations.
   ================================================================

EOF
  read -r -p "   Press Enter to return to menu..." _
}

# delete submenu for the interactive loop; arg: <target-json>
_delete_submenu() {
  local t="$1" is_folder opt confirm f
  is_folder="$(printf '%s' "$t" | jq -r '.IsFolder // false')"
  while true; do
    clear
    printf '\n   === Target Cleanup Options: %s ===\n' "$(printf '%s' "$t" | jq -r '.Name')"
    printf '   ================================================================\n'
    printf '   1. Delete Virtual Disk (.qcow2)\n'
    printf '   2. Delete Cached ISO (.iso)\n'
    printf '   3. Full Purge (Delete Disk, ISO, and Pinned Trust Hash)\n'
    printf '   4. Cancel & Go Back\n'
    printf '   ================================================================\n\n'
    read -r -p "   Select an option [1-4]: " opt
    case "${opt:-4}" in
      4|"") return 0 ;;
      1)
        read -r -p "   Are you sure you want to delete the virtual disk .qcow2 file? [y/N] " confirm
        if [[ "$confirm" =~ ^[Yy]([Ee][Ss])?$ ]]; then
          mapfile -t hits < <(_glob_existing "$_DATA_DIR" "$(_disk_glob "$t")")
          if [ "${#hits[@]}" -gt 0 ]; then
            for f in "${hits[@]}"; do rm -f "$f"; done
            printf '   [OK] Deleted virtual disk(s).\n'
          else
            printf '   [i] Disk does not exist.\n'
          fi
          sleep 1.5
        fi
        return 0 ;;
      2)
        read -r -p "   Are you sure you want to delete the cached .iso file? [y/N] " confirm
        if [[ "$confirm" =~ ^[Yy]([Ee][Ss])?$ ]]; then
          mapfile -t hits < <(_glob_existing "$_DATA_DIR" "$(_iso_glob "$t")")
          if [ "${#hits[@]}" -gt 0 ]; then
            for f in "${hits[@]}"; do
              if [ "$is_folder" = "true" ]; then rm -rf "$f"; else rm -f "$f"; fi
            done
            printf '   [OK] Deleted cached ISO(s).\n'
          else
            printf '   [i] Cached ISO does not exist.\n'
          fi
          sleep 1.5
        fi
        return 0 ;;
      3)
        read -r -p "   Are you sure you want to perform a full purge (.qcow2, .iso, and trust hash)? [y/N] " confirm
        if [[ "$confirm" =~ ^[Yy]([Ee][Ss])?$ ]]; then
          mapfile -t dhits < <(_glob_existing "$_DATA_DIR" "$(_disk_glob "$t")")
          mapfile -t ihits < <(_glob_existing "$_DATA_DIR" "$(_iso_glob "$t")")
          for f in "${dhits[@]}"; do rm -f "$f"; done
          for f in "${ihits[@]}"; do
            if [ "$is_folder" = "true" ]; then rm -rf "$f"; else rm -f "$f"; fi
          done
          _purge_trust_keys "$t"
          printf '   [OK] Full purge completed.\n'
          sleep 1.5
        fi
        return 0 ;;
      *) printf '   Invalid selection. Enter 1-4.\n'; sleep 1 ;;
    esac
  done
}

# ==============================================================================
# NO ARGUMENT -- Interactive prompt mode
# ==============================================================================
if [ -z "$TARGET" ]; then
  COUNT="$(cw_target_count)"
  while true; do
    clear
    _show_menu
    read -r -p "   Castle VM [1-$COUNT, target-id, help, exit]: " CHOICE
    [ -z "${CHOICE// }" ] && continue
    CHOICE="$(printf '%s' "$CHOICE" | tr '[:upper:]' '[:lower:]' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    case "$CHOICE" in
      exit|q) printf '   Exiting Castle VM. Goodbye!\n'; exit 0 ;;
      help|h) _show_help; continue ;;
    esac
    SELECTED=""
    if [[ "$CHOICE" =~ ^[0-9]+$ ]]; then
      if [ "$CHOICE" -ge 1 ] && [ "$CHOICE" -le "$COUNT" ]; then
        SELECTED="$(cw_target_json $((CHOICE - 1)))"
      else
        printf "   [FAIL] Index %s is out of range.\n" "$CHOICE"; sleep 1; continue
      fi
    else
      MATCHES="$(cw_find_targets "$CHOICE")"
      N="$(printf '%s' "$MATCHES" | grep -c . || true)"
      if [ "$N" -eq 0 ]; then
        printf "   [FAIL] Unknown target: '%s'\n" "$CHOICE"; sleep 1.5; continue
      elif [ "$N" -gt 1 ]; then
        printf '   [?] Ambiguous -- matches multiple targets:\n'
        printf '%s\n' "$MATCHES" | jq -r '     " * \(.Id) -- \(.Name)"'
        read -r -p "   Press Enter to try again..." _
        continue
      else
        SELECTED="$MATCHES"
      fi
    fi
    while true; do
      clear
      _show_target_details "$SELECTED"
      read -r -p "   Boot this VM? [Y]es / [N]o / [D]elete Options / [B]ack (default: Y): " BOOT
      BOOT="${BOOT:-y}"
      BOOT="$(printf '%s' "$BOOT" | tr '[:upper:]' '[:lower:]' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
      case "$BOOT" in
        y|yes) TARGET="$(printf '%s' "$SELECTED" | jq -r '.Id')"; break 2 ;;
        n|no|b|back) SELECTED=""; break ;;
        d|delete) _delete_submenu "$SELECTED" ;;
        *) printf '   Invalid choice. Enter Y, N, D, or B.\n'; sleep 1 ;;
      esac
    done
  done
fi

# ==============================================================================
# LIST -- Show static list of all targets and exit
# ==============================================================================
if [ "$TARGET" = "list" ]; then
  printf '\n  Castle VM -- Target Registry\n'
  printf '  ================================================================\n\n'
  n="$(cw_target_count)"
  i=0
  while [ "$i" -lt "$n" ]; do
    entry="$(cw_target_json "$i")"; i=$((i + 1))
    id="$(printf '%s' "$entry" | jq -r '.Id')"
    name="$(printf '%s' "$entry" | jq -r '.Name')"
    is_folder="$(printf '%s' "$entry" | jq -r '.IsFolder // false')"
    mapfile -t isos < <(_glob_existing "$_DATA_DIR" "$(_iso_glob "$entry")")
    mapfile -t disks < <(_glob_existing "$_DATA_DIR" "$(_disk_glob "$entry")")
    iso_icon="[   ]"; disk_icon="      "; size_str=""
    if [ "${#isos[@]}" -gt 0 ]; then
      iso_icon="[ISO]"
      if [ "$is_folder" = "true" ]; then
        size_str="($(_gb1 "$(du -sb "${isos[0]}" | cut -f1)") GB)"
      else
        size_str="($(_gb1 "$(stat -c%s "${isos[0]}")") GB)"
      fi
    fi
    [ "${#disks[@]}" -gt 0 ] && disk_icon="[DISK]"
    printf '  %s %s  %-18s %s %s\n' "$iso_icon" "$disk_icon" "$id" "$name" "$size_str"
  done
  printf '\n  [ISO] = ISO cached  [DISK] = VM disk exists  [   ] = Not downloaded\n'
  printf '\n  Usage: ./start.sh <target-id>\n\n'
  exit 0
fi

# ==============================================================================
# RESOLVE TARGET
# ==============================================================================
RESOLVED="$(_resolve_single_target "$TARGET")" || exit 1

# Apply Dynamic Version Resolution (Phase 2 feature)
ISO="$(resolve_target_version "$RESOLVED")"
ISO_FILE="$(printf '%s' "$ISO" | jq -r '.File // empty')"
ISO_PATH="$_DATA_DIR/$ISO_FILE"
ISO_NAME="$(printf '%s' "$ISO" | jq -r '.Name')"
ISO_ID="$(printf '%s' "$ISO" | jq -r '.Id')"

printf '\n  Castle VM Pipeline\n'
printf '  Target: %s\n' "$ISO_NAME"
printf '  ================================================================\n'

VERIFIED=false
while [ "$VERIFIED" != true ]; do
  # ==========================================================================
  # PHASE 1 -- ISO ACQUISITION (Download if missing)
  # ==========================================================================
  printf '\n  [*] Phase 1: ISO Acquisition\n'
  printf '  -------------------------------------------\n'

  if [ -e "$ISO_PATH" ]; then
    printf '  [OK] ISO cached: %s (%s GB)\n' "$ISO_PATH" "$(_gb2 "$(stat -c%s "$ISO_PATH" 2>/dev/null || du -sb "$ISO_PATH" | cut -f1)")"
  else
    SOURCE_FOLDER="$_DATA_DIR/$ISO_ID"
    if { [ "$ISO_ID" = "win11-home" ] || [ "$ISO_ID" = "win11-pro" ]; } && [ -d "$SOURCE_FOLDER" ]; then
      if ! new_unattended_windows_iso "$ISO" "$SOURCE_FOLDER" "$ISO_PATH"; then
        printf '  [FAIL] Failed to generate unattended ISO. Aborting.\n'
        exit 1
      fi
    else
      printf '  [>] ISO not found locally. Starting secure download...\n'
      mkdir -p "$_DATA_DIR"
      ISO_URL="$(printf '%s' "$ISO" | jq -r '.Url // empty')"
      if ! download_stream "$ISO_URL" "$ISO_PATH"; then
        printf '  [FAIL] Download failed. Aborting.\n'
        exit 1
      fi
    fi
  fi

  # ==========================================================================
  # PHASE 2 -- INTEGRITY VERIFICATION (Always runs, even for cached ISOs)
  # ==========================================================================
  printf '\n  [*] Phase 2: Integrity Verification\n'
  printf '  -------------------------------------------\n'

  if verify_iso_integrity "$ISO" "$ISO_PATH"; then
    VERIFIED=true
  fi

  if [ "$VERIFIED" != true ]; then
    printf '\n  [FAIL] INTEGRITY CHECK FAILED -- Refusing to boot unverified image.\n'
    read -r -p "     Delete and re-download? [Y]es / [N]o (default: N): " CHOICE
    if [[ "$CHOICE" =~ ^[Yy]([Ee][Ss])?$ ]]; then
      rm -f "$ISO_PATH"
      printf '  [i] Deleted corrupted ISO. Retrying...\n'
    else
      printf '  [FAIL] Exiting without boot.\n'
      exit 1
    fi
  fi
done

# ==============================================================================
# DOWNLOAD-ONLY TARGETS -- Firmware images that are flashed, not booted
# ==============================================================================
if [ "$(printf '%s' "$ISO" | jq -r '.DownloadOnly // false')" = "true" ]; then
  printf '\n  [OK] Download-only target verified and cached: %s\n' "$ISO_PATH"
  printf '  [i] This is handheld firmware -- flash/copy it to an SD card (e.g. balenaEtcher/Rufus).\n'
  printf '  [i] QEMU provisioning and boot are skipped for this target.\n'
  exit 0
fi

# ==============================================================================
# PHASE 3 -- DISK PROVISIONING
# ==============================================================================
printf '\n  [*] Phase 3: Disk Provisioning\n'
printf '  -------------------------------------------\n'

if ! qemu_installed; then exit 1; fi

if [ -n "$INSTANCE" ]; then
  DISK_PATH="$_DATA_DIR/instances/$INSTANCE.qcow2"
else
  DISK_PATH="$_DATA_DIR/$(_disk_glob "$ISO")"
fi
DISK_SIZE="$(printf '%s' "$ISO" | jq -r '.DiskSize // "40G"')"
FIRST_BOOT=false
if new_virtual_disk "$DISK_PATH" "$DISK_SIZE" "$BASE_DISK"; then
  FIRST_BOOT=true
else
  rc=$?
  if [ "$rc" -eq 2 ]; then exit 1; fi
  FIRST_BOOT=false
fi

# ==============================================================================
# PHASE 4 -- HARDWARE DETECTION
# ==============================================================================
printf '\n  [*] Phase 4: Hardware Detection\n'
printf '  -------------------------------------------\n'

HARDWARE="$(get_host_hardware_profile)"
printf '  * CPU: %s cores (Host: %s) -- %s\n' \
  "$(printf '%s' "$HARDWARE" | jq -r '.cpu_cores')" \
  "$(printf '%s' "$HARDWARE" | jq -r '.host_cores')" \
  "$(printf '%s' "$HARDWARE" | jq -r '.cpu_profile')"
printf '  * RAM: %s (Host: %sGB)\n' \
  "$(printf '%s' "$HARDWARE" | jq -r '.memory')" \
  "$(printf '%s' "$HARDWARE" | jq -r '.total_ram_gb')"

# ==============================================================================
# PHASE 5 -- LAUNCH
# ==============================================================================
start_castle_vm "$ISO" "$DISK_PATH" "$ISO_PATH" "$HARDWARE" \
  "$FIRST_BOOT" "$BACKGROUND" "$VNC"
