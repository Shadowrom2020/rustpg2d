"""A kit figure as voxel blocks on the kit's skeleton, built piece by piece: the body (head and
neck, with what its race adds), outfits, hair, beards, headwear and cloaks. Every piece is its own
mesh on the same bones, so the engine can draw any combination in one pose.

Sizes are in voxels of the first look study (0.045 m, about one world pixel) times `R` = 2: one
voxel is one pixel of the drawn figure (see README.md)."""
import numpy as np

from kitlib import X, Y, Z, Mesh, Rig, Vox, frame, norm, rgb

R = 2
V = 0.045 / R

# The kit's colours. Colour features recolour exact colours, so pieces share these.
SKIN, HAIR, EYE = "#e0ac84", "#5a3a22", "#2a2230"
CLOTH, CLOTH_DARK, TROUSERS, BOOTS = "#3f6fb0", "#2c4c7c", "#6b5a48", "#4a3526"
LEATHER, LEATHER_DARK = "#9a6a3c", "#5e3e24"
METAL, METAL_DARK, MAIL, MAIL_DARK, TRIM = "#aab4c2", "#6c7686", "#8d949f", "#70767f", "#c9a03c"
IVORY, HORN, DRAGON_EYE, CLOAK = "#efe8d0", "#d8cfae", "#f2c84b", "#3d5a3a"


def shade(h, k):
    return "#%02x%02x%02x" % tuple(min(255, int(v * k)) for v in rgb(h))


MOUTH, BELLY = shade(SKIN, .74), shade(SKIN, 1.12)

# Torso outline by height (0 = crotch, 1 = shoulders), in first-study voxels.
W_M = [(0, 6.0), (.12, 7.0), (.3, 6.6), (.45, 5.8), (.62, 6.6), (.8, 8.0), (.93, 8.0), (1, 6.8)]
W_F = [(0, 6.0), (.12, 7.2), (.3, 6.6), (.47, 4.6), (.62, 5.6), (.8, 6.8), (.93, 6.6), (1, 5.6)]
D_M = [(0, 3.4), (.15, 4.0), (.45, 3.2), (.8, 4.2), (1, 3.4)]
D_F = [(0, 3.4), (.15, 4.0), (.45, 3.0), (.72, 4.4), (.85, 4.2), (1, 3.2)]


def rnd(x):
    return int(x + 0.5)


def ev(x):
    return max(2, 2 * rnd(x / 2))


def u(x):
    return max(1, rnd(x * R))


def interp(keys, t):
    for (a, va), (b, vb) in zip(keys, keys[1:]):
        if a <= t <= b:
            return va + (vb - va) * (t - a) / (b - a)
    return keys[-1][1]


TH = max(1, rnd(R * .75))        # how thick shells are (hair, pauldrons, cuffs)


# ---------------------------------------------------------------- outfits: colours by zone

def clothes():
    return dict(pelvis=CLOTH, waist=CLOTH, chest=CLOTH, belt=LEATHER, buckle=METAL, neckline=True, upper=[(0, CLOTH)], fore=[(0, CLOTH), (.4, SKIN)],
                thigh=[(0, CLOTH), (.26, TROUSERS)], shin=[(0, TROUSERS), (.45, BOOTS)], foot=BOOTS)


def leather_armor():
    return dict(pelvis=LEATHER, waist=LEATHER, chest=LEATHER, belt=LEATHER_DARK, buckle=METAL, strap=LEATHER_DARK, neckline=True, upper=[(0, CLOTH)], pauldron={(0, .34): LEATHER_DARK},
                fore=[(0, CLOTH), (.2, LEATHER_DARK)], thigh=[(0, LEATHER), (.14, TROUSERS)], shin=[(0, TROUSERS), (.2, BOOTS)], shin_shell={(.2, .32): BOOTS}, foot=BOOTS)


def chain_mail():
    return dict(pelvis=MAIL, waist=MAIL, chest=MAIL, bands=(MAIL, MAIL_DARK), belt=LEATHER_DARK, buckle=METAL, collar=MAIL_DARK, upper=[(0, MAIL), (.6, CLOTH)], pauldron={(0, .25): MAIL_DARK},
                fore=[(0, CLOTH), (.3, LEATHER_DARK)], thigh=[(0, MAIL), (.3, TROUSERS)], shin=[(0, TROUSERS), (.3, BOOTS)], foot=BOOTS)


