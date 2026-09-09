# Castle Family Data Vault

Brando's directive (2026-09-09): Castle isn't a single-user NAS. It's the
family's data home — with real privacy *between* family members, an
age-aware account system, and a receipt/financial vault that third-party
systems (like Clover POS) can push into.

## The model: nuclear family, zero implicit trust

Mom, Dad, Sam, Sally. Every member gets a **private vault** on Castle.
Defaults:

- Sally cannot see Sam's data. Sam cannot see Sally's.
- Dad cannot see all of Mom's data. Mom cannot see all of Dad's.
- Nothing is shared unless someone explicitly shares it.

Sharing is a ladder, each rung explicit:

1. **Private** — only the owner.
2. **Shared to a member** — Sam shares a folder with Mom.
3. **Shared to the family** — the vacation photos everyone gets.
4. **Shared to the world** — a public link, revocable.

No "family admin sees everything" backdoor. If Dad needs something of
Mom's, Mom shares it. That's the whole point — Castle restores data
ownership *inside* the family, not just against Big Tech.

## Age-tiered accounts

- **Under 18**: can own records — receipts in their name, school docs,
  savings. Restricted surface: can't share to the world, can't add
  integrations, guardian approves new devices.
- **18+**: full vault, full sharing ladder, integrations, API tokens.

The tier is a property of the account, not a separate product. A kid's
account graduates; their data comes with them.

## The receipt vault

Castle backs up financial records and receipts as a first-class citizen:

- **Clover (and friends)**: the vision is POS systems pushing receipts
  straight to your Castle — webhook/API receiver per user vault, so a
  purchase at a Clover terminal lands in *your* records, not just theirs.
- **Email ingestion**: forward receipts to `receipts@<you>.castle`.
- **Scan**: photo → OCR → structured receipt in the vault.
- Every receipt is searchable, timestamped, and stays yours when you
  switch banks, switch POS vendors, switch anything.

## Remote access: Tailscale + WireGuard + hosted DNS

Already in the Castle architecture (see README diagram): the Castle sits
on a tailnet. Every family member's phone/laptop joins the same tailnet,
so `vault.castle` / `receipts.castle` resolve anywhere on Earth via the
hosted DNS (Pi-hole) — no port forwarding, no dynamic-DNS hacks, no
exposing SMB to the open internet. WireGuard is the underlay; Tailscale
is the easy button. Drawbridge (VPN gateway ingress) remains the path
for devices that can't run Tailscale.

## Build order

1. Vault-per-user data layout + auth (users, age tier on the account). ✅ done
   (`vault/vault.py`: registry + 0700 layout with inbox/documents/photos/
   receipts/shared, a real flamethrower Tier-1 data key per vault, age
   tiers with guardian approval for child devices, forced escrow for
   children, integrations adult-only, `verify` consistency check.)
2. Sharing ladder (private → member → family → world), all explicit,
   all revocable, all logged. ✅ done
   (`vault/vault.py` grant/revoke: child world-share refused, dup grants
   and phantom revokes refused, every event in append-only activity.jsonl,
   `delete-user` crypto-shreds key + wipes dirs behind typed confirmation;
   26 regression tests green, temp dirs only.)
