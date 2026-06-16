#!/usr/bin/env bash

# Ensure we are running as root in the live ISO
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo)."
  exit 1
fi

echo "🚀 Starting automated CachyOS installation..."

# Launch the CachyOS CLI installer with the configuration
# If this command differs, please replace it with the correct cachyos-installer command
cachyos-installer --cli --config ./autoinstall-cachy.yml

echo "📦 Installation complete. Entering chroot to run post-install tasks..."

# Standard Arch installers mount the newly installed system to /mnt
# We run these commands inside the installed system so they actually persist
arch-chroot /mnt /bin/bash << 'EOF'
  echo "🔄 Updating system..."
  # Assuming cachy-update has a flag or accepts 'yes' implicitly, 
  # or you might need to pipe 'yes' into it: yes | cachy-update
  cachy-update

  echo "🧹 Removing orphaned packages..."
  # Use || true to prevent the script from failing if there are no orphans
  pacman -Rns $(pacman -Qtdq) --noconfirm || true

  echo "🔑 Resetting pacman keyrings..."
  rm -rf /etc/pacman.d/gnupg
  pacman-key --init
  pacman-key --populate archlinux cachyos
  pacman -Sy archlinux-keyring cachyos-keyring --noconfirm

  echo "✅ Post-install tasks completed successfully!"
EOF