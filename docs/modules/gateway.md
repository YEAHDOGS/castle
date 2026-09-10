# Gateway — Castle replaces your router

**Gateway.** The Castle vision from day one: reinvent the modem/router.
Brando's founding example (2026-09-09): two people on the LAN download
Rainbow Six Siege — 58 GB, the identical file — and the network fetches it
twice like an idiot. Castle sits at the edge of the network and makes
redundant downloads not happen.

One honest boundary up front: the *modem* half (DOCSIS certs, fiber ONT
authentication) is ISP-bound — you can't fully replace what the ISP
cryptographically requires. Castle replaces **everything behind the modem**:
router, DHCP, DNS, firewall, ad blocking, and the smart cache. That's 95% of
what a "modem router" is.

## The smart cache (the Siege problem)

This problem is solved in the wild — **lancache** exists and does exactly
this for Steam/Epic/Riot/Windows Update on LANs. Castle's version is native
to the box, not a sidecar container.

How it actually works:

- **Per-service caches** for known-cacheable endpoints (game CDNs, OS
  updates). These are plain HTTP range requests behind the scenes — very
  cacheable. First download fills the cache; the second machine on the LAN
  pulls at LAN speed.
- **Peer-to-peer LAN transfer** where platforms support it (Steam's built-in
  LAN transfer, Windows Delivery Optimization). Castle coordinates and
  prioritizes local peers before hitting the WAN.
- **Explicit proxy** (WPAD/proxy config on managed devices) as the opt-in
  power mode.

**The HTTPS honesty section:** you cannot transparently cache TLS traffic
without breaking it — that's the whole point of TLS. Castle never
man-in-the-middles your traffic silently. The cache works on the traffic
classes that are legitimately cacheable (game/OS CDNs, LAN P2P) and stays
out of everything else. Any HTTPS interception would require an explicit
opt-in with a Castle CA installed on the device, and it's off by default.

## Brando's three questions, answered

- **How big can the cache be?** Quota per content class (games / OS updates /
  general), carved from the storage pool — with 20 TB on the box, the cache
  gets a generous slice but it's the *lowest-priority* tenant: LRU eviction,
  and it shrinks first when real data needs the room. Cache is a guest, not
  a resident.
- **Abuse?** Authenticated proxy, per-device bandwidth quotas, and the cache
  only stores allowlisted content classes. A device can't use Castle's cache
  as a free CDN for arbitrary internet content.
- **Stale versions?** Cache keys are content hashes, not filenames — Siege
  v2.1 and v2.2 are different keys, so "stale" mostly can't happen. The
  mutable edges (update manifests) get short TTLs with revalidation.

Cache poisoning defense: cached objects are hash-verified against the
origin's manifest before they're served. A bad first download doesn't become
everyone's bad download.

## Ad blocking — Pi-hole, brought home

Brando had Pi-hole running on the prototype and called it beautiful. The
Gateway bakes it in as a native Castle service instead of a sidecar:

- DNS sinkholing with community blocklists, regex rules, per-client
  allow/deny — the full Pi-hole feature set.
- Per-device and per-user stats in the Castle App (what got blocked, where
  the trackers were phoning home).
- One-tap pause ("disable blocking for 5 minutes") for the sites that break.

Same soul as Pi-hole, zero separate maintenance, configured from the same
app as everything else.

## Router duties (the unglamorous checklist)

DHCP, DNS, stateful firewall, VLANs + isolated guest network, QoS/traffic
shaping (game traffic first, bulk downloads last), WireGuard server for
remote access *and* WireGuard client for outbound VPN, per-device bandwidth
accounting. The Kennel's isolated phone segment and the Drive Librarian's
untrusted-media stance both ride on these VLANs.

## What the dashboard shows

Live WAN vs. LAN-served byte counts ("today Castle saved you 116 GB of
downloads"), per-device usage, cache hit rate, blocked ad/tracker stats.
The money stat is the point: redundant data stops flying, and you can see
it stop.
