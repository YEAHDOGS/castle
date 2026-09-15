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
        Id                    = "cachyos"
        Name                  = "CachyOS Linux"
        Description           = "Performance-tuned Arch-based distribution with custom kernels and desktop options."
        # Dynamic Resolver Configuration
        ResolverType          = "HtmlDirectory"
        ResolverUrl           = "https://mirror.cachyos.org/ISO/desktop/"
        ResolverRegex         = 'href="([^"]+)/"'
        ResolverFilter        = '^\d{6}$'
        # Templates
        UrlTemplate           = 'https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso'
        HashUrlSha256Template = 'https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso.sha256'
        SigUrlTemplate        = 'https://mirror.cachyos.org/ISO/desktop/$v/cachyos-desktop-linux-$v.iso.sig'
        IsoNameTemplate       = 'cachyos-desktop-linux-$v.iso'
        FileTemplate          = 'cachyos-$v.iso'
        
        GpgKey                = "F3B607488DB35A47"
        GpgServer             = "hkps://keys.openpgp.org"
        # QEMU Profile
        DiskSize              = "40G"
        OsFamily              = "linux"
    },
    @{
        Id                    = "endeavouros"
        Name                  = "EndeavourOS Linux"
        Description           = "Friendly, terminal-centric Arch-based distribution with a GUI installer."
        # -- Mirror selection --
        # Every URL below carries $m, the base directory of the chosen mirror
        # (no trailing slash). The mirror list is scraped live from the official
        # download page: each row there links the ISO, its .sha512sum and its
        # .sig on the same mirror, so one base serves all three. If the page is
        # unreachable or its markup changes, the snapshot in `Mirrors` is used
        # (scraped from https://endeavouros.com/download/ on 2026-09-14).
        MirrorPageUrl         = "https://endeavouros.com/download/"
        # Named groups the generic scraper needs: region (section), then per
        # row country, name and url (a link to the ISO; base = url minus file).
        MirrorSectionRegex    = '(?s)<section class="mirror-group"[^>]*>\s*<h3[^>]*id="mirrors-[a-z-]+">(?<region>[^<]+)<.*?</section>'
        MirrorRowRegex        = '(?s)<tr>\s*<th[^>]*class="c-loc"[^>]*>.*?<span class="country">(?<country>[^<]+)</span>.*?<td class="c-name"><a href="(?<url>[^"]+\.iso)"[^>]*>(?<name>[^<]+)<'
        MirrorDefault         = "mirrors.gigenet.com"
        Mirrors               = @(
            @{ Region = "Africa"; Country = "South Africa"; Name = "Urban Wave"; Base = "https://mirrors.urbanwave.co.za/endeavouros/iso" },
            @{ Region = "Asia"; Country = "China"; Name = "Jilin University"; Base = "https://mirrors.jlu.edu.cn/endeavouros/iso" },
            @{ Region = "Asia"; Country = "China"; Name = "SJTU"; Base = "https://mirror.sjtu.edu.cn/endeavouros/iso" },
            @{ Region = "Asia"; Country = "China"; Name = "Tuna"; Base = "https://mirrors.tuna.tsinghua.edu.cn/endeavouros/iso" },
            @{ Region = "Asia"; Country = "India"; Name = "Albony"; Base = "https://mirror.albony.in/endeavouros/iso" },
            @{ Region = "Asia"; Country = "India"; Name = "Nxtgen"; Base = "https://mirrors.nxtgen.com/endeavouros-mirror/iso" },
            @{ Region = "Asia"; Country = "Japan"; Name = "Miraa"; Base = "https://www.miraa.jp/endeavouros/iso" },
            @{ Region = "Asia"; Country = "Singapore"; Name = "Freedif"; Base = "https://mirror.freedif.org/EndeavourOS/iso" },
            @{ Region = "Asia"; Country = "Singapore"; Name = "Jingk"; Base = "https://mirror.jingk.ai/endeavouros/iso" },
            @{ Region = "Asia"; Country = "South Korea"; Name = "YuruMirror"; Base = "https://mirror.funami.tech/endeavouros/iso" },
            @{ Region = "Asia"; Country = "Taiwan"; Name = "Archlinux Taiwan"; Base = "https://mirror.archlinux.tw/EndeavourOS/iso" },
            @{ Region = "Europe"; Country = "Belgium"; Name = "Belnet"; Base = "https://ftp.belnet.be/mirror/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Denmark"; Name = "C0urier"; Base = "https://mirrors.c0urier.net/linux/endeavouros/iso" },
            @{ Region = "Europe"; Country = "France"; Name = "Rznet"; Base = "https://mirror.rznet.fr/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Germany"; Name = "Alpix"; Base = "https://mirror.alpix.eu/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Germany"; Name = "Diyarciftci"; Base = "https://mirror.diyarciftci.xyz/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Germany"; Name = "Moson"; Base = "https://mirror.moson.org/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Germany"; Name = "RZ TU-BS"; Base = "https://ftp.rz.tu-bs.de/pub/mirror/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Greece"; Name = "Fosszone"; Base = "https://fosszone.csd.auth.gr/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Sweden"; Name = "Retropc"; Base = "https://mirror.retropc.se/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Sweden"; Name = "Umea University"; Base = "https://mirror.accum.se/mirror/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Switzerland"; Name = "Adfinis"; Base = "https://pkg.adfinis-on-exoscale.ch/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Switzerland"; Name = "Go Foss"; Base = "https://mirror.gofoss.xyz/endeavouros/iso" },
            @{ Region = "Europe"; Country = "Ukraine"; Name = "Distrohub"; Base = "https://distrohub.kyiv.ua/endeavouros/iso" },
            @{ Region = "Europe"; Country = "United Kingdom"; Name = "C48"; Base = "https://repo.c48.uk/endeavouros/iso" },
            @{ Region = "North America"; Country = "United States"; Name = "Gigenet"; Base = "https://mirrors.gigenet.com/endeavouros/iso" }
        )
        # Dynamic Resolver Configuration (runs against the chosen mirror)
        ResolverType          = "HtmlDirectory"
        ResolverUrl           = '$m/'
        ResolverRegex         = 'href="EndeavourOS_([^"]+)\.iso"'
        # Templates
        UrlTemplate           = '$m/EndeavourOS_$v.iso'
        # The mirrors publish the checksum as <iso>.sha512sum (not .sha512).
        HashUrlSha512Template = '$m/EndeavourOS_$v.iso.sha512sum'
        SigUrlTemplate        = '$m/EndeavourOS_$v.iso.sig'
        IsoNameTemplate       = 'EndeavourOS_$v.iso'
        FileTemplate          = 'endeavouros-$v.iso'
        # QEMU Profile
        DiskSize              = "40G"
        OsFamily              = "linux"
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
        Id                    = "talos"
        Name                  = "Talos Linux (Secure Immutable K8s)"
        Description           = "Secure, immutable, and minimal Linux OS built specifically for running Kubernetes."
        # Dynamic Resolver Configuration
        ResolverType          = "GitHub"
        ResolverRepo          = "siderolabs/talos"
        # Templates
        UrlTemplate           = 'https://github.com/siderolabs/talos/releases/download/$v/metal-amd64.iso'
        HashUrlSha256Template = 'https://github.com/siderolabs/talos/releases/download/$v/sha256sum.txt'
        HashUrlSha512Template = 'https://github.com/siderolabs/talos/releases/download/$v/sha512sum.txt'
        IsoNameTemplate       = 'metal-amd64.iso'
        FileTemplate          = 'talos-$v.iso'
        # QEMU Profile
        DiskSize              = "20G"
        OsFamily              = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # ENTERPRISE LINUX
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id                    = "rocky"
        Name                  = "Rocky Linux Minimal (RHEL Core)"
        Description           = "Enterprise Linux distro offering 100% bug-for-bug compatibility with RHEL."
        # Dynamic Resolver Configuration
        ResolverType          = "HtmlDirectory"
        ResolverUrl           = "https://download.rockylinux.org/pub/rocky/"
        ResolverRegex         = 'href="([89]|10)/"'
        # Templates
        UrlTemplate           = 'https://download.rockylinux.org/pub/rocky/$v/isos/x86_64/Rocky-$v-latest-x86_64-minimal.iso'
        IsoNameTemplate       = 'Rocky-$v-latest-x86_64-minimal.iso'
        FileTemplate          = 'rockylinux-$v.iso'
        HashUrlSha256Template = 'https://download.rockylinux.org/pub/rocky/$v/isos/x86_64/CHECKSUM'
        # Rocky signs the checksum file, not the ISO directly — GPG disabled until
        # we add checksum-file-level signature verification support.
        # QEMU Profile
        DiskSize              = "30G"
        OsFamily              = "linux"
    },
    @{
        Id                    = "debian"
        Name                  = "Debian NetInst"
        Description           = "Highly stable and community-driven Linux distribution, known as the Universal OS."
        # Dynamic Resolver Configuration
        ResolverType          = "HtmlDirectory"
        ResolverUrl           = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/"
        ResolverRegex         = 'href="debian-([\d\.]+)-amd64-netinst\.iso"'
        # Templates
        UrlTemplate           = 'https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-$v-amd64-netinst.iso'
        IsoNameTemplate       = 'debian-$v-amd64-netinst.iso'
        FileTemplate          = 'debian-$v.iso'
        HashUrlSha256Template = 'https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS'
        HashUrlSha512Template = 'https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA512SUMS'
        # Debian signs the checksum file, not the ISO directly — GPG disabled until
        # we add checksum-file-level signature verification support
        # QEMU Profile
        DiskSize              = "30G"
        OsFamily              = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # UBUNTU
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id                    = "ubuntu"
        Name                  = "Ubuntu Desktop LTS"
        Description           = "The most widely used Linux desktop OS, backed by Canonical."
        # Dynamic Resolver Configuration
        ResolverType          = "HtmlDirectory"
        ResolverUrl           = "https://releases.ubuntu.com/24.04/"
        ResolverRegex         = 'href="ubuntu-(\d+\.\d+(?:\.\d+)?)-desktop-amd64\.iso"'
        # Templates
        UrlTemplate           = 'https://releases.ubuntu.com/24.04/ubuntu-$v-desktop-amd64.iso'
        IsoNameTemplate       = 'ubuntu-$v-desktop-amd64.iso'
        FileTemplate          = 'ubuntu-$v.iso'
        HashUrlSha256Template = 'https://releases.ubuntu.com/24.04/SHA256SUMS'
        # Ubuntu signs the checksum file, not the ISO directly
        # SigUrl      = "https://releases.ubuntu.com/24.04/SHA256SUMS.gpg"
        # GpgKey      = "843938DF228D22F7B3742BC0D94AA3F0EFE21092"
        # GpgServer   = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize              = "40G"
        OsFamily              = "linux"
    },
    @{
        Id                    = "ubuntu-server"
        Name                  = "Ubuntu Server LTS (AI Hardware Optimized)"
        Description           = "LTS server operating system optimized for container, cloud, and AI hardware workloads."
        # Dynamic Resolver Configuration
        ResolverType          = "HtmlDirectory"
        ResolverUrl           = "https://releases.ubuntu.com/24.04/"
        ResolverRegex         = 'href="ubuntu-(\d+\.\d+(?:\.\d+)?)-live-server-amd64\.iso"'
        # Templates
        UrlTemplate           = 'https://releases.ubuntu.com/24.04/ubuntu-$v-live-server-amd64.iso'
        IsoNameTemplate       = 'ubuntu-$v-live-server-amd64.iso'
        FileTemplate          = 'ubuntu-server-$v.iso'
        HashUrlSha256Template = 'https://releases.ubuntu.com/24.04/SHA256SUMS'
        # Ubuntu signs the checksum file, not the ISO directly
        # SigUrl        = "https://releases.ubuntu.com/24.04/SHA256SUMS.gpg"
        # GpgKey        = "843938DF228D22F7B3742BC0D94AA3F0EFE21092"
        # GpgServer     = "hkps://keyserver.ubuntu.com"
        # QEMU Profile
        DiskSize              = "40G"
        OsFamily              = "linux"
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
    # DOCKER DESKTOPS (containers, not QEMU -- delegated to docker.ps1)
    # ══════════════════════════════════════════════════════════════════════════
    # Runtime = "docker" entries skip the ISO/QEMU pipeline entirely: start.ps1
    # hands them to docker.ps1 -Profile <DockerProfile>. Needs Docker Desktop.

    @{
        Id            = "docker-windows"
        Name          = "Windows 11 in Docker"
        Description   = "Windows 11 Pro as a KVM VM inside a container (dockur/windows). Installs from data/win11-pro.iso, runs every .exe in scripts/wine-apps at the end of setup, shares that folder as Z:. Web viewer http://127.0.0.1:8006, RDP 127.0.0.1:3389."
        Runtime       = "docker"
        DockerProfile = "windows"
        OsFamily      = "windows"
    },
    @{
        Id            = "docker-arch-wine"
        Name          = "Arch + Wine desktop in Docker"
        Description   = "Arch Linux XFCE desktop with Wine served to a browser tab (linuxserver webtop). Launches every .exe in scripts/wine-apps under Wine at login. https://127.0.0.1:3001."
        Runtime       = "docker"
        DockerProfile = "arch-wine"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # WINDOWS
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id          = "win11-home"
        Name        = "Windows 11 Home"
        Description = "Local retail copy of Windows 11 Home edition (Unattended ISO)."
        File        = "win11-home.iso"
        DiskSize    = "64G"
        OsFamily    = "windows"
        Firmware    = "uefi"
    },
    @{
        Id          = "win11-pro"
        Name        = "Windows 11 Pro"
        Description = "Local retail copy of Windows 11 Pro edition (Unattended ISO)."
        File        = "win11-pro.iso"
        DiskSize    = "64G"
        OsFamily    = "windows"
        Firmware    = "uefi"
    },
    @{
        Id          = "windows11"
        Name        = "Windows 11 Enterprise (Evaluation)"
        Description = "Microsoft Windows 11 Enterprise (Evaluation), requiring UEFI and virtual TPM."
        Url         = "https://archive.org/download/windows-11-enterprise-evaluation-iso/22000.194.210913-1444.co_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso"
        IsoName     = "22000.194.210913-1444.co_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso"
        File        = "windows11-enterprise-eval.iso"
        HashUrlSha1 = "https://archive.org/download/windows-11-enterprise-evaluation-iso/windows-11-enterprise-evaluation-iso_files.xml"
        # QEMU Profile
        DiskSize    = "64G"
        OsFamily    = "windows"
        Firmware    = "uefi"
    },
    @{
        Id          = "windows10"
        Name        = "Windows 10 Enterprise (Evaluation)"
        Description = "Microsoft Windows 10 Enterprise (Evaluation) for client desktop workloads."
        Url         = "https://archive.org/download/Win10_Enterprise_Eval_x64/19044.1288.211006-0501.21h2_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso"
        IsoName     = "19044.1288.211006-0501.21h2_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso"
        File        = "windows10-enterprise-eval.iso"
        HashUrlSha1 = "https://archive.org/download/Win10_Enterprise_Eval_x64/Win10_Enterprise_Eval_x64_files.xml"
        # QEMU Profile
        DiskSize    = "64G"
        OsFamily    = "windows"
    },
    @{
        Id          = "windows8"
        Name        = "Windows 8.1 Pro VL (x64)"
        Description = "Legacy Microsoft Windows 8.1 Professional VL (x64) installation."
        Url         = "https://archive.org/download/win-8.1-pro-vl-x-64/Win8.1_Pro_VL_x64_English.iso"
        IsoName     = "Win8.1_Pro_VL_x64_English.iso"
        File        = "windows8.1-pro-vl-x64.iso"
        # Archive.org _files.xml contains per-file SHA1 hashes in XML format
        HashUrlSha1 = "https://archive.org/download/win-8.1-pro-vl-x-64/win-8.1-pro-vl-x-64_files.xml"
        # QEMU Profile
        DiskSize    = "40G"
        OsFamily    = "windows"
    },
    @{
        Id          = "windows7"
        Name        = "Windows 7 SP1 Ultimate"
        Description = "Classic Microsoft Windows 7 SP1 Ultimate (x64) installation."
        Url         = "https://archive.org/download/windows-7-ultimate-x-64-sp-1-fully-updated/Windows%207%20Ultimate%20x64%20-%20SP1%20%28Fully%20Updated%29.iso"
        IsoName     = "Windows 7 Ultimate x64 - SP1 (Fully Updated).iso"
        File        = "windows7-ultimate-sp1.iso"
        # Archive.org _files.xml contains per-file SHA1 hashes in XML format
        HashUrlSha1 = "https://dn760108.eu.archive.org/0/items/windows-7-ultimate-x-64-sp-1-fully-updated/windows-7-ultimate-x-64-sp-1-fully-updated_files.xml"
        # QEMU Profile
        DiskSize    = "40G"
        OsFamily    = "windows"
    },
    @{
        Id          = "windowsxp"
        Name        = "Windows XP Professional SP3 (x86)"
        Description = "Legacy Microsoft Windows XP Professional SP3 (x86) installation."
        Url         = "https://archive.org/download/WinXPProSP3x86/en_windows_xp_professional_with_service_pack_3_x86_cd_vl_x14-73974.iso"
        IsoName     = "en_windows_xp_professional_with_service_pack_3_x86_cd_vl_x14-73974.iso"
        File        = "windowsxp-pro-sp3-x86.iso"
        # Archive.org _files.xml contains per-file MD5 hashes in XML format
        HashUrlMd5  = "https://archive.org/download/WinXPProSP3x86/WinXPProSP3x86_files.xml"
        # QEMU Profile
        DiskSize    = "20G"
        OsFamily    = "windows"
    },
    @{
        Id          = "windowsserver"
        Name        = "Windows Server 2022 (Evaluation)"
        Description = "Microsoft Windows Server 2022 Evaluation edition."
        Url         = "https://archive.org/download/windows-server-2022_build-20348.169/20348.169.210806-2348.fe_release_svc_refresh_SERVER_EVAL_x64FRE_en-us.iso"
        IsoName     = "20348.169.210806-2348.fe_release_svc_refresh_SERVER_EVAL_x64FRE_en-us.iso"
        File        = "windows-server-2022-eval.iso"
        HashUrlSha1 = "https://archive.org/download/windows-server-2022_build-20348.169/windows-server-2022_build-20348.169_files.xml"
        # QEMU Profile
        DiskSize    = "64G"
        OsFamily    = "windows"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # CLOUD IMAGES (no installer: boot the image, cloud-init does the rest)
    # ══════════════════════════════════════════════════════════════════════════
    # ImageKind = "cloud" tells the pipeline the download is a bootable qcow2,
    # not an installer ISO: every VM disk is a linked clone of the verified
    # image, no CD-ROM is attached, and scripts/cloud-init/<CloudInit>/user-data
    # is served as the NoCloud seed with a per-VM generated meta-data.

    @{
        Id            = "ubuntu-cloud"
        Name          = "Ubuntu 24.04 LTS (cloud image, headless)"
        Description   = "Official Ubuntu Noble cloud image booted directly with cloud-init. Headless server with SSH, Docker, ufw -- ready in about a minute."
        Url           = "https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img"
        IsoName       = "noble-server-cloudimg-amd64.img"
        File          = "ubuntu-noble-cloudimg-amd64.img"
        HashUrlSha256 = "https://cloud-images.ubuntu.com/noble/current/SHA256SUMS"
        # QEMU Profile
        ImageKind     = "cloud"
        CloudInit     = "server"
        # UEFI: under WHPX the legacy SeaBIOS/GRUB real-mode boot path hangs at
        # "Booting from Hard Disk"; the cloud image ships an EFI system partition.
        Firmware      = "uefi"
        DiskSize      = "40G"
        OsFamily      = "linux"
    },
    @{
        Id            = "ubuntu-cloud-desktop"
        Name          = "Ubuntu 24.04 LTS (cloud image, desktop)"
        Description   = "Same cloud image, plus a minimal GNOME desktop with auto-login installed by cloud-init on first boot (long first boot)."
        Url           = "https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img"
        IsoName       = "noble-server-cloudimg-amd64.img"
        File          = "ubuntu-noble-cloudimg-amd64.img"
        HashUrlSha256 = "https://cloud-images.ubuntu.com/noble/current/SHA256SUMS"
        # QEMU Profile
        ImageKind     = "cloud"
        CloudInit     = "desktop"
        # UEFI: under WHPX the legacy SeaBIOS/GRUB real-mode boot path hangs at
        # "Booting from Hard Disk"; the cloud image ships an EFI system partition.
        Firmware      = "uefi"
        DiskSize      = "60G"
        OsFamily      = "linux"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # ANDROID (Mobile/Desktop OS)
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id            = "android"
        Name          = "Android-x86 (9.0-r2 Pie)"
        Description   = "An open-source project to port Android to the x86 platform, running smoothly under QEMU."
        Url           = "https://downloads.sourceforge.net/project/android-x86/Release%209.0/android-x86_64-9.0-r2.iso"
        IsoName       = "android-x86_64-9.0-r2.iso"
        File          = "android-x86_64-9.0-r2.iso"
        HashUrlSha1   = "https://raw.githubusercontent.com/android-x86/android-x86-com.github.io/master/releases/releasenote-9-0-r2.html"
        # QEMU Profile
        DiskSize      = "16G"
        OsFamily      = "android"
    },

    # ══════════════════════════════════════════════════════════════════════════
    # RETRO HANDHELD FIRMWARE (Download & verify only — flash to SD card)
    # ══════════════════════════════════════════════════════════════════════════

    @{
        Id                 = "minui"
        Name               = "MinUI (Handheld Launcher)"
        Description        = "Minimal custom launcher for retro handhelds (Anbernic, Miyoo, Trimui). Download-only: copy to SD card."
        # Dynamic Resolver Configuration
        ResolverType       = "GitHubAsset"
        ResolverRepo       = "shauninman/MinUI"
        # Asset names drop the tag's leading 'v' (v20251127-1 -> MinUI-20251127-1-base.zip),
        # so the file is resolved from the release asset list rather than a URL template.
        ResolverAssetRegex = '^MinUI-.*-base\.zip$'
        # Templates
        FileTemplate       = 'minui-$v.zip'
        # Verification: MinUI publishes no checksum files or signatures — the
        # resolver pins the SHA256 digest GitHub computes for the asset.
        # ARM handheld firmware, not bootable in QEMU — acquisition + verification only.
        DownloadOnly       = $true
        OsFamily           = "linux"
    },
    @{
        Id                 = "knulli"
        Name               = "Knulli CFW (Anbernic RG34XX)"
        Description        = "Batocera-based custom firmware for retro handhelds. RG34XX SD image. Download-only: flash to SD card."
        # Dynamic Resolver Configuration
        ResolverType       = "GitHubAsset"
        ResolverRepo       = "knulli-cfw/distribution"
        # Asset names embed per-device build dates and release codenames that do
        # not match the release tag, so the file is resolved from the asset list.
        # Swap the device slug (rg35xx-sp, rg-cubexx, trimui-brick, ...) to fetch
        # a different image; (?!sp-) keeps rg34xx from matching rg34xx-sp.
        ResolverAssetRegex = '^knulli-h700-rg34xx-(?!sp-).*\.img\.gz$'
        # Templates
        FileTemplate       = 'knulli-rg34xx-$v.img.gz'
        # Verification: .sha256/.md5 companion assets + GitHub API asset digest,
        # all wired up dynamically by the GitHubAsset resolver.
        # ARM handheld firmware, not bootable in QEMU — acquisition + verification only.
        DownloadOnly       = $true
        OsFamily           = "linux"
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
