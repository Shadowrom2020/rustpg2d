"""Converts Meshy walk / attack grids (WORK/gen/walk_<name>.png, attack_<name>.png: 4 x 4 cells,
rows front / left / right / back) into a game character sheet WORK/out/<name>.png: 4 columns x 16
rows (walk 0-3, idle 4-7, attack 8-11, hurt 12-15; facings down, left, right, up).

usage: sprites.py NAME... | --all

- Each cell keeps its figure: the largest shape and the parts near it (weapons), dropping pieces
  of neighbouring cells (touching the cell's sides, or lying wholly above / below the figure).
- One scale per size class (the median of its characters' natural scales: hats add height
  instead of shrinking bodies), feet on (fw / 2, fh - 3).
- The side row faces left (SIDE_FACES_RIGHT: right); the other side is its mirror; single frames
  turned the other way are flipped. Faded frames (the model sometimes fades one) are replaced
  (walk: the frame two steps on) or dropped (attack).
- Idle frame 2 breathes (the upper body one pixel lower); hurt leans back; characters without an
  attack sheet stand in the attack rows. One palette per sheet, a soft shadow under the feet."""
import json, os, sys
from collections import deque

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cast import CAST, SIDE_FACES_RIGHT, SIZE
from common import WORK

GEN = os.path.join(WORK, "gen")
OUT = os.path.join(WORK, "out")
os.makedirs(OUT, exist_ok=True)
NCOL = 48
FACINGS = ["down", "left", "right", "up"]


def label(m):
    """4-connected components of a boolean mask: (labels, sizes) with sizes[0] = 0."""
    h, w = m.shape
    lab = np.zeros((h, w), np.int32)
    sizes = [0]
    for y0, x0 in zip(*np.nonzero(m)):
        if lab[y0, x0]:
            continue
        n = len(sizes)
        lab[y0, x0] = n
        q = deque([(y0, x0)])
        cnt = 0
        while q:
            y, x = q.popleft()
            cnt += 1
            for yy, xx in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
                if 0 <= yy < h and 0 <= xx < w and m[yy, xx] and not lab[yy, xx]:
                    lab[yy, xx] = n
                    q.append((yy, xx))
        sizes.append(cnt)
    return lab, sizes


def fill_holes(a):
    """Partly transparent pixels inside the figure (background removal eats into light areas)
    become opaque with their opaque neighbours' colour; real gaps (fully transparent) stay."""
    alpha = a[..., 3]
    outside = alpha <= 8
    h, w = outside.shape
    seen = np.zeros_like(outside)
    q = deque([(y, x) for y in range(h) for x in (0, w - 1) if outside[y, x]] + [(y, x) for x in range(w) for y in (0, h - 1) if outside[y, x]])
    for y, x in q:
        seen[y, x] = True
    while q:
        y, x = q.popleft()
        for yy, xx in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
            if 0 <= yy < h and 0 <= xx < w and alpha[yy, xx] <= 128 and not seen[yy, xx]:
                seen[yy, xx] = True
                q.append((yy, xx))
    holes = (alpha > 8) & (alpha <= 128) & ~seen
    if not holes.any():
        return a
    out = a.copy()
    solid = alpha > 128
    for y, x in zip(*np.nonzero(holes)):
        ys, xs = slice(max(0, y - 2), y + 3), slice(max(0, x - 2), x + 3)
        m = solid[ys, xs]
        if m.any():
            out[y, x, :3] = a[ys, xs][m][:, :3].mean(axis=0)
        out[y, x, 3] = 255
    return out


def keep_figure(cell):
    a = np.asarray(cell).copy()
    m = a[..., 3] > 64
    if not m.any():
        return None
    lab, sizes = label(m)
    main = int(np.argmax(sizes))
    keep = np.array([s >= sizes[main] * 0.02 for s in sizes])
    keep[0] = False
    for side in (lab[:, 0], lab[:, -1]):
        for l in set(side[side > 0].tolist()):
            if l != main:
                keep[l] = False
    ys, xs = np.nonzero(lab == main)
    top, bot = ys.min(), ys.max()
    mx = (xs.max() - xs.min()) * 0.25
    x0, x1 = xs.min() - mx, xs.max() + mx
    for l in range(1, len(sizes)):
        if keep[l] and l != main:
            ly, lx = np.nonzero(lab == l)
            # Wholly above or below the figure: the feet / head of the neighbouring row.
            if ly.max() < top or ly.min() > bot + 2 or lx.min() > x1 or lx.max() < x0:
                keep[l] = False
    a[~keep[lab]] = 0
    a = fill_holes(a)
    ys, xs = np.nonzero(a[..., 3] > 128)
    if len(ys) == 0:
        return None
    return Image.fromarray(a[ys.min():ys.max() + 1, xs.min():xs.max() + 1], "RGBA")


