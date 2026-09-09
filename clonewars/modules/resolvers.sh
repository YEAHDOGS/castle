#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- Dynamic Version Discovery Engine (bash twin)
# ==============================================================================
# Bash twin of modules/resolvers.ps1. Resolves available versions of a target
# OS from remote sources (GitHub Releases API, HTML directory listings),
# prompts for a version, and instantiates *Template fields.
#
#   get_html_directory_versions <url> <regex> [filter]
#       # versions on stdout, one per line, latest first
#   parse_html_versions <regex> [filter]      # same, HTML on stdin (testable)
#   get_github_release_versions <repo>        # tag names on stdout
#   filter_github_release_tags                # same, API JSON on stdin
#   resolve_github_release_asset <target-json> <version> [release-json]
#       # updated target JSON on stdout; rc 1 on failure. If release-json is
#       # omitted it is fetched from the GitHub API (needs network).
#   invoke_version_prompt <target-json> <v1> [v2 ...]
#       # menu -> stderr, chosen version -> stdout
#   resolve_target_version <target-json>      # resolved target JSON
#
# Interactive prompts print to stderr and return values on stdout, mirroring
# how the PS functions Write-Host to the console but return values to the
# caller.
# ==============================================================================

_RESOLVERS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_CW_DATA_DIR="$(dirname "$_RESOLVERS_ROOT")/data"
_GITHUB_UA="CastleVM-Provisioner"

# --- internal: natural-sort key (PS pads each digit run to 10 chars) -----------
_natural_key() {
  python3 -c '
import re, sys
for line in sys.stdin:
    v = line.rstrip("\n")
    print(re.sub(r"\d+", lambda m: m.group(0).zfill(10), v) + "\t" + v)
'
}

# Get-HtmlDirectoryVersions -> get_html_directory_versions / parse_html_versions
parse_html_versions() {
  local regex="$1"
  local filter="${2:-}"
  python3 -c '
import re, sys
regex, filtr = sys.argv[1], sys.argv[2]
html = sys.stdin.read()
try:
    versions = list(dict.fromkeys(m.group(1) for m in re.finditer(regex, html)))
except re.error as e:
    print(f"  [FAIL] Failed to scrape HTML directory: {e}")
    sys.exit(0)
if filtr:
    try:
        versions = [v for v in versions if re.search(filtr, v)]
    except re.error as e:
        print(f"  [FAIL] Failed to scrape HTML directory: {e}")
        sys.exit(0)
def key(v):
    return re.sub(r"\d+", lambda m: m.group(0).zfill(10), v)
for v in sorted(versions, key=key, reverse=True):
    print(v)
' "$regex" "$filter" || {
    printf '  [FAIL] Failed to scrape HTML directory\n'
    return 0
  }
}

get_html_directory_versions() {
  local url="$1" regex="$2" filter="${3:-}"
  local html
  if ! html="$(curl -sSL "$url" 2>/dev/null)"; then
    printf '  [FAIL] Failed to scrape HTML directory: curl error\n'
    return 0
  fi
  printf '%s' "$html" | parse_html_versions "$regex" "$filter"
}

# Get-GitHubReleaseVersions -> get_github_release_versions / filter_github_release_tags
filter_github_release_tags() {
  # Basic GitHub API read without auth (rate limited to 60/hr) -- same as PS.
  jq -r '.[] | select(.prerelease == false and .draft == false) | .tag_name' 2>/dev/null || {
    printf '  [FAIL] Failed to query GitHub API\n'
    return 0
  }
}

get_github_release_versions() {
  local repo="$1"
  local json
  if ! json="$(curl -sSL -H "User-Agent: $_GITHUB_UA" "https://api.github.com/repos/$repo/releases" 2>/dev/null)"; then
    printf '  [FAIL] Failed to query GitHub API for %s\n' "$repo"
    return 0
  fi
  printf '%s' "$json" | filter_github_release_tags || \
    printf '  [FAIL] Failed to query GitHub API for %s\n' "$repo"
}

