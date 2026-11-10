<#
.SYNOPSIS
    Starts Castle on this computer: the box with its web app, then the desktop app.

.DESCRIPTION
    1. Builds the box daemon (castled) and the web app if they have not been built yet.
    2. Starts the box in the background, serving the web app at http://127.0.0.1:<port>.
    3. Opens the desktop app, which connects to that same box.

    The box keeps running after the desktop app is closed, so the web app stays up.
    Stop it with:  .\start.ps1 -Stop

    The vault, the games folder, the clonewars folder and the port come from the desktop
    app's box.json (%APPDATA%\net.wearedogs.castle\box.json), so the web app and the
    desktop app always show the same box. -Data and -Port override it for one run.

.EXAMPLE
    .\start.ps1              start everything
    .\start.ps1 -Build       rebuild the daemon and the web app first
    .\start.ps1 -Browser     also open the web app in the default browser
    .\start.ps1 -NoApp       web service only
    .\start.ps1 -Stop        stop the box this script started
#>
[CmdletBinding()]
param(
    [switch]$Build,
    [switch]$Browser,
    [switch]$NoApp,
    [switch]$Stop,
    [string]$Data,
    [int]$Port
)

$ErrorActionPreference = 'Stop'

$DefaultPort = 8787
$StartTimeoutSeconds = 20
$ProbeMilliseconds = 250
$Root = $PSScriptRoot
$DaemonDir = Join-Path $Root 'castled'
$ClientDir = Join-Path $Root 'client'
$DaemonExe = Join-Path $DaemonDir 'target\release\castled.exe'
$WebDir = Join-Path $ClientDir 'dist'
$ConfigDir = Join-Path $env:APPDATA 'net.wearedogs.castle'
$ConfigFile = Join-Path $ConfigDir 'box.json'
$PidFile = Join-Path $ConfigDir 'start.pid'
$LogFile = Join-Path $ConfigDir 'start-box.log'
# The desktop app: a build from this checkout if there is one, otherwise the installed copy.
$AppCandidates = @(
    (Join-Path $ClientDir 'src-tauri\target\release\castle.exe'),
    (Join-Path $env:LOCALAPPDATA 'Castle\castle.exe')
)

function Write-Step([string]$Text) { Write-Host "[castle] $Text" -ForegroundColor Cyan }

function Test-Listening([int]$OnPort) {
    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        return $client.ConnectAsync('127.0.0.1', $OnPort).Wait($ProbeMilliseconds)
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

# The box this script started earlier, if it is still running.
function Get-StartedBox {
    if (-not (Test-Path $PidFile)) { return $null }
    $saved = Get-Content $PidFile -Raw | ConvertFrom-Json
    $process = Get-Process -Id $saved.pid -ErrorAction SilentlyContinue
    if ($process -and $process.ProcessName -eq 'castled') { return $saved }
    Remove-Item $PidFile -Force
    return $null
}

if ($Stop) {
    $started = Get-StartedBox
    if (-not $started) { Write-Step 'No box started by this script is running.'; return }
    Stop-Process -Id $started.pid -Force
    Remove-Item $PidFile -Force
    Write-Step "Stopped the box (process $($started.pid))."
    return
}

# ---- settings shared with the desktop app ----
New-Item -ItemType Directory -Force $ConfigDir | Out-Null
$config = if (Test-Path $ConfigFile) { Get-Content $ConfigFile -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
if (-not $Port) { $Port = if ($config.port) { [int]$config.port } else { $DefaultPort } }
if (-not $Data) { $Data = if ($config.data) { [string]$config.data } else { Join-Path $env:USERPROFILE 'Castle' } }
$url = "http://127.0.0.1:$Port"

# ---- build what is missing ----
if ($Build -or -not (Test-Path $DaemonExe)) {
    Write-Step 'Building the box daemon (the first build takes a few minutes).'
    Push-Location $DaemonDir
    try { cargo build --release; if ($LASTEXITCODE) { throw 'The daemon did not build.' } } finally { Pop-Location }
}
if ($Build -or -not (Test-Path (Join-Path $WebDir 'index.html'))) {
    Write-Step 'Building the web app.'
    Push-Location $ClientDir
    try {
        if (-not (Test-Path 'node_modules')) { npm ci --no-audit --no-fund; if ($LASTEXITCODE) { throw 'npm ci failed.' } }
        npm run build; if ($LASTEXITCODE) { throw 'The web app did not build.' }
    } finally { Pop-Location }
}

# ---- the box ----
$started = Get-StartedBox
if ($started -and $Build) {
    Stop-Process -Id $started.pid -Force
    Remove-Item $PidFile -Force
    $started = $null
    Start-Sleep -Milliseconds 500
}
if ($started -and $started.port -eq $Port) {
    Write-Step "The box is already running (process $($started.pid))."
} elseif (Test-Listening $Port) {
    throw "Something else is already listening on port $Port. If the Castle desktop app is open, close it and run this again: on its own it starts a box without the web app."
} else {
    New-Item -ItemType Directory -Force $Data | Out-Null
    $boxArgs = @('--data', $Data, '--bind', "127.0.0.1:$Port", '--web', $WebDir)
    # A folder that is not there right now (an unplugged drive) is left out, so the box still starts.
    if ($config.arcade -and (Test-Path $config.arcade)) { $boxArgs += @('--arcade', [string]$config.arcade) }
    if ($config.clonewars -and (Test-Path $config.clonewars)) { $boxArgs += @('--clonewars', [string]$config.clonewars) }
    $quoted = $boxArgs | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }
    $box = Start-Process -FilePath $DaemonExe -ArgumentList $quoted -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput $LogFile -RedirectStandardError "$LogFile.err"
    $deadline = (Get-Date).AddSeconds($StartTimeoutSeconds)
    while (-not (Test-Listening $Port)) {
        if ($box.HasExited -or (Get-Date) -gt $deadline) {
            throw "The box did not start. See $LogFile.err"
        }
        Start-Sleep -Milliseconds $ProbeMilliseconds
    }
    @{ pid = $box.Id; port = $Port } | ConvertTo-Json | Set-Content $PidFile
    Write-Step "Box started (process $($box.Id)). Vault: $Data"
}

$token = (Get-Content (Join-Path $Data '.castle\token') -Raw).Trim()
$link = "$url/#connect=$token"
Write-Step "Web app: $link"
Write-Step '         (the part after # is the key to the box; do not share the link)'
if ($Browser) { Start-Process $link }

# ---- the desktop app ----
if (-not $NoApp) {
    $app = $AppCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $app) {
        Write-Step 'No desktop app found. Build one with:  cd client; npm run desktop'
    } elseif (Get-Process castle -ErrorAction SilentlyContinue) {
        Write-Step 'The desktop app is already open.'
    } else {
        Start-Process $app
        Write-Step "Desktop app opened ($app)."
    }
}
Write-Step 'The box keeps running in the background. Stop it with:  .\start.ps1 -Stop'
