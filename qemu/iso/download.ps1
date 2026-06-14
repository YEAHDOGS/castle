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

    $LocalFile = $Target.File
    $Algo = if ($Target.HashAlgorithm) { $Target.HashAlgorithm } else { "SHA512" }

    if (Test-Path $LocalFile) {
        Write-Host "📦 Asset exists on local target space storage path: $LocalFile" -ForegroundColor Yellow
    }
    else {
        $DownloadSuccess = Start-NetworkStream -Url $Target.Url -Path $LocalFile -HttpClient $HttpClient
        if (-not $DownloadSuccess) { continue }
    }

    # Sequence Cryptographic Verification Loop
    $HashMatch = Test-CryptographicHash -FilePath $LocalFile -ExpectedHash $Target.ExpectedHash -HashUrl $Target.HashUrl -Algorithm $Algo -HttpClient $HttpClient
    
    if ($HashMatch) {
        $GpgMatch = Test-GpgSignature -TargetFile $LocalFile -SigUrl $Target.SigUrl -GpgKey $Target.GpgKey -GpgServer $Target.GpgServer -HttpClient $HttpClient
        if (-not $GpgMatch) {
            Write-Host "⚠️ Signature structure unverified. Deleting target asset payload space." -ForegroundColor Red
            Remove-Item $LocalFile -Force
        }
        else {
            Write-Host "🎯 Asset validation successful. Image deployment finalized." -ForegroundColor Green
        }
    }
    else {
        Write-Host "⚠️ Hash integrity mismatch. Deleting corrupted filesystem fragments." -ForegroundColor Red
        Remove-Item $LocalFile -Force
    }
}

$HttpClient.Dispose()
Write-Host "`n🏁 Processing routine pipeline jobs concluded." -ForegroundColor Cyan