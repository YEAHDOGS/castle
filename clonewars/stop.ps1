# ==============================================================================
# Castle VM -- clean guest shutdown over QMP
# ==============================================================================
# Every VM launched by start.ps1 opens a loopback QMP socket (port printed at
# boot, first VM gets 4444). This sends the guest an ACPI power-off request, the
# same as pressing the power button, so the OS shuts down cleanly -- which is
# what a golden disk needs before it is cloned. Falls back to nothing: if the
# guest ignores ACPI, close the QEMU window instead.
#
#   .\stop.ps1                 # power off the VM on port 4444
#   .\stop.ps1 -Port 4445      # another VM
#   .\stop.ps1 -Port 4444 -Hard   # immediate QEMU quit (like pulling the plug)
# ==============================================================================
param (
    [int]$Port = 4444,
    [switch]$Hard
)

try {
    $Client = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $Port)
}
catch {
    Write-Host "  [FAIL] No QEMU QMP socket on 127.0.0.1:$Port -- is the VM running, and was it started by start.ps1?" -ForegroundColor Red
    Exit 1
}

try {
    $Stream = $Client.GetStream()
    $Stream.ReadTimeout = 5000
    $Writer = [System.IO.StreamWriter]::new($Stream); $Writer.AutoFlush = $true
    $Reader = [System.IO.StreamReader]::new($Stream)
    $Reader.ReadLine() | Out-Null                       # greeting banner
    $Writer.WriteLine('{"execute":"qmp_capabilities"}')
    $Reader.ReadLine() | Out-Null
    $Command = if ($Hard) { "quit" } else { "system_powerdown" }
    $Writer.WriteLine("{`"execute`":`"$Command`"}")
    $Reply = try { $Reader.ReadLine() } catch { "" }
    if ($Hard) {
        Write-Host "  [STOP] Sent quit to the VM on port $Port." -ForegroundColor Yellow
    }
    else {
        Write-Host "  [STOP] Sent ACPI power-off to the VM on port $Port -- the guest is shutting down." -ForegroundColor Green
        Write-Host "     Wait for the QEMU window to close before cloning or deleting its disk." -ForegroundColor DarkGray
    }
}
finally {
    $Client.Dispose()
}
