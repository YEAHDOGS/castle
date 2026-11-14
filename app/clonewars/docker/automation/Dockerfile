# ==============================================================================
# Castle automation container: the light way to run a bot
# ==============================================================================
# One image every automation bot shares. A bot is a container made from it: no
# kernel to boot, no init system, no desktop environment, just an X server in
# memory (Xvfb), a window manager (Openbox) and whatever the bot's setup puts
# on top. Its writable layer is its disk: packages a setup installs and the
# home folder stay with the bot until the container is removed.
#
# Live control (castle-live: x11vnc + noVNC on port 6080) is off until the
# app's "Take control" asks for it, behind a one-time password.
#
# Built by container.ps1 (tagged by a hash of this folder, so an edit here
# makes the next launch rebuild). The guest scripts are the same ones a VM
# gets: castle-lib.sh works out that it is in a container (no systemd, no
# cloud-init, no scratch disk) and falls back to what is here.
# ==============================================================================
FROM debian:trixie-slim

ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8

# What every setup script leans on. The rest (Firefox, Python libraries, ...)
# is installed by the bot's own setup the first time it runs, and kept.
RUN apt-get update && apt-get install -y --no-install-recommends \
        xvfb openbox xdotool x11-xserver-utils x11-utils x11-apps xterm netpbm \
        sudo ca-certificates curl procps psmisc libarchive-tools \
        python3 python3-venv fonts-dejavu-core dbus-x11 \
        x11vnc novnc python3-websockify \
    && rm -rf /var/lib/apt/lists/*

# The desktop user, uid 1000 like the VM guests. No sudo of its own: the
# setup runs as root and hands commands to this user.
RUN useradd -m -u 1000 -s /bin/bash castle

COPY castle-container-init castle-screen castle-live /usr/local/bin/
RUN cd /usr/local/bin && sed -i 's/\r$//' castle-container-init castle-screen castle-live \
    && chmod 755 castle-container-init castle-screen castle-live

LABEL net.wearedogs.castle.kind="automation"
ENTRYPOINT ["/usr/local/bin/castle-container-init"]