3. Receipt ingestion: email forward first (simplest), then scan/OCR,
   then the Clover-style POS webhook receiver. 🟡 email forward done
   (`vault/receipts.py`: ingest raw RFC822 from `receipts@<you>.castle` into
   the addressed user's receipts/ — merchant/total extraction with honest
   confidence, attachments stored 0600 alongside the original .eml, ingest
   refused unless the `email-forward` integration is registered (adult-only
   surface), mis-addressed mail refused (never lands in the wrong vault),
   idempotent by sha256 content hash, activity log carries metadata only —
   never body/attachment bytes; 14 regression tests green, temp dirs only.
   `vault.py ingest-receipt FILE` CLI). POS webhook receiver ✅ done
   (`vault/poshook.py`: `ingest-pos` ingests one JSON push body into the
   addressed user's receipts/ — `"user"` in the payload must match the
   vault being written to, address mismatches/unknown users refused;
   `pos-webhook` integration required (add-integration is adult-only, so
   child accounts can't enable the surface); money is an exact decimal
   string — float totals refused, never rounded; idempotent by sha256 of
   canonical JSON plus per-user `transaction_id` dedupe (webhook retries
   never dupe); shares receipts/ storage with email ingestion (rcpt dirs,
   index.jsonl with a `source` field); 0600/0700 perms, raw push kept as
   `raw.json`, activity log carries metadata only; 16 regression tests
   green, temp dirs only. No network listener on purpose — HTTP surface
   stays outside; this module owns the trust boundary.)
   Scan/OCR ✅ done
   (`vault/scan.py`: ingests a scan image + its OCR text into the named
   user's receipts/ — the caller supplies the image and the OCR text,
   Castle never runs the OCR engine itself. Image validated by magic
   bytes (JPEG/PNG/WebP, 1KB–50MB) so renamed junk is refused; OCR text
   must be non-empty (a scan without extracted text is not a receipt);
   merchant = stated hint (high confidence) or first OCR line (low),
   total via the same honest extraction as email ingestion (unknown, never
   zero); idempotent by sha256(image+text); unsafe filenames refused
   before any write (canary test proves no directory escape); `scan`
   integration required (adult-only to register); image/OCR/receipt all
   0600, activity log metadata only; `vault.py ingest-scan FILE --ocr
   TEXT --user NAME` CLI; 23 regression tests green, temp dirs only.
   Step 3 fully done — email + POS webhook + scan/OCR all land in the
   shared receipts/ layout.)
   API-token auth (2026-09-09): `vault/apiauth.py` wires the DOGS Token
   Authority into the ingestion trust boundary — scoped, short-lived,
   revocable bearer tokens (`iss=aud=castle-vault`, sub = the vault user,
   purpose = the integration name) for machine callers like a Clover POS
   push. OFF by default (zero behavior change); `vault.py api-auth
   --purpose pos-webhook --on` flips a purpose to token-required per
   vault root, `vault.py mint-api-token USER --purpose ...` mints (stdout
   — bearer credential; the activity log records jti only, never the
   token), `revoke-api-token` kills a leaked one. Removing an
   integration voids its tokens implicitly. 21 regression tests green,
   temp dirs only.
4. Tailscale onboarding flow per device + hosted DNS names per service. ✅ done
   (`vault/tailnet.py`: `tailnet-onboard` starts onboarding for a device
   that's already registered on the account (guardian approval for kids
   is inherited from step 1's `add-device`), and prints an operator
   runbook — the exact `sudo tailscale up --authkey <PASTE-AUTH-KEY>`
   command with a slugged hostname; the auth key is never stored,
   never logged, only pasted by the operator. `tailnet-claim` marks a
   pending device active after it joins, accepting ONLY 100.64.0.0/10
   addresses — LAN/loopback/public IPs are refused, not recorded.
   `tailnet-status` resolves tailnet records against the live registry
   so revoked devices drop out of the active set; `tailnet-drop`
   purges a record for re-homing. `tailnet-dns --castle-ip 100.X`
   writes the Pi-hole dnsmasq fragment naming the services from the
   vision doc (`address=/vault.castle/...`, `address=/receipts.castle/...`);
   Castle never reloads DNS itself — the operator installs the
   fragment. No network calls anywhere in the module (zero outbound).
   `vault.py tailnet-onboard|tailnet-claim|tailnet-status|tailnet-drop|
   tailnet-dns` CLI; tailnet.json is 0600, dnsmasq.d is 0700, the
   activity log carries metadata only. 23 fixture-based regression
   tests green, temp dirs only.)
5. Guardian controls for under-18 accounts. ✅ done
   (`vault/guardian.py`: scoped guardian power — only over the guardian's
   OWN children, never adults (no family-admin backdoor, per the vision).
   `revoke_device`: guardian revokes a child's device (lost/rogue), adults
   self-revoke; `guardian_revoke_share`: guardian revokes any share their
   child granted; `graduate`: guardian-approved child → adult tier with
   data, devices, shares and escrow preserved (adult surface unlocks:
   world shares, integrations); `guardian_review`: metadata-only activity
   filter for supervision. Phantom/unauthorized attempts are refusals, not
   silent no-ops; every exercise is activity-logged. `vault.py` CLI:
   `revoke-device`, `guardian-revoke-share`, `graduate`; 18 fixture-based
   regression tests green, temp dirs only.)
6. Vault integrity: file manifest + drift audit. ✅ done
   (`vault/manifest.py` builds a deterministic JSON manifest of a vault
   directory — relative paths, sizes, SHA-256 (real crypto via
   ``hashlib``, streamed in chunks), mtimes; symlinked targets, the
   filesystem root, and manifests living inside the tree they describe
   are all refused; symlinks and non-regular files are skipped, never
   followed. `vault/verify.py` re-scans against a manifest and reports
   ADDED / REMOVED / MODIFIED / UNCHANGED (exit 0 when clean, 2 with a
   human-readable diff, 1 on a corrupt manifest — corruption is refused,
   never trusted). `vault.py manifest <dir> --out FILE` and
   `vault.py audit <dir> --manifest FILE` CLI. Manifests are 0600 and
   byte-deterministic. 26 regression tests green, temp dirs only.)
7. Vault init/lock/unlock: encrypted-at-rest sealing. ✅ done
   (`vault/vault_lock.py`: `vault.py vault-init DIR --out F.castle`,
   `vault-lock`, `vault-unlock` — openssl only, no new hosts.
   `openssl enc` on this machine refuses AEAD ciphers, so the suite
   fails closed to AES-256-CBC + encrypt-then-MAC HMAC-SHA256 (recorded
   in the header for future GCM migration). Passphrase → PBKDF2-SHA256
   (600k iters, 64 bytes split enc/MAC); keyfile must be exactly 32
   bytes and already mode 0600 — the CLI checks permissions, never
   chmods, and refuses 0777 keyfiles before any crypto runs. Lock is
   dry-run by default, burns plaintext via the flamethrower
   directory-burn only after the sealed copy decrypt-verifies in the
   same run. Unlock verifies HMAC before decrypt, tarball SHA-256
   before extract, per-file hashes after extract; tampered vaults and
   wrong passphrases fail closed. 25 regression tests green, temp dirs
   only.)
8. Encrypted backups: seal a user's vault to the backup target, proved
   restorable. ✅ done
   (`vault/backup.py`: `vault.py backup USER --target-dir DIR
   --passphrase-file F [--yes USER]` seals one user's vault into a
   timestamped `<user>-<utc>.castle` on the backup target using the
   step-7 lock (AES-256-CBC + encrypt-then-MAC, passphrase or raw
   keyfile) — the live vault is NEVER touched or burned (a backup that
   destroys the original is a fire, not a backup). The backup is
   decrypt-verified in the same run: HMAC, tarball SHA-256, then every
   per-file SHA-256 in the header checked against the tarball members —
   a backup that hasn't been proven restorable is a rumor.
   `vault.py backup-verify FILE.castle` re-proves an existing backup
   bit-exact without extracting to disk. Dry-run is the default, real
   backups need typed confirmation (the username); existing backups are
   never overwritten (new timestamped names are minted); targets that
   are/inside/contain the vault are refused; secrets living inside the
   vault or the target are refused; wrong passphrase or tampered file
   fails closed; one activity record per backup (metadata only).
   23 fixture-based regression tests green, temp dirs only.)
   Chunked lane (2026-09-09): `vault/chunkseal.py` adds a pure-stdlib
   chunked container (castle-chunks/v1, per-chunk HMAC-SHA256, KDF
   params in the header) alongside the openssl-backed format —
   `vault.py backup --chunks`; `backup-verify` auto-detects the
   format. Crypto choices and honest limitations: docs/CRYPTO-NOTES.md.
