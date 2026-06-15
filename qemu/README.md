# Castle VM -- QEMU Provisioning Pipeline

Castle VM is a modular, zero-dependency, automated PowerShell virtual machine pipeline for Windows. It enables developers to query, stream, cryptographically verify, provision, and boot target operating systems in QEMU with a single command.

---

## Quick Start

You can run the VM pipeline in two modes:

### 1. Interactive Mode (Command Line Menu)
Simply execute the entry point script with no arguments to boot the interactive selection prompt.
```powershell
.\start.ps1
```
- Select an operating system target from the numbered registry.
- View detailed metadata, description, storage configurations, and caching status.
- Confirm selection to dynamically resolve, download, verify, and launch the virtual machine.

### 2. Direct Target Mode
Bypass the menu and immediately boot a target by passing its ID:
```powershell
.\start.ps1 cachyos
.\start.ps1 ubuntu-server
.\start.ps1 windows11
```

### 3. Cleanup & Deletion Mode
You can clean up cached installer ISOs or delete VM storage disks directly from the command line:
- **Delete Virtual Disk (`.qcow2`):**
  ```powershell
  .\start.ps1 cachyos -DeleteDisk
  ```
- **Delete Cached ISO (`.iso`):**
  ```powershell
  .\start.ps1 cachyos -DeleteIso
  ```
- **Full Purge (Delete both files & remove pinned trust hashes):**
  ```powershell
  .\start.ps1 cachyos -Purge
  ```

To view a static registry of targets and cache statuses:
```powershell
.\start.ps1 list
```

### 4. Swarm Mode & Multi-Instance Boot
To spin up multiple separate instances of the same target OS simultaneously, or run background jobs:
- **Asynchronous Background Launch (`-Background`):**
  Launch a VM process in the background and return immediately to the shell:
  ```powershell
  .\start.ps1 cachyos -Background
  ```
- **Isolated Custom Instance Name (`-Instance`):**
  Create a distinct VM disk under `data/instances/<instance-id>.qcow2` to run multiple distinct nodes side-by-side:
  ```powershell
  .\start.ps1 cachyos -Instance node-01
  .\start.ps1 cachyos -Instance node-02
  ```
- **Instant Linked Clones (`-BaseDisk`):**
  Boot an instance backed by a read-only base disk template to save massive disk space and enable sub-second provisioning:
  ```powershell
  .\start.ps1 cachyos -Instance node-03 -BaseDisk .\data\cachyos.qcow2
  ```
- **Batch Swarm Launching:**
  Spin up a swarm of 10 VMs running simultaneously in the background with a single PowerShell line:
  ```powershell
  1..10 | ForEach-Object {
      .\start.ps1 cachyos -Instance "node-$_" -Background
  }
  ```

---

## Directory Structure

All components are fully modularized under the following directory layout:
```text
qemu/
├── start.ps1              # Core entry point, CLI loop, and pipeline runner
├── README.md              # Technical pipeline documentation (this file)
├── qemu-notes.md          # Architectural research notes
├── data/                  # Cached assets (created automatically)
│   ├── *.iso              # Cached installer images
│   ├── *.qcow2            # VM virtual storage disks
│   └── .castle_trust.json # Local trust pinnings database
├── modules/               # Modular script handlers
│   ├── hardware.ps1       # CPU cores, RAM configuration, and WHPX detection
│   ├── launch.ps1         # QEMU profile builder and launch engine
│   ├── network.ps1        # Large file networking streams and progress bar
│   ├── resolvers.ps1      # GitHub API and HTML directory version parsers
│   ├── targets.ps1        # Target registry & configuration manifest
│   └── verify.ps1         # Multi-algorithm crypto and trust pinning implementation
└── scripts/               # Linux guest autoinstall shell/YAML assets
```

---

## Core Pipeline Architecture

When a target is chosen, the engine executes a sequential 5-phase pipeline to transition the OS target from a remote URL into a running VM.

### Phase 1: Dynamic Version Discovery & ISO Acquisition
1. If the target has a `ResolverType` (e.g. `HtmlDirectory` or `GitHub`), the engine queries the remote server to find available release versions and prompts you to select one.
2. If the chosen ISO is not found in `data/`, the pipeline initiates a secure download, streaming it chunk-by-chunk to prevent memory leaks and showing a console progress bar.

### Phase 2: Cryptographic Verification (Trust Pinning System)
To ensure the pipeline is "sealed from madness," every image must pass rigorous integrity verification:
1. **Trust Pinning:** Upon the first successful validation of a download (authenticated via remote SHA hash files or GPG signatures), the target's computed cryptographic signature is saved to the local trust store (`data/.castle_trust.json`).
2. **Offline Pinning:** On subsequent boots, the pipeline verifies the ISO against this local database. This prevents man-in-the-middle attacks, DNS poisoning, and local bit-rot, allowing offline boots to remain cryptographically secure.
3. **Multi-Algorithm Auditing:** Supports `SHA256`, `SHA512`, `SHA384`, `SHA1`, and `MD5`. If a target specifies multiple validation paths, the engine performs multi-checksum auditing.

