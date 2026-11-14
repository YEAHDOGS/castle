<#
.SYNOPSIS
    Castle -- Google's official Android Emulator (AVD), self-contained under data/.
.DESCRIPTION
    Installs a private Android SDK into data/android-sdk (nothing goes into
    %LOCALAPPDATA% or ~/.android), creates an AVD, boots it with hardware
    acceleration (WHPX on Windows, KVM on Linux) and waits until Android reports
    boot_completed. With -Copy every .apk in data/copy is installed and the new
    apps are launched.

    Downloads and how they are checked:
      * cmdline-tools: zip URL and SHA-1 read from Google's SDK manifest
        (dl.google.com/android/repository/repository2-3.xml, over HTTPS); the zip
        must match before it is unpacked, then its SHA-512 is pinned in
        data/.castle_trust.json like every other clonewars download.
      * platform-tools, emulator, system image: installed by sdkmanager, which
        checks each archive against the same signed-over-HTTPS manifests.

    The emulator has its own user-mode network: the guest gets internet through
    the host, and the host is 10.0.2.2 from inside Android. adb on the host
    reaches the guest as emulator-<port>.
.EXAMPLE
    .\android.ps1                       # install on first run, then boot to the home screen
    .\android.ps1 -Copy                 # same, then adb install every .apk in data/copy and launch it
    .\android.ps1 -Apk C:\x\app.apk     # install (and launch) one APK
    .\android.ps1 -Headless             # no window (CI / screenshots via adb)
    .\android.ps1 -Stop                 # clean shutdown of the Castle emulator
    .\android.ps1 -Wipe                 # factory-reset the AVD's data on this boot
    .\android.ps1 -Purge                # delete the AVD (the SDK stays)
    .\android.ps1 -Name castle-bot1 -Copy   # a second phone; with castle-tasks.sh in the payload
                                        # (written by the Castle app) its tasks run on the phone
#>
param (
    # The AVD. CASTLE_AVD_NAME gives each Castle automation bot a phone of its own.
    [string]$Name = $(if ($env:CASTLE_AVD_NAME) { $env:CASTLE_AVD_NAME } else { "castle" }),
    # SDK package id of the system image. Play Store images run ARM-only apps
    # through Google's ARM->x86 translation (API 30+) and can sign in to Play.
    [string]$Image = $(if ($env:CASTLE_AVD_IMAGE) { $env:CASTLE_AVD_IMAGE } else { "system-images;android-35;google_apis_playstore;x86_64" }),
    [string]$Device = "pixel_7",
    [int]$RamMB = $(if ($env:CASTLE_AVD_RAM_MB) { [int]$env:CASTLE_AVD_RAM_MB } else { 4096 }),
    [string]$DataSize = "16G",
    [Alias("c")]
    [switch]$Copy,
    [string[]]$Apk = @(),
    # Also adb push the non-APK files of data/copy to /sdcard/Download/castle-copy.
    [switch]$Push,
    [switch]$Headless,
    [switch]$Wipe,
    [switch]$ColdBoot,
    [switch]$Stop,
    [switch]$Purge,
    # Accept the Android SDK licenses without the interactive prompt (scripted runs).
    # CASTLE_ACCEPT_ANDROID_LICENSES=1 does the same.
    [switch]$AcceptLicenses,
    [switch]$InstallOnly,
    [int]$BootTimeoutSec = 600
)

$ErrorActionPreference = "Stop"
. (Join-Path (Join-Path $PSScriptRoot "modules") "quiet.ps1")
if ($Name -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,62}$') { Write-Host "  [FAIL] -Name must be letters, digits, dots, dashes and underscores." -ForegroundColor Red; Exit 1 }
# CASTLE_VM_SNAPSHOT=1 (a throwaway run from the Castle app): the phone starts factory-fresh.
if ($env:CASTLE_VM_SNAPSHOT -eq "1") { $Wipe = $true; $ColdBoot = $true }
. (Join-Path (Join-Path $PSScriptRoot "modules") "network.ps1")
. (Join-Path (Join-Path $PSScriptRoot "modules") "verify.ps1")

$IsWin = -not (($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true) -or ($IsMacOS -eq $true))
$HostOs = if ($IsWin) { "windows" } elseif ($IsMacOS) { "macosx" } else { "linux" }
$Exe = if ($IsWin) { ".exe" } else { "" }
$Bat = if ($IsWin) { ".bat" } else { "" }

