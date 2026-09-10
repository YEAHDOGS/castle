# Family Recovery — Surviving Lost Keys on a Crypto-Shredding Machine

Brando's directive (2026-09-09): crypto-shredding cuts both ways. A
deleted key **is** deleted data — that's the guarantee that makes the
flamethrower real. It also means a *lost* key is lost data. A family
vault where forgetting your passphrase or losing your phone deletes
the kids' baby photos forever isn't a vault, it's a trap. Recovery is
the design that keeps the guarantee without the trap.

## The paradox, stated plainly

Every vault on Castle is encrypted at rest with a per-vault data key
(see `flamethrower/keyring.py`: 256-bit CSPRNG key, keyring dir 0700,
key files 0600). The flamethrower Tier-1 kill destroys the key —
instant, media-independent, total. So key loss is exactly as final as
key destruction, whether you meant it or not:

- Dad forgets the vault passphrase → same outcome as Dad flamethrowing it.
- Mom's phone (the only device with her key unlocked) goes through the
  washing machine → same outcome.
- Dad dies. His key dies with him. Mom needs the will and the family
  photos — they were in Dad's vault, encrypted, keyless. The data is
  gone at the level of physics. **Nobody recovers it, not even Mom,
  not even the Castle itself.** The crypto doesn't care about probate.

So: recovery must exist, and recovery must never become the
family-admin backdoor the vision forbids (see FAMILY-DATA-VAULT.md:
"no family admin sees everything"). The design walks that line.

## What recovery must survive (threats)

1. **Forgotten passphrase / lost device.** The common case. Owner is
   alive, cooperative, and verified.
2. **Death or incapacitation.** Owner can't participate. Family needs
   defined heirs, not a locksmith.
3. **Castle destroyed.** House fire takes the box. Recovery material
   must exist off-Castle, or the whole family fails the threat that
   backups were supposed to survive.
4. **Malicious insider.** The person running the recovery ceremony
   cannot be a single self-interested family member. A kid cannot
   "recover" Dad's vault. A divorcing spouse cannot recover the
   other's.
5. **Simultaneous compromise.** Castle is stolen *and* the owner is
   gone. Recovery material alone must not open the vault without the
   quorum.

## What recovery must NOT survive (non-goals)

- **Subpoena fishing.** Recovery is a quorum ceremony with a delay
  window — it is not a backdoor with a warrant canary, and it's not
  fast. Coercing one person gets you nothing.
- **Silent recovery.** Every step is audit-logged. A recovery the
  owner didn't know about is a breach, and the log must show it.
- **Same-day convenience.** Recovery is slow by design. Fast access is
  what unlocks are for. If you can wait a week, you weren't locked out
  of lunch — you were locked out of your life.

## Data model: the recovery contract

Every vault carries a **recovery contract** — metadata, 0600, in the
keyring record — written at vault creation and changeable only by the
vault owner with the same interlocks as a burn (typed confirmation).
The contract names:

- **Quorum (m-of-n):** the set of people who can authorize recovery
  and the threshold. Defaults: child vaults → 1-of-2 (both parents;
  see forced escrow below); adult vaults → owner picks, minimum 2-of-3
  with at least one person outside the household.
- **Delay window:** how long between quorum approval and execution.
  Default 7 days. The owner can abort any time during the window —
  this is the anti-coercion property: nobody can be quietly recovered
  against.
- **Heirs (death clause):** named separately from the quorum. The
  quorum recovers *to* the heirs; heirs don't self-authorize.
- **Escrow copies:** where key material lives. A quorum that can
  authorize but has nothing to authorize *with* is theater.

### Escrow, three copies, none on the same disk

The existing keyring escrow (`flamethrower/keyring.py --escrow`) keeps
one escrow copy in `escrow/<name>.key` **on the Castle itself** — and
the current recovery primitive (`recover-vault`) restores from it.
That's the honest starting point, and it's honest about its limits:

- Copy 1: on-Castle escrow (today). Survives lost passphrases, lost
  devices. Dies with the Castle (fire, theft, disk death).
- Copy 2: off-Castle split. Per-vault data key split into shares held
  by quorum members — any m-of-n reconstructs. Shares leave the Castle
  (QR codes, printed, USB in a safe-deposit box). No single share
  opens anything.
- Copy 3: family recovery group — the drive-librarian playbook
  extended: a sealed recovery envelope per vault on a designated
  family recovery drive, inventoried by the Drive Librarian's udev
  fingerprinting so a missing recovery drive is an *alert*, not a
  discovery.

The rule: **a vault is not recoverable until Copy 1 + one off-Castle
copy exist.** Until then the contract is unsigned, and the UI says so
in red. A recovery story that starts "the Castle burned down" and
ends "we had one copy, it was on the Castle" isn't a story, it's an
obituary.

## The ceremony (concrete steps)

Recovery is a state machine, not a button. All states persist in
0600 metadata; every transition lands in the audit log.

1. **Request.** Someone (the owner, or a quorum member for an
   incapacitated owner) files a recovery request naming the vault.
   The request records: who asked, which vault, the reason, and — if
   the owner is alive — the owner's own declaration. Dry-run first:
   the tool prints exactly what would be reconstructed, from which
   copies, to whom.
2. **Quorum approval.** m-of-n quorum members approve within a 30-day
   window, each from their own device, each approval a signed record.
   The approver set can never include the heir receiving the data.
   Approval is public inside the family (everyone sees that a recovery
   is in flight — secret recovery is the backdoor).
3. **Delay.** The 7-day window starts at full quorum. The owner (if
   alive) can abort with one tap, no quorum needed to abort. Aborts
   are also audit-logged — a coerced abort and a genuine one look the
   same in the log, which is why the delay is long enough to ask in
   person.
