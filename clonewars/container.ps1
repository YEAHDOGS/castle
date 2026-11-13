<#
.SYNOPSIS
    Castle -- automations and machines in containers instead of VMs.
.DESCRIPTION
    The light way to run a clone. A VM boots a kernel, an init system and a
    desktop for every bot; a container shares the host's kernel (Docker
    Desktop's on Windows) and starts only what the bot needs, so it is up in
    seconds and uses the memory its programs use, not a fixed guest size.

    Two kinds, one image each, shared by every container made from it:
      container-debian   Debian 13 + an X server in memory (Xvfb) + Openbox.
                         Runs the same scripts/setup-*.sh a VM does, so a
                         launch profile (packages, root and user commands, a
                         payload) works the same; the screen is 1440x900 or
                         the profile's Screen.
      container-android  Google's Android Emulator, headless, on KVM. Installs
                         every .apk in the payload, pushes the rest to
                         Download/castle-copy and runs castle-tasks.sh on the
                         phone.

    A container is a machine: named castle-<instance> (castle-<target> without
    one), its writable layer is its disk, so what a setup installs is kept
    and the second start skips it. -Temporary makes a throwaway (removed when
    it stops). The payload and the launch profile are staged into
    data/containers/<name>/ before every start and mounted read-only, the
    scripts folder too; nothing on the host is writable from inside.

    start.ps1 hands container targets to this script, so profiles work:
        .\start.ps1 -Profile penguin -Target container-debian
.EXAMPLE
    .\container.ps1 container-debian -Instance job-01        # make (or start) castle-job-01
    .\container.ps1 container-debian -Instance job-01 -Stop  # stop it (it is kept)
    .\container.ps1 container-debian -Instance job-01 -Remove
    .\container.ps1 container-android -Instance phone-1 -Copy   # data\copy: APKs installed, the rest pushed
    .\container.ps1 -List
#>
param (
    [Parameter(Position = 0)]
    [string]$Target,
    [string]$Instance,
    [Alias("c")]
    [switch]$Copy,
    # Folder whose files ride in as the payload (default: data/copy, or CASTLE_COPY_DIR).
    [string]$CopyDir,
    # A castle-profile.env written by start.ps1 (packages and commands for the setup).
    [string]$ProfileEnv,
    # scripts/setup-<name>.sh to run; "auto" and empty mean setup-profile.sh.
    [string]$Setup = "profile",
    [string]$Screen,
    [int]$MemoryGB = 0,
    [int]$Cores = 0,
    [int]$StopAfterMinutes = 0,
    [switch]$Temporary,
    [switch]$Stop,
    [switch]$Remove,
    [switch]$List,
    [int]$TimeoutMin = 0
)

$ErrorActionPreference = "Stop"
. (Join-Path (Join-Path $PSScriptRoot "modules") "targets.ps1")
. (Join-Path (Join-Path $PSScriptRoot "modules") "containers.ps1")

$DataDir = Join-Path $PSScriptRoot "data"
$Label = "net.wearedogs.castle.unit"

if ($List) {
    if (-not (Initialize-DockerEngine)) { Exit 1 }
    $R = Invoke-Docker ps -a --filter "label=$Label" --format "table {{.Names}}\t{{.Image}}\t{{.Status}}"
    Write-Host $R.Out
    Exit $R.ExitCode
}

$Entry = $IsoMatrix | Where-Object { $_.Id -eq $Target -and $_.Runtime -eq "container" } | Select-Object -First 1
if (-not $Entry) {
    $Known = @($IsoMatrix | Where-Object { $_.Runtime -eq "container" } | ForEach-Object Id)
    Write-Host "  [FAIL] '$Target' is not a container target. Container targets: $($Known -join ', ')" -ForegroundColor Red
    Exit 1
}
if ($Instance -and $Instance -notmatch '^[a-z0-9][a-z0-9-]{0,62}$') {
    Write-Host "  [FAIL] -Instance must be lowercase letters, digits and dashes." -ForegroundColor Red
    Exit 1
}
$Kind = [string]$Entry.ContainerKind
$Name = "castle-$(if ($Instance) { $Instance } else { $Entry.Id })"
$Stage = Join-Path (Join-Path $DataDir "containers") $Name

if (-not (Initialize-DockerEngine)) { Exit 1 }

