# Stops the ComfyUI process started by start.ps1 / the MCP server. Ollama is left running (it idles at ~0 VRAM).
. (Join-Path $PSScriptRoot 'common.ps1')
$pidFile = Join-Path $Runtime 'comfyui.pid'
if (Test-Path $pidFile) {
  $id = [int](Get-Content $pidFile)
  Stop-Process -Id $id -ErrorAction SilentlyContinue
  Remove-Item $pidFile
  Write-Ok "stopped ComfyUI (pid $id)"
} else { Write-Warn2 'no ComfyUI pid file; nothing to stop' }
