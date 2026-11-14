# Castle Clone Factory -- QEMU Provisioning Pipeline

Castle Clone Factory is a modular, zero-dependency, automated PowerShell virtual machine pipeline for Windows. It enables developers to query, stream, cryptographically verify, provision, and boot target operating systems in QEMU with a single command.

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
.\start.ps1 ubuntu-cloud
.\start.ps1 win11-pro
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

### Download-Only Firmware Targets
Some targets are retro-handheld firmware images (ARM), which cannot boot in the x86 QEMU pipeline. For these, the pipeline downloads, cryptographically verifies, and caches the image in `app/data/clonewars/`, then stops before disk provisioning — flash the verified file to an SD card (e.g. with balenaEtcher/Rufus):
```powershell
.\start.ps1 minui     # MinUI launcher (Anbernic/Miyoo/Trimui) -- base .zip
.\start.ps1 knulli    # Knulli CFW (Batocera-based) -- RG34XX SD image
```
Both resolve their versions from GitHub Releases and prompt you to pick one. Knulli is verified against its published `.sha256`/`.md5` checksum assets plus the GitHub API asset digest; MinUI publishes no checksum files, so the GitHub API SHA256 asset digest is used. To fetch a different Knulli device image, edit the `ResolverAssetRegex` device slug in `modules/targets.ps1` (e.g. `rg35xx-sp`, `rg-cubexx`, `trimui-brick`).

### Cloud-Image Targets (no installer, boots straight to a configured box)
`ubuntu-cloud` (headless server) and `ubuntu-cloud-desktop` (GNOME + Wine, auto-login, launches your Windows apps) download the official Ubuntu 24.04 cloud image, verify it against Canonical's `SHA256SUMS` (then pin it), and boot a linked clone of it with **cloud-init** doing the setup on first boot — no installer, no prompts:
```powershell
.\start.ps1 ubuntu-cloud              # SSH-ready in about a minute
.\start.ps1 ubuntu-cloud-desktop      # first boot installs GNOME + Wine (~1.5 GB), reboots once, lands on the desktop with your apps running
```
- Login is `castle` / `wearedogs` (same convention as the CachyOS scripts); SSH is port-forwarded from the host and the port is printed at boot (`ssh castle@127.0.0.1 -p 2222`).
- The profiles live in `scripts/cloud-init/<server|desktop>/user-data`; edit them to change packages, users, or first-boot commands. A per-VM `meta-data` (instance id + hostname) is generated next to the disk under `app/data/clonewars/seeds/`, so every `-Instance` clone gets its own hostname.
- The seed is served to the guest as a read-only FAT volume labelled `CIDATA` straight from that folder — no ISO tooling needed on the host.
- Both targets share one cached image (`app/data/clonewars/ubuntu-noble-cloudimg-amd64.img`); VM disks are keyed by target id (`app/data/clonewars/ubuntu-cloud.qcow2`).
- Cloud targets boot UEFI (OVMF): on this host's WHPX the legacy SeaBIOS/GRUB path hangs at "Booting from Hard Disk", the EFI path does not. All guests run plain WHPX with the default in-hypervisor irqchip: `kernel-irqchip=off` made the EndeavourOS live ISO take 30+ minutes on "Rebuild Dynamic Linker Cache" instead of reaching the desktop in ~2.5 minutes (`CASTLE_VM_IRQCHIP=off` re-enables it for experiments).

#### Scratch disk, silent installers
`-Copy` also attaches a sparse qcow2 (`app/data/clonewars/scratch/<target>-scratch.qcow2`, grows only with use) as a second virtio disk with serial `castlescratch`. The guest scripts format it once (ext4, and only a disk with that serial is ever touched), then keep `~/castle-copy`, `~/.wine` and `~/opengoal` on it. Without it a live ISO holds all of that in RAM, which is what made an 8 GB guest OOM-kill the OpenGOAL compiler while a Wine installer was running. It persists across boots, so a second run finds the game already built and the apps already installed. `-DeleteDisk` / `-Purge` remove it with the target's disk; `CASTLE_VM_SCRATCH=0` keeps everything in RAM.

Installers in the payload are handled without clicking: the Wine flow sniffs each file (Inno Setup, NSIS, InstallShield, `.msi`), runs it with that family's silent switches, waits for it, and then launches the Start Menu shortcut it created, so a setup `.exe` ends with the installed program on screen. A silent run that fails falls back to the installer's own wizard. Plain programs just start.
#### Drop files in, get Wine and your programs running (EndeavourOS)
`app/data/clonewars/copy/` is a payload folder. Whatever is in it rides into the VM as a read-only CD-ROM labelled `CASTLECOPY` (an ISO image built on the host, any size), and one flag does the rest. `-Setup auto` is the default: it runs the Wine flow when the payload holds any `.exe`/`.msi` (installers included, they run like on Windows) and the OpenGOAL flow when a PS2 disc ISO is present, in that order:

```powershell
.\start.ps1 endeavouros -Copy                    # boot, copy app/data/clonewars/copy -> ~/castle-copy, then whatever the payload needs (-Setup auto)
.\start.ps1 endeavouros -c -Setup wine           # force the Wine flow only: install Wine, run every .exe/.msi in the payload
.\start.ps1 endeavouros -c -Program Setup.exe    # same, but run only that one (a name inside app/data/clonewars/copy, or a path in the guest)
.\start.ps1 endeavouros                          # interactive: asks the same question whenever app/data/clonewars/copy is not empty
```

`scripts/setup-wine.sh` (on the `CASTLE` share) enables `[multilib]`, installs `wine wine-gecko wine-mono winetricks`, copies the share into the desktop user's home (`~/castle-copy`, writable, so installers can write next to their exe) and starts the programs inside that user's desktop session. The host never waits for a login: the EndeavourOS ISO ships `qemu-guest-agent` and the launcher already attaches its socket, so `start.ps1` waits for the agent to answer (up to 15 min), runs the script as root through it (`modules/guest.ps1`) and streams the log into your console. No guest agent (another distro, or an installed system without it)? The pipeline prints the one-liner to run inside the VM instead:

