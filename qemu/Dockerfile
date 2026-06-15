# Use a lightweight Debian image as base
FROM debian:bookworm-slim

# Prevent interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install core dependencies, QEMU suite, NoVNC, websockify, and tools
RUN apt-get update && apt-get install -y \
    curl \
    gnupg \
    apt-transport-https \
    ca-certificates \
    qemu-system-x86 \
    qemu-utils \
    procps \
    iptables \
    iproute2 \
    novnc \
    websockify \
    && rm -rf /var/lib/apt/lists/*

# Add Microsoft official production package repository and install PowerShell Core (pwsh)
RUN curl -s https://packages.microsoft.com/keys/microsoft.asc | gpg --dearmor > /etc/apt/trusted.gpg.d/microsoft.gpg \
    && echo "deb [arch=amd64] https://packages.microsoft.com/debian/12/prod bookworm main" > /etc/apt/sources.list.d/microsoft.list \
    && apt-get update && apt-get install -y powershell \
    && rm -rf /var/lib/apt/lists/*

# Set up application directory
WORKDIR /app

# Copy provisioning files, modules, and default autoinstall profiles
COPY modules/ /app/modules/
COPY scripts/ /app/scripts/
COPY start.ps1 /app/start.ps1
COPY README.md /app/README.md
COPY entrypoint.sh /app/entrypoint.sh

# Make startup script executable
RUN chmod +x /app/entrypoint.sh

# Expose NoVNC port (HTTP interface)
EXPOSE 8006

# Declare mount point for persistent caching of ISOs and virtual disks
VOLUME /app/data

# Run entrypoint script
ENTRYPOINT ["/app/entrypoint.sh"]