$DataDir = Join-Path $PSScriptRoot "data"
# CASTLE_COPY_DIR gives one launch a payload folder of its own (the Castle app sets it).
$CopyDir = if ($env:CASTLE_COPY_DIR) { $env:CASTLE_COPY_DIR } else { Join-Path $DataDir "copy" }
$Sdk = Join-Path $DataDir "android-sdk"
$CmdlineBin = Join-Path (Join-Path (Join-Path $Sdk "cmdline-tools") "latest") "bin"
$SdkManager = Join-Path $CmdlineBin "sdkmanager$Bat"
$AvdManager = Join-Path $CmdlineBin "avdmanager$Bat"
$Adb = Join-Path (Join-Path $Sdk "platform-tools") "adb$Exe"
$Emulator = Join-Path (Join-Path $Sdk "emulator") "emulator$Exe"
$StateDir = Join-Path $Sdk "castle"
$PidFile = Join-Path $StateDir "$Name.pid"
$PortFile = Join-Path $StateDir "$Name.port"

# Keep every piece of Android state inside data/android-sdk: SDK, AVDs, adb
# keys, emulator prefs. The folder can be moved or deleted as a unit.
$env:ANDROID_HOME = $Sdk
$env:ANDROID_SDK_ROOT = $Sdk
$env:ANDROID_USER_HOME = Join-Path $Sdk ".android"
$env:ANDROID_AVD_HOME = Join-Path $env:ANDROID_USER_HOME "avd"
$env:ANDROID_EMULATOR_HOME = $env:ANDROID_USER_HOME
foreach ($d in @($Sdk, $StateDir, $env:ANDROID_USER_HOME, $env:ANDROID_AVD_HOME)) {
    if (-not (Test-Path $d)) { New-Item -ItemType Directory -Path $d -Force | Out-Null }
}

function Write-Step([string]$Text) { Write-Host ""; Write-Host "  [*] $Text" -ForegroundColor Cyan; Write-Host "  -------------------------------------------" -ForegroundColor DarkGray }

function Get-CastleEmulatorSerial {
    if (-not (Test-Path $PortFile)) { return $null }
    return "emulator-$((Get-Content $PortFile -Raw).Trim())"
}

function Get-CastleEmulatorProcess {
    if (-not (Test-Path $PidFile)) { return $null }
    $EmuPid = [int](Get-Content $PidFile -Raw).Trim()
    return Get-Process -Id $EmuPid -ErrorAction SilentlyContinue
}

function Invoke-Adb {
    param([string[]]$AdbArgs, [switch]$Quiet)
    $Serial = Get-CastleEmulatorSerial
    $Full = if ($Serial) { @("-s", $Serial) + $AdbArgs } else { $AdbArgs }
    # Windowless (modules/quiet.ps1): the Castle box runs this script with no console.
    $R = Invoke-Quiet $Adb $Full
    $Out = ($R.Out + $R.Err).Trim()
    if (-not $Quiet) { return @($Out -split "`r?`n") }
    return $Out
}

# ==============================================================================
# STOP / PURGE
# ==============================================================================
if ($Stop -or $Purge) {
    $Proc = Get-CastleEmulatorProcess
    if ($Proc -and (Test-Path $Adb)) {
        Write-Host "  [STOP] Asking the '$Name' emulator to shut down (adb emu kill)..." -ForegroundColor Yellow
        Invoke-Adb -AdbArgs @("emu", "kill") -Quiet | Out-Null
        if (-not $Proc.WaitForExit(30000)) {
            Write-Host "  [STOP] Still running after 30 s -- stopping PID $($Proc.Id) (ours, from $PidFile)." -ForegroundColor Yellow
            Stop-Process -Id $Proc.Id -Force
        }
        Write-Host "  [OK] Emulator stopped." -ForegroundColor Green
    }
    elseif ($Stop) {
        Write-Host "  [i] The '$Name' emulator is not running." -ForegroundColor DarkGray
    }
    Remove-Item $PidFile, $PortFile -Force -ErrorAction SilentlyContinue
    if ($Purge) {
        if (Test-Path $AvdManager) { Invoke-Quiet $AvdManager @("delete", "avd", "-n", $Name) | Out-Null }
        Remove-Item (Join-Path $env:ANDROID_AVD_HOME "$Name.avd"), (Join-Path $env:ANDROID_AVD_HOME "$Name.ini") -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "  [OK] AVD '$Name' deleted. The SDK in $Sdk is kept (delete the folder to remove it too)." -ForegroundColor Green
    }
    Exit 0
}

