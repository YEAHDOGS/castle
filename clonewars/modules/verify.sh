#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- Cryptographic Integrity Verification Engine (bash twin)
# ==============================================================================
# Bash twin of modules/verify.ps1. Three verification layers:
#   1. verify_crypto_hash  -- SHA256/SHA512/SHA1/MD5 comparison (curl replaces
#                             the PS HttpClient for remote manifests)
#   2. verify_gpg_signature -- detached GPG signature verification (gpg CLI)
#   3. verify_iso_integrity -- high-level wrapper combining both, with HMAC-
#                             pinned local trust store (openssl replaces .NET)
#
# Manifest formats supported (same as PS): remote hash manifests, static
# hardcoded hashes, archive.org _files.xml, "hash  filename", BSD/RHEL
# "ALGO(filename) = hash".
#
# verify_crypto_hash contract: rc 0 = verified (computed hash on stdout),
#   rc 1 = mismatch, rc 2 = aborted ($null in PS: no hash / file missing).
# verify_iso_integrity: rc 0 = trusted, rc 1 = failed.
# ==============================================================================

_VERIFY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_CW_DATA_DIR="$(dirname "$_VERIFY_ROOT")/data"

# --- file hashing (duplicated from the arcade twin, like the PS original ----
# --- duplicates Get-FileHashDotNet; clonewars default is SHA512) -------------
_verify_hash_algo() {
  case "$(printf '%s' "${1:-SHA512}" | tr '[:lower:]' '[:upper:]')" in
    SHA512) printf 'sha512' ;;
    SHA256) printf 'sha256' ;;
    MD5)    printf 'md5' ;;
    SHA1)   printf 'sha1' ;;
    *)      printf 'sha256' ;;
  esac
}

# Get-FileHashDotNet -> cw_file_hash
cw_file_hash() {
  local path="$1" algo="${2:-SHA512}" algo_lc
  algo_lc="$(_verify_hash_algo "$algo")"
  case "$algo_lc" in
    sha512) sha512sum "$path" | awk '{print $1}' ;;
    sha256) sha256sum "$path" | awk '{print $1}' ;;
    md5)    md5sum "$path" | awk '{print $1}' ;;
    sha1)   sha1sum "$path" | awk '{print $1}' ;;
  esac | tr '[:upper:]' '[:lower:]'
}

# --- HMAC (same key semantics as the PS: UTF-8 bytes of the hex string) -------
# Get-HmacSha256 -> cw_hmac_sha256
cw_hmac_sha256() {
  local data="$1" hexkey="$2"
  printf '%s' "$data" | openssl dgst -sha256 -hmac "$hexkey" | awk '{print $NF}'
}

# Get-OrCreateHmacKey -> get_or_create_hmac_key
# PS tries DPAPI decryption first and falls back to plaintext on Linux
# containers; this twin keeps the Linux behavior (0600 plaintext file).
get_or_create_hmac_key() {
  local key_file="$1" key=""
  if [ -f "$key_file" ]; then
    key="$(tr -d ' \t\r\n' < "$key_file")"
    if [ "${#key}" -eq 64 ]; then
      printf '%s\n' "$key"
      return 0
    fi
  fi
  key="$(openssl rand -hex 32)"
  printf '%s' "$key" > "$key_file"
  chmod 600 "$key_file"
  printf '%s\n' "$key"
}

