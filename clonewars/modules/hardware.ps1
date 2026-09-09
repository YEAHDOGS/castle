# ==============================================================================
# Castle VM — Host Hardware Detection
# ==============================================================================
# Analyzes the host machine's CPU, RAM, and vendor to determine optimal
# QEMU allocation settings. Leaves headroom for the host OS.
# ==============================================================================

function Get-HostHardwareProfile {
    <#
    .SYNOPSIS
        Detects host CPU cores, RAM, and vendor. Returns a hashtable of optimal
        QEMU allocation settings.
    .OUTPUTS
        @{
            CpuCores   = [int]    — Cores to allocate (host - 2, minimum 2)
            Memory     = [string] — RAM string (e.g., "8G")
            CpuProfile = [string] — QEMU -cpu model for the host vendor
            HostCores  = [int]    — Total host logical processors
            TotalRamGB = [int]    — Total host RAM in GB
            CpuVendor  = [string] — Raw vendor string
        }
    #>

    $IsLinuxOS = ($PSVersionTable.OS -like "*Linux*") -or ($IsLinux -eq $true)

    # 1. CPU Cores — standard .NET cross-platform logical core count
    $HostCores = [Environment]::ProcessorCount
    $CpuCores = [Math]::Max(2, $HostCores - 2)

    # 2. Total RAM & CPU Vendor (Platform specific)
    $TotalRamGB = 16 # Fallback default
    $CpuVendor = "Unknown"

    if ($IsLinuxOS) {
        # RAM detection on Linux via /proc/meminfo
        if (Test-Path "/proc/meminfo") {
            $MemInfo = Get-Content "/proc/meminfo" -ErrorAction SilentlyContinue
            $MemTotalLine = $MemInfo | Select-String "MemTotal"
            if ($MemTotalLine -match "(\d+)") {
                $TotalRamKB = [double]$Matches[1]
                $TotalRamGB = [Math]::Round($TotalRamKB / 1024 / 1024)
            }
        }

        # CPU Vendor detection on Linux via /proc/cpuinfo
        if (Test-Path "/proc/cpuinfo") {
            $CpuInfo = Get-Content "/proc/cpuinfo" -ErrorAction SilentlyContinue
            $VendorLine = $CpuInfo | Select-String "vendor_id" | Select-Object -First 1
            if ($VendorLine -match "vendor_id\s*:\s*(.+)") {
                $CpuVendor = $Matches[1].Trim()
            }
        }
    }
    else {
        # Windows RAM & CPU Vendor via CIM/WMI
        try {
            $CompSys = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue
            if ($CompSys) {
                $TotalRamGB = [Math]::Round($CompSys.TotalPhysicalMemory / 1GB)
            }
            $Processors = @(Get-CimInstance Win32_Processor -ErrorAction SilentlyContinue)
            if ($Processors -and $Processors[0]) {
                $CpuVendor = $Processors[0].Manufacturer
            }
        }
        catch {
            # Catch block just in case CIM is blocked or errors out
        }
    }

    # RAM Allocation -- 50% of total RAM, clamped between 4GB and a ceiling.
    # The ceiling defaults to 8GB (safe on laptops) but can be raised for
    # beefy home-server hosts:  $env:CASTLE_VM_MAX_RAM_GB = 32
    $MaxRamGB = 8
    if ($env:CASTLE_VM_MAX_RAM_GB -and ($env:CASTLE_VM_MAX_RAM_GB -as [int])) {
        $MaxRamGB = [Math]::Max(4, [int]$env:CASTLE_VM_MAX_RAM_GB)
    }
    $MemoryGB = [Math]::Min($MaxRamGB, [Math]::Max(4, [Math]::Round($TotalRamGB / 2)))

    # CPU Profile Vendor Selection
    $CpuProfile = switch -Wildcard ($CpuVendor) {
        "*Intel*" { "Haswell-v4" }
        "*Advanced Micro Devices*" { "EPYC-v4" }
        default { "max" }
    }

    return @{
        CpuCores   = $CpuCores
        Memory     = "${MemoryGB}G"
        CpuProfile = $CpuProfile
        HostCores  = $HostCores
        TotalRamGB = $TotalRamGB
        CpuVendor  = $CpuVendor
    }
}
