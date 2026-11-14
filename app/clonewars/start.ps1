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

    # Mirror to download from, for targets with a mirror list (list number, or a
    # substring of the host / name / country). Skips the mirror prompt.
    # CASTLE_MIRROR does the same for scripted runs.
    [Parameter()]
    [String]$Mirror,

    [Parameter()]
    [Switch]$Vnc,

    # Copy data/copy into the guest (read-only USB share CASTLECOPY), install
    # Wine there with scripts/setup-wine.sh and run the Windows programs it
    # finds -- all driven from the host through the QEMU guest agent. Targets
    # that support it carry SupportsCopy in targets.ps1 (EndeavourOS first).
    [Parameter()]
    [Alias("c")]
    [Switch]$Copy,

    # Program(s) to run after the Wine install: a file name inside data/copy or
    # a path inside the guest. Implies -Copy. Without it, -Copy runs every .exe.
    [Parameter()]
    [Alias("p")]
    [String[]]$Program,

    # Which guest setup runs with -Copy: scripts/setup-<name>.sh.
    #   auto     (default) whatever the payload calls for: Wine + every .exe/.msi
    #            when there are any, OpenGOAL when a PS2 disc ISO is present
    #   wine     install Wine, run the payload's .exe/.msi files
    #   opengoal install OpenGOAL, build Jak and Daxter from the disc ISO in
    #            data/copy, launch it (software OpenGL via llvmpipe)
    [Parameter()]
    [String]$Setup = "auto",

    # Launch profile: one .psd1 file naming the target, VM size, payload,
    # packages and commands (see modules/profiles.ps1). A name looks in
    # profiles\<name>.psd1; "list" shows them. Command-line switches win over
    # the profile's values.
    [Parameter()]
    [String]$Profile,

    # EXPERIMENTAL host-GPU 3D for the guest (virtio-vga-gl + SDL OpenGL
    # display). Hangs QEMU on this host (see the note near the end of the copy
    # payload block); left in for other hosts / newer QEMU builds. CASTLE_VM_GL=1/0.
    [Parameter()]
    [Switch]$Gl,

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
. (Join-Path $ModuleRoot "quiet.ps1")
. (Join-Path $ModuleRoot "targets.ps1")
. (Join-Path $ModuleRoot "network.ps1")
. (Join-Path $ModuleRoot "verify.ps1")
. (Join-Path $ModuleRoot "hardware.ps1")
. (Join-Path $ModuleRoot "launch.ps1")
. (Join-Path $ModuleRoot "resolvers.ps1")
. (Join-Path $ModuleRoot "guest.ps1")
. (Join-Path $ModuleRoot "profiles.ps1")

$DataDir = Join-Path $PSScriptRoot "data"
# CASTLE_COPY_DIR points one launch at its own payload folder, so several VMs
# can be prepared at once without sharing data/copy.
$CopyDir = if ($env:CASTLE_COPY_DIR) { $env:CASTLE_COPY_DIR } else { Join-Path $DataDir "copy" }
# Files staged per launch carry the instance name, for the same reason.
function Get-LaunchSuffix { if ([string]::IsNullOrEmpty($Instance)) { "" } else { "-$Instance" } }

function Get-CopyPayloadFiles {
    # Everything under data/copy except the folder's own README.
    if (-not (Test-Path $CopyDir)) { return @() }
    return @(Get-ChildItem $CopyDir -Recurse -File | Where-Object { -not ($_.Name -ieq "README.md" -and $_.DirectoryName -eq (Get-Item $CopyDir).FullName) })
}