def plate_armor():
    return dict(pelvis=METAL, waist=METAL_DARK, chest=METAL, bands=(METAL_DARK, METAL), belt=TRIM, collar=METAL_DARK, upper=[(0, METAL), (.7, METAL_DARK)], pauldron={(0, .5): METAL},
                fore=[(0, METAL)], hand=METAL_DARK, thigh=[(0, METAL), (.85, METAL_DARK)], thigh_shell={(0, .25): METAL}, shin=[(0, METAL_DARK), (.15, METAL)], foot=METAL_DARK)


def robe():
    return dict(pelvis=CLOTH, waist=CLOTH, chest=CLOTH, belt=TRIM, neckline=True, upper=[(0, CLOTH)], fore=[(0, CLOTH)], cuff={(.45, 1): CLOTH}, thigh=[(0, CLOTH_DARK)],
                shin=[(0, CLOTH_DARK), (.7, BOOTS)], foot=BOOTS, skirt=CLOTH, hem=TRIM)


OUTFITS = dict(clothes=clothes, leather_armor=leather_armor, chain_mail=chain_mail, plate_armor=plate_armor, robe=robe)
HAIR_STYLES = ("short", "long", "ponytail", "bun", "mohawk")
BEARDS = ("short", "long")
HEADWEAR = ("helmet", "hood")


