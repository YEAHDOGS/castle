#!/usr/bin/env bash
# ==============================================================================
# Archive Utility Library Module (bash twin)
# ==============================================================================
# Bash twin of archive.ps1.
#   Expand-ZipFile    -> expand_zip_file    (Expand-Archive  -> unzip -o)
#   Compress-Directory -> compress_directory (Compress-Archive -> zip -r)
#
# Public functions (0 = success, 1 = failure; log lines mirror the PS output):
#   expand_zip_file <zip-path> <destination>
#   compress_directory <source-dir> <destination-zip>
# ==============================================================================

# Expand-ZipFile -> expand_zip_file
expand_zip_file() {
  local path="$1"
  local dest="$2"
  printf '  [ARCHIVE] Extracting archive: %s to %s...\n' "$(basename "$path")" "$dest"
  if [ ! -f "$path" ]; then
    printf '  [FAIL] ZIP archive not found at: %s\n' "$path"
    return 1
  fi
  mkdir -p "$dest"
  # -o: overwrite without prompting == Expand-Archive -Force
  if unzip -o -q "$path" -d "$dest" 2>/dev/null; then
    printf '  [OK] Extraction completed successfully.\n'
    return 0
  fi
  printf '  [FAIL] Failed to extract archive: %s\n' "$path"
  return 1
}

# Compress-Directory -> compress_directory
compress_directory() {
  local path="$1"
  local dest="$2"
  printf '  [ARCHIVE] Compressing directory: %s to %s...\n' "$path" "$dest"
  if [ ! -e "$path" ]; then
    printf '  [FAIL] Directory not found at: %s\n' "$path"
    return 1
  fi
  local dest_parent base src_parent
  dest_parent="$(dirname "$dest")"
  mkdir -p "$dest_parent"
  # Zip the basename from its parent dir so entries are stored relative,
  # like Compress-Archive does. Existing zip is refreshed (-Force).
  base="$(basename "$path")"
  src_parent="$(dirname "$path")"
  if ( cd "$src_parent" && zip -qr "$dest" "$base" 2>/dev/null ); then
    printf '  [OK] Compression completed successfully.\n'
    return 0
  fi
  printf '  [FAIL] Failed to compress directory: %s\n' "$path"
  return 1
}