# ==============================================================================
# LAUNCH PROFILE -- profiles\<name>.psd1 fills in what the command line left out
# ==============================================================================
$ProfileData = $null
$ProfileEnvFile = $null
if ($Profile -eq "list") {
    Write-Host ""
    Write-Host "  Castle launch profiles (.\start.ps1 -Profile <name>)" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------" -ForegroundColor DarkGray
    foreach ($Entry in Get-CastleProfiles -Root $PSScriptRoot) {
        Write-Host ("  {0}  {1}  {2}" -f $Entry.Name.PadRight(16), ([string]$Entry.Target).PadRight(18), $Entry.Description) -ForegroundColor White
    }
    Write-Host ""
    Exit 0
}
if (-not [string]::IsNullOrWhiteSpace($Profile)) {
    $ProfileData = Import-CastleProfile -Name $Profile -Root $PSScriptRoot
    if (-not $ProfileData) { Exit 1 }
    Write-Host ""
    Write-Host "  [PROFILE] $([System.IO.Path]::GetFileNameWithoutExtension($ProfileData.Path)) -- $($ProfileData.Description)" -ForegroundColor Cyan
    if (-not $PSBoundParameters.ContainsKey("Target") -and $ProfileData.Target) { $Target = [string]$ProfileData.Target }
    if (-not $PSBoundParameters.ContainsKey("Instance") -and $ProfileData.Instance) { $Instance = [string]$ProfileData.Instance }
    if (-not $PSBoundParameters.ContainsKey("Mirror") -and $ProfileData.Mirror) { $Mirror = [string]$ProfileData.Mirror }
    if (-not $PSBoundParameters.ContainsKey("Background") -and $ProfileData.Background) { $Background = $true }
    if (-not $PSBoundParameters.ContainsKey("Setup")) { $Setup = if ($ProfileData.Setup) { [string]$ProfileData.Setup } else { "profile" } }
    if ([string]::IsNullOrWhiteSpace($Target)) {
        Write-Host "  [FAIL] The profile names no Target; pass one: .\start.ps1 <target-id> -Profile $Profile" -ForegroundColor Red
        Exit 1
    }
    if (-not ($DeleteDisk -or $DeleteIso -or $Purge)) {
        foreach ($Key in @($ProfileData.Env.Keys | Where-Object { $_ })) { Set-Item -Path "Env:$Key" -Value ([string]$ProfileData.Env[$Key]) }
        if ($ProfileData.MemoryGB) { $env:CASTLE_VM_RAM_GB = [string]$ProfileData.MemoryGB }
        if ($ProfileData.Cores) { $env:CASTLE_VM_CORES = [string]$ProfileData.Cores }
        $Global:CastleExtraQemuArgs = @($ProfileData.QemuArgs | Where-Object { $_ })
        if (-not (Update-CastleProfilePayload -Profile $ProfileData -CopyDir $CopyDir)) { Exit 1 }
        $ProfileEnvFile = Write-CastleProfileEnv -Profile $ProfileData -Path (Join-Path (Join-Path (Join-Path $DataDir "seeds") "profile$(Get-LaunchSuffix)") "castle-profile.env")
        # A profile is applied through the guest agent, like a copy payload.
        $Copy = $true
    }
}
else { $Global:CastleExtraQemuArgs = @() }
# CASTLE_VM_SNAPSHOT=1: a throwaway run. QEMU keeps every disk write in a
# temporary file and drops it when the VM closes; the disk file is not changed.
if ($env:CASTLE_VM_SNAPSHOT -eq "1") { $Global:CastleExtraQemuArgs = @($Global:CastleExtraQemuArgs) + @("-snapshot") }

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

    # Docker targets: no ISO, no qcow2; their data is a Docker volume that docker.ps1 owns.
    if ($Matched[0].Runtime -eq "docker") {
        if ($DeleteIso -and -not ($DeleteDisk -or $Purge)) {
            Write-Host "  [i] '$($Matched[0].Id)' has no cached ISO of its own; use -DeleteDisk or -Purge to remove its container data." -ForegroundColor DarkGray
            Exit 0
        }
        & (Join-Path $PSScriptRoot "docker.ps1") -Profile $Matched[0].DockerProfile -Down -Purge
        Exit $LASTEXITCODE
    }

    # Google emulator: the AVD lives in data/android-sdk; android.ps1 removes it.
    if ($Matched[0].Runtime -eq "avd") {
        & (Join-Path $PSScriptRoot "android.ps1") -Purge
        Exit $LASTEXITCODE
    }

    # Containers: the container is the disk; container.ps1 removes it.
    if ($Matched[0].Runtime -eq "container") {
        $RemoveArgs = @{ Target = $Matched[0].Id; Remove = $true }
        if ($Instance) { $RemoveArgs.Instance = $Instance }
        & (Join-Path $PSScriptRoot "container.ps1") @RemoveArgs
        Exit $LASTEXITCODE
    }

    $Resolved = Resolve-TargetVersion -Target $Matched[0] -Mirror $Mirror -NoMirrorPrompt
    # The same naming the launch uses: a cloud-image target's disk is <id>.qcow2,
    # while its File is the shared downloaded image, which -DeleteDisk must not touch.
    $DiskName = Get-TargetDiskName $Resolved
    $DiskFile = if (-not [string]::IsNullOrEmpty($Instance)) {
        Join-Path (Join-Path $DataDir "instances") "$Instance.qcow2"
    }
    else {
        Join-Path $DataDir $DiskName
    }
    $IsoFile = Join-Path $DataDir $Resolved.File

    if ($DeleteDisk -or $Purge) {
        if (Test-Path $DiskFile) {
            Write-Host "  [CLEANUP] Deleting virtual disk: $DiskFile" -ForegroundColor Yellow
            Remove-Item $DiskFile -Force
        }
        else {
            Write-Host "  [CLEANUP] Virtual disk does not exist: $DiskFile" -ForegroundColor DarkGray
        }
        # The -Copy scratch disk (payload copy, Wine prefix, game builds) goes with it.
        $ScratchName = if (-not [string]::IsNullOrEmpty($Instance)) { "$Instance-scratch.qcow2" } else { "$($Resolved.Id)-scratch.qcow2" }
        $ScratchFile = Join-Path (Join-Path $DataDir "scratch") $ScratchName
        if (Test-Path $ScratchFile) {
            Write-Host "  [CLEANUP] Deleting scratch disk: $ScratchFile" -ForegroundColor Yellow
            Remove-Item $ScratchFile -Force
        }
    }

    if ($DeleteIso -or $Purge) {
        if (Test-Path $IsoFile) {
            Write-Host "  [CLEANUP] Deleting cached ISO: $IsoFile" -ForegroundColor Yellow
            if ($Matched[0].IsFolder) {
                Remove-Item $IsoFile -Recurse -Force
            }
            else {
                Remove-Item $IsoFile -Force
            }
        }
        else {
            $FileNamePattern = if ($Matched[0].FileTemplate) { $Matched[0].FileTemplate -replace '\$v', '*' } else { $Resolved.File }
            if (-not $FileNamePattern) { $FileNamePattern = "$($Resolved.Id).iso" }
            $PatternPath = Join-Path $DataDir $FileNamePattern
            $WildcardFiles = @(Get-Item $PatternPath -ErrorAction SilentlyContinue)
            if ($WildcardFiles.Count -gt 0) {
                foreach ($wf in $WildcardFiles) {
                    Write-Host "  [CLEANUP] Deleting cached ISO: $($wf.FullName)" -ForegroundColor Yellow
                    if ($Matched[0].IsFolder) {
                        Remove-Item $wf.FullName -Recurse -Force
                    }
                    else {
                        Remove-Item $wf.FullName -Force
                    }
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
                $_ -match "^$($Resolved.Id)-.+\.(iso|zip|img\.gz)$"
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
        
        $DiskNamePattern = Get-TargetDiskName $Entry
        $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
        $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)

        $IsoIcon = if ($CachedIsos.Count -gt 0) { "[ISO]" } else { "[   ]" }
        $DiskIcon = if ($CachedDisks.Count -gt 0) { "[DISK]" } else { "      " }

        $SizeStr = ""
        if ($CachedIsos.Count -gt 0) {
            if ($Entry.IsFolder) {
                $FolderSize = (Get-ChildItem $CachedIsos[0].FullName -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
                $SizeGB = "{0:N1}" -f ($FolderSize / 1GB)
            }
            else {
                $SizeGB = "{0:N1}" -f ($CachedIsos[0].Length / 1GB)
            }
            $SizeStr = "($SizeGB GB)"
        }

        if ($Entry.Runtime -eq "docker") { $IsoIcon = "[DKR]"; $DiskIcon = "      "; $SizeStr = "" }
        if ($Entry.Runtime -eq "avd") { $IsoIcon = "[AVD]"; $DiskIcon = "      "; $SizeStr = "" }
        if ($Entry.Runtime -eq "container") { $IsoIcon = "[CTR]"; $DiskIcon = "      "; $SizeStr = "" }

        $IdxStr = "[{0,2}]" -f $Index
        Write-Host "   $IdxStr $IsoIcon $DiskIcon  $($Entry.Id.PadRight(18)) $($Entry.Name) $SizeStr" -ForegroundColor White
        $Index++
    }

    Write-Host ""
    Write-Host "   [ISO] = ISO cached  [DISK] = VM disk exists  [   ] = Not downloaded  [DKR] = Docker container (docker.ps1)  [AVD] = Google Android Emulator (android.ps1)  [CTR] = container (container.ps1)" -ForegroundColor DarkGray
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
    
    $DiskNamePattern = Get-TargetDiskName $Entry
    $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
    $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)

    $IsoStatus = if ($CachedIsos.Count -gt 0) {
        $SizeGB = "{0:N2}" -f ($CachedIsos[0].Length / 1GB)
        "Cached ($SizeGB GB) at $($CachedIsos[0].Name)"
    }
    else {
        "Not downloaded (will fetch automatically)"
    }

    $TargetDiskSize = if ($Entry.DownloadOnly) { "N/A" } elseif ($Entry.DiskSize) { $Entry.DiskSize } else { "40G" }
    $DiskStatus = if ($Entry.DownloadOnly) {
        "N/A (download-only firmware target)"
    }
    elseif ($CachedDisks.Count -gt 0) {
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
    if ($Entry.Runtime -eq "docker") {
        Write-Host "   * Runtime:       Docker container -- docker.ps1 -Profile $($Entry.DockerProfile) (needs Docker Desktop running)" -ForegroundColor White
        Write-Host "   * Data:          Docker volume owned by docker.ps1; 'Delete' here runs docker.ps1 -Down -Purge" -ForegroundColor White
    }
    else {
        Write-Host "   * Disk Size:     $TargetDiskSize" -ForegroundColor White
        Write-Host "   * ISO Cache:     $IsoStatus" -ForegroundColor White
        Write-Host "   * Virtual Disk:  $DiskStatus" -ForegroundColor White
    }
    if ($Entry.SupportsCopy) {
        $PayloadFiles = @(Get-CopyPayloadFiles)
        $PayloadStatus = if ($PayloadFiles.Count -gt 0) {
            $PayloadMB = "{0:N0}" -f (($PayloadFiles | Measure-Object -Property Length -Sum).Sum / 1MB)
            "$($PayloadFiles.Count) file(s), $PayloadMB MB in data/copy -- boot asks whether to copy + install Wine"
        }
        else {
            "data/copy is empty (drop .exe files there; -Copy installs Wine and runs them)"
        }
        Write-Host "   * Copy Payload:  $PayloadStatus" -ForegroundColor White
    }
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
    Write-Host "     -Mirror <n|host|country>    Pick a download mirror without the prompt"
    Write-Host "                                 (targets with a mirror list, e.g. endeavouros)."
    Write-Host "     -Copy | -c                  Copy data/copy into the VM, install Wine and run"
    Write-Host "                                 every .exe in it (EndeavourOS; via the guest agent)."
    Write-Host "     -Program <name> | -p        Run only that program after the install (implies -Copy)."
    Write-Host "     -Setup auto|wine|opengoal   Guest setup script for -Copy (scripts/setup-<name>.sh; auto = by payload)."
    Write-Host "     -Profile <name|path|list>   Launch profile: target, VM size, payload, packages and commands in one"
    Write-Host "                                 file (profiles\<name>.psd1; see modules/profiles.ps1)."
    Write-Host "     -Gl                         Host-GPU OpenGL for the guest (virtio-vga-gl; -Setup opengoal implies it)."
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
            elseif (($BootChoice -eq "d" -or $BootChoice -eq "delete") -and $SelectedTarget.Runtime -eq "docker") {
                $Confirm = Read-Host "   Stop the '$($SelectedTarget.DockerProfile)' container and delete its data volume (the installed OS)? [y/N]"
                if ($Confirm -eq "y" -or $Confirm -eq "yes") {
                    & (Join-Path $PSScriptRoot "docker.ps1") -Profile $SelectedTarget.DockerProfile -Down -Purge
                    Start-Sleep -Seconds 1.5
                }
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
                            $DiskNamePattern = Get-TargetDiskName $SelectedTarget
                            $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
                            $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)
                            if ($CachedDisks.Count -gt 0) {
                                foreach ($cd in $CachedDisks) { Remove-Item $cd.FullName -Force; Remove-Item ([System.IO.Path]::ChangeExtension($cd.FullName, ".vars.fd")) -Force -ErrorAction SilentlyContinue }
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
                                    if ($SelectedTarget.IsFolder) {
                                        Remove-Item $wf.FullName -Recurse -Force
                                    }
                                    else {
                                        Remove-Item $wf.FullName -Force
                                    }
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
                            $DiskNamePattern = Get-TargetDiskName $SelectedTarget
                            $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
                            $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)
                            if ($CachedDisks.Count -gt 0) {
                                foreach ($cd in $CachedDisks) { Remove-Item $cd.FullName -Force; Remove-Item ([System.IO.Path]::ChangeExtension($cd.FullName, ".vars.fd")) -Force -ErrorAction SilentlyContinue }
                            }
                            
                            # 2. ISO
                            $FileNamePattern = if ($SelectedTarget.FileTemplate) { $SelectedTarget.FileTemplate -replace '\$v', '*' } else { $SelectedTarget.File }
                            if (-not $FileNamePattern) { $FileNamePattern = "$($SelectedTarget.Id).iso" }
                            $PatternPath = Join-Path $DataDir $FileNamePattern
                            $WildcardFiles = @(Get-Item $PatternPath -ErrorAction SilentlyContinue)
                            if ($WildcardFiles.Count -gt 0) {
                                foreach ($wf in $WildcardFiles) {
                                    if ($SelectedTarget.IsFolder) {
                                        Remove-Item $wf.FullName -Recurse -Force
                                    }
                                    else {
                                        Remove-Item $wf.FullName -Force
                                    }
                                }
                            }
                            
                            # 3. Pinned Trust Hash
                            $TrustFile = Join-Path $DataDir ".castle_trust.json"
                            if (Test-Path $TrustFile) {
                                $TrustStore = Get-PinnedTrustStore
                                $KeysToRemove = @($TrustStore.Keys) | Where-Object {
                                    $_ -eq $SelectedTarget.File -or
                                    $_ -eq "$($SelectedTarget.Id).iso" -or
                                    $_ -match "^$($SelectedTarget.Id)-.+\.(iso|zip|img\.gz)$"
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
        
        $DiskNamePattern = Get-TargetDiskName $Entry
        $DiskPatternPath = Join-Path $DataDir $DiskNamePattern
        $CachedDisks = @(Get-Item $DiskPatternPath -ErrorAction SilentlyContinue)

        $IsoIcon = if ($CachedIsos.Count -gt 0) { "[ISO]" } else { "[   ]" }
        $DiskIcon = if ($CachedDisks.Count -gt 0) { "[DISK]" } else { "      " }

        $SizeStr = ""
        if ($CachedIsos.Count -gt 0) {
            if ($Entry.IsFolder) {
                $FolderSize = (Get-ChildItem $CachedIsos[0].FullName -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
                $SizeGB = "{0:N1}" -f ($FolderSize / 1GB)
            }
            else {
                $SizeGB = "{0:N1}" -f ($CachedIsos[0].Length / 1GB)
            }
            $SizeStr = "($SizeGB GB)"
        }

        if ($Entry.Runtime -eq "docker") { $IsoIcon = "[DKR]"; $DiskIcon = "      "; $SizeStr = "" }
        if ($Entry.Runtime -eq "avd") { $IsoIcon = "[AVD]"; $DiskIcon = "      "; $SizeStr = "" }
        if ($Entry.Runtime -eq "container") { $IsoIcon = "[CTR]"; $DiskIcon = "      "; $SizeStr = "" }

        Write-Host "  $IsoIcon $DiskIcon  $($Entry.Id.PadRight(18)) $($Entry.Name) $SizeStr" -ForegroundColor White
    }

    Write-Host ""
    Write-Host "  [ISO] = ISO cached  [DISK] = VM disk exists  [   ] = Not downloaded  [DKR] = Docker container (docker.ps1)  [AVD] = Google Android Emulator (android.ps1)  [CTR] = container (container.ps1)" -ForegroundColor DarkGray
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

# Docker targets: hand over to docker.ps1 (no ISO pipeline, no QEMU).
if ($Matched[0].Runtime -eq "docker") {
    Write-Host ""
    Write-Host "  Castle VM Pipeline -> Docker" -ForegroundColor Cyan
    Write-Host "  Target: $($Matched[0].Name)  (docker.ps1 -Profile $($Matched[0].DockerProfile))" -ForegroundColor White
    Write-Host "  ================================================================" -ForegroundColor DarkGray
    & (Join-Path $PSScriptRoot "docker.ps1") -Profile $Matched[0].DockerProfile
    Exit $LASTEXITCODE
}

# Containers: container.ps1 builds the image once and runs the setup inside.
if ($Matched[0].Runtime -eq "container") {
    Write-Host ""
    Write-Host "  Castle VM Pipeline -> container (container.ps1)" -ForegroundColor Cyan
    $CArgs = @{ Target = $Matched[0].Id; Setup = $Setup; CopyDir = $CopyDir }
    if ($Instance) { $CArgs.Instance = $Instance }
    if ($Copy) { $CArgs.Copy = $true }
    if ($ProfileEnvFile) { $CArgs.ProfileEnv = $ProfileEnvFile }
    if ($ProfileData -and $ProfileData.Screen) { $CArgs.Screen = [string]$ProfileData.Screen }
    & (Join-Path $PSScriptRoot "container.ps1") @CArgs
    Exit $LASTEXITCODE
}

# Google Android Emulator: android.ps1 owns the SDK, the AVD and the boot.
if ($Matched[0].Runtime -eq "avd") {
    Write-Host ""
    Write-Host "  Castle VM Pipeline -> Android Emulator (android.ps1)" -ForegroundColor Cyan
    $AvdArgs = @{}
    if ($Copy) { $AvdArgs.Copy = $true }
    # CASTLE_AVD_PUSH=1: also push the payload's non-APK files to /sdcard/Download/castle-copy.
    if ($env:CASTLE_AVD_PUSH -eq "1") { $AvdArgs.Push = $true }
    & (Join-Path $PSScriptRoot "android.ps1") @AvdArgs
    Exit $LASTEXITCODE
}

# Apply mirror selection + Dynamic Version Resolution (Phase 2 feature)
$Iso = Resolve-TargetVersion -Target $Matched[0] -Mirror $Mirror
$IsoPath = Join-Path $DataDir $Iso.File

Write-Host ""
Write-Host "  Castle VM Pipeline" -ForegroundColor Cyan
Write-Host "  Target: $($Iso.Name)" -ForegroundColor White
Write-Host "  ================================================================" -ForegroundColor DarkGray

# ==============================================================================
# COPY PAYLOAD -- data/copy -> guest, Wine install, program launch
# ==============================================================================
$RunAll = $false
$Programs = @()
if ($Program) { $Programs = @($Program | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) }
if ($Programs.Count -gt 0) { $Copy = $true }

if ($Iso.SupportsCopy) {
    $CopyFiles = @(Get-CopyPayloadFiles)
    $CopyExes = @($CopyFiles | Where-Object { $_.Extension -ieq ".exe" } | Sort-Object FullName)
    $CopyRoot = if (Test-Path $CopyDir) { (Get-Item $CopyDir).FullName } else { $CopyDir }

    if (-not $Copy -and $CopyFiles.Count -gt 0 -and -not $Background) {
        # Interactive: the folder has something in it, ask before booting.
        Write-Host ""
        Write-Host "  [COPY] data/copy holds $($CopyFiles.Count) file(s):" -ForegroundColor Cyan
        foreach ($f in ($CopyFiles | Select-Object -First 12)) {
            $Rel = $f.FullName.Substring($CopyRoot.Length).TrimStart('\', '/')
            Write-Host ("     * {0}  ({1:N1} MB)" -f $Rel, ($f.Length / 1MB)) -ForegroundColor White
        }
        if ($CopyFiles.Count -gt 12) { Write-Host "     * ... and $($CopyFiles.Count - 12) more" -ForegroundColor DarkGray }
        $Answer = Read-Host "  Copy them into the VM, install Wine and run the Windows programs? [Y/n]"
        if ($Answer -notmatch '^(n|no)$') {
            $Copy = $true
            if ($CopyExes.Count -gt 1) {
                Write-Host ""
                for ($i = 0; $i -lt $CopyExes.Count; $i++) {
                    $Rel = $CopyExes[$i].FullName.Substring($CopyRoot.Length).TrimStart('\', '/')
                    Write-Host ("     [{0,2}] {1}" -f ($i + 1), $Rel) -ForegroundColor White
                }
                $Pick = Read-Host "  Run [A]ll .exe files, or pick one [1-$($CopyExes.Count)] (default: A)"
                $PickIndex = 0
                if ([int]::TryParse($Pick.Trim(), [ref]$PickIndex) -and $PickIndex -ge 1 -and $PickIndex -le $CopyExes.Count) {
                    $Programs = @($CopyExes[$PickIndex - 1].FullName.Substring($CopyRoot.Length).TrimStart('\', '/') -replace '\\', '/')
                }
                else {
                    $RunAll = $true
                }
            }
            else {
                $RunAll = $true
            }
        }
    }
    elseif ($Copy -and $Programs.Count -eq 0) {
        $RunAll = $true
    }

    if ($Copy) {
        # *.iso files are attached to the VM as extra CD-ROMs (a game disc,
        # any size). Everything else becomes the CASTLECOPY share: on Windows
        # an ISO image built with the built-in IMAPI2 (any size, disk-speed
        # reads), attached as a CD-ROM too; elsewhere QEMU's virtual FAT of a
        # hard-linked staging folder (fixed 504 MB, so over-cap files are
        # left out with a warning).
        $ShareCap = 480MB
        $ExtraCdroms = @()
        $ShareFiles = @()
        foreach ($f in $CopyFiles) {
            if ($f.Extension -ieq ".iso") { $ExtraCdroms += $f.FullName; continue }
            # A zip that only wraps an .iso already extracted next to it is
            # dead weight (the guest copies the share into RAM on a live ISO).
            if ($f.Extension -ieq ".zip") {
                $SevenZip = Get-SevenZip
                if ($SevenZip) {
                    $Entries = @(& $SevenZip l -ba -slt $f.FullName 2>$null | Where-Object { $_ -match '^Path = ' } | ForEach-Object { $_.Substring(7) })
                    if ($Entries.Count -eq 1 -and $Entries[0] -match '\.iso$' -and (Test-Path (Join-Path $f.DirectoryName ([System.IO.Path]::GetFileName($Entries[0]))))) {
                        Write-Host "  [COPY] Skipping $($f.Name): its ISO is already extracted next to it" -ForegroundColor DarkGray
                        continue
                    }
                }
            }
            $ShareFiles += $f
        }
        $ShareBytes = ($ShareFiles | Measure-Object -Property Length -Sum).Sum
        if ($null -eq $ShareBytes) { $ShareBytes = 0 }

        $SeedsDir = Join-Path $DataDir "seeds"
        $CopyIsoPath = Join-Path $SeedsDir "copy-share$(Get-LaunchSuffix).iso"
        $EffectiveCopyIso = $null
        $EffectiveCopyDir = $null
        if ($ShareFiles.Count -gt 0) {
            Write-Host "  [COPY] Building the payload image ($("{0:N0}" -f ($ShareBytes / 1MB)) MB)..." -ForegroundColor DarkGray
            $EffectiveCopyIso = New-CastlePayloadIso -SourceDir $CopyDir -OutputPath $CopyIsoPath -Label "CASTLECOPY" -IncludeFiles $ShareFiles
        }
        if (-not $EffectiveCopyIso) {
            $StageDir = Join-Path $SeedsDir "copy-share$(Get-LaunchSuffix)"
            if (Test-Path $StageDir) { Remove-Item $StageDir -Recurse -Force }
            New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
            $Kept = 0; $KeptBytes = 0
            foreach ($f in $ShareFiles) {
                if ($f.Length -gt $ShareCap -or ($KeptBytes + $f.Length) -gt $ShareCap) {
                    Write-Host "  [COPY] Skipping $($f.Name): $("{0:N0}" -f ($f.Length / 1MB)) MB does not fit the $("{0:N0}" -f ($ShareCap / 1MB)) MB virtual FAT share" -ForegroundColor Yellow
                    continue
                }
                $Rel = $f.FullName.Substring($CopyRoot.Length).TrimStart('\', '/')
                $Dest = Join-Path $StageDir $Rel
                $DestDir = Split-Path $Dest -Parent
                if (-not (Test-Path $DestDir)) { New-Item -ItemType Directory -Path $DestDir -Force | Out-Null }
                try { New-Item -ItemType HardLink -Path $Dest -Target $f.FullName -ErrorAction Stop | Out-Null }
                catch { Copy-Item $f.FullName $Dest -Force }
                $Kept++; $KeptBytes += $f.Length
            }
            $EffectiveCopyDir = $StageDir
        }

        if ($CopyFiles.Count -eq 0) {
            Write-Host "  [COPY] data/copy is empty -- $Setup setup will still run in the VM." -ForegroundColor Yellow
        }
        else {
            $What = if ($RunAll) { "run everything it finds" } elseif ($Programs.Count -gt 0) { "run $($Programs -join ', ')" } else { "install only" }
            $ShareKind = if ($EffectiveCopyIso) { "CASTLECOPY ISO" } else { "CASTLECOPY FAT share" }
            Write-Host "  [COPY] $($ShareFiles.Count) file(s), $("{0:N0}" -f ($ShareBytes / 1MB)) MB -> $ShareKind; $($ExtraCdroms.Count) disc ISO(s) as CD-ROM; setup-$Setup.sh, then $What" -ForegroundColor Cyan
        }
    }
}
elseif ($Copy) {
    Write-Host "  [FAIL] '$($Iso.Id)' does not support -Copy yet. Targets with SupportsCopy in targets.ps1: $(($IsoMatrix | Where-Object { $_.SupportsCopy } | ForEach-Object { $_.Id }) -join ', ')" -ForegroundColor Red
    Exit 1
}
if (-not $Copy) { $ExtraCdroms = @(); $EffectiveCopyIso = $null; $EffectiveCopyDir = $null }

# Scratch disk for -Copy runs: a sparse qcow2 next to the target's disk
# (data/scratch/<target>-scratch.qcow2). The guest keeps the payload copy, the
# Wine prefix and game builds on it instead of in RAM, and it persists, so a
# second run skips the rebuilds. CASTLE_VM_SCRATCH=0 keeps everything in RAM.
$ScratchDisk = $null
# Cloud-image targets keep everything on their own disk and need none.
if ($Copy -and $env:CASTLE_VM_SCRATCH -ne "0" -and $Iso.ImageKind -ne "cloud") {
    $ScratchName = if (-not [string]::IsNullOrEmpty($Instance)) { "$Instance-scratch.qcow2" } else { "$($Iso.Id)-scratch.qcow2" }
    $ScratchDisk = Join-Path (Join-Path $DataDir "scratch") $ScratchName
}

$SetupScript = "setup-$($Setup.ToLower()).sh"
if ($Copy -and -not (Test-Path (Join-Path (Join-Path $PSScriptRoot "scripts") $SetupScript))) {
    Write-Host "  [FAIL] Unknown setup '$Setup': scripts/$SetupScript does not exist." -ForegroundColor Red
    Exit 1
}
# -Gl stays strictly opt-in: on this host's QEMU (11.0, Windows) virtio-vga-gl
# with "-display sdl,gl=on" deadlocks QEMU's main loop before the guest even
# boots (QMP stops answering, one core spins). OpenGOAL runs on Mesa's
# llvmpipe inside the guest instead (OpenGL 4.5 in software; it needs 4.3).
if ($env:CASTLE_VM_GL -eq "1") { $Gl = $true }
if ($env:CASTLE_VM_GL -eq "0") { $Gl = $false }

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
        $SourceFolderName = $Iso.Id
        $SourceFolderPath = Join-Path $DataDir $SourceFolderName
        if (($Iso.Id -eq "win11-home" -or $Iso.Id -eq "win11-pro") -and (Test-Path $SourceFolderPath)) {
            $BuildOk = New-UnattendedWindowsIso -Target $Iso -SourceFolder $SourceFolderPath -OutputIsoPath $IsoPath
            if (-not $BuildOk) {
                Write-Host "  [FAIL] Failed to generate unattended ISO. Aborting." -ForegroundColor Red
                $HttpClient.Dispose()
                Exit 1
            }
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
    }

    # ==============================================================================
    # PHASE 2 -- INTEGRITY VERIFICATION (Always runs, even for cached ISOs)
    # ==============================================================================
    Write-Host ""
    Write-Host "  [*] Phase 2: Integrity Verification" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------" -ForegroundColor DarkGray

    # CASTLE_SKIP_VERIFY=1 skips every integrity layer (hash, GPG, trust pin):
    # for mirrors that publish no checksums or when the operator has verified the
    # image some other way. Nothing is pinned in that case.
    if ($env:CASTLE_SKIP_VERIFY -eq "1") {
        Write-Host "  [?] CASTLE_SKIP_VERIFY=1 -- integrity verification skipped, booting unverified image." -ForegroundColor Yellow
        $Verified = $true
    }
    else {
        $Verified = Test-IsoIntegrity -Target $Iso -FilePath $IsoPath -HttpClient $HttpClient
    }

    if (-not $Verified) {
        Write-Host ""
        Write-Host "  [FAIL] INTEGRITY CHECK FAILED -- Refusing to boot unverified image." -ForegroundColor Red
        Write-Host "     Set CASTLE_SKIP_VERIFY=1 to always bypass this check." -ForegroundColor DarkGray
        $Choice = Read-Host "     [D]elete and re-download / [B]ypass and boot unverified / [N]o (default: N)"
        if ($Choice -match "^(d|y|yes|delete)$") {
            Remove-Item $IsoPath -Force
            Write-Host "  [i] Deleted corrupted ISO. Retrying..." -ForegroundColor Yellow
        }
        elseif ($Choice -match "^(b|bypass)$") {
            Write-Host "  [?] Booting UNVERIFIED image (operator override). Nothing will be pinned." -ForegroundColor Yellow
            $Verified = $true
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
# DOWNLOAD-ONLY TARGETS -- Firmware images that are flashed, not booted
# ==============================================================================
if ($Iso.DownloadOnly) {
    Write-Host ""
    Write-Host "  [OK] Download-only target verified and cached: $IsoPath" -ForegroundColor Green
    Write-Host "  [i] This is handheld firmware -- flash/copy it to an SD card (e.g. balenaEtcher/Rufus)." -ForegroundColor DarkGray
    Write-Host "  [i] QEMU provisioning and boot are skipped for this target." -ForegroundColor DarkGray
    Exit 0
}

# ==============================================================================
# PHASE 3 -- DISK PROVISIONING
# ==============================================================================
Write-Host ""
Write-Host "  [*] Phase 3: Disk Provisioning" -ForegroundColor Cyan
Write-Host "  -------------------------------------------" -ForegroundColor DarkGray

if (-not (Test-QemuInstalled)) { Exit 1 }

$DiskPath = if (-not [string]::IsNullOrEmpty($Instance)) {
    Join-Path (Join-Path $DataDir "instances") "$Instance.qcow2"
}
else {
    Join-Path $DataDir (Get-TargetDiskName $Iso)
}
$DiskSize = if ($Iso.DiskSize) { $Iso.DiskSize } else { "40G" }
# Cloud images are never installed to: every VM disk is a linked clone. An
# -Instance of a cloud target clones the target's finished "golden" disk when
# one exists (data/<target>.qcow2, built by a plain `.\start.ps1 <target>` run),
# so it boots straight to the configured desktop and apps in seconds instead of
# re-running the whole first-boot package install; without a golden disk it
# falls back to the raw verified image. Shut the golden VM down before cloning
# it: a base that is being written to corrupts every clone.
$GoldenDisk = Join-Path $DataDir (Get-TargetDiskName $Iso)
$HasGolden = (Test-Path $GoldenDisk) -and ((Get-Item $GoldenDisk).Length -gt 1MB)
$EffectiveBase = if (-not [string]::IsNullOrEmpty($BaseDisk)) { $BaseDisk }
elseif ($Iso.ImageKind -eq "cloud" -and -not [string]::IsNullOrEmpty($Instance) -and $HasGolden) { Write-Host "  [DISK] Golden disk found -- instance will be a linked clone of it: $GoldenDisk" -ForegroundColor DarkGray; $GoldenDisk }
elseif ($Iso.ImageKind -eq "cloud") { $IsoPath }
else { $null }
$FirstBoot = New-VirtualDisk -DiskPath $DiskPath -DiskSize $DiskSize -BaseDisk $EffectiveBase
# Data-disk targets (BlissOS): the disk is a bare filesystem the live system
# mounts, never an install target. Format it once; an existing filesystem is
# always kept (Test-Ext4Disk reads the superblock, not the file size).
if ($Iso.DataDiskFs -eq "ext4") {
    # Both checks: a blank/new disk (New-VirtualDisk) that also has no ext4
    # superblock. Either one alone could misfire (a lazily initialised ext4
    # qcow2 is tiny; a disk locked by a running VM cannot be read).
    if ($FirstBoot -and -not (Test-Ext4Disk -DiskPath $DiskPath)) {
        Initialize-Ext4Disk -DiskPath $DiskPath -DiskSize $DiskSize | Out-Null
    }
    $FirstBoot = $false
}

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
    -Vnc:$Vnc `
    -CopyIso $EffectiveCopyIso `
    -CopyDir $EffectiveCopyDir `
    -ScratchDisk $ScratchDisk `
    -Programs $Programs `
    -RunAll:$RunAll `
    -SetupScript $SetupScript `
    -ProfileFile $ProfileEnvFile `
    -ExtraCdroms $ExtraCdroms `
    -Gl:$Gl