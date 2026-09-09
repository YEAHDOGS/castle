# Castle redundancy — the "can't get fucked" architecture

Brando's directive (2026-09-09): encryption at rest is great in theory, but in
practice it gets people fucked — lost keys, corrupted headers, one dead disk.
Castle's redundancy has to survive disk death, key loss, power loss,
fire/theft, AND Brando's own mistakes, without ever losing data or locking
him out. Losing any ONE thing — a password, a USB key, a disk, a building —
never loses data. That's the bar.

## The layers (each one covers a different way to die)

### 1. The filesystem: ZFS
Copy-on-write, checksums on every block, scrubs that detect bitrot before it
spreads, snapshots that roll back ransomware or a bad `rm -rf` in seconds.
Power loss mid-write is a non-event — ZFS never overwrites live data.
This is the foundation everything else sits on. btrfs is the fallback if ZFS
licensing ever becomes a problem; the architecture doesn't depend on either
by name, it depends on: checksums, snapshots, copy-on-write.

### 2. Disk redundancy: mirrors, not wishes
Honest note: partitions on the same disk protect against software accidents
and filesystem corruption. They do NOT protect against the disk dying. Real
redundancy is multiple physical disks.
- **Good tier:** 2-disk mirror. One disk dies, you keep working, you replace
  the disk, it resilver.
- **Fortress tier:** 3-way mirror or RAIDZ2. Survives ANY two disks dying at
  once — including the classic "second disk dies during the rebuild" nightmare.
- Hot-spare bay if the chassis has room: a dead disk starts rebuilding
  itself before you even notice.

### 3. Encryption that can't lock him out
LUKS2 with multiple key slots (it supports 8 — use them):
- Slot 1: passphrase he knows.
- Slot 2: hardware key (YubiKey-style) on his keychain.
- Slot 3: paper backup, sealed, stored somewhere that isn't his house.
- LUKS header backed up to three separate places. A corrupted header with no
  backup is unrecoverable — this is the #1 way encryption fucks people, and
  it's a solved problem: `luksHeaderBackup`, stored redundantly.
- Optional: Shamir's Secret Sharing — split the key into 5 shards, any 3
  recover it. Give shards to people/places he'd have to lose simultaneously.

The rule, written on the wall: **no single lost object = lost data.**

### 4. Power: UPS
Dirty shutdowns kill filesystems that aren't ZFS and kill disks that are
mid-write. A UPS that signals Castle to shut down cleanly turns "power
outage" from a data-loss event into a nap.

### 5. Offsite: 3-2-1
3 copies, 2 different media, 1 offsite. The offsite copy is encrypted before
it leaves the building — whoever holds it can't read it.
- Long-term vision: Castle warehouse facilities (Brando's plan) — encrypted
  replicas between Castles.
- Interim: encrypted USB drive at a trusted location, rotated monthly, or a
  cheap VPS holding only ciphertext.
- **A backup you haven't restored is a rumor.** Restores get tested on a
  schedule, automatically, and a failed test pages like a fire alarm.

### 6. Watching it: SMART + scrubs + alerts
Disks tell you they're dying before they die (SMART). Scrubs run on schedule
and verify every checksum. Any degradation pushes an alert — the zero-token
uptime pinger work is the building block for this. Silence means healthy;
anything else means act NOW, because redundancy only saves you if you replace
the dead disk before the second one goes.

## Tiers, plainly

| | Good | Fortress |
|---|---|---|
| Filesystem | ZFS | ZFS |
| Disks | 2-disk mirror | 3-way mirror or RAIDZ2 + hot spare |
| Snapshots | daily | hourly + daily, longer retention |
| Encryption keys | passphrase + paper backup | + hardware key + Shamir shards |
| Power | surge protector | UPS with clean-shutdown signaling |
| Offsite | manual USB rotation | automated encrypted replica |
| Monitoring | scrub logs | SMART + scrub + push alerts |

Good is honest redundancy. Fortress is "the most powerful redundancy system
in the world" for a box that sits in a house. Start at Good, design for
Fortress — every layer above is additive, never a reinstall.
