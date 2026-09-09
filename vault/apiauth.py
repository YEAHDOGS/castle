#!/usr/bin/env python3
"""Castle API-token auth — wires the DOGS Token Authority
(castle/tokens/tokens.py, the encrypted anti-JWT bearer tokens) into the
family data vault's ingestion trust boundary (FAMILY-DATA-VAULT.md
build-order step 3).

Why: the ingestion modules (email-forward, pos-webhook, scan) currently
gate on *registration* of an integration — an operator act that says
nothing about WHO is calling. A Clover POS pushing receipts or a
mail-forward agent needs a real caller credential that is:

  - scoped: minted for exactly one user vault + one purpose
  - short-lived: tokens expire (default 1h, hard max 24h — the Token
    Authority refuses longer ttls)
  - revocable: `revoke-api-token` kills a leaked token instantly
  - opaque on the wire: the anti-JWT — claims are ENCRYPTED, not merely
    signed, and aud == "castle-vault" is enforced, so a token minted for
    Castle never verifies on any other audience

How it fits the trust boundary:

  - OFF by default: ingestion behaves exactly as before (registration
    gate only). Zero regression risk — existing callers keep working
    until the operator opts in.
  - `vault.py api-auth --purpose pos-webhook --on` flips one purpose to
    token-required. The flag is per-vault-root operator policy, stored in
    users.json under "api_auth". `--off` flips it back.
  - `vault.py mint-api-token USER --purpose pos-webhook [--ttl 3600]`
    mints a scoped bearer token and prints it to stdout (a bearer
    credential — treat stdout like a password field). The activity log
    records jti / purpose / ttl only — never the token.
  - The three ingest() functions (poshook, receipts, scan) take an
    optional keyword-only `api_token=`: when the purpose is
    token-required for this vault root, ingestion without a valid token
    is refused, fail closed.
  - require() ALSO re-checks that the integration is still registered:
    removing an integration kills that surface's tokens implicitly, even
    before they expire.

Stdlib only + the on-machine openssl CLI (via tokens.py). No network,
no new hosts.
"""

import os
import sys
import time

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault as _vault  # noqa: E402  (registry, perms, activity plumbing)
sys.path.insert(0, os.path.join(_HERE, "..", "tokens"))
import tokens as _tokens  # noqa: E402  (DOGS Token Authority)

# Token claims namespace for Castle ingestion.
ISS = "castle-vault"
AUD = "castle-vault"

# Purpose == the integration name it authorizes. One token, one vault,
# one purpose — never interchangeable.
PURPOSES = ("pos-webhook", "email-forward", "scan")

KEYRING_SUBDIR = "tokenkeys"
MIN_TTL = 60               # a sub-minute webhook credential is useless
DEFAULT_TTL = 3600         # hard max is the Token Authority's 24h


def _keyring_dir(root):
    return os.path.join(root, KEYRING_SUBDIR)


def _ensure_keyring(root):
    """Create the vault-root token keyring on first mint. Refuses to
    touch an existing one — init is not rotation."""
    kdir = _keyring_dir(root)
    try:
        _tokens.init_keyring(d=kdir, note="castle-vault api tokens")
    except _tokens.TokenError as e:
        if "already initialized" not in str(e):
            raise
    return kdir


def api_auth_status(root):
    """Which purposes currently require api tokens. {} == all off."""
    _vault._ensure_dirs(root)
    doc = _vault._load_users(root)
    cfg = doc.get("api_auth") or {}
    return {p: bool(v) for p, v in cfg.items() if p in PURPOSES}


def set_api_auth(root, purpose, required):
    """Operator switch: require (or stop requiring) api tokens for one
    ingestion purpose on this vault root."""
    if purpose not in PURPOSES:
        raise ValueError("refused: unknown purpose %r (want one of %s)" % (
            purpose, ", ".join(PURPOSES)))
    _vault._ensure_dirs(root)
    doc = _vault._load_users(root)
    cfg = doc.get("api_auth") or {}
    cfg[purpose] = bool(required)
    doc["api_auth"] = cfg
    _vault._save_users(root, doc)
    _vault._activity(root, "system", "api-auth",
                     "purpose=%s required=%s" % (purpose, bool(required)))
    return "api tokens %s for purpose %s" % (
        "required" if required else "not required", purpose)


