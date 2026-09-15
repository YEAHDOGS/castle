#!/usr/bin/env bash
# ==============================================================================
# Castle VM -- boot any ISO in QEMU with a disk of the size you choose (Linux)
# ==============================================================================
# Point it at an ISO (EndeavourOS, Ubuntu, Kali, Arch, anything bootable) and a
# disk size. It creates a qcow2 of that size under data/adhoc/, boots the
# installer from the ISO on the first run, and boots the installed disk on
# every later run (the ISO stays attached as a CD for repairs). Standalone: no
# manifest, no download -- for images you already have on disk.
#
# Verification is optional: give a checksum and/or a detached GPG signature and
# the ISO is checked before booting. Files next to the ISO under the usual
# names are picked up automatically (--no-verify skips that). Nothing is
# verified unless something was given or found, and only a failing check that
# was requested refuses to boot (--force overrides).
#
# Usage:
#   ./boot-iso.sh <iso> [size] [options]
#     size              5, 5G, 5GB, 10gb, 15G, 40G ...   (default 10G)
#   disk:
#     --name NAME       VM name; disk is data/adhoc/NAME.qcow2 (default <iso>-<size>)
#     --disk PATH       explicit qcow2 path
#     --fresh           delete the existing disk (+ NVRAM) so the installer runs again
#     --boot cd|disk    force boot device (default: CD on a blank disk, disk afterwards)
#     --snapshot        QEMU -snapshot: all writes discarded at exit (try a live ISO safely)
#   machine:
#     --uefi            boot with OVMF (per-VM NVRAM next to the disk)
#     --windows         ide disk + e1000 NIC (Windows / macOS guests) instead of virtio
#     --machine pc|q35  chipset (default pc)
#     --cpu MODEL       -cpu model (default: host with KVM, max without)
#     --cores N         guest cores (default: host cores minus 2, min 2)
#     --mem 4G          guest RAM (default: half of host RAM, 4-8 GB; CASTLE_VM_MAX_RAM_GB raises the cap)
#     --accel kvm|tcg   accelerator (default: kvm if /dev/kvm is usable)
#   devices:
#     --vga std|virtio|qxl|vmware|cirrus|none   (default std; virtio is best for Linux desktops)
#     --display gtk|sdl|none                     (default: QEMU's default window)
#     --audio [BACKEND] sound card (intel-hda); backend pa|pipewire|alsa (default pa)
#     --forward H:G[,H:G] host->guest TCP forwards on 127.0.0.1, e.g. 2222:22,8080:80
#     --no-net          no network card
#     --share DIR       host folder exposed read-only as a USB stick labelled SHARE
#     --localtime       guest RTC in host local time (Windows guests)
#     --extra "ARGS"    raw QEMU arguments appended verbatim
#   verification (optional):
#     --sha512 SRC      hex digest, .sha512/.sha512sum/SHA512SUMS file, or URL to one
#     --sha256 SRC      hex digest, .sha256/.sha256sum/SHA256SUMS file, or URL to one
#     --sig SRC         detached GPG signature (.sig/.asc file or URL) of the ISO, or of
#                       the checksum file when named so (SHA256SUMS.gpg, SHA256SUMS.sign)
#     --gpg-key ID      import this signing key first (else a missing key is fetched by id)
#     --gpg-server URL  keyserver (default hkps://keys.openpgp.org)
#     --no-verify       do not auto-detect checksum/signature files next to the ISO
#     --force           boot even if a requested verification failed
#   run:
#     --background      detach QEMU and return to the shell (prints the PID)
#     --vnc             display over VNC on 127.0.0.1:5900 instead of a window
#     --dry-run         print the QEMU command line and exit
#
# Examples:
#   ./boot-iso.sh data/endeavouros-Titan-Nova-2026.08.15.iso 10G
#   ./boot-iso.sh ~/isos/EndeavourOS_Titan-Nova-2026.08.15.iso 15gb --sig ~/isos/EndeavourOS_Titan-Nova-2026.08.15.iso.sig --sha512 ~/isos/EndeavourOS_Titan-Nova-2026.08.15.iso.sha512sum
#   ./boot-iso.sh ~/isos/ubuntu-24.04.3-desktop-amd64.iso 15G --sha256 ~/isos/SHA256SUMS --sig ~/isos/SHA256SUMS.gpg
#   ./boot-iso.sh ~/isos/kali-linux-2026.3-installer-amd64.iso 20G --vga virtio --audio --forward 2222:22
#   ./boot-iso.sh data/endeavouros-Titan-Nova-2026.08.15.iso 5G --snapshot --fresh --uefi
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="$SCRIPT_DIR/data"
SCRIPTS_DIR="$SCRIPT_DIR/scripts"

