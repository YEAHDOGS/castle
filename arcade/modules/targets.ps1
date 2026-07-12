# ==============================================================================
# Retro Console Downloader - Target Registry Manifest
# ==============================================================================

$ArcadeMatrix = @(
    @{
        Id            = "ps1-bios-5501"
        Name          = "PlayStation 1 BIOS (SCPH-5501)"
        Description   = "Required system firmware for North American region PS1 emulation."
        Url           = "https://archive.org/download/bios_batocera/scph5501.bin"
        File          = "scph5501.bin"
        HashAlgorithm = "MD5"
        ExpectedHash  = "490f666e1afb15b7362b406ed1cea246"
        Type          = "bios"
        Platform      = "playstation"
    },
    @{
        Id            = "ps1-bios-1001"
        Name          = "PlayStation 1 BIOS (SCPH-1001)"
        Description   = "Required system firmware for early North American region PS1 emulation."
        Url           = "https://archive.org/download/bios_batocera/scph1001.bin"
        File          = "scph1001.bin"
        HashAlgorithm = "MD5"
        ExpectedHash  = "dc2b9bf8da62ec93e868cfd29f0d067d"
        Type          = "bios"
        Platform      = "playstation"
    },
    @{
        Id            = "gba-bios"
        Name          = "Game Boy Advance BIOS"
        Description   = "Required system firmware for GBA emulation."
        Url           = "https://archive.org/download/gba_bios/gba_bios.bin"
        File          = "gba_bios.bin"
        HashAlgorithm = "MD5"
        ExpectedHash  = "a860e8c0b6d573d191e4ec7db1b1e4f6"
        Type          = "bios"
        Platform      = "gba"
    },
    @{
        Id            = "nes-alterego"
        Name          = "Alter Ego (NES Homebrew)"
        Description   = "A popular logic-based puzzle platformer NES homebrew game."
        Url           = "https://archive.org/download/pouet_71667/Alter_Ego.nes"
        File          = "Alter_Ego.nes"
        HashAlgorithm = "MD5"
        ExpectedHash  = "73d0e0b0147a1f2371c10b26db3292e1"
        Type          = "rom"
        Platform      = "nes"
    }
)
