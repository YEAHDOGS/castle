#!/usr/bin/env bash
# ==============================================================================
# Retro Console Downloader Pipeline (bash twin)
# ==============================================================================
# Bash twin of dl/consoles.ps1. Downloads, cryptographically verifies,
# extracts, and structures retro console BIOS/ROM files.
#
# Usage:
#   consoles.sh [target-id | list] [--purge]
#   consoles.sh                  # interactive menu loop
#   consoles.sh gba-bios
#   consoles.sh --purge gba-bios
# ==============================================================================
set -uo pipefail

# --- module loader ------------------------------------------------------------
_CONSOLES_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_MODULE_ROOT="$(dirname "$_CONSOLES_ROOT")/modules"
# shellcheck source=/dev/null
source "$_MODULE_ROOT/targets.sh"
# shellcheck source=/dev/null
source "$_MODULE_ROOT/security.sh"
# shellcheck source=/dev/null
source "$_MODULE_ROOT/archive.sh"
# shellcheck source=/dev/null
source "$_MODULE_ROOT/network.sh"

DATA_DIR="$(dirname "$_CONSOLES_ROOT")/data"
BIOS_DIR="$DATA_DIR/bios"
ROMS_DIR="$DATA_DIR/roms"

TARGET=""
PURGE=0
for arg in "$@"; do
  case "$arg" in
    --purge|-p) PURGE=1 ;;
    *) TARGET="$arg" ;;
  esac
done

# --- dynamic path resolver ----------------------------------------------------
# Get-TargetPaths -> target_paths <target-json>
# Prints "<folder> <file>" (paths contain no spaces in this manifest).
target_paths() {
  local entry="$1" type platform folder
  type="$(printf '%s' "$entry" | jq -r '.Type')"
  platform="$(printf '%s' "$entry" | jq -r '.Platform')"
  if [ "$type" = "bios" ]; then
    folder="$BIOS_DIR"
  else
    folder="$ROMS_DIR/$platform"
  fi
  printf '%s %s\n' "$folder" "$folder/$(printf '%s' "$entry" | jq -r '.File')"
}

# --- target lookup ------------------------------------------------------------
# Prints matching target JSON lines; sets MATCH_COUNT. Mirrors the PS
# `$ArcadeMatrix | Where-Object { Id -eq or Name -like }` logic (see
# arcade_find_targets in targets.sh).
lookup_targets() {
  mapfile -t MATCHES < <(arcade_find_targets "$1")
  MATCH_COUNT="${#MATCHES[@]}"
}

# --- purge / cleanup ----------------------------------------------------------
if [ "$PURGE" -eq 1 ]; then
  if [ -z "$TARGET" ]; then
    printf '  [FAIL] Target ID is required to purge.\n'
    exit 1
  fi
  lookup_targets "$TARGET"
  if [ "$MATCH_COUNT" -eq 0 ]; then
    printf "  [FAIL] Target '%s' not found in registry.\n" "$TARGET"
    exit 1
  fi
  if [ "$MATCH_COUNT" -gt 1 ]; then
    printf "  [?] Ambiguous target '%s'. Matches multiple entries:\n" "$TARGET"
    printf '%s\n' "${MATCHES[@]}" | jq -r '    * \(.Id)'
    exit 1
  fi
  read -r _folder target_file <<< "$(target_paths "${MATCHES[0]}")"
  if [ -f "$target_file" ]; then
    printf '  [CLEANUP] Removing downloaded asset file: %s\n' "$target_file"
    rm -f "$target_file"
  else
    printf '  [CLEANUP] File does not exist: %s\n' "$target_file"
  fi
  printf '  [CLEANUP] Purge completed successfully.\n'
  exit 0
fi

# --- menu ---------------------------------------------------------------------
show_arcade_menu() {
  local count idx=1 entry folder file status id name
  count="$(arcade_target_count)"
  printf '\n   === Retro Console Downloader - Target Registry ===\n'
  printf '   ================================================================\n\n'
  for ((i = 0; i < count; i++)); do
    entry="$(arcade_target_json "$i")"
    read -r _folder file <<< "$(target_paths "$entry")"
    if [ -f "$file" ]; then status="[CACHED]"; else status="[      ]"; fi
    id="$(printf '%s' "$entry" | jq -r '.Id')"
    name="$(printf '%s' "$entry" | jq -r '.Name')"
    printf '   [%2d] %s  %-18s %s\n' "$idx" "$status" "$id" "$name"
    idx=$((idx + 1))
  done
  printf '\n   [CACHED] = Asset downloaded & verified\n'
  printf "   Usage: Select a number, enter a Target ID, type 'list', or 'exit'.\n\n"
}

# --- list command ---------------------------------------------------------------
if [ "$TARGET" = "list" ]; then
  count="$(arcade_target_count)"
  printf '\n  Registered Retro Console Targets\n'
  printf '  ================================================================\n'
  for ((i = 0; i < count; i++)); do
    entry="$(arcade_target_json "$i")"
    read -r _folder file <<< "$(target_paths "$entry")"
    if [ -f "$file" ]; then status="CACHED"; else status="MISSING"; fi
    printf '  * ID: %-16s Type: %-6s Status: %-8s Name: %s\n' \
      "$(printf '%s' "$entry" | jq -r '.Id')" \
      "$(printf '%s' "$entry" | jq -r '.Type')" \
      "$status" \
      "$(printf '%s' "$entry" | jq -r '.Name')"
  done
  printf '\n'
  exit 0
