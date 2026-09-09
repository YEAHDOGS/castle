# Castle crypto notes — what we use, and what we're honest about

This file records the cryptographic choices in the Castle vault/backup
tooling, their limitations, and the migration path. Brando's rule:
never fabricate security — document the real construction and its
real weaknesses.

## The two backup containers

| | castle-vault/v1 | castle-chunks/v1 |
|---|---|---|
| Code | `vault/vault_lock.py` | `vault/chunkseal.py` |
| Deps | Python stdlib + `openssl` CLI | Python stdlib only |
| Cipher | AES-256-CBC (real AES, via openssl) | SHA-256 counter-mode stream cipher (prototype) |
| Integrity | encrypt-then-MAC: one HMAC-SHA256 over the whole ciphertext | encrypt-then-MAC: one HMAC-SHA256 *per chunk* + whole-payload SHA-256 |
| KDF | PBKDF2-HMAC-SHA256, 600k iters (passphrase) / raw 32-byte keyfile | scrypt (N=2^15, r=8, p=1, 64-byte out), falling back to PBKDF2-HMAC-SHA256 600k if scrypt is unavailable; params stored in the header |
| Granularity | one monolithic ciphertext | 1 MiB chunks (configurable); a flipped byte names the exact chunk that failed |

`backup.py` writes `castle-vault/v1` by default and `castle-chunks/v1`
with `--chunks`; `backup-verify` detects the format from the header
automatically. The certificate records `container`, `cipher`, and
`mac` so it's always visible which suite sealed a given backup.

## Honest limitations of castle-chunks/v1

1. **The stream cipher is not AES.** The Python stdlib has no AES, and
   `pip install` is off the table (default-deny network). SHA-256 in
   counter mode is a sound *construction* — unique (key, nonce,
   chunk-index, block-counter) per keystream block, no keystream reuse
   — but it has not had the cryptanalysis AES has. Treat it as
   prototype-grade. (Precedent: `flamethrower/fileburn.py` already
   ships the same construction labeled as a prototype.)
2. **Production path: AES-256-GCM.** The header's `cipher` field names
   the suite precisely so migration is mechanical: when a real crypto
   library (e.g. `cryptography`) is on the machine, add a GCM suite,
   keep the format id versioned, and re-seal.
3. **Side channels.** The pure-Python XOR loop is not constant-time;
   this matters against local timing adversaries, not against a stolen
   backup file. The threat model here is *data at rest on a disk you
   don't control* — wrong passphrase and tampered bytes fail closed,
   which is what the tests pin down.
4. **KDF memory.** scrypt at N=2^15/r=8/p=1 uses ~32 MiB. If the box
   can't do it, the code fails *over* to PBKDF2-600k and records which
   one ran in the header — the reader redoes exactly that. Neither
   choice is silent.

## What the tests actually prove

- Round-trip: sealed bytes open to the exact input bytes.
- Wrong passphrase: typed `ChunkSealError`, no traceback, nothing
  half-decrypted. The CLI prints `refused: ...` and exits 1.
- Flipped byte anywhere in the body: refused, and the error names the
  chunk index (tamper localization, not just "bad file").
- Truncated bodies, bad headers, and foreign format ids: refused.
- KDF params round-trip through the header (the suite a backup was
  sealed with is the suite it opens with).

## Rules that hold across both containers

- Secrets arrive via `--passphrase-file` / `--keyfile` only, mode
  exactly 0600 (checked, never chmod'd), never argv/env/logs.
- Key material is split (enc key vs MAC key) — one secret, two keys.
- Verify-before-trust: HMAC first, decrypt second, per-file hashes
  third. Tampered or wrong-key backups are refused, never partially
  restored.
- A backup that hasn't been proven restorable in the same run is a
  rumor, not a backup.
