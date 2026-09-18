#!/usr/bin/env bash
# ==============================================================================
# Castle: helpers shared by the setup-*.sh guest scripts (sourced, not run).
# ==============================================================================

# castle_scratch_setup <user> <home>
#   The host attaches a sparse qcow2 as a virtio disk with serial
#   "castlescratch" (start.ps1 -Copy). On a live ISO everything written lands
#   in RAM, so the payload copy, the Wine prefix and OpenGOAL's build all move
#   onto that disk instead, and they survive reboots. The disk is only ever
#   touched when it carries that serial, so no other disk can be formatted by
#   mistake. Without the disk this is a no-op and everything stays in RAM.
castle_scratch_setup() {
  local user="$1" home="$2" dev="/dev/disk/by-id/virtio-castlescratch" mnt="/mnt/castle-scratch" i d
  for i in 1 2 3 4 5; do [ -e "$dev" ] && break; sleep 1; done
  if [ ! -e "$dev" ]; then
    echo "[i] No scratch disk attached; working in RAM."
    return 0
  fi
  if ! blkid -o value -s TYPE "$dev" 2>/dev/null | grep -q .; then
    echo "[+] Scratch disk is blank -- formatting it (ext4, label CASTLESCRATCH)..."
    mkfs.ext4 -q -L CASTLESCRATCH -E lazy_itable_init=1,lazy_journal_init=1 "$dev"
  fi
  mkdir -p "$mnt"
  mountpoint -q "$mnt" || mount -o noatime,discard "$dev" "$mnt" || { echo "[-] Could not mount the scratch disk; working in RAM."; return 0; }
  # Heavy folders live on the scratch disk; the home directory only holds links.
  for d in castle-copy opengoal .wine; do
    mkdir -p "$mnt/$d"
    if [ -e "$home/$d" ] && [ ! -L "$home/$d" ]; then
      # Something already there from an earlier RAM-only run: fold it in.
      cp -a "$home/$d/." "$mnt/$d/" 2>/dev/null && rm -rf "$home/$d"
    fi
    ln -sfn "$mnt/$d" "$home/$d"
  done
  chown -R "$user:" "$mnt" 2>/dev/null || true
  echo "[+] Scratch disk: $dev on $mnt ($(df -h "$mnt" | awk 'NR==2 {print $4}') free) -> ~/castle-copy, ~/opengoal, ~/.wine"
}
