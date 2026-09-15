#!/usr/bin/env bash
# ==============================================================================
# Castle -- rebuild a Windows install folder into an ISO that dockur/windows
# can drive unattended. Runs INSIDE a throwaway dockurr/windows container
# (build-iso.ps1 starts it); that image already ships wimlib + genisoimage.
# ==============================================================================
# Why: Castle's own win11-pro.iso is built for a FAT32 USB stick, so its
# install image is split into sources/install.swm + install2.swm and it carries
# Castle's autounattend.xml. dockur only recognises install.wim / install.esd,
# so it falls back to a manual install: no virtio drivers, no answer file, and
# Castle's answer file then fails because it cannot see the virtio disk.
#
# This merges the .swm parts back into one install.wim (no recompression) and
# drops Castle's autounattend.xml, so dockur detects the edition, injects its
# drivers and runs its own unattended setup (which also runs /oem/install.bat).
#
#   /src  Windows install folder, read-only (clonewars/data/win11-pro)
#   /out  destination folder; the ISO is written as $ISO_NAME
set -euo pipefail
SRC=/src
OUT=/out/${ISO_NAME:-win11-pro-docker.iso}
WORK=/tmp/iso

[ -f "$SRC/sources/install.swm" ] || [ -f "$SRC/sources/install.wim" ] || {
  echo "ERROR: $SRC/sources has neither install.swm nor install.wim" >&2; exit 1; }

echo "[1/3] Copying boot files (everything except the install image and Castle's answer file)..."
mkdir -p "$WORK"
# Over Docker Desktop's file sharing (v9fs) the directory mtime changes while
# tar reads it; tar then exits 1 ("file changed as we read it") although every
# file was copied. Exit 1 is tolerated, anything else is a real failure.
{ tar -C "$SRC" --warning=no-file-changed \
      --exclude='./sources/install*.swm' --exclude='./sources/install.wim' \
      --exclude='./autounattend.xml' -cf - . || [ $? -eq 1 ]; } | tar -C "$WORK" -xf -

echo "[2/3] Merging the install image into sources/install.wim..."
if [ -f "$SRC/sources/install.wim" ]; then
  cp "$SRC/sources/install.wim" "$WORK/sources/install.wim"
else
  wimlib-imagex export "$SRC/sources/install.swm" all "$WORK/sources/install.wim" \
      --ref="$SRC/sources/install*.swm"
fi
wimlib-imagex info "$WORK/sources/install.wim" | grep -E '^(Image Count|Name|Display Name):' | head -6

echo "[3/3] Building $OUT ..."
rm -f "$OUT"
genisoimage -o "$OUT" \
    -b boot/etfsboot.com -no-emul-boot -boot-load-size 8 -c BOOT.CAT \
    -iso-level 4 -J -l -D -N -joliet-long -relaxed-filenames -V CCCOMA_X64FRE_EN-US_DV9 \
    -udf -boot-info-table \
    -eltorito-alt-boot -eltorito-boot efi/microsoft/boot/efisys.bin -no-emul-boot \
    -allow-limited-size -quiet "$WORK" \
    2> >(grep -v 'does not conform to ISO-9660' >&2 || true)
ls -l "$OUT"
echo "DONE"
