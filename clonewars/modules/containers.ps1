# ==============================================================================
# CONTAINERS -- the Docker engine, Castle's images, and KVM inside Docker Desktop
# ==============================================================================
# Used by container.ps1 (automation and Android containers). Plain helpers:
# nothing here starts a container. Every docker and wsl call goes through
# Invoke-Quiet (modules/quiet.ps1), so none of them opens a console window.

. (Join-Path $PSScriptRoot "quiet.ps1")

# A plain function on purpose: docker's own flags (-f, -t, ...) land in $args as they are.
function Invoke-Docker { return Invoke-Quiet docker ([string[]]$args) }

function Test-DockerCli { return [bool](Get-Command docker -ErrorAction SilentlyContinue) }

function Test-DockerEngine {
    $R = Invoke-Docker version --format '{{.Server.Version}}'
    return ($R.ExitCode -eq 0 -and $R.Out.Trim() -ne "")
}

function Initialize-DockerEngine {
    <#
    .SYNOPSIS
        Makes sure a Docker engine answers. On Windows, a Docker Desktop that is
        installed but not running is started (minimized) and waited for.
    .OUTPUTS
        $true when the engine answers.
    #>
    param ([int]$TimeoutSec = 180)
    if (-not (Test-DockerCli)) {
        Write-Host "  [FAIL] docker not found on PATH." -ForegroundColor Red
        Write-Host "     Install Docker Desktop:  winget install Docker.DockerDesktop   (then reboot)" -ForegroundColor DarkGray
        Write-Host "     or an engine in WSL2 / Linux:  sudo apt install -y docker.io" -ForegroundColor DarkGray
        return $false
    }
    if (Test-DockerEngine) { return $true }
    $Desktop = Join-Path $env:ProgramFiles "Docker\Docker\Docker Desktop.exe"
    if ($env:ProgramFiles -and (Test-Path $Desktop)) {
        Write-Host "  [DOCKER] The engine is not running; starting Docker Desktop..." -ForegroundColor Yellow
        Start-Process -FilePath $Desktop -WindowStyle Minimized
        $Sw = [System.Diagnostics.Stopwatch]::StartNew()
        while ($Sw.Elapsed.TotalSeconds -lt $TimeoutSec) {
            Start-Sleep -Seconds 3
            if (Test-DockerEngine) {
                Write-Host ("  [OK] Docker engine up after {0:N0} s." -f $Sw.Elapsed.TotalSeconds) -ForegroundColor Green
                return $true
            }
        }
        Write-Host "  [FAIL] Docker Desktop did not answer within $TimeoutSec s." -ForegroundColor Red
        return $false
    }
    Write-Host "  [FAIL] The Docker engine is not running (docker version could not reach it)." -ForegroundColor Red
    return $false
}

function Enable-DockerKvm {
    <#
    .SYNOPSIS
        /dev/kvm for containers. Docker Desktop's WSL2 VM has the kvm_intel /
        kvm_amd modules but does not load them or create /dev/kvm by itself
        (and sometimes leaves a stray /dev/kvm directory). Not persistent:
        Docker Desktop restarts reset it, so this runs before every start.
    .OUTPUTS
        $true when /dev/kvm is (or is assumed to be) there.
    #>
    $IsWin = -not (($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true) -or ($IsMacOS -eq $true))
    if (-not $IsWin) { return (Test-Path /dev/kvm) }
    $Distros = ((Invoke-Quiet wsl @("-l", "-q")).Out -replace "`0", "") -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    if ($Distros -notcontains "docker-desktop") {
        Write-Host "  [*] No docker-desktop WSL distro; assuming the engine already exposes /dev/kvm." -ForegroundColor Yellow
        return $true
    }
    $KvmFix = 'modprobe kvm_intel 2>/dev/null || modprobe kvm_amd 2>/dev/null; ' +
              '[ -d /dev/kvm ] && rmdir /dev/kvm; sleep 1; ' +
              '[ -c /dev/kvm ] || mknod /dev/kvm c 10 232; chmod 666 /dev/kvm; ' +
              '[ -c /dev/kvm ] && echo kvm-ok'
    $Kvm = Invoke-Quiet wsl @("-d", "docker-desktop", "--", "sh", "-c", $KvmFix)
    $KvmOut = "$($Kvm.Out) $($Kvm.Err)".Trim()
    if ("$KvmOut" -match "kvm-ok") {
        Write-Host "  [OK] /dev/kvm ready in Docker Desktop's VM." -ForegroundColor Green
        return $true
    }
    Write-Host "  [FAIL] Could not set up /dev/kvm in the docker-desktop distro: $KvmOut" -ForegroundColor Red
    Write-Host "     Needs Windows 11 with nested virtualization (default on) and the Docker Desktop WSL2 backend." -ForegroundColor DarkGray
    return $false
}

function Get-CastleImageTag {
    <#
    .SYNOPSIS
        castle/<kind>:<12 hex>, the hash of every file in the image's folder,
        so an edit to the Dockerfile or its scripts makes a new tag.
    #>
    param ([string]$Kind, [string]$Folder)
    $Sha = [System.Security.Cryptography.SHA256]::Create()
    $Stream = [System.IO.MemoryStream]::new()
    foreach ($File in Get-ChildItem -LiteralPath $Folder -Recurse -File | Sort-Object { $_.FullName.Substring($Folder.Length) -replace '\\', '/' }) {
        $Rel = [System.Text.Encoding]::UTF8.GetBytes(($File.FullName.Substring($Folder.Length) -replace '\\', '/') + "`n")
        $Stream.Write($Rel, 0, $Rel.Length)
        # Line endings do not count: a CRLF checkout builds the same image.
        $Bytes = [System.Text.Encoding]::UTF8.GetBytes(([System.IO.File]::ReadAllText($File.FullName) -replace "`r", ""))
        $Stream.Write($Bytes, 0, $Bytes.Length)
    }
    $Hash = -join ($Sha.ComputeHash($Stream.ToArray()) | Select-Object -First 6 | ForEach-Object { $_.ToString("x2") })
    return "castle/${Kind}:$Hash"
}

function Install-CastleImage {
    <#
    .SYNOPSIS
        Builds castle/<kind> from docker/<kind> unless that exact version is
        already in the engine. Returns the tag, or $null when the build failed.
    #>
    param ([string]$Kind, [string]$Folder, [string[]]$BuildArgs = @())
    $Tag = Get-CastleImageTag -Kind $Kind -Folder $Folder
    if ((Invoke-Docker image inspect $Tag).ExitCode -eq 0) {
        Write-Host "  [OK] Image $Tag is built." -ForegroundColor Green
        return $Tag
    }
    $Note = if ($Kind -eq "android") { "the SDK and a system image, ~3 GB, 5-15 min" } else { "Debian + X server, ~250 MB, a minute or two" }
    Write-Host "  [DOCKER] Building $Tag once ($Note)..." -ForegroundColor Cyan
    $Build = @("build", "--progress", "plain", "-t", $Tag, "-t", "castle/${Kind}:latest")
    foreach ($B in $BuildArgs) { $Build += @("--build-arg", $B) }
    $Build += $Folder
    $R = Invoke-Quiet docker $Build -Stream
    if ($R.ExitCode -ne 0) {
        Write-Host "  [FAIL] docker build of $Tag exited with $($R.ExitCode)." -ForegroundColor Red
        return $null
    }
    Write-Host "  [OK] Built $Tag." -ForegroundColor Green
    return $Tag
}