function Get-ContainerState {
    # "" when there is no such container, else running | exited | created | ...
    # "?" when docker itself could not be asked (it did not start: low memory, say).
    $R = Invoke-Docker inspect -f '{{.State.Status}}' $Name
    if ($R.ExitCode -eq 127) { return "?" }
    if ($R.ExitCode -ne 0) { return "" }
    return $R.Out.Trim()
}

if ($Stop -or $Remove) {
    $State = Get-ContainerState
    if ($State -eq "?") { Write-Host "  [FAIL] docker could not be started (is this computer out of memory?)." -ForegroundColor Red; Exit 1 }
    if ($State -eq "running") {
        Write-Host "  [STOP] Stopping $Name..." -ForegroundColor Yellow
        Invoke-Docker stop -t 20 $Name | Out-Null
    }
    if ($Remove) {
        if ($State) { Invoke-Docker rm -f $Name | Out-Null }
        if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
        Write-Host "  [OK] $Name removed, its disk with it." -ForegroundColor Green
    }
    elseif ($State) { Write-Host "  [OK] $Name stopped (kept: the next start resumes it)." -ForegroundColor Green }
    else { Write-Host "  [i] There is no container called $Name." -ForegroundColor DarkGray }
    Exit 0
}

# CASTLE_VM_SNAPSHOT=1 (a throwaway run) and the sizes start.ps1 takes from a profile.
if ($env:CASTLE_VM_SNAPSHOT -eq "1") { $Temporary = $true }
if ($MemoryGB -le 0 -and $env:CASTLE_VM_RAM_GB) { $MemoryGB = [int]$env:CASTLE_VM_RAM_GB }
if ($Cores -le 0 -and $env:CASTLE_VM_CORES) { $Cores = [int]$env:CASTLE_VM_CORES }
if ($StopAfterMinutes -le 0 -and $env:CASTLE_STOP_AFTER_MIN) { $StopAfterMinutes = [int]$env:CASTLE_STOP_AFTER_MIN }
if (-not $CopyDir) { $CopyDir = if ($env:CASTLE_COPY_DIR) { $env:CASTLE_COPY_DIR } else { Join-Path $DataDir "copy" } }
if ($Setup -eq "auto" -or -not $Setup) { $Setup = "profile" }
if ($Setup -notmatch '^[a-z0-9-]+$') { Write-Host "  [FAIL] -Setup must be a plain script name." -ForegroundColor Red; Exit 1 }
if ($Screen -and $Screen -notmatch '^\d{3,5}x\d{3,5}$') { Write-Host "  [FAIL] -Screen must look like 1440x900." -ForegroundColor Red; Exit 1 }

Write-Host ""
Write-Host "  Castle containers -- $($Entry.Name)" -ForegroundColor Cyan
Write-Host "  Container: $Name$(if ($Temporary) { '  (temporary: removed when it stops)' })" -ForegroundColor White
Write-Host "  ================================================================" -ForegroundColor DarkGray

# -- The image ------------------------------------------------------------------
$ImageDir = Join-Path (Join-Path $PSScriptRoot "docker") $Kind
$BuildArgs = @()
if ($Kind -eq "android") {
    if (-not (Enable-DockerKvm)) { Exit 1 }
    # The SDK license the owner accepted on this host (android.ps1 asks once).
    $Licenses = Join-Path (Join-Path (Join-Path $DataDir "android-sdk") "licenses") "android-sdk-license"
    $Into = Join-Path $ImageDir "licenses"
    if (Test-Path $Licenses) {
        New-Item -ItemType Directory -Path $Into -Force | Out-Null
        Copy-Item (Join-Path (Split-Path $Licenses -Parent) "*") $Into -Force
    }
    elseif (-not (Test-Path (Join-Path $Into "android-sdk-license"))) {
        Write-Host "  [FAIL] The Android SDK license has not been accepted on this box yet." -ForegroundColor Red
        Write-Host "     Run .\android.ps1 -InstallOnly once and accept it (https://developer.android.com/studio/terms)." -ForegroundColor DarkGray
        Exit 1
    }
    if ($env:CASTLE_CONTAINER_ANDROID_IMAGE) { $BuildArgs += "ANDROID_IMAGE=$($env:CASTLE_CONTAINER_ANDROID_IMAGE)" }
}
$Image = Install-CastleImage -Kind $Kind -Folder $ImageDir -BuildArgs $BuildArgs
if (-not $Image) { Exit 1 }

