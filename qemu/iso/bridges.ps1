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
        [string]$IsoName,
        [object]$HttpClient
    )

    Write-Host "🔒 Running integrity audit via $Algorithm validation..." -ForegroundColor Yellow
    
    if (-not [string]::IsNullOrEmpty($HashUrl)) {
        try {
            Write-Host "📡 Querying remote checksum manifest..." -ForegroundColor DarkGray
            $FetchedData = $HttpClient.GetStringAsync($HashUrl).GetAwaiter().GetResult()
            
            if (-not [string]::IsNullOrEmpty($IsoName)) {
                # 1. Multi-line manifest parsing (Line must contain our specific ISO target name)
                $EscapedIsoName = [regex]::Escape($IsoName)
        
                # --- DUAL FORMAT PATTERN ---
                # Pattern 1 (Standard Linux): Matches hash at start, followed by whitespace/asterisk and filename
                # Pattern 2 (BSD / RHEL Style): Matches Algorithm(filename) = hash
                $Pattern = "(?mi)(?:^([a-fA-F0-9]+)\s+\*?${EscapedIsoName}\s*$|^\w+\s*\(${EscapedIsoName}\)\s*=\s*([a-fA-F0-9]+)\s*$)"
        
                if ($FetchedData -match $Pattern) {
                    # Since we have two capturing groups in an OR condition, 
                    # $Matches[1] captures the standard format, $Matches[2] captures the BSD style.
                    $ExpectedHash = if (![string]::IsNullOrEmpty($Matches[1])) { $Matches[1] } else { $Matches[2] }
                }
                else {
                    Write-Host "⚠️ Targeted file string '$IsoName' not listed within retrieved network manifest." -ForegroundColor Yellow
                }
            }
            else {
                # 2. Fallback for single-line raw checksum streams (.sha256 / .sha512 files)
                # Broadened regex range to cleanly capture MD5 (32), SHA1 (40), SHA256 (64), or SHA512 (128)
                if ($FetchedData -match "([a-fA-F0-9]{32,128})") {
                    $ExpectedHash = $Matches[1]
                }
            }
        }
        catch {
            Write-Host "⚠️ Remote hash lookup failed. Falling back to local data map attributes." -ForegroundColor Yellow
        }
    }

    if ([string]::IsNullOrEmpty($ExpectedHash)) {
        Write-Host "❌ Missing cryptographic hash target signature. Verification aborted." -ForegroundColor Red
        return $false
    }

    # Verify target payload exists locally before calling Get-FileHash
    if (-not (Test-Path $FilePath)) {
        Write-Host "❌ Target path local payload storage node not found: $FilePath" -ForegroundColor Red
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
        Write-Host "⚠️ GPG executable not detected on environment paths." -ForegroundColor Orange
        Write-Host "If you have Git installed, your PATH variable or your $PROFILE must point to the gpg.exe in the Git\usr\bin folder. Skipping layer..." -ForegroundColor Orange
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