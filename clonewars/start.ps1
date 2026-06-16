<#
.SYNOPSIS
    Castle VM -- One-command virtual machine pipeline.
.DESCRIPTION
    Downloads, cryptographically verifies, provisions, and boots QEMU virtual machines.
    Sources modular components from the modules/ directory.
.PARAMETER Target
    The OS target ID to boot (e.g., "cachyos", "ubuntu", "alpine").
    Omit or pass "list" to see all available targets.
.EXAMPLE
    .\start.ps1 cachyos
    .\start.ps1 ubuntu
    .\start.ps1 list
#>
param (
    [Parameter(Position = 0)]
    [String]$Target,

    [Parameter()]
    [String]$Instance,

    [Parameter()]
    [Switch]$Background,

    [Parameter()]
    [String]$BaseDisk,

    [Parameter()]
    [Switch]$Vnc,

    [Parameter()]
    [Switch]$DeleteDisk,

    [Parameter()]
    [Switch]$DeleteIso,

    [Parameter()]
    [Switch]$Purge
)

# ==============================================================================
# MODULE LOADER
# ==============================================================================
$ModuleRoot = Join-Path $PSScriptRoot "modules"
. "$ModuleRoot\targets.ps1"
. "$ModuleRoot\network.ps1"
. "$ModuleRoot\verify.ps1"
. "$ModuleRoot\hardware.ps1"
. "$ModuleRoot\launch.ps1"
. "$ModuleRoot\resolvers.ps1"

$DataDir = Join-Path $PSScriptRoot "data"

# ==============================================================================
# DIRECT COMMAND LINE CLEANUP / REMOVAL
# ==============================================================================
if ($DeleteDisk -or $DeleteIso -or $Purge) {
    if ([string]::IsNullOrWhiteSpace($Target)) {
        Write-Host "  [FAIL] Target name is required for deletion." -ForegroundColor Red
        Exit 1
    }

    $CleanInput = $Target.Trim().ToLower()
    $Matched = @($IsoMatrix | Where-Object { $_.Id -eq $CleanInput })
    if ($Matched.Count -eq 0) {
        $Matched = @($IsoMatrix | Where-Object { $_.Id -like "*$CleanInput*" -or $_.Name.ToLower() -like "*$CleanInput*" })
    }

    if ($Matched.Count -eq 0) {
        Write-Host "  [FAIL] Unknown target: '$Target'" -ForegroundColor Red
        Exit 1
    }
    if ($Matched.Count -gt 1) {
        Write-Host "  [?] Ambiguous -- '$Target' matches multiple entries:" -ForegroundColor Yellow
        foreach ($m in $Matched) {
            Write-Host "     * $($m.Id) -- $($m.Name)" -ForegroundColor White
        }
        Exit 1
    }

    $Resolved = Resolve-TargetVersion -Target $Matched[0]
    $DiskName = if ($Resolved.File) { $Resolved.File -replace '\.(iso|img\.gz)$', '.qcow2' } else { "$($Resolved.Id).qcow2" }
    $DiskFile = Join-Path $DataDir $DiskName
    $IsoFile = Join-Path $DataDir $Resolved.File

    if ($DeleteDisk -or $Purge) {
        if (Test-Path $DiskFile) {
            Write-Host "  [CLEANUP] Deleting virtual disk: $DiskFile" -ForegroundColor Yellow
            Remove-Item $DiskFile -Force
        }
        else {
            Write-Host "  [CLEANUP] Virtual disk does not exist: $DiskFile" -ForegroundColor DarkGray
        }
    }

    if ($DeleteIso -or $Purge) {
        if (Test-Path $IsoFile) {
            Write-Host "  [CLEANUP] Deleting cached ISO: $IsoFile" -ForegroundColor Yellow
            Remove-Item $IsoFile -Force
        }
        else {
            $FileNamePattern = if ($Matched[0].FileTemplate) { $Matched[0].FileTemplate -replace '\$v', '*' } else { $Resolved.File }
            if (-not $FileNamePattern) { $FileNamePattern = "$($Resolved.Id).iso" }
            $PatternPath = Join-Path $DataDir $FileNamePattern
            $WildcardFiles = @(Get-Item $PatternPath -ErrorAction SilentlyContinue)
            if ($WildcardFiles.Count -gt 0) {
                foreach ($wf in $WildcardFiles) {
                    Write-Host "  [CLEANUP] Deleting cached ISO: $($wf.FullName)" -ForegroundColor Yellow
                    Remove-Item $wf.FullName -Force
                }
            }
            else {
                Write-Host "  [CLEANUP] Cached ISO does not exist: $IsoFile" -ForegroundColor DarkGray
            }
        }
    }

    if ($Purge) {
        $TrustFile = Join-Path $DataDir ".castle_trust.json"
        if (Test-Path $TrustFile) {
            $TrustStore = Get-PinnedTrustStore
            $KeysToRemove = @($TrustStore.Keys) | Where-Object { 
                $_ -eq $Resolved.File -or 
                $_ -eq "$($Resolved.Id).iso" -or 
                $_ -match "^$($Resolved.Id)-\d+\.iso$" 
            }
            foreach ($k in $KeysToRemove) {
                Write-Host "  [CLEANUP] Removing pinned hash for: $k" -ForegroundColor Yellow
                $TrustStore.Remove($k) | Out-Null
            }
            Save-PinnedTrustStore -TrustStore $TrustStore
        }
    }

    Write-Host "  [CLEANUP] Direct cleanup completed successfully." -ForegroundColor Green
    Exit 0
}