4. **Execute.** After the window, any quorum member completes the
   reconstruction: shares combine, the key is verified against the
   recorded key receipt (SHA-256, same as `recover-vault` does today),
   and installed at `keys/<name>.key` 0600. Key bytes never touch
   stdout, logs, or the request record — only hashes.
5. **Certificate.** A recovery certificate issues: vault, quorum
   members, window dates, key receipt hash, executor. One audit record
   per recovery. Recovery does not destroy the escrow copies —
   recovering must never burn the recovery path.

## Age tiers: who gets forced escrow, who opts in

- **Child vaults: forced.** The vision already says forced escrow for
  children (`vault.py` step 1), and recovery inherits it: guardian(s)
  hold the off-Castle shares, 1-of-2 to approve, heirs default to the
  guardian(s). A kid's data survives the kid's lost phone *and* the
  parent's lost phone. `guardian.py` initiates; the other guardian
  approves. Neither guardian alone can open the vault — approval
  authorizes a recovery *to the owner*, not to the approver.
- **Adult vaults: opt-in, nudged hard.** Adults choose their quorum,
  their heirs, their delay. The default prompt at vault creation: 2
  people outside the household + 7-day delay. The app nags until the
  contract is signed (unsigned contract = red banner, same energy as
  "your backup hasn't run in 30 days"). Opt-out is allowed — some
  adults will want a vault nobody can ever open, and the design
  respects that as an explicit choice, not a default accident.

## Failure modes (the honest table)

| Failure | What happens | Why it's acceptable |
|---|---|---|
| Quorum unreachable (members dead/lost) | Vault unrecoverable | Same as no-recovery design; the contract was explicit about n |
| One quorum member malicious | Nothing — needs m approvals, delay, owner abort | m-of-n, not 1-of-n |
| Owner coerced to approve | Owner's own approval isn't enough; delay + public-in-family | Coercion needs the whole quorum |
| Castle destroyed | Off-Castle shares (copies 2–3) reconstruct on a new Castle | This is why one copy never lives on the Castle alone |
| Quorum member's share leaked | Nothing — one share is noise | m-of-n, never 1-of-n |
| Owner dies with unsigned contract | Vault unrecoverable; family is told this in advance | Honest failure beats surprise failure |
| Recovery tool bug forks the key | `recover-vault` already refuses when the live key is present | Same interlock carries over: never reconstruct over a live key |

## Security properties (what we promise)

1. **No backdoor.** There is no master key, no Castle-held recovery
   key, no family admin. The Castle operator (Brando) cannot recover
   Mom's vault. If the Castle could, it's not your data — it's a
   cloud service with extra steps.
2. **Public-in-family.** Every request, approval, abort, and execution
   is visible to the whole family and audit-logged. Stealth is the
   enemy of consent.
3. **Delay as defense.** The 7-day window is the anti-coercion and
   anti-compromise property. Fast recovery is fast theft.
4. **Fail closed.** Missing quorum → no recovery. Missing off-Castle
   copy → no recovery. Corrupt share → refused, never reconstructed
   partially. The current `recover-vault` refusals (no escrow, live
   key present, fork refusal) are the prototype of this posture.
5. **Escrow destruction parity.** Flamethrower Tier-1 destruction must
   enumerate and kill every copy (keyring + escrow + shares). A burn
   that misses the off-Castle share is a leak, not a deletion — the
   destruction certificate must list the shares and their fate
   (physical destruction, return-to-holder, or "unrecoverable by
   design" if the holder is gone, stated honestly).

## Honest limitations

- **Paper is the real off-Castle medium.** QR codes and USBs work;
  printed shares in a safe-deposit box survive the house fire, the
  dead laptop, and the dead phone all at once. The design says this
  out loud because a recovery story that depends on a working
  smartphone is a story for the non-emergency case.
- **m-of-n crypto isn't implemented yet.** The data model names it;
  `keyring.py` today does escrow-copy recovery (m=1 of 1 on-Castle).
  The honest path is: implement share-splitting (stdlib, e.g.
  XOR-based n-of-n first, then Shamir m-of-n) *after* the quorum
  ceremony and audit states are tested. Crypto without ceremony is a
  footgun; ceremony without crypto is still the current system.
- **Social recovery degrades under family fracture.** Divorce,
  estrangement, and death all change the quorum. The contract needs a
  re-sign reminder on family-structure changes (annual "recovery
  health check" — the app nags, same as backups). A stale quorum is
  a quiet failure mode; the health check makes it loud.
- **The Castle can't verify quorum identity cryptographically yet.**
  Today, approvals are operator-run records. Device-bound signing
  (each quorum member's approval signed by their Castle-registered
  device key) is the follow-up.

## Build order

1. **Recovery contract schema** in the keyring metadata (quorum,
   heirs, delay, copies) — unsigned by default, red-bannered.
2. **State machine**: request → approve → delay → execute → cert,
   with the abort path and audit records. Dry-run default, typed
   confirmation, mirrors the flamethrower interlocks.
3. **Off-Castle share export/import** (XOR n-of-n shares first —
   simple, auditable, stdlib; Shamir m-of-n after).
4. **Health check**: the app flags unsigned contracts, unreachable
   quorum members, and missing off-Castle copies — a recovery story
   you can't test is a story, not a plan.
5. **Device-signed approvals** for quorum members.

Crypto-shredding promised that deletion is total. Recovery promises
that *accidental* deletion doesn't have to be. Both promises are kept
by the same mechanism: the key is everything, and the design never
forgets it.
