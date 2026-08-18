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

    # Boot order: CD-ROM first for fresh installs, disk first for existing VMs.
    # Existing VMs keep the CD as a fallback so a disk that never got installed to
    # drops through to the installer instead of dead-ending at the BIOS.
    $BootOrder = if ($FirstBoot) { "dc" } else { "cd" }

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
    )

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
            "-drive", "file=fat:ro:$ScriptsDir,format=raw,if=none,id=castlescripts,readonly=on"
            "-device", "usb-storage,drive=castlescripts"
        )
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


function New-UnattendedWindowsIso {
    param (
        [hashtable]$Target,
        [string]$SourceFolder,
        [string]$OutputIsoPath
    )

    Write-Host "  [ISO BUILD] Generating unattended ISO: $OutputIsoPath" -ForegroundColor Yellow

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
    $TemplateXmlPath = Join-Path $PhoenixDir "win-install\autounattend.xml"
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
        # win11-pro
        $XmlText = $XmlText -replace 'set \^"IMG_PARAM=/Name:\^"Windows 11 Pro\^"\^"', 'set ^"IMG_PARAM=/Index:1^"'
        $XmlText = $XmlText -replace '<ProductKey>VK7JG-NPHTM-C97JM-9MPGT-3V66T</ProductKey>', "<ProductKey>$ProKey</ProductKey>"
    }

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
    $OscdimgPath = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"
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
    $EtfsBoot = Join-Path $SourceFolder "boot\etfsboot.com"
    $EfiSys = Join-Path $SourceFolder "efi\microsoft\boot\efisys.bin"

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