# Resolve-GitHubReleaseAsset -> resolve_github_release_asset
# Some repos name assets with per-device dates/codenames that don't match the
# release tag, so the asset is matched by ResolverAssetRegex against the
# release asset list; companion checksum assets and GitHub's own SHA256
# digest are wired up as extra verification layers.
resolve_github_release_asset() {
  local target="$1" version="$2" release_json="${3:-}"
  local repo
  repo="$(printf '%s' "$target" | jq -r '.ResolverRepo')"
  if [ -z "$release_json" ]; then
    if ! release_json="$(curl -sSL -H "User-Agent: $_GITHUB_UA" \
        "https://api.github.com/repos/$repo/releases/tags/$version" 2>/dev/null)"; then
      printf "  [FAIL] Failed to query GitHub release '%s' for %s\n" "$version" "$repo"
      return 1
    fi
  fi
  RESOLVE_TARGET="$target" python3 -c '
import json, os, re, sys
target = json.loads(os.environ["RESOLVE_TARGET"])
release = json.loads(sys.stdin.read())
rx = target.get("ResolverAssetRegex", "")
assets = release.get("assets", [])
asset = next((a for a in assets if re.search(rx, a.get("name", ""))), None)
if not asset:
    print(f"  [FAIL] No asset in release matches pattern: {rx}", file=sys.stderr)
    sys.exit(1)
target["Url"] = asset["browser_download_url"]
target["IsoName"] = asset["name"]
print(f"  [OK] Resolved release asset: {asset["name"]}", file=sys.stderr)
companions = {".sha256": "HashUrlSha256", ".sha512": "HashUrlSha512", ".md5": "HashUrlMd5"}
by_name = {a.get("name"): a for a in assets}
for suffix, key in companions.items():
    c = by_name.get(asset["name"] + suffix)
    if c:
        target[key] = c["browser_download_url"]
        print(f"     [+] Found companion checksum asset: {c["name"]}", file=sys.stderr)
digest = asset.get("digest") or ""
m = re.match(r"^sha256:([a-fA-F0-9]{64})$", digest)
if m:
    target["ExpectedHash"] = m.group(1)
    target["HashAlgorithm"] = "SHA256"
    print("     [+] Pinned GitHub API asset digest (SHA256).", file=sys.stderr)
print(json.dumps(target))
' <<< "$release_json"
}

# --- internal: disk-status label for the version prompt -------------------------
# Mirrors Get-DiskStatus in the PS Invoke-VersionPrompt.
_disk_status_label() {
  local target="$1" ver="$2" file file_template target_id
  file="$(printf '%s' "$target" | jq -r '.File // empty')"
  file_template="$(printf '%s' "$target" | jq -r '.FileTemplate // empty')"
  target_id="$(printf '%s' "$target" | jq -r '.Id')"

  local disk_name iso_name
  if [ -n "$file" ]; then
    disk_name="$(printf '%s' "$file" | sed -E 's/\.(iso|img\.gz|zip)$/.qcow2/')"
  elif [ -n "$file_template" ]; then
    disk_name="$(printf '%s' "$file_template" | sed "s/\\\$v/$ver/g" | sed -E 's/\.(iso|img\.gz|zip)$/.qcow2/')"
  else
    disk_name="$target_id.qcow2"
  fi
  if [ -n "$file" ]; then
    iso_name="$(printf '%s' "$file" | sed -E 's/\.qcow2$//')"
  elif [ -n "$file_template" ]; then
    iso_name="$(printf '%s' "$file_template" | sed "s/\\\$v/$ver/g" | sed -E 's/\.qcow2$/.iso/')"
  else
    iso_name="$target_id.iso"
  fi

  local labels=""
  [ -e "$_CW_DATA_DIR/$iso_name" ] && labels="$labels [ISO]"
  [ -e "$_CW_DATA_DIR/$disk_name" ] && labels="$labels [DISK]"
  printf '%s' "$labels"
}