```bash
sudo mount -o ro /dev/disk/by-label/CASTLE /mnt && sudo bash /mnt/setup-wine.sh --copy --all
```

Measured on this host: QEMU start to 7-Zip opening under Wine in about 80 seconds (direct boot, agent answering ~20 s in, 1 GB Wine install from the Worldwide mirrors). Two live-ISO quirks the script handles: the ISO ships `/etc/pacman.d/mirrorlist` with every server commented out (pacman says "no servers configured for repository"), so it enables the Worldwide block when nothing is active; and the root overlay is RAM, so it grows the archiso cow space to 8G first. Install EndeavourOS to the disk and the same flag works against the installed system, where Wine and `~/castle-copy` persist.

#### Jak and Daxter from scratch: OpenGOAL
`-Setup opengoal` swaps the Wine script for `scripts/setup-opengoal.sh`. Put the game disc `.iso` (a plain ISO9660 dump; unzip it) and, optionally, the official `opengoal-linux-<ver>.tar.gz` in `app/data/clonewars/copy`, then:

```powershell
.\start.ps1 endeavouros -c -Setup opengoal      # boot, mount the disc, extract + decompile + compile, launch the game
```

How the payload gets in: `.iso` files in `app/data/clonewars/copy` are attached to the VM as extra CD-ROMs (`/dev/sr1..`, any size; commas in the file name are escaped for QEMU). Everything else is packed into `app/data/clonewars/seeds/copy-share.iso` with Windows' built-in IMAPI2 (ISO9660 + Joliet + UDF, exactly the size of its contents, a 1.3 GB payload takes about 3 s) and attached as another CD-ROM labelled `CASTLECOPY`; there is no size cap and reads run at disk speed. A zip that only wraps an ISO already extracted next to it is left out. On a non-Windows host the old virtual-FAT share is used instead (fixed 504 MB, over-cap files skipped). In the guest the script finds the disc by its `SYSTEM.CNF`, puts a writable overlayfs on top of the read-only mount (the extractor writes a `buildinfo.json` next to the game files), unpacks the release into `~/opengoal`, runs `extractor --extract/--decompile/--compile --proj-path data --folder <disc>` as the desktop user, and starts `gk -- -boot -fakeiso` in the KDE session with `LP_NUM_THREADS` set to every core. Without a tarball in the payload it fetches the latest Linux release from GitHub. Measured: disc detected as "Jak & Daxter: The Precursor Legacy (Black Label), NTSC-U", extract 1 s, decompile + compile about 40 s on 14 cores, Daxter on the Naughty Dog splash within ~3 minutes of `start.ps1`.

Sound: every VM now carries an Intel HDA codec on the first host backend the QEMU build lists (Windows: DirectSound, then SDL; `CASTLE_VM_AUDIO=none` or a backend name overrides). PipeWire in the guest picks it up as the default sink and the script sets it to 100% (the live session starts at 40%); use the host mixer as the volume knob. Graphics are Mesa llvmpipe (software OpenGL 4.6; OpenGOAL needs 4.3). `-Gl` / `CASTLE_VM_GL=1` tries virtio-vga-gl with an SDL OpenGL display, but on this host's QEMU 11 build that deadlocks QEMU before the guest boots (QMP stops answering, one core spins), so it stays opt-in. The script is uploaded through the guest agent each run, so editing `scripts/setup-*.sh` takes effect on a running VM without a reboot.
The EndeavourOS target also boots the ISO's "All GPUs" entry directly (`DirectBoot` in `targets.ps1`): the kernel and initramfs are pulled out of the ISO once with 7-Zip into `app/data/clonewars/kernels/<iso>/` and handed to QEMU with that entry's own command line, so the 15-second boot menu never shows, and `systemd.mask=ldconfig.service` is appended so archiso's "Rebuild Dynamic Linker Cache" step (a needless ~19 s on every live boot, 30+ min under `kernel-irqchip=off`) is skipped. `CASTLE_VM_DIRECTBOOT=0` brings the menu back; without 7-Zip on the host the menu simply shows as before.
#### Wine apps from a folder, from scratch, fast
Put any Windows `.exe` files in `scripts/wine-apps/` (git-ignored). That folder is on the read-only USB share every VM gets (FAT label `CASTLE`), and the desktop profile launches every `.exe` in it under Wine (64-bit and 32-bit) at login, with the Mono/Gecko download prompts suppressed. Windows apps like to write next to their exe, so the launcher copies them into `~/wine-apps` first.

The fast path is **build once, clone many**:
```powershell
.\start.ps1 ubuntu-cloud-desktop                      # 1. golden build: verified image -> GNOME + Wine (~10 min, one reboot). Shut it down when it reaches the desktop.
.\start.ps1 ubuntu-cloud-desktop -Instance job-01     # 2. seconds: linked clone of the golden disk, boots straight to the desktop with the apps up
.\start.ps1 ubuntu-cloud-desktop -Instance job-02 -Background
```
An `-Instance` of a cloud target automatically clones the target's finished disk (`app/data/clonewars/ubuntu-cloud-desktop.qcow2`) when it exists and falls back to the raw image otherwise. Clones re-run only cloud-init's cheap per-instance steps and skip the first-boot reboot. Never boot the golden disk while clones depend on it. To rebuild the golden image (new profile, new base image), delete `app/data/clonewars/ubuntu-cloud-desktop.qcow2` and its `.vars.fd`, then run step 1 again. Dropping a new `.exe` in the folder needs no rebuild: it is picked up at the next login of any VM.

### Launch profiles: one file for target, size, payload, installs and commands
A profile says what a VM is for, so a launch is one short command instead of a long one plus a hand-written setup script:

