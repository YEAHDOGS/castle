# ==============================================================================
# Castle VM -- Dynamic Version Discovery Engine
# ==============================================================================
# Resolves available versions of a target OS dynamically from remote sources.
# Supports GitHub Releases API and HTML Directory parsing.
# Prompts the user to select a version before yielding the final target paths.
# ==============================================================================

function Get-HtmlDirectoryVersions {
    param (
        [string]$Url,
        [string]$Regex,
        [string]$Filter
    )

    try {
        $Html = (Invoke-WebRequest $Url -UseBasicParsing).Content
        $Matches = [regex]::Matches($Html, $Regex)
        $Versions = $Matches | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique

        if (-not [string]::IsNullOrEmpty($Filter)) {
            $Versions = $Versions | Where-Object { $_ -match $Filter }
        }

        # Sort descending (latest first) with natural numeric sorting
        return @($Versions | Sort-Object { [regex]::Replace($_, '\d+', { $args[0].Value.PadLeft(10, '0') }) } -Descending)
    }
    catch {
        Write-Host "  [FAIL] Failed to scrape HTML directory: $_" -ForegroundColor Red
        return @()
    }
}

function Get-GitHubReleaseVersions {
    param (
        [string]$Repo
    )

    try {
        $ApiUrl = "https://api.github.com/repos/$Repo/releases"
        # Basic GitHub API call without auth (rate limited to 60/hr)
        $Headers = @{ "User-Agent" = "CastleVM-Provisioner" }
        $Response = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers
        
        $Versions = $Response | Where-Object { $_.prerelease -eq $false -and $_.draft -eq $false } | Select-Object -ExpandProperty tag_name
        return @($Versions)
    }
    catch {
        Write-Host "  [FAIL] Failed to query GitHub API for $Repo : $_" -ForegroundColor Red
        return @()
    }
}

function Invoke-VersionPrompt {
    param (
        [string]$TargetName,
        [array]$Versions
    )

    if ($Versions.Count -eq 0) {
        Write-Host "  [FAIL] No versions discovered." -ForegroundColor Red
        return $null
    }

    Write-Host ""
    Write-Host "  [?] Multiple versions discovered for $TargetName" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------------" -ForegroundColor DarkGray
    Write-Host "  [0] Latest ($($Versions[0]))" -ForegroundColor Green

    # Show up to 5 alternative versions
    $MaxIdx = [Math]::Min($Versions.Count - 1, 5)
    for ($i = 1; $i -le $MaxIdx; $i++) {
        Write-Host "  [$i] $($Versions[$i])" -ForegroundColor White
    }
    
    Write-Host ""
    $InputRaw = Read-Host "  Select a version number [Default: 0]"

    if ([string]::IsNullOrWhiteSpace($InputRaw)) {
        return $Versions[0]
    }

    $SelectedIdx = $InputRaw -as [int]
    if ($SelectedIdx -ne $null -and $SelectedIdx -ge 0 -and $SelectedIdx -le $MaxIdx) {
        return $Versions[$SelectedIdx]
    }
    else {
        Write-Host "  [FAIL] Invalid selection. Defaulting to Latest." -ForegroundColor Yellow
        return $Versions[0]
    }
}

function Resolve-TargetVersion {
    <#
    .SYNOPSIS
        Takes a Target template, dynamically discovers versions, prompts the user,
        and returns a fully instantiated Target hashtable.
    #>
    param (
        [hashtable]$Target
    )

    # If it's a static target (no ResolverType), just return it as-is
    if ([string]::IsNullOrEmpty($Target.ResolverType)) {
        return $Target
    }

    Write-Host "  [>] Discovering available versions for $($Target.Name)..." -ForegroundColor Cyan
    $Versions = @()

    if ($Target.ResolverType -eq "HtmlDirectory") {
        $Versions = Get-HtmlDirectoryVersions `
            -Url $Target.ResolverUrl `
            -Regex $Target.ResolverRegex `
            -Filter $Target.ResolverFilter
    }
    elseif ($Target.ResolverType -eq "GitHub") {
        $Versions = Get-GitHubReleaseVersions -Repo $Target.ResolverRepo
    }

    if ($Versions.Count -eq 0) {
        Write-Host "  [FAIL] Could not resolve any versions for $($Target.Name)." -ForegroundColor Red
        Exit 1
    }

    $SelectedVersion = Invoke-VersionPrompt -TargetName $Target.Name -Versions $Versions
    Write-Host "  [OK] Selected version: $SelectedVersion" -ForegroundColor Green

    # Clone the target to avoid modifying the static manifest structure permanently
    $ResolvedTarget = $Target.Clone()

    # Process all keys ending in 'Template'
    $TemplateKeys = @($ResolvedTarget.Keys) | Where-Object { $_ -like "*Template" }
    
    foreach ($Key in $TemplateKeys) {
        $BaseKey = $Key -replace "Template$", ""
        $TemplateString = $ResolvedTarget[$Key]
        
        # Replace $v with the selected version
        $ResolvedString = $TemplateString -replace '\$v', $SelectedVersion
        
        # Add the resolved string as the base property (e.g. UrlTemplate -> Url)
        $ResolvedTarget[$BaseKey] = $ResolvedString
    }

    return $ResolvedTarget
}
