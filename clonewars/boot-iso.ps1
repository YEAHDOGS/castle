<#
.SYNOPSIS
    Boot any ISO file in QEMU with a virtual disk of the size you choose.
.DESCRIPTION
    Point it at an ISO (EndeavourOS, Ubuntu, Kali, Arch, anything bootable) and
    a disk size. It creates a qcow2 of that size under data/adhoc/, boots the
    installer from the ISO on the first run, and boots the installed disk on
    every later run (the ISO stays attached as a CD for repairs). No manifest
    entry, no download: this is for images you already have on disk.

    Verification is optional. Hand it a checksum (.sha512 / .sha512sum /
    .sha256 / SHA256SUMS / bare hex) and/or a detached GPG signature (.sig /
    .asc, of the ISO or of the checksum file, as Ubuntu and Kali do) and it
    checks the ISO before booting. Files sitting next to the ISO under the
    usual names are picked up automatically; -NoVerify skips them. Nothing is
    verified unless something was given or found, and only a failing check
    that was actually requested refuses to boot (-Force overrides).

    Reuses the pipeline's modules (modules/launch.ps1, hardware.ps1, verify.ps1)
    so the VM gets the same accelerator, virtio, UEFI, QMP and USB-share setup
    as `start.ps1` targets, plus the knobs below on top.
.PARAMETER Iso
    Path to the ISO file to boot.
.PARAMETER DiskSize
    Virtual disk size: 5, 5G, 5GB, 10gb, 15G, 40G ... (default 10G).
.PARAMETER Name
    VM name; the disk is data/adhoc/<Name>.qcow2. Default: ISO base name + size.
.PARAMETER Disk
    Explicit qcow2 path instead of data/adhoc/<Name>.qcow2.
.PARAMETER Fresh
    Delete the existing disk (and its NVRAM) first so the installer runs again.
.PARAMETER BootFrom
    auto (default: CD on a blank disk, disk afterwards), cd, or disk.
.PARAMETER Snapshot
    QEMU -snapshot: every write goes to a temp overlay and is discarded at exit.
    Try a live ISO or an installed disk without changing anything.
.PARAMETER Firmware
    bios (default) or uefi (OVMF, per-VM NVRAM next to the disk).
.PARAMETER OsFamily
    linux (default: virtio disk, std VGA), windows or macos (ide disk).
.PARAMETER Machine
    QEMU machine type: pc (default) or q35 (AHCI/PCIe; slower to boot on WHPX).
.PARAMETER Cpu
    QEMU -cpu model (host, max, Haswell-v4, EPYC-v4 ...). Default: host profile.
.PARAMETER Cores
    Guest CPU cores. Default: host cores minus 2 (min 2).
.PARAMETER Memory
    Guest RAM, e.g. 4G. Default: half of host RAM, clamped 4-8 GB.
.PARAMETER Accel
    auto (default: whpx on Windows, kvm on Linux, tcg fallback), whpx, kvm, tcg.
.PARAMETER Vga
    std (default), virtio (best for Linux desktops), qxl, vmware, cirrus, none.
.PARAMETER Display
    gtk (default), sdl, none. Ignored with -Vnc.
.PARAMETER Audio
    Add a sound card (intel-hda + hda-duplex). Backend: dsound on Windows, pa on Linux.
.PARAMETER PortForward
    Host->guest TCP forwards on the user-mode NIC: "2222:22", "2222:22,8080:80".
.PARAMETER NoNet
    No network card at all.
.PARAMETER Share
    Host folder exposed to the guest read-only as a USB stick labelled SHARE.
.PARAMETER LocalTime
    Guest RTC in host local time (Windows guests expect it).
.PARAMETER ExtraArgs
    Raw QEMU arguments appended verbatim, e.g. -ExtraArgs "-device","virtio-rng-pci".
.PARAMETER Sha512
    Expected SHA512: hex string, a .sha512/.sha512sum/SHA512SUMS file, or a URL to one.
.PARAMETER Sha256
    Expected SHA256: hex string, a .sha256/.sha256sum/SHA256SUMS file, or a URL to one.
.PARAMETER Sig
    Detached GPG signature (.sig/.asc file or URL) of the ISO, or of the checksum
    file when its name says so (SHA256SUMS.gpg, SHA256SUMS.sign).
.PARAMETER GpgKey
    Signing key id/fingerprint to import before verifying. Without it a missing
    key is fetched from the keyserver by the id the signature names.