```powershell
.\start.ps1 -Profile list                          # what is there
.\start.ps1 -Profile penguin                       # profiles\penguin.psd1
.\start.ps1 -Profile penguin -Target endeavouros   # the command line wins over the file
.\start.ps1 -Profile C:\somewhere\mine.psd1        # any path
```

A profile is a PowerShell data file (`.psd1`, plain data, nothing runs when it is loaded). Every key is optional:

```powershell
@{
    Description  = "What this VM is for"
    Target       = "debian-x11"                 # target id from modules/targets.ps1
    Instance     = "job-01"                     # own disk under app\data\clonewars\instances
    Background   = $true
    MemoryGB     = 4                            # exact guest RAM (no 4 GB floor)
    Cores        = 6
    Screen       = "1440x900"                   # guest screen mode (X11 guests)
    QemuArgs     = @("-rtc", "base=localtime")  # extra raw QEMU arguments, appended last
    Env          = @{ CASTLE_VM_AUDIO = "none" } # CASTLE_* knobs for this launch only
    Payload      = @(                           # host folders packed into app\data\clonewars\copy before boot
        @{ Source = "..\..\..\..\my-project"; As = "my-project.zip"; Include = @(".env") }
    )
    Packages     = @{ apt = @("htop"); pacman = @("htop") }   # the guest's own package manager picks its list
    RootCommands = @("timedatectl set-timezone UTC")          # as root, in order; a failure stops the setup
    UserCommands = @("xterm -e htop")                         # as the desktop user, inside the X11 session
    Setup        = "penguin"                    # then scripts\setup-penguin.sh; without it: setup-profile.sh
}
```

- **Payload.** `Source` is relative to the profile file. A git checkout is packed as git sees it (tracked and untracked files, ignored ones left out), so virtualenvs, build output and recordings stay behind; `Include` adds ignored files by name. Other folders are packed whole.
- **How it reaches the guest.** `MemoryGB`, `Cores`, `QemuArgs` and `Env` shape the QEMU launch. Packages, commands and `Screen` are written to `app\data\clonewars\seeds\profile\castle-profile.env`, uploaded through the guest agent as `/tmp/castle-profile.env`, and applied by `castle_profile_apply` in `scripts/castle-lib.sh`.
- **Setup scripts stay small.** `castle-lib.sh` carries what every setup needs on both Arch-based and Debian-based guests: `castle_wait_cloud_init`, `castle_user`, `castle_pkg_install` / `castle_need` (pacman or apt), `castle_mount_label`, `castle_wait_x11`, `castle_on_x`, `castle_in_session`, `castle_ensure_screen`. `scripts/setup-penguin.sh` is the worked example; `scripts/setup-profile.sh` is the generic one that only applies the profile.
- `CASTLE_VM_RAM_GB` and `CASTLE_VM_CORES` set the same exact sizes without a profile.

#### `debian-x11`: the light graphical guest
Debian 13's cloud image with Xorg + Openbox (no desktop environment), Firefox, Python and the guest agent, installed by cloud-init on first boot (`scripts/cloud-init/x11/user-data`). It auto-logs in as `castle` into a real X11 session, which screen readers and input senders (mss, pynput, xdotool) need. First boot installs the packages and starts the desktop without a reboot; later boots and `-Instance` clones skip that. It takes `-Copy`, `-Setup` and `-Profile` like `endeavouros`, and its disk persists, so nothing is re-installed per boot the way a live ISO does.

#### Example: the Cart Surfer bot, hands-off
`profiles\penguin.psd1` packs the `penguin-autosurf` checkout next to this repo (plus its `.env`), boots `debian-x11` in the background, and `scripts/setup-penguin.sh` unpacks the project, builds its Python environment, opens Firefox on the game and starts the bot, which signs in and plays on a loop:

```powershell
.\start.ps1 -Profile penguin
```

### Android: run apps / APKs locally
Two targets, pick by what you need:

