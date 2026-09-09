# Flamethrower — Tier 1 crypto-shredding key lifecycle

Implements FLAMETHROWER.md build-order steps 1–2: per-vault encryption key
lifecycle (generation → escrow → destruction ceremony → deletion
certificate). Stdlib Python only — no third-party crypto, no network.

## The idea

Every vault gets a random 256-bit data key. Deleting the key **is**
deleting the data: AES-256 ciphertext without its key is noise, on any
media (HDD, SSD, SD — crypto-shredding is media-independent). The
certificate labels everything honestly; it records *that* something
burned, never its contents.

## Usage

```bash
# one-time setup (keyring dir is 0700, key files 0600)
python3 keyring.py init

# create a vault key; --escrow keeps a family-recovery copy
python3 keyring.py create-vault mom --escrow
python3 keyring.py list

# destruction ceremony: dry-run is the DEFAULT
python3 keyring.py destroy-vault mom
# → lists exactly what would die; exits 2; nothing burned

# real burn needs TYPED confirmation (the vault name, not "y")
python3 keyring.py destroy-vault mom --yes mom
# → overwrites key bytes 3× (CSPRNG), fsync, unlink — keyring AND escrow
# → emits certificates/<uuid>.json

# verify a deletion certificate (fails on tampered/missing fields)
python3 keyring.py verify-cert certificates/<uuid>.json

# burn the whole keyring (typed: DESTROY-ALL)
python3 keyring.py destroy-master --yes DESTROY-ALL
```

## Safety interlocks (same DNA as the Phoenix nuke tool)

- Dry-run default; real run is the exception.
- Typed confirmation (vault name / `DESTROY-ALL`).
- Explicit target enumeration before the burn; no globs, no "older than X".
- Key bytes never touch stdout, logs, or certificates — certificates carry
  only a SHA-256 receipt proving *which* key died.

## Tests

```bash
python3 test_keyring.py   # 11 fixture-based regression tests, temp dirs only
```

## What's next (per FLAMETHROWER.md build order)

3. Media detection for Tier 2/3 (this module already ships a best-effort
   `detect_media()` used on certificates).
4. Tier 2 firmware-erase routines (NVMe format / ATA Secure Erase, shared
   with Phoenix tooling) — refusing to "overwrite" flash and call it done.
5. Tier 3 HDD file shredder with honest media labeling.
6. Wire the keyring into the vault-per-user layout from FAMILY-DATA-VAULT.md.
