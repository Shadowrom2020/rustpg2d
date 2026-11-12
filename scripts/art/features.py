"""Colour features of figure sheets (skin, hair, clothing): picks each feature's shades from the
art by rules, keeps them apart from other parts that share a shade (pixels of a feature's region
get a one-step-different colour, +1 in blue), renders a check image (the features tinted) and
writes the shades into a sheets.ron (keeping each feature's colour list).

usage:
  python3 features.py palette IMAGE                  the image's colours with where they are
  python3 features.py pick NAME IMAGE OUT_DIR        apply NAME's rules (FIGURES) to a copy of IMAGE
                                                     in OUT_DIR: NAME.png, NAME.json, NAME_check.png
  python3 features.py write SHEETS.RON ID SHADES.json   replace sheet ID's feature shades

A colour's statistics: n (pixels), my (mean height in the frame, 0 = top), top / bot (share in
the top 36 % / bottom 22 % of the frame), h (hue 0..360), s (saturation), l (lightness)."""
import colorsys, json, os, re, shutil, sys
from collections import defaultdict

from PIL import Image


def analyze(path, fw=64, fh=96):
    """[(rgb, n, my, top, bot, hue, sat, light)] of the opaque colours, most used first."""
    im = Image.open(path).convert("RGBA")
    px = im.load()
    st = defaultdict(lambda: [0, 0.0, 0, 0])
    for y in range(im.height):
        ry = (y % fh) / fh
        for x in range(im.width):
            r, g, b, a = px[x, y]
            if a < 200:
                continue
            s = st[(r, g, b)]
            s[0] += 1
            s[1] += ry
            s[2] += ry < 0.36
            s[3] += ry > 0.78
    out = []
    for c, (n, sy, top, bot) in st.items():
        h, l, sat = colorsys.rgb_to_hls(*[v / 255 for v in c])
        out.append((c, n, sy / n, top / n, bot / n, h * 360, sat, l))
    return sorted(out, key=lambda t: -t[1])


def split(path, shades, region, fw=64, fh=96, xr=(0.0, 1.0)):
    """Pixels of `shades` inside `region` (fractions of the frame's height, `xr` of its width) get
    an unused variant colour (blue +-1..3). Returns ({old: new}, pixels changed)."""
    im = Image.open(path).convert("RGBA")
    px = im.load()
    used = {px[x, y][:3] for y in range(im.height) for x in range(im.width)}
    var = {}
    for c in shades:
        for d in (1, -1, 2, -2, 3):
            v = (c[0], c[1], min(255, max(0, c[2] + d)))
            if v not in used:
                var[c] = v
                used.add(v)
                break
    n = 0
    for y in range(im.height):
        if not region[0] <= (y % fh) / fh < region[1]:
            continue
        for x in range(im.width):
            if not xr[0] <= (x % fw) / fw < xr[1]:
                continue
            r, g, b, a = px[x, y]
            if a and (r, g, b) in var:
                px[x, y] = var[(r, g, b)] + (a,)
                n += 1
    im.save(path)
    return var, n


def check(path, feats, out, fw=64, fh=96):
    """A strip of six frames (standing, walking left, walking down, walking up, attacking left,
    hurt) as drawn and with the features tinted (magenta, green, cyan, yellow)."""
    im = Image.open(path).convert("RGBA")
    tint = [(255, 0, 255), (0, 255, 0), (0, 255, 255), (255, 255, 0)]
    frames = [(0, 4), (1, 1), (1, 0), (0, 3), (1, 9), (0, 12)]
    w = len(frames) * fw * 3
    o = Image.new("RGBA", (w * 2, fh * 3), (50, 50, 60, 255))
    for k, (c, r) in enumerate(frames):
        fr = im.crop((c * fw, r * fh, (c + 1) * fw, (r + 1) * fh))
        t = fr.copy()
        p = t.load()
        for y in range(fh):
            for x in range(fw):
                q = p[x, y]
                for i, (_, sh) in enumerate(feats):
                    if q[3] and q[:3] in sh:
                        p[x, y] = tint[i] + (255,)
        o.alpha_composite(fr.resize((fw * 3, fh * 3), Image.NEAREST), (k * fw * 3, 0))
        o.alpha_composite(t.resize((fw * 3, fh * 3), Image.NEAREST), (w + k * fw * 3, 0))
    o.save(out)


def make(name, src, rules, out_dir, head=(0.0, 0.36), fw=64, fh=96):
    """rules: {feature: (kind, predicate[, region[, x range]])}; kind 'split' = the feature's
    pixels inside the region get variant shades (they share shades with other parts)."""
    p = os.path.join(out_dir, name + ".png")
    shutil.copy(src, p)
    col = {c: rest for c, *rest in analyze(p, fw, fh)}
    out = {}
    for feat, (kind, pred, *reg) in rules.items():
        shades = [c for c, v in col.items() if pred(*v)]
        if kind == "split":
            var, n = split(p, shades, reg[0] if reg else head, fw, fh, reg[1] if len(reg) > 1 else (0.0, 1.0))
            shades = list(var.values())
            print(f"{name}: {feat} split {n} px, {len(shades)} shades")
        else:
            print(f"{name}: {feat} {len(shades)} shades")
        out[feat] = [list(c) for c in shades]
    json.dump(out, open(os.path.join(out_dir, name + ".json"), "w"))
    check(p, [(k, set(map(tuple, v))) for k, v in out.items()], os.path.join(out_dir, name + "_check.png"), fw, fh)
    return out