# --- internal: extract expected hash from a fetched manifest -------------------
# Mirrors the PS Attempt 1 (XML) + Attempt 2 (text patterns) logic.
# Args: <algorithm> <isoname> <static-expected> <xml-target> ; manifest on stdin.
# Prints the hash on stdout (empty if none found); diagnostics on stderr.
# Like the PS original, a passed-in static hash gates Attempt 2 (text), while
# Attempt 1 (XML) may still overwrite it.
_extract_manifest_hash() {
  local algo="$1" isoname="$2" static_expected="$3" xml_target="$4"
  # NOTE: the manifest arrives on stdin, but python's stdin is taken by the
  # heredoc below, so it is passed through the environment instead.
  local manifest
  manifest="$(cat)"
  MANIFEST_ALGO="$algo" MANIFEST_ISO="$isoname" \
  MANIFEST_STATIC="$static_expected" MANIFEST_XML_TARGET="$xml_target" \
  MANIFEST_DATA="$manifest" python3 <<'PYEOF'
import os, re, sys, xml.etree.ElementTree as ET
algo = os.environ["MANIFEST_ALGO"]
isoname = os.environ["MANIFEST_ISO"]
static_expected = os.environ["MANIFEST_STATIC"]
xml_target = os.environ["MANIFEST_XML_TARGET"]
data = os.environ["MANIFEST_DATA"]
found = ""

def done(msg=None):
    if msg:
        print(msg, file=sys.stderr)
    print(found)
    sys.exit(0)

# -- Attempt 1: XML manifest (Archive.org _files.xml) --
if data.lstrip().startswith("<"):
    try:
        root = ET.fromstring(data)
        for f in root.iter("file"):
            if f.get("name") == xml_target and f.get(algo.lower()):
                found = f.get(algo.lower())
                print(f"     [+] Found {algo} hash in XML manifest for: {xml_target}", file=sys.stderr)
                break
    except ET.ParseError:
        print("     [?] XML parsing failed, falling back to text matching...", file=sys.stderr)

# -- Attempt 2: text manifest (only if XML did not yield a hash AND no static
# -- hash was passed in -- mirrors the PS IsNullOrEmpty($ExpectedHash) gate)
if not found and not static_expected:
    if isoname:
        esc = re.escape(isoname)
        pat = (r"(?mi)(?:^([a-fA-F0-9]+)\s+\*?" + esc + r"\s*$"
               r"|^\w+\s*\(" + esc + r"\)\s*=\s*([a-fA-F0-9]+)\s*$)")
        m = re.search(pat, data)
        if m:
            found = m.group(1) or m.group(2)
        else:
            length = {"MD5": 32, "SHA1": 40, "SHA256": 64, "SHA512": 128}.get(algo.upper(), 64)
            p1 = r"(?is)" + esc + r".{1,250}?([a-fA-F0-9]{" + str(length) + r"})"
            p2 = r"(?is)([a-fA-F0-9]{" + str(length) + r"}).{1,250}?" + esc
            m1 = re.search(p1, data)
            m2 = re.search(p2, data)
            if m1:
                found = m1.group(1)
                print(f"     [+] Found {algo} hash via proximity fallback near filename: {found}", file=sys.stderr)
            elif m2:
                found = m2.group(1)
                print(f"     [+] Found {algo} hash via proximity fallback near filename: {found}", file=sys.stderr)
            elif re.match(r"^([a-fA-F0-9]{" + str(length) + r"})$", data.strip()):
                found = re.match(r"^([a-fA-F0-9]{" + str(length) + r"})$", data.strip()).group(1)
                print(f"     [+] Manifest is a bare {algo} checksum file: {found}", file=sys.stderr)
            else:
                print(f"     [?] File '{isoname}' not found in remote manifest.", file=sys.stderr)
    else:
        m = re.search(r"([a-fA-F0-9]{32,128})", data)
        if m:
            found = m.group(1)
done()
PYEOF
}

# Test-CryptographicHash -> verify_crypto_hash
# Usage: verify_crypto_hash <file> [expected] [hashurl] [algo] [isoname]
verify_crypto_hash() {
  local file_path="$1" expected="${2:-}" hash_url="${3:-}"
  local algo="${4:-SHA512}" iso_name="${5:-}"

  printf '  [HASH] Verifying %s integrity...\n' "$algo"

  if [ -n "$hash_url" ]; then
    local fetched
    if fetched="$(curl -sSL "$hash_url" 2>/dev/null)"; then
      local preview
      preview="$(printf '%s' "$fetched" | head -c 200 | tr -d '\r' | tr '\n' ' ')"
      printf '     [>] Fetching remote manifest from: %s\n' "$hash_url"
      printf '     [+] Retrieved content from link (first 200 chars): %s\n' "${preview:-empty}"
      local xml_target="$iso_name"
      [ -z "$xml_target" ] && xml_target="$(basename "$file_path")"
      local extracted
      # stderr flows straight through; stdout (the hash) is captured
      extracted="$(_extract_manifest_hash "$algo" "$iso_name" "$expected" "$xml_target" <<< "$fetched")"
      if [ -n "$extracted" ]; then
        expected="$extracted"
      fi
    else
      printf '     [?] Remote hash lookup failed: curl error\n'
    fi
  fi

  if [ -z "$expected" ]; then
    printf '  [FAIL] No valid hash found. Verification aborted.\n'
    return 2
  fi
  if [ ! -f "$file_path" ]; then
    printf '  [FAIL] File not found: %s\n' "$file_path"
    return 2
  fi

  local computed clean
  computed="$(cw_file_hash "$file_path" "$algo")"
  clean="$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]' | tr -d ' \t\r\n')"
  if [ "$computed" = "$clean" ]; then
    printf '  [OK] %s verified (%s)\n' "$algo" "$computed"
    printf '%s\n' "$computed"
    return 0
  fi
  printf '  [FAIL] HASH MISMATCH!\n'
  printf '     Computed: %s\n' "$computed"
  printf '     Expected: %s\n' "$clean"
  return 1
}