die()  { echo "  [FAIL] $*" >&2; exit 1; }
note() { echo "  $*"; }
usage() { sed -n '3,63p' "$0"; }

# ------------------------------------------------------------------------------
# Arguments
# ------------------------------------------------------------------------------
ISO=""; SIZE="10G"; NAME=""; DISK=""; FRESH=0; BOOT_FORCE=""; SNAPSHOT=0
FIRMWARE="bios"; DISK_IF="virtio"; NIC_MODEL="virtio-net-pci"; MACHINE="pc"; CPU=""; CORES=""; MEM=""; ACCEL=""
VGA="std"; DISPLAY_MODE=""; AUDIO=""; FORWARD=""; NONET=0; SHARE=""; LOCALTIME=0; EXTRA=""
SHA512=""; SHA256=""; SIG=""; GPG_KEY=""; GPG_SERVER="hkps://keys.openpgp.org"; NOVERIFY=0; FORCE=0
BACKGROUND=0; VNC=0; DRYRUN=0

positional=()
while [ $# -gt 0 ]; do
    case "$1" in
        --name)        NAME="$2"; shift 2 ;;
        --disk)        DISK="$2"; shift 2 ;;
        --fresh)       FRESH=1; shift ;;
        --boot)        BOOT_FORCE="$2"; shift 2 ;;
        --snapshot)    SNAPSHOT=1; shift ;;
        --uefi)        FIRMWARE="uefi"; shift ;;
        --windows|--ide) DISK_IF="ide"; NIC_MODEL="e1000"; shift ;;
        --machine)     MACHINE="$2"; shift 2 ;;
        --cpu)         CPU="$2"; shift 2 ;;
        --cores)       CORES="$2"; shift 2 ;;
        --mem|--memory) MEM="$2"; shift 2 ;;
        --accel)       ACCEL="$2"; shift 2 ;;
        --vga)         VGA="$2"; shift 2 ;;
        --display)     DISPLAY_MODE="$2"; shift 2 ;;
        --audio)       AUDIO="pa"; if [ $# -ge 2 ] && [[ "$2" != --* ]] && [[ "$2" =~ ^(pa|pipewire|alsa|oss|sdl|none)$ ]]; then AUDIO="$2"; shift; fi; shift ;;
        --forward)     FORWARD="$2"; shift 2 ;;
        --no-net)      NONET=1; shift ;;
        --share)       SHARE="$2"; shift 2 ;;
        --localtime)   LOCALTIME=1; shift ;;
        --extra)       EXTRA="$2"; shift 2 ;;
        --sha512)      SHA512="$2"; shift 2 ;;
        --sha256)      SHA256="$2"; shift 2 ;;
        --sig)         SIG="$2"; shift 2 ;;
        --gpg-key)     GPG_KEY="$2"; shift 2 ;;
        --gpg-server)  GPG_SERVER="$2"; shift 2 ;;
        --no-verify)   NOVERIFY=1; shift ;;
        --force)       FORCE=1; shift ;;
        --background)  BACKGROUND=1; shift ;;
        --vnc)         VNC=1; shift ;;
        --dry-run)     DRYRUN=1; shift ;;
        -h|--help)     usage; exit 0 ;;
        --*)           die "unknown option: $1 (see --help)" ;;
        *)             positional+=("$1"); shift ;;
    esac
