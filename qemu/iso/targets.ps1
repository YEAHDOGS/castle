# ==============================================================================
# Manifest
# ==============================================================================

$IsoMatrix = @(
    @{
        Id            = "cachyos"
        Name          = "CachyOS Linux"
        Url           = "https://iso.cachyos.org/desktop/260426/cachyos-desktop-linux-260426.iso"
        IsoName       = "cachyos-desktop-linux-260426.iso"
        File          = "cachyos.iso"
        HashUrlSha256 = "https://iso.cachyos.org/desktop/260426/cachyos-desktop-linux-260426.iso.sha256"
        SigUrl        = "https://iso.cachyos.org/desktop/260426/cachyos-desktop-linux-260426.iso.sig"
        GpgKey        = "F3B607488DB35A47"
        GpgServer     = "hkps://keys.openpgp.org"
    },
    @{
        Id            = "endeavouros"
        Name          = "EndeavourOS Linux"
        Url           = "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso"
        IsoName       = "EndeavourOS_Titan-Neo-2026.04.27.iso"
        File          = "endeavouros.iso"
        HashUrlSha512 = "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso.sha512"
        SigUrl        = "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso.sig"
        GpgKey        = "CDF595A1"
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "arch"
        Name          = "Arch Linux Base"
        Url           = "https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso"
        IsoName       = "archlinux-x86_64.iso"
        File          = "arch.iso"
        # Arch publishes a single dynamic text file containing both md5/sha1/sha256 metrics
        HashUrlSha256 = "https://geo.mirror.pkgbuild.com/iso/latest/sha256sums.txt"
        SigUrl        = "https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x3e80ca1a8b89f69cba57d98a76a5ef9054449a5c"
        GpgKey        = "9D26B1A84E65183D" # Arch Linux Master Key (Pierre Schmitz)
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "alpine"
        Name          = "Alpine Linux Extended"
        Url           = "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-extended-3.20.0-x86_64.iso"
        IsoName       = "alpine-extended-3.20.0-x86_64.iso"
        File          = "alpine.iso"
        # Alpine signs the raw sha512 file itself inside the release tree
        HashUrlSha512 = "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-extended-3.20.0-x86_64.iso.sha512"
    },
    @{
        Id            = "talos"
        Name          = "Talos Linux (Secure Immutable K8s)"
        Url           = "https://github.com/siderolabs/talos/releases/download/v1.13.4/metal-amd64.iso"
        IsoName       = "metal-amd64.iso"
        File          = "talos.iso"
        HashUrlSha256 = "https://github.com/siderolabs/talos/releases/download/v1.13.4/sha256sum.txt"
        HashUrlSha512 = "https://github.com/siderolabs/talos/releases/download/v1.13.4/sha512sum.txt"
    },
    @{
        Id            = "rocky"
        Name          = "Rocky Linux Minimal (RHEL Core)"
        Url           = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.2-x86_64-minimal.iso"
        IsoName       = "Rocky-10.2-x86_64-minimal.iso"
        File          = "rockylinux.iso"
        HashUrlSha256 = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/CHECKSUM"
        SigUrl        = "https://download.rockylinux.org/pub/rocky/9/isos/x86_64/CHECKSUM.SIG"
        GpgKey        = "21040B48A14A2ED7" # Rocky Linux 9 Signing Key
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "debian"
        Name          = "Debian NetInst"
        Url           = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.5.0-amd64-netinst.iso"
        IsoName       = "debian-13.5.0-amd64-netinst.iso"
        File          = "debian.iso"
        HashUrlSha256 = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS"
        HashUrlSha512 = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS"
        # Debian gpg .sig file verifies the sha512 key file, not the iso directly...?
        # SigUrl        = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS.sign"
        # GpgKey        = "DF9B9C49EAA9298432589D76DA87E80D6294BE9B"
        # GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "ubuntu"
        Name          = "Ubuntu Desktop LTS"
        Url           = "https://releases.ubuntu.com/24.04/ubuntu-24.04.4-desktop-amd64.iso"
        IsoName       = "ubuntu-24.04.4-desktop-amd64.iso"
        File          = "ubuntu.iso"
        HashUrlSha256 = "https://releases.ubuntu.com/24.04/SHA256SUMS"
        # Ubuntu gpg .sig file verifies the sha256 key file, not the iso directly...?
        # SigUrl        = "https://releases.ubuntu.com/24.04/SHA256SUMS.gpg"
        # GpgKey        = "843938DF228D22F7B3742BC0D94AA3F0EFE21092" # Ubuntu CD Image Automatic Signing Key
        # GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "ubuntu-server"
        Name          = "Ubuntu Server LTS (AI Hardware Optimized)"
        Url           = "https://releases.ubuntu.com/24.04/ubuntu-24.04-live-server-amd64.iso"
        IsoName       = "ubuntu-24.04-live-server-amd64.iso"
        File          = "ubuntu-server-latest.iso"
        HashUrlSha256 = "https://releases.ubuntu.com/24.04/SHA256SUMS"
        SigUrl        = "https://releases.ubuntu.com/24.04/SHA256SUMS.gpg"
        GpgKey        = "843938DF228D22F7B3742BC0D94AA3F0EFE21092"
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "vyos"
        Name          = "VyOS Network Router (LTS)"
        Url           = "https://github.com/vyos/vyos-rolling-nightly/releases/download/1.5-rolling-20260515/vyos-1.5-rolling-20260515-amd64.iso"
        IsoName       = "vyos-1.5-rolling-20260515-amd64.iso"
        File          = "vyos-routing-latest.iso"
        # VyOS generates flat standalone SHA256 checksum tracking files
        HashUrlSha256 = "https://github.com/vyos/vyos-rolling-nightly/releases/download/1.5-rolling-20260515/vyos-1.5-rolling-20260515-amd64.iso.sha256"
    },
    @{
        Id            = "openwrt"
        Name          = "OpenWrt x86 Combined Image"
        Url           = "https://downloads.openwrt.org/releases/23.05.3/targets/x86/64/openwrt-23.05.3-x86-64-generic-ext4-combined.img.gz"
        IsoName       = "openwrt-23.05.3-x86-64-generic-ext4-combined.img.gz"
        File          = "openwrt-x86-core.img.gz"
        # OpenWrt maps directory structures to a central hash manifest table similar to Debian/Arch
        HashUrlSha256 = "https://downloads.openwrt.org/releases/23.05.3/targets/x86/64/sha256sums"
    },
    @{
        Id            = "fedora"
        Name          = "Fedora CoreOS (Stable Container Host)"
        Url           = "https://builds.coreos.fedoraproject.org/prod/streams/stable/builds/40.20240512.3.0/x86_64/fedora-coreos-40.20240512.3.0-live.x86_64.iso"
        IsoName       = "fedora-coreos-40.20240512.3.0-live.x86_64.iso"
        File          = "fedora-coreos-immutable.iso"
        HashUrlSha256 = "https://builds.coreos.fedoraproject.org/prod/streams/stable/builds/40.20240512.3.0/x86_64/fedora-coreos-40.20240512.3.0-live.x86_64.iso.sha256"
    },
    @{
        Id            = "fedora-server"
        Name          = "Fedora Server Core"
        Url           = "https://download.fedoraproject.org/pub/fedora/linux/releases/40/Server/x86_64/iso/Fedora-Server-dvd-x86_64-40-1.1.iso"
        IsoName       = "Fedora-Server-dvd-x86_64-40-1.1.iso"
        File          = "fedora-server-latest.iso"
        # Fedora aggregates validation parameters into a centralized manifest table
        HashUrlSha256 = "https://getfedora.org/static/checksums/40/Fedora-Server-40-1.1-x86_64-CHECKSUM"
    },
    @{
        Id            = "kali"
        Name          = "Kali Linux Rolling (Installer)"
        Url           = "https://cdimage.kali.org/kali-rolling/kali-linux-rolling-installer-amd64.iso"
        IsoName       = "kali-linux-rolling-installer-amd64.iso"
        File          = "kali-linux-latest.iso"
        # Kali hosts raw SHA256SUMS text tables in their rolling target tree
        HashUrlSha256 = "https://cdimage.kali.org/kali-rolling/SHA256SUMS"
        SigUrl        = "https://cdimage.kali.org/kali-rolling/SHA256SUMS.gpg"
        GpgKey        = "44C6513A8E4CC3D30F757453547B1AE444C0CE43" # Kali Linux Official Release Key
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "opensuse-leap"
        Name          = "openSUSE Leap (Stable)"
        Url           = "https://download.opensuse.org/distribution/leap/15.6/iso/openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso"
        IsoName       = "openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso"
        File          = "opensuse-leap-latest.iso"
        HashUrlSha256 = "https://download.opensuse.org/distribution/leap/15.6/iso/openSUSE-Leap-15.6-CR-DVD-x86_64-Build710.1-Media.iso.sha256"
    },
    @{
        Id            = "windows11"
        Name          = "Windows 11 Enterprise (Evaluation)"
        Url           = "https://software-static.download.prss.microsoft.com/kv/win/ch9/enterprise/26100.1742.240906-0331.ge_release_svc_refresh_CLIENTENTERPRISEEVAL_x64FRE_en-us.iso"
        File          = "windows11-enterprise-eval.iso"
        HashAlgorithm = "SHA256" # Windows payloads utilize hardcoded verification profiles
        ExpectedHash  = "4A87FA29B12D0BE3A48C079140D83A6B4E54CE373A12E8DA4999FA44BB968132"
    },
    @{
        Id            = "windows10"
        Name          = "Windows 10 Enterprise (Evaluation)"
        Url           = "https://software-static.download.prss.microsoft.com/sg/download/evalcenter/Win10_Enterprise_Evaluation_x64_en-us.iso"
        File          = "windows10-enterprise-eval.iso"
        # Microsoft evaluation streams do not provide standalone online hash files;
        # Keeping this as your baseline profile control target
        HashAlgorithm = "SHA256"
        ExpectedHash  = "E10A7C1844A68C6F19A96E7E0BFE23EB114BA717CD8F77B13214E8746AA6E3B9"
    },
    @{
        Id          = "windows8"
        Name        = "Windows 8.1 Pro VL (x64)"
        Url         = "https://archive.org/download/win-8.1-pro-vl-x-64/Win8.1_Pro_VL_x64_English.iso"
        File        = "windows8.1-pro-vl-x64.iso"
        # Pulls down the Archive.org central cryptographic XML manifest for the item directory
        HashUrlSha1 = "https://archive.org/download/win-8.1-pro-vl-x-64/win-8.1-pro-vl-x-64_meta.xml"
    },
    @{
        Id            = "windows7"
        Name          = "Windows 7 SP1 Ultimate"
        Url           = "https://archive.org/download/win7-ultimate-sp1-x64/Win7_Ult_SP1_English_COEM_x64.iso"
        File          = "windows7-ultimate-sp1.iso"
        # Archive.org tracks historical SHA1 attributes natively in the XML manifest tree
        HashUrlSha1   = "https://archive.org/download/win7-ultimate-sp1-x64/win7-ultimate-sp1-x64_meta.xml"
        HashAlgorithm = "SHA1"
        ExpectedHash  = "G2312AD3A2B23F4426FA5B4A31C28AF314A0B70D" # Archive verification profile
    },
    @{
        Id         = "windowsxp"
        Name       = "Windows XP Professional SP3 (x86)"
        Url        = "https://archive.org/download/WinXPProSP3x86/en_windows_xp_professional_with_service_pack_3_x86_cd_vl_x14-73974.iso"
        File       = "windowsxp-pro-sp3-x86.iso"
        # Leverages the standard MD5 checksum manifest stream generated for the item workspace
        HashUrlMd5 = "https://archive.org/download/WinXPProSP3x86/WinXPProSP3x86_meta.xml"
    },
    @{
        Id            = "mac"
        Name          = "macOS Bare-Metal Bootstrap (OpenCore Shim)"
        Url           = "https://github.com/thenickdude/KVM-Opencore/releases/download/v22/OpenCore-v22.iso"
        File          = "macos-kvm-opencore-shim.iso"
        HashAlgorithm = "SHA256"
        ExpectedHash  = "7BC510928C3D2918804E11F72234098A843E8203D4FE9401C0FEE6541D09A8E2"
    }
)