def write(path, sid, shades_json):
    """Replaces the shades of sheet `sid`'s features in a sheets.ron (their colours stay)."""
    s = open(path).read()
    feats = json.load(open(shades_json))
    start = s.index(f'id: "{sid}",')
    end = s.find("\n    ),", start)
    block = s[start:end]
    for name, shades in feats.items():
        m = re.search(r'name: "%s",\s*shades: \[(.*?)\]' % re.escape(name), block, re.S)
        if not m:
            print("no feature", name, "in", sid)
            continue
        body = "".join(f"\n{' ' * 20}({c[0]}, {c[1]}, {c[2]})," for c in shades) + "\n" + " " * 16
        block = block[: m.start(1)] + body + block[m.end(1) :]
    open(path, "w").write(s[:start] + block + s[end:])


# The figures' rules (0.17 art without weapons). Predicates take (n, my, top, bot, h, s, l).
def red(h):
    return h <= 14 or h >= 345


FIGURES = {
    "hero": {
        "skin": ("plain", lambda n, my, top, bot, h, s, l: 15 <= h <= 35 and l >= 0.42 and bot < 0.03 and s >= 0.15),
        "hair": ("split", lambda n, my, top, bot, h, s, l: top >= 0.35 and 10 <= h <= 30 and s >= 0.3 and l < 0.4),
        "clothing": ("plain", lambda n, my, top, bot, h, s, l: 205 <= h <= 218 and s >= 0.24 and bot < 0.02),
    },
    "heroine": {
        "skin": ("plain", lambda n, my, top, bot, h, s, l: 22 <= h <= 32 and l >= 0.45 and s >= 0.25 and bot < 0.05),
        "hair": ("split", lambda n, my, top, bot, h, s, l: top >= 0.38 and 17 <= h <= 26 and s >= 0.33 and l < 0.42, (0.0, 0.5)),
        "clothing": ("plain", lambda n, my, top, bot, h, s, l: 205 <= h <= 222 and s >= 0.18 and bot < 0.02 and l >= 0.25),
    },
    "elf": {
        "skin": ("plain", lambda n, my, top, bot, h, s, l: 23 <= h <= 28 and s >= 0.3 and l >= 0.55 and bot < 0.02),
        "hair": ("split", lambda n, my, top, bot, h, s, l: 35 <= h <= 60 and 0.1 <= s <= 0.3 and l >= 0.42 and top >= 0.6, (0.0, 0.45)),
        "clothing": ("plain", lambda n, my, top, bot, h, s, l: 98 <= h <= 120 and s >= 0.14 and bot < 0.02),
    },
    "elf_f": {
        "skin": ("plain", lambda n, my, top, bot, h, s, l: 33 <= h <= 39.5 and 0.5 <= s <= 0.75 and l >= 0.5 and bot < 0.02),
        "hair": ("split", lambda n, my, top, bot, h, s, l: 26 <= h <= 46 and s >= 0.45 and top >= 0.5 and not (33 <= h <= 39.5 and 0.5 <= s <= 0.75 and l >= 0.5 and bot < 0.02), (0.0, 0.5)),
        "clothing": ("plain", lambda n, my, top, bot, h, s, l: 105 <= h <= 118 and s >= 0.19 and bot < 0.02),
    },
    # The helmet shares the darker skin shades: only the light ones.
    "dwarf": {
        "skin": ("plain", lambda n, my, top, bot, h, s, l: 22 <= h <= 26 and l >= 0.44 and s >= 0.3 and bot < 0.02),
    },
    "dwarf_f": {
        "skin": ("plain", lambda n, my, top, bot, h, s, l: 22 <= h <= 31 and s >= 0.45 and l >= 0.53),
        "hair": ("split", lambda n, my, top, bot, h, s, l: 12 <= h <= 22 and s >= 0.45 and 0.19 <= l <= 0.45 and top >= 0.1, (0.0, 0.58)),
    },
}

if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "palette":
        for c, n, my, top, bot, h, s, l in analyze(sys.argv[2]):
            print(f"{c} n={n:5d} y={my:.2f} top={top:.2f} bot={bot:.2f} hue={h:5.1f} sat={s:.2f} l={l:.2f}")
    elif cmd == "pick":
        make(sys.argv[2], sys.argv[3], FIGURES[sys.argv[2]], sys.argv[4])
    elif cmd == "write":
        write(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        print(__doc__)
