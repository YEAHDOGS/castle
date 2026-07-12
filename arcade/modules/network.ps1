# ==============================================================================
# Arcade Network Download Library Module
# ==============================================================================

function Start-DownloadStream {
    param (
        [Parameter(Mandatory=$true)]
        [string]$Url,
        [Parameter(Mandatory=$true)]
        [string]$Path
    )

    Write-Host "  [NETWORK] Starting secure download stream from: $Url" -ForegroundColor Cyan

    $ParentDir = Split-Path $Path -Parent
    if (-not (Test-Path $ParentDir)) {
        New-Item -ItemType Directory -Path $ParentDir -Force | Out-Null
    }

    $HttpClient = $null
    $FileStream = $null
    $DownloadStream = $null

    try {
        Add-Type -AssemblyName System.Net.Http
        $HttpClient = [System.Net.Http.HttpClient]::new()
        $HttpClient.DefaultRequestHeaders.UserAgent.ParseAdd("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        
        $ResponseTask = $HttpClient.GetAsync($Url, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead)
        $Response = $ResponseTask.GetAwaiter().GetResult()

        if (-not $Response.IsSuccessStatusCode) {
            throw "HTTP $($Response.StatusCode) -- Connection failed"
        }

        $TotalBytes = $Response.Content.Headers.ContentLength
        $ReadableSize = if ($TotalBytes) { "{0:N2} MB" -f ($TotalBytes / 1MB) } else { "Unknown size" }
        Write-Host "  [NETWORK] Target Size: $ReadableSize -- Streaming payload to: $Path..." -ForegroundColor Green

        $DownloadStream = $Response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $FileStream = [System.IO.FileStream]::new(
            $Path,
            [System.IO.FileMode]::Create,
            [System.IO.FileAccess]::Write,
            [System.IO.FileShare]::None
        )

        $Buffer = [System.Byte[]]::new(131072)  # 128KB I/O buffer
        $BytesRead = 0
        $TotalBytesRead = 0
        $LastLoggedMB = 0

        while (($BytesRead = $DownloadStream.Read($Buffer, 0, $Buffer.Length)) -gt 0) {
            $FileStream.Write($Buffer, 0, $BytesRead)
            $TotalBytesRead += $BytesRead

            $CurrentMB = [Math]::Floor($TotalBytesRead / 10MB)
            if ($CurrentMB -gt $LastLoggedMB) {
                $LastLoggedMB = $CurrentMB
                $ProgressMB = "{0:N2}" -f ($TotalBytesRead / 1MB)
                $Percent = if ($TotalBytes) { "{0:P0}" -f ($TotalBytesRead / $TotalBytes) } else { "Streaming" }
                Write-Host "     -> $Percent ($ProgressMB MB)" -ForegroundColor Gray
            }
        }

        $FileStream.Flush()
        $FileStream.Close()
        $DownloadStream.Close()
        $HttpClient.Dispose()

        Write-Host "  [OK] Download complete: $Path" -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "  [FAIL] Download failed: $_" -ForegroundColor Red
        if ($FileStream) { $FileStream.Close() }
        if ($DownloadStream) { $DownloadStream.Close() }
        if ($HttpClient) { $HttpClient.Dispose() }
        if (Test-Path $Path) { Remove-Item $Path -Force }
        return $false
    }
}
