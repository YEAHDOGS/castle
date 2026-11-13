# ==============================================================================
# Castle Android container: Google's emulator, one per bot
# ==============================================================================
# The same Android Emulator android.ps1 runs on the host, inside a container:
# headless, accelerated by the KVM the Docker engine exposes (container.ps1
# sets /dev/kvm up in Docker Desktop's VM). Every Android bot is a container
# made from this image; they share the SDK and the system image, and each one
# keeps its own phone (the AVD's data) in its writable layer.
#
# Downloads and how they are checked (the same way android.ps1 does it):
#   * cmdline-tools: zip name and SHA-1 read from Google's SDK manifest over
#     HTTPS; the zip must match before it is unpacked.
#   * platform-tools, emulator, system image: installed by Google's own tool,
#     which checks each archive against the same manifests.
# The SDK license is the one already accepted on the host: container.ps1
# copies data/android-sdk/licenses into the build, and without it the build
# stops.
# ==============================================================================
FROM debian:trixie-slim

# google_apis (not Play Store) so root tasks work: `su 0` exists on it.
ARG ANDROID_IMAGE="system-images;android-35;google_apis;x86_64"
ARG ANDROID_DEVICE="pixel_7"
ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8 \
    ANDROID_HOME=/opt/android-sdk ANDROID_SDK_ROOT=/opt/android-sdk \
    ANDROID_USER_HOME=/opt/android-home ANDROID_AVD_HOME=/opt/android-home/avd ANDROID_EMULATOR_HOME=/opt/android-home
ENV PATH="$PATH:/opt/android-sdk/platform-tools:/opt/android-sdk/emulator:/opt/android-sdk/cmdline-tools/latest/bin"

# The emulator's own libraries (it links X11 and GL even with no window),
# Java for Google's SDK tools, and what the scripts below use.
RUN apt-get update && apt-get install -y --no-install-recommends \
        openjdk-21-jre-headless ca-certificates curl unzip python3 procps \
        libdbus-1-3 libfontconfig1 libpulse0 libx11-6 libx11-xcb1 libxcb1 libxdamage1 libnss3 \
        libxcomposite1 libxcursor1 libxi6 libxext6 libxfixes3 libxrender1 libxtst6 libxkbfile1 \
        libgl1 libegl1 libgbm1 libdrm2 libasound2t64 zlib1g libbsd0 \
    && rm -rf /var/lib/apt/lists/*

COPY licenses/ /opt/android-sdk/licenses/
RUN test -s /opt/android-sdk/licenses/android-sdk-license || { echo "No accepted Android SDK license in the build: run android.ps1 once on the host and accept it there."; exit 1; }

COPY fetch-cmdline-tools.py /tmp/fetch-cmdline-tools.py
RUN set -eu; cd /tmp; \
    curl -fsSL https://dl.google.com/android/repository/repository2-3.xml -o manifest.xml; \
    python3 fetch-cmdline-tools.py manifest.xml > cmdline-tools.txt; \
    read -r ZIP SHA1 < cmdline-tools.txt; \
    echo "cmdline-tools: $ZIP (sha1 $SHA1)"; \
    curl -fsSL "https://dl.google.com/android/repository/$ZIP" -o tools.zip; \
    echo "$SHA1  tools.zip" | sha1sum -c -; \
    unzip -q tools.zip -d unpack; mkdir -p /opt/android-sdk/cmdline-tools; \
    mv unpack/cmdline-tools /opt/android-sdk/cmdline-tools/latest; \
    rm -rf tools.zip unpack manifest.xml cmdline-tools.txt fetch-cmdline-tools.py

# Newer cmdline-tools hand installs to Google's "Android CLI" (bin/android);
# older ones have the classic sdkmanager. Either checks every archive.
RUN set -eu; BIN=/opt/android-sdk/cmdline-tools/latest/bin; \
    if [ -x "$BIN/android" ]; then \
        "$BIN/android" --no-metrics --sdk=/opt/android-sdk sdk install platform-tools emulator "$(echo "$ANDROID_IMAGE" | tr ';' '/')"; \
    else \
        "$BIN/sdkmanager" --sdk_root=/opt/android-sdk --install platform-tools emulator "$ANDROID_IMAGE"; \
    fi; \
    test -x /opt/android-sdk/emulator/emulator; test -x /opt/android-sdk/platform-tools/adb

RUN set -eu; mkdir -p "$ANDROID_AVD_HOME"; \
    echo no | avdmanager create avd -n castle -k "$ANDROID_IMAGE" -d "$ANDROID_DEVICE"; \
    CFG="$ANDROID_AVD_HOME/castle.avd/config.ini"; \
    sed -i '/^disk\.dataPartition\.path/d; /^hw\.gpu\./d; /^hw\.keyboard/d; /^showDeviceFrame/d; /^disk\.dataPartition\.size/d' "$CFG"; \
    printf '%s\n' "hw.keyboard=yes" "hw.gpu.enabled=yes" "hw.gpu.mode=swiftshader_indirect" "showDeviceFrame=no" "disk.dataPartition.size=8G" >> "$CFG"

COPY castle-android-init /usr/local/bin/castle-android-init
COPY castle-screen /usr/local/bin/castle-screen
RUN sed -i 's/\r$//' /usr/local/bin/castle-android-init /usr/local/bin/castle-screen \
    && chmod 755 /usr/local/bin/castle-android-init /usr/local/bin/castle-screen

LABEL net.wearedogs.castle.kind="android"
ENTRYPOINT ["/usr/local/bin/castle-android-init"]
