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
        Write-EndpointInfo -Url $Url
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
        Write-EndpointInfo -Url $ApiUrl
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
        Write-EndpointInfo -Url $ApiUrl
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

function Get-MirrorHost {
    param ([hashtable]$Mirror)
    try { return ([System.Uri]$Mirror.Base).Host } catch { return $Mirror.Base }
}

function Get-TargetMirrors {
    <#
    .SYNOPSIS
        The mirror list for a target: scraped live from MirrorPageUrl when the
        target defines one (MirrorRowRegex with named groups country/name/url,
        optionally grouped by MirrorSectionRegex with a region group), falling
        back to the static Mirrors snapshot in the manifest when the page is
        unreachable or yields nothing. A mirror's Base is the ISO link's
        directory: the checksum and signature live next to the ISO, so one
        base serves every download of that target.
    .OUTPUTS
        Array of @{ Region; Country; Name; Base; Source = "live"|"snapshot" }.
    #>
    param ([hashtable]$Target)

    $Live = @()
    if ($Target.MirrorPageUrl -and $Target.MirrorRowRegex) {
        try {
            Write-Host "  [>] Scraping mirror list from $($Target.MirrorPageUrl)..." -ForegroundColor Cyan
            Write-EndpointInfo -Url $Target.MirrorPageUrl
            $Html = (Invoke-WebRequest $Target.MirrorPageUrl -UseBasicParsing -Headers @{ "User-Agent" = "CastleVM-Provisioner" }).Content
            $Sections = if ($Target.MirrorSectionRegex) {
                [regex]::Matches($Html, $Target.MirrorSectionRegex) | ForEach-Object { @{ Region = $_.Groups["region"].Value.Trim(); Html = $_.Value } }
            }
            else { @(@{ Region = ""; Html = $Html }) }

            foreach ($Section in $Sections) {
                foreach ($Row in [regex]::Matches($Section.Html, $Target.MirrorRowRegex)) {
                    $Url = $Row.Groups["url"].Value
                    if (-not $Url) { continue }
                    $Region = if ($Row.Groups["region"].Success) { $Row.Groups["region"].Value.Trim() } else { $Section.Region }
                    $Live += @{
                        Region  = $Region
                        Country = $Row.Groups["country"].Value.Trim()
                        Name    = $Row.Groups["name"].Value.Trim()
                        Base    = $Url.Substring(0, $Url.LastIndexOf('/'))
                        Source  = "live"
                    }
                }
            }
        }
        catch {
            Write-Host "  [?] Mirror page fetch failed: $($_.Exception.Message)" -ForegroundColor Yellow
        }
    }

    if ($Live.Count -gt 0) {
        Write-Host "  [OK] $($Live.Count) mirrors scraped live." -ForegroundColor Green
        return $Live
    }

    $Snapshot = @($Target.Mirrors | ForEach-Object { $m = $_.Clone(); $m.Source = "snapshot"; $m })
    if ($Snapshot.Count -gt 0) {
        Write-Host "  [?] Using the manifest's mirror snapshot ($($Snapshot.Count) mirrors)." -ForegroundColor Yellow
    }
    return $Snapshot
}

