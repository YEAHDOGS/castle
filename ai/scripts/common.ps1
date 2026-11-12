# Shared helpers, dot-sourced by every script. Windows PowerShell 5.1 and PowerShell 7 compatible.
$ErrorActionPreference = 'Stop'

$Repo     = Split-Path -Parent $PSScriptRoot
$Stack    = Get-Content (Join-Path $Repo 'stack.json') -Raw | ConvertFrom-Json
$Runtime  = Join-Path $Repo 'runtime'
$ComfyDir = Join-Path $Runtime 'ComfyUI'
$FluxDir  = Join-Path $Runtime 'models\flux3-klein'
$Outputs  = Join-Path $Repo 'outputs'
$ComfyUrl = "http://127.0.0.1:$($Stack.comfyui.port)"
$VenvPy   = Join-Path $ComfyDir '.venv\Scripts\python.exe'

function Write-Step($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    ok: $msg" -ForegroundColor Green }
function Write-Warn2($msg){ Write-Host "    !! $msg" -ForegroundColor Yellow }

# Pick up PATH entries added by installers since this shell started (keeps the current entries).
function Update-SessionPath {
  $have = $env:Path -split ';'
  $new  = ([Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')) -split ';' |
          Where-Object { $_ -and ($have -notcontains $_) }
  if ($new) { $env:Path = (@($have) + @($new)) -join ';' }
}

# Run a native command and fail loudly on a non-zero exit code.
function Exec([scriptblock]$block, [string]$what) {
  & $block
  if ($LASTEXITCODE -ne 0) { throw "$what failed (exit $LASTEXITCODE)" }
}

Update-SessionPath

function Test-Cmd($name) { [bool](Get-Command $name -ErrorAction SilentlyContinue) }

function Test-Http($url) {
  try { Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 3 | Out-Null; $true } catch { $false }
}

function Start-Ollama {
  if (Test-Http "$($Stack.ollama.host)/api/version") { return }
  Start-Process ollama -ArgumentList 'serve' -WindowStyle Hidden
  for ($i = 0; $i -lt 30; $i++) { if (Test-Http "$($Stack.ollama.host)/api/version") { return }; Start-Sleep 1 }
  throw 'Ollama did not start on ' + $Stack.ollama.host
}

function Start-ComfyUI {
  if (Test-Http "$ComfyUrl/system_stats") { Write-Ok "ComfyUI already running at $ComfyUrl"; return }
  New-Item -ItemType Directory -Force $Outputs | Out-Null
  $log = Join-Path $Runtime 'comfyui.log'
  $cargs = @('main.py') + $Stack.comfyui.args + @('--output-directory', "`"$Outputs`"", '--extra-model-paths-config', "`"$(Join-Path $Runtime 'extra_model_paths.yaml')`"")
  $p = Start-Process -FilePath $VenvPy -ArgumentList $cargs -WorkingDirectory $ComfyDir -WindowStyle Hidden `
        -RedirectStandardOutput $log -RedirectStandardError "$log.err" -PassThru
  Set-Content (Join-Path $Runtime 'comfyui.pid') $p.Id
  for ($i = 0; $i -lt 180; $i++) {
    if (Test-Http "$ComfyUrl/system_stats") { Write-Ok "ComfyUI up at $ComfyUrl (pid $($p.Id))"; return }
    if ($p.HasExited) { throw "ComfyUI exited early, see $log.err" }
    Start-Sleep 1
  }
  throw "ComfyUI did not come up in 180s, see $log.err"
}
