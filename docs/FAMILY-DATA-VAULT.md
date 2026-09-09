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
4. Tailscale onboarding flow per device + hosted DNS names per service.
5. Guardian controls for under-18 accounts.
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