# ==============================================================================
# INTERACTIVE CLI HELPER FUNCTIONS
# ==============================================================================
function Show-CastleMenu {
    Write-Host ""
    Write-Host "   === Castle VM -- Target Registry ===" -ForegroundColor Cyan
    Write-Host "   ================================================================" -ForegroundColor DarkGray
    Write-Host ""

    $Index = 1
    foreach ($Entry in $IsoMatrix) {
        $FileName = if ($Entry.File) { $Entry.File } elseif ($Entry.FileTemplate) { $Entry.FileTemplate -replace '\$v', '*' } else { "$($Entry.Id).iso" }
        $IsoPattern = Join-Path $DataDir $FileName
        $CachedIsos = @(Get-Item $IsoPattern -ErrorAction SilentlyContinue)
        
        $DiskNamePattern = if ($Entry.File) { $Entry.File -replace '\.(iso|img\.gz)$', '.qcow2' } elseif ($Entry.FileTemplate) { ($Entry.FileTemplate -replace '\$v', '*') -replace '\.(iso|img\.gz)$', '.qcow2' } else { "$($Entry.Id).qcow2" }
        $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
        $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)

        $IsoIcon = if ($CachedIsos.Count -gt 0) { "[ISO]" } else { "[   ]" }
        $DiskIcon = if ($CachedDisks.Count -gt 0) { "[DISK]" } else { "      " }

        $SizeStr = ""
        if ($CachedIsos.Count -gt 0) {
            $SizeGB = "{0:N1}" -f ($CachedIsos[0].Length / 1GB)
            $SizeStr = "($SizeGB GB)"
        }

        $IdxStr = "[{0,2}]" -f $Index
        Write-Host "   $IdxStr $IsoIcon $DiskIcon  $($Entry.Id.PadRight(18)) $($Entry.Name) $SizeStr" -ForegroundColor White
        $Index++
    }

    Write-Host ""
    Write-Host "   [ISO] = ISO cached  [DISK] = VM disk exists  [   ] = Not downloaded" -ForegroundColor DarkGray
    Write-Host "   Usage: Select a number, enter a Target ID, type 'help' or 'exit'." -ForegroundColor DarkGray
    Write-Host ""
}

