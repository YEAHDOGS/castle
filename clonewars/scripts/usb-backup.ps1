# ==============================================================================
# Castle / clonewars -- back up a plugged-in USB drive or SD card
# ==============================================================================
# Thin wrapper: the real tool lives in the phoenix repo (scripts/usb/usb-backup.ps1).
# Nothing is copied here; phoenix is sparse-fetched from GitHub at run time via
# phoenix-import.ps1 and the script runs from that cache.
#
#   .\usb-backup.ps1                        # checklist GUI: drives + outputs (Image / Folder / Iso / Zip / Burn)
#   .\usb-backup.ps1 -Disks 1 -Do Iso       # one burnable ISO, no admin needed
#   .\usb-backup.ps1 -Disks 1 -Do Image     # raw image (this one asks for admin; Windows requires it for raw reads)
#
# Pin: set $env:PHOENIX_REF to a tag/commit for a reproducible run; add -VerifySignature to require signed commits.
# ==============================================================================
param([Parameter(ValueFromRemainingArguments = $true)][object[]]$PassThru)

# TODO: switch to 'master' once the phoenix branch below is merged
$Ref = if ($env:PHOENIX_REF) { $env:PHOENIX_REF } else { 'claude/r36s-tools' }
$bootstrap = "https://raw.githubusercontent.com/YEAHDOGS/phoenix/$Ref/scripts/tools/phoenix-import.ps1"

if (-not (Get-Command Sync-PhoenixRepo -ErrorAction SilentlyContinue)) {
    Write-Host "[castle] loading phoenix-import from $bootstrap" -ForegroundColor DarkGray
    Invoke-Expression (Invoke-RestMethod "${bootstrap}?nocache=$([DateTime]::UtcNow.Ticks)")   # raw.githubusercontent caches ~5 min
}
# Fetch the repo once, then use the importer from the fetched tree itself so a stale CDN copy can never run.
$tree = Sync-PhoenixRepo -Ref $Ref -Paths @('scripts/tools', 'scripts/usb')
. (Join-Path $tree 'scripts/tools/phoenix-import.ps1')
Invoke-PhoenixScript 'scripts/usb/usb-backup.ps1' -Ref $Ref -Offline -Args $PassThru
