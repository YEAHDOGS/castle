# ==============================================================================
# Castle -- Arch Linux desktop + Wine, served to a browser tab
# ==============================================================================
# Base: linuxserver.io "webtop" (Arch + XFCE, KasmVNC web desktop). On top of it:
# multilib enabled, Wine (64 + 32-bit), Wine's Mono/Gecko so no first-run
# download prompts, and an XFCE autostart entry that launches every Windows
# .exe from clonewars/scripts/wine-apps/ (mounted read-only at /castle/wine-apps)
# as soon as the desktop is up.
#
# This is a container, not a VM: it shares the host kernel (WSL2 on Windows).
# It is the fastest way to a Linux desktop with your apps running, at the cost
# of VM-grade isolation. See compose.yml for the hardening applied.
# ==============================================================================
FROM lscr.io/linuxserver/webtop:arch-xfce

# Enable [multilib] (32-bit libraries) and install Wine. The webtop image ships a
# trimmed pacman.conf with no multilib block to uncomment, so append one.
# The stock mirrorlist has two mirrors and pacman pulls 5 files at once, which
# drops TLS connections through Docker Desktop's NAT; add fallbacks and go serial.
# nginx is pinned: the image's fancyindex module is built by linuxserver against
# the shipped nginx and a newer nginx refuses to load it (web desktop crash-loops).
RUN grep -q '^\[multilib\]' /etc/pacman.conf || \
    printf '\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n' >> /etc/pacman.conf && \
    sed -i 's/^ParallelDownloads = .*/ParallelDownloads = 1/' /etc/pacman.conf && \
    printf 'Server = https://mirrors.kernel.org/archlinux/$repo/os/$arch\nServer = https://mirror.rackspace.com/archlinux/$repo/os/$arch\nServer = https://america.mirror.pkgbuild.com/$repo/os/$arch\n' >> /etc/pacman.d/mirrorlist && \
    pacman -Syu --noconfirm --needed --ignore nginx \
        wine wine-mono wine-gecko winetricks \
        lib32-gnutls lib32-libpulse lib32-alsa-plugins \
        ttf-dejavu ttf-liberation && \
    pacman -Scc --noconfirm

COPY castle-wine-launch /usr/local/bin/castle-wine-launch
COPY 50-castle-autostart.sh /custom-cont-init.d/50-castle-autostart.sh
RUN chmod 0755 /usr/local/bin/castle-wine-launch /custom-cont-init.d/50-castle-autostart.sh
