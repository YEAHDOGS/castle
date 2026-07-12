# ==============================================================================
# Archive Utility Library Module
# ==============================================================================

function Expand-ZipFile {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Path,
        [Parameter(Mandatory=$true)]
        [string]$DestinationPath
    )

    Write-Host "  [ARCHIVE] Extracting archive: $(Split-Path $Path -Leaf) to $DestinationPath..." -ForegroundColor Yellow

    if (-not (Test-Path $Path)) {
        Write-Host "  [FAIL] ZIP archive not found at: $Path" -ForegroundColor Red
        return $false
    }

    if (-not (Test-Path $DestinationPath)) {
        New-Item -ItemType Directory -Path $DestinationPath -Force | Out-Null
    }

    try {
        Expand-Archive -Path $Path -DestinationPath $DestinationPath -Force -ErrorAction Stop
        Write-Host "  [OK] Extraction completed successfully." -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "  [FAIL] Failed to extract archive: $_" -ForegroundColor Red
        return $false
    }
}

function Compress-Directory {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Path,
        [Parameter(Mandatory=$true)]
        [string]$DestinationPath
    )

    Write-Host "  [ARCHIVE] Compressing directory: $Path to $DestinationPath..." -ForegroundColor Yellow

    if (-not (Test-Path $Path)) {
        Write-Host "  [FAIL] Directory not found at: $Path" -ForegroundColor Red
        return $false
    }

    $ParentDir = Split-Path $DestinationPath -Parent
    if (-not (Test-Path $ParentDir)) {
        New-Item -ItemType Directory -Path $ParentDir -Force | Out-Null
    }

    try {
        Compress-Archive -Path $Path -DestinationPath $DestinationPath -Force -ErrorAction Stop
        Write-Host "  [OK] Compression completed successfully." -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "  [FAIL] Failed to compress directory: $_" -ForegroundColor Red
        return $false
    }
}
