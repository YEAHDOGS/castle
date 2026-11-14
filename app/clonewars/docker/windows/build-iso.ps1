# ==============================================================================
# Castle -- build data\win11-pro-docker.iso for the windows Docker profile
# ==============================================================================
# Turns the Windows install folder (clonewars\data\win11-pro, the same one
# start.ps1 uses for the QEMU win11-pro target) into an ISO that dockur/windows
# can install unattended. See build-iso.sh for why the stock Castle ISO is not
# usable as-is. Runs the build inside a throwaway dockurr/windows container, so
# nothing is installed on the host and no admin rights are needed.
#
#   .\build-iso.ps1                        # data\win11-pro -> data\win11-pro-docker.iso
#   .\build-iso.ps1 -Source D:\win11 -Output D:\win11-docker.iso
#
# docker.ps1 -Profile windows calls this automatically when the ISO is missing.
# Takes 5-15 minutes: ~2 GB of boot files are copied and a ~5 GB ISO written
# through Docker Desktop's file sharing.
# ==============================================================================
param (
    [string]$Source = (Join-Path (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent) "data\win11-pro"),
    [string]$Output = (Join-Path (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent) "data\win11-pro-docker.iso")
)
$ErrorActionPreference = "Stop"

if (-not (Test-Path (Join-Path $Source "sources"))) {
    Write-Host "  [FAIL] No Windows install folder at $Source (expected a sources\ subfolder)." -ForegroundColor Red
    Exit 1
}
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Host "  [FAIL] docker not found on PATH." -ForegroundColor Red
    Exit 1
}

$Source = (Resolve-Path $Source).Path
$OutDir = Split-Path $Output -Parent
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory $OutDir | Out-Null }
$OutDir = (Resolve-Path $OutDir).Path
$IsoName = Split-Path $Output -Leaf
$Script = Join-Path $PSScriptRoot "build-iso.sh"

Write-Host "  [ISO] Building $Output from $Source (inside a dockurr/windows container)..." -ForegroundColor Cyan
& docker run --rm --entrypoint bash `
    -v "${Source}:/src:ro" `
    -v "${OutDir}:/out" `
    -v "${Script}:/build-iso.sh:ro" `
    -e "ISO_NAME=$IsoName" `
    dockurr/windows /build-iso.sh
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $Output)) {
    Write-Host "  [FAIL] ISO build failed (exit $LASTEXITCODE)." -ForegroundColor Red
    Exit 1
}
$Gb = "{0:N2}" -f ((Get-Item $Output).Length / 1GB)
Write-Host "  [OK] $Output ($Gb GB)" -ForegroundColor Green