# Invoke-VersionPrompt -> invoke_version_prompt
invoke_version_prompt() {
  local target="$1"; shift
  local -a versions=("$@")
  local name
  name="$(printf '%s' "$target" | jq -r '.Name')"

  if [ "${#versions[@]}" -eq 0 ]; then
    printf '  [FAIL] No versions discovered.\n' >&2
    return 1
  fi

  {
    printf '\n  [?] Multiple versions discovered for %s\n' "$name"
    printf '  -------------------------------------------------\n'
    printf '  [0] Latest (%s)%s\n' "${versions[0]}" "$(_disk_status_label "$target" "${versions[0]}")"
    local max_idx i
    max_idx=$(( ${#versions[@]} - 1 ))
    [ "$max_idx" -gt 5 ] && max_idx=5
    for ((i = 1; i <= max_idx; i++)); do
      printf '  [%d] %s%s\n' "$i" "${versions[i]}" "$(_disk_status_label "$target" "${versions[i]}")"
    done
    printf '\n'
  } >&2

  local input_raw
  read -rp "  Select a version number [Default: 0] " input_raw >&2
  if [ -z "$input_raw" ]; then
    printf '%s\n' "${versions[0]}"
    return 0
  fi
  local max_idx=$(( ${#versions[@]} - 1 ))
  [ "$max_idx" -gt 5 ] && max_idx=5
  if [[ "$input_raw" =~ ^[0-9]+$ ]] && [ "$input_raw" -ge 0 ] && [ "$input_raw" -le "$max_idx" ]; then
    printf '%s\n' "${versions[$input_raw]}"
  else
    printf '  [FAIL] Invalid selection. Defaulting to Latest.\n' >&2
    printf '%s\n' "${versions[0]}"
  fi
}

# Resolve-TargetVersion -> resolve_target_version
# Static targets (no ResolverType) pass through untouched.
resolve_target_version() {
  local target="$1"
  local resolver_type
  resolver_type="$(printf '%s' "$target" | jq -r '.ResolverType // empty')"
  if [ -z "$resolver_type" ]; then
    printf '%s\n' "$target"
    return 0
  fi

  local name
  name="$(printf '%s' "$target" | jq -r '.Name')"
  printf '  [>] Discovering available versions for %s...\n' "$name" >&2

  local -a versions=()
  case "$resolver_type" in
    HtmlDirectory)
      mapfile -t versions < <(get_html_directory_versions \
        "$(printf '%s' "$target" | jq -r '.ResolverUrl')" \
        "$(printf '%s' "$target" | jq -r '.ResolverRegex')" \
        "$(printf '%s' "$target" | jq -r '.ResolverFilter // empty')")
      ;;
    GitHub|GitHubAsset)
      mapfile -t versions < <(get_github_release_versions \
        "$(printf '%s' "$target" | jq -r '.ResolverRepo')")
      ;;
  esac

  if [ "${#versions[@]}" -eq 0 ]; then
    printf '  [FAIL] Could not resolve any versions for %s.\n' "$name" >&2
    return 1
  fi

  local selected
  selected="$(invoke_version_prompt "$target" "${versions[@]}")" || return 1
  printf '  [OK] Selected version: %s\n' "$selected" >&2

  # Instantiate *Template keys: UrlTemplate -> Url with $v replaced, etc.
  # (The Template keys themselves are kept, like the PS hashtable clone.)
  local resolved
  resolved="$(printf '%s' "$target" | jq -c --arg v "$selected" '
    . + with_entries(select(.key | endswith("Template"))
      | .key |= rtrimstr("Template")
      | .value |= gsub("\\$v"; $v))')"

  if [ "$resolver_type" = "GitHubAsset" ]; then
    resolved="$(resolve_github_release_asset "$resolved" "$selected")" || {
      printf '  [FAIL] Could not resolve a release asset for %s.\n' "$name" >&2
      return 1
    }
  fi

  printf '%s\n' "$resolved"
}
