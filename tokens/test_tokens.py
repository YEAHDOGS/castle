#!/usr/bin/env python3
"""Fixture-based regression tests for the DOGS token authority.

Temp dirs only, fake key material throughout. Nothing here touches a real
keyring.
"""

import contextlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import tokens  # noqa: E402  (module under test is tokens/tokens.py)


@contextlib.contextmanager
def frozen_time(t):
    real = time.time
    time.time = lambda: t
    try:
        yield
    finally:
        time.time = real


def _flip(s):
    """Flip one char, staying inside the b64u alphabet."""
    c = s[-1]
    return s[:-1] + ("B" if c != "B" else "A")


class TokenTest(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="castle-tokens-test-")
        self.r = tokens.init_keyring(self.root)
        self.kid = self.r["kid"]
        self.base = dict(iss="dogs-id", aud="weed.wearedogs.net",
                         sub="visitor", purpose="age-verified-21", ttl=600)

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def mint(self, **kw):
        args = dict(self.base)
        args.update(kw)
        return tokens.mint(self.root, **args)

    # -- core roundtrip -------------------------------------------------

    def test_roundtrip(self):
        tok = self.mint()
        claims = tokens.verify(self.root, tok, aud="weed.wearedogs.net")
        self.assertEqual(claims["iss"], "dogs-id")
        self.assertEqual(claims["aud"], "weed.wearedogs.net")
        self.assertEqual(claims["sub"], "visitor")
        self.assertEqual(claims["purpose"], "age-verified-21")
        self.assertEqual(claims["exp"] - claims["iat"], 600)
        self.assertRegex(claims["jti"], r"^[0-9a-f]{32}$")

    def test_ciphertext_is_not_plaintext(self):
        tok = self.mint(sub="super-secret-subject")
        parts = tok.split(".")
        self.assertEqual(len(parts), 4)
        self.assertEqual(parts[0], "dogs1")
        blob = tokens._b64u_decode(parts[2])
        self.assertNotIn(b"super-secret-subject", blob)
        self.assertNotIn(b"weed.wearedogs.net", blob)

    # -- tamper: fail closed --------------------------------------------

    def test_tamper_ciphertext_refused(self):
        tok = self.mint()
        p = tok.split(".")
        bad = ".".join([p[0], p[1], _flip(p[2]), p[3]])
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, bad, aud="weed.wearedogs.net")

    def test_tamper_tag_refused(self):
        tok = self.mint()
        p = tok.split(".")
        bad = ".".join([p[0], p[1], p[2], _flip(p[3])])
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, bad, aud="weed.wearedogs.net")

    def test_tamper_header_refused(self):
        # header is MACed too — flipping it breaks the tag
        tok = self.mint()
        p = tok.split(".")
        bad = ".".join([p[0], _flip(p[1]), p[2], p[3]])
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, bad, aud="weed.wearedogs.net")

    # -- expiry / skew ---------------------------------------------------

    def test_expired_refused(self):
        t0 = int(time.time())
        with frozen_time(t0):
            tok = self.mint(ttl=600)
        with frozen_time(t0 + 1000):  # exp+skew = t0+900
            with self.assertRaises(tokens.TokenError):
                tokens.verify(self.root, tok, aud="weed.wearedogs.net")

    def test_leeway_accepts_recently_expired(self):
        t0 = int(time.time())
        with frozen_time(t0):
            tok = self.mint(ttl=600)
        with frozen_time(t0 + 700):  # past exp, inside 300s skew
            claims = tokens.verify(self.root, tok, aud="weed.wearedogs.net")
        self.assertEqual(claims["aud"], "weed.wearedogs.net")

    def test_future_iat_refused(self):
        t0 = int(time.time())
        with frozen_time(t0 + 1000):
            tok = self.mint(ttl=600)
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, tok, aud="weed.wearedogs.net")

    # -- audience binding ------------------------------------------------

    def test_wrong_audience_refused(self):
        tok = self.mint(aud="weed.wearedogs.net")
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, tok, aud="evil.example.com")

    # -- kid handling ----------------------------------------------------

    def _reheader(self, tok, **over):
        p = tok.split(".")
        header = json.loads(tokens._b64u_decode(p[1]).decode("utf-8"))
        header.update(over)
        h = tokens._b64u_encode(json.dumps(
            header, sort_keys=True, separators=(",", ":")).encode("utf-8"))
        return ".".join([p[0], h, p[2], p[3]])

    def test_unknown_kid_refused(self):
        tok = self.mint()
        bad = self._reheader(tok, kid="kdeadbeef1234")
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, bad, aud="weed.wearedogs.net")

    def test_kid_path_traversal_refused(self):
        tok = self.mint()
        bad = self._reheader(tok, kid="../../../etc")
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, bad, aud="weed.wearedogs.net")
        bad2 = self._reheader(tok, kid="/abs/path")
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, bad2, aud="weed.wearedogs.net")

    def test_malformed_tokens_refused(self):
        bads = ["", "dogs1", "dogs1.a.b", "dogs1.a.b.c.d.e",
                "jwt1." + ".".join(self.mint().split(".")[1:]),
                "dogs1.!!!.b.c", "dogs1." + "A" * 20000]
        for b in bads:
            with self.assertRaises(tokens.TokenError, msg=b[:20]):
                tokens.verify(self.root, b, aud="weed.wearedogs.net")

    # -- rotation --------------------------------------------------------

    def test_rotation_old_verifies_new_mints(self):
        t1 = self.mint()
        r = tokens.rotate_keys(self.root)
        new_kid = r["kid"]
        self.assertNotEqual(new_kid, self.kid)
        t2 = self.mint()
        # old token still verifies under retired kid
        c1 = tokens.verify(self.root, t1, aud="weed.wearedogs.net")
        self.assertEqual(c1["aud"], "weed.wearedogs.net")
        # new mint uses the new kid
        p = t2.split(".")
        header = json.loads(tokens._b64u_decode(p[1]).decode("utf-8"))
        self.assertEqual(header["kid"], new_kid)
        tokens.verify(self.root, t2, aud="weed.wearedogs.net")
        # metadata: old retired, new active
        by_kid = {k["kid"]: k for k in tokens.list_keys(self.root)}
        self.assertEqual(by_kid[self.kid]["status"], "retired")
        self.assertEqual(by_kid[new_kid]["status"], "active")

    # -- revocation ------------------------------------------------------

    def test_revoke_by_jti(self):
        tok = self.mint()
        claims = tokens.verify(self.root, tok, aud="weed.wearedogs.net")
        tokens.revoke(self.root, claims["jti"], reason="test leak")
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, tok, aud="weed.wearedogs.net")

    def test_revoke_by_token(self):
        tok = self.mint()
        tokens.verify(self.root, tok, aud="weed.wearedogs.net")
        tokens.revoke(self.root, tok, reason="full token revoke")
        with self.assertRaises(tokens.TokenError):
            tokens.verify(self.root, tok, aud="weed.wearedogs.net")

    def test_prune_revoked(self):
        t0 = int(time.time())
        with frozen_time(t0):
            tok = self.mint(ttl=600)
        tokens.revoke(self.root, tok)            # has exp -> prunable
        tokens.revoke(self.root, "ab" * 16)      # bare jti -> kept forever
        with frozen_time(t0 + 5000):
            r = tokens.prune_revoked(self.root)
        self.assertEqual(r["dropped"], 1)
        self.assertEqual(r["kept"], 1)
        # the pruned token verifies again (no longer revoked)
        with frozen_time(t0 + 100):
            tokens.verify(self.root, tok, aud="weed.wearedogs.net")

    # -- key hygiene -----------------------------------------------------

    def test_list_keys_never_leaks_bytes(self):
        keyfile = os.path.join(self.root, "keys", self.kid + ".key")
        with open(keyfile, "rb") as f:
            raw_hex = f.read().hex()
        out = json.dumps(tokens.list_keys(self.root))
        self.assertIn(self.kid, out)
        self.assertNotIn(raw_hex, out)

    def test_keyfile_permissions(self):
        keyfile = os.path.join(self.root, "keys", self.kid + ".key")
        self.assertEqual(os.stat(keyfile).st_mode & 0o777, 0o600)
        with open(keyfile, "rb") as f:
            self.assertEqual(len(f.read()), 32)

    # -- input validation ------------------------------------------------

    def test_mint_input_validation(self):
        with self.assertRaises(tokens.TokenError):
            self.mint(aud="")
        with self.assertRaises(tokens.TokenError):
            self.mint(ttl=0)
        with self.assertRaises(tokens.TokenError):
            self.mint(ttl=99999999)
        with self.assertRaises(tokens.TokenError):
            self.mint(purpose="bad\npurpose")

    def test_double_init_refused(self):
        with self.assertRaises(tokens.TokenError):
            tokens.init_keyring(self.root)

    # -- CLI roundtrip ---------------------------------------------------

    def test_cli_roundtrip(self):
        cli_root = tempfile.mkdtemp(prefix="castle-tokens-cli-")
        self.addCleanup(shutil.rmtree, cli_root, True)
        r = self._cli_init(cli_root)
        self.assertEqual(r.returncode, 0, r.stderr)
        r = self._cli2(cli_root, "mint", "--iss", "dogs-id",
                       "--aud", "weed.wearedogs.net", "--sub", "visitor",
                       "--purpose", "age-verified-21", "--ttl", "600")
        self.assertEqual(r.returncode, 0, r.stderr)
        tok = r.stdout.strip()
        self.assertTrue(tok.startswith("dogs1."))
        r = self._cli2(cli_root, "verify", tok,
                       "--aud", "weed.wearedogs.net")
        self.assertEqual(r.returncode, 0, r.stderr)
        claims = json.loads(r.stdout)
        self.assertEqual(claims["purpose"], "age-verified-21")
        r = self._cli2(cli_root, "verify", tok, "--aud", "evil.example.com")
        self.assertNotEqual(r.returncode, 0)
        r = self._cli2(cli_root, "revoke", tok, "--reason", "cli test")
        self.assertEqual(r.returncode, 0, r.stderr)
        r = self._cli2(cli_root, "verify", tok,
                       "--aud", "weed.wearedogs.net")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("revoked", r.stderr)
        r = self._cli2(cli_root, "list-keys")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn("error", r.stderr)

    def _cli_init(self, cli_root):
        return subprocess.run(
            [sys.executable, os.path.join(_HERE, "tokens.py"),
             "--dir", cli_root, "init"],
            capture_output=True, text=True)

    def _cli2(self, cli_root, *args):
        return subprocess.run(
            [sys.executable, os.path.join(_HERE, "tokens.py"),
             "--dir", cli_root] + list(args),
            capture_output=True, text=True)


if __name__ == "__main__":
    unittest.main(verbosity=2)
