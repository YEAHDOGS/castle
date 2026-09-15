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
    # Windows-subsystem build: no console window. Used for background launches so a
    # detached VM does not leave a stray command prompt on the desktop.
    $Script:QemuSystemWindowed = Join-Path $Script:QemuDir "qemu-system-x86_64w.exe"
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


function Get-TargetDiskName {
    <#
    .SYNOPSIS
        Name of a target's default VM disk under data/. Cloud-image targets share
        one downloaded image between several targets, so their disks are keyed by
        target Id instead of by download file name.
    #>
    param ([hashtable]$Target)

    if ($Target.ImageKind -eq "cloud") { return "$($Target.Id).qcow2" }
    if ($Target.File) { return ($Target.File -replace '\.(iso|img\.gz|zip)$', '.qcow2') }
    if ($Target.FileTemplate) { return (($Target.FileTemplate -replace '\$v', '*') -replace '\.(iso|img\.gz|zip)$', '.qcow2') }
    return "$($Target.Id).qcow2"
}


function ConvertTo-Bytes {
    param ([string]$Size)
    if ($Size -match '^\s*(\d+)\s*([KMGT])?B?\s*$') {
        $n = [int64]$Matches[1]
        switch ($Matches[2]) {
            "K" { return $n * 1KB }
            "M" { return $n * 1MB }
            "G" { return $n * 1GB }
            "T" { return $n * 1TB }
            default { return $n }
        }
    }
    return 0
}