# -- Stage what goes in -----------------------------------------------------------
# Fixed folders per container, so a kept container's mounts stay valid; their
# contents are replaced on every start.
$StageCopy = Join-Path $Stage "copy"
$StageProfile = Join-Path $Stage "profile"
foreach ($D in @($StageCopy, $StageProfile)) {
    if (Test-Path $D) { Remove-Item $D -Recurse -Force }
    New-Item -ItemType Directory -Path $D -Force | Out-Null
}
$Payload = 0
if (($Copy -or $env:CASTLE_COPY_DIR -or $ProfileEnv) -and (Test-Path $CopyDir)) {
    foreach ($Item in Get-ChildItem -LiteralPath $CopyDir -Force | Where-Object { $_.Name -ne "README.md" }) {
        Copy-Item -LiteralPath $Item.FullName -Destination $StageCopy -Recurse -Force
    }
    $Payload = @(Get-ChildItem -LiteralPath $StageCopy -Recurse -File).Count
}
if ($ProfileEnv -and (Test-Path $ProfileEnv)) { Copy-Item $ProfileEnv (Join-Path $StageProfile "castle-profile.env") -Force }
# Per-launch settings the container reads at start (its environment is fixed when it is made).
$Settings = @("CASTLE_SETUP='$Setup'", "CASTLE_STOP_AFTER_MIN='$StopAfterMinutes'")
if ($Screen) { $Settings += "CASTLE_SCREEN='$Screen'" }
if ($Kind -eq "android") {
    $Settings += "CASTLE_MEMORY_MB='$([math]::Max(2048, $(if ($MemoryGB -gt 0) { $MemoryGB * 1024 } else { 3072 })))'"
    $Settings += "CASTLE_CORES='$(if ($Cores -gt 0) { $Cores } else { 2 })'"
    if ($Temporary) { $Settings += "CASTLE_WIPE='1'" }
}
[System.IO.File]::WriteAllText((Join-Path $StageProfile "castle-container.env"), (($Settings -join "`n") + "`n"), [System.Text.UTF8Encoding]::new($false))
Write-Host "  [STAGE] $Payload payload file(s)$(if ($ProfileEnv) { ' + launch profile' }) in $Stage" -ForegroundColor Cyan

# -- Make or reuse the container -------------------------------------------------
$State = Get-ContainerState
if ($State -eq "?") {
    Write-Host "  [FAIL] docker could not be started (is this computer out of memory?)." -ForegroundColor Red
    Exit 1
}
if ($State -eq "running") {
    Write-Host "  [i] $Name is already running." -ForegroundColor Green
    Exit 0
}
if ($State) {
    $Was = (Invoke-Docker inspect -f '{{.Config.Image}}' $Name).Out.Trim()
    if ($Temporary -or $Was -ne $Image) {
        $Why = if ($Temporary) { "a temporary run starts clean" } else { "it was made from $Was; the image is now $Image" }
        Write-Host "  [i] Making $Name again ($Why)." -ForegroundColor Yellow
        Invoke-Docker rm -f $Name | Out-Null
        $State = ""
    }
}

# Memory and CPU limits. Android's emulator needs headroom above the phone's RAM:
# at +2 GB a 3 GB phone sat at its limit (4.6 of 5 GB) and drew a black screen.
$LimitGB = if ($MemoryGB -gt 0) { $MemoryGB } else { 4 }
if ($Kind -eq "android") { $LimitGB += 3 }   # the emulator itself uses ~1.5-2.5 GB on top of the phone's RAM
$CpuCount = if ($Cores -gt 0) { [math]::Min($Cores, [Environment]::ProcessorCount) } else { [math]::Min(4, [Environment]::ProcessorCount) }
$Limits = @("--memory", "${LimitGB}g", "--memory-swap", "${LimitGB}g", "--cpus", "$CpuCount")

