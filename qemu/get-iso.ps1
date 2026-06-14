<#
.SYNOPSIS
    Castle Infrastructure: Multi-Matrix ISO Engine
.DESCRIPTION
    Deploys OS images down to storage buffers utilizing high-speed async streams.
.PARAMETER Iso
    Filter down to a specific target image by name (e.g., -i cachy, -iso windows7, -i mac)
.EXAMPLE
    .\Fetch-Iso.ps1 -Iso cachy
    .\Fetch-Iso.ps1 -i windows11
#>
param (
    [Parameter(Mandatory = $false, Position = 0)]
    [Alias("i")]
    [String]$Iso
)

# ==============================================================================
# TARGET MATRICES MAP
# ==============================================================================
$IsoMatrix = @(
    @{
        Id   = "cachyos"
        Name = "CachyOS Linux"
        Url  = "https://iso.cachyos.org/desktop/260426/cachyos-desktop-linux-260426.iso"
        File = ".\cachyos-desktop-latest.iso"
    },
    @{
        Id   = "endeavouros"
        Name = "EndeavourOS Linux"
        Url  = "https://mirror.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso"
        File = ".\endeavouros-latest.iso"
    },
    @{
        Id   = "debian"
        Name = "Debian NetInst"
        Url  = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.5.0-amd64-netinst.iso"
        File = ".\debian-amd64-netinst.iso"
    },
    @{
        Id   = "windows11"
        Name = "Windows 11 Enterprise (Evaluation)"
        Url  = "https://software-static.download.prss.microsoft.com/kv/win/ch9/enterprise/26100.1742.240906-0331.ge_release_svc_refresh_CLIENTENTERPRISEEVAL_x64FRE_en-us.iso"
        File = ".\windows11-enterprise-eval.iso"
    },
    @{
        Id   = "windows10"
        Name = "Windows 10 Enterprise (Evaluation)"
        Url  = "https://software-static.download.prss.microsoft.com/pr/mp/vct/Win1022H2/Enterprise/19045.2006.220908-0225.22h2_release_svc_refresh_CLIENTENTERPRISEEVAL_x64FRE_en-us.iso"
        File = ".\windows10-enterprise-eval.iso"
    },
    @{
        Id   = "windows7"
        Name = "Windows 7 SP1 Ultimate"
        Url  = "https://archive.org/download/win7-ultimate-sp1-x64/Win7_Ult_SP1_English_COEM_x64.iso"
        File = ".\windows7-ultimate-sp1.iso"
    },
    @{
        Id   = "mac"
        Name = "macOS Bare-Metal Bootstrap (OpenCore Shim)"
        Url  = "https://github.com/thenickdude/KVM-Opencore/releases/download/v22/OpenCore-v22.iso"
        File = ".\macos-kvm-opencore-shim.iso"
    }
)

# ==============================================================================
# PIPELINE FILTERING
# ==============================================================================
$ProcessingQueue = $IsoMatrix

if (-not [String]::IsNullOrWhiteSpace($Iso)) {
    # Sanitize inputs to allow flexible match boundaries
    $CleanInput = $Iso.Trim().ToLower().Replace(" ", "")
    $ProcessingQueue = $IsoMatrix | Where-Object { 
        $_.Id -like "*$CleanInput*" -or $_.Name.ToLower().Replace(" ", "") -like "*$CleanInput*" 
    }

    if ($ProcessingQueue.Count -eq 0) {
        Write-Host "❌ [Castle] Identity matrix check failed. No targets matched string signature: '$Iso'" -ForegroundColor Red
        Write-Host "💡 Available keys: (($IsoMatrix | ForEach-Object { $_.Id }) -join ', ')" -ForegroundColor DarkGray
        Exit
    }
}

# Pre-Flight Target Directory Checks
$TargetDir = [System.IO.Path]::GetDirectoryName((Resolve-Path -Path ".\").Path)
if (-not (Test-Path $TargetDir)) {
    New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
}

# Initialize Async Engine Client
$HttpClient = [System.Net.Http.HttpClient]::new()

foreach ($Target in $ProcessingQueue) {
    Write-Host "`n========================================================================" -ForegroundColor DarkGray
    Write-Host "🚀 Deploying Target Stack: $($Target.Name)" -ForegroundColor Magenta
    Write-Host "========================================================================" -ForegroundColor DarkGray

    $OutFile = $Target.File
    $IsoUrl = $Target.Url

    # Pre-Flight Local Check
    if (Test-Path $OutFile) {
        Write-Host "📦 [Castle] ISO asset already exists locally: $OutFile" -ForegroundColor Yellow
        continue
    }

    Write-Host "📡 [Castle] Establishing stream pipeline to: $IsoUrl" -ForegroundColor Cyan
    Write-Host "💾 [Castle] Destination Target: $OutFile" -ForegroundColor Cyan

    try {
        $ResponseTask = $HttpClient.GetAsync($IsoUrl, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead)
        $Response = $ResponseTask.GetAwaiter().GetResult()
        
        if (-not $Response.IsSuccessStatusCode) {
            throw "Server returned status code: $($Response.StatusCode)"
        }

        # Size Resolution Metrics
        $TotalBytes = $Response.Content.Headers.ContentLength
        $ReadableSize = if ($TotalBytes) { "{0:N2} GB" -f ($TotalBytes / 1GB) } else { "Unknown" }
        
        Write-Host "⚡ [Castle] Payload Detected. Size: $ReadableSize. Streaming bits..." -ForegroundColor Green

        # Stream Initialization
        $DownloadStream = $Response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $FileStream = [System.IO.FileStream]::new($OutFile, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
        $Buffer = [System.Byte[]]::new(65536) # 64KB Page Blocks
        $BytesRead = 0
        $TotalBytesRead = 0
        
        while (($BytesRead = $DownloadStream.Read($Buffer, 0, $Buffer.Length)) -gt 0) {
            $FileStream.Write($Buffer, 0, $BytesRead)
            $TotalBytesRead += $BytesRead
            
            # Non-blocking telemetry output ticks every ~50MB
            if ($TotalBytesRead % 52428800 -lt 65536) {
                $Percent = if ($TotalBytes) { "{0:P0}" -f ($TotalBytesRead / $TotalBytes) } else { "Streaming..." }
                Write-Host "   -> Transferred: $Percent ($("{0:N2}" -f ($TotalBytesRead / 1GB)) GB)" -ForegroundColor Gray
            }
        }

        # Safe Resource Shutdown
        $FileStream.Flush()
        $FileStream.Close()
        $DownloadStream.Close()

        Write-Host "🎯 [Castle] Deployment verified: $OutFile" -ForegroundColor Green
    }
    catch {
        Write-Host "❌ [Castle] Critical Pipeline Failure on $($Target.Name): $_" -ForegroundColor Red
        if ($FileStream) { $FileStream.Close() }
        if (Test-Path $OutFile) { Remove-Item $OutFile -Force }
    }
}

# Global Context Clean-Up
$HttpClient.Dispose()
Write-Host "`n🏁 [Castle] Runtime job operations concluded." -ForegroundColor Cyan