function Show-TargetDetails {
    param (
        [hashtable]$Entry
    )

    $FileName = if ($Entry.File) { $Entry.File } elseif ($Entry.FileTemplate) { $Entry.FileTemplate -replace '\$v', '*' } else { "$($Entry.Id).iso" }
    $IsoPattern = Join-Path $DataDir $FileName
    $CachedIsos = @(Get-Item $IsoPattern -ErrorAction SilentlyContinue)
    
    $DiskNamePattern = if ($Entry.File) { $Entry.File -replace '\.(iso|img\.gz)$', '.qcow2' } elseif ($Entry.FileTemplate) { ($Entry.FileTemplate -replace '\$v', '*') -replace '\.(iso|img\.gz)$', '.qcow2' } else { "$($Entry.Id).qcow2" }
    $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
    $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)

    $IsoStatus = if ($CachedIsos.Count -gt 0) {
        $SizeGB = "{0:N2}" -f ($CachedIsos[0].Length / 1GB)
        "Cached ($SizeGB GB) at $($CachedIsos[0].Name)"
    }
    else {
        "Not downloaded (will fetch automatically)"
    }

    $TargetDiskSize = if ($Entry.DiskSize) { $Entry.DiskSize } else { "40G" }
    $DiskStatus = if ($CachedDisks.Count -gt 0) {
        $DiskGB = "{0:N2}" -f ($CachedDisks[0].Length / 1GB)
        "Provisioned ($DiskGB GB) at $($CachedDisks[0].Name)"
    }
    else {
        "Not provisioned (will create new $TargetDiskSize disk)"
    }

    Write-Host ""
    Write-Host "   Target Details: $($Entry.Name)" -ForegroundColor Cyan
    Write-Host "   ================================================================" -ForegroundColor DarkGray
    Write-Host "   * Target ID:     $($Entry.Id)" -ForegroundColor White
    Write-Host "   * Description:   $($Entry.Description)" -ForegroundColor Yellow
    Write-Host "   * OS Family:     $($Entry.OsFamily)" -ForegroundColor White
    Write-Host "   * Disk Size:     $TargetDiskSize" -ForegroundColor White
    Write-Host "   * ISO Cache:     $IsoStatus" -ForegroundColor White
    Write-Host "   * Virtual Disk:  $DiskStatus" -ForegroundColor White
    Write-Host "   ================================================================" -ForegroundColor DarkGray
    Write-Host ""
}

function Show-HelpDocs {
    Write-Host ""
    Write-Host "   === Castle VM -- CLI Documentation ===" -ForegroundColor Cyan
    Write-Host "   ================================================================" -ForegroundColor DarkGray
    Write-Host "   Castle VM is a modular virtual machine pipeline using QEMU."
    Write-Host "   It handles automatic ISO download, cryptographic verification,"
    Write-Host "   local caching, trust tracking (Trust Pinning), disk provisioning, and boot."
    Write-Host ""
    Write-Host "   COMMAND LINE USAGE:" -ForegroundColor White
    Write-Host "     .\start.ps1 <target-id>     Directly boot/install a specific target."
    Write-Host "     .\start.ps1 list            Show static list of available targets."
    Write-Host "     .\start.ps1                 Start this interactive prompting loop."
    Write-Host ""
    Write-Host "   SECURITY & TRUST PINNING:" -ForegroundColor White
    Write-Host "     * Remote Verification: Checks remote SHA hashes and GPG signatures."
    Write-Host "     * Local Trust Store: After first success, hashes are saved in"
    Write-Host "       data\.castle_trust.json. Future boots re-verify against the local"
    Write-Host "       trust store to protect against bit rot or offline tampering."
    Write-Host ""
    Write-Host "   DIRECTORY STRUCTURE:" -ForegroundColor White
    Write-Host "     * start.ps1                 Unified CLI entry point."
    Write-Host "     * data/                     Downloaded ISOs, virtual disks, and trust store."
    Write-Host "     * modules/                  Modular helper scripts for network, targets,"
    Write-Host "                                 verification, hardware, and launch."
    Write-Host "     * scripts/                  Autoinstall configurations."
    Write-Host "   ================================================================" -ForegroundColor DarkGray
    Write-Host ""
    Read-Host "   Press Enter to return to menu..." | Out-Null
}

