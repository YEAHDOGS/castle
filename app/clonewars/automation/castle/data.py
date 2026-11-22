"""What a Castle script does to the rows it read: keep, measure, divide, sort and sum them up.

Rows are dicts, one per item of a list, keyed by field name. Every function returns new rows
and leaves the old ones alone. Text that holds a number ("Price $31.97") counts as that number.
"""

import builtins
import re
import statistics

_MONEY = re.compile(r"[$€£]\s*(\d[\d,]*(?:\.\d+)?)")
_NUMBER = re.compile(r"-?\d[\d,]*(?:\.\d+)?")

# What each unit is in its kind's base unit: milliliters, grams, or items.
_UNITS = {
    "quart": ("volume", 946.352946), "gallon": ("volume", 3785.411784), "liter": ("volume", 1000.0),
    "milliliter": ("volume", 1.0), "fluid ounce": ("volume", 29.5735295625),
    "pound": ("weight", 453.59237), "ounce": ("weight", 28.349523125), "gram": ("weight", 1.0),
    "kilogram": ("weight", 1000.0), "item": ("count", 1.0),
}
# How units are written in listings, longest spellings first so "fl oz" wins over "oz".
_SPELLINGS = [
    (r"fl\.?\s*oz\.?|fluid\s+ounces?", "fluid ounce"),
    (r"quarts?|qts?\.?|qt\.?", "quart"),
    (r"gallons?|gal\.?", "gallon"),
    (r"milliliters?|millilitres?|ml", "milliliter"),
    (r"liters?|litres?|ltrs?|l", "liter"),
    (r"pounds?|lbs?\.?", "pound"),
    (r"ounces?|oz\.?", "ounce"),
    (r"kilograms?|kgs?", "kilogram"),
    (r"grams?|g", "gram"),
    (r"count|ct|pieces?|pcs", "item"),
]
_AMOUNT = re.compile(
    r"(?<![\w.])(\d+(?:\.\d+)?)\s*-?\s*(" + "|".join(s for s, _ in _SPELLINGS) + r")(?![a-z])",
    re.IGNORECASE,
)
_PACK = re.compile(r"(?:\b(\d+)\s*-?\s*(?:pack|pk)\b|\bpack\s+of\s+(\d+)\b|\b(\d+)\s*[x×]\s*(?=\d))", re.IGNORECASE)


def number(value):
    """The number in a value: 6.39 for "$6.39/qt", None when there is none."""
    if value is None or isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value) if value == value else None
    text = str(value)
    m = _MONEY.search(text) or _NUMBER.search(text)
    if not m:
        return None
    try:
        return float((m.group(1) if m.lastindex else m.group(0)).replace(",", ""))
    except ValueError:
        return None


def _unit_of(spelled):
    for pattern, unit in _SPELLINGS:
        if re.fullmatch(pattern, spelled, re.IGNORECASE):
            return unit
    return None


def amount_in(text, unit):
    """How much of `unit` a listing's text says it holds: "Motor Oil, 5 Quarts" is 5 quarts,
    "6 Pack 1 qt" is 6, "1 Gallon" is about 4. None when it does not say."""
    if text is None or unit not in _UNITS:
        return None
    kind, size = _UNITS[unit]
    text = str(text)
    for m in _AMOUNT.finditer(text):
        found = _unit_of(m.group(2))
        if found == "ounce" and kind == "volume":
            found = "fluid ounce"  # a bottle's "oz" is fluid ounces
        if found is None or _UNITS[found][0] != kind:
            continue
        qty = float(m.group(1)) * _UNITS[found][1] / size
        pack = _PACK.search(text)
        if pack:
            count = next(int(g) for g in pack.groups() if g)
            if 1 < count <= 100:
                qty *= count
        return round(qty, 4)
    return None


def keep(rows, field, op, arg=None):
    return [r for r in rows if _test(r.get(field), op, arg)]


def drop(rows, field, op, arg=None):
    return [r for r in rows if not _test(r.get(field), op, arg)]


def _test(value, op, arg):
    if op == "has":
        return value is not None and str(arg).lower() in str(value).lower()
    if op == "matches":
        return value is not None and re.search(arg, str(value), re.IGNORECASE) is not None
    if op == "empty":
        return value is None or str(value).strip() == ""
    if op == "not empty":
        return not (value is None or str(value).strip() == "")
    n = number(value)
    if op == "below":
        return n is not None and n < float(arg)
    if op == "above":
        return n is not None and n > float(arg)
    raise ValueError(f"unknown test {op}")


def unique(rows, field):
    seen = set()
    out = []
    for r in rows:
        key = r.get(field)
        if key is not None and key in seen:
            continue
        seen.add(key)
        out.append(r)
    return out


def measure(rows, field, unit, name):
    return [dict(r, **{name: amount_in(r.get(field), unit)}) for r in rows]


def _other(row, other):
    return number(row.get(other)) if isinstance(other, str) else float(other)


def divide(rows, field, other, name):
    def one(r):
        a, b = number(r.get(field)), _other(r, other)
        return None if a is None or not b else round(a / b, 4)
    return [dict(r, **{name: one(r)}) for r in rows]


def multiply(rows, field, other, name):
    def one(r):
        a, b = number(r.get(field)), _other(r, other)
        return None if a is None or b is None else round(a * b, 4)
    return [dict(r, **{name: one(r)}) for r in rows]


def sort(rows, field, descending=False):
    known = [r for r in rows if number(r.get(field)) is not None]
    unknown = [r for r in rows if number(r.get(field)) is None]
    return builtins.sorted(known, key=lambda r: number(r.get(field)), reverse=descending) + unknown


def _numbers(rows, field):
    return [n for n in (number(r.get(field)) for r in rows) if n is not None]


def average(rows, field):
    ns = _numbers(rows, field)
    return round(builtins.sum(ns) / len(ns), 4) if ns else None


def sum(rows, field):  # noqa: A001 - the script's own word
    return round(builtins.sum(_numbers(rows, field)), 4)


def min(rows, field):  # noqa: A001
    ns = _numbers(rows, field)
    return builtins.min(ns) if ns else None


def max(rows, field):  # noqa: A001
    ns = _numbers(rows, field)
    return builtins.max(ns) if ns else None


def median(rows, field):
    ns = _numbers(rows, field)
    return statistics.median(ns) if ns else None


def lowest(rows, field):
    known = [r for r in rows if number(r.get(field)) is not None]
    return dict(builtins.min(known, key=lambda r: number(r.get(field)))) if known else None


def highest(rows, field):
    known = [r for r in rows if number(r.get(field)) is not None]
    return dict(builtins.max(known, key=lambda r: number(r.get(field)))) if known else None
