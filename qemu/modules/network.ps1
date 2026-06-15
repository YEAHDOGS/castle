# ==============================================================================
# Castle VM -- Network Download Engine
# ==============================================================================
# Streams large files over HTTP with progress reporting.
# Uses System.Net.Http.HttpClient for efficient chunked I/O.
# ==============================================================================

function Start-NetworkStream {
    param (
        [string]$Url,
        [string]$Path,
        [object]$HttpClient
    )

    Write-Host "  [>] Downloading: $Url" -ForegroundColor Cyan
    try {
        $ResponseTask = $HttpClient.GetAsync($Url, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead)
        $Response = $ResponseTask.GetAwaiter().GetResult()

        if (-not $Response.IsSuccessStatusCode) {
            throw "HTTP $($Response.StatusCode) -- Connection failed"
        }

        $TotalBytes = $Response.Content.Headers.ContentLength
        $ReadableSize = if ($TotalBytes) { "{0:N2} GB" -f ($TotalBytes / 1GB) } else { "Unknown size" }
        Write-Host "  [>] Payload: $ReadableSize -- Streaming to disk..." -ForegroundColor Green

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

        while (($BytesRead = $DownloadStream.Read($Buffer, 0, $Buffer.Length)) -gt 0) {
            $FileStream.Write($Buffer, 0, $BytesRead)
            $TotalBytesRead += $BytesRead

            # Log progress every ~50MB
            if ($TotalBytesRead % 52428800 -lt 131073) {
                $ProgressGB = "{0:N2}" -f ($TotalBytesRead / 1GB)
                $Percent = if ($TotalBytes) { "{0:P0}" -f ($TotalBytesRead / $TotalBytes) } else { "Streaming" }
                Write-Host "     -> $Percent ($ProgressGB GB)" -ForegroundColor Gray
            }
        }

        $FileStream.Flush()
        $FileStream.Close()
        $DownloadStream.Close()
        Write-Host "  [OK] Download complete: $Path" -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "  [FAIL] Download failed: $_" -ForegroundColor Red
        if ($FileStream) { $FileStream.Close() }
        if (Test-Path $Path) { Remove-Item $Path -Force }
        return $false
    }
}
