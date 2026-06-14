# ==============================================================================
# UTILITY STORAGE & VERIFICATION ENGINE
# ==============================================================================

function Start-NetworkStream {
    param (
        [string]$Url,
        [string]$Path,
        [object]$HttpClient
    )

    Write-Host "📡 Opening network pipeline to: $Url" -ForegroundColor Cyan
    try {
        $ResponseTask = $HttpClient.GetAsync($Url, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead)
        $Response = $ResponseTask.GetAwaiter().GetResult()
        
        if (-not $Response.IsSuccessStatusCode) {
            throw "Target endpoint connection failure. Status Code: $($Response.StatusCode)"
        }

        $TotalBytes = $Response.Content.Headers.ContentLength
        $ReadableSize = if ($TotalBytes) { "{0:N2} GB" -f ($TotalBytes / 1GB) } else { "Variable Stream" }
        Write-Host "⚡ Payload Size Detected: $ReadableSize. Fetching bytes..." -ForegroundColor Green

        $DownloadStream = $Response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $FileStream = [System.IO.FileStream]::new($Path, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
        $Buffer = [System.Byte[]]::new(131072) # 128KB I/O buffer blocks
        $BytesRead = 0
        $TotalBytesRead = 0
        
        while (($BytesRead = $DownloadStream.Read($Buffer, 0, $Buffer.Length)) -gt 0) {
            $FileStream.Write($Buffer, 0, $BytesRead)
            $TotalBytesRead += $BytesRead
            
            if ($TotalBytesRead % 52428800 -lt 131073) {
                $Percent = if ($TotalBytes) { "{0:P0}" -f ($TotalBytesRead / $TotalBytes) } else { "Streaming" }
                Write-Host "   -> Disk Write: $Percent ($("{0:N2}" -f ($TotalBytesRead / 1GB)) GB)" -ForegroundColor Gray
            }
        }

        $FileStream.Flush()
        $FileStream.Close()
        $DownloadStream.Close()
        Write-Host "🎯 I/O buffer closed and finalized: $Path" -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "❌ Fatal error processing network stream: $_" -ForegroundColor Red
        if ($FileStream) { $FileStream.Close() }
        if (Test-Path $Path) { Remove-Item $Path -Force }
        return $false
    }
}

function Test-CryptographicHash {
    param (
        [string]$FilePath,
        [string]$ExpectedHash,
        [string]$HashUrl,
        [string]$Algorithm = "SHA512",
        [object]$HttpClient
    )

    Write-Host "🔒 Running integrity audit via $Algorithm validation..." -ForegroundColor Yellow
    
    if (-not [string]::IsNullOrEmpty($HashUrl)) {
        try {
            Write-Host "📡 Querying remote checksum manifest..." -ForegroundColor DarkGray
            $FetchedData = $HttpClient.GetStringAsync($HashUrl).GetAwaiter().GetResult()
            if ($FetchedData -match "([a-fA-F0-9]{64,128})") {
                $ExpectedHash = $Matches[1]
            }
        }
        catch {
            Write-Host "⚠️ Remote hash lookup failed. Falling back to local data map attributes." -ForegroundColor Orange
        }
    }

    if ([string]::IsNullOrEmpty($ExpectedHash)) {
        Write-Host "❌ Missing cryptographic hash target signature. Verification aborted." -ForegroundColor Red
        return $false
    }

    $ComputedHash = (Get-FileHash -Path $FilePath -Algorithm $Algorithm).Hash.ToLower().Trim()
    $TargetCleanHash = $ExpectedHash.ToLower().Trim()

    if ($ComputedHash -eq $TargetCleanHash) {
        Write-Host "✅ Checksum validation succeeded! ($Algorithm : $ComputedHash)" -ForegroundColor Green
        return $true
    }
    else {
        Write-Host "❌ CRITICAL: Cryptographic payload mismatch detected!" -ForegroundColor Red
        Write-Host "   -> Computed: $ComputedHash" -ForegroundColor Red
        Write-Host "   -> Expected: $TargetCleanHash" -ForegroundColor Yellow
        return $false
    }
}

function Test-GpgSignature {
    param (
        [string]$TargetFile,
        [string]$SigUrl,
        [string]$GpgKey,
        [string]$GpgServer,
        [object]$HttpClient
    )

    if (-not (Get-Command gpg -ErrorAction SilentlyContinue)) {
        Write-Host "⚠️ GPG executable not detected on environment paths. Skipping layer..." -ForegroundColor Orange
        return $true
    }

    if ([string]::IsNullOrEmpty($SigUrl) -or [string]::IsNullOrEmpty($GpgKey)) {
        Write-Host "ℹ️ No validation signature targets linked to this profile context." -ForegroundColor DarkGray
        return $true
    }

    $SigFile = "$TargetFile.sig"
    Write-Host "🛡️ Evaluating GPG trust token authenticity..." -ForegroundColor Yellow

    try {
        Write-Host "📡 Syncing verification fingerprint [$GpgKey] from keyserver: $GpgServer" -ForegroundColor DarkGray
        gpg --keyserver $GpgServer --recv-keys $GpgKey 2>$null

        $SigBytes = $HttpClient.GetByteArrayAsync($SigUrl).GetAwaiter().GetResult()
        [System.IO.File]::WriteAllBytes($SigFile, $SigBytes)

        $GpgAudit = gpg --verify $SigFile $TargetFile 2>&1
        Remove-Item $SigFile -Force

        if ($GpgAudit -match "Good signature") {
            Write-Host "✅ Authentic cryptographic signature confirmed." -ForegroundColor Green
            return $true
        }
        else {
            Write-Host "❌ CRITICAL FAILURE: GPG verification engine rejected signature!" -ForegroundColor Red
            Write-Host $GpgAudit -ForegroundColor DarkGray
            return $false
        }
    }
    catch {
        Write-Host "❌ Error encountered executing GPG engine validation: $_" -ForegroundColor Red
        if (Test-Path $SigFile) { Remove-Item $SigFile -Force }
        return $false
    }
}