Write-Host ""
Write-Host "  Castle Android -- Google Android Emulator" -ForegroundColor Cyan
Write-Host "  AVD: $Name   Image: $Image" -ForegroundColor White
Write-Host "  SDK: $Sdk" -ForegroundColor DarkGray
Write-Host "  ================================================================" -ForegroundColor DarkGray

$Existing = Get-CastleEmulatorProcess
if ($Existing) {
    Write-Host "  [i] The '$Name' emulator is already running (PID $($Existing.Id), $(Get-CastleEmulatorSerial))." -ForegroundColor Green
}

# ==============================================================================
# PHASE 1 -- Java (sdkmanager/avdmanager are Java tools; they need 17+)
# ==============================================================================
if (-not $Existing) {
    $JavaOk = $false
    try {
        $J = Invoke-Quiet java @("-version")
        $JavaVer = "$($J.Out)$($J.Err)"
        if ($JavaVer -match 'version "(\d+)') { $JavaOk = ([int]$Matches[1] -ge 17) }
    }
    catch { }
    if (-not $JavaOk -and -not (Test-Path $Emulator)) {
        Write-Host "  [FAIL] Java 17+ is needed once, to run Google's sdkmanager. Install one and re-run:" -ForegroundColor Red
        Write-Host "     winget install Microsoft.OpenJDK.21" -ForegroundColor White
        Exit 1
    }
}

# ==============================================================================
# PHASE 2 -- cmdline-tools (verified against Google's manifest)
# ==============================================================================
if (-not $Existing -and -not (Test-Path $SdkManager)) {
    Write-Step "Android SDK command-line tools"
    Add-Type -AssemblyName System.Net.Http
    $Http = [System.Net.Http.HttpClient]::new()
    try {
        $ManifestUrl = "https://dl.google.com/android/repository/repository2-3.xml"
        Write-Host "  [>] Reading Google's SDK manifest: $ManifestUrl" -ForegroundColor DarkGray
        [xml]$Manifest = $Http.GetStringAsync($ManifestUrl).GetAwaiter().GetResult()
        $Pkg = $Manifest.SelectNodes("//remotePackage[@path='cmdline-tools;latest']") |
            Where-Object { $_.channelRef.ref -eq "channel-0" } | Select-Object -First 1
        $Archive = @($Pkg.archives.archive) | Where-Object { $_.'host-os' -eq $HostOs } | Select-Object -First 1
        if (-not $Archive) { throw "no cmdline-tools archive for host-os '$HostOs' in the manifest" }
        $ZipName = $Archive.complete.url
        $ZipSha1 = $Archive.complete.checksum.'#text'
        $ZipPath = Join-Path $Sdk $ZipName
        Write-Host "  [i] cmdline-tools $($Pkg.revision.major).$($Pkg.revision.minor): $ZipName (sha1 $ZipSha1)" -ForegroundColor White

        if (-not (Test-Path $ZipPath)) {
            $Ok = Start-NetworkStream -Url "https://dl.google.com/android/repository/$ZipName" -Path $ZipPath -HttpClient $Http
            if (-not $Ok) { throw "download failed" }
        }
        $Actual = Get-FileHashDotNet -Path $ZipPath -Algorithm "SHA1"
        if ($Actual -ne $ZipSha1.ToLower()) {
            Remove-Item $ZipPath -Force
            throw "SHA-1 mismatch: manifest $ZipSha1, file $Actual (deleted)"
        }
        Write-Host "  [OK] SHA-1 matches Google's manifest." -ForegroundColor Green
        $Trust = Get-PinnedTrustStore
        $Trust[$ZipName] = (Get-FileHashDotNet -Path $ZipPath -Algorithm "SHA512")
        Save-PinnedTrustStore -TrustStore $Trust

        # The zip holds cmdline-tools/...; sdkmanager insists on living in cmdline-tools/latest.
        $Unpack = Join-Path $Sdk "cmdline-tools-unpack"
        if (Test-Path $Unpack) { Remove-Item $Unpack -Recurse -Force }
        Expand-Archive -Path $ZipPath -DestinationPath $Unpack -Force
        $Dest = Join-Path (Join-Path $Sdk "cmdline-tools") "latest"
        New-Item -ItemType Directory -Path (Split-Path $Dest -Parent) -Force | Out-Null
        if (Test-Path $Dest) { Remove-Item $Dest -Recurse -Force }
        Move-Item (Join-Path $Unpack "cmdline-tools") $Dest
        Remove-Item $Unpack -Recurse -Force
        Remove-Item $ZipPath -Force
        Write-Host "  [OK] Installed to $Dest" -ForegroundColor Green
    }
    catch {
        Write-Host "  [FAIL] cmdline-tools: $($_.Exception.Message)" -ForegroundColor Red
        Exit 1
    }
    finally { $Http.Dispose() }
}

