# ==============================================================================
# Manifest
# ==============================================================================

$IsoMatrix = @(
    @{
        Id            = "cachyos"
        Name          = "CachyOS Linux"
        Url           = "https://iso.cachyos.org/desktop/260426/cachyos-desktop-linux-260426.iso"
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
        File          = "endeavouros-latest.iso"
        HashUrlSha512 = "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso.sha512"
        SigUrl        = "https://mirrors.gigenet.com/endeavouros/iso/EndeavourOS_Titan-Neo-2026.04.27.iso.sig"
        GpgKey        = "CDF595A1"
        GpgServer     = "hkps://keyserver.ubuntu.com"
    },
    @{
        Id        = "debian"
        Name      = "Debian NetInst"
        Url       = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.5.0-amd64-netinst.iso"
        File      = "debian-amd64-netinst.iso"
        HashUrl   = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS"
        SigUrl    = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS.sign"
        GpgKey    = "DA87E80D6294BE73"
        GpgServer = "hkps://keyserver.ubuntu.com"
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
        Id            = "windows7"
        Name          = "Windows 7 SP1 Ultimate"
        Url           = "https://archive.org/download/win7-ultimate-sp1-x64/Win7_Ult_SP1_English_COEM_x64.iso"
        File          = "windows7-ultimate-sp1.iso"
        HashAlgorithm = "SHA1"
        ExpectedHash  = "G2312AD3A2B23F4426FA5B4A31C28AF314A0B70D" # Archive verification profile
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