# ==============================================================================
# NO ARGUMENT -- Interactive prompt mode
# ==============================================================================
if ([string]::IsNullOrWhiteSpace($Target)) {
    while ($true) {
        Clear-Host
        Show-CastleMenu
        
        $Choice = Read-Host "   Castle VM [1-$($IsoMatrix.Count), target-id, help, exit]"
        if ([string]::IsNullOrWhiteSpace($Choice)) { continue }
        $Choice = $Choice.Trim().ToLower()

        if ($Choice -eq "exit" -or $Choice -eq "q") {
            Write-Host "   Exiting Castle VM. Goodbye!" -ForegroundColor Yellow
            Exit 0
        }

        if ($Choice -eq "help" -or $Choice -eq "h") {
            Show-HelpDocs
            continue
        }

        $SelectedIndex = -1
        if ([int]::TryParse($Choice, [ref]$SelectedIndex)) {
            if ($SelectedIndex -ge 1 -and $SelectedIndex -le $IsoMatrix.Count) {
                $SelectedTarget = $IsoMatrix[$SelectedIndex - 1]
            }
            else {
                Write-Host "   [FAIL] Index $SelectedIndex is out of range." -ForegroundColor Red
                Start-Sleep -Seconds 1
                continue
            }
        }
        else {
            # Find target matching input
            $Matched = @($IsoMatrix | Where-Object { $_.Id -eq $Choice })
            if ($Matched.Count -eq 0) {
                $Matched = @($IsoMatrix | Where-Object { $_.Id -like "*$Choice*" -or $_.Name.ToLower() -like "*$Choice*" })
            }

            if ($Matched.Count -eq 0) {
                Write-Host "   [FAIL] Unknown target: '$Choice'" -ForegroundColor Red
                Start-Sleep -Seconds 1.5
                continue
            }
            elseif ($Matched.Count -gt 1) {
                Write-Host "   [?] Ambiguous -- matches multiple targets:" -ForegroundColor Yellow
                foreach ($m in $Matched) {
                    Write-Host "     * $($m.Id) -- $($m.Name)" -ForegroundColor White
                }
                Read-Host "   Press Enter to try again..." | Out-Null
                continue
            }
            else {
                $SelectedTarget = $Matched[0]
            }
        }

        # Show target details and ask to boot
        while ($true) {
            Clear-Host
            Show-TargetDetails -Entry $SelectedTarget
            $BootChoice = Read-Host "   Boot this VM? [Y]es / [N]o / [D]elete Options / [B]ack (default: Y)"
            if ([string]::IsNullOrWhiteSpace($BootChoice)) { $BootChoice = "y" }
            $BootChoice = $BootChoice.Trim().ToLower()

            if ($BootChoice -eq "y" -or $BootChoice -eq "yes") {
                $Target = $SelectedTarget.Id
                break
            }
            elseif ($BootChoice -eq "n" -or $BootChoice -eq "no" -or $BootChoice -eq "b" -or $BootChoice -eq "back") {
                $SelectedTarget = $null
                break
            }
            elseif ($BootChoice -eq "d" -or $BootChoice -eq "delete") {
                # Render deletion sub-menu
                while ($true) {
                    Clear-Host
                    Write-Host ""
                    Write-Host "   === Target Cleanup Options: $($SelectedTarget.Name) ===" -ForegroundColor Cyan
                    Write-Host "   ================================================================" -ForegroundColor DarkGray
                    Write-Host "   1. Delete Virtual Disk (.qcow2)" -ForegroundColor White
                    Write-Host "   2. Delete Cached ISO (.iso)" -ForegroundColor White
                    Write-Host "   3. Full Purge (Delete Disk, ISO, and Pinned Trust Hash)" -ForegroundColor White
                    Write-Host "   4. Cancel & Go Back" -ForegroundColor White
                    Write-Host "   ================================================================" -ForegroundColor DarkGray
                    Write-Host ""
                    
                    $DelOption = Read-Host "   Select an option [1-4]"
                    if ($DelOption -eq "4" -or [string]::IsNullOrWhiteSpace($DelOption)) {
                        break
                    }

                    if ($DelOption -eq "1") {
                        $Confirm = Read-Host "   Are you sure you want to delete the virtual disk .qcow2 file? [y/N]"
                        if ($Confirm -eq "y" -or $Confirm -eq "yes") {
                            $DiskNamePattern = if ($SelectedTarget.File) { $SelectedTarget.File -replace '\.(iso|img\.gz)$', '.qcow2' } elseif ($SelectedTarget.FileTemplate) { ($SelectedTarget.FileTemplate -replace '\$v', '*') -replace '\.(iso|img\.gz)$', '.qcow2' } else { "$($SelectedTarget.Id).qcow2" }
                            $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
                            $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)
                            if ($CachedDisks.Count -gt 0) {
                                foreach ($cd in $CachedDisks) { Remove-Item $cd.FullName -Force }
                                Write-Host "   [OK] Deleted virtual disk(s)." -ForegroundColor Green
                            }
                            else {
                                Write-Host "   [i] Disk does not exist." -ForegroundColor Gray
                            }
                            Start-Sleep -Seconds 1.5
                        }
                        break
                    }
                    elseif ($DelOption -eq "2") {
                        $Confirm = Read-Host "   Are you sure you want to delete the cached .iso file? [y/N]"
                        if ($Confirm -eq "y" -or $Confirm -eq "yes") {
                            $FileNamePattern = if ($SelectedTarget.FileTemplate) { $SelectedTarget.FileTemplate -replace '\$v', '*' } else { $SelectedTarget.File }
                            if (-not $FileNamePattern) { $FileNamePattern = "$($SelectedTarget.Id).iso" }
                            $PatternPath = Join-Path $DataDir $FileNamePattern
                            
                            $WildcardFiles = @(Get-Item $PatternPath -ErrorAction SilentlyContinue)
                            if ($WildcardFiles.Count -gt 0) {
                                foreach ($wf in $WildcardFiles) {
                                    Remove-Item $wf.FullName -Force
                                }
                                Write-Host "   [OK] Deleted cached ISO(s)." -ForegroundColor Green
                            }
                            else {
                                Write-Host "   [i] Cached ISO does not exist." -ForegroundColor Gray
                            }
                            Start-Sleep -Seconds 1.5
                        }
                        break
                    }
                    elseif ($DelOption -eq "3") {
                        $Confirm = Read-Host "   Are you sure you want to perform a full purge (.qcow2, .iso, and trust hash)? [y/N]"
                        if ($Confirm -eq "y" -or $Confirm -eq "yes") {
                            # 1. Disk
                            $DiskNamePattern = if ($SelectedTarget.File) { $SelectedTarget.File -replace '\.(iso|img\.gz)$', '.qcow2' } elseif ($SelectedTarget.FileTemplate) { ($SelectedTarget.FileTemplate -replace '\$v', '*') -replace '\.(iso|img\.gz)$', '.qcow2' } else { "$($SelectedTarget.Id).qcow2" }
                            $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
                            $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)
                            if ($CachedDisks.Count -gt 0) {
                                foreach ($cd in $CachedDisks) { Remove-Item $cd.FullName -Force }
                            }
                            
                            # 2. ISO
                            $FileNamePattern = if ($SelectedTarget.FileTemplate) { $SelectedTarget.FileTemplate -replace '\$v', '*' } else { $SelectedTarget.File }
                            if (-not $FileNamePattern) { $FileNamePattern = "$($SelectedTarget.Id).iso" }
                            $PatternPath = Join-Path $DataDir $FileNamePattern
                            $WildcardFiles = @(Get-Item $PatternPath -ErrorAction SilentlyContinue)
                            if ($WildcardFiles.Count -gt 0) {
                                foreach ($wf in $WildcardFiles) { Remove-Item $wf.FullName -Force }
                            }
                            
                            # 3. Pinned Trust Hash
                            $TrustFile = Join-Path $DataDir ".castle_trust.json"
                            if (Test-Path $TrustFile) {
                                $TrustStore = Get-PinnedTrustStore
                                $KeysToRemove = @($TrustStore.Keys) | Where-Object { 
                                    $_ -eq $SelectedTarget.File -or 
                                    $_ -eq "$($SelectedTarget.Id).iso" -or 
                                    $_ -match "^$($SelectedTarget.Id)-\d+\.iso$" 
                                }
                                foreach ($k in $KeysToRemove) { $TrustStore.Remove($k) | Out-Null }
                                Save-PinnedTrustStore -TrustStore $TrustStore
                            }
                            
                            Write-Host "   [OK] Full purge completed." -ForegroundColor Green
                            Start-Sleep -Seconds 1.5
                        }
                        break
                    }
                    else {
                        Write-Host "   Invalid selection. Enter 1-4." -ForegroundColor Red
                        Start-Sleep -Seconds 1
                    }
                }
            }
            else {
                Write-Host "   Invalid choice. Enter Y, N, D, or B." -ForegroundColor Red
                Start-Sleep -Seconds 1
            }
        }

        if ($SelectedTarget) {
            break
        }
    }
}