# Test-GpgSignature -> verify_gpg_signature
# Usage: verify_gpg_signature <target-file> [sigurl] [gpgkey] [gpgserver]
verify_gpg_signature() {
  local target_file="$1" sig_url="${2:-}" gpg_key="${3:-}" gpg_server="${4:-}"

  if ! command -v gpg >/dev/null 2>&1; then
    printf '  [?] GPG not found on PATH. Skipping signature verification.\n'
    printf '     Tip: install gnupg from your package manager.\n'
    return 0
  fi
  if [ -z "$sig_url" ] || [ -z "$gpg_key" ]; then
    printf '  [i] No GPG signature configured for this target.\n'
    return 0
  fi

  local sig_file="$target_file.sig"
  printf '  [GPG] Verifying signature...\n'
  printf '     [>] Fetching key [%s] from %s\n' "$gpg_key" "$gpg_server"
  gpg --keyserver "$gpg_server" --recv-keys "$gpg_key" >/dev/null 2>&1

  if ! curl -sSL --fail -o "$sig_file" "$sig_url" 2>/dev/null; then
    printf '  [FAIL] GPG verification error: could not download signature\n'
    rm -f "$sig_file"
    return 1
  fi

  local gpg_out
  gpg_out="$(gpg --verify "$sig_file" "$target_file" 2>&1)"
  rm -f "$sig_file"

  if printf '%s' "$gpg_out" | grep -q "Good signature"; then
    printf '  [OK] GPG signature verified -- authentic.\n'
    return 0
  fi
  printf '  [FAIL] GPG SIGNATURE REJECTED!\n'
  printf '     %s\n' "$gpg_out"
  return 1
}

# Get-PinnedTrustStore -> get_pinned_trust_store (prints trust JSON object)
get_pinned_trust_store() {
  local trust_file="$_CW_DATA_DIR/.castle_trust.json"
  local sig_file="$_CW_DATA_DIR/.castle_trust.sig"
  local home_dir="${USERPROFILE:-$HOME}"
  local key_file="$home_dir/.castle_vm_key"

  if [ -f "$trust_file" ]; then
    local json_data key calc_sig stored_sig
    json_data="$(tr -d '\r\n' < "$trust_file" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    key="$(get_or_create_hmac_key "$key_file")"
    calc_sig="$(cw_hmac_sha256 "$json_data" "$key")"
    stored_sig=""
    [ -f "$sig_file" ] && stored_sig="$(tr -d ' \t\r\n' < "$sig_file" | tr '[:upper:]' '[:lower:]')"
    if [ "$calc_sig" = "$stored_sig" ]; then
      printf '%s\n' "$json_data" | jq -c '.' 2>/dev/null || printf '{}\n'
      return 0
    fi
    printf '  [SECURITY WARNING] Local trust store signature verification failed!\n'
    printf '     The trust store has been tampered with or is corrupted. Ignoring local cache.\n'
    rm -f "$trust_file" "$sig_file"
  fi
  printf '{}\n'
}

# Save-PinnedTrustStore -> save_pinned_trust_store <trust-json>
save_pinned_trust_store() {
  local trust_store="$1"
  local trust_file="$_CW_DATA_DIR/.castle_trust.json"
  local sig_file="$_CW_DATA_DIR/.castle_trust.sig"
  local home_dir="${USERPROFILE:-$HOME}"
  local key_file="$home_dir/.castle_vm_key"

  local json_data key sig
  json_data="$(printf '%s' "$trust_store" | jq -c -S '.' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  printf '%s' "$json_data" > "$trust_file"
  key="$(get_or_create_hmac_key "$key_file")"
  sig="$(cw_hmac_sha256 "$json_data" "$key")"
  printf '%s' "$sig" > "$sig_file"
}