function Select-TargetMirror {
    <#
    .SYNOPSIS
        Picks one mirror. -Mirror (or CASTLE_MIRROR) selects without a prompt:
        a list number, or a substring of the host / name / country ("tuna",
        "germany", "gigenet"). Otherwise the list is shown grouped by region and
        Enter keeps the target's default mirror (MirrorDefault host, else the
        first entry), so scripted runs behave exactly as before.
    #>
    param (
        [hashtable]$Target,
        [array]$Mirrors,
        [string]$Mirror
    )

    if ($Mirrors.Count -eq 0) { return $null }

    # Default: MirrorDefault host if present in the list, else first entry.
    $DefaultIdx = 0
    if ($Target.MirrorDefault) {
        for ($i = 0; $i -lt $Mirrors.Count; $i++) {
            if ((Get-MirrorHost $Mirrors[$i]) -eq $Target.MirrorDefault) { $DefaultIdx = $i; break }
        }
    }

    $Choice = if ($Mirror) { $Mirror } elseif ($env:CASTLE_MIRROR) { $env:CASTLE_MIRROR } else { $null }

    if (-not $Choice) {
        Write-Host ""
        Write-Host "  [?] Mirrors for $($Target.Name) (Enter = default, or type a number / host / country)" -ForegroundColor Cyan
        Write-Host "  -------------------------------------------------" -ForegroundColor DarkGray
        $LastRegion = $null
        for ($i = 0; $i -lt $Mirrors.Count; $i++) {
            $m = $Mirrors[$i]
            if ($m.Region -ne $LastRegion) {
                Write-Host "  $($m.Region)" -ForegroundColor White
                $LastRegion = $m.Region
            }
            $Label = "  [{0,2}] {1,-16} {2,-18} {3}" -f ($i + 1), $m.Country, $m.Name, (Get-MirrorHost $m)
            if ($i -eq $DefaultIdx) { Write-Host "$Label  (default)" -ForegroundColor Green } else { Write-Host $Label -ForegroundColor Gray }
        }
        Write-Host ""
        $Choice = Read-Host "  Select a mirror [Default: $($DefaultIdx + 1)]"
    }

    if ([string]::IsNullOrWhiteSpace($Choice) -or $Choice -eq "default") { return $Mirrors[$DefaultIdx] }

    $Idx = $Choice -as [int]
    if ($Idx -ne $null -and $Idx -ge 1 -and $Idx -le $Mirrors.Count) { return $Mirrors[$Idx - 1] }

    $Needle = $Choice.Trim().ToLower()
    $Hit = @($Mirrors | Where-Object {
        (Get-MirrorHost $_).ToLower().Contains($Needle) -or
        ([string]$_.Name).ToLower().Contains($Needle) -or
        ([string]$_.Country).ToLower().Contains($Needle)
    }) | Select-Object -First 1
    if ($Hit) { return $Hit }

    Write-Host "  [FAIL] No mirror matches '$Choice'. Using the default." -ForegroundColor Yellow
    return $Mirrors[$DefaultIdx]
}

function Resolve-TargetMirror {
    <#
    .SYNOPSIS
        For targets with a mirror list, chooses a mirror and substitutes its
        base for `$m` in every string value of the target (ResolverUrl, the
        Url/HashUrl*/SigUrl templates, ...). Targets without mirrors pass
        through untouched. Returns a clone; the manifest is never modified.
    #>
    param (
        [hashtable]$Target,
        [string]$Mirror,
        [switch]$NoPrompt
    )

    if (-not $Target.Mirrors -and -not $Target.MirrorPageUrl) { return $Target }

    $Mirrors = if ($NoPrompt -and -not $Mirror -and -not $env:CASTLE_MIRROR) {
        @($Target.Mirrors | ForEach-Object { $m = $_.Clone(); $m.Source = "snapshot"; $m })
    }
    else { @(Get-TargetMirrors -Target $Target) }

    $Chosen = if ($NoPrompt -and -not $Mirror -and -not $env:CASTLE_MIRROR) {
        Select-TargetMirror -Target $Target -Mirrors $Mirrors -Mirror "default"
    }
    else { Select-TargetMirror -Target $Target -Mirrors $Mirrors -Mirror $Mirror }

    if (-not $Chosen) {
        Write-Host "  [FAIL] No mirrors available for $($Target.Name)." -ForegroundColor Red
        Exit 1
    }

    $Base = ([string]$Chosen.Base).TrimEnd('/')
    Write-Host "  [OK] Mirror: $($Chosen.Name) ($($Chosen.Country)) -> $Base" -ForegroundColor Green

    $Resolved = $Target.Clone()
    foreach ($Key in @($Resolved.Keys)) {
        if ($Resolved[$Key] -is [string] -and $Resolved[$Key].Contains('$m')) {
            $Resolved[$Key] = $Resolved[$Key] -replace '\$m', $Base
        }
    }
    $Resolved.Mirror = $Chosen
    return $Resolved
}

function Resolve-TargetVersion {
    <#
    .SYNOPSIS
        Takes a Target template, picks a mirror when the target has several,
        dynamically discovers versions, prompts the user, and returns a fully
        instantiated Target hashtable.
    .PARAMETER Mirror
        Mirror selection without a prompt (number, host, name or country).
    .PARAMETER NoMirrorPrompt
        Use the default mirror silently (cleanup paths, where the mirror only
        matters for listing versions).
    #>
    param (
        [hashtable]$Target,
        [string]$Mirror,
        [switch]$NoMirrorPrompt
    )

    # Mirror first: the version listing and every download then come from it.
    $Target = Resolve-TargetMirror -Target $Target -Mirror $Mirror -NoPrompt:$NoMirrorPrompt

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
