# Kennel — docked Android phones as Castle nodes

**Kennel.** Name confirmed by Brando (2026-09-09). Old Android phones go in
the Kennel instead of a drawer. Castle charges them over USB and each phone becomes a 24/7 node
you can reach from anywhere — SSH/VNC into it, run apps on it, automate it —
like a tiny VM farm made of hardware you already own.

Brando's use case (2026-09-09): keep Legends of Idleon running 24/7 on a
docked phone, managed through Castle from anywhere in the world.

## How it works

- **Connection:** ADB over USB. udev rules pin each phone to a stable device
  name by serial number, so phone #3 is always phone #3. ADB over TCP as
  fallback for phones on the LAN.
- **Screen + control:** scrcpy — full screen mirror and input control in a
  browser tab or window. Lower latency and sharper than VNC, purpose-built
  for exactly this.
- **Shell:** `adb shell`, or Termux with an SSH server for a real persistent
  shell. `adb forward` / reverse for tunneling services off the phone.
- **Remote access:** WireGuard into Castle, then scrcpy/adb to any kenneled
  phone. From Brandon's laptop or phone, anywhere on Earth, it feels like
  the phone is in his hand.
- **Orchestration:** a small Castle service tracks docked phones (serial,
  model, battery, state), keeps ADB alive across reboots, and exposes them in
  the Castle dashboard: tap a phone, get its screen.

## The battery problem (this is the one that bites)

A phone held at 100% charge 24/7 will swell its battery — that's chemistry,
not opinion. Mitigations, in order of preference:
1. **Charge limiting** (AccA / kernel charge control where supported): hold
   at 60–70%, top up only when needed.
2. **Battery bypass mode** on phones/kernels that support it: run straight
   off USB power, battery idle.
3. **Thermal:** keep docked phones cool and ventilated. Heat + full charge
   is what kills them fast.
4. Worst case: treat phone batteries as consumables and cycle the Kennel —
   but 1–3 should make that unnecessary.

## What it's good for

- **Idle games 24/7** (Idleon and friends) — the founding use case.
- **Self-hosted SMS gateway** — the Kennel sends texts through your own SIM.
  This is the Twilio-bill killer and it lives entirely on hardware you own.
- **2FA / authenticator vault** — codes live on a phone in your house, not
  in someone's cloud.
- **Automation hub** — Tasker/MacroDroid running around the clock: location
  triggers, smart-home glue, scheduled jobs.
- **App testing farm** — every screen size you own, one dashboard.

## Security model

Phones are **untrusted peripherals**, not part of Castle's trusted core:
- ADB keys pinned per phone; a new/unknown phone never auto-trusts.
- Kennel phones sit on an isolated network segment — they can reach the
  internet and the services they need, they cannot reach the vault.
- A compromised phone costs you the phone, never the Castle.

## Clone Wars integration (Brando 2026-09-09)

Clone Wars — the Castle Clone Factory (`clonewars/`) — is the Kennel realm's
provisioning engine. The Kennel keeps the phones; Clone Wars keeps the
golden images and the machines around them:

- **Golden phone images:** a known-good image per phone role (Idleon farmer,
  SMS gateway, automation hub). A phone dies at 3am? Flash a spare from the
  golden image, re-dock it, back in business in minutes.
- **VM sidecars:** the services that pair with kenneled phones (dashboard,
  SMS API, automation brains) get provisioned through the same Clone Wars
  pipeline — one system images everything in the realm.
- **Trust pinning carries over:** Clone Wars' cryptographic verification
  applies to phone images too. A golden image is checksummed and pinned;
  a flashed phone that doesn't match the pin doesn't join the Kennel.

One realm, one provisioning story: Clone Wars builds it, Kennel runs it.

## Roadmap

1. Single phone over USB: persistent ADB, scrcpy in browser, charge limit.
2. Multi-phone: udev pinning, dashboard with per-phone screens.
3. SMS gateway service + API.
4. Phase-2 chassis: dedicated Kennel bay(s) with cooling and per-port power
   control.
