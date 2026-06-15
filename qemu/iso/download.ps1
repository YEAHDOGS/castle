<#
.SYNOPSIS
    Multi-Matrix Asset Download Orchestrator
.DESCRIPTION
    Decoupled utility parsing manifest structures and executing secure streaming routines.
.PARAMETER Iso
    Filter down to a specific target image by name (e.g., -i cachy, -iso windows11, -i mac)
#>
param (
    [Parameter(Mandatory = $false, Position = 0)]
    [Alias("i")]
    [String]$Iso
)

# Dot-source companion configuration arrays and processing methods
. "$PSScriptRoot\targets.ps1"
. "$PSScriptRoot\bridges.ps1"

$ProcessingQueue = $IsoMatrix
if (-not [String]::IsNullOrWhiteSpace($Iso)) {
    $CleanInput = $Iso.Trim().ToLower().Replace(" ", "")
    $ProcessingQueue = $IsoMatrix | Where-Object { 
        $_.Id -like "*$CleanInput*" -or $_.Name.ToLower().Replace(" ", "") -like "*$CleanInput*" 
    }

    if ($ProcessingQueue.Count -eq 0) {
        Write-Host "❌ Identity matrix filtering failed. Unknown token string input: '$Iso'" -ForegroundColor Red
        Write-Host "💡 Available options: (($IsoMatrix | ForEach-Object { $_.Id }) -join ', ')" -ForegroundColor DarkGray
        Exit
    }
}

$HttpClient = [System.Net.Http.HttpClient]::new()

foreach ($Target in $ProcessingQueue) {
    Write-Host "`n========================================================================" -ForegroundColor DarkGray
    Write-Host "🚀 Pipeline Target Initialized: $($Target.Name)" -ForegroundColor Magenta
    Write-Host "========================================================================" -ForegroundColor DarkGray

    $FileName = $Target.File
    
    # Resolves perfectly every single time, no matter your working directory
    $Path = Join-Path $PSScriptRoot "..\data\$FileName"
    $LocalFile = [System.IO.Path]::GetFullPath($Path)

    
    if (Test-Path $LocalFile) {
        Write-Host "📦 Asset exists on local target space storage path: $LocalFile" -ForegroundColor Yellow
    }
    else {
        # Ensure the destination data directory actually exists before downloading
        $TargetDir = Split-Path $LocalFile -Parent
        if (-not (Test-Path $TargetDir)) {
            New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
        }
        
        $DownloadSuccess = Start-NetworkStream -Url $Target.Url -Path $LocalFile -HttpClient $HttpClient
        if (-not $DownloadSuccess) { continue }
    }
    
    # 1. Determine dynamic hash vs static profile hash
    $HashMatch = $false
    
    if ($Target.ExpectedHash) {
        # Profile has an explicit hardcoded target hash (Windows / Mac Shim)
        $Algo = if ($Target.HashAlgorithm) { $Target.HashAlgorithm } else { "SHA512" }
        Write-Host "🔍 Executing static profile hash verification ($Algo)..." -ForegroundColor Cyan
        $HashMatch = Test-CryptographicHash -FilePath $LocalFile -ExpectedHash $Target.ExpectedHash -Algorithm $Algo -HttpClient $HttpClient
    }
    else {
        # FORCE the output into a true array of strings using @(...)
        $SelectedKeys = @($Target.Keys)
        | Where-Object { $_ -like "HashUrl*" }

        # Ensure we check .Count on a guaranteed array structure
        if ($SelectedKeys.Count -gt 0) {
            $HashMatch = $true

            foreach ($Key in $SelectedKeys) {
                # Ensure $Key is treated strictly as a full string identifier
                $KeyString = [string]$Key
                
                # Strip "HashUrl" off the front to extract the algorithm (e.g., "Sha512")
                $DynamicAlgo = $KeyString -replace "^HashUrl", ""
                $RemoteHashUrl = $Target[$KeyString]
                $MyIsoName = $Target.IsoName

                Write-Host "🌐 Fetching remote manifest ($DynamicAlgo) via: $RemoteHashUrl" -ForegroundColor Cyan
                
                # Run verification using the dynamically discovered URL and algorithm
                $CurrentMatch = Test-CryptographicHash -FilePath $LocalFile -HashUrl $RemoteHashUrl -Algorithm $DynamicAlgo -IsoName $MyIsoName -HttpClient $HttpClient
                
                # Accumulate the pass/fail state
                $HashMatch = $HashMatch -and $CurrentMatch

                if (-not $CurrentMatch) {
                    Write-Host "❌ Validation failed for algorithm: $DynamicAlgo" -ForegroundColor Red
                    break 
                }
            }
        }
        else {
            Write-Host "❌ Error: Target contains no cryptographic baseline profile definition." -ForegroundColor Red
            continue
        }
    }
}

# 2. Sequence Verification and GPG execution
if ($HashMatch) {
    # Only execute GPG signatures for targets that actually define them (Linux targets)
    if ($Target.SigUrl) {
        Write-Host "🔐 Verifying detached GPG cryptographic armor..." -ForegroundColor Cyan
        $GpgMatch = Test-GpgSignature -TargetFile $LocalFile -SigUrl $Target.SigUrl -GpgKey $Target.GpgKey -GpgServer $Target.GpgServer -HttpClient $HttpClient
            
        if (-not $GpgMatch) {
            Write-Host "⚠️ Signature structure unverified. Not deleting target asset payload space." -ForegroundColor Red
            # Remove-Item $LocalFile -Force
            continue
        }
    }
        
    Write-Host "🎯 Asset validation successful. Image deployment finalized." -ForegroundColor Green
}
else {
    Write-Host "⚠️ Hash integrity mismatch. Not deleting corrupted filesystem fragments." -ForegroundColor Red
    # Remove-Item $LocalFile -Force
}


$HttpClient.Dispose()
Write-Host "`n🏁 Processing routine pipeline jobs concluded." -ForegroundColor Cyan