function Get-FreeTcpPort {
    param ([int]$Start = 2222)
    $Active = @()
    try {
        $Active = [System.Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | ForEach-Object { $_.Port }
    }
    catch { }
    for ($p = $Start; $p -lt $Start + 200; $p++) {
        if ($p -notin $Active) { return $p }
    }
    return $Start
}


function New-CloudInitSeed {
    <#
    .SYNOPSIS
        Materializes the NoCloud seed folder for one VM: the target's shared
        user-data plus a generated meta-data carrying a per-VM instance id and
        hostname, so linked clones in a swarm do not collide.
    .OUTPUTS
        Path of the seed folder (served to the guest as a FAT volume labelled CIDATA).
    #>
    param (
        [hashtable]$Target,
        [string]$DiskPath
    )

    $ProfileName = if ($Target.CloudInit) { $Target.CloudInit } else { "server" }
    $ProfileDir = Join-Path (Join-Path (Join-Path (Split-Path $PSScriptRoot -Parent) "scripts") "cloud-init") $ProfileName
    $UserData = Join-Path $ProfileDir "user-data"
    if (-not (Test-Path $UserData)) {
        Write-Host "  [FAIL] cloud-init profile not found: $UserData" -ForegroundColor Red
        Exit 1
    }

    $VmName = [System.IO.Path]::GetFileNameWithoutExtension($DiskPath)
    $SeedDir = Join-Path (Join-Path (Split-Path $DiskPath -Parent) "seeds") $VmName
    New-Item -ItemType Directory -Path $SeedDir -Force | Out-Null
    Copy-Item $UserData (Join-Path $SeedDir "user-data") -Force
    $MetaData = "instance-id: castle-$VmName`nlocal-hostname: $VmName`n"
    [System.IO.File]::WriteAllText((Join-Path $SeedDir "meta-data"), $MetaData)
    return $SeedDir
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
        $DiskBytes = (Get-Item $DiskPath).Length
        $DiskGB = "{0:N2}" -f ($DiskBytes / 1GB)

        # A qcow2 that was created but never installed to holds only metadata (~200 KB).
        # Booting it from disk lands on "no bootable device", so treat it as a first boot.
        # Linked clones are legitimately tiny, so skip the check when a backing file exists.
        $HasBacking = $false
        try {
            $Info = & $Script:QemuImg info --output=json $DiskPath 2>$null | ConvertFrom-Json
            if ($Info.'backing-filename') { $HasBacking = $true }
        }
        catch { }

        if ($DiskBytes -lt 4MB -and -not $HasBacking) {
            Write-Host "  [DISK] Virtual disk exists but is blank: $DiskPath" -ForegroundColor Yellow
            Write-Host "  [DISK] Nothing installed yet -- booting the installer from CD-ROM." -ForegroundColor Yellow
            return $true
        }

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
        $CloneArgs = @("create", "-f", "qcow2", "-b", $AbsoluteBase, "-F", "qcow2", $DiskPath)
        # A clone may be larger than its base (cloud images ship ~3.5G virtual);
        # cloud-init's growpart then expands the root filesystem on first boot.
        try {
            $BaseInfo = & $Script:QemuImg info --output=json $AbsoluteBase 2>$null | ConvertFrom-Json
            if ((ConvertTo-Bytes $DiskSize) -gt [int64]$BaseInfo.'virtual-size') { $CloneArgs += $DiskSize }
        }
        catch { }
        & $Script:QemuImg @CloneArgs | Out-Null

        # A UEFI base keeps its boot entries in its NVRAM file. Seed the clone
        # with a copy so it boots the installed OS immediately instead of
        # depending on the firmware's fallback device enumeration.
        $BaseVars = [System.IO.Path]::ChangeExtension($AbsoluteBase, ".vars.fd")
        $CloneVars = [System.IO.Path]::ChangeExtension($DiskPath, ".vars.fd")
        if ((Test-Path $BaseVars) -and -not (Test-Path $CloneVars)) {
            Copy-Item $BaseVars $CloneVars
        }
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
    $IsCloudImage = ($Target.ImageKind -eq "cloud")

    # Boot order: CD-ROM first for fresh installs, disk first for existing VMs.
    # Existing VMs keep the CD as a fallback so a disk that never got installed to
    # drops through to the installer instead of dead-ending at the BIOS.
    # Cloud images have no installer: always boot the (cloned) disk.
    $BootOrder = if ($IsCloudImage) { "c" } elseif ($FirstBoot) { "dc" } else { "cd" }

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
    # WHPX: Linux guests get kernel-irqchip=off (the APIC is emulated in QEMU
    # instead of the hypervisor), which is the usual WHPX advice for Linux.
    # Windows guests keep the default: WinPE and the installed system boot fine
    # with it, and Windows stalled on a black screen after the bootloader with
    # the APIC routed through QEMU.
    $Accel = if ($IsLinuxOS) { "kvm" } elseif ($OsFamily -eq "linux") { "whpx,kernel-irqchip=off" } else { "whpx" }

    # -- Base arguments (universal) --
    # Windows guests get the Q35 chipset so their "ide" drives land on its AHCI
    # (SATA) controller instead of the PIIX legacy IDE bus. Windows has an inbox
    # AHCI driver, and install/boot I/O is several times faster than emulated
    # IDE. Linux targets keep the default machine (they use virtio anyway).
    # Q35 (AHCI) was tried for Windows but the installed system hangs on a black
    # screen after the bootloader under WHPX; the default machine (PIIX IDE) boots.
    # Opt back in for experiments with CASTLE_VM_MACHINE=q35.
    $MachineArgs = if ($OsFamily -eq "windows" -and $env:CASTLE_VM_MACHINE -eq "q35") { @("-machine", "q35") } else { @() }

    # A disk that is still being installed to is throwaway until the install
    # finishes, so guest flush requests are skipped (cache=unsafe) to speed up
    # the installer's write-heavy phases. A host crash mid-install just means
    # reinstalling. Installed disks get the normal writeback cache.
    $DiskCache = if ($FirstBoot) { "unsafe" } else { "writeback" }

    $Args = @(
        "-accel", $Accel
        "-accel", "tcg"
    ) + $MachineArgs + @(
        "-cpu", $Hardware.CpuProfile
        "-smp", $Hardware.CpuCores
        "-m", $Hardware.Memory
        "-drive", "file=$DiskPath,if=$DiskInterface,format=qcow2,cache=$DiskCache,discard=unmap"
    )

    if ($IsCloudImage) {
        # The seed folder is exposed as a read-only FAT volume labelled CIDATA,
        # which cloud-init's NoCloud datasource picks up on first boot. SSH is
        # port-forwarded from the host through QEMU's user-mode network.
        $SeedDir = New-CloudInitSeed -Target $Target -DiskPath $DiskPath
        $SshPort = Get-FreeTcpPort -Start 2222
        $Args += @(
            "-blockdev", "driver=vvfat,node-name=cidata,dir=$SeedDir,label=CIDATA,rw=off,read-only=on"
            "-device", "virtio-blk-pci,drive=cidata"
            "-nic", "user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:$SshPort-:22"
        )
        Write-Host "  [*] cloud-init: $SeedDir" -ForegroundColor White
        Write-Host "  [*] SSH:        ssh castle@127.0.0.1 -p $SshPort   (once cloud-init finishes)" -ForegroundColor Green
    }
    elseif ($FirstBoot -or -not ($OsFamily -eq "windows" -and $Target.Firmware -eq "uefi")) {
        $Args += @("-drive", "file=$IsoPath,media=cdrom,readonly=on")
    }
    else {
        # An installed UEFI Windows disk never gets the installer CD back: the
        # unattended ISO boots without a keypress and its answer file wipes disk 0,
        # so if the firmware ever picked the CD first it would reinstall over a
        # finished machine. Delete the disk (-DeleteDisk) to reinstall on purpose.
        Write-Host "  [*] Installer CD detached (disk already installed)." -ForegroundColor DarkGray
    }

    $Args += @(
        "-boot", "order=$BootOrder"
        "-vga", $VgaType
        "-usb"
        "-device", "usb-tablet"
    )

    # Attach the /scripts folder as a Virtual FAT drive (skip for android target).
    # It rides on USB rather than the default IDE bus: ide-hd refuses a read-only
    # backing node ("Block node is read-only"), and USB keeps the share out of the
    # hard-disk enumeration so it can never shadow the real boot disk.
    $ScriptsDir = Join-Path (Split-Path $PSScriptRoot -Parent) "scripts"
    if ((Test-Path $ScriptsDir) -and $Target.Id -ne "android") {
        $Args += @(
            # Labelled CASTLE so guests can mount it by label (LABEL=CASTLE / /dev/disk/by-label/CASTLE).
            "-blockdev", "driver=vvfat,node-name=castlescripts,dir=$ScriptsDir,label=CASTLE,rw=off,read-only=on"
            "-device", "usb-storage,drive=castlescripts"
        )
    }

    # QMP control socket (loopback only) so the pipeline and tooling can ask the
    # guest to power off cleanly, query state, or take a screendump without
    # touching the window. The port is printed at boot.
    $QmpPort = Get-FreeTcpPort -Start 4444
    $Args += @("-qmp", "tcp:127.0.0.1:$QmpPort,server=on,wait=off")
    Write-Host "  [*] QMP:        127.0.0.1:$QmpPort" -ForegroundColor White

    # QEMU guest agent channel (loopback). Guests that run qemu-guest-agent (the
    # cloud-init profiles install it) accept guest-exec / guest-file-read over it,
    # which is how tooling can read logs or run commands inside a VM without a
    # login. Harmless for guests without the agent.
    $QgaPort = Get-FreeTcpPort -Start ($QmpPort + 100)
    $Args += @(
        "-chardev", "socket,id=qga0,host=127.0.0.1,port=$QgaPort,server=on,wait=off"
        "-device", "virtio-serial"
        "-device", "virtserialport,chardev=qga0,name=org.qemu.guest_agent.0"
    )
    Write-Host "  [*] Guest agent: 127.0.0.1:$QgaPort" -ForegroundColor White

    if ($Vnc) {
        $VncBind = if (Test-RunningInDocker) { "0.0.0.0" } else { "127.0.0.1" }
        $Args += @("-vnc", "$($VncBind):$($VncDisplay)")
    }
    else {
        $Args += @("-display", "gtk")
    }

    # -- UEFI firmware (Windows 11, modern targets) --
    if ($Target.Firmware -eq "uefi") {
        # Split code/vars firmware images are loaded as two pflash drives rather
        # than via -bios: QEMU's -bios loader requires a 64K-multiple ROM, which
        # the bundled edk2-x86_64-code.fd is not ("could not load PC BIOS").
        # pflash also gives the guest a writable NVRAM, so UEFI boot entries
        # survive reboots. Each VM disk gets its own vars copy next to it
        # (<disk>.vars.fd), seeded from the platform's vars template.
        #
        # OVMF lives in different places per platform. On Linux the Windows
        # QEMU-dir variable is unset, so probe well-known system paths
        # (Debian/Ubuntu ship them in /usr/share/OVMF via the 'ovmf' package).
        $FirmwarePairs = if ($IsLinuxOS) {
            @(
                @{ Code = "/usr/share/OVMF/OVMF_CODE_4M.fd";     Vars = "/usr/share/OVMF/OVMF_VARS_4M.fd" },
                @{ Code = "/usr/share/OVMF/OVMF_CODE.fd";        Vars = "/usr/share/OVMF/OVMF_VARS.fd" },
                @{ Code = "/usr/share/edk2/x86_64/OVMF_CODE.fd"; Vars = "/usr/share/edk2/x86_64/OVMF_VARS.fd" },
                @{ Code = "/usr/share/edk2/ovmf/OVMF_CODE.fd";   Vars = "/usr/share/edk2/ovmf/OVMF_VARS.fd" },
                @{ Code = "/usr/share/qemu/OVMF.fd";             Vars = $null }
            )
        }
        else {
            $ShareDir = Join-Path $Script:QemuDir "share"
            @(
                # QEMU's own firmware descriptors pair the x86_64 code image with edk2-i386-vars.fd.
                @{ Code = (Join-Path $ShareDir "edk2-x86_64-code.fd"); Vars = (Join-Path $ShareDir "edk2-i386-vars.fd") },
                @{ Code = (Join-Path $ShareDir "OVMF_CODE.fd");        Vars = (Join-Path $ShareDir "OVMF_VARS.fd") },
                @{ Code = (Join-Path $ShareDir "OVMF.fd");             Vars = $null }
            )
        }
        $OvmfHint = if ($IsLinuxOS) { "/usr/share/OVMF/  (install the 'ovmf' package)" } else { (Join-Path $Script:QemuDir "share") }
        $Firmware = $FirmwarePairs | Where-Object { Test-Path $_.Code } | Select-Object -First 1

        if ($Firmware) {
            # pflash unit order matters: unit 0 is the read-only code image, unit 1 the vars store.
            $Args += @("-drive", "if=pflash,format=raw,readonly=on,file=$($Firmware.Code)")
            if ($Firmware.Vars -and (Test-Path $Firmware.Vars)) {
                $VarsPath = [System.IO.Path]::ChangeExtension($DiskPath, ".vars.fd")
                if (-not (Test-Path $VarsPath)) {
                    Copy-Item $Firmware.Vars $VarsPath
                }
                $Args += @("-drive", "if=pflash,format=raw,file=$VarsPath")
                Write-Host "  [*] Firmware: UEFI ($($Firmware.Code))" -ForegroundColor White
                Write-Host "  [*] NVRAM:    $VarsPath" -ForegroundColor White
            }
            else {
                Write-Host "  [*] Firmware: UEFI ($($Firmware.Code)) -- no vars template found, NVRAM will not persist" -ForegroundColor White
            }
        }
        else {
            Write-Host "  [?] UEFI firmware (OVMF) not found -- falling back to BIOS." -ForegroundColor Yellow
            Write-Host "     Windows 11 requires UEFI. Install OVMF in: $OvmfHint" -ForegroundColor DarkGray
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
    $BootMode = if ($Target.ImageKind -eq "cloud") { if ($FirstBoot) { "Disk (cloud image, first boot: cloud-init)" } else { "Disk (cloud image)" } } elseif ($FirstBoot) { "CD-ROM (install)" } else { "Disk (existing)" }
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
        # Start-Process joins -ArgumentList with spaces and does not quote, so any
        # argument containing whitespace (e.g. a firmware path under Program Files)
        # must be wrapped here. The foreground call operator below quotes on its own.
        $QuotedArgs = $QemuArgs | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }
        $QemuExe = if ($Script:QemuSystemWindowed -and (Test-Path $Script:QemuSystemWindowed)) { $Script:QemuSystemWindowed } else { $Script:QemuSystem }
        $Process = Start-Process -FilePath $QemuExe -ArgumentList $QuotedArgs -NoNewWindow -PassThru
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


function New-UnattendedWindowsIso {
    param (
        [hashtable]$Target,
        [string]$SourceFolder,
        [string]$OutputIsoPath
    )

    Write-Host "  [ISO BUILD] Generating unattended ISO: $OutputIsoPath" -ForegroundColor Yellow

    # oscdimg.exe is a Windows-only utility -- fail fast with a clear message
    # instead of cascading into confusing path errors on Linux/macOS.
    $IsWindowsOS = -not (($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true) -or ($IsMacOS -eq $true))
    if (-not $IsWindowsOS) {
        Write-Host "  [FAIL] Unattended Windows ISO generation requires Windows (oscdimg.exe)." -ForegroundColor Red
        Write-Host "     Generate the ISO on your Windows host, then copy it into clonewars/data/." -ForegroundColor DarkGray
        return $false
    }

    # 1. Load keys from .env
    $EnvVars = @{}
    $EnvFile = Join-Path (Split-Path $PSScriptRoot -Parent) ".env"
    if (Test-Path $EnvFile) {
        Get-Content $EnvFile | ForEach-Object {
            if ($_ -match '^\s*([^#=\s]+)\s*=\s*(.*)$') {
                $Key = $Matches[1].Trim()
                $Val = $Matches[2].Trim() -replace "^['`"]", "" -replace "['`"]$", ""
                $EnvVars[$Key] = $Val
            }
        }
    }

    $HomeKey = if ($EnvVars.ContainsKey("WINDOWS_HOME_KEY")) { $EnvVars["WINDOWS_HOME_KEY"] } else { "YTMG3-N6KCE-XXBTM-8D96T-YQRFF" }
    $ProKey = if ($EnvVars.ContainsKey("WINDOWS_PRO_KEY")) { $EnvVars["WINDOWS_PRO_KEY"] } else { "VK7JG-NPHTM-C97JM-9MPGT-3V66T" }

    # 2. Read template autounattend.xml from phoenix project
    $PhoenixDir = Join-Path (Split-Path (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent) -Parent) "phoenix"
    $TemplateXmlPath = Join-Path (Join-Path $PhoenixDir "win-install") "autounattend.xml"
    if (-not (Test-Path $TemplateXmlPath)) {
        Write-Host "  [FAIL] Unattended XML template not found at: $TemplateXmlPath" -ForegroundColor Red
        return $false
    }

    $XmlText = Get-Content $TemplateXmlPath -Raw

    # 3. Replace keys and image select based on target Id
    if ($Target.Id -eq "win11-home") {
        $XmlText = $XmlText -replace 'set \^"IMG_PARAM=/Name:\^"Windows 11 Pro\^"\^"', 'set ^"IMG_PARAM=/Index:1^"'
        $XmlText = $XmlText -replace '<ProductKey>VK7JG-NPHTM-C97JM-9MPGT-3V66T</ProductKey>', "<ProductKey>$HomeKey</ProductKey>"
        $XmlText = $XmlText -replace '<InstallFromName>Windows 11 Pro</InstallFromName>', "<InstallFromName>Windows 11 Home</InstallFromName>"
    }
    else {
        # win11-pro: keep the template's /Name:"Windows 11 Pro" selector. Retail
        # multi-edition media puts Home at index 1 and Pro at index 6, so forcing
        # /Index:1 here would install Home with a Pro key.
        $XmlText = $XmlText -replace '<ProductKey>VK7JG-NPHTM-C97JM-9MPGT-3V66T</ProductKey>', "<ProductKey>$ProKey</ProductKey>"
    }

    # 3b. Speed: drop dism's /CheckIntegrity /Verify from the WinPE apply step.
    # They re-hash every file as it is written, which roughly doubles the apply
    # time on an emulated disk; the media was already verified when extracted.
    $XmlText = $XmlText -replace '\s+/CheckIntegrity /Verify', ''

    # 4. Inject Chocolatey installation command into FirstLogonCommands
    $ChocoCommand = @'
				<SynchronousCommand wcm:action="add">
					<Order>2</Order>
					<CommandLine>powershell.exe -WindowStyle "Normal" -ExecutionPolicy "Bypass" -NoProfile -Command "Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1')); choco install 7zip notepadplusplus git vscode vlc curl sysinternals -y"</CommandLine>
				</SynchronousCommand>
'@
    $XmlText = $XmlText -replace '(<FirstLogonCommands>[\s\S]*?<\/SynchronousCommand>\s*)(<\/FirstLogonCommands>)', "`$1`r`n$ChocoCommand`r`n`$2"

    # 5. Save autounattend.xml temporarily to source folder
    $TempXmlPath = Join-Path $SourceFolder "autounattend.xml"
    $XmlText | Set-Content $TempXmlPath -Force -NoNewline

    # 6. Locate oscdimg.exe
    $OscdimgPath = Join-Path (Join-Path (Join-Path $env:LOCALAPPDATA "Microsoft") "WinGet") "Packages"
    $OscdimgFile = Get-ChildItem -Path $OscdimgPath -Filter "oscdimg.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName -First 1

    if (-not $OscdimgFile) {
        # Fallback to PATH search
        $OscdimgFile = Get-Command "oscdimg.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
    }

    if (-not $OscdimgFile) {
        Write-Host "  [FAIL] oscdimg.exe utility not found. Please ensure Microsoft.OSCDIMG is installed." -ForegroundColor Red
        if (Test-Path $TempXmlPath) { Remove-Item $TempXmlPath -Force }
        return $false
    }

    # 7. Compile ISO
    $EtfsBoot = Join-Path (Join-Path $SourceFolder "boot") "etfsboot.com"
    # Prefer the no-prompt UEFI boot sector: with efisys.bin the installer shows
    # "Press any key to boot from CD or DVD", and when nobody presses one the
    # firmware falls through to the blank disk and PXE, so an unattended install
    # never starts. Retail media ships efisys_noprompt.bin alongside it.
    $EfiBootDir = Join-Path (Join-Path (Join-Path $SourceFolder "efi") "microsoft") "boot"
    $EfiSys = Join-Path $EfiBootDir "efisys_noprompt.bin"
    if (-not (Test-Path $EfiSys)) { $EfiSys = Join-Path $EfiBootDir "efisys.bin" }

    Write-Host "  [ISO BUILD] Compiling bootable ISO..." -ForegroundColor Yellow
    $Process = Start-Process -FilePath $OscdimgFile -ArgumentList "-m", "-o", "-u2", "-udfver102", "-bootdata:2#p0,e,b`"$EtfsBoot`"#pEF,e,b`"$EfiSys`"", "`"$SourceFolder`"", "`"$OutputIsoPath`"" -NoNewWindow -PassThru -Wait

    # 8. Cleanup temporary XML
    if (Test-Path $TempXmlPath) {
        Remove-Item $TempXmlPath -Force
    }

    if ($Process.ExitCode -eq 0) {
        Write-Host "  [OK] Unattended ISO generated successfully: $OutputIsoPath" -ForegroundColor Green
        return $true
    }
    else {
        Write-Host "  [FAIL] oscdimg.exe failed with exit code: $($Process.ExitCode)" -ForegroundColor Red
        return $false
    }
}
