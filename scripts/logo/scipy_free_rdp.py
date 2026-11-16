"""Ramer-Douglas-Peucker polyline simplification, iterative, no SciPy."""


def _dist(p, a, b):
    (px, py), (ax, ay), (bx, by) = p, a, b
    dx, dy = bx - ax, by - ay
    if dx == 0 and dy == 0:
        return ((px - ax) ** 2 + (py - ay) ** 2) ** 0.5
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    cx, cy = ax + t * dx, ay + t * dy
    return ((px - cx) ** 2 + (py - cy) ** 2) ** 0.5


def rdp(points, eps):
    if len(points) < 3:
        return list(points)
    keep = [False] * len(points)
    keep[0] = keep[-1] = True
    stack = [(0, len(points) - 1)]
    while stack:
        s, e = stack.pop()
        if e <= s + 1:
            continue
        a, b = points[s], points[e]
        best, bi = 0.0, -1
        for i in range(s + 1, e):
            d = _dist(points[i], a, b)
            if d > best:
                best, bi = d, i
        if best > eps:
            keep[bi] = True
            stack.append((s, bi))
            stack.append((bi, e))
    return [p for p, k in zip(points, keep) if k]
