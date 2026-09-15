# ==============================================================================
# Castle VM -- Network Download Engine
# ==============================================================================
# Streams large files over HTTP with progress reporting.
# Uses System.Net.Http.HttpClient for efficient chunked I/O.
# Every remote fetch (ISO, checksum manifest, signature, keyserver, resolver
# query) prints the endpoint it is about to talk to: the host name resolved,
# at run time, through the DNS server this machine is actively using.
# ==============================================================================

$Script:CastleDnsCache = @{}
$Script:CastleActiveDns = $null

function Get-ActiveDnsServer {
    <#
    .SYNOPSIS
        The DNS server the host is actually using right now. Nothing is
        hardcoded: on Windows it is the DNS client configuration of the
        interface that owns the default route (lowest metric, so a VPN that
        took over routing wins), on Linux/containers the first nameserver in
        /etc/resolv.conf. Returns $null when it cannot be determined; the
        caller then falls back to the OS resolver.
    #>
    if ($Script:CastleActiveDns -ne $null) { return $Script:CastleActiveDns }

    $Server = $null
    try {
        if (Get-Command Get-NetRoute -ErrorAction SilentlyContinue) {
            $Routes = @(Get-NetRoute -DestinationPrefix "0.0.0.0/0", "::/0" -ErrorAction SilentlyContinue |
                Sort-Object { $_.RouteMetric + $_.InterfaceMetric })
            foreach ($Route in $Routes) {
                $Dns = @(Get-DnsClientServerAddress -InterfaceIndex $Route.InterfaceIndex -ErrorAction SilentlyContinue |
                    Sort-Object { if ($_.AddressFamily -eq 2) { 0 } else { 1 } } |
                    ForEach-Object { $_.ServerAddresses } | Where-Object { $_ })
                if ($Dns.Count -gt 0) { $Server = $Dns[0]; break }
            }
            if (-not $Server) {
                # No default route carries DNS (some VPN adapters): first interface that has any.
                $Dns = @(Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
                    Where-Object { $_.ServerAddresses } | ForEach-Object { $_.ServerAddresses })
                if ($Dns.Count -gt 0) { $Server = $Dns[0] }
            }
        }
        elseif (Test-Path "/etc/resolv.conf") {
            $Line = Get-Content "/etc/resolv.conf" -ErrorAction SilentlyContinue |
                Where-Object { $_ -match '^\s*nameserver\s+(\S+)' } | Select-Object -First 1
            if ($Line -match '^\s*nameserver\s+(\S+)') { $Server = $Matches[1] }
        }
    }
    catch { }

    $Script:CastleActiveDns = if ($Server) { [string]$Server } else { "" }
    return $Script:CastleActiveDns
}


function Resolve-EndpointAddress {
    <#
    .SYNOPSIS
        Resolves the host of a URL to its IP address(es) by querying the active
        DNS server directly (Resolve-DnsName -Server ... -DnsOnly, so no hosts
        file or cached answer). Falls back to the OS resolver where the DnsClient
        cmdlets do not exist (Linux/containers), which itself uses the active
        nameserver. Answers are cached per host for the run.
    .OUTPUTS
        @{ Host; Addresses = string[]; DnsServer; Method } or $null for a URL
        with no resolvable host.
    #>
    param ([string]$Url)

    $HostName = $null
    try { $HostName = ([System.Uri]$Url).DnsSafeHost } catch { }
    if ([string]::IsNullOrWhiteSpace($HostName)) { return $null }

    if ($Script:CastleDnsCache.ContainsKey($HostName)) { return $Script:CastleDnsCache[$HostName] }

    $DnsServer = Get-ActiveDnsServer
    $Addresses = @()
    $Method = "dns"

    $Literal = $null
    if ([System.Net.IPAddress]::TryParse($HostName, [ref]$Literal)) {
        $Addresses = @($HostName)
        $Method = "literal"
    }
    else {
        if ($DnsServer -and (Get-Command Resolve-DnsName -ErrorAction SilentlyContinue)) {
            try {
                $Records = @(Resolve-DnsName -Name $HostName -Server $DnsServer -DnsOnly -ErrorAction Stop)
                # A/AAAA only: CNAME hops are part of the answer but are not endpoints.
                $Addresses = @($Records | Where-Object { $_.Type -eq "A" } | ForEach-Object { $_.IPAddress }) +
                             @($Records | Where-Object { $_.Type -eq "AAAA" } | ForEach-Object { $_.IPAddress })
            }
            catch { $Addresses = @() }
        }
        if ($Addresses.Count -eq 0) {
            try {
                $Addresses = @([System.Net.Dns]::GetHostAddresses($HostName) |
                    Sort-Object { if ($_.AddressFamily -eq "InterNetwork") { 0 } else { 1 } } |
                    ForEach-Object { $_.IPAddressToString })
                $Method = "system"
            }
            catch { $Addresses = @() }
        }
    }

    $Result = @{ Host = $HostName; Addresses = @($Addresses | Select-Object -Unique); DnsServer = $DnsServer; Method = $Method }
    $Script:CastleDnsCache[$HostName] = $Result
    return $Result
}