.PARAMETER GpgServer
    Keyserver for key import (default hkps://keys.openpgp.org).
.PARAMETER NoVerify
    Do not auto-detect checksum/signature files next to the ISO.
.PARAMETER Force
    Boot even if a requested verification failed.
.PARAMETER Background
    Detach the VM and return to the shell (windowed QEMU, no console left behind).
.PARAMETER Vnc
    Serve the display over VNC (127.0.0.1) instead of a window.
.PARAMETER DryRun
    Print the QEMU command line (after verification) and exit without launching.
.EXAMPLE
    .\boot-iso.ps1 .\data\endeavouros-Titan-Nova-2026.08.15.iso 10G
    .\boot-iso.ps1 C:\isos\EndeavourOS_Titan-Nova-2026.08.15.iso 15gb -Sig C:\isos\EndeavourOS_Titan-Nova-2026.08.15.iso.sig -Sha512 C:\isos\EndeavourOS_Titan-Nova-2026.08.15.iso.sha512sum
    .\boot-iso.ps1 C:\isos\ubuntu-24.04.3-desktop-amd64.iso 15G -Sha256 C:\isos\SHA256SUMS -Sig C:\isos\SHA256SUMS.gpg
    .\boot-iso.ps1 C:\isos\kali-linux-2026.3-installer-amd64.iso 20G -Vga virtio -Audio -PortForward 2222:22
    .\boot-iso.ps1 .\data\endeavouros-Titan-Nova-2026.08.15.iso 5G -Snapshot -Fresh -Firmware uefi
#>
param (
    [Parameter(Position = 0, Mandatory = $true)]
    [string]$Iso,

    [Parameter(Position = 1)]
    [string]$DiskSize = "10G",

    # -- disk --
    [string]$Name,
    [string]$Disk,
    [switch]$Fresh,
    [ValidateSet("auto", "cd", "disk")]
    [string]$BootFrom = "auto",
    [switch]$Snapshot,

    # -- machine --
    [ValidateSet("bios", "uefi")]
    [string]$Firmware = "bios",
    [ValidateSet("linux", "windows", "macos")]
    [string]$OsFamily = "linux",
    [ValidateSet("pc", "q35")]
    [string]$Machine = "pc",
    [string]$Cpu,
    [int]$Cores = 0,
    [string]$Memory,
    [ValidateSet("auto", "whpx", "kvm", "tcg")]
    [string]$Accel = "auto",

    # -- devices --
    [ValidateSet("std", "virtio", "qxl", "vmware", "cirrus", "none")]
    [string]$Vga = "std",
    [ValidateSet("gtk", "sdl", "none")]
    [string]$Display = "gtk",
    [switch]$Audio,
    [string]$PortForward,
    [switch]$NoNet,
    [string]$Share,
    [switch]$LocalTime,
    [string[]]$ExtraArgs,

    # -- verification (all optional) --
    [string]$Sha512,
    [string]$Sha256,
    [string]$Sig,
    [string]$GpgKey,
    [string]$GpgServer = "hkps://keys.openpgp.org",
    [switch]$NoVerify,
    [switch]$Force,

    # -- run --
    [switch]$Background,
    [switch]$Vnc,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

# ------------------------------------------------------------------------------
# Modules: the same QEMU builder, hardware profiler and hashing the pipeline uses.
# ------------------------------------------------------------------------------
$ModuleRoot = Join-Path $PSScriptRoot "modules"
. (Join-Path $ModuleRoot "hardware.ps1")
. (Join-Path $ModuleRoot "network.ps1")
. (Join-Path $ModuleRoot "verify.ps1")
. (Join-Path $ModuleRoot "launch.ps1")

$IsLinuxOS = ($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true)

# ------------------------------------------------------------------------------
# Inputs
# ------------------------------------------------------------------------------
if (-not (Test-Path $Iso)) {
    Write-Host "  [FAIL] ISO not found: $Iso" -ForegroundColor Red
    Exit 1
}
$IsoPath = (Get-Item $Iso).FullName
$IsoDir = Split-Path $IsoPath -Parent
$IsoFile = Split-Path $IsoPath -Leaf
$IsoBase = [System.IO.Path]::GetFileNameWithoutExtension($IsoPath)

function ConvertTo-QemuSize {
    param ([string]$Text, [string]$What)
    if ($Text -notmatch '^\s*(\d+)\s*([kKmMgGtT])?[bB]?\s*$') {
        Write-Host "  [FAIL] $What not understood: '$Text' (use 5G, 10GB, 15gb, 512M ...)" -ForegroundColor Red
        Exit 1
    }
    $Unit = if ($Matches[2]) { $Matches[2].ToUpper() } else { "G" }
    return "$([int64]$Matches[1])$Unit"
}
$DiskSize = ConvertTo-QemuSize $DiskSize "Disk size"
if ($DiskSize -match '^(\d+)G$' -and [int]$Matches[1] -lt 4) {
    Write-Host "  [?] $DiskSize is small for a desktop Linux install (most want 15 GB or more with a desktop)." -ForegroundColor Yellow
}

$Slug = ($IsoBase.ToLower() -replace '[^a-z0-9._-]+', '-').Trim('-')
if ([string]::IsNullOrWhiteSpace($Name)) { $Name = "$Slug-$DiskSize" }
$Name = ($Name -replace '[^A-Za-z0-9._-]+', '-').Trim('-')

$DataDir = Join-Path $PSScriptRoot "data"
$DiskPath = if ($Disk) { $Disk } else { Join-Path (Join-Path $DataDir "adhoc") "$Name.qcow2" }
$DiskDir = Split-Path $DiskPath -Parent
if (-not (Test-Path $DiskDir)) { New-Item -ItemType Directory -Path $DiskDir -Force | Out-Null }
$VarsPath = [System.IO.Path]::ChangeExtension($DiskPath, ".vars.fd")

if ($Share -and -not (Test-Path $Share -PathType Container)) {
    Write-Host "  [FAIL] -Share folder not found: $Share" -ForegroundColor Red
    Exit 1
}
$Forwards = @()
if ($PortForward) {
    foreach ($Entry in ($PortForward -split '[,\s]+' | Where-Object { $_ })) {
        if ($Entry -notmatch '^(\d+):(\d+)$') {
            Write-Host "  [FAIL] -PortForward entry not understood: '$Entry' (use host:guest, e.g. 2222:22)" -ForegroundColor Red
            Exit 1
        }
        $Forwards += "hostfwd=tcp:127.0.0.1:$($Matches[1])-:$($Matches[2])"
    }
}

Write-Host ""
Write-Host "  Castle VM -- boot ISO" -ForegroundColor Cyan
Write-Host "  ================================================================" -ForegroundColor DarkGray
Write-Host "  * ISO:      $IsoPath" -ForegroundColor White
Write-Host "  * Disk:     $DiskPath ($DiskSize)$(if ($Snapshot) { '  [snapshot: writes discarded]' })" -ForegroundColor White
Write-Host "  * Machine:  $Machine, $Firmware, $OsFamily guest, vga $Vga" -ForegroundColor White

if (-not (Test-QemuInstalled)) { Exit 1 }

# ==============================================================================
# VERIFICATION (optional)
# ==============================================================================
# Explicit -Sha512/-Sha256/-Sig win; otherwise look next to the ISO for the
# names distros ship: <iso>.sig/.asc, <iso>.sha512sum/.sha512, <iso>.sha256sum/
# .sha256, SHA512SUMS/SHA256SUMS(+.gpg/.sign), sha512sums.txt/sha256sums.txt.
if (-not $NoVerify) {
    if (-not $Sha512) {
        $Sha512 = @("$IsoFile.sha512sum", "$IsoFile.sha512", "SHA512SUMS", "sha512sums.txt", "sha512sum.txt") |
            ForEach-Object { Join-Path $IsoDir $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
        if ($Sha512) { Write-Host "  [i] Found next to ISO: $(Split-Path $Sha512 -Leaf)" -ForegroundColor DarkGray }
    }
    if (-not $Sha256) {
        $Sha256 = @("$IsoFile.sha256sum", "$IsoFile.sha256", "SHA256SUMS", "sha256sums.txt", "sha256sum.txt") |
            ForEach-Object { Join-Path $IsoDir $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
        if ($Sha256) { Write-Host "  [i] Found next to ISO: $(Split-Path $Sha256 -Leaf)" -ForegroundColor DarkGray }
    }
    if (-not $Sig) {
        $Candidates = @("$IsoFile.sig", "$IsoFile.asc")
        foreach ($Sums in @($Sha512, $Sha256)) {
            if ($Sums -and (Test-Path $Sums)) { $Candidates += @("$Sums.gpg", "$Sums.sign", "$Sums.sig", "$Sums.asc") }
        }
        $Sig = $Candidates | ForEach-Object { if ([System.IO.Path]::IsPathRooted($_)) { $_ } else { Join-Path $IsoDir $_ } } |
            Where-Object { Test-Path $_ } | Select-Object -First 1
        if ($Sig) { Write-Host "  [i] Found next to ISO: $(Split-Path $Sig -Leaf)" -ForegroundColor DarkGray }
    }
}

$Http = $null
function Get-VerifyText {
    # A hex digest, a local file's content, or a URL's content.
    param ([string]$Source)
    if ($Source -match '^[a-fA-F0-9]{32,128}$') { return $Source }
    if (Test-Path $Source -PathType Leaf) { return [System.IO.File]::ReadAllText((Get-Item $Source).FullName) }
    if ($Source -match '^https?://') {
        if (-not $Script:Http) { Add-Type -AssemblyName System.Net.Http; $Script:Http = [System.Net.Http.HttpClient]::new() }
        Write-Host "     [>] Fetching $Source" -ForegroundColor DarkGray
        Write-EndpointInfo -Url $Source -Indent "     "
        return $Script:Http.GetStringAsync($Source).GetAwaiter().GetResult()
    }
    throw "not a hex digest, file or URL: $Source"
}
function Get-VerifyFile {
    # Local path for a file given as a path or URL (downloaded to temp).
    param ([string]$Source)
    if (Test-Path $Source -PathType Leaf) { return (Get-Item $Source).FullName }
    if ($Source -match '^https?://') {
        if (-not $Script:Http) { Add-Type -AssemblyName System.Net.Http; $Script:Http = [System.Net.Http.HttpClient]::new() }
        $Tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("castle-" + [System.IO.Path]::GetFileName(([System.Uri]$Source).LocalPath))
        Write-Host "     [>] Fetching $Source" -ForegroundColor DarkGray
        Write-EndpointInfo -Url $Source -Indent "     "
        [System.IO.File]::WriteAllBytes($Tmp, $Script:Http.GetByteArrayAsync($Source).GetAwaiter().GetResult())
        return $Tmp
    }
    throw "file not found: $Source"
}
function Find-ExpectedHash {
    # Pull the digest for our ISO out of any checksum layout: bare hex,
    # "hash  filename", "hash *filename", "ALGO(filename) = hash", or a
    # multi-file manifest where the filename sits near the hash.
    param ([string]$Text, [int]$HexLen, [string]$FileName)
    $T = $Text.Trim()
    if ($T -match "^([a-fA-F0-9]{$HexLen})$") { return $Matches[1] }
    $Esc = [regex]::Escape($FileName)
    if ($T -match "(?mi)^([a-fA-F0-9]{$HexLen})\s+\*?\.?/?${Esc}\s*$") { return $Matches[1] }
    if ($T -match "(?mi)^\w+\s*\(${Esc}\)\s*=\s*([a-fA-F0-9]{$HexLen})\s*$") { return $Matches[1] }
    if ($T -match "(?is)([a-fA-F0-9]{$HexLen}).{1,250}?${Esc}") { return $Matches[1] }
    if ($T -match "(?is)${Esc}.{1,250}?([a-fA-F0-9]{$HexLen})") { return $Matches[1] }
    # Single-entry checksum file that names the ISO differently (renamed download).
    $All = @([regex]::Matches($T, "(?m)^([a-fA-F0-9]{$HexLen})\b") | ForEach-Object { $_.Groups[1].Value })
    if ($All.Count -eq 1) { return $All[0] }
    return $null
}
function Test-HashSource {
    param ([string]$Source, [string]$Algorithm, [int]$HexLen)
    Write-Host "  [HASH] $Algorithm from $Source" -ForegroundColor Yellow
    try { $Text = Get-VerifyText $Source } catch { Write-Host "  [FAIL] $($_.Exception.Message)" -ForegroundColor Red; return $false }
    $Expected = Find-ExpectedHash -Text $Text -HexLen $HexLen -FileName $IsoFile
    if (-not $Expected) {
        Write-Host "  [FAIL] No $Algorithm digest for '$IsoFile' in that source." -ForegroundColor Red
        return $false
    }
    $Computed = Get-FileHashDotNet -Path $IsoPath -Algorithm $Algorithm
    if ($Computed -eq $Expected.ToLower()) {
        Write-Host "  [OK] $Algorithm verified ($Computed)" -ForegroundColor Green
        return $true
    }
    Write-Host "  [FAIL] $Algorithm MISMATCH" -ForegroundColor Red
    Write-Host "     Computed: $Computed" -ForegroundColor Red
    Write-Host "     Expected: $($Expected.ToLower())" -ForegroundColor Yellow
    return $false
}
function Test-SigSource {
    param ([string]$Source)
    if (-not (Test-GpgUsable)) {
        Write-Host "  [?] No working gpg on PATH -- signature check skipped (Git for Windows ships gpg.exe in Git\usr\bin)." -ForegroundColor Yellow
        return $null
    }
    try { $SigFile = Get-VerifyFile $Source } catch { Write-Host "  [FAIL] $($_.Exception.Message)" -ForegroundColor Red; return $false }

    # What is signed: the ISO, or a checksum file whose name the signature carries
    # (SHA256SUMS.gpg -> SHA256SUMS, as Ubuntu and Kali publish).
    $SigLeaf = Split-Path $SigFile -Leaf
    $Signed = $IsoPath
    $Stripped = $SigLeaf -replace '\.(gpg|sig|sign|asc)$', ''
    foreach ($Sums in @($Sha512, $Sha256)) {
        if ($Sums -and (Test-Path $Sums -PathType Leaf) -and ((Split-Path $Sums -Leaf) -eq $Stripped)) { $Signed = (Get-Item $Sums).FullName }
    }
    if ($Signed -ne $IsoPath -and $Stripped -ne $IsoFile) {
        Write-Host "  [GPG] Signature covers the checksum file $(Split-Path $Signed -Leaf); the ISO is tied to it by its digest." -ForegroundColor DarkGray
    }
    Write-Host "  [GPG] Verifying $SigLeaf against $(Split-Path $Signed -Leaf)..." -ForegroundColor Yellow

    if ($GpgKey) {
        Write-Host "     [>] Importing key $GpgKey from $GpgServer" -ForegroundColor DarkGray
        Write-EndpointInfo -Url $GpgServer -Indent "     "
        gpg --keyserver $GpgServer --recv-keys $GpgKey 2>$null | Out-Null
    }
    $Out = (gpg --status-fd 1 --verify $SigFile $Signed 2>&1) -join "`n"
    if ($Out -match 'NO_PUBKEY ([A-F0-9]+)' -and -not $GpgKey) {
        $Missing = $Matches[1]
        Write-Host "     [>] Signing key $Missing is not in your keyring; fetching it from $GpgServer" -ForegroundColor DarkGray
        Write-EndpointInfo -Url $GpgServer -Indent "     "
        gpg --keyserver $GpgServer --recv-keys $Missing 2>$null | Out-Null
        $Out = (gpg --status-fd 1 --verify $SigFile $Signed 2>&1) -join "`n"
    }
    if ($SigFile -like (Join-Path ([System.IO.Path]::GetTempPath()) "castle-*")) { Remove-Item $SigFile -Force -ErrorAction SilentlyContinue }

    if ($Out -match '(?m)^\[GNUPG:\] GOODSIG (\S+) (.*)$') {
        $KeyId = $Matches[1]; $Uid = $Matches[2]
        $Fpr = if ($Out -match '(?m)^\[GNUPG:\] VALIDSIG ([A-F0-9]+)') { $Matches[1] } else { $KeyId }
        Write-Host "  [OK] Good signature from $Uid" -ForegroundColor Green
        Write-Host "     Fingerprint: $Fpr -- compare it with the one the distro publishes." -ForegroundColor DarkGray
        return $true
    }
    Write-Host "  [FAIL] GPG signature rejected." -ForegroundColor Red
    ($Out -split "`n" | Where-Object { $_ -notmatch '^\[GNUPG:\]' } | Select-Object -First 6) | ForEach-Object { Write-Host "     $_" -ForegroundColor DarkGray }
    return $false
}

$Requested = @($Sha512, $Sha256, $Sig | Where-Object { $_ })
if ($Requested.Count -gt 0) {
    Write-Host ""
    Write-Host "  [*] Verification" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------" -ForegroundColor DarkGray
    $Results = @()
    if ($Sha512) { $Results += Test-HashSource -Source $Sha512 -Algorithm "SHA512" -HexLen 128 }
    if ($Sha256) { $Results += Test-HashSource -Source $Sha256 -Algorithm "SHA256" -HexLen 64 }
    if ($Sig)    { $Results += Test-SigSource -Source $Sig }
    if ($Script:Http) { $Script:Http.Dispose() }
    if ($Results -contains $false) {
        if ($Force) {
            Write-Host "  [?] Verification failed -- booting anyway (-Force)." -ForegroundColor Yellow
        }
        else {
            Write-Host "  [FAIL] Verification failed. Refusing to boot; pass -Force to boot anyway or -NoVerify to skip checks." -ForegroundColor Red
            Exit 1
        }
    }
    elseif ($Results -notcontains $true) {
        Write-Host "  [?] No verification could be performed." -ForegroundColor Yellow
    }
}
else {
    Write-Host "  [i] No checksum or signature given or found -- booting unverified (that is fine for your own ISOs)." -ForegroundColor DarkGray
}

# ==============================================================================
# DISK
# ==============================================================================
Write-Host ""
if ($Fresh -and (Test-Path $DiskPath)) {
    Write-Host "  [DISK] -Fresh: deleting $DiskPath" -ForegroundColor Yellow
    Remove-Item $DiskPath -Force
    if (Test-Path $VarsPath) { Remove-Item $VarsPath -Force }
}
if (Test-Path $DiskPath) {
    try {
        $Info = & $Script:QemuImg info --output=json $DiskPath 2>$null | ConvertFrom-Json
        $Have = [int64]$Info.'virtual-size'
        $Want = ConvertTo-Bytes $DiskSize
        if ($Want -gt 0 -and $Have -ne $Want) {
            Write-Host "  [?] Disk already exists at $("{0:N0}" -f ($Have / 1GB)) GB; the requested $DiskSize is ignored. Use -Fresh or -Name for a new disk." -ForegroundColor Yellow
        }
    }
    catch { }
}
$FirstBoot = New-VirtualDisk -DiskPath $DiskPath -DiskSize $DiskSize
if ($BootFrom -eq "cd") { $FirstBoot = $true } elseif ($BootFrom -eq "disk") { $FirstBoot = $false }

# ==============================================================================
# HARDWARE
# ==============================================================================
$Hardware = Get-HostHardwareProfile
if ($Memory) { $Hardware.Memory = ConvertTo-QemuSize $Memory "Memory" }
if ($Cores -gt 0) { $Hardware.CpuCores = $Cores }
if ($Cpu) { $Hardware.CpuProfile = $Cpu }

# Synthetic target: what Build-QemuArgs / Start-CastleVm need and nothing more.
$Target = @{
    Id       = $Name
    Name     = "$IsoBase (ad hoc ISO)"
    OsFamily = $OsFamily
    DiskSize = $DiskSize
}
if ($Firmware -eq "uefi") { $Target.Firmware = "uefi" }

# ==============================================================================
# QEMU ARGUMENTS: the module's baseline, then the knobs applied on top.
# ==============================================================================
function Set-QemuOption {
    # Replace the value after every occurrence of $Flag, or append flag+value.
    param ([System.Collections.Generic.List[string]]$List, [string]$Flag, [string]$Value)
    $Found = $false
    for ($i = 0; $i -lt $List.Count - 1; $i++) {
        if ($List[$i] -eq $Flag) { $List[$i + 1] = $Value; $Found = $true }
    }
    if (-not $Found) { $List.Add($Flag); $List.Add($Value) }
}
function Remove-QemuOption {
    param ([System.Collections.Generic.List[string]]$List, [string]$Flag, [string]$ValuePattern = ".*")
    for ($i = $List.Count - 2; $i -ge 0; $i--) {
        if ($List[$i] -eq $Flag -and $List[$i + 1] -match $ValuePattern) { $List.RemoveRange($i, 2) }
    }
}

function Get-AdhocQemuArgs {
    param ([bool]$UseVnc)
    $VncDisplay = if ($UseVnc) { Get-FreeVncDisplay } else { 0 }
    $Base = Build-QemuArgs -Target $Target -DiskPath $DiskPath -IsoPath $IsoPath -Hardware $Hardware -FirstBoot $FirstBoot -Vnc $UseVnc -VncDisplay $VncDisplay
    $A = [System.Collections.Generic.List[string]]::new()
    foreach ($x in $Base) { $A.Add([string]$x) }

    # Accelerator: the module emits "-accel <best> -accel tcg"; replace with the choice.
    if ($Accel -ne "auto") {
        Remove-QemuOption $A "-accel"
        $Pair = @("-accel", $Accel)
        if ($Accel -eq "whpx" -and $OsFamily -eq "linux") { $Pair[1] = "whpx,kernel-irqchip=off" }
        if ($Accel -ne "tcg") { $Pair += @("-accel", "tcg") }
        $A.InsertRange(0, [string[]]$Pair)
    }
    if ($Machine -eq "q35") { Remove-QemuOption $A "-machine"; $A.InsertRange(0, [string[]]@("-machine", "q35")) }
    Set-QemuOption $A "-vga" $Vga
    if (-not $UseVnc) { Set-QemuOption $A "-display" $Display }
    if ($Snapshot) { $A.Add("-snapshot") }
    if ($LocalTime) { $A.Add("-rtc"); $A.Add("base=localtime") }

    # Network: QEMU adds a default user-mode NIC when none is given; be explicit
    # so forwards and -NoNet work.
    if ($NoNet) {
        $A.Add("-nic"); $A.Add("none")
    }
    elseif ($Forwards.Count -gt 0) {
        $Model = if ($OsFamily -eq "linux") { "virtio-net-pci" } else { "e1000" }
        $A.Add("-nic"); $A.Add("user,model=$Model,$($Forwards -join ',')")
    }

    if ($Audio) {
        $Backend = if ($IsLinuxOS) { "pa" } else { "dsound" }
        $A.AddRange([string[]]@("-audiodev", "$Backend,id=snd0", "-device", "intel-hda", "-device", "hda-duplex,audiodev=snd0"))
    }
    if ($Share) {
        $ShareDir = (Get-Item $Share).FullName
        $A.AddRange([string[]]@("-blockdev", "driver=vvfat,node-name=share0,dir=$ShareDir,label=SHARE,rw=off,read-only=on", "-device", "usb-storage,drive=share0"))
    }
    if ($ExtraArgs) { $A.AddRange([string[]]$ExtraArgs) }
    return $A.ToArray()
}

# ==============================================================================
# LAUNCH
# ==============================================================================
$BootMode = if ($FirstBoot) { "CD-ROM (install)" } else { "Disk (existing)" }
Write-Host ""
Write-Host "  [BOOT] $($Target.Name)" -ForegroundColor Cyan
Write-Host "  -------------------------------------------" -ForegroundColor DarkGray
Write-Host "  * Boot:      $BootMode" -ForegroundColor White
Write-Host "  * CPU:       $($Hardware.CpuCores) cores -- $($Hardware.CpuProfile)" -ForegroundColor White
Write-Host "  * RAM:       $($Hardware.Memory)" -ForegroundColor White
if ($PortForward) { Write-Host "  * Forwards:  $PortForward (host 127.0.0.1 -> guest)" -ForegroundColor Green }
if ($Share) { Write-Host "  * Share:     $Share (read-only USB, label SHARE)" -ForegroundColor White }

$QemuArgs = Get-AdhocQemuArgs -UseVnc:$Vnc
if ($Vnc) {
    $VncArg = ($QemuArgs | Select-Object -Index (([array]::IndexOf($QemuArgs, "-vnc")) + 1))
    Write-Host "  * VNC:       $VncArg (port 5900 + display)" -ForegroundColor Green
}

if ($DryRun) {
    Write-Host ""
    Write-Host "  [DRY RUN] Would launch:" -ForegroundColor Cyan
    Write-Host "  $($Script:QemuSystem)" -ForegroundColor White
    $QemuArgs | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
    Exit 0
}

if ($Background) {
    $QuotedArgs = $QemuArgs | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }
    $QemuExe = if ($Script:QemuSystemWindowed -and (Test-Path $Script:QemuSystemWindowed)) { $Script:QemuSystemWindowed } else { $Script:QemuSystem }
    $Process = Start-Process -FilePath $QemuExe -ArgumentList $QuotedArgs -NoNewWindow -PassThru
    Write-Host "  [BOOT] Background VM started (PID: $($Process.Id))." -ForegroundColor Green
}
else {
    Write-Host "  [BOOT] Launching QEMU (close the window or power off the guest to return)..." -ForegroundColor Green
    & $Script:QemuSystem @QemuArgs
    Write-Host "  [OK] VM exited (code $LASTEXITCODE)." -ForegroundColor Green
}
