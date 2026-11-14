# ==============================================================================
# LAUNCH PROFILES
# ==============================================================================
# A profile is one file that says what a VM is for: which target to boot, how
# big it is, what to put on it, what to install and what to run. It replaces a
# long command line plus a hand-written setup script:
#
#   .\start.ps1 -Profile penguin          # profiles/penguin.psd1
#   .\start.ps1 -Profile C:\path\my.psd1  # any path
#   .\start.ps1 -Profile penguin -Target endeavouros   # command line wins
#
# Profiles are PowerShell data files (.psd1): plain data, no code runs when one
# is loaded. Every key is optional.
#
#   @{
#       Description  = "What this VM is for"
#       Target       = "debian-x11"        # target id from modules/targets.ps1
#       Instance     = "job-01"            # own disk under app/data/clonewars/instances
#       Background   = $true               # windowed QEMU, return to the shell
#       MemoryGB     = 4                   # exact guest RAM
#       Cores        = 4                   # exact guest CPU count
#       Screen       = "1440x900"          # guest screen mode (X11 guests)
#       QemuArgs     = @("-rtc", "base=localtime")   # extra raw QEMU arguments
#       Env          = @{ CASTLE_VM_AUDIO = "none" } # CASTLE_* knobs for this launch
#
#       # Host folders packed into app/data/clonewars/copy before boot. A git checkout is
#       # packed as git sees it (tracked + untracked, ignored files left out);
#       # Include adds ignored files by name (a .env, say).
#       Payload      = @(
#           @{ Source = "..\..\..\..\my-project"; As = "my-project.zip"; Include = @(".env") }
#       )
#
#       # Installed before anything runs; the list for the guest's own package
#       # manager is used.
#       Packages     = @{ apt = @("python3-venv"); pacman = @("python") }
#       RootCommands = @("timedatectl set-timezone UTC")          # as root, in order
#       UserCommands = @("xterm -e htop")                         # as the desktop user, in the X11 session
#
#       Setup        = "penguin"           # then scripts/setup-penguin.sh; default: setup-profile.sh
#   }
#
# Packages and commands travel to the guest as /tmp/castle-profile.env, read by
# castle_profile_apply in scripts/castle-lib.sh.

function Resolve-CastleProfilePath {
    param ([string]$Name, [string]$Root)
    if ([string]::IsNullOrWhiteSpace($Name)) { return $null }
    foreach ($Candidate in @($Name, "$Name.psd1", (Join-Path (Join-Path $Root "profiles") $Name), (Join-Path (Join-Path $Root "profiles") "$Name.psd1"))) {
        if (Test-Path -LiteralPath $Candidate -PathType Leaf) { return (Get-Item -LiteralPath $Candidate).FullName }
    }
    return $null
}


function Get-CastleProfiles {
    <# Names and descriptions of the profiles under profiles/. #>
    param ([string]$Root)
    $Dir = Join-Path $Root "profiles"
    if (-not (Test-Path $Dir)) { return @() }
    foreach ($File in Get-ChildItem $Dir -Filter "*.psd1" -File | Sort-Object Name) {
        $Data = try { Import-PowerShellDataFile -LiteralPath $File.FullName } catch { @{ Description = "(unreadable: $($_.Exception.Message))" } }
        [pscustomobject]@{ Name = $File.BaseName; Target = $Data.Target; Description = $Data.Description; Path = $File.FullName }
    }
}


function Import-CastleProfile {
    <#
    .SYNOPSIS
        Loads a profile by name or path and checks its keys.
    .OUTPUTS
        The profile hashtable with Path and Dir added, or $null (after printing why).
    #>
    param ([string]$Name, [string]$Root)

    $Path = Resolve-CastleProfilePath -Name $Name -Root $Root
    if (-not $Path) {
        Write-Host "  [FAIL] Profile '$Name' not found (looked for a file and for profiles\$Name.psd1)." -ForegroundColor Red
        $Known = @(Get-CastleProfiles -Root $Root | ForEach-Object { $_.Name })
        if ($Known.Count -gt 0) { Write-Host "  [i] Profiles: $($Known -join ', ')" -ForegroundColor DarkGray }
        return $null
    }
    try { $Data = Import-PowerShellDataFile -LiteralPath $Path }
    catch {
        Write-Host "  [FAIL] Profile $Path is not a valid .psd1 data file: $($_.Exception.Message)" -ForegroundColor Red
        return $null
    }
    $Allowed = @("Description", "Target", "Instance", "Background", "MemoryGB", "Cores", "Screen", "QemuArgs", "Env",
        "Payload", "Packages", "RootCommands", "UserCommands", "Setup", "Mirror")
    $Unknown = @($Data.Keys | Where-Object { $_ -notin $Allowed })
    if ($Unknown.Count -gt 0) {
        Write-Host "  [FAIL] Profile $Path has unknown key(s): $($Unknown -join ', '). Allowed: $($Allowed -join ', ')" -ForegroundColor Red
        return $null
    }
    if ($Data.Screen -and $Data.Screen -notmatch '^\d{3,5}x\d{3,5}$') {
        Write-Host "  [FAIL] Profile Screen must look like 1440x900, not '$($Data.Screen)'." -ForegroundColor Red
        return $null
    }
    foreach ($Key in @($Data.Env.Keys | Where-Object { $_ })) {
        if ($Key -notlike "CASTLE_*") {
            Write-Host "  [FAIL] Profile Env may only set CASTLE_* variables, not '$Key'." -ForegroundColor Red
            return $null
        }
    }
    $Data.Path = $Path
    $Data.Dir = Split-Path $Path -Parent
    return $Data
}


