"""Write the two Castle marks into landing/: the traced sketch and the refined drawing.

usage: python gen_logo.py <trace.json> <landing dir> <preview.html>
Both share viewBox 0 0 300 420 so one can fade into the other in place.
"""
import json, math, sys

trace, landing, preview = sys.argv[1], sys.argv[2], sys.argv[3]
t = json.load(open(trace))
OX, OY = 8, 9  # the 284x402 scan sits inside the 300x420 box

# ---------------- the sketch, exactly as drawn ----------------
sk_paths = []
for p in t["paths"]:
    d = "M" + " L".join(f"{x + OX:.1f} {y + OY:.1f}" for x, y in p)
    sk_paths.append(f'<path d="{d}"/>')
sketch = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 420" role="img" aria-label="Castle, the original pen sketch">\n'
          '<!-- Traced from the original drawing, wobble and all. Centerline trace, no cleanup. -->\n'
          f'<g id="mark" fill="none" stroke="currentColor" stroke-width="{t["stroke"]}" stroke-linecap="round" stroke-linejoin="round">\n'
          + "\n".join(sk_paths) + "\n</g>\n</svg>\n")
open(f"{landing}/logo-sketch.svg", "w", encoding="utf-8").write(sketch)

# ---------------- the refined drawing ----------------
# Same bones as the sketch, on a 5-unit grid, true verticals, 45-degree roofs.
G = 380          # ground
L, R = 85, 125   # tower walls
APEX = (105, 85)
BLOCK_R = 175    # the block beside the spire
RIDGE = 240
P1, P2, V = 125, 205, 165   # gable peaks and the valley
EAVE = 280
HALL_R = 245
ARCH_R = 12
ARCH_C = (105, 312)
lines = [
    # tower and spire
    ((L, 190), APEX), (APEX, (R, 190)), ((L, 190), (BLOCK_R, 190)),
    ((L, 190), (L, G)), ((R, 190), (R, RIDGE)),
    # block and ridge
    ((BLOCK_R, 190), (BLOCK_R, RIDGE)), ((P1, RIDGE), (P2, RIDGE)),
    # the M roof: two 45-degree gables
    ((L, EAVE), (P1, RIDGE)), ((P1, RIDGE), (V, EAVE)), ((V, EAVE), (P2, RIDGE)), ((P2, RIDGE), (HALL_R, EAVE)),
    ((V, EAVE), (HALL_R, EAVE)),
    # walls
    ((V, RIDGE), (V, G)), ((HALL_R, EAVE), (HALL_R, G)), ((195, 335), (195, G)),
    # ground and buttress
    ((30, G), (HALL_R, G)), ((30, G), (L, 290)),
    # door legs
    ((ARCH_C[0] - ARCH_R, ARCH_C[1]), (ARCH_C[0] - ARCH_R, G)), ((ARCH_C[0] + ARCH_R, ARCH_C[1]), (ARCH_C[0] + ARCH_R, G)),
]
parts = [f'<path d="M{a[0]} {a[1]}L{b[0]} {b[1]}"/>' for a, b in lines]
parts.append(f'<path d="M{ARCH_C[0] - ARCH_R} {ARCH_C[1]}A{ARCH_R} {ARCH_R} 0 0 1 {ARCH_C[0] + ARCH_R} {ARCH_C[1]}"/>')
parts.append(f'<circle cx="{ARCH_C[0] + 3}" cy="{ARCH_C[1] + 10}" r="2.5"/>')

# smoke: a coil climbing from the apex up and to the right, three loops, each smaller
def coil():
    pts = []
    x0, y0 = APEX
    x1, y1 = 160, 8
    n = 3
    steps = 180
    for i in range(steps + 1):
        s = i / steps
        a = s * n * 2 * math.pi
        r = 17 * (1 - 0.5 * s)
        # a point travelling along the line apex->top-right, circling as it goes
        cx, cy = x0 + (x1 - x0) * s, y0 + (y1 - y0) * s
        pts.append((cx + r * math.sin(a) + r, cy - r * math.cos(a) + r))
    # straight run first so the coil starts at the apex
    return pts

pts = coil()
# Catmull-Rom to cubic Bezier, so the file is a few curves instead of 180 line segments
def crb(points, k=1.0):
    out = [f"M{points[0][0]:.1f} {points[0][1]:.1f}"]
    stride = 6
    sel = points[::stride] + [points[-1]]
    for i in range(len(sel) - 1):
        p0 = sel[i - 1] if i > 0 else sel[i]
        p1, p2 = sel[i], sel[i + 1]
        p3 = sel[i + 2] if i + 2 < len(sel) else p2
        c1 = (p1[0] + (p2[0] - p0[0]) / 6 * k, p1[1] + (p2[1] - p0[1]) / 6 * k)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6 * k, p2[1] - (p3[1] - p1[1]) / 6 * k)
        out.append(f"C{c1[0]:.1f} {c1[1]:.1f} {c2[0]:.1f} {c2[1]:.1f} {p2[0]:.1f} {p2[1]:.1f}")
    return "".join(out)

parts.append(f'<path d="M{APEX[0]} {APEX[1]}L{pts[0][0]:.1f} {pts[0][1]:.1f}"/>')
parts.append(f'<path d="{crb(pts)}"/>')

refined = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 420" role="img" aria-label="Castle">\n'
           '<!-- The sketch, redrawn: same bones on a 5-unit grid, true verticals, 45-degree roofs, hollow. -->\n'
           '<g id="mark" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">\n'
           + "\n".join(parts) + "\n</g>\n</svg>\n")
open(f"{landing}/logo-castle.svg", "w", encoding="utf-8").write(refined)

open(preview, "w", encoding="utf-8").write(f"""<!doctype html><meta charset=utf-8>
<style>body{{background:#050608;color:#f3f2ee;display:flex;gap:40px;padding:30px;font-family:sans-serif}}
figure{{margin:0;width:300px}} svg{{width:300px;height:420px;border:1px solid #333}}
.over{{position:relative}} .over svg{{position:absolute;inset:0}} .over .b{{color:#ff6a00;opacity:.8}}
img{{width:300px;filter:invert(1)}}</style>
<figure><img src="logo/sketch.jpg"><figcaption>scan</figcaption></figure>
<figure>{sketch}<figcaption>logo-sketch.svg</figcaption></figure>
<figure>{refined}<figcaption>logo-castle.svg</figcaption></figure>
<figure class=over>{sketch}<span class=b>{refined}</span><figcaption style="position:absolute;top:430px">overlay</figcaption></figure>
""")
print("wrote", f"{landing}/logo-sketch.svg", len(sketch), "bytes;", f"{landing}/logo-castle.svg", len(refined), "bytes")
