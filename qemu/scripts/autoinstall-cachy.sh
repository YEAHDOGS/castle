#!/usr/bin/env bash

# Ensure we are running as root in the live ISO
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo)."
  exit 1
fi

echo "🚀 Starting automated CachyOS installation..."

# Launch Calamares in target mode pointing to your automated settings
calamares --platform wayland --config ./autoinstall-cachy.yaml