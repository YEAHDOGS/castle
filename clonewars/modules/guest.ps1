# ==============================================================================
# QEMU GUEST AGENT CLIENT
# ==============================================================================
# launch.ps1 attaches a virtio-serial port named org.qemu.guest_agent.0 to
# every VM and exposes it on a loopback TCP socket. Guests running
# qemu-guest-agent (the Arch/EndeavourOS ISOs ship it, and its udev rule starts
# it the moment the port appears) answer JSON commands on that socket: run a
# program, read a file. That is how `start.ps1 -Copy` installs Wine and starts
# programs inside a VM with no login, no SSH key and no cloud-init profile.
#
# Protocol: one JSON object per line each way. The socket chardev takes one
# client at a time, so a session is opened once and closed when done.

function Invoke-QgaCommand {
    param (
        [hashtable]$Session,
        [string]$Execute,
        [hashtable]$Arguments = $null,
        [int]$TimeoutMs = 15000
    )

    $Msg = [ordered]@{ execute = $Execute }
    if ($Arguments) { $Msg.arguments = $Arguments }
    $Json = ($Msg | ConvertTo-Json -Depth 8 -Compress) + "`n"
    $Bytes = [System.Text.Encoding]::UTF8.GetBytes($Json)
    $Session.Stream.ReadTimeout = $TimeoutMs
    $Session.Stream.Write($Bytes, 0, $Bytes.Length)
    $Session.Stream.Flush()

    while ($true) {
        $Line = $Session.Reader.ReadLine()
        if ($null -eq $Line) { throw "guest agent connection closed" }
        # Anything before the JSON (the 0xFF sync marker decodes to U+FFFD) is noise.
        $Line = $Line -replace '^[^{]*', ''
        if ($Line.Length -eq 0) { continue }
        try { $Obj = $Line | ConvertFrom-Json } catch { continue }
        if ($Obj.PSObject.Properties['error']) { throw "guest agent: $Execute -> $($Obj.error.desc)" }
        if ($Obj.PSObject.Properties['return']) { return $Obj.return }
    }
}


function Connect-CastleGuestAgent {
    <#
    .SYNOPSIS
        Polls the guest-agent socket until the agent inside the VM answers a
        guest-sync, or the timeout passes, or the QEMU process goes away.
    .OUTPUTS
        A session hashtable (Client/Stream/Reader) or $null.
    #>
    param (
        [int]$Port,
        [int]$TimeoutSec = 900,
        [System.Diagnostics.Process]$QemuProcess = $null
    )

    $Deadline = (Get-Date).AddSeconds($TimeoutSec)
    $Attempt = 0
    while ((Get-Date) -lt $Deadline) {
        if ($QemuProcess -and $QemuProcess.HasExited) { return $null }
        $Attempt++
        $Client = $null
        try {
            $Client = [System.Net.Sockets.TcpClient]::new()
            $Client.Connect("127.0.0.1", $Port)
            $Stream = $Client.GetStream()
            $Reader = [System.IO.StreamReader]::new($Stream, [System.Text.Encoding]::UTF8)
            $Session = @{ Client = $Client; Stream = $Stream; Reader = $Reader }
            # No 0xFF "parser reset" byte here: qemu-ga 11 does not swallow it,
            # it answers the following command with "stray U+FFFD" instead.
            # A plain guest-sync is enough to prove the agent is listening.
            $Id = Get-Random -Minimum 1 -Maximum 1000000
            $Reply = Invoke-QgaCommand -Session $Session -Execute "guest-sync" -Arguments @{ id = $Id } -TimeoutMs 4000
            if ($Reply -eq $Id) { return $Session }
        }
        catch { }
        if ($Client) { $Client.Dispose() }
        if ($Attempt % 12 -eq 1) {
            $Left = [int]($Deadline - (Get-Date)).TotalMinutes
            Write-Host "  [GUEST] Waiting for the guest agent inside the VM ($Left min left)..." -ForegroundColor DarkGray
        }
        Start-Sleep -Seconds 5
    }
    return $null
}


function Send-CastleGuestFile {
    <#
    .SYNOPSIS
        Copies one host file into the guest through the agent (guest-file-write,
        base64 in 32 KB pieces). Used to hand the setup script over directly:
        the FAT share snapshot QEMU took at boot would serve a stale copy of a
        script edited while the VM runs.
    #>
    param (
        [hashtable]$Session,
        [string]$LocalPath,
        [string]$RemotePath,
        [string]$Mode = "w"
    )
    $Bytes = [System.IO.File]::ReadAllBytes($LocalPath)
    $Handle = Invoke-QgaCommand -Session $Session -Execute "guest-file-open" -Arguments @{ path = $RemotePath; mode = $Mode }
    try {
        for ($Off = 0; $Off -lt $Bytes.Length; $Off += 32768) {
            $Len = [Math]::Min(32768, $Bytes.Length - $Off)
            $B64 = [Convert]::ToBase64String($Bytes, $Off, $Len)
            Invoke-QgaCommand -Session $Session -Execute "guest-file-write" -Arguments @{ handle = $Handle; 'buf-b64' = $B64 } | Out-Null
        }
    }
    finally {
        Invoke-QgaCommand -Session $Session -Execute "guest-file-close" -Arguments @{ handle = $Handle } | Out-Null
    }
}