function New-CastlePayloadZip {
    <#
    .SYNOPSIS
        Packs a host folder into a zip for the copy payload. A git checkout is
        packed as git sees it (tracked and untracked files, ignored ones left
        out), so virtualenvs, build output and secrets stay behind unless
        named in -Include. Any other folder is packed whole.
    .OUTPUTS
        Number of files packed.
    #>
    param (
        [string]$Source,
        [string]$Destination,
        [string[]]$Include = @()
    )

    $Root = (Get-Item -LiteralPath $Source).FullName.TrimEnd('\', '/')
    $Names = $null
    if ((Test-Path (Join-Path $Root ".git")) -and (Get-Command git -ErrorAction SilentlyContinue)) {
        # Windowless: from the Castle box a plain & git would flash a console.
        $Git = Invoke-Quiet git @("-C", $Root, "ls-files", "-co", "--exclude-standard")
        if ($Git.ExitCode -eq 0) { $Names = $Git.Lines }
    }
    if ($null -eq $Names) {
        $Names = @(Get-ChildItem -LiteralPath $Root -Recurse -File | ForEach-Object { $_.FullName.Substring($Root.Length + 1) -replace '\\', '/' })
    }
    $Names = @(@($Names) + @($Include | Where-Object { $_ }) | ForEach-Object { $_ -replace '\\', '/' } | Sort-Object -Unique)

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $Temp = "$Destination.tmp"
    if (Test-Path $Temp) { Remove-Item $Temp -Force }
    $Count = 0
    $Zip = [System.IO.Compression.ZipFile]::Open($Temp, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($Name in $Names) {
            $File = Join-Path $Root $Name
            if (-not (Test-Path -LiteralPath $File -PathType Leaf)) { continue }
            # Entry names use forward slashes: the guest unpacks them on Linux.
            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($Zip, $File, $Name, [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
            $Count++
        }
    }
    finally { $Zip.Dispose() }
    Move-Item $Temp $Destination -Force
    return $Count
}


function Update-CastleProfilePayload {
    <# Builds every Payload entry of a profile into the copy folder. Returns $false on failure. #>
    param ([hashtable]$Profile, [string]$CopyDir)

    foreach ($Item in @($Profile.Payload)) {
        if (-not $Item) { continue }
        if (-not $Item.Source) {
            Write-Host "  [FAIL] Profile Payload entry needs a Source folder." -ForegroundColor Red
            return $false
        }
        $Source = if ([System.IO.Path]::IsPathRooted($Item.Source)) { $Item.Source } else { Join-Path $Profile.Dir $Item.Source }
        if (-not (Test-Path -LiteralPath $Source -PathType Container)) {
            Write-Host "  [FAIL] Profile payload folder not found: $Source" -ForegroundColor Red
            return $false
        }
        $As = if ($Item.As) { $Item.As } else { "$((Get-Item -LiteralPath $Source).Name).zip" }
        if ($As -match '[\\/]' -or $As -notlike "*.zip") {
            Write-Host "  [FAIL] Profile Payload 'As' must be a plain .zip file name, not '$As'." -ForegroundColor Red
            return $false
        }
        if (-not (Test-Path $CopyDir)) { New-Item -ItemType Directory -Path $CopyDir -Force | Out-Null }
        $Destination = Join-Path $CopyDir $As
        $Count = New-CastlePayloadZip -Source $Source -Destination $Destination -Include @($Item.Include)
        Write-Host ("  [PROFILE] Packed {0} file(s) from {1} -> app\data\clonewars\copy\{2} ({3:N1} MB)" -f $Count, (Get-Item -LiteralPath $Source).FullName, $As, ((Get-Item $Destination).Length / 1MB)) -ForegroundColor Cyan
    }
    return $true
}


function ConvertTo-BashArray {
    param ([string]$Name, [object[]]$Items)
    $Quoted = @($Items | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | ForEach-Object { "'" + (([string]$_) -replace "'", "'\''") + "'" })
    return "$Name=($($Quoted -join ' '))"
}


function Write-CastleProfileEnv {
    <#
    .SYNOPSIS
        Writes the guest half of a profile (packages, commands, screen) as a
        bash file that castle_profile_apply sources. LF line endings.
    .OUTPUTS
        Path of the file.
    #>
    param ([hashtable]$Profile, [string]$Path)

    $Lines = @(
        "# Castle launch profile: $([System.IO.Path]::GetFileNameWithoutExtension($Profile.Path)) (generated by start.ps1, do not edit)"
        (ConvertTo-BashArray -Name "CASTLE_PKGS_APT" -Items @($Profile.Packages.apt))
        (ConvertTo-BashArray -Name "CASTLE_PKGS_PACMAN" -Items @($Profile.Packages.pacman))
        (ConvertTo-BashArray -Name "CASTLE_ROOT_COMMANDS" -Items @($Profile.RootCommands))
        (ConvertTo-BashArray -Name "CASTLE_USER_COMMANDS" -Items @($Profile.UserCommands))
    )
    if ($Profile.Screen) { $Lines += "CASTLE_SCREEN='$($Profile.Screen)'" }
    $Dir = Split-Path $Path -Parent
    if (-not (Test-Path $Dir)) { New-Item -ItemType Directory -Path $Dir -Force | Out-Null }
    [System.IO.File]::WriteAllText($Path, (($Lines -join "`n") + "`n"), [System.Text.UTF8Encoding]::new($false))
    return $Path
}