# ==============================================================================
# PHASE 3 -- SDK packages: platform-tools (adb), emulator, system image
# ==============================================================================
$ImageDir = Join-Path $Sdk (($Image -split ';') -join [System.IO.Path]::DirectorySeparatorChar)
$Needed = @()
if (-not (Test-Path $Adb)) { $Needed += "platform-tools" }
if (-not (Test-Path $Emulator)) { $Needed += "emulator" }
if (-not (Test-Path (Join-Path $ImageDir "system.img"))) { $Needed += $Image }

if (-not $Existing -and $Needed.Count -gt 0) {
    Write-Step "SDK packages: $($Needed -join ', ')"
    # Since cmdline-tools 23 sdkmanager is a shim that downloads and runs
    # Google's "Android CLI" (cmdline-tools/latest/bin/android). That CLI
    # announces usage metrics, so it is called directly with --no-metrics.
    # Package ids use '/' there instead of ';' (the ';' form also gets split
    # by cmd.exe when passed to sdkmanager.bat unquoted).
    $AndroidCli = Join-Path $CmdlineBin "android$Exe"
    $Accept = $AcceptLicenses -or $env:CASTLE_ACCEPT_ANDROID_LICENSES -eq "1"
    if (-not $Accept -and -not (Test-Path (Join-Path (Join-Path $Sdk "licenses") "android-sdk-license"))) {
        Write-Host "  [i] Installing these means accepting the Android SDK License Agreement:" -ForegroundColor Yellow
        Write-Host "     https://developer.android.com/studio/terms" -ForegroundColor White
        $Answer = Read-Host "  Accept and continue? [y/N]"
        if ($Answer -notmatch '^y(es)?$') { Write-Host "  [FAIL] License not accepted." -ForegroundColor Red; Exit 1 }
    }
    elseif ($Accept) {
        Write-Host "  [i] Android SDK License Agreement accepted via -AcceptLicenses (https://developer.android.com/studio/terms)." -ForegroundColor Yellow
    }
    $CliIds = @($Needed | ForEach-Object { $_ -replace ';', '/' })
    Write-Host "  [>] android sdk install $($CliIds -join ' ')   (system images are ~1.5-2 GB)" -ForegroundColor DarkGray
    if (Test-Path $AndroidCli) {
        & $AndroidCli --no-metrics "--sdk=$Sdk" sdk install @CliIds
    }
    else {
        # Older cmdline-tools without the Android CLI: classic sdkmanager.
        ("y`n" * 30) | & $SdkManager "--sdk_root=$Sdk" --licenses 2>&1 | Out-Null
        & $SdkManager "--sdk_root=$Sdk" --install @($Needed | ForEach-Object { "`"$_`"" })
    }
    if ($LASTEXITCODE -ne 0) { Write-Host "  [FAIL] SDK install exited with $LASTEXITCODE." -ForegroundColor Red; Exit 1 }
    foreach ($Check in @($Adb, $Emulator, (Join-Path $ImageDir "system.img"))) {
        if (-not (Test-Path $Check)) { Write-Host "  [FAIL] Missing after install: $Check" -ForegroundColor Red; Exit 1 }
    }
    Write-Host "  [OK] SDK packages installed." -ForegroundColor Green
}

# ==============================================================================
# PHASE 4 -- AVD
# ==============================================================================
$AvdDir = Join-Path $env:ANDROID_AVD_HOME "$Name.avd"
$AvdConfig = Join-Path $AvdDir "config.ini"
if (-not $Existing -and -not (Test-Path $AvdConfig)) {
    Write-Step "Creating AVD '$Name' ($Device)"
    # "no" answers avdmanager's "custom hardware profile?" question.
    # avdmanager.bat's own Java check turns 'version "25"' (no minor part)
    # into 25 and compares it with 170, so JDK 25 fails it. Phase 1 already
    # checked for 17+.
    $env:SKIP_JDK_VERSION_CHECK = "1"
    # The package id is quoted by hand: cmd.exe (avdmanager.bat) splits on ';'.
    $Made = Invoke-Quiet $AvdManager @("create", "avd", "-n", $Name, "-k", "`"$Image`"", "-d", $Device) -InputText "no`n"
    ($Made.Out + $Made.Err) -split "`r?`n" | ForEach-Object { if ($_ -and $_ -notmatch '^\[=*\s*\]') { "     $_" } }
    if (-not (Test-Path $AvdConfig)) { Write-Host "  [FAIL] avdmanager did not create $AvdConfig." -ForegroundColor Red; Exit 1 }
}
if (Test-Path $AvdConfig) {
    # Castle defaults on top of the device profile. Re-applied every boot so
    # -RamMB / CASTLE_AVD_RAM_MB changes take effect.
    $Want = [ordered]@{
        "hw.ramSize"               = "$RamMB"
        "disk.dataPartition.size"  = $DataSize
        "hw.keyboard"              = "yes"       # host keyboard types into Android
        "hw.gpu.enabled"           = "yes"
        "hw.gpu.mode"              = "auto"      # host GPU when it works, SwiftShader otherwise
        "hw.audioInput"            = "yes"
        "hw.audioOutput"           = "yes"
        "showDeviceFrame"          = "no"
        "fastboot.forceColdBoot"   = "no"        # Quick Boot: later boots resume a snapshot in seconds
    }
    # avdmanager 23 writes disk.dataPartition.path=<temp>, which puts the
    # userdata image in a temp folder; dropping it keeps the data (installed
    # apps, sign-ins) in the AVD folder so it survives reboots.
    $Lines = @(Get-Content $AvdConfig | Where-Object { $_ -notmatch '^\s*disk\.dataPartition\.path\s*=\s*<temp>' })
    foreach ($Key in $Want.Keys) {
        $Found = $false
        for ($i = 0; $i -lt $Lines.Count; $i++) {
            if ($Lines[$i] -match "^\s*$([regex]::Escape($Key))\s*=") { $Lines[$i] = "$Key=$($Want[$Key])"; $Found = $true }
        }
        if (-not $Found) { $Lines += "$Key=$($Want[$Key])" }
    }
    Set-Content -Path $AvdConfig -Value $Lines
}

if ($InstallOnly) {
    Write-Host ""
    Write-Host "  [OK] SDK and AVD ready. Boot with: .\android.ps1" -ForegroundColor Green
    Exit 0
}

# ==============================================================================
# PHASE 5 -- Boot
# ==============================================================================
if (-not $Existing) {
    Write-Step "Booting the emulator"
    $AccelRun = Invoke-Quiet $Emulator @("-accel-check")
    $Accel = "$($AccelRun.Out)$($AccelRun.Err)".Trim()
    # Output is "accel:", a status code, the human line, "accel".
    $AccelLine = ($Accel -split "`r?`n" | Where-Object { $_ -match 'usable|install|not|error|fail' } | Select-Object -First 1)
    Write-Host "  [i] Acceleration: $AccelLine" -ForegroundColor White
    if ($Accel -notmatch 'is installed and usable|is usable') {
        Write-Host "  [?] No usable hypervisor reported -- the emulator will be very slow." -ForegroundColor Yellow
        Write-Host "     Windows: enable 'Windows Hypervisor Platform' (optionalfeatures.exe) and reboot." -ForegroundColor DarkGray
    }

    # Even console ports 5554..5584; the adb port is console+1.
    $Port = 5554
    while ($Port -le 5584) {
        $Busy = $false
        foreach ($p in @($Port, ($Port + 1))) {
            try { $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $p); $l.Start(); $l.Stop() } catch { $Busy = $true }
        }
        if (-not $Busy) { break }
        $Port += 2
    }
    Set-Content -Path $PortFile -Value $Port

    $EmuArgs = @("-avd", $Name, "-port", "$Port", "-netdelay", "none", "-netspeed", "full", "-no-boot-anim")
    if ($Headless) { $EmuArgs += "-no-window" }
    if ($Wipe) { $EmuArgs += "-wipe-data" }
    if ($ColdBoot) { $EmuArgs += "-no-snapshot-load" }

    # The emulator is a console program that opens its own window. Started with
    # CreateNoWindow its console stays invisible (no black box on the desktop),
    # and it keeps running after this script exits.
    $Psi = [System.Diagnostics.ProcessStartInfo]::new()
    $Psi.FileName = $Emulator
    foreach ($a in $EmuArgs) { [void]$Psi.ArgumentList.Add($a) }
    $Psi.UseShellExecute = $false
    $Psi.CreateNoWindow = $true
    $Psi.WorkingDirectory = Split-Path $Emulator -Parent
    $Proc = [System.Diagnostics.Process]::Start($Psi)
    Set-Content -Path $PidFile -Value $Proc.Id
    Write-Host "  [BOOT] Emulator started (PID $($Proc.Id), console port $Port, adb serial emulator-$Port)." -ForegroundColor Green
}

$Serial = Get-CastleEmulatorSerial
Invoke-Quiet $Adb @("start-server") | Out-Null
Write-Host "  [*] Waiting for Android to finish booting (up to $BootTimeoutSec s)..." -ForegroundColor DarkGray
$Sw = [System.Diagnostics.Stopwatch]::StartNew()
$Booted = $false
$LastState = ""
while ($Sw.Elapsed.TotalSeconds -lt $BootTimeoutSec) {
    $EmuProc = Get-CastleEmulatorProcess
    if (-not $EmuProc) {
        Write-Host "  [FAIL] The emulator process exited during boot." -ForegroundColor Red
        Exit 1
    }
    $State = Invoke-Adb -AdbArgs @("get-state") -Quiet
    if ($State -eq "device") {
        $Done = Invoke-Adb -AdbArgs @("shell", "getprop", "sys.boot_completed") -Quiet
        if ($Done -eq "1") { $Booted = $true; break }
        $State = "booting"
    }
    elseif ($State -match 'offline') { $State = "kernel up, adb connecting" }
    else { $State = "starting" }
    if ($State -ne $LastState) { Write-Host ("     {0,4:N0}s  {1}" -f $Sw.Elapsed.TotalSeconds, $State) -ForegroundColor DarkGray; $LastState = $State }
    Start-Sleep -Seconds 3
}
if (-not $Booted) {
    Write-Host "  [FAIL] Android did not report boot_completed within $BootTimeoutSec s." -ForegroundColor Red
    Exit 1
}
Write-Host ("  [OK] Android booted in {0:N0} s -- home screen is up ({1})." -f $Sw.Elapsed.TotalSeconds, $Serial) -ForegroundColor Green
# Unlock the keyguard (fresh AVDs have no PIN, so this lands on the launcher).
Invoke-Adb -AdbArgs @("shell", "input", "keyevent", "KEYCODE_WAKEUP") -Quiet | Out-Null
Invoke-Adb -AdbArgs @("shell", "wm", "dismiss-keyguard") -Quiet | Out-Null

# ==============================================================================
# PHASE 6 -- Payload: APKs (and optionally other files) from data/copy
# ==============================================================================
$ApkFiles = @()
if ($Copy -and (Test-Path $CopyDir)) { $ApkFiles += @(Get-ChildItem $CopyDir -Recurse -File -Filter *.apk | ForEach-Object FullName) }
$ApkFiles += @($Apk | Where-Object { $_ -and (Test-Path $_) } | ForEach-Object { (Resolve-Path $_).Path })
if ($ApkFiles.Count -gt 0) {
    Write-Step "Installing $($ApkFiles.Count) APK(s)"
    $Before = @((Invoke-Adb -AdbArgs @("shell", "pm", "list", "packages", "-3") -Quiet) -split "`r?`n" | ForEach-Object { $_ -replace '^package:', '' })
    foreach ($f in $ApkFiles) {
        Write-Host "  [>] adb install $([System.IO.Path]::GetFileName($f))" -ForegroundColor White
        $Out = Invoke-Adb -AdbArgs @("install", "-r", "-g", $f) -Quiet
        $Color = if ($Out -match 'Success') { "Green" } else { "Red" }
        Write-Host "     $(($Out -split "`r?`n" | Select-Object -Last 1))" -ForegroundColor $Color
    }
    $After = @((Invoke-Adb -AdbArgs @("shell", "pm", "list", "packages", "-3") -Quiet) -split "`r?`n" | ForEach-Object { $_ -replace '^package:', '' })
    $New = @($After | Where-Object { $_ -and $_ -notin $Before })
    foreach ($Pkg in $New) {
        Write-Host "  [>] Launching $Pkg" -ForegroundColor White
        Invoke-Adb -AdbArgs @("shell", "monkey", "-p", $Pkg, "-c", "android.intent.category.LAUNCHER", "1") -Quiet | Out-Null
    }
}
elseif ($Copy) {
    Write-Host "  [i] -Copy: no .apk files in data/copy." -ForegroundColor DarkGray
}
if ($Push -and (Test-Path $CopyDir)) {
    $Others = @(Get-ChildItem $CopyDir -Recurse -File | Where-Object { $_.Extension -ine ".apk" -and $_.Name -ine "README.md" -and $_.Name -ine "castle-tasks.sh" })
    if ($Others.Count -gt 0) {
        Write-Step "Pushing $($Others.Count) file(s) to /sdcard/Download/castle-copy"
        Invoke-Adb -AdbArgs @("shell", "mkdir", "-p", "/sdcard/Download/castle-copy") -Quiet | Out-Null
        foreach ($f in $Others) {
            Write-Host ("  [>] {0} ({1:N0} MB)" -f $f.Name, ($f.Length / 1MB)) -ForegroundColor White
            Invoke-Adb -AdbArgs @("push", $f.FullName, "/sdcard/Download/castle-copy/") -Quiet | Out-Null
        }
    }
}

# ==============================================================================
# PHASE 7 -- An Android automation's tasks, on the phone
# ==============================================================================
# castle-tasks.sh (written by the Castle app: the fields as variables, then the
# tasks) is pushed and started detached, so it outlives this script. Its log is
# /data/local/tmp/castle-automation.log on the phone.
$TasksFile = Join-Path $CopyDir "castle-tasks.sh"
if ($Copy -and (Test-Path $TasksFile)) {
    Write-Step "Starting the automation's tasks on the phone"
    $Lf = Join-Path $StateDir "$Name-tasks.sh"
    [System.IO.File]::WriteAllText($Lf, ([System.IO.File]::ReadAllText($TasksFile) -replace "`r", ""), [System.Text.UTF8Encoding]::new($false))
    Invoke-Adb -AdbArgs @("push", $Lf, "/data/local/tmp/castle-tasks.sh") -Quiet | Out-Null
    Invoke-Adb -AdbArgs @("shell", "chmod 755 /data/local/tmp/castle-tasks.sh; setsid sh /data/local/tmp/castle-tasks.sh > /data/local/tmp/castle-automation.log 2>&1 < /dev/null &") -Quiet | Out-Null
    Remove-Item $Lf -Force -ErrorAction SilentlyContinue
    Write-Host "  [OK] Tasks started (log on the phone: /data/local/tmp/castle-automation.log)." -ForegroundColor Green
}

Write-Host ""
Write-Host "  ================================================================" -ForegroundColor DarkGray
Write-Host "  Android is running." -ForegroundColor Green
Write-Host "  * adb:      $Adb -s $Serial shell" -ForegroundColor White
Write-Host "  * Install:  .\android.ps1 -Apk path\to\app.apk   (or drop APKs in data\copy and use -Copy)" -ForegroundColor White
Write-Host "  * Network:  guest has internet through the host; the host is 10.0.2.2 inside Android" -ForegroundColor White
Write-Host "  * Stop:     .\android.ps1 -Stop   (state is snapshotted; next boot resumes in seconds)" -ForegroundColor White
Write-Host ""