function Invoke-CastleGuestSetup {
    <#
    .SYNOPSIS
        Runs scripts/<Script> inside the guest through the guest agent:
        mounts the CASTLE share, starts the script as root with the requested
        program list, and streams its log back to this console until it exits.
        setup-wine.sh and setup-opengoal.sh share the same flag vocabulary
        (--copy, --all, --program X).
    .OUTPUTS
        $true when the script finished with exit code 0.
    #>
    param (
        [int]$QgaPort,
        [string]$Script = "setup-wine.sh",
        [string[]]$Programs = @(),
        [switch]$RunAll = $false,
        [int]$TimeoutSec = 900,
        [System.Diagnostics.Process]$QemuProcess = $null
    )

    $ScriptArgs = "--copy"
    if ($RunAll) { $ScriptArgs += " --all" }
    foreach ($p in $Programs) {
        if ([string]::IsNullOrWhiteSpace($p)) { continue }
        $Escaped = $p -replace "'", "'\''"
        $ScriptArgs += " --program '$Escaped'"
    }
    $Manual = "sudo mount -o ro /dev/disk/by-label/CASTLE /mnt && sudo bash /mnt/$Script $ScriptArgs"

    Write-Host ""
    Write-Host "  [GUEST] $Script -- waiting for the VM to boot (up to $([int]($TimeoutSec / 60)) min)..." -ForegroundColor Cyan
    $Qga = Connect-CastleGuestAgent -Port $QgaPort -TimeoutSec $TimeoutSec -QemuProcess $QemuProcess
    if (-not $Qga) {
        if ($QemuProcess -and $QemuProcess.HasExited) {
            Write-Host "  [GUEST] The VM exited before the guest agent answered." -ForegroundColor Yellow
        }
        else {
            Write-Host "  [GUEST] No guest agent answered. Run the setup inside the VM instead:" -ForegroundColor Yellow
            Write-Host "          $Manual" -ForegroundColor White
        }
        return $false
    }

    $Log = "/tmp/castle-$([System.IO.Path]::GetFileNameWithoutExtension($Script)).log"
    $RemoteScript = "/tmp/castle-$Script"
    # The script itself is uploaded through the agent (always the current host
    # copy). The CASTLE share is still mounted at /mnt/castle for anything the
    # script wants from it, but its absence is not fatal.
    $Cmd = "for i in `$(seq 1 10); do [ -e /dev/disk/by-label/CASTLE ] && break; sleep 2; done; " +
    "mkdir -p /mnt/castle && (mountpoint -q /mnt/castle || mount -o ro /dev/disk/by-label/CASTLE /mnt/castle 2>/dev/null); " +
    ": > $Log; exec bash $RemoteScript $ScriptArgs >> $Log 2>&1"

    $Handle = $null
    try {
        $LocalScript = Join-Path (Join-Path (Split-Path $PSScriptRoot -Parent) "scripts") $Script
        if (-not (Test-Path $LocalScript)) { throw "scripts/$Script not found on the host" }
        Send-CastleGuestFile -Session $Qga -LocalPath $LocalScript -RemotePath $RemoteScript
        Write-Host "  [GUEST] Agent up. Running $Script $ScriptArgs" -ForegroundColor Green
        Write-Host "  -------------------------------------------" -ForegroundColor DarkGray
        $Exec = Invoke-QgaCommand -Session $Qga -Execute "guest-exec" -Arguments @{
            path             = "/bin/bash"
            arg              = @("-c", $Cmd)
            'capture-output' = $true
        }
        $GuestPid = $Exec.pid
        $Offset = 0
        $Status = $null

        while ($true) {
            if (-not $Handle) {
                try { $Handle = Invoke-QgaCommand -Session $Qga -Execute "guest-file-open" -Arguments @{ path = $Log; mode = "r" } } catch { }
            }
            if ($Handle) {
                # Re-seek every read: glibc's EOF is sticky, a plain re-read after
                # the file grew would return nothing.
                Invoke-QgaCommand -Session $Qga -Execute "guest-file-seek" -Arguments @{ handle = $Handle; offset = $Offset; whence = "set" } | Out-Null
                $Chunk = Invoke-QgaCommand -Session $Qga -Execute "guest-file-read" -Arguments @{ handle = $Handle; count = 65536 }
                if ($Chunk.count -gt 0) {
                    $Offset += [int]$Chunk.count
                    $Text = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Chunk.'buf-b64'))
                    Write-Host ($Text -replace '(?m)^', '  ') -NoNewline
                    if (-not $Text.EndsWith("`n")) { Write-Host "" }
                }
            }
            $Status = Invoke-QgaCommand -Session $Qga -Execute "guest-exec-status" -Arguments @{ pid = $GuestPid }
            if ($Status.exited) {
                if ($Status.'err-data') {
                    $Err = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Status.'err-data'))
                    if ($Err.Trim()) { Write-Host ($Err -replace '(?m)^', '  ') -ForegroundColor Yellow }
                }
                if ($Status.'out-data') {
                    $Out = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Status.'out-data'))
                    if ($Out.Trim()) { Write-Host ($Out -replace '(?m)^', '  ') }
                }
                break
            }
            if ($QemuProcess -and $QemuProcess.HasExited) { throw "VM exited" }
            Start-Sleep -Seconds 2
        }

        Write-Host "  -------------------------------------------" -ForegroundColor DarkGray
        $Code = if ($null -ne $Status.exitcode) { [int]$Status.exitcode } else { -1 }
        if ($Code -eq 0) {
            Write-Host "  [GUEST] $Script finished. Copied files live in ~/castle-copy inside the VM." -ForegroundColor Green
            return $true
        }
        Write-Host "  [GUEST] $Script exited with code $Code (log inside the VM: $Log)." -ForegroundColor Red
        return $false
    }
    catch {
        Write-Host "  [GUEST] Lost the guest agent: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host "          If the VM is still up, run inside it:  $Manual" -ForegroundColor DarkGray
        return $false
    }
    finally {
        if ($Handle) { try { Invoke-QgaCommand -Session $Qga -Execute "guest-file-close" -Arguments @{ handle = $Handle } -TimeoutMs 3000 | Out-Null } catch { } }
        $Qga.Client.Dispose()
    }
}