def faded(fig):
    """The model faded this figure out (its solid pixels are mostly see-through)."""
    a = np.asarray(fig)[..., 3]
    m = a > 64
    return m.any() and np.median(a[m]) < 235


def cells(path):
    """The 16 cells of a grid as figures: {(row, col): Image}."""
    im = Image.open(path).convert("RGBA")
    w, h = im.width / 4, im.height / 4
    out = {}
    for r in range(4):
        for c in range(4):
            f = keep_figure(im.crop((round(c * w), round(r * h), round((c + 1) * w), round((r + 1) * h))))
            if f is not None:
                out[(r, c)] = f
    return out


def place(fig, k, fw, fh):
    """A figure scaled by k into a fw x fh frame, feet on (fw/2, fh-3), kept inside the frame."""
    a = np.asarray(fig)[..., 3] > 128
    legs = a[int(a.shape[0] * 0.75):]
    lx = np.nonzero(legs.any(axis=0))[0]
    mid = (lx.min() + lx.max()) / 2 if len(lx) else a.shape[1] / 2
    S = 8
    big = fig.resize((max(1, round(fig.width * k * S)), max(1, round(fig.height * k * S))), Image.LANCZOS)
    canvas = Image.new("RGBA", (fw * S, fh * S))
    left = round(fw / 2 * S - mid * k * S)
    if big.width < (fw - 2) * S:
        left = max(S, min(left, fw * S - S - big.width))
    canvas.alpha_composite(big, (left, round((fh - 2.5) * S - big.height)))
    arr = np.asarray(canvas).astype(float) / 255
    rgb, al = arr[..., :3] * arr[..., 3:4], arr[..., 3:4]
    rgb = rgb.reshape(fh, S, fw, S, 3).mean(axis=(1, 3))
    al = al.reshape(fh, S, fw, S, 1).mean(axis=(1, 3))
    col = np.where(al > 0, rgb / np.maximum(al, 1e-6), 0)
    fr = np.zeros((fh, fw, 4), np.uint8)
    fr[..., :3] = np.clip(col * 255, 0, 255)
    fr[..., 3] = np.where(al[..., 0] >= 0.5, 255, 0)
    return fr


def breathe(fr):
    """Idle frame 2: everything above the hips one pixel lower (the figure's upper 55 %)."""
    ys = np.nonzero(fr[..., 3].any(axis=1))[0]
    if len(ys) == 0:
        return fr
    cut = int(ys.min() + (ys.max() - ys.min()) * 0.55)
    out = fr.copy()
    out[ys.min() + 1:cut + 1] = fr[ys.min():cut]
    out[ys.min()] = 0
    return out


def lean(fr, facing):
    """Hurt: the upper body leans back (away from the facing; front / back views: up and back)."""
    ys = np.nonzero(fr[..., 3].any(axis=1))[0]
    if len(ys) == 0:
        return fr
    top, bot = ys.min(), ys.max()
    out = np.zeros_like(fr)
    dx = {"down": 0, "up": 0, "left": 1, "right": -1}[facing]
    for y in range(fr.shape[0]):
        t = max(0.0, (bot - y) / max(1, bot - top) - 0.35) / 0.65  # 0 at the hips, 1 at the head
        sx = round(dx * 3 * t)
        sy = -1 if facing in ("down", "up") and t > 0.5 else 0
        yy = y + sy
        if 0 <= yy < fr.shape[0]:
            src = np.roll(fr[y], sx, axis=0)
            if sx > 0:
                src[:sx] = 0
            elif sx < 0:
                src[sx:] = 0
            m = src[..., 3] > 0
            out[yy][m] = src[m]
            if sy:  # the row it left stays filled (the next row up overwrites it): no gap
                out[y][m] = src[m]
    return out


def iou(a, b):
    a, b = a[..., 3] == 255, b[..., 3] == 255
    return (a & b).sum() / max(1, (a | b).sum())


def natural_scale(name):
    cls = CAST[name][0]
    walk = cells(os.path.join(GEN, f"walk_{name}.png"))
    stand = walk.get((0, 0)) or next(iter(walk.values()))
    return SIZE[cls][0] / stand.height


