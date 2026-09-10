# Castle Vision: Chain of Custody for Every File

*Dictated 2026-09-10. Vision only — not a build order.*

## The idea

Every file on the Castle carries a **chain of custody** — and Castle should be
transparent about it:

- **who** created it, who touched it, who touched it last
- **when** each touch happened
- **what** changed between touches
- **which** revision this is (the count)

No forensics license required. The history is the feature: any file, on
demand, shows where it's been. Nothing hidden, nothing "trust us."

## Two anchors

1. **User accounts.** Identity is the foundation — a chain of custody with no
   "who" is just a pile of timestamps. Every link in the chain is pinned to a
   Castle user account (admin vs. user profiles already exist in the identity
   model — the chain hangs off those).
2. **Counts.** Version/edit counts are tremendously important. The revision
   number is what makes the sequence a *chain* instead of a rumor — point at
   revision 14 and know exactly what came before and after.

## The OS already knows (but won't tell you)

This isn't science fiction. Windows already tracks most of it:

- **USN Change Journal** — a per-volume log of every file change
- **File History** — periodic versioned backups
- **Volume Shadow Copy (VSS)** — point-in-time snapshots

But none of it is a *premier* part of the operating system. It's buried: no
first-class UI, no user-facing timeline, no "show me this file's history"
that a normal person can reach. Forensics tools walk in and read it all day
long — the data is sitting right there. The user just isn't allowed to see it.

Castle's Data Vault flips that: the vault **is** the place where file history
lives in the open.

## Relationship to Chains

Chains ("git for save data") is the model in miniature — snapshot, hash,
parent pointer, message. The Data Vault generalizes it: a save file is just a
file, and the machinery doesn't care what the bytes mean. Chains stays scoped
to save data; this document is the north star for the vault.

## What this is not (yet)

- Not a replacement for backups (Drive Librarian's immutable plug-in backups
  still do that job).
- Not surveillance — the chain belongs to the file's owner, on their Castle.
- Not a v1 feature. Vision only.

## Open questions

- Where does the chain live? (Sidecar ledger, filesystem xattrs/ADS, vault DB?)
- How do we handle files that move across machines / accounts?
- What does "restore revision N of any file" look like in the Castle app?
- How does the chain interact with the immutable ledger in the Armoury?
