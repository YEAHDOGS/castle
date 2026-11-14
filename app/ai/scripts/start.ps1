<#
.SYNOPSIS
  Starts the local stack: Ollama (Qwen3.5) and ComfyUI (FLUX.3 Klein). Pass -Opencode to launch opencode afterwards.
.NOTES
  ComfyUI is optional here: the comfy MCP server starts it on the first image request.
#>
param([switch]$Opencode, [switch]$NoComfy)
. (Join-Path $PSScriptRoot 'common.ps1')

Write-Step 'Ollama'
Start-Ollama
Write-Ok $Stack.ollama.host

if (-not $NoComfy) { Write-Step 'ComfyUI'; Start-ComfyUI }

if ($Opencode) { opencode }
