# ==============================================================================
# Castle VM -- QEMU Launch Engine
# ==============================================================================
# Builds per-OS QEMU argument arrays and launches virtual machines.
#
# OS Families:
#   linux   -- virtio disk, virtio VGA, standard boot
#   windows -- IDE disk, standard VGA, UEFI support for Win11
#   macos   -- Penryn CPU, special device flags (experimental)
# ==============================================================================

$IsLinuxOS = ($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true)
if ($IsLinuxOS) {
    $Script:QemuImg = "qemu-img"
    $Script:QemuSystem = "qemu-system-x86_64"
}
else {
    $Script:QemuDir = "C:\Program Files\qemu"
    $Script:QemuImg = Join-Path $Script:QemuDir "qemu-img.exe"
    $Script:QemuSystem = Join-Path $Script:QemuDir "qemu-system-x86_64.exe"
}


function Test-RunningInDocker {
    return (Test-Path "/.dockerenv") -or (-not [string]::IsNullOrEmpty($env:DOCKER_CONTAINER))
}


function Get-FreeVncDisplay {
    <#
    .SYNOPSIS
        Finds the first available TCP port for VNC starting at 5900
        and returns its display number (e.g. 5900 -> 0, 5901 -> 1).
    #>
    try {
        $TcpProperties = [System.Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties()
        $ActiveListeners = $TcpProperties.GetActiveTcpListeners()
        $ActivePorts = @($ActiveListeners | ForEach-Object { $_.Port })
    }
    catch {
        return 0
    }

    for ($Display = 0; $Display -lt 100; $Display++) {
        $Port = 5900 + $Display
        if ($Port -notin $ActivePorts) {
            return $Display
        }
    }
    return 0
}


function Test-QemuInstalled {
    <#
    .SYNOPSIS
        Checks if QEMU is installed and accessible. Returns $true/$false.
    #>
    $IsLinuxOS = ($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true)
    if ($IsLinuxOS) {
        if (Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue) { return $true }
        Write-Host "  [FAIL] QEMU not found. Please install QEMU using your package manager." -ForegroundColor Red
        Write-Host "     Example: apt-get install -y qemu-system-x86 qemu-utils" -ForegroundColor DarkGray
        return $false
    }
    else {
        if (Test-Path $Script:QemuSystem) { return $true }
        if (Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue) {
            $Script:QemuSystem = (Get-Command "qemu-system-x86_64").Source
            $Script:QemuImg = (Get-Command "qemu-img").Source
            return $true
        }
        Write-Host "  [FAIL] QEMU not found at: $Script:QemuDir" -ForegroundColor Red
        Write-Host "     Install QEMU for Windows: https://www.qemu.org/download/#windows" -ForegroundColor DarkGray
        return $false
    }
}


function New-VirtualDisk {
    <#
    .SYNOPSIS
        Creates a new QCOW2 virtual disk if one does not already exist.
        Supports QEMU backing-file linked clones.
    .OUTPUTS
        $true if this is a fresh disk (first boot), $false if disk already existed.
    #>
    param (
        [string]$DiskPath,
        [string]$DiskSize = "40G",
        [string]$BaseDisk = $null
    )

    if (Test-Path $DiskPath) {
        $DiskGB = "{0:N2}" -f ((Get-Item $DiskPath).Length / 1GB)
        Write-Host "  [DISK] Virtual disk exists: $DiskPath ($DiskGB GB on disk)" -ForegroundColor Green
        return $false
    }

    # Ensure parent directory exists
    $ParentDir = Split-Path $DiskPath -Parent
    if (-not (Test-Path $ParentDir)) {
        New-Item -ItemType Directory -Path $ParentDir -Force | Out-Null
    }

    if (-not [string]::IsNullOrEmpty($BaseDisk)) {
        if (-not (Test-Path $BaseDisk)) {
            Write-Host "  [FAIL] Base backing disk not found: $BaseDisk" -ForegroundColor Red
            Exit 1
        }
        $AbsoluteBase = (Get-Item $BaseDisk).FullName
        Write-Host "  [DISK] Creating linked clone backed by base: $AbsoluteBase" -ForegroundColor Yellow
        & $Script:QemuImg create -f qcow2 -b $AbsoluteBase -F qcow2 $DiskPath | Out-Null
    }
    else {
        Write-Host "  [DISK] Creating $DiskSize virtual disk: $DiskPath" -ForegroundColor Yellow
        & $Script:QemuImg create -f qcow2 $DiskPath $DiskSize | Out-Null
    }
    return $true
}


function Build-QemuArgs {
    <#
    .SYNOPSIS
        Constructs a QEMU argument array based on target OS family and hardware profile.
    #>
    param (
        [hashtable]$Target,
        [string]$DiskPath,
        [string]$IsoPath,
        [hashtable]$Hardware,
        [bool]$FirstBoot = $false,
        [bool]$Vnc = $false,
        [int]$VncDisplay = 0
    )

    $OsFamily = if ($Target.OsFamily) { $Target.OsFamily } else { "linux" }

    # Boot order: CD-ROM first for fresh installs, disk first for existing VMs
    $BootOrder = if ($FirstBoot) { "d" } else { "c" }

    # Per-OS defaults
    $DiskInterface = switch ($OsFamily) {
        "windows" { "ide" }
        "macos" { "ide" }
        default { "virtio" }
    }

    $VgaType = switch ($OsFamily) {
        "windows" { "std" }
        "macos" { "std" }
        default { "virtio" }
    }

    $IsLinuxOS = ($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true)
    $Accel = if ($IsLinuxOS) { "kvm" } else { "whpx" }

    # -- Base arguments (universal) --
    $Args = @(
        "-accel", $Accel
        "-accel", "tcg"
        "-cpu", $Hardware.CpuProfile
        "-smp", $Hardware.CpuCores
        "-m", $Hardware.Memory
        "-drive", "file=$DiskPath,if=$DiskInterface,format=qcow2"
        "-drive", "file=$IsoPath,media=cdrom,readonly=on"
        "-boot", "order=$BootOrder"
        "-vga", $VgaType
        "-usb"
        "-device", "usb-tablet"
    )

    # Attach the /scripts folder as a Virtual FAT drive
    $ScriptsDir = Join-Path (Split-Path $PSScriptRoot -Parent) "scripts"
    if (Test-Path $ScriptsDir) {
        $Args += @("-drive", "file=fat:rw:$ScriptsDir,format=raw,media=disk")
    }

    if ($Vnc) {
        $VncBind = if (Test-RunningInDocker) { "0.0.0.0" } else { "127.0.0.1" }
        $Args += @("-vnc", "$($VncBind):$($VncDisplay)")
    }
    else {
        $Args += @("-display", "gtk")
    }

    # -- UEFI firmware (Windows 11, modern targets) --
    if ($Target.Firmware -eq "uefi") {
        $OvmfCandidates = @(
            (Join-Path $Script:QemuDir "share\edk2-x86_64-code.fd"),
            (Join-Path $Script:QemuDir "share\OVMF_CODE.fd"),
            (Join-Path $Script:QemuDir "share\OVMF.fd")
        )
        $OvmfPath = $OvmfCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1

        if ($OvmfPath) {
            $Args += @("-bios", $OvmfPath)
            Write-Host "  [*] Firmware: UEFI ($OvmfPath)" -ForegroundColor White
        }
        else {
            Write-Host "  [?] UEFI firmware (OVMF) not found -- falling back to BIOS." -ForegroundColor Yellow
            Write-Host "     Windows 11 requires UEFI. Install OVMF in: $Script:QemuDir\share\" -ForegroundColor DarkGray
        }
    }

    # -- macOS-specific note (experimental) --
    if ($OsFamily -eq "macos") {
        Write-Host "  [?] macOS support is experimental. Additional setup may be required." -ForegroundColor Yellow
    }

    return $Args
}


function Start-CastleVm {
    <#
    .SYNOPSIS
        Launches a QEMU virtual machine with the constructed argument array.
    #>
    param (
        [hashtable]$Target,
        [string]$DiskPath,
        [string]$IsoPath,
        [hashtable]$Hardware,
        [bool]$FirstBoot = $false,
        [switch]$Background = $false,
        [switch]$Vnc = $false
    )

    $OsFamily = if ($Target.OsFamily) { $Target.OsFamily } else { "linux" }
    $BootMode = if ($FirstBoot) { "CD-ROM (install)" } else { "Disk (existing)" }
    $DiskIf = if ($OsFamily -eq "windows" -or $OsFamily -eq "macos") { "ide" } else { "virtio" }

    $VncDisplay = 0
    $VncBind = "127.0.0.1"
    if ($Vnc) {
        $VncDisplay = Get-FreeVncDisplay
        $VncBind = if (Test-RunningInDocker) { "0.0.0.0" } else { "127.0.0.1" }
    }

    Write-Host ""
    Write-Host "  [BOOT] Launching VM" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------" -ForegroundColor DarkGray
    Write-Host "  * Target:    $($Target.Name)" -ForegroundColor White
    Write-Host "  * OS Family: $OsFamily" -ForegroundColor White
    Write-Host "  * Boot:      $BootMode" -ForegroundColor White
    Write-Host "  * CPU:       $($Hardware.CpuCores) cores -- $($Hardware.CpuProfile)" -ForegroundColor White
    Write-Host "  * RAM:       $($Hardware.Memory)" -ForegroundColor White
    Write-Host "  * Disk:      $DiskIf interface" -ForegroundColor White
    if ($Vnc) {
        $VncPort = 5900 + $VncDisplay
        Write-Host "  * VNC:       Listening on $($VncBind):$($VncPort) (Display :$VncDisplay)" -ForegroundColor Green
    }

    $QemuArgs = Build-QemuArgs `
        -Target $Target `
        -DiskPath $DiskPath `
        -IsoPath $IsoPath `
        -Hardware $Hardware `
        -FirstBoot $FirstBoot `
        -Vnc $Vnc `
        -VncDisplay $VncDisplay

    if ($Background) {
        Write-Host "  [BOOT] Launching VM in the background (asynchronous)..." -ForegroundColor Green
        $Process = Start-Process -FilePath $Script:QemuSystem -ArgumentList $QemuArgs -NoNewWindow -PassThru
        Write-Host "  [BOOT] Background VM Process started (PID: $($Process.Id))." -ForegroundColor Green
    }
    else {
        Write-Host ""
        Write-Host "  Castle VM booting..." -ForegroundColor Green
        if ($Vnc) {
            Write-Host "  VNC mode active. Connect a VNC client to display." -ForegroundColor DarkGray
        }
        else {
            Write-Host "  Close the QEMU window to shut down." -ForegroundColor DarkGray
        }
        Write-Host ""
        & $Script:QemuSystem $QemuArgs
    }
}
