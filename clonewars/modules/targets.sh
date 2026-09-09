#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- ISO Target Manifest & QEMU Boot Profiles (bash twin)
# ==============================================================================
# Bash twin of modules/targets.ps1. The PS $IsoMatrix array of hashtables is
# a JSON array here; helpers expose it via jq. "Registry" in the PS banner
# means this manifest (the target registry) -- not the Windows Registry.
#
# Helpers:
#   cw_target_count            # -> 27
#   cw_target_json <index>     # -> target object as JSON (0-based)
#   cw_find_targets <query>    # -> matching targets, one JSON object/line
#                              #    exact Id match first, else Id/Name
#                              #    substring match (case-insensitive),
#                              #    mirroring the PS -like logic in start.ps1
# ==============================================================================

CW_ISO_MATRIX_JSON='
[
  {
    "Id": "cachyos",
    "Name": "CachyOS Linux",
    "Description": "Performance-tuned Arch-based distribution with custom kernels and desktop options.",
    "ResolverType": "HtmlDirectory",
    "ResolverUrl": "https://mirror.cachyos.org/ISO/desktop/",
    "ResolverRegex": "href=\"([^\"]+)/\"",
    "ResolverFilter": "^\\d{6}$",
    "UrlTemplate": "https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso",
    "HashUrlSha256Template": "https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso.sha256",
    "SigUrlTemplate": "https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso.sig",
    "IsoNameTemplate": "cachyos-desktop-linux-$v.iso",
    "FileTemplate": "cachyos-$v.iso",
    "GpgKey": "F3B607488DB35A47",
    "GpgServer": "hkps://keys.openpgp.org",
    "DiskSize": "40G",
    "OsFamily": "linux"
  },
  {
    "Id": "endeavouros",
    "Name": "EndeavourOS Linux",
    "Description": "Friendly, terminal-centric Arch-based distribution with a GUI installer.",
    "ResolverType": "HtmlDirectory",
    "ResolverUrl": "https://mirrors.gigenet.com/endeavouros/iso/",
    "ResolverRegex": "href=\"EndeavourOS_([^\"]+)\\.iso\"",
    "UrlTemplate": "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_$v.iso",
    "HashUrlSha512Template": "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_$v.iso.sha512",
    "SigUrlTemplate": "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_$v.iso.sig",
    "IsoNameTemplate": "EndeavourOS_$v.iso",
    "FileTemplate": "endeavouros-$v.iso",
    "DiskSize": "40G",
    "OsFamily": "linux"
  },
  {
    "Id": "arch",
    "Name": "Arch Linux Base",
    "Description": "Rolling-release base Linux system emphasizing simplicity, minimalism, and control.",
    "Url": "https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso",
    "IsoName": "archlinux-x86_64.iso",
    "File": "arch.iso",
    "HashUrlSha256": "https://geo.mirror.pkgbuild.com/iso/latest/sha256sums.txt",
    "SigUrl": "https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso.sig",
    "GpgKey": "9D26B1A84E65183D",
    "GpgServer": "hkps://keyserver.ubuntu.com",
    "DiskSize": "30G",
    "OsFamily": "linux"
  },
  {
    "Id": "alpine",
    "Name": "Alpine Linux Extended",
    "Description": "Ultra-lightweight, security-oriented Linux based on musl libc and BusyBox.",
    "Url": "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-extended-3.20.0-x86_64.iso",
    "IsoName": "alpine-extended-3.20.0-x86_64.iso",
    "File": "alpine.iso",
    "HashUrlSha512": "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-extended-3.20.0-x86_64.iso.sha512",
    "DiskSize": "10G",
    "OsFamily": "linux"
  },
  {
    "Id": "talos",
    "Name": "Talos Linux (Secure Immutable K8s)",
    "Description": "Secure, immutable, and minimal Linux OS built specifically for running Kubernetes.",
    "ResolverType": "GitHub",
    "ResolverRepo": "siderolabs/talos",
    "UrlTemplate": "https://github.com/siderolabs/talos/releases/download/$v/metal-amd64.iso",
    "HashUrlSha256Template": "https://github.com/siderolabs/talos/releases/download/$v/sha256sum.txt",
    "HashUrlSha512Template": "https://github.com/siderolabs/talos/releases/download/$v/sha512sum.txt",
    "IsoNameTemplate": "metal-amd64.iso",
    "FileTemplate": "talos-$v.iso",
    "DiskSize": "20G",
    "OsFamily": "linux"
  },
  {
    "Id": "rocky",
    "Name": "Rocky Linux Minimal (RHEL Core)",
    "Description": "Enterprise Linux distro offering 100% bug-for-bug compatibility with RHEL.",
    "ResolverType": "HtmlDirectory",
    "ResolverUrl": "https://download.rockylinux.org/pub/rocky/",
    "ResolverRegex": "href=\"([89]|10)/\"",
    "UrlTemplate": "https://download.rockylinux.org/pub/rocky/$v/isos/x86_64/Rocky-$v-latest-x86_64-minimal.iso",
    "IsoNameTemplate": "Rocky-$v-latest-x86_64-minimal.iso",
    "FileTemplate": "rockylinux-$v.iso",
    "HashUrlSha256Template": "https://download.rockylinux.org/pub/rocky/$v/isos/x86_64/CHECKSUM",
    "DiskSize": "30G",
    "OsFamily": "linux"
  },
  {
    "Id": "debian",
    "Name": "Debian NetInst",
    "Description": "Highly stable and community-driven Linux distribution, known as the Universal OS.",
    "ResolverType": "HtmlDirectory",
    "ResolverUrl": "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/",
    "ResolverRegex": "href=\"debian-([\\d\\.]+)-amd64-netinst\\.iso\"",
    "UrlTemplate": "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-$v-amd64-netinst.iso",
    "IsoNameTemplate": "debian-$v-amd64-netinst.iso",
    "FileTemplate": "debian-$v.iso",
    "HashUrlSha256Template": "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS",
    "HashUrlSha512Template": "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS",
    "DiskSize": "30G",
    "OsFamily": "linux"
  },
  {
    "Id": "ubuntu",
    "Name": "Ubuntu Desktop LTS",
    "Description": "The most widely used Linux desktop OS, backed by Canonical.",
    "ResolverType": "HtmlDirectory",
    "ResolverUrl": "https://releases.ubuntu.com/24.04/",
    "ResolverRegex": "href=\"ubuntu-(\\d+\\.\\d+(?:\\.\\d+)?)-desktop-amd64\\.iso\"",
    "UrlTemplate": "https://releases.ubuntu.com/24.04/ubuntu-$v-desktop-amd64.iso",
    "IsoNameTemplate": "ubuntu-$v-desktop-amd64.iso",
    "FileTemplate": "ubuntu-$v.iso",
    "HashUrlSha256Template": "https://releases.ubuntu.com/24.04/SHA256SUMS",
    "DiskSize": "40G",
    "OsFamily": "linux"
  },
  {
    "Id": "ubuntu-server",
    "Name": "Ubuntu Server LTS (AI Hardware Optimized)",
    "Description": "LTS server operating system optimized for container, cloud, and AI hardware workloads.",
    "ResolverType": "HtmlDirectory",
    "ResolverUrl": "https://releases.ubuntu.com/24.04/",
    "ResolverRegex": "href=\"ubuntu-(\\d+\\.\\d+(?:\\.\\d+)?)-live-server-amd64\\.iso\"",
    "UrlTemplate": "https://releases.ubuntu.com/24.04/ubuntu-$v-live-server-amd64.iso",
    "IsoNameTemplate": "ubuntu-$v-live-server-amd64.iso",
    "FileTemplate": "ubuntu-server-$v.iso",
    "HashUrlSha256Template": "https://releases.ubuntu.com/24.04/SHA256SUMS",
    "DiskSize": "40G",
    "OsFamily": "linux"
  },
  {
    "Id": "vyos",
    "Name": "VyOS Network Router (LTS)",
    "Description": "Debian-based network routing, firewall, and VPN software appliance.",
    "Url": "https://github.com/vyos/vyos-rolling-nightly/releases/download/1.5-rolling-20260515/vyos-1.5-rolling-20260515-amd64.iso",
    "IsoName": "vyos-1.5-rolling-20260515-amd64.iso",
    "File": "vyos-routing-latest.iso",
    "HashUrlSha256": "https://github.com/vyos/vyos-rolling-nightly/releases/download/1.5-rolling-20260515/vyos-1.5-rolling-20260515-amd64.iso.sha256",
    "DiskSize": "10G",
    "OsFamily": "linux"
  },
  {
    "Id": "openwrt",
    "Name": "OpenWrt x86 Combined Image",
    "Description": "Linux-based operating system designed for network routing, switches, and APs.",
    "Url": "https://downloads.openwrt.org/releases/23.05.3/targets/x86/64/openwrt-23.05.3-x86-64-generic-ext4-combined.img.gz",
    "IsoName": "openwrt-23.05.3-x86-64-generic-ext4-combined.img.gz",
    "File": "openwrt-x86-core.img.gz",
    "HashUrlSha256": "https://downloads.openwrt.org/releases/23.05.3/targets/x86/64/sha256sums",
    "DiskSize": "2G",
    "OsFamily": "linux"
  },
  {
    "Id": "fedora",
    "Name": "Fedora CoreOS (Stable Container Host)",
    "Description": "CoreOS container-focused host with automated provisioning via Ignition.",
    "Url": "https://builds.coreos.fedoraproject.org/prod/streams/stable/builds/40.20240512.3.0/x86_64/fedora-coreos-40.20240512.3.0-live.x86_64.iso",
    "IsoName": "fedora-coreos-40.20240512.3.0-live.x86_64.iso",
    "File": "fedora-coreos-immutable.iso",
    "HashUrlSha256": "https://builds.coreos.fedoraproject.org/prod/streams/stable/builds/40.20240512.3.0/x86_64/fedora-coreos-40.20240512.3.0-live.x86_64.iso.sha256",
    "DiskSize": "30G",
    "OsFamily": "linux"
  },
  {
    "Id": "fedora-server",
    "Name": "Fedora Server Core",
    "Description": "Short-lifecycle server platform showcasing the latest Linux software innovations.",
    "Url": "https://download.fedoraproject.org/pub/fedora/linux/releases/40/Server/x86_64/iso/Fedora-Server-dvd-x86_64-40-1.1.iso",
    "IsoName": "Fedora-Server-dvd-x86_64-40-1.1.iso",
    "File": "fedora-server-latest.iso",
    "HashUrlSha256": "https://getfedora.org/static/checksums/40/Fedora-Server-40-1.1-x86_64-CHECKSUM",
    "DiskSize": "30G",
    "OsFamily": "linux"
  },
  {
    "Id": "kali",
    "Name": "Kali Linux Rolling (Installer)",
    "Description": "Debian-based distribution tailored for security auditing and penetration testing.",
    "Url": "https://cdimage.kali.org/kali-rolling/kali-linux-rolling-installer-amd64.iso",
    "IsoName": "kali-linux-rolling-installer-amd64.iso",
    "File": "kali-linux-latest.iso",
    "HashUrlSha256": "https://cdimage.kali.org/kali-rolling/SHA256SUMS",
    "SigUrl": "https://cdimage.kali.org/kali-rolling/SHA256SUMS.gpg",
    "GpgKey": "44C6513A8E4CC3D30F757453547B1AE444C0CE43",
    "GpgServer": "hkps://keyserver.ubuntu.com",
    "DiskSize": "40G",
    "OsFamily": "linux"
  },
  {
    "Id": "opensuse-leap",
    "Name": "openSUSE Leap (Stable)",
    "Description": "Stable Linux distribution built using enterprise-grade SUSE Linux sources.",
    "Url": "https://download.opensuse.org/distribution/leap/15.6/iso/openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso",
    "IsoName": "openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso",
    "File": "opensuse-leap-latest.iso",
    "HashUrlSha256": "https://download.opensuse.org/distribution/leap/15.6/iso/openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso.sha256",
    "DiskSize": "30G",
    "OsFamily": "linux"
  },
  {
    "Id": "win11-home",
    "Name": "Windows 11 Home",
    "Description": "Local retail copy of Windows 11 Home edition (Unattended ISO).",
    "File": "win11-home.iso",
    "DiskSize": "64G",
    "OsFamily": "windows",
    "Firmware": "uefi"
  },
  {
    "Id": "win11-pro",
    "Name": "Windows 11 Pro",
    "Description": "Local retail copy of Windows 11 Pro edition (Unattended ISO).",
    "File": "win11-pro.iso",
    "DiskSize": "64G",
    "OsFamily": "windows",
    "Firmware": "uefi"
  },
  {
    "Id": "windows11",
    "Name": "Windows 11 Enterprise (Evaluation)",
    "Description": "Microsoft Windows 11 Enterprise (Evaluation), requiring UEFI and virtual TPM.",
    "Url": "https://archive.org/download/windows-11-enterprise-evaluation-iso/22000.194.210913-1444.co_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso",
    "IsoName": "22000.194.210913-1444.co_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso",
    "File": "windows11-enterprise-eval.iso",
    "HashUrlSha1": "https://archive.org/download/windows-11-enterprise-evaluation-iso/windows-11-enterprise-evaluation-iso_files.xml",
    "DiskSize": "64G",
    "OsFamily": "windows",
    "Firmware": "uefi"
  },
  {
    "Id": "windows10",
    "Name": "Windows 10 Enterprise (Evaluation)",
    "Description": "Microsoft Windows 10 Enterprise (Evaluation) for client desktop workloads.",
    "Url": "https://archive.org/download/Win10_Enterprise_Eval_x64/19044.1288.211006-0501.21h2_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso",
    "IsoName": "19044.1288.211006-0501.21h2_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso",
    "File": "windows10-enterprise-eval.iso",
    "HashUrlSha1": "https://archive.org/download/Win10_Enterprise_Eval_x64/Win10_Enterprise_Eval_x64_files.xml",
    "DiskSize": "64G",
    "OsFamily": "windows"
  },
  {
    "Id": "windows8",
    "Name": "Windows 8.1 Pro VL (x64)",
    "Description": "Legacy Microsoft Windows 8.1 Professional VL (x64) installation.",
    "Url": "https://archive.org/download/win-8.1-pro-vl-x-64/Win8.1_Pro_VL_x64_English.iso",
    "IsoName": "Win8.1_Pro_VL_x64_English.iso",
    "File": "windows8.1-pro-vl-x64.iso",
    "HashUrlSha1": "https://archive.org/download/win-8.1-pro-vl-x-64/win-8.1-pro-vl-x-64_files.xml",
    "DiskSize": "40G",
    "OsFamily": "windows"
  },
  {
    "Id": "windows7",
    "Name": "Windows 7 SP1 Ultimate",
    "Description": "Classic Microsoft Windows 7 SP1 Ultimate (x64) installation.",
    "Url": "https://archive.org/download/windows-7-ultimate-x-64-sp-1-fully-updated/Windows%207%20Ultimate%20x64%20-%20SP1%20%28Fully%20Updated%29.iso",
    "IsoName": "Windows 7 Ultimate x64 - SP1 (Fully Updated).iso",
    "File": "windows7-ultimate-sp1.iso",
    "HashUrlSha1": "https://dn760108.eu.archive.org/0/items/windows-7-ultimate-x-64-sp-1-fully-updated/windows-7-ultimate-x-64-sp-1-fully-updated_files.xml",
    "DiskSize": "40G",
    "OsFamily": "windows"
  },
  {
    "Id": "windowsxp",
    "Name": "Windows XP Professional SP3 (x86)",
    "Description": "Legacy Microsoft Windows XP Professional SP3 (x86) installation.",
    "Url": "https://archive.org/download/WinXPProSP3x86/en_windows_xp_professional_with_service_pack_3_x86_cd_vl_x14-73974.iso",
    "IsoName": "en_windows_xp_professional_with_service_pack_3_x86_cd_vl_x14-73974.iso",
    "File": "windowsxp-pro-sp3-x86.iso",
    "HashUrlMd5": "https://archive.org/download/WinXPProSP3x86/WinXPProSP3x86_files.xml",
    "DiskSize": "20G",
    "OsFamily": "windows"
  },
  {
    "Id": "windowsserver",
    "Name": "Windows Server 2022 (Evaluation)",
    "Description": "Microsoft Windows Server 2022 Evaluation edition.",
    "Url": "https://archive.org/download/windows-server-2022_build-20348.169/20348.169.210806-2348.fe_release_svc_refresh_SERVER_EVAL_x64FRE_en-us.iso",
    "IsoName": "20348.169.210806-2348.fe_release_svc_refresh_SERVER_EVAL_x64FRE_en-us.iso",
    "File": "windows-server-2022-eval.iso",
    "HashUrlSha1": "https://archive.org/download/windows-server-2022_build-20348.169/windows-server-2022_build-20348.169_files.xml",
    "DiskSize": "64G",
    "OsFamily": "windows"
  },
  {
    "Id": "android",
    "Name": "Android-x86 (9.0-r2 Pie)",
    "Description": "An open-source project to port Android to the x86 platform, running smoothly under QEMU.",
    "Url": "https://downloads.sourceforge.net/project/android-x86/Release%209.0/android-x86_64-9.0-r2.iso",
    "IsoName": "android-x86_64-9.0-r2.iso",
    "File": "android-x86_64-9.0-r2.iso",
    "HashUrlSha1": "https://raw.githubusercontent.com/android-x86/android-x86-com.github.io/master/releases/releasenote-9-0-r2.html",
    "DiskSize": "16G",
    "OsFamily": "android"
  },
  {
    "Id": "minui",
    "Name": "MinUI (Handheld Launcher)",
    "Description": "Minimal custom launcher for retro handhelds (Anbernic, Miyoo, Trimui). Download-only: copy to SD card.",
    "ResolverType": "GitHubAsset",
    "ResolverRepo": "shauninman/MinUI",
    "ResolverAssetRegex": "^MinUI-.*-base\\.zip$",
    "FileTemplate": "minui-$v.zip",
    "DownloadOnly": true,
    "OsFamily": "linux"
  },
  {
    "Id": "knulli",
    "Name": "Knulli CFW (Anbernic RG34XX)",
    "Description": "Batocera-based custom firmware for retro handhelds. RG34XX SD image. Download-only: flash to SD card.",
    "ResolverType": "GitHubAsset",
    "ResolverRepo": "knulli-cfw/distribution",
    "ResolverAssetRegex": "^knulli-h700-rg34xx-(?!sp-).*\\.img\\.gz$",
    "FileTemplate": "knulli-rg34xx-$v.img.gz",
    "DownloadOnly": true,
    "OsFamily": "linux"
  },
  {
    "Id": "mac",
    "Name": "macOS Bare-Metal Bootstrap (OpenCore Shim)",
    "Description": "macOS bootstrap loader using KVM-Opencore virtual machine shim.",
    "Url": "https://github.com/thenickdude/KVM-Opencore/releases/download/v22/OpenCore-v22.iso",
    "File": "macos-kvm-opencore-shim.iso",
    "HashAlgorithm": "SHA256",
    "ExpectedHash": "7BC510928C3D2918804E11F72234098A843E8203D4FE9401C0FEE6541D09A8E2",
    "DiskSize": "64G",
    "OsFamily": "macos"
  }
]'

# Number of registered targets.
cw_target_count() {
  printf '%s' "$CW_ISO_MATRIX_JSON" | jq -r 'length'
}

# Print the target at 0-based index as compact JSON.
cw_target_json() {
  local idx="$1"
  printf '%s' "$CW_ISO_MATRIX_JSON" | jq -c ".[$idx]"
}

# Find targets: exact Id match wins (PS -eq is case-insensitive); otherwise
# substring match on Id or Name, case-insensitive (PS -like semantics).
cw_find_targets() {
  local query="$1"
  local clean
  clean="$(printf '%s' "$query" | tr '[:upper:]' '[:lower:]' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  local exact
  exact="$(printf '%s' "$CW_ISO_MATRIX_JSON" | jq -c --arg q "$clean" \
    '.[] | select((.Id | ascii_downcase) == $q)')"
  if [ -n "$exact" ]; then
    printf '%s\n' "$exact"
    return 0
  fi
  printf '%s' "$CW_ISO_MATRIX_JSON" | jq -c --arg q "$clean" \
    '.[] | select((.Id | ascii_downcase | contains($q)) or (.Name | ascii_downcase | contains($q)))'
}
