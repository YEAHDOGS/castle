# Castle hardware roadmap — the router/modem killer

Brando's vision (2026-09-09): Castle replaces the router/modem. Not a box
that plugs into the ISP's box — the box itself. With room to grow: storage
bays, port modules, accessories we can sell (switches).

## The honest phase plan

**Phase 0 — now: Raspberry Pi prototype.** One GbE port, USB storage. It
proves the software: routing, DNS filtering, VPN, the vault, the modules.
A Pi cannot do 10 Ethernet ports and we don't pretend it can.

**Phase 1 — commodity x86 appliance.** N100-class mini PC with 4x 2.5GbE
ports — these exist today, they're cheap, they run real router duty with
headroom. This is the first thing we'd actually sell as "the Castle box."

**Phase 2 — custom/ODM appliance.** Designed for the vision:
- 8–10 Ethernet ports (2.5GbE, a couple 10GbE SFP+ for uplink/NAS).
- Modular storage bays: 2.5"/3.5" SATA + M.2 NVMe. SSD tier for hot data,
  HDD tier for bulk — best $/GB wins, cache sorts it out.
- Expansion slot for modules (see below) — buy the base box, add what you need.

## Ports and modules

- **Ethernet:** 10 ports on one box is really a router + a switch glued
  together. Architecture it that way: the Castle is the router, and the
  **DOGS Switch** is the accessory we sell alongside it — managed, silent,
  pairs with Castle for one-dashboard control. That's a product line, not a
  compromise.
- **Telephone jacks (FXS):** real but niche — a VoIP ATA module for analog
  phones/fax over the Castle's connection. Expansion module, not core. The
  market is small and shrinking; we serve it without betting the box on it.
- **Storage bays:** hot-swap, tool-less. Mix SSDs and HDDs however you want;
  the software tiers them. Bays are also the redundancy story — this is where
  the mirrors from REDUNDANCY.md physically live.
- **Phone dock (Kennel):** USB bay(s) for docked Android phones — see
  `docs/modules/kennel.md`. Charging + ADB + remote control, built into the
  chassis on Phase 2, USB-attached on Phase 0/1.

## Design principles (non-negotiable)

1. **Silent and low-power.** It lives in a house, not a rack. No 40mm screamers.
2. **No cloud dependency.** Every feature works with the WAN cable unplugged.
   Phone-home is a bug, not a feature.
3. **Every module optional.** Base box is complete; modules add, never gate.
4. **Standard parts.** No custom connectors, no proprietary drives. If we die,
   the customer's hardware still works.
5. **Software outlives hardware.** Phase 0's Pi and Phase 2's appliance run
   the same Castle OS — upgrading the box never means rebuilding your life.