done
[ ${#positional[@]} -ge 1 ] || { usage; exit 1; }
ISO="${positional[0]}"
[ ${#positional[@]} -ge 2 ] && SIZE="${positional[1]}"

[ -f "$ISO" ] || die "ISO not found: $ISO"
ISO="$(cd "$(dirname "$ISO")" && pwd)/$(basename "$ISO")"
ISO_DIR="$(dirname "$ISO")"; ISO_FILE="$(basename "$ISO")"; ISO_BASE="${ISO_FILE%.*}"

to_qemu_size() {  # "5", "5g", "5GB", "512M" -> "5G" / "512M"
    local t="$1" what="$2"
    if [[ "$t" =~ ^[[:space:]]*([0-9]+)[[:space:]]*([kKmMgGtT])?[bB]?[[:space:]]*$ ]]; then
        local u="${BASH_REMATCH[2]:-G}"; echo "${BASH_REMATCH[1]}${u^^}"
    else
        die "$what not understood: '$t' (use 5G, 10GB, 15gb, 512M ...)"
    fi
}
SIZE="$(to_qemu_size "$SIZE" "disk size")"
if [[ "$SIZE" =~ ^([0-9]+)G$ ]] && [ "${BASH_REMATCH[1]}" -lt 4 ]; then
    note "[?] $SIZE is small for a desktop Linux install (most want 15 GB or more with a desktop)."
fi
[ -n "$MEM" ] && MEM="$(to_qemu_size "$MEM" "memory")"
case "$MACHINE" in pc|q35) ;; *) die "--machine must be pc or q35" ;; esac
case "$VGA" in std|virtio|qxl|vmware|cirrus|none) ;; *) die "--vga must be std|virtio|qxl|vmware|cirrus|none" ;; esac
case "$BOOT_FORCE" in ""|cd|disk) ;; *) die "--boot must be cd or disk" ;; esac
[ -z "$SHARE" ] || [ -d "$SHARE" ] || die "--share folder not found: $SHARE"

SLUG="$(echo "$ISO_BASE" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9._-]+/-/g; s/^-+//; s/-+$//')"
[ -n "$NAME" ] || NAME="${SLUG}-${SIZE}"
NAME="$(echo "$NAME" | sed -E 's/[^A-Za-z0-9._-]+/-/g; s/^-+//; s/-+$//')"
[ -n "$DISK" ] || DISK="$DATA_DIR/adhoc/$NAME.qcow2"
mkdir -p "$(dirname "$DISK")"
VARS="${DISK%.qcow2}.vars.fd"

command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 not found (install qemu-system-x86)"
command -v qemu-img >/dev/null || die "qemu-img not found (install qemu-utils)"

echo
echo "  Castle VM -- boot ISO"
echo "  ================================================================"
note "* ISO:      $ISO"
note "* Disk:     $DISK ($SIZE)$([ "$SNAPSHOT" = 1 ] && echo '  [snapshot: writes discarded]')"
note "* Machine:  $MACHINE, $FIRMWARE, $DISK_IF disk, vga $VGA"

# ==============================================================================
# VERIFICATION (optional)
# ==============================================================================
if [ "$NOVERIFY" = 0 ]; then
    if [ -z "$SHA512" ]; then
        for c in "$ISO_FILE.sha512sum" "$ISO_FILE.sha512" SHA512SUMS sha512sums.txt sha512sum.txt; do
            [ -f "$ISO_DIR/$c" ] && { SHA512="$ISO_DIR/$c"; note "[i] Found next to ISO: $c"; break; }
        done
    fi
    if [ -z "$SHA256" ]; then
        for c in "$ISO_FILE.sha256sum" "$ISO_FILE.sha256" SHA256SUMS sha256sums.txt sha256sum.txt; do
            [ -f "$ISO_DIR/$c" ] && { SHA256="$ISO_DIR/$c"; note "[i] Found next to ISO: $c"; break; }
        done
    fi
    if [ -z "$SIG" ]; then
        cands=("$ISO_DIR/$ISO_FILE.sig" "$ISO_DIR/$ISO_FILE.asc")
        for sums in "$SHA512" "$SHA256"; do
            [ -n "$sums" ] && [ -f "$sums" ] && cands+=("$sums.gpg" "$sums.sign" "$sums.sig" "$sums.asc")
        done
        for c in "${cands[@]}"; do
            [ -f "$c" ] && { SIG="$c"; note "[i] Found next to ISO: $(basename "$c")"; break; }
        done
    fi
