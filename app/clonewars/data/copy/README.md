# data/copy — payload for the VM

Drop files here (Windows installers, portable `.exe`s, anything). With `.\start.ps1 endeavouros -Copy` this folder rides into the VM as a read-only USB share (FAT label `CASTLECOPY`), `scripts/setup-wine.sh` copies it to `~/castle-copy` inside the guest, installs Wine, and runs every `.exe` it finds (or just the one named with `-Program`). Interactive `.\start.ps1` asks the same whenever this folder is not empty.

Limit: about 500 MB total (QEMU's virtual FAT drive). Contents are git-ignored.