| | `android-avd` (Google's emulator) | `blissos` (BlissOS 16, QEMU guest) |
|---|---|---|
| Android | 15, Google Play image | 13, GApps |
| Runs ARM-only apps | yes (Google's ARM translation) | depends on the build's native bridge |
| Graphics | host GPU (the emulator's own renderer) | software (SwiftShader); virgl hangs QEMU on this host |
| Lives in | `app/data/clonewars/android-sdk/` (SDK + AVD, nothing in `%LOCALAPPDATA%`) | `app/data/clonewars/Bliss-*.iso` + an ext4 `/data` disk next to it |
| Needs | Java 17+ once (Google's `avdmanager`), WHPX/KVM | WSL Ubuntu once (formats the ext4 disk), WHPX/KVM |

```powershell
.\start.ps1 android-avd            # first run: SDK + Android 15 image (~2.5 GB), creates the AVD, boots to the home screen (~95 s cold, seconds after that via Quick Boot)
.\start.ps1 android-avd -Copy      # same, then adb-installs every .apk in app\data\clonewars\copy and launches it
.\android.ps1 -Apk C:\path\app.apk # install + launch one APK into the running emulator
.\android.ps1 -Stop                # clean shutdown (snapshot saved) ; -Wipe = factory reset ; -Purge = delete the AVD

.\start.ps1 blissos -Background    # verified ISO (SourceForge SHA256), straight into Android, no installer
```
- **Downloads and checks.** `android.ps1` reads the cmdline-tools zip name and SHA-1 from Google's SDK manifest over HTTPS, checks the zip and pins its SHA-512 in `.castle_trust.json`. The emulator, platform-tools and system image are installed by Google's own tool, which checks them against the same manifests. Installing them means accepting the [Android SDK License](https://developer.android.com/studio/terms): it asks once, or pass `-AcceptLicenses` (`CASTLE_ACCEPT_ANDROID_LICENSES=1`). Since cmdline-tools 23, `sdkmanager` forwards to Google's new "Android CLI", which announces usage metrics; `android.ps1` calls it directly with `--no-metrics`. Pick another image with `CASTLE_AVD_IMAGE` (e.g. `system-images;android-36;google_apis_playstore;x86_64`) and the RAM with `CASTLE_AVD_RAM_MB` (default 4096).
- **Network.** Both get internet through the host's user-mode NAT; inside Android the host is `10.0.2.2`. The AVD shows up in `adb devices` as `emulator-5554`. BlissOS forwards its adbd to a free loopback port printed at boot (`adb connect 127.0.0.1:5655`); use `app\data\clonewars\android-sdk\platform-tools\adb.exe` once the AVD target has installed it.
- **BlissOS boot.** Every boot loads the ISO's kernel/initrd directly with its "No HW Acceleration" entry (`nomodeset HWACCEL=0`) plus `DATA=vda SETUPWIZARD=0`. The VM disk is not installed to: it is a bare ext4 volume (formatted once through WSL, then never touched again) mounted as `/data`, so apps and settings persist. Without WSL it still boots, with `/data` in RAM. First boot is slow (several minutes while Android optimises apps under software rendering).

### Containers: the light engine for automations (`container.ps1`)
A VM boots a kernel, an init system and a desktop for every bot, and holds a fixed share of memory whether the bot uses it or not. A container shares the Docker engine's kernel and starts only what the bot needs. Two container targets, one image each, built once and shared by every container made from them:

| | `container-debian` | `container-android` |
|---|---|---|
| What runs | Debian 13 + an in-memory X server (Xvfb, 1440x900 or the profile's `Screen`) + Openbox | Google's Android Emulator, headless, on KVM; Android 15 `google_apis` (root tasks work) |
| Setup | the same `scripts/setup-*.sh` and launch profiles as the VMs (`castle-lib.sh` knows it is in a container) | every `.apk` in the payload installed and opened, other files pushed to `Download/castle-copy`, `castle-tasks.sh` started on the phone |
| Image | `docker/automation/`, about 750 MB | `docker/android/`, SDK + system image, a few GB |
| Measured | penguin-autosurf (Firefox + the Python bot) up and playing in about 60 s from a fresh container, 9 s from a kept one; an idle box is 46 MB | |

```powershell
.\start.ps1 -Profile penguin -Target container-debian -Instance penguin-1   # the Cart Surfer bot, in a container
.\container.ps1 container-debian -Instance job-01            # a plain container machine (castle-job-01)
.\container.ps1 container-debian -Instance job-01 -Stop      # stop it; it is kept, the next start resumes it
.\container.ps1 container-debian -Instance job-01 -Remove    # delete it, its "disk" with it
.\container.ps1 container-android -Instance phone-1 -Copy    # Android: APKs from app\data\clonewars\copy installed and opened
.\container.ps1 -List
```
- **A container is a machine.** It is called `castle-<instance>` (`castle-<target>` without one). Its writable layer is its disk: what a setup installs stays, so the second start skips it. `CASTLE_VM_SNAPSHOT=1` (a temporary run from the app) makes a throwaway that is removed when it stops. Editing anything under `docker/<kind>/` gives the image a new tag, and a kept container made from an older image is made again on its next start.
- **What goes in, read-only.** Before every start the payload and the launch profile are staged into `app/data/clonewars/containers/<name>/` and mounted read-only, the scripts folder too. Nothing on the host is writable from inside. Memory and CPU are Docker limits (`MemoryGB`, `Cores`), not reservations.
- **Seeing it.** `docker exec <name> castle-screen` writes a PNG of the screen (the Castle app shows these, once a second). **Take control** in the app runs `castle-live start` inside the container: x11vnc plus a noVNC web viewer behind a new one-time password, on a port published to the host's `127.0.0.1` only. Nothing listens on it until then, and closing the viewer turns it off.
- **No console windows.** The Castle box runs these scripts without a console, so every `docker`, `wsl`, `git` and `adb` call goes through `Invoke-Quiet` (`modules/quiet.ps1`), which starts it without a window. A plain `& docker ...` in a script the box runs opens a console window per call.
- **Needs.** Docker Desktop on Windows (started minimized if it is installed but not running) or any Docker engine on Linux. `container-android` also needs KVM in the engine, which `container.ps1` sets up in Docker Desktop's VM before every start like `docker.ps1` does, and the Android SDK license already accepted on this box (`.\android.ps1 -InstallOnly` asks once; the license file is copied into the build).

### Android automations
An automation on an Android target (`android-avd`, `container-android`) runs on the phone, never on the box. Each bot gets its own phone: an AVD of its own (`castle-<automation>-<bot>`, made by `android.ps1 -Name`) or a container of its own. The project's APKs are installed and opened, its other files land in `Download/castle-copy` (its `.git` and `.env` stay behind), and the tasks become `castle-tasks.sh`: the bot's fields as variables, then the tasks in order as commands for the phone's own shell (`am start ...`, `input tap 540 1200`, `input text ...`, `sleep 5`), looped when the automation restarts. ROOT tasks run through `su 0`, which the container's `google_apis` image has and the Play Store emulator does not. The script runs detached and logs to `/data/local/tmp/castle-automation.log` on the phone.

### Boot any ISO you already have (`boot-iso.ps1` / `boot-iso.sh`)
Outside the manifest: point the script at any bootable ISO (EndeavourOS, Ubuntu, Kali, Arch, Windows ...) and a disk size, and it boots it in QEMU with a fresh qcow2 of that size under `app/data/clonewars/adhoc/`. First run boots the installer from the ISO; later runs boot the installed disk with the ISO still attached as a CD. Same QEMU setup as the pipeline (WHPX/KVM with TCG fallback, virtio, OVMF with per-VM NVRAM, the `CASTLE` USB share, QMP and guest-agent sockets on Windows), plus knobs on top:
```powershell
.\boot-iso.ps1 ..\data\clonewars\endeavouros-Titan-Nova-2026.08.15.iso 10G                       # 5G / 10gb / 15GB ... all accepted
.\boot-iso.ps1 C:\isos\kali-linux-2026.3-installer-amd64.iso 20G -Vga virtio -Audio -PortForward 2222:22 -Share C:\stuff
.\boot-iso.ps1 ..\data\clonewars\endeavouros-Titan-Nova-2026.08.15.iso 5G -Snapshot -Fresh -Firmware uefi -Machine q35 -Cores 4 -Memory 4G
.\boot-iso.ps1 <iso> 10G -DryRun                                                      # print the QEMU command line, launch nothing
```
```bash
./boot-iso.sh ~/isos/EndeavourOS_Titan-Nova-2026.08.15.iso 15gb --vga virtio --audio --forward 2222:22,8080:80 --background
```
- **Knobs** (same names in both scripts): disk `-Name`/`-Disk`/`-Fresh`/`-BootFrom cd|disk`/`-Snapshot` (writes discarded at exit, good for live ISOs); machine `-Firmware bios|uefi`, `-OsFamily linux|windows|macos`, `-Machine pc|q35`, `-Cpu`, `-Cores`, `-Memory`, `-Accel auto|whpx|kvm|tcg`; devices `-Vga std|virtio|qxl|vmware|cirrus|none`, `-Display gtk|sdl|none`, `-Audio`, `-PortForward host:guest[,...]`, `-NoNet`, `-Share <folder>` (read-only USB stick labelled `SHARE`), `-LocalTime`, `-ExtraArgs` (raw QEMU arguments); run `-Background`, `-Vnc`, `-DryRun`.
- **Optional verification.** Give `-Sha512` / `-Sha256` (a hex digest, a `.sha512sum`/`.sha256`/`SHA256SUMS`-style file, or a URL to one) and/or `-Sig` (a detached `.sig`/`.asc`, of the ISO or of the checksum file as Ubuntu and Kali sign `SHA256SUMS.gpg`) and the ISO is checked before it boots. Files sitting next to the ISO under those usual names are picked up automatically (`-NoVerify` turns that off). Nothing is verified unless something was given or found; a requested check that fails refuses to boot unless `-Force`. A signature whose key is missing is fetched by key id from the keyserver (`-GpgServer`, default keys.openpgp.org; `-GpgKey` imports a specific key first) and the fingerprint is printed so you can compare it with the distro's published one. Every remote fetch shows its `[DNS]` endpoint line. A `gpg` that is on PATH but does not run (some toolbox stubs) counts as absent: the signature check is skipped, not failed.

### 4. Swarm Mode & Multi-Instance Boot
To spin up multiple separate instances of the same target OS simultaneously, or run background jobs:
- **Asynchronous Background Launch (`-Background`):**
  Launch a VM process in the background and return immediately to the shell (uses the windowed QEMU build, so no console window is left behind):
  ```powershell
  .\start.ps1 cachyos -Background
  ```
- **Isolated Custom Instance Name (`-Instance`):**
  Create a distinct VM disk under `app/data/clonewars/instances/<instance-id>.qcow2` to run multiple distinct nodes side-by-side:
  ```powershell
  .\start.ps1 cachyos -Instance node-01
  .\start.ps1 cachyos -Instance node-02
  ```
- **Instant Linked Clones (`-BaseDisk`):**
  Boot an instance backed by a read-only base disk template to save massive disk space and enable sub-second provisioning:
  ```powershell
  .\start.ps1 cachyos -Instance node-03 -BaseDisk ..\data\clonewars\cachyos.qcow2
  ```
  This is also the fast path to a Windows desktop: install `win11-pro` **once** (the unattended install is the slow part), then never install again -- every further machine is a linked clone of that finished disk and boots straight to the desktop in well under a minute. A UEFI base's NVRAM (`.vars.fd`) is copied alongside the clone so its boot entry is intact:
  ```powershell
  .\start.ps1 win11-pro -Instance dev-01 -BaseDisk ..\data\clonewars\win11-pro.qcow2 -Background
  ```
  Never boot the base disk again while clones depend on it -- a write to the base corrupts every clone. Treat `app\data\clonewars\win11-pro.qcow2` as the golden image once it is installed.
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
clonewars/
├── start.ps1              # Core entry point, CLI loop, and pipeline runner
├── README.md              # Technical pipeline documentation (this file)
├── qemu-notes.md          # Architectural research notes
├── modules/               # Modular script handlers
│   ├── hardware.ps1       # CPU cores, RAM configuration, and WHPX detection
│   ├── launch.ps1         # QEMU profile builder and launch engine
│   ├── network.ps1        # Large file networking streams and progress bar
│   ├── resolvers.ps1      # GitHub API and HTML directory version parsers
│   ├── targets.ps1        # Target registry & configuration manifest
│   └── verify.ps1         # Multi-algorithm crypto and trust pinning implementation
└── scripts/               # Guest provisioning assets (attached to every VM as a USB drive)
    ├── cachyos/           # CachyOS post-install provisioner + smoke test
    └── cloud-init/        # NoCloud user-data profiles for the cloud-image targets
```

Everything the scripts download or create lives outside this folder, in `app/data/clonewars/`
(git-ignored; `CASTLE_CLONEWARS_DATA` points them elsewhere):

```text
app/data/clonewars/
├── *.iso                 # Cached installer images
├── *.qcow2               # VM virtual storage disks
├── .castle_trust.json    # Local trust pinnings database
├── copy/                 # Payload that rides into a VM with -Copy (about 500 MB at most)
├── instances/            # -Instance disks
├── scratch/, seeds/      # Per-launch scratch disks and cloud-init seeds
├── containers/           # What each container gets, staged before every start
└── android-sdk/          # The Android Emulator's private SDK and AVDs
```

### Guest Provisioning Scripts

Every VM boot attaches `clonewars/scripts/` as a **read-only USB drive** inside the guest, so provisioning assets are reachable with zero networking:

- `scripts/cloud-init/<server|desktop>/user-data` — cloud-init profiles for the `ubuntu-cloud*` targets (see *Cloud-Image Targets* above). These are the from-scratch, zero-touch path.
- `scripts/cachyos/autoinstall-cachy.{sh,yml}` — experimental CachyOS install driver + config. Current CachyOS media does not ship the `cachyos-installer --cli` it calls, so this is not yet unattended.
- `scripts/cachyos/main-install.sh` — post-install provisioner implementing the `qemu-notes.md` checklist (WireGuard, Tailscale, `ufw` default-deny + SSH allow, Samba, OpenSSH, Docker, `qemu-guest-agent`, btrfs tooling). Run with sudo inside the guest; set `CASTLE_INSTALL_OLLAMA=1` to also install Ollama.
- `scripts/cachyos/test.sh` — smoke test verifying the provisioner (binaries + enabled services).

---

## Core Pipeline Architecture

When a target is chosen, the engine executes a sequential 5-phase pipeline to transition the OS target from a remote URL into a running VM.

### Phase 1: Dynamic Version Discovery & ISO Acquisition
1. If the target has a `ResolverType` (e.g. `HtmlDirectory`, `GitHub`, or `GitHubAsset`), the engine queries the remote server to find available release versions and prompts you to select one. `GitHubAsset` targets additionally resolve the concrete download asset from the release's asset list by regex — needed when asset filenames embed dates or codenames that do not match the release tag — and automatically wire up any companion `.sha256`/`.sha512`/`.md5` checksum assets plus the SHA256 digest GitHub computes for every release asset.
2. If the chosen ISO is not found in `app/data/clonewars/`, the pipeline initiates a secure download, streaming it chunk-by-chunk to prevent memory leaks and showing a console progress bar.
3. **Mirror selection:** targets that declare a mirror list get a mirror prompt before version discovery, grouped by region, with the target's `MirrorDefault` preselected so Enter behaves as before. The list is scraped live from the distro's download page (`MirrorPageUrl` + `MirrorSectionRegex`/`MirrorRowRegex`, named groups `region`, `country`, `name`, `url`) and falls back to the `Mirrors` snapshot in `modules/targets.ps1` when the page is down or its markup changed. A mirror's base is the directory of its ISO link; the checksum and GPG signature are fetched from that same base, so the ISO, `.sha512sum`/`.sha256` and `.sig` always come from the mirror you picked (the GPG key still comes from the keyserver). Every URL in such a target carries `$m` for the mirror base. Skip the prompt with `.\start.ps1 endeavouros -Mirror tuna` (list number, host, name or country substring) or `CASTLE_MIRROR=...`. Wired up for `endeavouros` (26 mirrors from https://endeavouros.com/download/); the mechanism is generic and other targets get it by adding the same keys.
4. **Endpoint transparency:** every remote fetch (ISO, checksum manifests, GPG signature and keyserver, resolver queries) is preceded by a `[DNS] host -> ip (via <dns server>)` line. The address is never hardcoded: it is resolved at run time by querying the DNS server the host is actively using (the DNS client of the interface that owns the default route on Windows, the first `nameserver` in `/etc/resolv.conf` on Linux), bypassing the hosts file and resolver cache. If that query fails, the OS resolver is used and the line says so.

### Phase 2: Cryptographic Verification (Trust Pinning System)
To ensure the pipeline is "sealed from madness," every image must pass rigorous integrity verification:
1. **Trust Pinning:** Upon the first successful validation of a download (authenticated via remote SHA hash files or GPG signatures), the target's computed cryptographic signature is saved to the local trust store (`app/data/clonewars/.castle_trust.json`).
2. **Offline Pinning:** On subsequent boots, the pipeline verifies the ISO against this local database. This prevents man-in-the-middle attacks, DNS poisoning, and local bit-rot, allowing offline boots to remain cryptographically secure.
3. **Multi-Algorithm Auditing:** Supports `SHA256`, `SHA512`, `SHA384`, `SHA1`, and `MD5`. If a target specifies multiple validation paths, the engine performs multi-checksum auditing — a static/pinned expected hash and remote checksum manifests are all checked cumulatively, and every configured layer must pass.

### Phase 3: Virtual Disk Provisioning
The engine provisions a thin-provisioned copy-on-write virtual disk (`qcow2`) in the `app/data/clonewars/` folder:
- **First Boot Detection:** If no disk exists, the engine runs `qemu-img` to create one and configures the VM to boot from the installer CD-ROM.
- **Cloud Images:** For `ImageKind = "cloud"` targets the verified download *is* the disk: the VM disk is created as a linked clone backed by the image (grown to the target's `DiskSize`), no CD-ROM is attached, and the cloud-init seed folder is generated under `app/data/clonewars/seeds/`.
- **Subsequent Boots:** If a disk already exists, the VM boots directly from the local disk drive while keeping the ISO attached in the secondary slot for package retrieval or repairs.

### Phase 4: Hardware Detection & Profiling
The pipeline queries the host hardware configuration dynamically:
- Automatically detects total CPU cores and system RAM.
- Allots an optimized subset (typically 50% up to a maximum threshold) to the VM.
- Resolves the correct CPU argument (defaulting to host-passthrough using the `WHPX` hypervisor on Windows).

### Phase 5: Launching the Virtual Machine
Based on the target's `OsFamily` (Linux, Windows, macOS) and `Firmware` metadata, the engine builds tailored QEMU launching configurations:
- **Linux:** Uses high-performance paravirtualized `virtio` disk interfaces and VGA graphics on plain WHPX (`kernel-irqchip=off` is off by default: it slowed Linux boots roughly 18x; set `CASTLE_VM_IRQCHIP=off` to try it).
- **Windows:** Legacy `ide` disk and CD-ROM on the default machine type (Q35/AHCI was tried for speed but the installed system hangs on a black screen after the bootloader under WHPX; opt back in with `CASTLE_VM_MACHINE=q35`), `std` VGA, and boots UEFI targets (Windows 11) from the OVMF firmware bundled with QEMU. Once a UEFI Windows disk is installed the installer CD is no longer attached (the no-prompt unattended ISO would otherwise reinstall over it if the firmware ever picked it first).
- **UEFI boot fallback:** `scripts/startup.nsh` rides on the USB share, and OVMF's shell runs it whenever no NVRAM boot entry works. It finds and launches `\EFI\Microsoft\Boot\bootmgfw.efi` (Windows never writes the generic `\EFI\BOOT\BOOTX64.EFI` the firmware looks for) or the Ubuntu/generic loaders, so Windows disks, fresh clones and lost NVRAM entries all still boot without a keypress. The firmware is loaded as `pflash` with a per-VM writable NVRAM (`app/data/clonewars/<disk>.vars.fd`) so UEFI boot entries survive reboots; deleting a disk also deletes its NVRAM. There is no virtual TPM: the unattended `win11-*` installs apply the image from WinPE, which does not run setup's hardware checks.
- **macOS:** Boots experimental configurations utilizing Penryn CPU profiles and special guest machine interfaces via the KVM-OpenCore shim.

---

## Prerequisites & Configuration

1. **QEMU for Windows:** QEMU must be installed at `C:\Program Files\qemu` (or available on `PATH`).
2. **Windows Hypervisor Platform (WHPX):** To run VMs at native speeds, enable WHPX:
   - Run PowerShell as Administrator:
     ```powershell
     Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform
     ```
3. **GnuPG (Optional):** To verify target signatures (like Arch/CachyOS) on the first run, install `gpg` and ensure it is available on your System `PATH`. If the GPG layer fails after the checksum already matched (keyserver down, rotated key), the pipeline asks whether to continue; set `CASTLE_SKIP_GPG=1` to bypass it without a prompt. To skip integrity verification entirely (hash, GPG and trust pin -- nothing gets pinned), set `CASTLE_SKIP_VERIFY=1`, or pick **[B]ypass** at the failure prompt.
4. **Unattended Windows installs (Optional):** The `win11-home` / `win11-pro` targets build an unattended ISO from a local retail copy. Copy `.env-template` to `.env` and fill in your product keys:
   ```powershell
   Copy-Item .env-template .env
   ```
   Without `.env`, the pipeline falls back to Microsoft's generic default install keys (unactivated). `.env` is git-ignored — never commit it. Building requires `oscdimg.exe` (`winget install Microsoft.OSCDIMG`) and a sibling `phoenix` checkout containing `win-install\autounattend.xml`.

   Extract the retail ISO's contents into a folder named after the target, e.g. `app\data\clonewars\win11-pro\` (it must contain `sources\install.wim`, `.esd`, or split `.swm` files and `efi\microsoft\boot\efisys_noprompt.bin`). On the next `.\start.ps1 win11-pro` the pipeline builds `app\data\clonewars\win11-pro.iso` with the answer file injected, using the no-prompt UEFI boot sector so the install starts without a keypress, and applies the edition by name (`Windows 11 Pro`), not by image index. Install speed knobs applied automatically: dism's `/CheckIntegrity /Verify` are stripped from the apply step, and a disk that is still being installed to runs with `cache=unsafe` (flushes skipped) until the install finishes.

**Graphical UI:** `gui.ps1` launches a zero-dependency WPF front-end over the same pipeline (launch, background, VNC, disk/ISO cleanup, purge).

---

## Docker Desktops (container fast path)

The fastest route to a Linux desktop with your Windows apps running is not a VM at all: `docker.ps1` brings up an **Arch Linux + XFCE + Wine** desktop as a container and serves it to a browser tab. After the one-time image build, it is up in seconds, and every `.exe` in `scripts/wine-apps/` launches under Wine as soon as the desktop starts (the same folder the `ubuntu-cloud-desktop` VM uses).

```powershell
.\docker.ps1              # build on first run, then up; prints https://127.0.0.1:3001 and the login
.\docker.ps1 -Logs        # follow the container
.\docker.ps1 -Down        # stop (keeps the Wine prefix / app data volume); add -Purge to wipe it
```
- **Needs a Docker engine.** Docker Desktop (`winget install Docker.DockerDesktop`, reboot) or an engine inside WSL2 (`wsl -d Ubuntu -- sudo apt install -y docker.io docker-compose-v2`).
- **Secure by default:** the desktop is published on the loopback interface only and over HTTPS only (self-signed certificate, accept the warning); a login is required, generated with a random password into `docker/arch-wine/.env` (git-ignored) on first run; the container runs unprivileged with `no-new-privileges`, no host Docker socket, and the app folder mounted read-only.
- **What it is not:** a VM. The container shares the host kernel (WSL2's, on Windows), so isolation is weaker than QEMU. For anything that must be sandboxed like a separate machine, use the VM targets or the `windows` profile below.
- **Image notes:** the `webtop:arch-xfce` base ships a trimmed `pacman.conf` (no `[multilib]` block, so the Dockerfile appends one), a two-mirror list that drops TLS mid-download through Docker Desktop's NAT (fallback mirrors added, downloads serialized), and an nginx module built by linuxserver against the shipped nginx, so `nginx` is pinned during the upgrade or the web desktop crash-loops.
- Profile files live in `docker/arch-wine/` (`Dockerfile`, `compose.yml`, the Wine launcher and its autostart hook). Add more profiles as sibling folders and list them in `docker.ps1`.

### Windows 11 profile (`dockur/windows`)

Both Docker profiles are also Castle targets, so `.\start.ps1 docker-windows` / `.\start.ps1 docker-arch-wine` (or picking them in the menu, marked `[DKR]`) hands over to `docker.ps1`; `-Purge` on those targets stops the container and deletes its data volume.

`docker.ps1 -Profile windows` runs a real Windows 11 VM inside a container ([dockur/windows](https://github.com/dockur/windows): QEMU + KVM, unattended install). It is the Windows counterpart of the Arch desktop: same `scripts/wine-apps/` folder, same loopback-only publishing, same generated login.

```powershell
.\docker.ps1 -Profile windows           # first run: unattended install, ~20 min; watch it at http://127.0.0.1:8006
.\docker.ps1 -Profile windows -Logs     # follow the install log
.\docker.ps1 -Profile windows -Down     # stop (keeps the VM disk volume); add -Purge to delete it
```
- **Install media:** Microsoft's download endpoints answer 403 from some networks, so the profile installs from a local ISO bound in as `/custom.iso` via `docker/windows/compose.iso.yml`: `app/data/clonewars/win11-pro-docker.iso`, or whatever `$env:CASTLE_WINDOWS_ISO` points at. Castle's own `win11-pro.iso` will not do: it is built for a FAT32 stick (split `install.swm` parts, Castle's `autounattend.xml`), which dockur cannot detect, so it falls back to a manual install without virtio drivers and Castle's answer file dies at diskpart. `docker/windows/build-iso.ps1` merges the split image back into `install.wim` and drops the answer file, inside a throwaway `dockurr/windows` container (no host tools, no admin); `docker.ps1` runs it automatically when the ISO is missing and `app/data/clonewars/win11-pro/` exists. With no ISO at all, the container tries Microsoft's servers.
- **Your apps:** `scripts/wine-apps/` is copied into the install as `C:\OEM\apps` and `docker/windows/oem/install.bat` runs each `.exe` once at the end of setup (silent switches for Discord and Mullvad, PuTTY copied to the desktop, anything else runs as-is and may show a wizard in the web viewer; log at `C:\OEM\install.log`). The live folder is also the `Shared` desktop folder / drive `Z:`.
- **Access:** web viewer at `http://127.0.0.1:8006` (no login of its own, hence loopback only), or RDP to `127.0.0.1:3389` with the login from `docker/windows/.env`. Sizing lives in `compose.yml` (8 GB RAM, 4 cores, 48 GB disk). The disk is a Docker named volume on purpose: as a bind mount on NTFS the image loses its sparseness and all 48 GB get allocated on the host.
- **KVM on Docker Desktop:** needs Windows 11 with the WSL2 backend. Docker Desktop's VM has the `kvm_intel`/`kvm_amd` modules but does not load them or create `/dev/kvm` (and can leave a stray `/dev/kvm` directory); `docker.ps1` fixes both inside the `docker-desktop` distro before every `up`. Docker Desktop restarts undo it, which is why the script repeats it each time.
## Docker Deployment & Containerization

Castle Clone Factory can run completely containerized inside Docker. In this mode, virtual machine displays are securely exposed via an in-container VNC server bridged to a web-accessible NoVNC gateway, allowing you to access the graphical installation interface directly from any web browser on the host.

### 1. Install Docker Desktop (Windows)
If Docker is not currently installed, you can install Docker Desktop from an Administrator PowerShell prompt using Windows Package Manager:
```powershell
winget install Docker.DockerDesktop
```
*Note: Restart your machine after installation to finalize WSL2 integrations.*

### 2. Build the Docker Image
Navigate to the `clonewars/` folder containing the `Dockerfile` and run:
```bash
docker build -t castle-vm .
```

### 3. Run the Container (Interactive Menu)
Launch the container interactively. We mount a host directory to `/app/data` to ensure downloaded ISOs, virtual disks, and the trust store persist between container runs:
```bash
docker run -it --device /dev/kvm -p 127.0.0.1:8006:8006 -v "$(pwd)/../data/clonewars:/app/data" castle-vm
```
- Open `http://localhost:8006/vnc.html` in your web browser.
- Select your target (e.g. CachyOS) and confirm boot.
- The VM display will immediately render inside the web page.

### 4. Run a Direct Background Instance
You can bypass the menu and spin up an isolated background VM node directly:
```bash
docker run -d --device /dev/kvm -p 127.0.0.1:8006:8006 -v "$(pwd)/../data/clonewars:/app/data" castle-vm cachyos -Instance docker-node-01 -Vnc
```

> [!TIP]
> **Performance Tip:** The `--device /dev/kvm` flag enables Linux KVM hardware acceleration inside the container. If this device is missing (e.g. nested virtualization is disabled in WSL2), the engine automatically falls back to software emulation (`tcg`), which runs slower.

> [!NOTE]
> **Container knobs:** `entrypoint.sh` honors `NOVNC_PORT` (default `8006`) and `VNC_TARGET` (default `localhost:5900`, i.e. QEMU display `:0`). Only display `:0` is bridged to the web gateway — extra `-Vnc` instances pick free displays (`:1`, `:2`, ...) that are not web-reachable.

---

## Troubleshooting

- **Ambiguous Target ID:** If you type a fuzzy name (e.g. `ubuntu`) that matches both `ubuntu` (Desktop) and `ubuntu-server`, the CLI will prompt you to be more specific.
- **WHPX Accelerator Error:** If QEMU fails to boot saying it cannot initialize WHPX, check if Hyper-V or another hypervisor (like VirtualBox or VMware) is locking host virtualization resources, or run the optional feature enablement command above.
- **Boots back to installer:** If a VM keeps booting to the ISO install disk after installation, it is because it is still in the "First Boot" stage or you have deleted/moved the virtual disk. The pipeline determines boot priority automatically based on the existence of the `app/data/clonewars/<target-id>.qcow2` file.
- **NoVNC screen is blank:** Ensure the VNC parameter (`-Vnc`) is active or passed when invoking the container. QEMU must run headlessly with a VNC display bound to bridge with NoVNC.
