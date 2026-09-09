#!/usr/bin/env bash
# ==============================================================================
# Hashing & Cryptographic Security Library Module (bash twin)
# ==============================================================================
# Bash twin of security.ps1. PS/.NET primitives map to CLI tools as follows:
#   Get-FileHashDotNet  -> sha256sum / md5sum / sha1sum / sha512sum /
#                          openssl dgst -sha384   (coreutils has no sha384sum)
#   Get-HmacSha256      -> openssl dgst -sha256 -hmac <key>
#                          (PS uses the UTF-8 bytes of the hex *string* as key)
#   Protect/Unprotect-FileAes -> openssl kdf (PBKDF2-HMAC-SHA1, 10000 iters,
#                          48 bytes: 32 key + 16 IV) + openssl enc -aes-256-cbc.
#                          File layout is identical to the PS version:
#                          16-byte raw salt prefix, then AES-256-CBC ciphertext
#                          with PKCS#7 padding. Files are cross-compatible
#                          between the .ps1 and .sh twins.
#
# Public functions:
#   file_hash <path> [algorithm]          # prints lowercase hex digest
#   verify_file_hash <path> <expected> [algorithm]  # 0=match, 1=mismatch/error
#   hmac_sha256 <data> <hexkey>          # prints lowercase hex HMAC
#   file_hmac_sha256 <path> <hexkey>     # prints lowercase hex HMAC of file
#   protect_file_aes <path> <password> [output]    # prints output path
#   unprotect_file_aes <path> <password> [output]  # prints output path
# ==============================================================================

# --- internal: hex helpers (no xxd dependency) --------------------------------
# bytes of stdin -> lowercase hex
_b2h() { od -A n -v -t x1 | tr -d ' \n'; }
# hex string $1 -> raw bytes on stdout
_h2b() { printf '%b' "$(printf '%s' "$1" | sed 's/../\\x&/g')"; }

# --- internal: hash algorithm handling ---------------------------------------
# Mirrors the PS switch: SHA256/MD5/SHA1/SHA512/SHA384, default SHA256.
# _security_hash_algo <name> -> prints canonical lowercase algo token.
_security_hash_algo() {
  case "$(printf '%s' "${1:-SHA256}" | tr '[:lower:]' '[:upper:]')" in
    SHA256) printf 'sha256' ;;
    MD5)    printf 'md5' ;;
    SHA1)   printf 'sha1' ;;
    SHA512) printf 'sha512' ;;
    SHA384) printf 'sha384' ;;
    *)      printf 'sha256' ;;
  esac
}

# Get-FileHashDotNet -> file_hash
# Prints the lowercase hex digest. Throws (non-zero exit) if file missing,
# mirroring the PS `throw "File not found"`.
file_hash() {
  local path="$1"
  local algo="${2:-SHA256}"
  if [ ! -f "$path" ]; then
    printf 'file_hash: file not found: %s\n' "$path" >&2
    return 1
  fi
  local algo_lc
  algo_lc="$(_security_hash_algo "$algo")"
  # coreutils for the common four (sha256sum/md5sum/sha1sum/sha512sum),
  # openssl for sha384 -- digest is always the last whitespace-separated field
  # of the tool output.
  case "$algo_lc" in
    sha256) sha256sum "$path" | awk '{print $1}' ;;
    md5)    md5sum "$path" | awk '{print $1}' ;;
    sha1)   sha1sum "$path" | awk '{print $1}' ;;
    sha512) sha512sum "$path" | awk '{print $1}' ;;
    *)      openssl dgst "-$algo_lc" "$path" | awk '{print $NF}' ;;
  esac | tr '[:upper:]' '[:lower:]'
}

# Test-FileHash -> verify_file_hash
# Returns 0 on match, 1 on mismatch or missing file. Logs like the PS original.
verify_file_hash() {
  local path="$1"
  local expected="$2"
  local algo="${3:-SHA256}"
  printf '  [SECURITY] Verifying %s hash of: %s...\n' "$algo" "$(basename "$path")"
  if [ ! -f "$path" ]; then
    printf '  [FAIL] File does not exist for validation: %s\n' "$path"
    return 1
  fi
  local computed clean_expected
  computed="$(file_hash "$path" "$algo")" || return 1
  clean_expected="$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  if [ "$computed" = "$clean_expected" ]; then
    printf '  [OK] Hash matches: %s\n' "$computed"
    return 0
  fi
  printf '  [FAIL] HASH MISMATCH!\n'
  printf '     Computed: %s\n' "$computed"
  printf '     Expected: %s\n' "$clean_expected"
  return 1
}

