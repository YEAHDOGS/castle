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

1. Vault-per-user data layout + auth (users, age tier on the account).
2. Sharing ladder (private → member → family → world), all explicit,
   all revocable, all logged.
3. Receipt ingestion: email forward first (simplest), then scan/OCR,
   then the Clover-style POS webhook receiver.
4. Tailscale onboarding flow per device + hosted DNS names per service.
5. Guardian controls for under-18 accounts.
