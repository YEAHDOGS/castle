#!/usr/bin/env bash
# ==============================================================================
# Retro Console Downloader - Target Registry Manifest (bash twin)
# ==============================================================================
# Bash twin of targets.ps1. The PS original keeps a $ArcadeMatrix array of
# hashtables; here the manifest is JSON and helpers expose it via jq.
# "Registry" here means the download-target registry manifest -- it has
# nothing to do with the Windows Registry.
#
# Usage:
#   source modules/targets.sh
#   arcade_target_count            # -> 4
#   arcade_target_json <index>     # -> target object as JSON (0-based)
#   arcade_find_targets <query>    # -> matching targets, one JSON object/line
#                                   #    exact Id match first, else substring
#                                   #    match on Id or Name (case-insensitive)
# ==============================================================================

# Manifest JSON. Field-for-field identical to $ArcadeMatrix in targets.ps1.
ARCADE_MATRIX_JSON='[
  {
    "Id": "ps1-bios-5501",
    "Name": "PlayStation 1 BIOS (SCPH-5501)",
    "Description": "Required system firmware for North American region PS1 emulation.",
    "Url": "https://archive.org/download/bios_batocera/scph5501.bin",
    "File": "scph5501.bin",
    "HashAlgorithm": "MD5",
    "ExpectedHash": "490f666e1afb15b7362b406ed1cea246",
    "Type": "bios",
    "Platform": "playstation"
  },
  {
    "Id": "ps1-bios-1001",
    "Name": "PlayStation 1 BIOS (SCPH-1001)",
    "Description": "Required system firmware for early North American region PS1 emulation.",
    "Url": "https://archive.org/download/bios_batocera/scph1001.bin",
    "File": "scph1001.bin",
    "HashAlgorithm": "MD5",
    "ExpectedHash": "dc2b9bf8da62ec93e868cfd29f0d067d",
    "Type": "bios",
    "Platform": "playstation"
  },
  {
    "Id": "gba-bios",
    "Name": "Game Boy Advance BIOS",
    "Description": "Required system firmware for GBA emulation.",
    "Url": "https://archive.org/download/gba_bios/gba_bios.bin",
    "File": "gba_bios.bin",
    "HashAlgorithm": "MD5",
    "ExpectedHash": "a860e8c0b6d573d191e4ec7db1b1e4f6",
    "Type": "bios",
    "Platform": "gba"
  },
  {
    "Id": "nes-alterego",
    "Name": "Alter Ego (NES Homebrew)",
    "Description": "A popular logic-based puzzle platformer NES homebrew game.",
    "Url": "https://archive.org/download/pouet_71667/Alter_Ego.nes",
    "File": "Alter_Ego.nes",
    "HashAlgorithm": "MD5",
    "ExpectedHash": "73d0e0b0147a1f2371c10b26db3292e1",
    "Type": "rom",
    "Platform": "nes"
  }
]'

# Number of registered targets.
arcade_target_count() {
  printf '%s' "$ARCADE_MATRIX_JSON" | jq -r 'length'
}

# Print the target at 0-based index as compact JSON.
arcade_target_json() {
  local idx="$1"
  printf '%s' "$ARCADE_MATRIX_JSON" | jq -c ".[$idx]"
}

# Find targets by query: exact Id match wins; otherwise substring match on
# Id or Name (case-insensitive). Mirrors the PS matching logic in
# consoles.ps1 (Id -eq, or Name.ToLower() -like "*query*").
# Prints one compact JSON object per line; exit 0 always (caller counts lines).
arcade_find_targets() {
  local query="$1"
  local clean
  clean="$(printf '%s' "$query" | tr '[:upper:]' '[:lower:]' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  local exact
  # PS uses -eq (case-insensitive) for the Id fast path.
  exact="$(printf '%s' "$ARCADE_MATRIX_JSON" | jq -c --arg q "$clean" '.[] | select((.Id | ascii_downcase) == $q)')"
  if [ -n "$exact" ]; then
    printf '%s\n' "$exact"
    return 0
  fi
  printf '%s' "$ARCADE_MATRIX_JSON" | jq -c --arg q "$clean" \
    '.[] | select((.Id | ascii_downcase | contains($q)) or (.Name | ascii_downcase | contains($q)))'
}
