# ==============================================================================
# Castle VM — ISO Target Manifest & QEMU Boot Profiles
# ==============================================================================
# Each entry defines:
#   Download   → Url, File, IsoName
#   Verify     → HashUrl*, ExpectedHash, SigUrl, GpgKey, GpgServer
#   Boot       → DiskSize, OsFamily, Firmware
# ==============================================================================

$IsoMatrix = @(

    # ══════════════════════════════════════════════════════════════════════════
    # ARCH-BASED LINUX
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "cachyos"
        Name          = "CachyOS Linux"
        Description   = "Performance-tuned Arch-based distribution with custom kernels and desktop options."
        # Dynamic Resolver Configuration
        ResolverType  = "HtmlDirectory"
        ResolverUrl   = "https://mirror.cachyos.org/ISO/desktop/"
        ResolverRegex = 'href="([^"]+)/"'
        ResolverFilter= '^\d{6}$'
        # Templates
        UrlTemplate   = 'https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso'
        HashUrlSha256Template = 'https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso.sha256'
        SigUrlTemplate= 'https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso.sig'
        IsoNameTemplate = 'cachyos-desktop-linux-$v.iso'
        FileTemplate  = 'cachyos-$v.iso'
        
        GpgKey        = "F3B607488DB35A47"
        GpgServer     = "hkps://keys.openpgp.org"
        # QEMU Profile
        DiskSize      = "40G"
        OsFamily      = "linux"
    },
    @{
        Id            = "endeavouros"
        Name          = "EndeavourOS Linux"
        Description   = "Friendly, terminal-centric Arch-based distribution with a GUI installer."
        # Dynamic Resolver Configuration
        ResolverType  = "GitHub"
        ResolverRepo  = "endeavouros-team/ISO"
        # Templates
        UrlTemplate   = 'https://github.com/endeavouros-team/ISO/releases/download/$v/EndeavourOS_$v.iso'
        HashUrlSha512Template = 'https://github.com/endeavouros-team/ISO/releases/download/$v/EndeavourOS_$v.iso.sha512sum'
        IsoNameTemplate = 'EndeavourOS_$v.iso'
        FileTemplate  = 'endeavouros-$v.iso'
        # Note: GPG sig resolving on GitHub releases varies by version tag formatting, keeping static for now or ignoring sig since hash works
        # QEMU Profile
        DiskSize      = "40G"
        OsFamily      = "linux"
    },
    @{
        Id            = "arch"
        Name          = "Arch Linux Base"
        Description   = "Rolling-release base Linux system emphasizing simplicity, minimalism, and control."
        # Arch uses 'latest' constantly, but has versions in subdirectories. Keeping static 'latest' for now since rolling.
        Url           = "https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso"
        IsoName       = "archlinux-x86_64.iso"
        File          = "arch.iso"
        HashUrlSha256 = "https://geo.mirror.pkgbuild.com/iso/latest/sha256sums.txt"
        SigUrl        = "https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso.sig"
        GpgKey        = "9D26B1A84E65183D"
        GpgServer     = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize      = "30G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # MINIMAL / IMMUTABLE LINUX
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "alpine"
        Name          = "Alpine Linux Extended"
        Description   = "Ultra-lightweight, security-oriented Linux based on musl libc and BusyBox."
        # Using a static version for Alpine until full HTML parser handles Alpine's complex directory structure
        Url           = "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-extended-3.20.0-x86_64.iso"
        IsoName       = "alpine-extended-3.20.0-x86_64.iso"
        File          = "alpine.iso"
        HashUrlSha512 = "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-extended-3.20.0-x86_64.iso.sha512"
        # QEMU Profile
        DiskSize      = "10G"
        OsFamily      = "linux"
    },
    @{
        Id            = "talos"
        Name          = "Talos Linux (Secure Immutable K8s)"
        Description   = "Secure, immutable, and minimal Linux OS built specifically for running Kubernetes."
        # Dynamic Resolver Configuration
        ResolverType  = "GitHub"
        ResolverRepo  = "siderolabs/talos"
        # Templates
        UrlTemplate   = 'https://github.com/siderolabs/talos/releases/download/$v/metal-amd64.iso'
        HashUrlSha256Template = 'https://github.com/siderolabs/talos/releases/download/$v/sha256sum.txt'
        HashUrlSha512Template = 'https://github.com/siderolabs/talos/releases/download/$v/sha512sum.txt'
        IsoNameTemplate = 'metal-amd64.iso'
        FileTemplate  = 'talos-$v.iso'
        # QEMU Profile
        DiskSize      = "20G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # ENTERPRISE LINUX
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "rocky"
        Name          = "Rocky Linux Minimal (RHEL Core)"
        Description   = "Enterprise Linux distro offering 100% bug-for-bug compatibility with RHEL."
        Url           = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.2-x86_64-minimal.iso"
        IsoName       = "Rocky-10.2-x86_64-minimal.iso"
        File          = "rockylinux.iso"
        HashUrlSha256 = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/CHECKSUM"
        SigUrl        = "https://download.rockylinux.org/pub/rocky/9/isos/x86_64/CHECKSUM.SIG"
        GpgKey        = "21040B48A14A2ED7"
        GpgServer     = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize      = "30G"
        OsFamily      = "linux"
    },
    @{
        Id            = "debian"
        Name          = "Debian NetInst"
        Description   = "Highly stable and community-driven Linux distribution, known as the Universal OS."
        Url           = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.5.0-amd64-netinst.iso"
        IsoName       = "debian-13.5.0-amd64-netinst.iso"
        File          = "debian.iso"
        HashUrlSha256 = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS"
        HashUrlSha512 = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS"
        # Debian signs the checksum file, not the ISO directly — GPG disabled until
        # we add checksum-file-level signature verification support
        # SigUrl      = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS.sign"
        # GpgKey      = "DF9B9C49EAA9298432589D76DA87E80D6294BE9B"
        # GpgServer   = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize      = "30G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # UBUNTU
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "ubuntu"
        Name          = "Ubuntu Desktop LTS"
        Description   = "Popular, user-friendly desktop operating system based on Debian."
        Url           = "https://releases.ubuntu.com/24.04/ubuntu-24.04.4-desktop-amd64.iso"
        IsoName       = "ubuntu-24.04.4-desktop-amd64.iso"
        File          = "ubuntu.iso"
        HashUrlSha256 = "https://releases.ubuntu.com/24.04/SHA256SUMS"
        # Ubuntu signs the checksum file, not the ISO directly
        # SigUrl      = "https://releases.ubuntu.com/24.04/SHA256SUMS.gpg"
        # GpgKey      = "843938DF228D22F7B3742BC0D94AA3F0EFE21092"
        # GpgServer   = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize      = "40G"
        OsFamily      = "linux"
    },
    @{
        Id            = "ubuntu-server"
        Name          = "Ubuntu Server LTS (AI Hardware Optimized)"
        Description   = "LTS server operating system optimized for container, cloud, and AI hardware workloads."
        Url           = "https://releases.ubuntu.com/24.04/ubuntu-24.04-live-server-amd64.iso"
        IsoName       = "ubuntu-24.04-live-server-amd64.iso"
        File          = "ubuntu-server-latest.iso"
        HashUrlSha256 = "https://releases.ubuntu.com/24.04/SHA256SUMS"
        SigUrl        = "https://releases.ubuntu.com/24.04/SHA256SUMS.gpg"
        GpgKey        = "843938DF228D22F7B3742BC0D94AA3F0EFE21092"
        GpgServer     = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize      = "40G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # NETWORK / ROUTER APPLIANCES
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "vyos"
        Name          = "VyOS Network Router (LTS)"
        Description   = "Debian-based network routing, firewall, and VPN software appliance."
        Url           = "https://github.com/vyos/vyos-rolling-nightly/releases/download/1.5-rolling-20260515/vyos-1.5-rolling-20260515-amd64.iso"
        IsoName       = "vyos-1.5-rolling-20260515-amd64.iso"
        File          = "vyos-routing-latest.iso"
        HashUrlSha256 = "https://github.com/vyos/vyos-rolling-nightly/releases/download/1.5-rolling-20260515/vyos-1.5-rolling-20260515-amd64.iso.sha256"
        # QEMU Profile
        DiskSize      = "10G"
        OsFamily      = "linux"
    },
    @{
        Id            = "openwrt"
        Name          = "OpenWrt x86 Combined Image"
        Description   = "Linux-based operating system designed for network routing, switches, and APs."
        Url           = "https://downloads.openwrt.org/releases/23.05.3/targets/x86/64/openwrt-23.05.3-x86-64-generic-ext4-combined.img.gz"
        IsoName       = "openwrt-23.05.3-x86-64-generic-ext4-combined.img.gz"
        File          = "openwrt-x86-core.img.gz"
        HashUrlSha256 = "https://downloads.openwrt.org/releases/23.05.3/targets/x86/64/sha256sums"
        # QEMU Profile
        DiskSize      = "2G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # FEDORA
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "fedora"
        Name          = "Fedora CoreOS (Stable Container Host)"
        Description   = "CoreOS container-focused host with automated provisioning via Ignition."
        Url           = "https://builds.coreos.fedoraproject.org/prod/streams/stable/builds/40.20240512.3.0/x86_64/fedora-coreos-40.20240512.3.0-live.x86_64.iso"
        IsoName       = "fedora-coreos-40.20240512.3.0-live.x86_64.iso"
        File          = "fedora-coreos-immutable.iso"
        HashUrlSha256 = "https://builds.coreos.fedoraproject.org/prod/streams/stable/builds/40.20240512.3.0/x86_64/fedora-coreos-40.20240512.3.0-live.x86_64.iso.sha256"
        # QEMU Profile
        DiskSize      = "30G"
        OsFamily      = "linux"
    },
    @{
        Id            = "fedora-server"
        Name          = "Fedora Server Core"
        Description   = "Short-lifecycle server platform showcasing the latest Linux software innovations."
        Url           = "https://download.fedoraproject.org/pub/fedora/linux/releases/40/Server/x86_64/iso/Fedora-Server-dvd-x86_64-40-1.1.iso"
        IsoName       = "Fedora-Server-dvd-x86_64-40-1.1.iso"
        File          = "fedora-server-latest.iso"
        HashUrlSha256 = "https://getfedora.org/static/checksums/40/Fedora-Server-40-1.1-x86_64-CHECKSUM"
        # QEMU Profile
        DiskSize      = "30G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # SECURITY / PENTESTING
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "kali"
        Name          = "Kali Linux Rolling (Installer)"
        Description   = "Debian-based distribution tailored for security auditing and penetration testing."
        Url           = "https://cdimage.kali.org/kali-rolling/kali-linux-rolling-installer-amd64.iso"
        IsoName       = "kali-linux-rolling-installer-amd64.iso"
        File          = "kali-linux-latest.iso"
        HashUrlSha256 = "https://cdimage.kali.org/kali-rolling/SHA256SUMS"
        SigUrl        = "https://cdimage.kali.org/kali-rolling/SHA256SUMS.gpg"
        GpgKey        = "44C6513A8E4CC3D30F757453547B1AE444C0CE43"
        GpgServer     = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize      = "40G"
        OsFamily      = "linux"
    },
    @{
        Id            = "opensuse-leap"
        Name          = "openSUSE Leap (Stable)"
        Description   = "Stable Linux distribution built using enterprise-grade SUSE Linux sources."
        Url           = "https://download.opensuse.org/distribution/leap/15.6/iso/openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso"
        IsoName       = "openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso"
        File          = "opensuse-leap-latest.iso"
        HashUrlSha256 = "https://download.opensuse.org/distribution/leap/15.6/iso/openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso.sha256"
        # QEMU Profile
        DiskSize      = "30G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # WINDOWS
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "windows11"
        Name          = "Windows 11 Enterprise (Evaluation)"
        Description   = "Microsoft Windows 11 Enterprise (Evaluation), requiring UEFI and virtual TPM."
        Url           = "https://software-static.download.prss.microsoft.com/kv/win/ch9/enterprise/26100.1742.240906-0331.ge_release_svc_refresh_CLIENTENTERPRISEEVAL_x64FRE_en-us.iso"
        File          = "windows11-enterprise-eval.iso"
        HashAlgorithm = "SHA256"
        ExpectedHash  = "4A87FA29B12D0BE3A48C079140D83A6B4E54CE373A12E8DA4999FA44BB968132"
        # QEMU Profile
        DiskSize      = "64G"
        OsFamily      = "windows"
        Firmware      = "uefi"
    },
    @{
        Id            = "windows10"
        Name          = "Windows 10 Enterprise (Evaluation)"
        Description   = "Microsoft Windows 10 Enterprise (Evaluation) for client desktop workloads."
        Url           = "https://software-static.download.prss.microsoft.com/sg/download/evalcenter/Win10_Enterprise_Evaluation_x64_en-us.iso"
        File          = "windows10-enterprise-eval.iso"
        HashAlgorithm = "SHA256"
        ExpectedHash  = "E10A7C1844A68C6F19A96E7E0BFE23EB114BA717CD8F77B13214E8746AA6E3B9"
        # QEMU Profile
        DiskSize      = "64G"
        OsFamily      = "windows"
    },
    @{
        Id            = "windows8"
        Name          = "Windows 8.1 Pro VL (x64)"
        Description   = "Legacy Microsoft Windows 8.1 Professional VL (x64) installation."
        Url           = "https://archive.org/download/win-8.1-pro-vl-x-64/Win8.1_Pro_VL_x64_English.iso"
        IsoName       = "Win8.1_Pro_VL_x64_English.iso"
        File          = "windows8.1-pro-vl-x64.iso"
        # Archive.org _files.xml contains per-file SHA1 hashes in XML format
        HashUrlSha1   = "https://archive.org/download/win-8.1-pro-vl-x-64/win-8.1-pro-vl-x-64_files.xml"
        # QEMU Profile
        DiskSize      = "40G"
        OsFamily      = "windows"
    },
    @{
        Id            = "windows7"
        Name          = "Windows 7 SP1 Ultimate"
        Description   = "Classic Microsoft Windows 7 SP1 Ultimate (x64) installation."
        Url           = "https://archive.org/download/win7-ultimate-sp1-x64/Win7_Ult_SP1_English_COEM_x64.iso"
        IsoName       = "Win7_Ult_SP1_English_COEM_x64.iso"
        File          = "windows7-ultimate-sp1.iso"
        # Archive.org _files.xml contains per-file SHA1 hashes in XML format
        HashUrlSha1   = "https://archive.org/download/win7-ultimate-sp1-x64/win7-ultimate-sp1-x64_files.xml"
        # QEMU Profile
        DiskSize      = "40G"
        OsFamily      = "windows"
    },
    @{
        Id            = "windowsxp"
        Name          = "Windows XP Professional SP3 (x86)"
        Description   = "Legacy Microsoft Windows XP Professional SP3 (x86) installation."
        Url           = "https://archive.org/download/WinXPProSP3x86/en_windows_xp_professional_with_service_pack_3_x86_cd_vl_x14-73974.iso"
        IsoName       = "en_windows_xp_professional_with_service_pack_3_x86_cd_vl_x14-73974.iso"
        File          = "windowsxp-pro-sp3-x86.iso"
        # Archive.org _files.xml contains per-file MD5 hashes in XML format
        HashUrlMd5    = "https://archive.org/download/WinXPProSP3x86/WinXPProSP3x86_files.xml"
        # QEMU Profile
        DiskSize      = "20G"
        OsFamily      = "windows"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # macOS (Experimental — Requires OpenCore Shim)
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "mac"
        Name          = "macOS Bare-Metal Bootstrap (OpenCore Shim)"
        Description   = "macOS bootstrap loader using KVM-Opencore virtual machine shim."
        Url           = "https://github.com/thenickdude/KVM-Opencore/releases/download/v22/OpenCore-v22.iso"
        File          = "macos-kvm-opencore-shim.iso"
        HashAlgorithm = "SHA256"
        ExpectedHash  = "7BC510928C3D2918804E11F72234098A843E8203D4FE9401C0FEE6541D09A8E2"
        # QEMU Profile
        DiskSize      = "64G"
        OsFamily      = "macos"
    }
)