def mint_api_token(root, user, purpose, ttl=DEFAULT_TTL):
    """Mint a scoped bearer token for `user`'s vault + `purpose`.

    Returns the token string (a bearer credential — the caller decides
    where it goes). Refuses: unknown user, unknown purpose, child
    account (integrations are the adult-only surface), bad ttl.
    """
    if purpose not in PURPOSES:
        raise ValueError("refused: unknown purpose %r (want one of %s)" % (
            purpose, ", ".join(PURPOSES)))
    if not isinstance(ttl, int) or isinstance(ttl, bool) or \
            not (MIN_TTL <= ttl <= _tokens.MAX_TTL):
        raise ValueError("refused: ttl must be an integer %d..%d seconds" % (
            MIN_TTL, _tokens.MAX_TTL))
    _vault._ensure_dirs(root)
    users = _vault._load_users(root)
    rec = _vault._require_user(users, user)
    if rec.get("tier") != "adult":
        raise ValueError("refused: api tokens are an adult-only surface "
                         "(%s is a child account)" % user)
    kdir = _ensure_keyring(root)
    try:
        token = _tokens.mint(d=kdir, iss=ISS, aud=AUD, sub=user,
                             purpose=purpose, ttl=ttl)
    except _tokens.TokenError as e:
        raise ValueError("refused: token mint failed (%s)" % e)
    # verify once, immediately, to log the jti (metadata) — the token
    # itself never touches the activity log.
    claims = _tokens.verify(d=kdir, token=token, aud=AUD)
    _vault._activity(root, "system", "mint-api-token",
                     "user=%s purpose=%s ttl=%d jti=%s" % (
                         user, purpose, ttl, claims["jti"]))
    return token


def verify_api_token(root, user, purpose, token):
    """Fail-closed verification of a presented token. Returns the claims
    dict. Refuses: uninitialized keyring, bad envelope, HMAC mismatch,
    wrong audience, wrong issuer, expired/future/revoked, token minted
    for a different user or purpose, or an integration that has since
    been removed (revoking the integration kills its tokens implicitly).
    """
    if purpose not in PURPOSES:
        raise ValueError("refused: unknown purpose %r" % (purpose,))
    _vault._ensure_dirs(root)
    kdir = _keyring_dir(root)
    users = _vault._load_users(root)
    rec = _vault._require_user(users, user)
    try:
        claims = _tokens.verify(d=kdir, token=token, aud=AUD)
    except _tokens.TokenError as e:
        raise ValueError("refused: api token invalid (%s)" % e)
    if claims.get("iss") != ISS:
        raise ValueError("refused: api token issued by %r, not %r" % (
            claims.get("iss"), ISS))
    if claims.get("sub") != user:
        raise ValueError("refused: api token is for user %r, not %r" % (
            claims.get("sub"), user))
    if claims.get("purpose") != purpose:
        raise ValueError("refused: api token purpose is %r, not %r" % (
            claims.get("purpose"), purpose))
    if purpose not in rec.get("integrations", []):
        raise ValueError("refused: %r integration no longer registered "
                         "for %s — token void" % (purpose, user))
    return claims


def revoke_api_token(root, jti_or_token, reason=""):
    """Revoke a token by jti or full token. Survives expiry (a leaked
    token stays dead)."""
    _vault._ensure_dirs(root)
    kdir = _keyring_dir(root)
    try:
        r = _tokens.revoke(d=kdir, jti_or_token=jti_or_token, reason=reason)
    except _tokens.TokenError as e:
        raise ValueError("refused: cannot revoke api token (%s)" % e)
    _vault._activity(root, "system", "revoke-api-token",
                     "jti=%s reason=%s" % (r["jti"], reason or "-"))
    return r


def enforce(root, user, purpose, api_token):
    """Trust-boundary hook for the ingest() functions. When the purpose
    is token-required for this vault root, `api_token` must be present
    and must verify — otherwise ingestion is refused. Returns the token
    claims on success, None when auth is not required (old behavior)."""
    if purpose not in PURPOSES:
        raise ValueError("refused: unknown purpose %r" % (purpose,))
    if not api_auth_status(root).get(purpose, False):
        return None
    if not api_token:
        raise ValueError("refused: purpose %r requires an api token "
                         "(mint one with: vault.py mint-api-token %s "
                         "--purpose %s)" % (purpose, user, purpose))
    return verify_api_token(root, user, purpose, api_token)