fi

# --- pipeline execution engine --------------------------------------------------
# Run-DownloadPipeline -> run_download_pipeline <target-json>
run_download_pipeline() {
  local entry="$1" folder dest_file dest_folder
  read -r folder dest_file <<< "$(target_paths "$entry")"
  dest_folder="$folder"
  mkdir -p "$dest_folder"

  printf '\n  Console Downloader Pipeline\n'
  printf '  Target: %s\n' "$(printf '%s' "$entry" | jq -r '.Name')"
  printf '  ================================================================\n'

  # Phase 1: stream download (skip if cached)
  if [ -f "$dest_file" ]; then
    printf '  [OK] Asset already cached locally at: %s\n' "$dest_file"
  else
    if ! download_stream "$(printf '%s' "$entry" | jq -r '.Url')" "$dest_file"; then
      printf '  [FAIL] Download execution aborted.\n'
      exit 1
    fi
  fi

  # Phase 2: cryptographic hash verification
  if ! verify_file_hash "$dest_file" \
      "$(printf '%s' "$entry" | jq -r '.ExpectedHash')" \
      "$(printf '%s' "$entry" | jq -r '.HashAlgorithm')"; then
    printf '  [FAIL] Cryptographic integrity verification failed! Deleting corrupted file.\n'
    rm -f "$dest_file"
    exit 1
  fi

  # Phase 3: extraction (optional; no current manifest entries set Extract)
  if [ "$(printf '%s' "$entry" | jq -r '.Extract // false')" = "true" ]; then
    local id extract_folder extract_file src target
    id="$(printf '%s' "$entry" | jq -r '.Id')"
    extract_folder="$dest_folder/extracted_$id"
    if expand_zip_file "$dest_file" "$extract_folder"; then
      extract_file="$(printf '%s' "$entry" | jq -r '.ExtractFile // empty')"
      if [ -n "$extract_file" ]; then
        src="$extract_folder/$extract_file"
        target="$dest_folder/$extract_file"
        if [ -f "$src" ]; then
          mv -f "$src" "$target"
          printf '  [OK] Extracted ROM file moved to target: %s\n' "$target"
        fi
      fi
      rm -rf "$extract_folder"
    fi
  fi

  printf '\n  [OK] Pipeline completed successfully for: %s!\n\n' \
    "$(printf '%s' "$entry" | jq -r '.Id')"
}

# --- interactive CLI loop -------------------------------------------------------
if [ -z "$TARGET" ]; then
  while true; do
    clear
    show_arcade_menu
    count="$(arcade_target_count)"
    read -rp "   Arcade [1-$count, target-id, list, exit] " choice
    [ -z "$choice" ] && continue
    choice="$(printf '%s' "$choice" | tr '[:upper:]' '[:lower:]' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"

    case "$choice" in
      exit|q)
        printf '   Exiting Console Downloader. Goodbye!\n'
        exit 0
        ;;
      list)
        clear
        printf '\n  Registered Targets:\n'
        for ((i = 0; i < count; i++)); do
          entry="$(arcade_target_json "$i")"
          printf '     * %s -- %s\n' "$(printf '%s' "$entry" | jq -r '.Id')" "$(printf '%s' "$entry" | jq -r '.Name')"
        done
        printf '\n'
        read -rp "   Press Enter to return to menu..." _
        continue
        ;;
    esac

    selected=""
    if [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le "$count" ]; then
      selected="$(arcade_target_json $((choice - 1)))"
    else
      lookup_targets "$choice"
      if [ "$MATCH_COUNT" -eq 0 ]; then
        printf "   [FAIL] Unknown target: '%s'\n" "$choice"
        sleep 1.5
        continue
      elif [ "$MATCH_COUNT" -gt 1 ]; then
        printf '   [?] Ambiguous -- matches multiple targets:\n'
        printf '%s\n' "${MATCHES[@]}" | jq -r '     * \(.Id)'
        read -rp "   Press Enter to try again..." _
        continue
      else
        selected="${MATCHES[0]}"
      fi
    fi

    run_download_pipeline "$selected"
    read -rp "   Pipeline completed. Press Enter to continue..." _
  done
else
  # --- direct command mode ------------------------------------------------------
  lookup_targets "$TARGET"
  if [ "$MATCH_COUNT" -eq 0 ]; then
    printf "  [FAIL] Target '%s' not found in registry.\n" "$TARGET"
    exit 1
  fi
  if [ "$MATCH_COUNT" -gt 1 ]; then
    printf "  [?] Ambiguous -- matches multiple targets:\n"
    printf '%s\n' "${MATCHES[@]}" | jq -r '    * \(.Id)'
    exit 1
  fi
  run_download_pipeline "${MATCHES[0]}"
fi
