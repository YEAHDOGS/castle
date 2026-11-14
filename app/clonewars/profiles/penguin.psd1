@{
    Description = "Cart Surfer bot (penguin-autosurf) on Club Penguin Journey: Firefox + the bot, playing on a loop"

    # The light guest: Debian + Xorg + Openbox. "endeavouros" works too
    # (.\start.ps1 -Profile penguin -Target endeavouros).
    Target      = "debian-x11"
    Background  = $true
    MemoryGB    = 4
    Cores       = 8
    Screen      = "1440x900"

    # The project sits next to the castle checkout. Packed as git sees it (no
    # virtualenvs, no recordings), plus the git-ignored .env the bot signs in with.
    Payload     = @(
        @{ Source = "..\..\..\..\penguin-autosurf"; As = "penguin-autosurf.zip"; Include = @(".env") }
    )

    # Unpacks the project, builds its Python environment, opens Firefox on the
    # game and starts the bot (scripts/setup-penguin.sh).
    Setup       = "penguin"
}