class Figure:
    """One build (proportions and sex) of the kit. Each method returns a piece's `Mesh`."""

    def __init__(self, build):
        self.b = build
        self.rig = Rig().proportions(**build.get("rig", {}))
        bulk = build.get("bulk", 1.0)
        self.TW, self.TD = build.get("wide", 1.0), build.get("deep", 1.0)
        self.aw = rnd((2 * bulk + 0.2) * R)
        self.lw = rnd(3 * bulk * R)
        self.hn = rnd(5 * build.get("head", 1.0) * R)
        self._head = None
        self._torso = None

    # ---- shared layout
    def limb(self, blocks, a, b, w, bone, zones, shell=None):
        a, b = np.asarray(a, float), np.asarray(b, float)
        x, y, z = frame(b - a)
        length = np.linalg.norm(b - a)
        rows = max(1, rnd(length / V))
        v = Vox(a - x * w * V / 2 - z * w * V / 2, x, y, z, (V, length / rows, V), bone)
        blocks.append(v)
        cut = 1 if w >= 4 else 0
        for j in range(rows):
            col = None
            for start, c in zones:
                if j >= rnd(start * rows):
                    col = c
            for i in range(w):
                for k in range(w):
                    if min(i, w - 1 - i) + min(k, w - 1 - k) >= cut:
                        v.set(i, j, k, col)
        for (f0, f1), c in (shell or {}).items():
            for j in range(rnd(f0 * rows), rnd(f1 * rows)):
                for i in range(-TH, w + TH):
                    for k in range(-TH, w + TH):
                        if (i, j, k) not in v.cells and min(i + TH, w + TH - 1 - i) + min(k + TH, w + TH - 1 - k) >= TH:
                            v.set(i, j, k, c)

    def torso(self):
        """Rows of the torso: (width, depth, zone) from the crotch up, and where they are."""
        if self._torso is None:
            rig = self.rig
            hips, neck = rig.p("Hips"), rig.p("neck")
            up = (rig.p("LeftUpLeg")[1] + rig.p("RightUpLeg")[1]) / 2
            y0, y1 = up - .045, neck[1]
            n = max(4, rnd((y1 - y0) / V))
            female = self.b.get("female")
            dims = []
            for j in range(n):
                t = (j + .5) / n
                dims.append((ev(interp(W_F if female else W_M, t) * self.TW * R), ev(interp(D_F if female else D_M, t) * self.TD * R), 0 if t < .3 else 1 if t < .62 else 2))
            self._torso = dict(n=n, y=(y0, y1), h=(y1 - y0) / n, ctr=np.array([hips[0], y0, (hips[2] + rig.p("Spine")[2]) / 2]), dims=dims, belt=rnd(.3 * n),
                               chest=next(j for j in range(n) if dims[j][2] == 2))
        return self._torso

    def head(self):
        """The head's own voxels (a block on the Head bone) and its face's measures."""
        if self._head is None:
            H = self.rig.p("Head")
            n = self.hn
            v = Vox(np.array([H[0] - n * V / 2, H[1] - .03, H[2] + .012 - n * V / 2]), X, Y, Z, V, "Head")
            cc = max(1, n // 5)
            for i in range(n):
                for j in range(n):
                    for k in range(n):
                        ex, ez, top = min(i, n - 1 - i), min(k, n - 1 - k), n - 1 - j
                        if ex + ez < 1 or ex + top < cc or ez + top < cc or ex + j < cc - 1:
                            continue
                        v.set(i, j, k, SKIN)
            e = rnd(n * .5)
            ew, eh, gap = max(1, rnd(R * .9)), max(1, rnd(R * .75)), u(1)
            l0 = n // 2 - (gap + 1) // 2 - ew if n % 2 else n // 2 - gap // 2 - ew
            self._head = dict(v=v, n=n, cc=cc, eye=e, ew=ew, eh=eh, l0=l0, r0=n - l0 - ew, mouth=e - u(1.25))
        return self._head

    # ---- pieces
    def body(self, ears=False, tusks=False, dragon=False, tail=False, horns=False):
        """The head and neck, with what a race adds to them."""
        m = Mesh(self.rig)
        hd = self.head()
        v, n, cc, e, ew, eh = Vox.like(hd["v"]), hd["n"], hd["cc"], hd["eye"], hd["ew"], hd["eh"]
        v.cells = dict(hd["v"].cells)
        front = n - 1
        if dragon:
            a, b = u(1), n - u(1)
            for k in range(n, n + u(3)):
                tip = k - n >= u(3) - max(1, u(.5))
                for i in range(a, b):
                    for j in range(u(1), u(3)):
                        if not (tip and (i in (a, b - 1) or j == u(3) - 1)):
                            v.set(i, j, k, BELLY if j < u(1) + max(1, u(.75)) else SKIN)
            hw = max(1, u(.9))
            for i0 in (1, n - 1 - ew):
                for i in range(i0, i0 + ew):
                    for j in range(u(3), u(3) + eh):
                        v.set(i, j, front, DRAGON_EYE)
            for i0 in (1, n - 1 - hw):
                for step in range(u(3)):
                    for i in range(i0, i0 + hw):
                        for d in range(hw):
                            v.set(i, n - cc + step + d, cc - 1 - step, HORN)
            for k in range(0, n - cc):
                for i in (n // 2 - 1, n // 2) if n % 2 == 0 else (n // 2,):
                    v.set(i, n, k, HORN)
        else:
            for i0 in (hd["l0"], hd["r0"]):
                for i in range(i0, i0 + ew):
                    for j in range(e, e + eh):
                        v.set(i, j, front, EYE)
            for i in range(n // 2 - u(.5), n - (n // 2 - u(.5))):
                v.set(i, hd["mouth"], front, MOUTH)
        if ears:
            for i0, d in ((-1, -1), (n, 1)):
                for step in range(u(1.5)):
                    for k in (n // 2 - step, n // 2 - step - 1):
                        v.set(i0 + d * (step // 2), e + step, k, SKIN)
        if tusks:
            for i in (hd["l0"] + ew - 1, hd["r0"]):
                for j in range(u(1), u(1) + u(1)):
                    v.set(i, j, n, IVORY)
        if horns:
            for i0, d in ((1, -1), (n - 2, 1)):
                for step in range(u(2)):
                    v.set(i0 + d * (step // 2), n - cc + step, n // 2 - step // 2, HORN)
                    v.set(i0 + d * (step // 2), n - cc + step, n // 2 - step // 2 - 1, HORN)
        v.emit(m)
        # The neck.
        rig = self.rig
        neck, head = rig.p("neck"), rig.p("Head")
        nw = ev(2 * R)
        rows = max(1, rnd((head[1] - .03 - neck[1]) / V))
        nv = Vox(np.array([neck[0], neck[1], neck[2] + .005]), X, Y, Z, (V, (head[1] - .03 - neck[1]) / rows, V), "neck")
        for j in range(rows):
            for i in range(-nw // 2, nw // 2):
                for k in range(-nw // 2, nw // 2):
                    if min(i + nw // 2, nw // 2 - 1 - i) + min(k + nw // 2, nw // 2 - 1 - k) >= 1:
                        nv.set(i, j, k, SKIN)
        nv.emit(m)
        if tail:
            self.tail(m, horn_tip=horns)
        return m

    def tail(self, m, horn_tip=False):
        hips = self.rig.p("Hips")
        w = ev(3 * R) if not horn_tip else ev(1.5 * R)
        d = self.torso()["dims"][0][1]
        v = Vox(np.array([hips[0], hips[1] - .09, hips[2]]), X, Y, Z, V, "Hips")
        steps = u(11)
        for s in range(steps):
            ww = max(1, rnd(w * (1 - s / steps * .8)))
            j = -rnd(s * 1.25)
            for i in range(-ww // 2, ww - ww // 2):
                for jj in range(j - 1, j + ww):
                    v.set(i, jj + u(2) - ww, -d // 2 - 1 - s, SKIN)
        v.emit(m)

    def outfit(self, name):
        """Everything below the neck in one outfit: torso, arms, hands, legs and feet."""
        o = OUTFITS[name]()
        m = Mesh(self.rig)
        rig, t = self.rig, self.torso()
        n, dims, belt0, h = t["n"], t["dims"], t["belt"], t["h"]
        bones = ["Hips", "Spine02", {"Spine01": .5, "Spine": .5}]
        zcol = [o["pelvis"], o["waist"], o["chest"]]
        vox = [Vox(t["ctr"], X, Y, Z, (V, h, V), bone) for bone in bones]
        for j, (w, d, z) in enumerate(dims):
            col = zcol[z]
            if o.get("bands") and col == o["bands"][0] and (j // u(1)) % 2:
                col = o["bands"][1]                           # mail and plate bands: every other row
            if o.get("belt") and belt0 <= j < belt0 + u(1):
                col = o["belt"]
            for i in range(-w // 2, w // 2):
                for k in range(-d // 2, d // 2):
                    if min(i + w // 2, w // 2 - 1 - i) + min(k + d // 2, d // 2 - 1 - k) >= 1:
                        vox[z].set(i, j, k, col)

        def front(j, i, color, back=False):
            w, d, z = dims[j]
            for k0, step in ((d // 2 - 1, -1),) + (((-d // 2, 1),) if back else ()):
                for k in (k0, k0 + step):
                    if (i, j, k) in vox[z].cells:
                        vox[z].set(i, j, k, color)
                        break
        if o.get("buckle"):
            for j in range(belt0, belt0 + u(1)):
                for i in range(-u(.5), u(.5)):
                    front(j, i, o["buckle"])
        if o.get("strap"):
            for j in range(belt0 + u(1), n):
                w = dims[j][0]
                x = (w // 2 - 2) - (n - 1 - j) * (w - 3) / max(1, n - 1 - belt0)
                for e in range(max(1, rnd(R * .6))):
                    front(j, rnd(x) - e, o["strap"], back=True)
        if o.get("collar"):
            for j in range(n - u(1), n):
                for (i, jj, k) in list(vox[2].cells):
                    if jj == j and abs(i + .5) < u(1.5):
                        vox[2].set(i, jj, k, o["collar"])
        elif o.get("neckline"):
            for step, j in enumerate(range(n - 1, n - 1 - u(1), -1)):
                for i in range(-u(.5) + step, u(.5) - step):
                    front(j, i, SKIN)
        blocks = list(vox)
        if o.get("skirt"):
            blocks.append(self.skirt(o["skirt"], o.get("hem")))
        for s in ("Left", "Right"):
            a, b, hnd, end = (rig.p(f"{s}{nm}") for nm in ("Arm", "ForeArm", "Hand", "Hand_End"))
            self.limb(blocks, a, b, self.aw, f"{s}Arm", o["upper"], o.get("pauldron"))
            self.limb(blocks, b, hnd, self.aw, f"{s}ForeArm", o["fore"], o.get("cuff"))
            self.limb(blocks, hnd, hnd + norm(end - hnd) * .09, self.aw, f"{s}Hand", [(0, o.get("hand", SKIN))])
            a, b, f = (rig.p(f"{s}{nm}") for nm in ("UpLeg", "Leg", "Foot"))
            self.limb(blocks, a, b, self.lw, f"{s}UpLeg", o["thigh"], o.get("thigh_shell"))
            self.limb(blocks, b, f, self.lw, f"{s}Leg", o["shin"], o.get("shin_shell"))
            w = self.lw
            rows, length, toe = max(2, rnd(f[1] / V)), w + u(2), u(1.5)
            fv = Vox(np.array([f[0] - w * V / 2, 0.0, f[2] + .045 - length * V / 2]), X, Y, Z, (V, f[1] / rows, V), f"{s}Foot")
            for j in range(rows):
                for i in range(w):
                    for k in range(length):
                        ahead = length - 1 - k
                        if ahead < toe and j >= rows - 1 - (toe - 1 - ahead) * rows // (2 * toe):
                            continue                     # the toe slopes down
                        if w >= 4 and min(i, w - 1 - i) + min(k, ahead) < 1:
                            continue
                        fv.set(i, j, k, o["foot"])
            blocks.append(fv)
        for v in blocks:
            v.emit(m)
        return m

    def skirt(self, color, hem):
        rig, t = self.rig, self.torso()
        hips = rig.p("Hips")
        top = (rig.p("LeftUpLeg")[1] + rig.p("RightUpLeg")[1]) / 2
        low = rig.p("LeftFoot")[1] + .018
        rows = max(1, rnd((top - low) / V))
        w0, d0 = t["dims"][rnd(.12 * t["n"])][:2]
        v = Vox(np.array([hips[0], low, t["ctr"][2]]), X, Y, Z, (V, (top - low) / rows, V), "Hips")
        for j in range(rows):
            f = 1 - j / max(1, rows - 1)                       # 1 at the hem
            w, d = ev(w0 + u(2) * (.6 + .6 * f)), ev(d0 + u(3) * (.7 + .5 * f))
            col = hem if hem and j < max(1, u(.5)) else color
            for i in range(-w // 2, w // 2):
                for k in range(-d // 2, d // 2):
                    if min(i + w // 2, w // 2 - 1 - i) + min(k + d // 2, d // 2 - 1 - k) >= 1:
                        v.set(i, j, k, col)
        return v

    def cap(self, v, color, rows, front=False, back=None, sides=None):
        """A shell round the head: over the top down to row `rows`, the back down to `back`, the
        sides down to `sides` (None = no further than the cap)."""
        hd = self.head()
        head, n, cc, t = hd["v"].cells, hd["n"], hd["cc"], TH
        lo, hi = rows, n + t
        for i in range(-t, n + t):
            for k in range(-t, n + (t if front else 0)):
                for j in range(min(lo, back if back is not None else lo, sides if sides is not None else lo), hi):
                    if (i, j, k) in head:
                        continue
                    ex, eb, top = min(i + t, n + t - 1 - i), k + t, hi - 1 - j
                    ef = n + t - 1 - k if front else 99
                    if ex + top < cc or eb + top < cc or ef + top < cc or ex + min(eb, ef) < 1:
                        continue
                    outer_side, outer_back = i < 0 or i >= n, k < 0
                    if j >= lo:
                        ok = j >= n or outer_side or outer_back or (front and k >= n)
                    else:
                        ok = (outer_back and back is not None and j >= back) or (outer_side and sides is not None and j >= sides and k < n - u(2))
                    if ok:
                        v.set(i, j, k, color)

    def hair(self, style):
        hd = self.head()
        head, n = hd["v"], hd["n"]
        v = Vox.like(head)
        mid = n // 2
        if style == "mohawk":
            w = max(1, u(.75))
            for i in range(mid - w // 2, mid - w // 2 + w):
                for k in range(-TH, n):
                    top = n + u(2) - (1 if k in (-TH, n - 1) else 0)
                    for j in range(n - u(2) if k < 0 else n - hd["cc"], top):
                        if (i, j, k) not in head.cells:
                            v.set(i, j, k, HAIR)
        else:
            long = style == "long"
            self.cap(v, HAIR, n - u(1), back=-u(3) if long else n - u(3), sides=u(1) if long else n - u(2))
            # A fringe over the brow, uneven.
            for i in range(1, n - 1):
                for j in range(n - u(1) - (1 if i % 3 == 0 else 0), n):
                    if j > hd["eye"] + hd["eh"] and (i, j, n) not in head.cells:
                        v.set(i, j, n, HAIR)
            if style == "ponytail":
                w = max(1, u(.75))
                for i in range(mid - w // 2, mid - w // 2 + w):
                    for k in range(-TH - w, -TH):
                        for j in range(-u(1), n - u(2)):
                            v.set(i, j, k, TRIM if j >= n - u(2) - max(1, u(.5)) else HAIR)
            if style == "bun":
                b = u(3)
                i0, j0, k0 = mid - b // 2, n - u(1), -TH - u(2)
                for i in range(i0, i0 + b):
                    for j in range(j0, j0 + b):
                        for k in range(k0, -TH + 1):
                            ex, ey, ez = min(i - i0, i0 + b - 1 - i), min(j - j0, j0 + b - 1 - j), min(k - k0, -TH - k)
                            if ex + ey >= 1 and ex + ez >= 1 and ey + ez >= 1:
                                v.set(i, j, k, HAIR)
        m = Mesh(self.rig)
        v.emit(m, hidden=head.cells)
        return m

    def beard(self, kind):
        hd = self.head()
        head, n, t = hd["v"], hd["n"], TH
        v = Vox.like(head)
        for j in range(0, u(2)):
            for i in range(-t, n + t):
                for k in range(n - u(2), n + t):
                    if (i, j, k) in head.cells or not (k >= n or i < 0 or i >= n):
                        continue
                    if min(i + t, n + t - 1 - i) + (n + t - 1 - k) < 1:
                        continue
                    if j == hd["mouth"] and k >= n and abs(i - (n - 1) / 2) <= u(.5):
                        continue                              # the mouth shows
                    v.set(i, j, k, HAIR)
        if kind == "long":
            for step, j in enumerate(range(-1, -u(5), -1)):
                inset = step * (n // 2 - 1) // max(1, u(5) - 1)
                for i in range(inset, n - inset):
                    for k in range(n - u(1.5) + min(step, u(.5)), n + t):
                        v.set(i, j, k, HAIR)
        m = Mesh(self.rig)
        v.emit(m, hidden=head.cells)
        return m

    def headwear(self, kind):
        hd = self.head()
        head, n, t, e, eh = hd["v"], hd["n"], TH, hd["eye"], hd["eh"]
        v = Vox.like(head)
        if kind == "helmet":
            self.cap(v, METAL, 0, front=True)
            for i in range(0, n):                                   # the eye slit, a nose guard left
                for k in range(n, n + t):
                    for j in range(e, e + eh):
                        if abs(i - (n - 1) / 2) > max(.6, R * .4):
                            v.cells.pop((i, j, k), None)
            w = max(1, u(.6))
            for i in range(n // 2 - w // 2, n // 2 - w // 2 + w):   # a crest
                for k in range(-t, n):
                    for j in range(n + t, n + t + u(1.5) - (1 if k in (-t, n - 1) else 0)):
                        v.set(i, j, k, TRIM)
        else:
            self.cap(v, CLOTH, n - u(1), back=-u(1.5), sides=-u(1.5))
            for i in range(-t, n + t):
                for j in range(-u(1.5), n):
                    for k in (n - u(2), n - u(2) + 1):
                        if (i < 0 or i >= n) and (i, j, k) not in head.cells:
                            v.set(i, j, k, CLOTH)
            for i in range(0, n):
                for k in range(n, n + t):
                    for j in range(n - u(1), n + t - 1):
                        if (i, j, k) not in head.cells:
                            v.set(i, j, k, CLOTH)
        m = Mesh(self.rig)
        v.emit(m, hidden=head.cells)
        return m

    def cloak(self):
        rig, t = self.rig, self.torso()
        n, h, q, dims = t["n"], t["h"], t["chest"], t["dims"]
        y0 = t["y"][0]
        hips, knee, foot = rig.p("Hips"), rig.p("LeftLeg"), rig.p("LeftFoot")
        th = max(1, TH - 1)
        m = Mesh(self.rig)
        up = Vox(t["ctr"], X, Y, Z, (V, h, V), {"Spine01": .5, "Spine": .5})
        for j in range(q, n + 1):
            w, d, _ = dims[min(j, n - 1)]
            w += 2 * u(1)
            for i in range(-w // 2, w // 2):
                for k in range(-d // 2 - th - 1, -d // 2 + u(1.5)):
                    if j < n and k >= -d // 2 - 1:
                        continue                              # behind the back only
                    if j == n and k >= -d // 2 and abs(i + .5) < w / 2 - u(1):
                        continue                              # over the shoulders: two straps
                    up.set(i, j, k, CLOAK)
        up.emit(m)
        low_y = (knee[1] + foot[1]) / 2
        rows = max(1, rnd((y0 + q * h - low_y) / V))
        w, d, _ = dims[q]
        w += 2 * u(1)
        lo = Vox(np.array([hips[0], low_y, t["ctr"][2] - V * .5]), X, Y, Z, (V, (y0 + q * h - low_y) / rows, V), "Hips")
        for j in range(rows):
            ww = w + 2 * rnd(u(1) * (1 - j / rows))
            for i in range(-ww // 2, ww // 2):
                if j == 0 and i % 4 == 0:
                    continue                                  # a ragged hem
                for k in range(-d // 2 - th - 1, -d // 2 - 1):
                    lo.set(i, j, k, CLOAK)
        lo.emit(m)
        return m

    def stand(self):
        """The standing figure's height (model units): floor to the top of the bare head."""
        hd = self.head()
        return float(hd["v"].o[1] + hd["n"] * V)
