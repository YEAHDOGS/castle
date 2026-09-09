# Castle Arcade — Retro Console Asset Pipeline

Arcade is the retro-gaming arm of Castle: a modular, zero-dependency PowerShell pipeline that **acquires, cryptographically verifies, extracts, and organizes** retro console assets (system firmware/BIOS files and ROM images) into a tidy local library.

```powershell
.\dl\consoles.ps1            # interactive menu
.\dl\consoles.ps1 gba-bios   # direct download + verify
.\dl\consoles.ps1 list       # registry with cached/missing status
.\dl\consoles.ps1 ps1-bios-5501 -Purge
```

## Directory Structure

```text
arcade/
├── README.md            # this file
├── dl/
│   └── consoles.ps1     # entry point: interactive menu + direct CLI
├── modules/
│   ├── targets.ps1      # asset registry ($ArcadeMatrix: URL, hash, layout)
│   ├── network.ps1      # chunked HTTP streaming downloader w/ progress
│   ├── security.ps1     # hashing (MD5/SHA1/SHA256/SHA512), HMAC, AES-256 helpers
│   └── archive.ps1      # zip extract / directory compress helpers
└── data/                # local library (git-ignored, created on demand)
    ├── bios/            # system firmware, e.g. scph5501.bin, gba_bios.bin
    └── roms/
        └── <platform>/  # e.g. roms/nes/Alter_Ego.nes
```

## Pipeline

For each registry entry (`dl/consoles.ps1 → Run-DownloadPipeline`):

1. **Acquire** — stream the file over HTTPS in 128 KB chunks (resumable-style progress logging, partial file deleted on failure).
2. **Verify** — hash the downloaded bytes and compare against the pinned `ExpectedHash` in the registry (`security.ps1 → Test-FileHash`). Mismatches are deleted, never kept.
3. **Extract (optional)** — if the entry sets `Extract = $true`, unzip and promote the configured `ExtractFile` into the library folder, then clean up the temp dir.

## Registry Format

Each entry in `modules/targets.ps1` (`$ArcadeMatrix`) declares:

| Key | Meaning |
|---|---|
| `Id` / `Name` / `Description` | identity |
| `Url` | download source |
| `File` | destination filename |
| `HashAlgorithm` / `ExpectedHash` | pinned integrity hash |
| `Type` | `bios` → `data/bios/`, `rom` → `data/roms/<Platform>/` |
| `Platform` | library subfolder for ROMs |
| `Extract` / `ExtractFile` | optional unzip + file promotion |

## Companion: saves/

Game *save data* versioning lives in [`../saves/`](../saves/) ("git for save data"): snapshot, diff, and restore emulator save files, with cloud sync designed to ride on Castle's storage story. Arcade handles the *assets*; saves/ handles your *progress*.