# Test-IsoIntegrity -> verify_iso_integrity <target-json> <file-path>
verify_iso_integrity() {
  local target="$1" file_path="$2"

  if [ "$(printf '%s' "$target" | jq -r '.IsFolder // false')" = "true" ]; then
    printf '  [i] Skipping integrity verification for folder target: %s\n' \
      "$(printf '%s' "$target" | jq -r '.Name')"
    return 0
  fi

  local iso_name
  iso_name="$(printf '%s' "$target" | jq -r '.File // empty')"
  [ -z "$iso_name" ] && iso_name="$(basename "$file_path")"

  # -- pinned trust store --
  local trust_store pinned
  trust_store="$(get_pinned_trust_store)"
  pinned="$(printf '%s' "$trust_store" | jq -r --arg k "$iso_name" '.[$k] // empty')"
  if [ -n "$pinned" ]; then
    printf '  [TRUST PIN] Validating against pinned local trust store...\n'
    local computed
    computed="$(cw_file_hash "$file_path" "SHA512")"
    if [ "$computed" = "$pinned" ]; then
      printf '  [OK] Trust Pinning verification passed. ISO matches pinned state.\n'
      return 0
    fi
    printf '  [FAIL] TRUST PIN MISMATCH! The cached ISO differs from the initially trusted version.\n'
    return 1
  fi

  printf '  [i] No pinned hash found. Running remote integrity checks...\n'

  local hash_match=1 final_hash="" checks=0 result
  # -- Layer A: static hash --
  local expected_hash
  expected_hash="$(printf '%s' "$target" | jq -r '.ExpectedHash // empty')"
  if [ -n "$expected_hash" ]; then
    checks=$((checks + 1))
    local algo
    algo="$(printf '%s' "$target" | jq -r '.HashAlgorithm // "SHA256"')"
    if result="$(verify_crypto_hash "$file_path" "$expected_hash" "" "$algo" "")"; then
      final_hash="$result"
    else
      hash_match=0
    fi
  fi

  # -- Layer B: remote hash manifest URLs (multi-algorithm audit) --
  if [ "$hash_match" -eq 1 ]; then
    local keys key algo url
    keys="$(printf '%s' "$target" | jq -r 'keys[]' | grep '^HashUrl' | grep -v 'Template$' | sort)"
    for key in $keys; do
      checks=$((checks + 1))
      algo="${key#HashUrl}"
      url="$(printf '%s' "$target" | jq -r --arg k "$key" '.[$k]')"
      if result="$(verify_crypto_hash "$file_path" "" "$url" "$algo" \
          "$(printf '%s' "$target" | jq -r '.IsoName // empty')")"; then
        final_hash="$result"
      else
        hash_match=0
        break
      fi
    done
  fi

  if [ "$checks" -eq 0 ]; then
    printf '  [?] No hash verification data for this target. Proceeding unverified.\n'
  fi

  # -- GPG signature layer (runs only if hash passed) --
  if [ "$hash_match" -eq 1 ]; then
    local sig_url
    sig_url="$(printf '%s' "$target" | jq -r '.SigUrl // empty')"
    if [ -n "$sig_url" ]; then
      if ! verify_gpg_signature "$file_path" "$sig_url" \
          "$(printf '%s' "$target" | jq -r '.GpgKey // empty')" \
          "$(printf '%s' "$target" | jq -r '.GpgServer // empty')"; then
        return 1
      fi
    fi
  fi

  # -- pin verified hash --
  if [ "$hash_match" -eq 1 ] && [ -n "$final_hash" ]; then
    printf '  [TRUST PIN] Pinning verified hash to local trust store.\n'
    local pinned_hash
    pinned_hash="$(cw_file_hash "$file_path" "SHA512")"
    trust_store="$(printf '%s' "$trust_store" | jq -c --arg k "$iso_name" --arg v "$pinned_hash" '. + {($k): $v}')"
    save_pinned_trust_store "$trust_store"
  fi

  [ "$hash_match" -eq 1 ]
}
