#!/usr/bin/env python3
"""Receipt search — FAMILY-DATA-VAULT.md: "Every receipt is searchable."

Read-only metadata search over ONE user's own ``receipts/index.jsonl``.
Index entries carry metadata only (id, content_hash, merchant, total,
ingested_at, source) — search never opens receipt.json, original.eml,
raw.json, images, or attachments, and writes nothing (reads don't spam
the activity log).

The zero-implicit-trust rule applies unchanged: a user can search only
their OWN vault. There is no cross-user query — Sally cannot search
Sam's receipts, and no guardian/admin backdoor is added here.

Filters combine with AND:
  merchant    case-insensitive substring on the extracted merchant
  min_total / max_total   Decimal strings; a receipt with unknown total
                          (extraction said "none") never matches a total
                          filter — unknown is not zero
  since / until           "YYYY-MM-DD", compared against ingested_at
                          (receipts are filed when they land in the vault)
  source      exact match: "email", "pos-webhook", or "scan"
                          (email-forward entries predate the source field
                          and carry no source key — they normalize to
                          "email")

Stdlib only. No network, no new hosts.
"""

import json
import os
import re
import sys
from decimal import Decimal, InvalidOperation

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
import vault as _vault  # noqa: E402  (registry, perms plumbing)
import receipts as _receipts  # noqa: E402  (shared receipts/ layout)

DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

# Index entries written before the source field existed (early
# email-forward runs) carry no "source" key; they are email receipts.
LEGACY_EMAIL_SOURCE = "email"


def _normalize(entry):
    """Fill defaults so legacy index rows filter consistently."""
    e = dict(entry)
    e.setdefault("source", LEGACY_EMAIL_SOURCE)
    return e


def _parse_total(value, name):
    if isinstance(value, float):
        raise ValueError("invalid %s: %r — floats are refused, pass an "
                         "exact decimal string like \"12.34\" (money is "
                         "never a float)" % (name, value))
    try:
        d = Decimal(str(value))
    except (InvalidOperation, ValueError, TypeError):
        raise ValueError("invalid %s: %r (want a decimal string like "
                         "\"12.34\")" % (name, value))
    if d < 0:
        raise ValueError("invalid %s: %r (totals are non-negative)" %
                         (name, value))
    return d


def _parse_date(value, name):
    if not isinstance(value, str) or not DATE_RE.match(value):
        raise ValueError("invalid %s: %r (want YYYY-MM-DD)" % (name, value))
    return value


def _total_of(entry):
    """Decimal total of an index entry, or None when unknown."""
    t = entry.get("total")
    if t is None:
        return None
    try:
        return Decimal(str(t))
    except (InvalidOperation, ValueError, TypeError):
        return None


def search(root, user, *, merchant=None, min_total=None, max_total=None,
           since=None, until=None, source=None, limit=100):
    """Return index-entry dicts for `user`'s receipts matching ALL filters.

    Read-only: opens the user's index.jsonl and nothing else.
    Malformed index lines are skipped (ingestion wrote them, not us).
    """
    _vault._ensure_dirs(root)
    _vault._require_user(_vault._load_users(root), user)
    if not isinstance(limit, int) or isinstance(limit, bool) or limit < 1:
        raise ValueError("limit must be a positive integer")

    fmin = _parse_total(min_total, "min_total") if min_total is not None else None
    fmax = _parse_total(max_total, "max_total") if max_total is not None else None
    if fmin is not None and fmax is not None and fmin > fmax:
        raise ValueError("min_total > max_total")
    fsince = _parse_date(since, "since") if since is not None else None
    funtil = _parse_date(until, "until") if until is not None else None
    if fsince is not None and funtil is not None and fsince > funtil:
        raise ValueError("since > until")
    fmerchant = merchant.lower() if merchant is not None else None

    p = _receipts._index_path(root, user)
    out = []
    if os.path.exists(p):
        with open(p, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                except ValueError:
                    continue  # corrupt line: skip, never guess
                if not isinstance(entry, dict):
                    continue
                e = _normalize(entry)

                if fmerchant is not None:
                    m = e.get("merchant") or ""
                    if fmerchant not in str(m).lower():
                        continue
                if fmin is not None or fmax is not None:
                    t = _total_of(e)
                    if t is None:
                        continue  # unknown total is not zero
                    if fmin is not None and t < fmin:
                        continue
                    if fmax is not None and t > fmax:
                        continue
                if fsince is not None or funtil is not None:
                    day = str(e.get("ingested_at") or "")[:10]
                    if fsince is not None and day < fsince:
                        continue
                    if funtil is not None and day > funtil:
                        continue
                if source is not None and e["source"] != source:
                    continue

                out.append(e)
                if len(out) >= limit:
                    break
    return out


def search_json(root, user, **filters):
    """search() result as a JSON array string (metadata only)."""
    return json.dumps(search(root, user, **filters), sort_keys=True, indent=2)
