<#
.SYNOPSIS
  Downloads the two models: Qwen3.5-9B (via Ollama) and FLUX.3 Image Klein (gated, via Hugging Face).
#>
param([switch]$OnlyQwen, [switch]$OnlyFlux)
. (Join-Path $PSScriptRoot 'common.ps1')

if (-not $OnlyFlux) {
  Write-Step "Qwen3.5 -> ollama pull $($Stack.ollama.model)"
  Start-Ollama
  Exec { ollama pull $Stack.ollama.model } 'ollama pull'
  # Aliases share the pulled weights (no extra disk); they add a 32k context + Qwen's coding sampler settings.
  # Thinking on/off is chosen per request by opencode (see opencode/opencode.template.json).
  foreach ($alias in $Stack.ollama.aliases) {
    Exec { ollama create $alias -f (Join-Path $Repo 'ollama\Modelfile') } "ollama create $alias"
    Write-Ok "model $alias ready"
  }
}

if (-not $OnlyQwen) {
  Write-Step "FLUX.3 Klein <- $($Stack.flux.hfRepo) (gated)"
  $hf = @('--from', 'huggingface_hub', 'hf')
  uvx @hf auth whoami *> $null
  if ($LASTEXITCODE -ne 0 -and -not $env:HF_TOKEN) {
    Write-Warn2 'Not logged in to Hugging Face. The FLUX repo is gated:'
    Write-Host  "      1. Open $($Stack.flux.licenseUrl) and accept the FLUX Kommunity license."
    Write-Host  '      2. Create a read token at https://huggingface.co/settings/tokens'
    Write-Host  '      3. Paste it at the prompt below (stored in ~/.cache/huggingface/token).'
    uvx @hf auth login
    if ($LASTEXITCODE -ne 0) { throw 'Hugging Face login failed' }
  }
  New-Item -ItemType Directory -Force $FluxDir | Out-Null
  $files = @($Stack.flux.files)
  uvx @hf download $Stack.flux.hfRepo @files --local-dir $FluxDir
  if ($LASTEXITCODE -ne 0) { throw "download failed - did you accept the license at $($Stack.flux.licenseUrl) ?" }
  foreach ($f in $files) {
    $p = Join-Path $FluxDir $f
    if (-not (Test-Path $p)) { throw "missing $p" }
    Write-Ok ("{0} ({1:N1} GB)" -f $f, ((Get-Item $p).Length / 1GB))
  }
}