function Write-EndpointInfo {
    <#
    .SYNOPSIS
        Prints the endpoint a URL points at right now, e.g.
          [DNS] mirror.cachyos.org -> 172.67.73.65, 104.26.5.68 (via 192.168.1.1)
        Called before every remote fetch so the operator can see exactly which
        server each ISO, checksum, signature and API call goes to.
    #>
    param (
        [string]$Url,
        [string]$Indent = "  "
    )

    $Info = Resolve-EndpointAddress -Url $Url
    if (-not $Info) { return }

    if ($Info.Addresses.Count -eq 0) {
        Write-Host "$Indent[DNS] $($Info.Host) -> unresolved$(if ($Info.DnsServer) { " (via $($Info.DnsServer))" })" -ForegroundColor Yellow
        return
    }

    $Via = switch ($Info.Method) {
        "literal" { "literal address" }
        "system"  { if ($Info.DnsServer) { "system resolver, DNS $($Info.DnsServer)" } else { "system resolver" } }
        default   { "via $($Info.DnsServer)" }
    }

    # Each address in its own shade of green. The console palette only has two
    # greens, so the shades are 24-bit ANSI colors; hosts without VT support
    # (old conhost) get plain green for all of them.
    $Shades = @(
        @(0, 255, 0),      # lime
        @(80, 200, 120),   # emerald
        @(160, 255, 160),  # pale
        @(0, 160, 60),     # deep
        @(180, 255, 60),   # chartreuse
        @(0, 210, 160),    # sea
        @(120, 180, 90),   # olive
        @(0, 120, 0)       # forest
    )
    $Vt = $Host.UI.SupportsVirtualTerminal
    $Esc = [char]27

    Write-Host "$Indent[DNS] $($Info.Host) -> " -ForegroundColor DarkCyan -NoNewline
    $Shown = @($Info.Addresses | Select-Object -First 4)
    for ($i = 0; $i -lt $Shown.Count; $i++) {
        if ($i -gt 0) { Write-Host ", " -ForegroundColor DarkCyan -NoNewline }
        if ($Vt) {
            $Rgb = $Shades[$i % $Shades.Count]
            Write-Host "$Esc[38;2;$($Rgb[0]);$($Rgb[1]);$($Rgb[2])m$($Shown[$i])$Esc[0m" -NoNewline
        }
        else {
            Write-Host $Shown[$i] -ForegroundColor Green -NoNewline
        }
    }
    if ($Info.Addresses.Count -gt 4) { Write-Host " (+$($Info.Addresses.Count - 4) more)" -ForegroundColor DarkCyan -NoNewline }
    Write-Host " ($Via)" -ForegroundColor DarkCyan
}


