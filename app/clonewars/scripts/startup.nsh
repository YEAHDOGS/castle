@echo -off
# ==============================================================================
# Castle / clonewars -- UEFI shell fallback boot script
# ==============================================================================
# clonewars/scripts/ is attached to every VM as a read-only USB FAT drive, so
# OVMF's built-in shell finds this file (its default search path includes the
# root of every mapped filesystem) whenever no NVRAM boot entry succeeded.
#
# Why it exists: OVMF's automatic "hard disk" boot option only tries
# \EFI\BOOT\BOOTX64.EFI. Windows' bcdboot installs \EFI\Microsoft\Boot\
# bootmgfw.efi and registers an NVRAM entry instead, so a Windows disk whose
# NVRAM entry is missing (fresh vars file, linked clone, entry lost on a
# QEMU exit at reboot) drops into this shell. This script walks the mapped
# filesystems and launches the first OS loader it finds.
# ==============================================================================
for %m in 0 1 2 3 4 5 6 7 8 9
    if exist fs%m:\EFI\Microsoft\Boot\bootmgfw.efi then
        echo castle: booting Windows from fs%m:
        fs%m:\EFI\Microsoft\Boot\bootmgfw.efi
    endif
endfor
for %m in 0 1 2 3 4 5 6 7 8 9
    if exist fs%m:\EFI\ubuntu\shimx64.efi then
        echo castle: booting Ubuntu from fs%m:
        fs%m:\EFI\ubuntu\shimx64.efi
    endif
    if exist fs%m:\EFI\BOOT\BOOTX64.EFI then
        echo castle: booting fallback loader from fs%m:
        fs%m:\EFI\BOOT\BOOTX64.EFI
    endif
endfor
echo castle: no OS loader found on any mapped filesystem.