# ==============================================================================
# LIST -- Show static list of all targets and exit
# ==============================================================================
if ($Target -eq "list") {
    Write-Host ""
    Write-Host "  Castle VM -- Target Registry" -ForegroundColor Cyan
    Write-Host "  ================================================================" -ForegroundColor DarkGray
    Write-Host ""

    foreach ($Entry in $IsoMatrix) {
        $FileName = if ($Entry.File) { $Entry.File } elseif ($Entry.FileTemplate) { $Entry.FileTemplate -replace '\$v', '*' } else { "$($Entry.Id).iso" }
        $IsoPattern = Join-Path $DataDir $FileName
        $CachedIsos = @(Get-Item $IsoPattern -ErrorAction SilentlyContinue)
        
        $DiskNamePattern = if ($Entry.File) { $Entry.File -replace '\.(iso|img\.gz)$', '.qcow2' } elseif ($Entry.FileTemplate) { ($Entry.FileTemplate -replace '\$v', '*') -replace '\.(iso|img\.gz)$', '.qcow2' } else { "$($Entry.Id).qcow2" }
        $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
        $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)

        $IsoIcon = if ($CachedIsos.Count -gt 0) { "[ISO]" } else { "[   ]" }
        $DiskIcon = if ($CachedDisks.Count -gt 0) { "[DISK]" } else { "      " }

        $SizeStr = ""
        if ($CachedIsos.Count -gt 0) {
            $SizeGB = "{0:N1}" -f ($CachedIsos[0].Length / 1GB)
            $SizeStr = "($SizeGB GB)"
        }

        Write-Host "  $IsoIcon $DiskIcon  $($Entry.Id.PadRight(18)) $($Entry.Name) $SizeStr" -ForegroundColor White
    }

    Write-Host ""
    Write-Host "  [ISO] = ISO cached  [DISK] = VM disk exists  [   ] = Not downloaded" -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  Usage: .\start.ps1 <target-id>" -ForegroundColor DarkGray
    Write-Host ""
    Exit 0
}

