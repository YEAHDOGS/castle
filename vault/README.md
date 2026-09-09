# Family Data Vault — core (`vault.py`)

Implements FAMILY-DATA-VAULT.md build-order step 1: vault-per-user data
layout + age-tiered accounts, wired into the flamethrower Tier-1 keyring.

## The model

Every family member gets a **private vault** — nothing is shared unless
explicitly granted via the sharing ladder (vision build-order step 2,
implemented in this module too). Every vault gets a real random 256-bit
data key in the flamethrower keyring; deleting the key **is** deleting the
data (crypto-shredding, media-independent).

## Age tiers (a property of the account)

| | adult (18+) | child (<18) |
|---|---|---|
| Private vault + all subdirs | ✅ | ✅ |
| Share to member / family | ✅ | ✅ |
| **Share to world** | ✅ | ❌ refused |
| **Integrations** (POS push, API tokens) | ✅ | ❌ refused |
| Device approval | self | guardian approves (`--by <guardian>`) |
| Key escrow (family recovery) | optional | always on |

## Usage

```bash
python3 vault.py init
python3 vault.py create-user dad --tier adult
python3 vault.py create-user sally --tier child --guardian mom

python3 vault.py add-device sally sally-phone --by mom
python3 vault.py add-integration dad clover-pos

# sharing ladder: explicit, revocable, logged
python3 vault.py grant-share mom --to family
python3 vault.py grant-share mom --to dad
python3 vault.py revoke-share mom --to dad
python3 vault.py show-shares mom

# true deletion: dry-run default; typed confirmation burns
python3 vault.py delete-user sally          # lists what would die
python3 vault.py delete-user sally --yes sally   # key crypto-shredded, dirs wiped

python3 vault.py verify

# integrity: SHA-256 manifest + drift audit (FAMILY-DATA-VAULT step 6)
python3 vault.py manifest ~/family-vaults/mom --out mom-manifest.json
python3 vault.py audit ~/family-vaults/mom --manifest mom-manifest.json  # exit 0 = clean

# encrypted-at-rest sealing (FAMILY-DATA-VAULT step 7) — openssl only.
# Secret files must already be mode 0600; lock is dry-run by default.
python3 vault.py vault-init ~/family-vaults/mom --out mom.castle --passphrase-file ~/.castle-pw
python3 vault.py vault-lock ~/family-vaults/mom --out mom.castle --passphrase-file ~/.castle-pw
python3 vault.py vault-lock ~/family-vaults/mom --out mom.castle --passphrase-file ~/.castle-pw --yes mom  # burns plaintext
python3 vault.py vault-unlock mom.castle --out ~/restored-mom --passphrase-file ~/.castle-pw
```

```bash
python3 test_vault.py   # 26 fixture-based regression tests, temp dirs only
python3 test_manifest.py  # 13 regression tests: manifest build/write/load + refusals
python3 test_verify.py    # 13 regression tests: ADDED/REMOVED/MODIFIED/UNCHANGED + CLI
python3 test_vault_lock.py  # 25 regression tests: init/lock/unlock, fail-closed refusals
```

Layout under the vault root (`--dir`, default `~/.castle-vault`), all
0700 dirs / 0600 files: `users.json`, `activity.jsonl`, `vaults/<user>/`
(inbox, documents, photos, receipts, shared), `keys/` (keyring root).

`activity.jsonl` is the append-only event log — who did what to which
vault, never key material or file bytes. (Deletions also get flamethrower
deletion certificates + the cross-burn audit log; the activity log
records *that* the burn happened.)

## Tests

```bash
python3 test_vault.py   # 26 fixture-based regression tests, temp dirs only
```

## Security rules this module enforces

- Name validation on every user/vault name (no path traversal).
- Child accounts: world-share and integrations refused, devices need
  guardian approval, keys always escrowed for family recovery.
- True deletion needs typed confirmation (the user name); dry-run is
  the default; revoking a non-existent share is a refusal, not a no-op.
- Key bytes never touch stdout, logs, certificates, or the activity log.