if (-not $State) {
    $Create = @("create", "--name", $Name, "--hostname", $Name, "--init", "--label", "$Label=$Name", "--label", "net.wearedogs.castle.target=$($Entry.Id)") + $Limits
    $Create += @("--mount", "type=bind,source=$StageCopy,target=/mnt/castle-copy,readonly",
                 "--mount", "type=bind,source=$StageProfile,target=/mnt/castle-profile,readonly")
    if ($Kind -eq "automation") {
        $Create += @("--shm-size", "1g", "--mount", "type=bind,source=$(Join-Path $PSScriptRoot 'scripts'),target=/mnt/castle,readonly")
        # The live viewer's port (castle-live), on this machine's loopback only. Nothing
        # listens on it until "Take control" starts the viewer with a one-time password.
        $Create += @("--publish", "127.0.0.1::6080")
    }
    if ($Kind -eq "android") { $Create += @("--device", "/dev/kvm") }
    if ($Temporary) { $Create += "--rm" }
    $Create += $Image
    $R = Invoke-Quiet docker $Create
    if ($R.ExitCode -ne 0) { Write-Host "  [FAIL] docker create: $($R.Err.Trim())" -ForegroundColor Red; Exit 1 }
    Write-Host "  [OK] Made $Name from $Image ($LimitGB GB, $CpuCount CPU)." -ForegroundColor Green
}
else {
    Invoke-Quiet docker (@("update") + $Limits + $Name) | Out-Null
}

$Since = [DateTimeOffset]::UtcNow.AddSeconds(-1).ToUnixTimeSeconds()
if ((Invoke-Docker start $Name).ExitCode -ne 0) { Write-Host "  [FAIL] docker start $Name failed." -ForegroundColor Red; Exit 1 }
# Marks the hand-off for the Castle app's launch timings, like QMP: does for a VM.
Write-Host "  CONTAINER: $Name started" -ForegroundColor Green

# -- Follow the setup -------------------------------------------------------------
$Limit = if ($TimeoutMin -gt 0) { $TimeoutMin } elseif ($Kind -eq "android") { 20 } else { 60 }
$Sw = [System.Diagnostics.Stopwatch]::StartNew()
$ShownOut = 0
$ShownErr = 0
$Result = $null
while ($Sw.Elapsed.TotalMinutes -lt $Limit) {
    Start-Sleep -Seconds 2
    # One call every two seconds, windowless (see modules/quiet.ps1). The container's own
    # output arrives on both streams; the setup prints to stdout.
    $R = Invoke-Docker logs --since $Since $Name
    $Lines = @($R.Out -split "`r?`n" | Where-Object { $_ -ne "" })
    $ErrLines = @($R.Err -split "`r?`n" | Where-Object { $_ -ne "" })
    for ($i = $ShownErr; $i -lt $ErrLines.Count; $i++) { Write-Host "    | $($ErrLines[$i])" -ForegroundColor DarkGray }
    $ShownErr = [math]::Max($ShownErr, $ErrLines.Count)
    for ($i = $ShownOut; $i -lt $Lines.Count; $i++) {
        Write-Host "    | $($Lines[$i])"
        if ($Lines[$i] -match '^\[castle\] setup finished') { $Result = 0 }
        elseif ($Lines[$i] -match '^\[castle\] setup failed') { $Result = 1 }
    }
    $ShownOut = [math]::Max($ShownOut, $Lines.Count)
    if ($null -ne $Result) { break }
    # Only a real answer counts: a docker call that could not start says nothing about the container.
    if ((Get-ContainerState) -notin @("running", "?")) { Write-Host "  [FAIL] $Name stopped during setup." -ForegroundColor Red; Exit 1 }
}
if ($null -eq $Result) { Write-Host "  [FAIL] The setup did not finish within $Limit min; $Name keeps running." -ForegroundColor Red; Exit 1 }
if ($Result -ne 0) { Write-Host "  [FAIL] The setup failed; $Name keeps running so its screen can be looked at." -ForegroundColor Red; Exit 1 }

Write-Host ""
Write-Host ("  [OK] {0} is set up and running ({1:N0} s)." -f $Name, $Sw.Elapsed.TotalSeconds) -ForegroundColor Green
Write-Host "  * Log:     docker logs -f $Name" -ForegroundColor White
Write-Host "  * Shell:   docker exec -it $Name bash" -ForegroundColor White
Write-Host "  * Stop:    .\container.ps1 $($Entry.Id)$(if ($Instance) { " -Instance $Instance" }) -Stop" -ForegroundColor DarkGray
Exit 0