def build(name, k):
    cls, action, _ = CAST[name]
    _, (fw, fh), _, rx = SIZE[cls]
    walk = cells(os.path.join(GEN, f"walk_{name}.png"))
    # Faded walk frames: the frame two steps on (the same pose with the other foot... or standing).
    for (r, c), f in list(walk.items()):
        if faded(f):
            alt = walk.get((r, (c + 2) % 4))
            if alt is not None and not faded(alt):
                walk[(r, c)] = alt
                print(f"  {name}: walk frame {r},{c} faded, replaced")
    fr = {key: place(f, k, fw, fh) for key, f in walk.items()}
    # Frames much smaller than the rest of their row (a faded figure the background removal ate).
    for r in range(4):
        areas = {c: (fr[(r, c)][..., 3] == 255).sum() for c in range(4) if (r, c) in fr}
        if len(areas) < 2:
            continue
        typical = sorted(areas.values())[len(areas) // 2]
        for c, a in areas.items():
            alt = (r, (c + 2) % 4)
            if a < 0.6 * typical and alt in fr and areas.get(alt[1], 0) >= 0.6 * typical:
                fr[(r, c)] = fr[alt].copy()
                print(f"  {name}: walk frame {r},{c} too small, replaced")
    if (1, 0) in fr:
        for c in (1, 2, 3):
            x = fr.get((1, c))
            if x is not None and iou(x, fr[(1, 0)][:, ::-1]) > iou(x, fr[(1, 0)]) + 0.05:
                fr[(1, c)] = x[:, ::-1].copy()
    W = {}
    flip = name in SIDE_FACES_RIGHT
    for c in range(4):
        W[("down", c)] = fr.get((0, c), fr.get((0, 0)))
        side = fr.get((1, c), fr.get((1, 0)))
        mirrored = side[:, ::-1].copy() if side is not None else None
        W[("left", c)], W[("right", c)] = (mirrored, side) if flip else (side, mirrored)
        W[("up", c)] = fr.get((3, c), fr.get((3, 0)))
    A = {}
    ap = os.path.join(GEN, f"attack_{name}.png")
    if action and os.path.exists(ap):
        att = cells(ap)

        def usable(x, f, ref):
            return x is not None and not faded(f) and (x[..., 3] == 255).sum() >= 0.5 * (ref[..., 3] == 255).sum()

        for c in range(4):
            for row, f in ((0, "down"), (1, "left"), (3, "up")):
                fig = att.get((row, c))
                x = place(fig, k, fw, fh) if fig is not None else None
                A[(f, c)] = x if usable(x, fig, W[(f, 0)]) else None
        ref = W[("left", 0)]
        for c in range(4):
            x = A[("left", c)]
            if x is not None and iou(x, ref[:, ::-1]) > iou(x, ref) + 0.03:
                A[("left", c)] = x[:, ::-1].copy()
        for c in range(4):
            A[("right", c)] = A[("left", c)][:, ::-1].copy() if A[("left", c)] is not None else None
    frames = {}
    for fi, f in enumerate(FACINGS):
        for c in range(4):
            frames[(fi, c)] = W[(f, c)]
        st = W[(f, 0)]
        frames[(4 + fi, 0)] = st
        frames[(4 + fi, 1)] = breathe(st)
        for c in range(4):
            frames[(8 + fi, c)] = A.get((f, c)) if A.get((f, c)) is not None else st
        frames[(12 + fi, 0)] = lean(st, f)
    opaque = np.concatenate([x[x[..., 3] == 255][:, :3] for x in frames.values() if x is not None])
    pal = np.array(Image.fromarray(opaque.reshape(1, -1, 3), "RGB").quantize(NCOL, method=Image.Quantize.MEDIANCUT).getpalette()[: NCOL * 3]).reshape(-1, 3)
    yy, xx = np.mgrid[0:fh, 0:fw]
    shadow = (((xx + 0.5 - fw / 2) / rx) ** 2 + ((yy + 0.5 - (fh - 3)) / 3.0) ** 2) <= 1
    sheet = np.zeros((fh * 16, fw * 4, 4), np.uint8)
    for (r, c), x in frames.items():
        if x is None:
            continue
        x = x.copy()
        m = x[..., 3] == 255
        px = x[m][:, :3].astype(int)
        x[m, :3] = pal[((px[:, None, :] - pal[None]) ** 2).sum(-1).argmin(1)]
        put = shadow & ~m
        x[put] = (0, 0, 0, 76)
        sheet[r * fh:(r + 1) * fh, c * fw:(c + 1) * fw] = x
    Image.fromarray(sheet, "RGBA").save(os.path.join(OUT, name + ".png"), optimize=True)
    return fw, fh


def main():
    have = [n for n in CAST if os.path.exists(os.path.join(GEN, f"walk_{n}.png"))]
    kp = os.path.join(WORK, "scales.json")
    ks = json.load(open(kp)) if os.path.exists(kp) else {}
    for n in have:
        if n not in ks:
            ks[n] = natural_scale(n)
    json.dump(ks, open(kp, "w"), indent=1)
    by = {}
    for n in have:
        by.setdefault(CAST[n][0], []).append(ks[n])
    med = {c: sorted(v)[len(v) // 2] for c, v in by.items()}
    names = have if sys.argv[1:] == ["--all"] else sys.argv[1:]
    for n in names:
        fw, fh = build(n, med[CAST[n][0]])
        print(f"{n}: scale {med[CAST[n][0]]:.3f}, frame {fw}x{fh}")


if __name__ == "__main__":
    main()
