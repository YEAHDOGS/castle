<#
.SYNOPSIS
    Retro Console Downloader Pipeline.
.DESCRIPTION
    Downloads, cryptographically verifies, extracts, and structures retro console BIOS/ROM files.
.PARAMETER Target
    The Target ID to boot/download (e.g. "gba-bios", "nes-alterego", "list").
.PARAMETER Purge
    Removes the downloaded files and folders for the specified target.
#>
param(
    [Parameter(Position=0)]
    [string]$Target,

    [Parameter()]
    [switch]$Purge
)

# ==============================================================================
# MODULE LOADER
# ==============================================================================
$ModuleRoot = Join-Path (Split-Path $PSScriptRoot -Parent) "modules"
. (Join-Path $ModuleRoot "targets.ps1")
. (Join-Path $ModuleRoot "security.ps1")
. (Join-Path $ModuleRoot "archive.ps1")
. (Join-Path $ModuleRoot "network.ps1")

$DataDir = Join-Path (Split-Path $PSScriptRoot -Parent) "data"

# Ensure core data directory structures exist
$BiosDir = Join-Path $DataDir "bios"
$RomsDir = Join-Path $DataDir "roms"

# ==============================================================================
# DYNAMIC PATH RESOLVER
# ==============================================================================
function Get-TargetPaths {
    param([hashtable]$Entry)
    
    # BIOS files go under data/bios/
    # ROM files go under data/roms/<platform>/
    if ($Entry.Type -eq "bios") {
        $DestFolder = $BiosDir
    }
    else {
        $DestFolder = Join-Path $RomsDir $Entry.Platform
    }

    $TargetFile = Join-Path $DestFolder $Entry.File
    return @{
        Folder = $DestFolder
        File   = $TargetFile
    }
}

# ==============================================================================
# PURGE / CLEANUP COMMAND
# ==============================================================================
if ($Purge) {
    if ([string]::IsNullOrWhiteSpace($Target)) {
        Write-Host "  [FAIL] Target ID is required to purge." -ForegroundColor Red
        Exit 1
    }

    $CleanInput = $Target.Trim().ToLower()
    $Matched = @($ArcadeMatrix | Where-Object { $_.Id -eq $CleanInput -or $_.Name.ToLower() -like "*$CleanInput*" })
    
    if ($Matched.Count -eq 0) {
        Write-Host "  [FAIL] Target '$Target' not found in registry." -ForegroundColor Red
        Exit 1
    }
    if ($Matched.Count -gt 1) {
        Write-Host "  [?] Ambiguous target '$Target'. Matches multiple entries:" -ForegroundColor Yellow
        foreach ($m in $Matched) { Write-Host "    * $($m.Id)" }
        Exit 1
    }

    $Entry = $Matched[0]
    $Paths = Get-TargetPaths -Entry $Entry

    if (Test-Path $Paths.File) {
        Write-Host "  [CLEANUP] Removing downloaded asset file: $($Paths.File)" -ForegroundColor Yellow
        Remove-Item $Paths.File -Force
    }
    else {
        Write-Host "  [CLEANUP] File does not exist: $($Paths.File)" -ForegroundColor Gray
    }

    Write-Host "  [CLEANUP] Purge completed successfully." -ForegroundColor Green
    Exit 0
}

# ==============================================================================
# TARGET DIRECTORY CHECK
# ==============================================================================
function Show-ArcadeMenu {
    Write-Host ""
    Write-Host "   === Retro Console Downloader - Target Registry ===" -ForegroundColor Cyan
    Write-Host "   ================================================================" -ForegroundColor DarkGray
    Write-Host ""

    $Index = 1
    foreach ($Entry in $ArcadeMatrix) {
        $Paths = Get-TargetPaths -Entry $Entry
        $Status = if (Test-Path $Paths.File) { "[CACHED]" } else { "[      ]" }

        $IdxStr = "[{0,2}]" -f $Index
        Write-Host "   $IdxStr $Status  $($Entry.Id.PadRight(18)) $($Entry.Name)" -ForegroundColor White
        $Index++
    }

    Write-Host ""
    Write-Host "   [CACHED] = Asset downloaded & verified" -ForegroundColor DarkGray
    Write-Host "   Usage: Select a number, enter a Target ID, type 'list', or 'exit'." -ForegroundColor DarkGray
    Write-Host ""
}

# ==============================================================================
# LIST COMMAND
# ==============================================================================
if ($Target -eq "list") {
    Write-Host ""
    Write-Host "  Registered Retro Console Targets" -ForegroundColor Cyan
    Write-Host "  ================================================================" -ForegroundColor DarkGray
    foreach ($Entry in $ArcadeMatrix) {
        $Paths = Get-TargetPaths -Entry $Entry
        $Status = if (Test-Path $Paths.File) { "CACHED" } else { "MISSING" }
        Write-Host "  * ID: $($Entry.Id.PadRight(16)) Type: $($Entry.Type.PadRight(6)) Status: $($Status.PadRight(8)) Name: $($Entry.Name)" -ForegroundColor White
    }
    Write-Host ""
    Exit 0
}

