<#
.SYNOPSIS
  Renders opencode/opencode.template.json into the global opencode config (~/.config/opencode/opencode.json)
  so the Qwen provider, the comfy MCP pipeline, the media agent and /image commands work in every project.
  An existing config that dogs-ai did not generate is backed up first.
#>
. (Join-Path $PSScriptRoot 'common.ps1')

Write-Step 'opencode global config'
$tplDir  = Join-Path $Repo 'opencode'
$repoFwd = $Repo -replace '\\', '/'
$text    = (Get-Content (Join-Path $tplDir 'opencode.template.json') -Raw).Replace('__REPO__', $repoFwd)
$cfg     = $text | ConvertFrom-Json

# Inline "@file:<path relative to opencode/>" prompts so the rendered config is self-contained.
foreach ($a in $cfg.agent.PSObject.Properties) {
  $p = $a.Value.prompt
  if ($p -and $p.StartsWith('@file:')) {
    $a.Value.prompt = Get-Content (Join-Path $tplDir $p.Substring(6)) -Raw
  }
}

$destDir = Join-Path $env:USERPROFILE '.config\opencode'
$dest    = Join-Path $destDir 'opencode.json'
New-Item -ItemType Directory -Force $destDir | Out-Null
if ((Test-Path $dest) -and -not (Select-String -Path $dest -Pattern '"\$dogs_ai"' -Quiet)) {
  $bak = "$dest.bak-$(Get-Date -Format yyyyMMdd-HHmmss)"
  Copy-Item $dest $bak
  Write-Warn2 "existing config backed up to $bak"
}
# UTF-8 without BOM (opencode's JSON parser rejects a BOM).
[IO.File]::WriteAllText($dest, ($cfg | ConvertTo-Json -Depth 20), (New-Object Text.UTF8Encoding $false))
Write-Ok "wrote $dest"
