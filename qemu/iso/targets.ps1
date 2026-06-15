# ==============================================================================
# Manifest
# ==============================================================================

$IsoMatrix = @(
    @{
        Id            = "cachyos"
        Name          = "CachyOS Linux"
        Url           = "https://iso.cachyos.org/desktop/260426/cachyos-desktop-linux-260426.iso"
        IsoName       = "cachyos-desktop-linux-260426.iso"
        File          = "cachyos-desktop-latest.iso"
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
        File          = "endeavouros-latest.iso"
        HashUrlSha512 = "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso.sha512"
        SigUrl        = "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso.sig"
        GpgKey        = "CDF595A1"
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "debian"
        Name          = "Debian NetInst"
        Url           = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.5.0-amd64-netinst.iso"
        IsoName       = "debian-13.5.0-amd64-netinst.iso"
        File          = "debian-amd64-netinst.iso"
        # HashUrlSha256 = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS"
        HashUrlSha512 = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS"
        SigUrl        = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS.sign"
        GpgKey        = "DF9B9C49EAA9298432589D76DA87E80D6294BE9B"
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id            = "ubuntu"
        Name          = "Ubuntu Desktop LTS"
        Url           = "https://releases.ubuntu.com/24.04/ubuntu-24.04.4-desktop-amd64.iso"
        IsoName       = "ubuntu-24.04.4-desktop-amd64.iso"
        File          = "ubuntu-desktop-latest.iso"
        HashUrlSha256 = "https://releases.ubuntu.com/24.04/SHA256SUMS"
        SigUrl        = "https://releases.ubuntu.com/24.04/SHA256SUMS.gpg"
        GpgKey        = "843938DF228D22F7B3742BC0D94AA3F0EFE21092" # Ubuntu CD Image Automatic Signing Key
        GpgServer     = "hkps://keyserver.ubuntu.com"
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
    }
    @{
        Id            = "mac"
        Name          = "macOS Bare-Metal Bootstrap (OpenCore Shim)"
        Url           = "https://github.com/thenickdude/KVM-Opencore/releases/download/v22/OpenCore-v22.iso"
        File          = "macos-kvm-opencore-shim.iso"
        HashAlgorithm = "SHA256"
        ExpectedHash  = "7BC510928C3D2918804E11F72234098A843E8203D4FE9401C0FEE6541D09A8E2"
    }
)