# Get-HmacSha256 -> hmac_sha256
# NOTE: like the PS original, the key is the UTF-8 bytes of the hex string
# itself (it is NOT hex-decoded first).
hmac_sha256() {
  local data="$1"
  local hexkey="$2"
  printf '%s' "$data" | openssl dgst -sha256 -hmac "$hexkey" | awk '{print $NF}'
}

# Get-FileHmacSha256 -> file_hmac_sha256
file_hmac_sha256() {
  local path="$1"
  local hexkey="$2"
  if [ ! -f "$path" ]; then
    printf 'file_hmac_sha256: file not found: %s\n' "$path" >&2
    return 1
  fi
  openssl dgst -sha256 -hmac "$hexkey" "$path" | awk '{print $NF}'
}

# --- internal: PBKDF2-HMAC-SHA1 key derivation (matches .NET Rfc2898DeriveBytes)
# .NET's Rfc2898DeriveBytes(string, salt, 10000) uses UTF-8 password bytes and
# HMAC-SHA1, and GetBytes(32)+GetBytes(16) pulls 48 sequential bytes.
# Prints "<keyhex> <ivhex>".
_security_derive_key_iv() {
  local password="$1"
  local salthex="$2"
  local passhex
  passhex="$(printf '%s' "$password" | _b2h)"
  local derived
  derived="$(printf '' | openssl kdf -keylen 48 \
    -kdfopt digest:SHA1 \
    -kdfopt "hexpass:${passhex}" \
    -kdfopt "hexsalt:${salthex}" \
    -kdfopt iter:10000 PBKDF2 2>/dev/null | tr -d ':\n' | tr '[:upper:]' '[:lower:]')"
  if [ "${#derived}" -ne 96 ]; then
    printf '_security_derive_key_iv: key derivation failed\n' >&2
    return 1
  fi
  printf '%s %s\n' "${derived:0:64}" "${derived:64:32}"
}

# Protect-FileAes -> protect_file_aes
# Output defaults to "<path>.aes". Layout: 16-byte salt || AES-256-CBC(PKCS7).
protect_file_aes() {
  local path="$1"
  local password="$2"
  local output="${3:-${path}.aes}"
  if [ ! -f "$path" ]; then
    printf 'protect_file_aes: file not found: %s\n' "$path" >&2
    return 1
  fi
  local salthex keyhex ivhex saltbin tmpout
  salthex="$(openssl rand -hex 16)"
  read -r keyhex ivhex < <(_security_derive_key_iv "$password" "$salthex") || return 1
  saltbin="$(mktemp)"; tmpout="$(mktemp)"
  _h2b "$salthex" > "$saltbin"
  if ! openssl enc -aes-256-cbc -K "$keyhex" -iv "$ivhex" -in "$path" -out "$tmpout" 2>/dev/null; then
    rm -f "$saltbin" "$tmpout"
    printf 'protect_file_aes: encryption failed\n' >&2
    return 1
  fi
  cat "$saltbin" "$tmpout" > "$output"
  rm -f "$saltbin" "$tmpout"
  printf '  [SECURITY] File encrypted successfully with AES-256: %s\n' "$output"
  printf '%s\n' "$output"
}

# Unprotect-FileAes -> unprotect_file_aes
# Output defaults to path with trailing ".aes" stripped. On bad password or
# corrupt data the partial output is removed and nothing is printed (PS
# returns $null) with a non-zero exit.
unprotect_file_aes() {
  local path="$1"
  local password="$2"
  local output="$3"
  if [ -z "$output" ]; then
    output="${path%.aes}"
  fi
  if [ ! -f "$path" ]; then
    printf 'unprotect_file_aes: file not found: %s\n' "$path" >&2
    return 1
  fi
  if [ "$(stat -c%s "$path")" -lt 16 ]; then
    printf 'unprotect_file_aes: invalid encrypted file: file size too small.\n' >&2
    return 1
  fi
  local salthex keyhex ivhex
  salthex="$(head -c 16 "$path" | _b2h)"
  read -r keyhex ivhex < <(_security_derive_key_iv "$password" "$salthex") || return 1
  if ! tail -c +17 "$path" | openssl enc -d -aes-256-cbc -K "$keyhex" -iv "$ivhex" -out "$output" 2>/dev/null; then
    printf '  [FAIL] AES Decryption failed (invalid password or corrupted data)\n'
    rm -f "$output"
    return 1
  fi
  printf '  [SECURITY] File decrypted successfully with AES-256: %s\n' "$output"
  printf '%s\n' "$output"
}