# ==============================================================================
# RESOLVE TARGET -- Find matching entry from the manifest
# ==============================================================================
$CleanInput = $Target.Trim().ToLower()

# Prefer exact match, then fuzzy
$Matched = @($IsoMatrix | Where-Object { $_.Id -eq $CleanInput })
if ($Matched.Count -eq 0) {
    $Matched = @($IsoMatrix | Where-Object { $_.Id -like "*$CleanInput*" -or $_.Name.ToLower() -like "*$CleanInput*" })
}

if ($Matched.Count -eq 0) {
    Write-Host ""
    Write-Host "  [FAIL] Unknown target: '$Target'" -ForegroundColor Red
    Write-Host "  [i] Run .\start.ps1 list to see available targets." -ForegroundColor DarkGray
    Write-Host ""
    Exit 1
}

if ($Matched.Count -gt 1) {
    Write-Host ""
    Write-Host "  [?] Ambiguous -- '$Target' matches multiple entries:" -ForegroundColor Yellow
    foreach ($m in $Matched) {
        Write-Host "     * $($m.Id) -- $($m.Name)" -ForegroundColor White
    }
    Write-Host "  [i] Be more specific (e.g., 'ubuntu-server' instead of 'ubuntu')." -ForegroundColor DarkGray
    Write-Host ""
    Exit 1
}

# Apply Dynamic Version Resolution (Phase 2 feature)
$Iso = Resolve-TargetVersion -Target $Matched[0]
$IsoPath = Join-Path $DataDir $Iso.File

Write-Host ""
Write-Host "  Castle VM Pipeline" -ForegroundColor Cyan
Write-Host "  Target: $($Iso.Name)" -ForegroundColor White
Write-Host "  ================================================================" -ForegroundColor DarkGray

Add-Type -AssemblyName System.Net.Http
$HttpClient = [System.Net.Http.HttpClient]::new()