# ==============================================================================
# PIPELINE EXECUTION ENGINE
# ==============================================================================
function Run-DownloadPipeline {
    param([hashtable]$Entry)

    Write-Host ""
    Write-Host "  Console Downloader Pipeline" -ForegroundColor Cyan
    Write-Host "  Target: $($Entry.Name)" -ForegroundColor White
    Write-Host "  ================================================================" -ForegroundColor DarkGray

    $Paths = Get-TargetPaths -Entry $Entry
    $DestinationFile = $Paths.File
    $DestinationFolder = $Paths.Folder

    # Phase 1: Stream Download (Skip if file already exists)
    if (Test-Path $DestinationFile) {
        Write-Host "  [OK] Asset already cached locally at: $DestinationFile" -ForegroundColor Green
    }
    else {
        $DownloadOk = Start-DownloadStream -Url $Entry.Url -Path $DestinationFile
        if (-not $DownloadOk) {
            Write-Host "  [FAIL] Download execution aborted." -ForegroundColor Red
            Exit 1
        }
    }

    # Phase 2: Cryptographic Hash Verification
    $Verified = Test-FileHash -Path $DestinationFile -ExpectedHash $Entry.ExpectedHash -Algorithm $Entry.HashAlgorithm
    if (-not $Verified) {
        Write-Host "  [FAIL] Cryptographic integrity verification failed! Deleting corrupted file." -ForegroundColor Red
        Remove-Item $DestinationFile -Force -ErrorAction SilentlyContinue
        Exit 1
    }

    # Phase 3: Extraction/Expansion (Optional)
    if ($Entry.Extract -eq $true) {
        $ExtractFolder = Join-Path $DestinationFolder "extracted_$($Entry.Id)"
        $ExtractOk = Expand-ZipFile -Path $DestinationFile -DestinationPath $ExtractFolder
        
        if ($ExtractOk) {
            # Find and copy targeted ROM file out, if configured
            if (-not [string]::IsNullOrEmpty($Entry.ExtractFile)) {
                $ExtractedRomSource = Join-Path $ExtractFolder $Entry.ExtractFile
                $FinalRomTarget = Join-Path $DestinationFolder $Entry.ExtractFile
                
                if (Test-Path $ExtractedRomSource) {
                    Move-Item -Path $ExtractedRomSource -Destination $FinalRomTarget -Force
                    Write-Host "  [OK] Extracted ROM file moved to target: $FinalRomTarget" -ForegroundColor Green
                }
            }
            # Cleanup temp extraction folder
            Remove-Item $ExtractFolder -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    Write-Host ""
    Write-Host "  [OK] Pipeline completed successfully for: $($Entry.Id)!" -ForegroundColor Green
    Write-Host ""
}

# ==============================================================================
# INTERACTIVE CLI LOOP
# ==============================================================================
if ([string]::IsNullOrWhiteSpace($Target)) {
    while ($true) {
        Clear-Host
        Show-ArcadeMenu
        
        $Choice = Read-Host "   Arcade [1-$($ArcadeMatrix.Count), target-id, list, exit]"
        if ([string]::IsNullOrWhiteSpace($Choice)) { continue }
        $Choice = $Choice.Trim().ToLower()

        if ($Choice -eq "exit" -or $Choice -eq "q") {
            Write-Host "   Exiting Console Downloader. Goodbye!" -ForegroundColor Yellow
            Exit 0
        }

        if ($Choice -eq "list") {
            Clear-Host
            Write-Host ""
            Write-Host "  Registered Targets:" -ForegroundColor Cyan
            foreach ($Entry in $ArcadeMatrix) {
                Write-Host "     * $($Entry.Id) -- $($Entry.Name)"
            }
            Write-Host ""
            Read-Host "   Press Enter to return to menu..." | Out-Null
            continue
        }

        $SelectedIndex = -1
        if ([int]::TryParse($Choice, [ref]$SelectedIndex)) {
            if ($SelectedIndex -ge 1 -and $SelectedIndex -le $ArcadeMatrix.Count) {
                $SelectedEntry = $ArcadeMatrix[$SelectedIndex - 1]
            }
            else {
                Write-Host "   [FAIL] Selection $SelectedIndex is out of range." -ForegroundColor Red
                Start-Sleep -Seconds 1
                continue
            }
        }
        else {
            # Find matching target Id
            $Matched = @($ArcadeMatrix | Where-Object { $_.Id -eq $Choice -or $_.Name.ToLower() -like "*$Choice*" })
            if ($Matched.Count -eq 0) {
                Write-Host "   [FAIL] Unknown target: '$Choice'" -ForegroundColor Red
                Start-Sleep -Seconds 1.5
                continue
            }
            elseif ($Matched.Count -gt 1) {
                Write-Host "   [?] Ambiguous -- matches multiple targets:" -ForegroundColor Yellow
                foreach ($m in $Matched) { Write-Host "     * $($m.Id)" }
                Read-Host "   Press Enter to try again..." | Out-Null
                continue
            }
            else {
                $SelectedEntry = $Matched[0]
            }
        }

        # Run pipeline for selected entry
        Run-DownloadPipeline -Entry $SelectedEntry
        Read-Host "   Pipeline completed. Press Enter to continue..." | Out-Null
    }
}
else {
    # Direct command mode
    $CleanInput = $Target.Trim().ToLower()
    $Matched = @($ArcadeMatrix | Where-Object { $_.Id -eq $CleanInput -or $_.Name.ToLower() -like "*$CleanInput*" })
    
    if ($Matched.Count -eq 0) {
        Write-Host "  [FAIL] Target '$Target' not found in registry." -ForegroundColor Red
        Exit 1
    }
    if ($Matched.Count -gt 1) {
        Write-Host "  [?] Ambiguous -- matches multiple targets:" -ForegroundColor Yellow
        foreach ($m in $Matched) { Write-Host "    * $($m.Id)" }
        Exit 1
    }

    Run-DownloadPipeline -Entry $Matched[0]
}
