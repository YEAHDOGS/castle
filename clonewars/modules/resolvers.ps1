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

function Resolve-GitHubReleaseAsset {
    <#
    .SYNOPSIS
        Resolves the concrete download asset for a "GitHubAsset" target.
        Some repos (e.g. Knulli) name assets with per-device dates and release
        codenames that do not match the release tag, so URL templates cannot
        predict the filename. Instead we match ResolverAssetRegex against the
        release's asset list, then wire up any companion checksum assets
        (.sha256/.sha512/.md5) and the SHA256 digest GitHub computes for every
        release asset.
    #>
    param (
        [hashtable]$Target,
        [string]$Version
    )

    try {
        $ApiUrl = "https://api.github.com/repos/$($Target.ResolverRepo)/releases/tags/$Version"
        $Headers = @{ "User-Agent" = "CastleVM-Provisioner" }
        $Release = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers
    }
    catch {
        Write-Host "  [FAIL] Failed to query GitHub release '$Version' for $($Target.ResolverRepo): $_" -ForegroundColor Red
        return $null
    }

    $Asset = @($Release.assets | Where-Object { $_.name -match $Target.ResolverAssetRegex }) | Select-Object -First 1
    if (-not $Asset) {
        Write-Host "  [FAIL] No asset in release '$Version' matches pattern: $($Target.ResolverAssetRegex)" -ForegroundColor Red
        return $null
    }

    $Target.Url = $Asset.browser_download_url
    $Target.IsoName = $Asset.name
    Write-Host "  [OK] Resolved release asset: $($Asset.name)" -ForegroundColor Green

    # Companion checksum files published alongside the image (e.g. Knulli)
    $CompanionMap = @{ ".sha256" = "HashUrlSha256"; ".sha512" = "HashUrlSha512"; ".md5" = "HashUrlMd5" }
    foreach ($Suffix in $CompanionMap.Keys) {
        $Companion = @($Release.assets | Where-Object { $_.name -eq "$($Asset.name)$Suffix" }) | Select-Object -First 1
        if ($Companion) {
            $Target[$CompanionMap[$Suffix]] = $Companion.browser_download_url
            Write-Host "     [+] Found companion checksum asset: $($Companion.name)" -ForegroundColor DarkGray
        }
    }

    # GitHub computes a SHA256 digest for every release asset. Pin it as an
    # extra verification layer -- for repos that publish no checksum files at
    # all (e.g. MinUI) it is the only one available.
    if ($Asset.digest -and $Asset.digest -match '^sha256:([a-fA-F0-9]{64})$') {
        $Target.ExpectedHash = $Matches[1]
        $Target.HashAlgorithm = "SHA256"
        Write-Host "     [+] Pinned GitHub API asset digest (SHA256)." -ForegroundColor DarkGray
    }

    return $Target
}

function Invoke-VersionPrompt {
    param (
        [hashtable]$Target,
        [array]$Versions
    )

    if ($Versions.Count -eq 0) {
        Write-Host "  [FAIL] No versions discovered." -ForegroundColor Red
        return $null
    }

    Write-Host ""
    Write-Host "  [?] Multiple versions discovered for $($Target.Name)" -ForegroundColor Cyan
    Write-Host "  -------------------------------------------------" -ForegroundColor DarkGray

    $DataDir = Join-Path (Split-Path $PSScriptRoot -Parent) "data"
    
    # Helper to check if disk exists for a version
    function Get-DiskStatus {
        param([string]$Ver)
        
        # 1. Determine the .qcow2 (Disk) filename
        $FileNamePattern = if ($Target.File) { 
            $Target.File -replace '\.(iso|img\.gz|zip)$', '.qcow2' 
        }
        elseif ($Target.FileTemplate) { 
            ($Target.FileTemplate -replace '\$v', $Ver) -replace '\.(iso|img\.gz|zip)$', '.qcow2' 
        }
        else { 
            "$($Target.Id).qcow2" 
        }

        # 2. Determine the .iso filename
        $IsoPattern = if ($Target.File) { 
            $Target.File -replace '\.(qcow2)$', '' 
        }
        elseif ($Target.FileTemplate) { 
            ($Target.FileTemplate -replace '\$v', $Ver) -replace '\.(qcow2)$', '.iso' 
        }
        else { 
            "$($Target.Id).iso" 
        }

        # 3. Build full absolute paths
        $DiskPath = Join-Path $DataDir $FileNamePattern
        $IsoPath = Join-Path $DataDir $IsoPattern

        # 4. Check paths and build the status string dynamically
        $Labels = ""
        if (Test-Path $IsoPath) { $Labels += " [ISO]" }
        if (Test-Path $DiskPath) { $Labels += " [DISK]" }

        return $Labels
    }

    $LatestDisk = Get-DiskStatus $Versions[0]
    Write-Host "  [0] Latest ($($Versions[0]))$LatestDisk" -ForegroundColor Green

    # Show up to 5 alternative versions
    $MaxIdx = [Math]::Min($Versions.Count - 1, 5)
    for ($i = 1; $i -le $MaxIdx; $i++) {
        $DiskStatus = Get-DiskStatus $Versions[$i]
        Write-Host "  [$i] $($Versions[$i])$DiskStatus" -ForegroundColor White
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
    elseif ($Target.ResolverType -eq "GitHub" -or $Target.ResolverType -eq "GitHubAsset") {
        $Versions = Get-GitHubReleaseVersions -Repo $Target.ResolverRepo
    }

    if ($Versions.Count -eq 0) {
        Write-Host "  [FAIL] Could not resolve any versions for $($Target.Name)." -ForegroundColor Red
        Exit 1
    }

    $SelectedVersion = Invoke-VersionPrompt -Target $Target -Versions $Versions
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

    # GitHubAsset targets resolve their download URL, checksum companions, and
    # API digest directly from the selected release's asset list
    if ($Target.ResolverType -eq "GitHubAsset") {
        $ResolvedTarget = Resolve-GitHubReleaseAsset -Target $ResolvedTarget -Version $SelectedVersion
        if (-not $ResolvedTarget) {
            Write-Host "  [FAIL] Could not resolve a release asset for $($Target.Name)." -ForegroundColor Red
            Exit 1
        }
    }

    return $ResolvedTarget
}
