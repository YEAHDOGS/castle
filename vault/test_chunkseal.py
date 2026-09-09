#!/usr/bin/env python3
"""Regression tests for vault/chunkseal.py — chunked encrypted backup
container.

Fixture-based, temp dirs only. Covers: round-trip seal/open, wrong
passphrase failing clean with a typed error (no traceback), tamper
detection on a flipped byte (names the chunk), KDF params stored in
the header, malformed files refused, and the CLI failing clean.
"""

import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import chunkseal  # noqa: E402
import vault_lock  # noqa: E402


def fresh_dir(tc, prefix):
    tmp = tempfile.mkdtemp(prefix=prefix)
    tc.addCleanup(shutil.rmtree, tmp, True)
    return tmp


def mk_secret(tc, text=b"test-passphrase"):
    fd, path = tempfile.mkstemp(prefix="castle-chunk-pw-")
    tc.addCleanup(os.unlink, path)
    os.write(fd, text)
    os.close(fd)
    os.chmod(path, 0o600)
    return ("passphrase", path)


def read_secret(tc, spec):
    kind, path = spec
    return vault_lock._read_secret(
        path if kind == "passphrase" else None,
        path if kind == "keyfile" else None)


class TestChunkSeal(unittest.TestCase):
    def seal_tmp(self, tc, payload, files=None, chunk_size=512):
        d = fresh_dir(tc, "castle-chunk-test-")
        src = os.path.join(d, "payload.bin")
        with open(src, "wb") as f:
            f.write(payload)
        out = os.path.join(d, "payload.castle")
        secret = read_secret(tc, mk_secret(tc))
        header = chunkseal.seal_file(src, out, secret, files=files,
                                    chunk_size=chunk_size)
        return d, out, header

    def test_roundtrip(self):
        tc, payload = self, os.urandom(5000)
        d, out, header = self.seal_tmp(tc, payload, chunk_size=700)
        self.assertEqual(header["format"], "castle-chunks/v1")
        self.assertEqual(header["chunk_count"], 8)  # ceil(5000/700)
        secret = read_secret(tc, mk_secret(tc))
        h2, pt = chunkseal.open_file(out, secret)
        self.assertEqual(pt, payload)
        self.assertEqual(h2["payload_sha256"], header["payload_sha256"])

    def test_empty_payload_roundtrip(self):
        tc = self
        d, out, header = self.seal_tmp(tc, b"", chunk_size=512)
        secret = read_secret(tc, mk_secret(tc))
        h2, pt = chunkseal.open_file(out, secret)
        self.assertEqual(pt, b"")

    def test_kdf_params_stored_in_header(self):
        tc = self
        _, _, header = self.seal_tmp(tc, b"data", chunk_size=512)
        params = header["kdf"]
        self.assertIn(params["kdf"], ("scrypt", "pbkdf2-sha256",
                                      "raw-keyfile"))
        if params["kdf"] == "scrypt":
            self.assertEqual(
                (params["n"], params["r"], params["p"]), (2 ** 15, 8, 1))
        elif params["kdf"] == "pbkdf2-sha256":
            self.assertEqual(params["iterations"], 600_000)
        self.assertEqual(len(bytes.fromhex(header["salt"])), 16)
        self.assertEqual(len(bytes.fromhex(header["nonce"])), 16)

    def test_wrong_passphrase_fails_clean(self):
        tc = self
        _, out, _ = self.seal_tmp(tc, b"secret data" * 100, chunk_size=64)
        wrong = read_secret(tc, mk_secret(tc, b"wrong-passphrase"))
        with self.assertRaises(chunkseal.ChunkSealError) as cm:
            chunkseal.open_file(out, wrong)
        self.assertIn("HMAC mismatch", str(cm.exception))

    def test_flipped_byte_detected_and_chunk_named(self):
        tc = self
        _, out, header = self.seal_tmp(tc, os.urandom(3000),
                                       chunk_size=512)
        with open(out, "rb") as _f:
            raw = bytearray(_f.read())
        # flip a byte in the body (past the header line)
        body_off = raw.index(b"\n") + 1
        raw[body_off + 4 + 1300] ^= 0x01
        bad = out + ".tampered"
        with open(bad, "wb") as f:
            f.write(bytes(raw))
        secret = read_secret(tc, mk_secret(tc))
        with self.assertRaises(chunkseal.ChunkSealError) as cm:
            chunkseal.open_file(bad, secret)
        # 1300 lands in chunk 2 (512-byte chunks) — the error names it
        self.assertIn("chunk 2", str(cm.exception))

    def test_truncated_body_refused(self):
        tc = self
        _, out, _ = self.seal_tmp(tc, os.urandom(2000), chunk_size=512)
        with open(out, "rb") as _f:
            raw = _f.read()
        bad = out + ".trunc"
        with open(bad, "wb") as f:
            f.write(raw[:len(raw) // 2])
        secret = read_secret(tc, mk_secret(tc))
        with self.assertRaises(chunkseal.ChunkSealError):
            chunkseal.open_file(bad, secret)

    def test_bad_header_refused(self):
        tc = self
        d = fresh_dir(tc, "castle-chunk-test-")
        bad = os.path.join(d, "junk.castle")
        with open(bad, "wb") as f:
            f.write(b"this is not a castle backup\n" + os.urandom(64))
        secret = read_secret(tc, mk_secret(tc))
        with self.assertRaises(chunkseal.ChunkSealError):
            chunkseal.open_file(bad, secret)

    def test_foreign_format_refused(self):
        tc = self
        _, out, header = self.seal_tmp(tc, b"data", chunk_size=512)
        header["format"] = "castle-vault/v1"  # the other lane's id
        header_raw = (json.dumps(header, sort_keys=True) + "\n"
                      ).encode("utf-8")
        with open(out, "rb") as _f:
            raw = _f.read()
        body = raw.split(b"\n", 1)[1]
        with open(out, "wb") as f:
            f.write(header_raw + body)
        secret = read_secret(tc, mk_secret(tc))
        with self.assertRaises(chunkseal.ChunkSealError):
            chunkseal.open_file(out, secret)

    def test_keyfile_mode_roundtrip(self):
        tc = self
        d = fresh_dir(tc, "castle-chunk-test-")
        fd, kf = tempfile.mkstemp(prefix="castle-chunk-kf-")
        tc.addCleanup(os.unlink, kf)
        key = os.urandom(32)
        os.write(fd, key)
        os.close(fd)
        os.chmod(kf, 0o600)
        src = os.path.join(d, "p.bin")
        with open(src, "wb") as f:
            f.write(b"keyfile secret" * 50)
        out = os.path.join(d, "p.castle")
        secret = vault_lock._read_secret(None, kf)
        header = chunkseal.seal_file(src, out, secret, chunk_size=128)
        self.assertEqual(header["kdf"]["kdf"], "raw-keyfile")
        h2, pt = chunkseal.open_file(out, vault_lock._read_secret(None, kf))
        self.assertEqual(pt, b"keyfile secret" * 50)

    def test_cli_verify_wrong_passphrase_no_traceback(self):
        tc = self
        _, out, _ = self.seal_tmp(tc, b"cli secret" * 100, chunk_size=64)
        good = mk_secret(tc)[1]
        bad = mk_secret(tc, b"nope")[1]
        p = subprocess.run(
            [sys.executable, os.path.join(_HERE, "chunkseal.py"),
             "verify", "--passphrase-file", bad, out],
            capture_output=True, text=True)
        self.assertEqual(p.returncode, 1)
        self.assertIn("refused:", p.stdout)
        self.assertNotIn("Traceback", p.stdout + p.stderr)
        p = subprocess.run(
            [sys.executable, os.path.join(_HERE, "chunkseal.py"),
             "verify", "--passphrase-file", good, out],
            capture_output=True, text=True)
        self.assertEqual(p.returncode, 0)
        self.assertIn("verified", p.stdout)

    def test_seal_output_is_0600(self):
        tc = self
        _, out, _ = self.seal_tmp(tc, b"perms", chunk_size=512)
        import stat
        self.assertEqual(stat.S_IMODE(os.stat(out).st_mode), 0o600)


if __name__ == "__main__":
    unittest.main(verbosity=2)
