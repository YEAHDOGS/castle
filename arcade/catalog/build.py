#!/usr/bin/env python3
"""Castle Arcade static catalog generator.

Zero-dependency (Python stdlib only). Reads catalog/games.json, validates it
against a small schema (fails loudly on bad data), and renders static HTML:

    python3 build.py                      # builds into site/
    python3 build.py --out /tmp/site     # custom output dir

Pages are plain, offline-friendly, no external hosts, no cookies, no tracking.
A post-build link check verifies every internal href resolves.
"""
import argparse
import html
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
GAMES_JSON = HERE / "games.json"

STATUS_OK = {"available", "coming-soon"}
GAME_FIELDS = {"id", "title", "platform", "type", "status", "genre",
               "description", "versions", "cover_art"}
VERSION_FIELDS = {"label", "file", "hash", "hash_algorithm", "region",
                  "release_date", "notes"}


def fail(msg):
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def load_games():
    try:
        data = json.loads(GAMES_JSON.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"catalog data not found: {GAMES_JSON}")
    except json.JSONDecodeError as e:
        fail(f"games.json is not valid JSON: {e}")
    if not isinstance(data, dict) or not isinstance(data.get("games"), list):
        fail("games.json must be an object with a 'games' array")
    return data["games"]


def validate(games):
    seen = set()
    for i, g in enumerate(games):
        where = f"games[{i}]"
        if not isinstance(g, dict):
            fail(f"{where}: entry must be an object")
        unknown = set(g) - GAME_FIELDS
        if unknown:
            fail(f"{where}: unknown field(s): {sorted(unknown)}")
        for field in ("id", "title", "platform", "type", "status"):
            if field not in g or not isinstance(g[field], str) or not g[field].strip():
                fail(f"{where}: '{field}' is required and must be a non-empty string")
        if g["id"] in seen:
            fail(f"{where}: duplicate id '{g['id']}'")
        seen.add(g["id"])
        if not re.fullmatch(r"[a-z0-9-]+", g["id"]):
            fail(f"{where}: id '{g['id']}' must be lowercase alnum + dashes")
        if g["status"] not in STATUS_OK:
            fail(f"{where}: status must be one of {sorted(STATUS_OK)}")
        if "description" in g and not isinstance(g["description"], str):
            fail(f"{where}: 'description' must be a string")
        if "versions" in g and not isinstance(g["versions"], list):
            fail(f"{where}: 'versions' must be an array")
        for j, v in enumerate(g.get("versions", [])):
            vwhere = f"{where}.versions[{j}]"
            if not isinstance(v, dict):
                fail(f"{vwhere}: version must be an object")
            unknown_v = set(v) - VERSION_FIELDS
            if unknown_v:
                fail(f"{vwhere}: unknown field(s): {sorted(unknown_v)}")
            if not isinstance(v.get("label"), str) or not v["label"].strip():
                fail(f"{vwhere}: 'label' is required and must be a non-empty string")
            for field in ("file", "hash", "hash_algorithm", "region", "notes"):
                if field in v and v[field] is not None and not isinstance(v[field], str):
                    fail(f"{vwhere}: '{field}' must be a string or null")
        if g["status"] == "coming-soon" and any(
            v.get("file") or v.get("hash") for v in g.get("versions", [])
        ):
            fail(f"{where}: coming-soon entries must not carry download metadata")
        if g["status"] == "available" and not g.get("versions"):
            fail(f"{where}: available entries must list at least one version")
    if not games:
        fail("catalog has no games")


CSS = """*{box-sizing:border-box}body{margin:0;background:#0d0f14;color:#e8e6df;
font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif}
a{color:#8fd0ff}.wrap{max-width:960px;margin:0 auto;padding:24px}
header{display:flex;flex-wrap:wrap;gap:12px;align-items:baseline;justify-content:space-between}
h1{font-size:1.6rem;margin:.5rem 0}.filters{display:flex;gap:10px;margin:16px 0;flex-wrap:wrap}
select{background:#171a22;color:#e8e6df;border:1px solid #2c3140;border-radius:6px;padding:6px 10px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(240px,1fr));gap:14px}
.card{background:#151923;border:1px solid #262c3d;border-radius:10px;padding:14px;display:block;text-decoration:none;color:inherit}
.card:hover{border-color:#8fd0ff}.card h2{font-size:1.05rem;margin:0 0 6px}
.meta{font-size:.8rem;color:#9aa3b2}.badge{display:inline-block;font-size:.72rem;font-weight:700;
text-transform:uppercase;letter-spacing:.05em;border-radius:999px;padding:3px 10px;margin-top:8px}
.badge.available{background:#123f26;color:#7ce8a8}.badge.coming-soon{background:#3d2f10;color:#ffd97c}
table{width:100%;border-collapse:collapse;margin-top:14px;font-size:.9rem}
th,td{text-align:left;padding:8px 10px;border-bottom:1px solid #262c3d;vertical-align:top}
th{color:#9aa3b2;font-weight:600}.mono{font-family:ui-monospace,Consolas,monospace;font-size:.82rem;word-break:break-all}
.crumbs{font-size:.85rem;color:#9aa3b2;margin-bottom:8px}footer{margin-top:32px;font-size:.78rem;color:#6b7280}
.note{background:#151923;border:1px solid #262c3d;border-radius:8px;padding:12px 14px;font-size:.9rem;margin-top:16px}
"""