function Start-NetworkStream {
    <#
    .SYNOPSIS
        Streams a URL to disk in 128 KB chunks with a progress line every 50 MB.
        Each chunk read has a timeout, and an interrupted transfer is resumed with
        an HTTP Range request, so a stalled socket (observed on this host: the
        connection went silent after the first ~128 KB and the old loop waited
        forever) costs a retry instead of a hung pipeline.
    #>
    param (
        [string]$Url,
        [string]$Path,
        [object]$HttpClient,
        [int]$MaxAttempts = 6,
        [int]$ReadTimeoutSec = 45
    )

    Write-Host "  [>] Downloading: $Url" -ForegroundColor Cyan
    Write-EndpointInfo -Url $Url

    $TotalBytes = $null
    $Attempt = 0
    while ($Attempt -lt $MaxAttempts) {
        $Attempt++
        $FileStream = $null
        $DownloadStream = $null
        $Response = $null
        $Existing = if (Test-Path $Path) { (Get-Item $Path).Length } else { 0 }

        try {
            $Request = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::Get, $Url)
            if ($Existing -gt 0) {
                $Request.Headers.Range = [System.Net.Http.Headers.RangeHeaderValue]::new($Existing, $null)
                Write-Host "  [>] Resuming at $("{0:N2}" -f ($Existing / 1GB)) GB (attempt $Attempt/$MaxAttempts)..." -ForegroundColor Yellow
            }
            $Response = $HttpClient.SendAsync($Request, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()

            if ($Existing -gt 0 -and $Response.StatusCode -ne [System.Net.HttpStatusCode]::PartialContent) {
                # Server ignored the Range header: start the file over.
                Write-Host "  [?] Server does not support resume -- restarting download." -ForegroundColor Yellow
                $Existing = 0
            }
            if (-not $Response.IsSuccessStatusCode) {
                throw "HTTP $($Response.StatusCode) -- Connection failed"
            }

            if ($Existing -gt 0) {
                if ($Response.Content.Headers.ContentRange -and $Response.Content.Headers.ContentRange.Length) {
                    $TotalBytes = $Response.Content.Headers.ContentRange.Length
                }
            }
            else {
                $TotalBytes = $Response.Content.Headers.ContentLength
                $ReadableSize = if ($TotalBytes) { "{0:N2} GB" -f ($TotalBytes / 1GB) } else { "Unknown size" }
                Write-Host "  [>] Payload: $ReadableSize -- Streaming to disk..." -ForegroundColor Green
            }

            $DownloadStream = $Response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
            $FileMode = if ($Existing -gt 0) { [System.IO.FileMode]::Append } else { [System.IO.FileMode]::Create }
            $FileStream = [System.IO.FileStream]::new($Path, $FileMode, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)

            $Buffer = [System.Byte[]]::new(131072)  # 128KB I/O buffer
            $TotalBytesRead = [int64]$Existing
            $NextMark = ([math]::Floor($TotalBytesRead / 52428800) + 1) * 52428800

            while ($true) {
                $Cts = [System.Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds($ReadTimeoutSec))
                try {
                    $BytesRead = $DownloadStream.ReadAsync($Buffer, 0, $Buffer.Length, $Cts.Token).GetAwaiter().GetResult()
                }
                finally { $Cts.Dispose() }
                if ($BytesRead -le 0) { break }

                $FileStream.Write($Buffer, 0, $BytesRead)
                $TotalBytesRead += $BytesRead

                if ($TotalBytesRead -ge $NextMark) {
                    $ProgressGB = "{0:N2}" -f ($TotalBytesRead / 1GB)
                    $Percent = if ($TotalBytes) { "{0:P0}" -f ($TotalBytesRead / $TotalBytes) } else { "Streaming" }
                    Write-Host "     -> $Percent ($ProgressGB GB)" -ForegroundColor Gray
                    $NextMark += 52428800
                }
            }

            $FileStream.Flush()
            $FileStream.Close(); $FileStream = $null
            $DownloadStream.Close(); $DownloadStream = $null
            $Response.Dispose(); $Response = $null

            if ($TotalBytes -and $TotalBytesRead -lt $TotalBytes) {
                throw "connection closed after $TotalBytesRead of $TotalBytes bytes"
            }

            Write-Host "  [OK] Download complete: $Path" -ForegroundColor Green
            return $true
        }
        catch {
            $Reason = if ($_.Exception -is [System.OperationCanceledException] -or $_.Exception.InnerException -is [System.OperationCanceledException]) {
                "no data for ${ReadTimeoutSec}s"
            }
            else { $_.Exception.Message }
            Write-Host "  [?] Download interrupted ($Reason) -- attempt $Attempt/$MaxAttempts" -ForegroundColor Yellow
            if ($FileStream) { try { $FileStream.Close() } catch { } }
            if ($DownloadStream) { try { $DownloadStream.Close() } catch { } }
            if ($Response) { try { $Response.Dispose() } catch { } }
            if ($Attempt -lt $MaxAttempts) { Start-Sleep -Seconds 3 }
        }
    }

    Write-Host "  [FAIL] Download failed after $MaxAttempts attempts." -ForegroundColor Red
    if (Test-Path $Path) { Remove-Item $Path -Force }
    return $false
}
