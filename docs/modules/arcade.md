# Arcade — the best seat in the house

Brando's directive (2026-09-09): Castle Arcade exists for one reason — a
great retro game experience, on every device he owns. Not a ROM folder. An
arcade.

## The reference experience (why this doc exists)

The Pokémon Gen 1 Recompilation Project
(`bryanthaboi/pokemon-gen1-recomp-project`) plus the Dramatic Shape Voxel
Mod (`dramaticshape/dramaticshapevoxelmod`) is the bar:

- **Native, not emulated.** The recomp is a real PC/Android program built
  from a matching ROM — 60fps, widescreen, no emulator overhead.
- **The voxel mod turns Kanto into a 3D diorama:** extruded terrain, real
  cast shadows, depth-buffered occlusion, tilt-shift miniature look,
  over-the-shoulder 3D battles, first/third-person cameras, VR experiments.
- **A living mod ecosystem:** voxel Pokédex with real Stadium 3D models,
  flying overhaul, weather/day-night forks, battle UI revamps — installed
  through a built-in mod manager.

This is the class of experience Arcade is designed around. If Arcade can't
make *this* feel incredible on every screen, it's not done.

## Two engines, one library

1. **Emulation lane** — libretro/RetroArch cores for everything up to the
   6th generation. Per-game shader presets: CRT-royale for the purists,
   integer scaling, zero apologies for scanlines.
2. **Native lane** — recompilation ports and their mod managers. These are
   real executables with real GPU appetites; they don't live in an emulator
   and Arcade doesn't treat them like ROMs. The library shows one game —
   "Pokémon Red" — and Arcade picks the best available engine for the box
   it's running on.

## The box problem (stated honestly)

- **Pi-class:** emulation through PS1/N64-era, 1080p shaders, flawless.
  It will NOT run voxel Kanto at 4K. Physics doesn't negotiate.
- **Console-class (x86 + real GPU):** the voxel mods, 4K shader stacks,
  upscaled PS2/GameCube/Wii. This is Arcade's flagship tier.
- The library, saves, and shaders are identical on both. Bigger muscle just
  means prettier. Upgrading the box never means rebuilding the library —
  same rule as the rest of Castle.

## Every device: stream it

One powerhouse, every screen. Castle runs **Sunshine**; phones, laptops,
TVs, and even Kennel-docked phones run **Moonlight** clients. Sub-20ms on
the LAN — it feels local because the network is local. This is how a phone
plays voxel Kanto at full quality: the GPU box does the work, the phone is
a window with buttons. Browser fallback where a native client doesn't exist.

## Presentation

- Per-game, per-device presets: the TV gets the CRT shader, the phone gets
  the clean upscale, the voxel mods get fed GPU and left alone — Arcade
  doesn't fight a mod's own lighting pipeline.
- Integer scaling by default. No blurry bilinear smears on pixel art.
- The mod's own options (camera pitch, tilt-shift, world curvature) surface
  in the Arcade UI, not buried in a config file.

## Saves: Chains

Every save is versioned in **Chains** ("git for save data"): snapshot, diff,
restore, sync. Start Pokémon on the TV, continue on the phone on the bus,
roll back the save you corrupted at 2am. The save follows the player, not
the device.

## Controllers

Bluetooth and USB gamepads, phone-as-controller through Moonlight, per-game
button maps. Picking up a controller and pressing start is the whole UX —
no launcher maze between you and the game.

## The content rule

Arcade never ships copyrighted ROMs or BIOS files. You dump what you own;
Arcade verifies (pinned hashes — the pipeline in `arcade/` already does
this), organizes, and plays it. Recompilation projects need your own dump
to build from — same rule, no exceptions. Clean library, clean conscience.

## Roadmap

1. Library + emulation lane on Pi-class hardware — prove the experience loop.
2. Streaming: Sunshine on Castle, Moonlight on every screen Brando owns.
3. Console-class GPU box: native ports lane, voxel mods, 4K presentation.
4. Netplay, per-game shader presets, live Chains save sync across devices.
