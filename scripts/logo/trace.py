"""Centerline-trace a pen sketch into an SVG of stroked polylines.

usage: python trace.py <in.jpg> <out.svg> <preview.png>
"""
import sys, json
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageOps
from skimage.morphology import skeletonize, remove_small_objects
from skimage.filters import threshold_otsu
from scipy_free_rdp import rdp  # local helper below

src, out_svg, out_png = sys.argv[1], sys.argv[2], sys.argv[3]
im = Image.open(src)
print("size", im.size, "mode", im.mode, "exif keys", list(im.getexif().keys()), "info", {k: v for k, v in im.info.items() if k != "exif"})
g = ImageOps.autocontrast(im.convert("L"))
a = np.asarray(g).astype(np.float32)
t = threshold_otsu(a)
ink = a < t
ink = remove_small_objects(ink, 12)
# line width estimate: ink area / skeleton length
sk = skeletonize(ink)
width = ink.sum() / max(sk.sum(), 1)
print("otsu", t, "ink px", int(ink.sum()), "skeleton px", int(sk.sum()), "est width", round(float(width), 2))

H, W = sk.shape
pts = set(zip(*np.nonzero(sk)))
N8 = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]

def nbrs(p):
    y, x = p
    return [(y + dy, x + dx) for dy, dx in N8 if (y + dy, x + dx) in pts]

deg = {p: len(nbrs(p)) for p in pts}
nodes = {p for p in pts if deg[p] != 2}
visited_edges = set()
paths = []

def walk(start, nxt):
    path = [start, nxt]
    prev, cur = start, nxt
    while cur not in nodes:
        cand = [n for n in nbrs(cur) if n != prev and n not in path[-3:]]
        if not cand:
            break
        prev, cur = cur, cand[0]
        path.append(cur)
        if len(path) > 100000:
            break
    return path

for n in nodes:
    for m in nbrs(n):
        key = frozenset((n, m))
        if key in visited_edges:
            continue
        p = walk(n, m)
        for i in range(len(p) - 1):
            visited_edges.add(frozenset((p[i], p[i + 1])))
        paths.append(p)
# pure loops (no nodes)
seen = set(q for p in paths for q in p)
for p in pts - seen:
    if p in seen:
        continue
    loop = [p]
    prev, cur = None, p
    while True:
        cand = [n for n in nbrs(cur) if n != prev]
        if not cand:
            break
        prev, cur = cur, cand[0]
        if cur == p:
            loop.append(cur)
            break
        loop.append(cur)
        if len(loop) > 100000:
            break
    seen.update(loop)
    paths.append(loop)

paths = [p for p in paths if len(p) >= 4]
EPS = float(sys.argv[4]) if len(sys.argv) > 4 else 0.75
simp = [rdp([(int(x), int(y)) for y, x in p], EPS) for p in paths]
total = sum(len(p) for p in simp)
print("paths", len(simp), "points", total)
# the long strokes, for reading the structure
def plen(p):
    return sum(((p[i][0]-p[i-1][0])**2 + (p[i][1]-p[i-1][1])**2) ** 0.5 for i in range(1, len(p)))
for p in sorted(simp, key=plen, reverse=True)[:40]:
    if plen(p) < 18:
        break
    print(f"len {plen(p):5.0f} n {len(p):3d}: " + " ".join(f"({x:.0f},{y:.0f})" for x, y in (p if len(p) <= 8 else [p[0], p[len(p)//4], p[len(p)//2], p[3*len(p)//4], p[-1]])))

sw = max(1.6, round(float(width), 1))
parts = []
for p in simp:
    d = "M" + " L".join(f"{x:.1f} {y:.1f}" for x, y in p)
    parts.append(d)
svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}">\n'
       f'<g fill="none" stroke="#000" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round">\n'
       + "\n".join(f'<path d="{d}"/>' for d in parts) + "\n</g>\n</svg>\n")
open(out_svg, "w", encoding="utf-8").write(svg)

pv = Image.new("RGB", (W, H), "white")
dr = ImageDraw.Draw(pv)
for p in simp:
    dr.line(p, fill=(0, 0, 0), width=int(round(sw)), joint="curve")
pv.save(out_png)
json.dump({"w": W, "h": H, "stroke": sw, "paths": simp}, open(out_svg + ".json", "w"))
