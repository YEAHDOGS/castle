#!/bin/bash
# ==============================================================================
# Castle: install the XFCE autostart entry into the user's config volume.
# ==============================================================================
# linuxserver.io images run every script in /custom-cont-init.d as root during
# container init. /config is a persistent volume (the user's home), so the
# autostart entry has to be written there at start rather than baked into the
# image layer, which the volume would hide.
mkdir -p /config/.config/autostart
cat > /config/.config/autostart/castle-wine-apps.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Castle Wine Apps
Comment=Launch the Windows apps from /castle/wine-apps under Wine
Exec=/usr/local/bin/castle-wine-launch
Terminal=false
X-GNOME-Autostart-enabled=true
EOF
chown -R abc:abc /config/.config/autostart
echo "[castle] Wine autostart entry installed."
