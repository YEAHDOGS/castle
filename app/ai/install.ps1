<#
.SYNOPSIS
  One-shot installer for the dogs-ai local stack: ComfyUI + FLUX.3 Klein, Ollama + Qwen3.5, opencode + the comfy MCP pipeline.
.DESCRIPTION
  Idempotent: re-run it any time to repair or update to the versions pinned in stack.json.
  Uses Chocolatey when run from an elevated shell with choco installed, otherwise winget (per-user, no admin).
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\install.ps1
  .\install.ps1 -SkipModels        # everything except the big downloads
#>
param(
  [switch]$SkipModels,
  [switch]$SkipOpencodeConfig
)
. (Join-Path $PSScriptRoot 'scripts\common.ps1')

# ---------------------------------------------------------------- 1. prerequisites
Write-Step 'Prerequisites (git, node, uv, ollama)'
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$useChoco = $isAdmin -and (Test-Cmd 'choco')
$prereqs = @(
  @{ cmd = 'git';    choco = 'git';        winget = 'Git.Git' },
  @{ cmd = 'node';   choco = 'nodejs-lts'; winget = 'OpenJS.NodeJS.LTS' },
  @{ cmd = 'uv';     choco = 'uv';         winget = 'astral-sh.uv' },
  @{ cmd = 'ollama'; choco = 'ollama';     winget = 'Ollama.Ollama' }
)
foreach ($p in $prereqs) {
  if (Test-Cmd $p.cmd) { Write-Ok "$($p.cmd) present"; continue }
  if ($useChoco) { choco install $p.choco -y --no-progress }
  else { winget install --id $p.winget -e --silent --accept-package-agreements --accept-source-agreements }
  Update-SessionPath
  if (-not (Test-Cmd $p.cmd)) { throw "Failed to install $($p.cmd). Install it manually and re-run." }
  Write-Ok "$($p.cmd) installed"
}
if (-not (Test-Cmd 'nvidia-smi')) { Write-Warn2 'nvidia-smi not found: ComfyUI needs an NVIDIA GPU + driver for usable speed.' }

# ---------------------------------------------------------------- 2. ComfyUI
Write-Step "ComfyUI $($Stack.comfyui.ref)"
New-Item -ItemType Directory -Force $Runtime, $Outputs, $FluxDir | Out-Null
if (-not (Test-Path (Join-Path $ComfyDir '.git'))) {
  Exec { git clone $Stack.comfyui.repo $ComfyDir } 'git clone ComfyUI'
}
Exec { git -C $ComfyDir fetch --tags --quiet } 'git fetch ComfyUI'
Exec { git -C $ComfyDir -c advice.detachedHead=false checkout --quiet $Stack.comfyui.ref } 'git checkout ComfyUI'
Write-Ok "checked out $($Stack.comfyui.ref)"

$venv = Join-Path $ComfyDir '.venv'
if (-not (Test-Path $VenvPy)) {
  uv venv --python $Stack.comfyui.python $venv
  if (-not (Test-Path $VenvPy)) {
    # A half-finished uv-managed Python install breaks venv creation; reinstall it once and retry.
    uv python install $Stack.comfyui.python --reinstall
    Exec { uv venv --python $Stack.comfyui.python $venv } 'uv venv'
  }
}
Exec { uv pip install --python $VenvPy torch torchvision torchaudio --index-url $Stack.comfyui.torchIndex } 'torch install'
Exec { uv pip install --python $VenvPy -r (Join-Path $ComfyDir 'requirements.txt') } 'ComfyUI requirements'
Exec { & $VenvPy -c "import torch; print('    torch', torch.__version__, 'cuda', torch.cuda.is_available())" } 'torch check'

# Point ComfyUI at the FLUX download folder (keeps the HF repo layout, no copying) and at runtime/models/linked,
# where the dogs server hard-links every model it finds (project models/ folder, every user's HF cache).
$fluxFwd = $FluxDir -replace '\\', '/'
$linkedFwd = (Join-Path $Runtime 'models\linked') -replace '\\', '/'
@"
dogs_ai:
  base_path: $fluxFwd
  diffusion_models: diffusion_models
  text_encoders: text_encoders
  vae: vae
dogs_linked:
  base_path: $linkedFwd
  diffusion_models: diffusion_models
  text_encoders: text_encoders
  vae: vae
  checkpoints: checkpoints
  loras: loras
"@ | Set-Content -Encoding ascii (Join-Path $Runtime 'extra_model_paths.yaml')
Write-Ok 'extra_model_paths.yaml written'

# ---------------------------------------------------------------- 3. opencode + MCP pipeline
Write-Step "opencode $($Stack.opencode.version)"
$have = ''
if (Test-Cmd 'opencode') { $have = (opencode --version 2>$null | Select-Object -Last 1) }
if ($have -ne $Stack.opencode.version) { Exec { npm install -g "$($Stack.opencode.npm)@$($Stack.opencode.version)" } 'npm install opencode' }
Update-SessionPath
Write-Ok "opencode $(opencode --version 2>$null | Select-Object -Last 1)"

Write-Step 'comfy MCP server (npm ci)'
Push-Location (Join-Path $Repo 'mcp\comfy-mcp'); try { Exec { npm ci --no-audit --no-fund } 'npm ci comfy-mcp' } finally { Pop-Location }

Write-Step 'dogs CLI + DOGS AI app'
Push-Location $Repo; try {
  Exec { npm link --no-audit --no-fund } 'npm link (dogs command)'
  Exec { npm run app:install } 'app install'
  Exec { npm --prefix app run build } 'app build'
} finally { Pop-Location }

# ---------------------------------------------------------------- 4. models
if (-not $SkipModels) {
  & (Join-Path $Repo 'scripts\download-models.ps1')
} else { Write-Warn2 'Skipping model downloads (-SkipModels). Run scripts\download-models.ps1 later.' }

# ---------------------------------------------------------------- 5. opencode global config
if (-not $SkipOpencodeConfig) { & (Join-Path $Repo 'scripts\link-opencode.ps1') }

Write-Step 'Done. Next:'
Write-Host '    dogs models                    # what is installed / missing'
Write-Host '    dogs gen image "a red fox"     # generate from the terminal'
Write-Host '    npm run desktop                # the DOGS AI desktop app'
Write-Host '    .\scripts\start.ps1            # starts Ollama + ComfyUI'
Write-Host '    opencode                       # in any project folder'
Write-Host '    .\scripts\doctor.ps1           # health check'
