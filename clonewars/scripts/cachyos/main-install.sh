#!/usr/bin/env bash
# ==============================================================================
# Castle guest provisioner -- CachyOS / Arch-based VMs
# ==============================================================================
# Run INSIDE the guest. The host attaches clonewars/scripts/ as a read-only
# USB drive (see modules/launch.ps1), so this file is reachable in the guest
# without any networking: mount the USB stick and run it with sudo.
#
# Implements the package checklist from qemu-notes.md:
#   WireGuard, Tailscale, ufw firewall, Samba shares, OpenSSH,
#   Docker, QEMU guest agent, btrfs tooling.
#
# Optional:
#   CASTLE_INSTALL_OLLAMA=1  also install Ollama (local LLM runtime)
#
# Pi-hole is intentionally NOT installed here -- it wants to own DNS for the
# whole LAN, so it belongs on dedicated hardware or its own container, not
# inside every guest. Run it via Docker on the host when ready.
# ==============================================================================
set -euo pipefail

if [ "$EUID" -ne 0 ]; then
  echo "[castle] Please run as root (sudo $0)." >&2
  exit 1
fi

if ! command -v pacman >/dev/null 2>&1; then
  echo "[castle] pacman not found -- this provisioner targets Arch-based guests (CachyOS)." >&2
  exit 1
fi

PACMAN="pacman -S --needed --noconfirm"

echo "[castle] === 1/5 refreshing package databases ==="
pacman -Sy --noconfirm

echo "[castle] === 2/5 core networking + firewall ==="
$PACMAN wireguard-tools tailscale ufw openssh

echo "[castle] === 3/5 storage + network shares ==="
$PACMAN samba btrfs-progs

echo "[castle] === 4/5 virtualization guest tooling ==="
$PACMAN qemu-guest-agent spice-vdagent docker

echo "[castle] === 5/5 enabling + hardening services ==="
systemctl enable --now sshd
systemctl enable --now qemu-guest-agent
systemctl enable --now docker
systemctl enable --now tailscaled

# ufw: default-deny inbound, allow SSH first so we don't lock ourselves out,
# then enable non-interactively.
ufw --force reset >/dev/null
ufw default deny incoming
ufw default allow outgoing
ufw allow OpenSSH
ufw --force enable
systemctl enable --now ufw

# Samba: enabled but unconfigured -- the operator defines shares in
# /etc/samba/smb.conf (see README "Data Vault" notes).
systemctl enable smb nmb

if [ "${CASTLE_INSTALL_OLLAMA:-0}" = "1" ]; then
  echo "[castle] === optional: installing Ollama ==="
  curl -fsSL https://ollama.com/install.sh | sh
  systemctl enable --now ollama
fi

echo ""
echo "[castle] provision complete."
echo "[castle] next steps:"
echo "  * sudo tailscale up               # join the tailnet (Drawbridge VPN)"
echo "  * configure /etc/samba/smb.conf   # Data Vault shares, then: sudo systemctl start smb nmb"
echo "  * ./test.sh                       # run the guest smoke test"