fi

TMP_FILES=()
cleanup() {
    # Must end with status 0: the EXIT trap's status would otherwise replace the script's.
    [ ${#TMP_FILES[@]} -gt 0 ] || return 0
    rm -f "${TMP_FILES[@]}"
}
trap cleanup EXIT

fetch_text() {  # hex digest | local file | URL -> text on stdout
    local src="$1"
    if [[ "$src" =~ ^[a-fA-F0-9]{32,128}$ ]]; then echo "$src"
    elif [ -f "$src" ]; then cat "$src"
    elif [[ "$src" =~ ^https?:// ]]; then note "   [>] Fetching $src" >&2; curl -fsSL "$src"
    else return 1; fi
}
fetch_file() {  # local file | URL -> local path on stdout
    local src="$1" tmp
    if [ -f "$src" ]; then echo "$src"
    elif [[ "$src" =~ ^https?:// ]]; then
        tmp="$(mktemp "/tmp/castle-$(basename "$src").XXXX")"; TMP_FILES+=("$tmp")
        note "   [>] Fetching $src" >&2; curl -fsSL -o "$tmp" "$src" && echo "$tmp"
    else return 1; fi
}
find_expected_hash() {  # text, hexlen, filename -> digest or empty
    local text="$1" len="$2" fname="$3" line hit
    hit="$(echo "$text" | grep -oiE "^[[:space:]]*[a-f0-9]{$len}[[:space:]]*$" | head -n1 | tr -d '[:space:]')"
    [ -n "$hit" ] && { echo "$hit"; return; }
    line="$(echo "$text" | grep -iF -- "$fname" | grep -oiE "[a-f0-9]{$len}" | head -n1)"
    [ -n "$line" ] && { echo "$line"; return; }
    # Single-entry checksum file naming the ISO differently (renamed download).
    local all; all="$(echo "$text" | grep -oiE "(^|[^a-f0-9])[a-f0-9]{$len}([^a-f0-9]|$)" | grep -oiE "[a-f0-9]{$len}" | sort -u)"
    [ "$(echo "$all" | grep -c .)" = 1 ] && echo "$all"
}
check_hash() {  # source, algo(512|256)
    local src="$1" bits="$2" len text expected computed
    len=$(( bits / 4 ))
    note "[HASH] SHA$bits from $src"
    text="$(fetch_text "$src")" || { note "[FAIL] not a hex digest, file or URL: $src"; return 1; }
    expected="$(find_expected_hash "$text" "$len" "$ISO_FILE" | tr '[:upper:]' '[:lower:]')"
    [ -n "$expected" ] || { note "[FAIL] No SHA$bits digest for '$ISO_FILE' in that source."; return 1; }
    computed="$("sha${bits}sum" "$ISO" | cut -d' ' -f1)"
    if [ "$computed" = "$expected" ]; then note "[OK] SHA$bits verified ($computed)"; return 0; fi
    note "[FAIL] SHA$bits MISMATCH"; note "   Computed: $computed"; note "   Expected: $expected"; return 1
}
check_sig() {  # source
    local src="$1" sigfile signed stripped sums out missing fpr
    command -v gpg >/dev/null || { note "[?] gpg not installed -- signature check skipped."; return 2; }
    sigfile="$(fetch_file "$src")" || { note "[FAIL] signature not found: $src"; return 1; }
    signed="$ISO"
    stripped="$(basename "$sigfile" | sed -E 's/\.(gpg|sig|sign|asc)(\.[A-Za-z0-9]{4})?$//')"
    for sums in "$SHA512" "$SHA256"; do
        [ -n "$sums" ] && [ -f "$sums" ] && [ "$(basename "$sums")" = "$stripped" ] && signed="$sums"
    done
    [ "$signed" != "$ISO" ] && note "[GPG] Signature covers the checksum file $(basename "$signed"); the ISO is tied to it by its digest."
    note "[GPG] Verifying $(basename "$sigfile") against $(basename "$signed")..."
    if [ -n "$GPG_KEY" ]; then
        note "   [>] Importing key $GPG_KEY from $GPG_SERVER"
        gpg --keyserver "$GPG_SERVER" --recv-keys "$GPG_KEY" >/dev/null 2>&1 || true
    fi
    out="$(gpg --status-fd 1 --verify "$sigfile" "$signed" 2>&1 || true)"
    missing="$(echo "$out" | sed -nE 's/^\[GNUPG:\] NO_PUBKEY ([A-F0-9]+).*/\1/p' | head -n1)"
    if [ -n "$missing" ] && [ -z "$GPG_KEY" ]; then
        note "   [>] Signing key $missing is not in your keyring; fetching it from $GPG_SERVER"
        gpg --keyserver "$GPG_SERVER" --recv-keys "$missing" >/dev/null 2>&1 || true
        out="$(gpg --status-fd 1 --verify "$sigfile" "$signed" 2>&1 || true)"
    fi
    if echo "$out" | grep -q '^\[GNUPG:\] GOODSIG'; then
        fpr="$(echo "$out" | sed -nE 's/^\[GNUPG:\] VALIDSIG ([A-F0-9]+).*/\1/p' | head -n1)"
        note "[OK] Good signature from $(echo "$out" | sed -nE 's/^\[GNUPG:\] GOODSIG [A-F0-9]+ (.*)/\1/p' | head -n1)"
        note "   Fingerprint: ${fpr:-?} -- compare it with the one the distro publishes."
        return 0
    fi
    note "[FAIL] GPG signature rejected."; echo "$out" | grep -v '^\[GNUPG:\]' | head -n6 | sed 's/^/     /'
    return 1
}

if [ -n "$SHA512$SHA256$SIG" ]; then
    echo; note "[*] Verification"; note "-------------------------------------------"
    failed=0; passed=0
    if [ -n "$SHA512" ]; then check_hash "$SHA512" 512 && passed=1 || failed=1; fi
    if [ -n "$SHA256" ]; then check_hash "$SHA256" 256 && passed=1 || failed=1; fi
    if [ -n "$SIG" ]; then
        set +e; check_sig "$SIG"; rc=$?; set -e
        case $rc in 0) passed=1 ;; 1) failed=1 ;; esac
    fi
    if [ "$failed" = 1 ]; then
        if [ "$FORCE" = 1 ]; then note "[?] Verification failed -- booting anyway (--force)."
        else die "Verification failed. Refusing to boot; pass --force to boot anyway or --no-verify to skip checks."; fi
    elif [ "$passed" = 0 ]; then note "[?] No verification could be performed."; fi
else
    note "[i] No checksum or signature given or found -- booting unverified (that is fine for your own ISOs)."
fi

# ==============================================================================
# DISK
# ==============================================================================
echo
if [ "$FRESH" = 1 ] && [ -f "$DISK" ]; then
    note "[DISK] --fresh: deleting $DISK"; rm -f "$DISK" "$VARS"
fi
FIRST_BOOT=0
if [ -f "$DISK" ]; then
    # A qcow2 that was created but never installed to holds only metadata (~200 KB);
    # booting it lands on "no bootable device", so treat it as a first boot.
    if [ "$(stat -c %s "$DISK")" -lt 4194304 ]; then
        note "[DISK] Virtual disk exists but is blank -- booting the installer from CD-ROM."; FIRST_BOOT=1
    else
        have=$(qemu-img info --output=json "$DISK" | sed -nE 's/.*"virtual-size": *([0-9]+).*/\1/p' | head -n1)
        want=$(numfmt --from=iec "$SIZE" 2>/dev/null || echo 0)
        if [ -n "$have" ] && [ "$want" != 0 ] && [ "$have" != "$want" ]; then
            note "[?] Disk already exists at $((have / 1073741824)) GB; the requested $SIZE is ignored. Use --fresh or --name for a new disk."
        fi
        note "[DISK] Existing disk -- booting from disk (ISO attached as CD)."
    fi
else
    note "[DISK] Creating $SIZE virtual disk: $DISK"
    qemu-img create -f qcow2 "$DISK" "$SIZE" >/dev/null; FIRST_BOOT=1
fi
[ "$BOOT_FORCE" = "cd" ] && FIRST_BOOT=1
[ "$BOOT_FORCE" = "disk" ] && FIRST_BOOT=0

# ==============================================================================
# HARDWARE (same policy as modules/hardware.ps1)
# ==============================================================================
HOST_CORES=$(nproc 2>/dev/null || echo 4)
[ -n "$CORES" ] || CORES=$(( HOST_CORES - 2 < 2 ? 2 : HOST_CORES - 2 ))
if [ -z "$MEM" ]; then
    total_kb=$(awk '/MemTotal/ {print $2}' /proc/meminfo 2>/dev/null || echo 16777216)
    total_gb=$(( (total_kb + 524288) / 1048576 ))
    max_gb="${CASTLE_VM_MAX_RAM_GB:-8}"
    mem_gb=$(( total_gb / 2 )); [ "$mem_gb" -lt 4 ] && mem_gb=4; [ "$mem_gb" -gt "$max_gb" ] && mem_gb="$max_gb"
    MEM="${mem_gb}G"
fi
if [ -z "$ACCEL" ]; then
    if [ -w /dev/kvm ]; then ACCEL="kvm"; else ACCEL="tcg"; note "[?] /dev/kvm not usable -- software emulation (slow)."; fi
fi
[ -n "$CPU" ] || { [ "$ACCEL" = "kvm" ] && CPU="host" || CPU="max"; }

# ==============================================================================
# QEMU command line
# ==============================================================================
[ "$FIRST_BOOT" = 1 ] && BOOT="dc" || BOOT="cd"
[ "$FIRST_BOOT" = 1 ] && CACHE="unsafe" || CACHE="writeback"   # installer writes are throwaway until it finishes

args=(-accel "$ACCEL")
[ "$ACCEL" != tcg ] && args+=(-accel tcg)
[ "$MACHINE" = q35 ] && args+=(-machine q35)
args+=(
    -cpu "$CPU" -smp "$CORES" -m "$MEM"
    -drive "file=$DISK,if=$DISK_IF,format=qcow2,cache=$CACHE,discard=unmap"
    -drive "file=$ISO,media=cdrom,readonly=on"
    -boot "order=$BOOT"
    -vga "$VGA"
    -usb -device usb-tablet
)
[ "$SNAPSHOT" = 1 ] && args+=(-snapshot)
[ "$LOCALTIME" = 1 ] && args+=(-rtc base=localtime)

# Network: be explicit so forwards and --no-net work.
if [ "$NONET" = 1 ]; then
    args+=(-nic none)
else
    fwd=""
    if [ -n "$FORWARD" ]; then
        IFS=', ' read -r -a pairs <<< "$FORWARD"
        for p in "${pairs[@]}"; do
            [ -z "$p" ] && continue
            [[ "$p" =~ ^([0-9]+):([0-9]+)$ ]] || die "--forward entry not understood: '$p' (use host:guest, e.g. 2222:22)"
            fwd+=",hostfwd=tcp:127.0.0.1:${BASH_REMATCH[1]}-:${BASH_REMATCH[2]}"
        done
    fi
    args+=(-nic "user,model=$NIC_MODEL$fwd")
fi

if [ -n "$AUDIO" ]; then
    args+=(-audiodev "$AUDIO,id=snd0" -device intel-hda -device hda-duplex,audiodev=snd0)
fi

if [ "$FIRMWARE" = "uefi" ]; then
    code=""; vars=""
    for pair in \
        "/usr/share/OVMF/OVMF_CODE_4M.fd:/usr/share/OVMF/OVMF_VARS_4M.fd" \
        "/usr/share/OVMF/OVMF_CODE.fd:/usr/share/OVMF/OVMF_VARS.fd" \
        "/usr/share/edk2/x86_64/OVMF_CODE.fd:/usr/share/edk2/x86_64/OVMF_VARS.fd" \
        "/usr/share/edk2/ovmf/OVMF_CODE.fd:/usr/share/edk2/ovmf/OVMF_VARS.fd" \
        "/usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd:/usr/share/edk2-ovmf/x64/OVMF_VARS.4m.fd" \
        "/usr/share/qemu/OVMF.fd:"; do
        c="${pair%%:*}"; v="${pair#*:}"
        [ -f "$c" ] && { code="$c"; vars="$v"; break; }
    done
    if [ -n "$code" ]; then
        # pflash: unit 0 read-only code, unit 1 the VM's own writable NVRAM.
        args+=(-drive "if=pflash,format=raw,readonly=on,file=$code")
        if [ -n "$vars" ] && [ -f "$vars" ]; then
            [ -f "$VARS" ] || cp "$vars" "$VARS"
            args+=(-drive "if=pflash,format=raw,file=$VARS")
            note "* NVRAM:    $VARS"
        fi
        note "* Firmware: UEFI ($code)"
    else
        note "[?] OVMF not found -- falling back to BIOS (install the 'ovmf' / 'edk2-ovmf' package)."
    fi
fi

# The pipeline's provisioning scripts, as a read-only USB stick labelled CASTLE.
if [ -d "$SCRIPTS_DIR" ]; then
    args+=(-blockdev "driver=vvfat,node-name=castlescripts,dir=$SCRIPTS_DIR,label=CASTLE,rw=off,read-only=on"
           -device usb-storage,drive=castlescripts)
fi
if [ -n "$SHARE" ]; then
    SHARE="$(cd "$SHARE" && pwd)"
    args+=(-blockdev "driver=vvfat,node-name=share0,dir=$SHARE,label=SHARE,rw=off,read-only=on"
           -device usb-storage,drive=share0)
    note "* Share:    $SHARE (read-only USB, label SHARE)"
fi

if [ "$VNC" = 1 ]; then
    args+=(-vnc 127.0.0.1:0); note "* VNC:      127.0.0.1:5900 (display :0)"
elif [ -n "$DISPLAY_MODE" ]; then
    args+=(-display "$DISPLAY_MODE")
fi
[ -n "$FORWARD" ] && note "* Forwards: $FORWARD (host 127.0.0.1 -> guest)"

if [ -n "$EXTRA" ]; then
    # shellcheck disable=SC2206
    extra_arr=($EXTRA); args+=("${extra_arr[@]}")
fi

echo
note "[BOOT] $([ "$FIRST_BOOT" = 1 ] && echo 'CD-ROM (install)' || echo 'Disk (existing)')  --  $CORES cores, $MEM RAM, cpu $CPU, $ACCEL"
if [ "$DRYRUN" = 1 ]; then
    note "[DRY RUN] Would launch:"
    printf '  qemu-system-x86_64'; printf ' %q' "${args[@]}"; echo
    exit 0
fi

if [ "$BACKGROUND" = 1 ]; then
    mkdir -p "$DATA_DIR/adhoc"
    nohup qemu-system-x86_64 "${args[@]}" >"$DATA_DIR/adhoc/$NAME.log" 2>&1 &
    note "[BOOT] Background VM started (PID: $!), log: $DATA_DIR/adhoc/$NAME.log"
else
    exec qemu-system-x86_64 "${args[@]}"
fi
