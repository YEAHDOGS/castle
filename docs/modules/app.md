# Castle App — one control plane, every screen

**Castle App.** Brando's big picture (2026-09-09): a mobile (iOS + Android)
and web app that configures the Castle, connects to it from anywhere, and
runs *everything* — browse files, create users, change passwords, security
settings, Gateway cache stats, Kennel phones, Drive Librarian shelf, Arcade.
One app, total control, no separate admin tools per module.

This doc also answers the codebase question: **how the apps, the web, and
the server modules share code without triplicating everything.**

## The sharing strategy (Brando's rule: all code is a liability)

The repo is already Kotlin Multiplatform (`app/` + `server/` on Ktor) — the
App continues that bet instead of introducing a second stack:

- **`:shared` core module** — the fat center. Data models, API client,
  validation rules, sync/offline logic, auth flows. Written once, in Kotlin,
  compiled to every target (JVM, Android, iOS via Kotlin/Native, JS).
- **Thin platform shells** — Android (Compose), iOS (SwiftUI or Compose
  Multiplatform against the same shared core), web. Shells render UI and
  handle platform input; they contain zero business logic worth testing.
- **Server modules speak one API** — Gateway, Kennel, Drive Librarian,
  Arcade all expose their config and state through the Ktor API using the
  *same shared models*. The app never hand-rolls a second interpretation of
  what a "drive" or "phone" is.
- **One test suite for shared logic.** A validation rule tested in
  `:shared` is tested on all platforms. Platform shells get UI tests only
  where the platform does something genuinely platform-specific.

New module checklist: define the models in `:shared`, expose them over the
API, and every client (iOS, Android, web) gets the feature with only UI
work. No feature is ever implemented three times.

## Remote access: Tailscale-first (Brando 2026-09-09)

The way you reach your Castle from anywhere is **Tailscale** — which is
WireGuard under the hood (same Noise-protocol crypto, ChaCha20-Poly1305),
plus a control plane that handles key exchange, NAT traversal, and
identity. No port forwarding, no dynamic-DNS prayers, revoked-lost-phone in
one click.

**Backup path: plain WireGuard.** If Tailscale's control plane is ever
unreachable, a manually-configured WireGuard peer on Castle is the fallback.
The crypto is identical — "less secure" is the wrong framing. The real
differences:

- *Key management:* Tailscale wins. Identity-based ACLs, easy revocation,
  key expiry. Most real VPN failures are key hygiene, not crypto — and
  that's where Tailscale earns its keep.
- *Third-party trust:* plain WireGuard wins. No company's control plane
  knows your network topology. Tailscale can't decrypt your traffic (private
  keys never leave your devices), but it does know which nodes exist.

The backup peer is locked down: separate keys from everything else, firewall
rule that only allows it when the primary path is down, and it's documented
as break-glass — not a daily driver. Two entry points is slightly more
attack surface; the redundancy is worth it, and the lockdown keeps it
honest.

**Endgame: Headscale.** Self-hosted Tailscale control plane, running on the
Castle itself. Same Tailscale clients and UX, zero Tailscale-the-company in
the loop. That's the Castle-native answer once the box is stable — Tailscale
today, Headscale when we're ready to own the whole stack.

## What the app does

- **Connect:** local discovery on the LAN; Tailscale when remote (plain
  WireGuard as break-glass backup).
  No cloud account required, no login wall to reach your own box — the
  no-login philosophy extends to Castle itself for local access.
- **Files:** browse the vault, upload/download, share links, see Drive
  Librarian snapshots and the drive shelf.
- **People:** create users, change passwords, set roles (admin / family /
  guest), per-user storage quotas.
- **Security:** firewall and VLAN status, WireGuard peers, login/audit log,
  update Castle itself.
- **Modules:** every module's dashboard lives here — Gateway cache stats and
  ad-block controls, Kennel phone screens, Arcade library.

## API-first discipline

The web dashboard and the mobile apps are all just API clients. If a feature
can't be done through the API, the API is incomplete — fix the API, don't
build a side door. This is what keeps the "succinct" promise real: the
server is the source of truth, the apps are views.

## Out of scope

- The app is not a file-sync client (that's a separate sync engine if
  Brando wants Dropbox-style sync later).
- No push-notification cloud service dependency — alerts go through the
  tunnel, not through someone else's server farm.
