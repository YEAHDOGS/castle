# Starts ComfyUI in the background if it is not already running. Used by start.ps1 and by the comfy MCP server.
. (Join-Path $PSScriptRoot 'common.ps1')
Start-ComfyUI