### Phase 3: Virtual Disk Provisioning
The engine provisions a thin-provisioned copy-on-write virtual disk (`qcow2`) in the `data/` folder:
- **First Boot Detection:** If no disk exists, the engine runs `qemu-img` to create one and configures the VM to boot from the installer CD-ROM.
- **Subsequent Boots:** If a disk already exists, the VM boots directly from the local disk drive while keeping the ISO attached in the secondary slot for package retrieval or repairs.

### Phase 4: Hardware Detection & Profiling
The pipeline queries the host hardware configuration dynamically:
- Automatically detects total CPU cores and system RAM.
- Allots an optimized subset (typically 50% up to a maximum threshold) to the VM.
- Resolves the correct CPU argument (defaulting to host-passthrough using the `WHPX` hypervisor on Windows).

### Phase 5: Launching the Virtual Machine
Based on the target's `OsFamily` (Linux, Windows, macOS) and `Firmware` metadata, the engine builds tailored QEMU launching configurations:
- **Linux:** Uses high-performance paravirtualized `virtio` disk interfaces and VGA graphics.
- **Windows:** Emulates legacy `ide` drives, utilizes modern `std` VGA display drivers, and mounts Open Virtual Machine Firmware (`OVMF` / UEFI) and a virtual TPM if Windows 11 is selected.
- **macOS:** Boots experimental configurations utilizing Penryn CPU profiles and special guest machine interfaces via the KVM-OpenCore shim.

---

## Prerequisites & Configuration

1. **QEMU for Windows:** QEMU must be installed at `C:\Program Files\qemu`.
2. **Windows Hypervisor Platform (WHPX):** To run VMs at native speeds, enable WHPX:
   - Run PowerShell as Administrator:
     ```powershell
     Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform
     ```
3. **GnuPG (Optional):** To verify target signatures (like Arch/CachyOS) on the first run, install `gpg` and ensure it is available on your System `PATH`.

---

## Docker Deployment & Containerization

Castle VM can run completely containerized inside Docker. In this mode, virtual machine displays are securely exposed via an in-container VNC server bridged to a web-accessible NoVNC gateway, allowing you to access the graphical installation interface directly from any web browser on the host.

### 1. Install Docker Desktop (Windows)
If Docker is not currently installed, you can install Docker Desktop from an Administrator PowerShell prompt using Windows Package Manager:
```powershell
winget install Docker.DockerDesktop
```
*Note: Restart your machine after installation to finalize WSL2 integrations.*

### 2. Build the Docker Image
Navigate to the `qemu/` folder containing the `Dockerfile` and run:
```bash
docker build -t castle-vm .
```

### 3. Run the Container (Interactive Menu)
Launch the container interactively. We mount a host directory to `/app/data` to ensure downloaded ISOs, virtual disks, and the trust store persist between container runs:
```bash
docker run -it --device /dev/kvm -p 8006:8006 -v "$(pwd)/data:/app/data" castle-vm
```
- Open `http://localhost:8006/vnc.html` in your web browser.
- Select your target (e.g. CachyOS) and confirm boot.
- The VM display will immediately render inside the web page.

### 4. Run a Direct Background Instance
You can bypass the menu and spin up an isolated background VM node directly:
```bash
docker run -d --device /dev/kvm -p 8006:8006 -v "$(pwd)/data:/app/data" castle-vm cachyos -Instance docker-node-01 -Vnc
```

> [!TIP]
> **Performance Tip:** The `--device /dev/kvm` flag enables Linux KVM hardware acceleration inside the container. If this device is missing (e.g. nested virtualization is disabled in WSL2), the engine automatically falls back to software emulation (`tcg`), which runs slower.

---

## Prerequisites & Configuration

1. **QEMU for Windows:** QEMU must be installed at `C:\Program Files\qemu` (for native host execution).
2. **Windows Hypervisor Platform (WHPX):** To run VMs at native speeds on Windows, enable WHPX:
   - Run PowerShell as Administrator:
     ```powershell
     Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform
     ```
3. **GnuPG (Optional):** To verify target signatures (like Arch/CachyOS) on the first run, install `gpg` and ensure it is available on your System `PATH`.

---

## Troubleshooting

- **Ambiguous Target ID:** If you type a fuzzy name (e.g. `ubuntu`) that matches both `ubuntu` (Desktop) and `ubuntu-server`, the CLI will prompt you to be more specific.
- **WHPX Accelerator Error:** If QEMU fails to boot saying it cannot initialize WHPX, check if Hyper-V or another hypervisor (like VirtualBox or VMware) is locking host virtualization resources, or run the optional feature enablement command above.
- **Boots back to installer:** If a VM keeps booting to the ISO install disk after installation, it is because it is still in the "First Boot" stage or you have deleted/moved the virtual disk. The pipeline determines boot priority automatically based on the existence of the `data/<target-id>.qcow2` file.
- **NoVNC screen is blank:** Ensure the VNC parameter (`-Vnc`) is active or passed when invoking the container. QEMU must run headlessly with a VNC display bound to bridge with NoVNC.