$Verified = $false
while (-not $Verified) {
    # ==============================================================================
    # PHASE 1 -- ISO ACQUISITION (Download if missing)
    # ==============================================================================
    Write-Host ""
    Write-Host "  [*] Phase 1: ISO Acquisition" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------" -ForegroundColor DarkGray

    if (Test-Path $IsoPath) {
        $SizeGB = "{0:N2}" -f ((Get-Item $IsoPath).Length / 1GB)
        Write-Host "  [OK] ISO cached: $IsoPath ($SizeGB GB)" -ForegroundColor Green
    }
    else {
        Write-Host "  [>] ISO not found locally. Starting secure download..." -ForegroundColor Yellow

        if (-not (Test-Path $DataDir)) {
            New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
        }

        $DownloadOk = Start-NetworkStream -Url $Iso.Url -Path $IsoPath -HttpClient $HttpClient
        if (-not $DownloadOk) {
            Write-Host "  [FAIL] Download failed. Aborting." -ForegroundColor Red
            $HttpClient.Dispose()
            Exit 1
        }
    }

    # ==============================================================================
    # PHASE 2 -- INTEGRITY VERIFICATION (Always runs, even for cached ISOs)
    # ==============================================================================
    Write-Host ""
    Write-Host "  [*] Phase 2: Integrity Verification" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------" -ForegroundColor DarkGray

    $Verified = Test-IsoIntegrity -Target $Iso -FilePath $IsoPath -HttpClient $HttpClient

    if (-not $Verified) {
        Write-Host ""
        Write-Host "  [FAIL] INTEGRITY CHECK FAILED -- Refusing to boot unverified image." -ForegroundColor Red
        $Choice = Read-Host "     Delete and re-download? [Y]es / [N]o (default: N)"
        if ($Choice -match "^y" -or $Choice -match "^yes") {
            Remove-Item $IsoPath -Force
            Write-Host "  [i] Deleted corrupted ISO. Retrying..." -ForegroundColor Yellow
        }
        else {
            Write-Host "  [FAIL] Exiting without boot." -ForegroundColor Red
            $HttpClient.Dispose()
            Exit 1
        }
    }
}
$HttpClient.Dispose()

# ==============================================================================
# PHASE 3 -- DISK PROVISIONING
# ==============================================================================
Write-Host ""
Write-Host "  [*] Phase 3: Disk Provisioning" -ForegroundColor Cyan
Write-Host "  -------------------------------------------" -ForegroundColor DarkGray

if (-not (Test-QemuInstalled)) { Exit 1 }

$DiskPath = if (-not [string]::IsNullOrEmpty($Instance)) {
    Join-Path $DataDir "instances\$Instance.qcow2"
}
else {
    $DiskName = if ($Iso.File) { $Iso.File -replace '\.(iso|img\.gz)$', '.qcow2' } else { "$($Iso.Id).qcow2" }
    Join-Path $DataDir $DiskName
}
$DiskSize = if ($Iso.DiskSize) { $Iso.DiskSize } else { "40G" }
$FirstBoot = New-VirtualDisk -DiskPath $DiskPath -DiskSize $DiskSize -BaseDisk $BaseDisk

# ==============================================================================
# PHASE 4 -- HARDWARE DETECTION
# ==============================================================================
Write-Host ""
Write-Host "  [*] Phase 4: Hardware Detection" -ForegroundColor Cyan
Write-Host "  -------------------------------------------" -ForegroundColor DarkGray

$Hardware = Get-HostHardwareProfile

Write-Host "  * CPU: $($Hardware.CpuCores) cores (Host: $($Hardware.HostCores)) -- $($Hardware.CpuProfile)" -ForegroundColor White
Write-Host "  * RAM: $($Hardware.Memory) (Host: $($Hardware.TotalRamGB)GB)" -ForegroundColor White

# ==============================================================================
# PHASE 5 -- LAUNCH
# ==============================================================================
Start-CastleVm `
    -Target $Iso `
    -DiskPath $DiskPath `
    -IsoPath $IsoPath `
    -Hardware $Hardware `
    -FirstBoot $FirstBoot `
    -Background:$Background `
    -Vnc:$Vnc