def page_shell(title, body, crumbs=""):
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{html.escape(title)} — Castle Arcade Catalog</title>
<style>{CSS}</style>
</head>
<body>
<div class="wrap">
{crumbs}
{body}
<footer>Castle Arcade catalog — a shelf view of the local library. No downloads, no tracking, no cookies.</footer>
</div>
</body>
</html>
"""


def card_html(g):
    badge = (f'<span class="badge {g["status"]}">{g["status"].replace("-", " ")}</span>')
    desc = html.escape((g.get("description") or "")[:140])
    return (f'<a class="card" href="game/{g["id"]}.html" '
            f'data-platform="{html.escape(g["platform"])}" data-status="{g["status"]}">'
            f'<h2>{html.escape(g["title"])}</h2>'
            f'<div class="meta">{html.escape(g["platform"])} &middot; {html.escape(g.get("type", ""))}</div>'
            f'<div class="meta" style="margin-top:6px">{desc}</div>{badge}</a>')


def render_index(games):
    platforms = sorted({g["platform"] for g in games})
    opts = "".join(f'<option value="{html.escape(p)}">{html.escape(p)}</option>'
                   for p in platforms)
    cards = "\n".join(card_html(g) for g in games)
    body = f"""<header><h1>Castle Arcade Catalog</h1></header>
<div class="filters">
<label>Platform <select id="f-platform"><option value="">All</option>{opts}</select></label>
<label>Status <select id="f-status"><option value="">All</option>
<option value="available">Available</option>
<option value="coming-soon">Coming soon</option></select></label>
</div>
<div class="grid" id="grid">{cards}</div>
<script>
const gp=document.getElementById('f-platform'),gs=document.getElementById('f-status');
function apply(){{document.querySelectorAll('.card').forEach(c=>{{
c.style.display=(!gp.value||c.dataset.platform===gp.value)&&(!gs.value||c.dataset.status===gs.value)?'':'none';}});}}
gp.addEventListener('change',apply);gs.addEventListener('change',apply);
</script>"""
    return page_shell("Catalog", body)


def render_game(g):
    crumbs = '<div class="crumbs"><a href="../index.html">&larr; Catalog</a></div>'
    badge = f'<span class="badge {g["status"]}">{g["status"].replace("-", " ")}</span>'
    desc = html.escape(g.get("description") or "")
    meta = (f'<div class="meta">{html.escape(g["platform"])} &middot; '
            f'{html.escape(g.get("type", ""))} &middot; {html.escape(g.get("genre", ""))}</div>')
    body = (f'<header><h1>{html.escape(g["title"])}</h1>{badge}</header>{meta}'
            f'<div class="note">{desc}</div>')
    if g["status"] == "coming-soon":
        body += ('<div class="note"><strong>Coming soon.</strong> '
                 'This slot is planned — nothing is listed or linked yet.</div>')
    rows = []
    for v in g.get("versions", []):
        h = html.escape(v.get("hash_algorithm", "") + ": " + v.get("hash", "")
                        if v.get("hash") else "—")
        rows.append("<tr>"
                    f"<td><strong>{html.escape(v['label'])}</strong></td>"
                    f"<td class=\"mono\">{html.escape(v.get('file') or '—')}</td>"
                    f"<td class=\"mono\">{h}</td>"
                    f"<td>{html.escape(v.get('region') or '—')}</td>"
                    f"<td>{html.escape(v.get('release_date') or '—')}</td>"
                    f"<td>{html.escape(v.get('notes') or '—')}</td></tr>")
    if rows:
        body += ("<h2>Versions</h2>"
                 "<table><thead><tr><th>Version</th><th>File</th><th>Hash</th>"
                 "<th>Region</th><th>Release</th><th>Notes</th></tr></thead>"
                 f"<tbody>{''.join(rows)}</tbody></table>")
    return page_shell(g["title"], body, crumbs)


def check_links(outdir):
    href_re = re.compile(r'href="([^"#]+?)"')
    problems = []
    pages = sorted(outdir.rglob("*.html"))
    if not pages:
        fail(f"no pages generated in {outdir}")
    for page in pages:
        for href in href_re.findall(page.read_text(encoding="utf-8")):
            if href.startswith(("http://", "https://", "mailto:")):
                problems.append(f"{page.name}: external href '{href}' (forbidden)")
                continue
            target = (page.parent / href).resolve()
            if not str(target).startswith(str(outdir.resolve())) or not target.is_file():
                problems.append(f"{page.name}: broken link '{href}'")
    if problems:
        fail("link check failed:\n  " + "\n  ".join(problems))
    return len(pages)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="site")
    args = ap.parse_args()
    out = Path(args.out)
    games = load_games()
    validate(games)
    gamedir = out / "game"
    gamedir.mkdir(parents=True, exist_ok=True)
    (out / "index.html").write_text(render_index(games), encoding="utf-8")
    for g in games:
        (gamedir / f"{g['id']}.html").write_text(render_game(g), encoding="utf-8")
    n = check_links(out)
    print(f"OK: validated {len(games)} games, wrote {n} pages to {out}/, all internal links resolve.")


if __name__ == "__main__":
    main()
