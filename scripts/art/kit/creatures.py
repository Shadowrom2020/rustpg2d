"""The kit's creatures: animals and monsters sculpted in voxels, one voxel per pixel of the drawn
creature (a model `height` world pixels tall is 2 x height voxels tall).

A creature is an unrigged model: the engine fits one of the rig templates to it (`rig_quadruped`,
`rig_bird`, `rig_flyer`, `rig_blob`, `rig_floating`: the skeleton stretched over the model, then
each joint moved to where the sculpt says its own shoulder, knee or wing root is) and it plays the
template's walk, idle, attack and hurt.

Creatures face +Z, stand on y = 0 and are symmetric in x."""
import math
import os

import numpy as np

from kitlib import HERE, StaticMesh, accessor, qmat, read_glb, rgb

LIBRARY = os.path.normpath(f"{HERE}/../../../crates/editor/assets/starter_library/models")
EYE, WHITE, NOSE = "#221c26", "#efe8d0", "#2a2228"


def shade(h, k):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(v * k))) for v in rgb(h))


class Sculpt:
    """A block of voxels to sculpt in: x in [-nx/2, nx/2), y in [0, ny), z in [-nz/2, nz/2)."""

    def __init__(self, nx, ny, nz):
        nx, nz = nx + nx % 2, nz + nz % 2
        self.n = (nx, ny, nz)
        self.g = np.zeros((nx, ny, nz), np.int16)
        self.pal = [None]
        i, j, k = np.indices((nx, ny, nz))
        self.X, self.Y, self.Z = i + .5 - nx / 2, j + .5, k + .5 - nz / 2

    def col(self, c):
        if c not in self.pal:
            self.pal.append(c)
        return self.pal.index(c)

    def ball(self, c, r, color):
        r = (r, r, r) if np.isscalar(r) else r
        m = ((self.X - c[0]) / r[0]) ** 2 + ((self.Y - c[1]) / r[1]) ** 2 + ((self.Z - c[2]) / r[2]) ** 2 <= 1
        self.g[m] = self.col(color)
        return m

    def limb(self, a, b, ra, rb, color, squash=(1, 1, 1)):
        """A tapered capsule from a (radius ra) to b (radius rb); `squash` flattens it per axis."""
        a, b = np.asarray(a, float), np.asarray(b, float)
        ab = b - a
        P = np.stack([self.X - a[0], self.Y - a[1], self.Z - a[2]], -1)
        t = np.clip((P @ ab) / max(float(ab @ ab), 1e-9), 0, 1)
        d = P - t[..., None] * ab
        dist = np.sqrt((d[..., 0] / squash[0]) ** 2 + (d[..., 1] / squash[1]) ** 2 + (d[..., 2] / squash[2]) ** 2)
        m = dist <= ra + (rb - ra) * t
        self.g[m] = self.col(color)
        return m

    def box(self, lo, hi, color):
        m = (self.X >= lo[0]) & (self.X < hi[0]) & (self.Y >= lo[1]) & (self.Y < hi[1]) & (self.Z >= lo[2]) & (self.Z < hi[2])
        self.g[m] = self.col(color)
        return m

    def paint(self, mask, color, on=None):
        """Recolours the voxels in `mask` (only those of the colours `on`, when given)."""
        m = (self.g > 0) & mask
        if on:
            m &= np.isin(self.g, [self.col(c) for c in on])
        self.g[m] = self.col(color)

    def carve(self, mask):
        self.g[mask] = 0

    def mirror(self):
        self.g = np.where(self.g > 0, self.g, self.g[::-1])

    def bounds(self):
        s = self.g > 0
        return (np.array([self.X[s].min() - .5, self.Y[s].min() - .5, self.Z[s].min() - .5]), np.array([self.X[s].max() + .5, self.Y[s].max() + .5, self.Z[s].max() + .5]))

    def mesh(self):
        """The faces between a voxel and nothing, those of one colour in one plane merged."""
        m = StaticMesh()
        nx, ny, nz = self.n
        off = np.array([-nx / 2, 0.0, -nz / 2])
        solid = self.g > 0
        for u in range(3):
            a, b = [n for n in range(3) if n != u]
            for sign in (1, -1):
                nb = np.zeros_like(solid)
                src = [slice(None)] * 3
                dst = [slice(None)] * 3
                src[u], dst[u] = (slice(1, None), slice(0, -1)) if sign > 0 else (slice(0, -1), slice(1, None))
                nb[tuple(dst)] = solid[tuple(src)]
                face = np.where(solid & ~nb, self.g, 0)
                normal = np.zeros(3)
                normal[u] = sign
                for p in range(self.n[u]):
                    sl = [slice(None)] * 3
                    sl[u] = p
                    plane = face[tuple(sl)].copy()                      # indexed [a][b]
                    for (i, j) in zip(*np.nonzero(plane)):
                        c = plane[i, j]
                        if c == 0:
                            continue
                        w = 1
                        while i + w < plane.shape[0] and plane[i + w, j] == c:
                            w += 1
                        h = 1
                        while j + h < plane.shape[1] and (plane[i:i + w, j + h] == c).all():
                            h += 1
                        plane[i:i + w, j:j + h] = 0
                        corners = []
                        for da, db in ((0, 0), (w, 0), (w, h), (0, h)):
                            q = [0.0, 0.0, 0.0]
                            q[u], q[a], q[b] = p + (1 if sign > 0 else 0), i + da, j + db
                            corners.append(np.array(q) + off)
                        m.quad(self.pal[c], *corners, None, normal)
        return m


# ---------------------------------------------------------------- fitting the rig templates

_templates = {}


def template(name):
    """A rig template's joints at rest and its mesh's bounds, as the engine's fit sees them."""
    if name not in _templates:
        j, b = read_glb(f"{LIBRARY}/{name}.glb")
        nodes = j["nodes"]
        world = [None] * len(nodes)

        def go(i, parent):
            n = nodes[i]
            m = np.eye(4)
            m[:3, :3] = qmat(n.get("rotation", [0, 0, 0, 1])) * np.array(n.get("scale", [1, 1, 1]))
            m[:3, 3] = n.get("translation", [0, 0, 0])
            world[i] = parent @ m
            for c in n.get("children", []):
                go(c, world[i])
        for r in j["scenes"][0]["nodes"]:
            go(r, np.eye(4))
        pos = np.vstack([accessor(j, b, p["attributes"]["POSITION"]) for p in j["meshes"][0]["primitives"]])
        _templates[name] = ({nodes[k]["name"]: world[k][:3, 3] for k in j["skins"][0]["joints"]}, pos.min(0), pos.max(0))
    return _templates[name]


def nudges(name, lo, hi, want):
    """`ModelDef.rig.joints` that put the template's joints where the sculpt wants them. The
    engine stretches the template over the model's bounds (the model centred in x and z, standing
    on y = 0) and then moves a joint by its offset times the model's height."""
    joints, tlo, thi = template(name)
    shift = np.array([-(lo[0] + hi[0]) / 2, -lo[1], -(lo[2] + hi[2]) / 2])
    mlo, mhi = lo + shift, hi + shift
    scale = (mhi - mlo) / np.maximum(thi - tlo, 1e-5)
    height = mhi[1] - mlo[1]
    out = []
    for joint in sorted(want):
        stretched = mlo + (joints[joint] - tlo) * scale
        off = (np.asarray(want[joint], float) + shift - stretched) / height
        if np.abs(off).max() > .004:
            out.append(dict(joint=joint, offset=[round(float(v), 4) for v in off]))
    return out


def sides(want, name, p):
    """A joint of both sides: `name`_L at +x and `name`_R at -x."""
    want[f"{name}_L"] = (abs(p[0]), p[1], p[2])
    want[f"{name}_R"] = (-abs(p[0]), p[1], p[2])


# ---------------------------------------------------------------- four legs

def quadruped(length, back, body, leg=2.5, spread=None, neck=(8, 40, 3.5), head=(5, 5, 6), snout=(7, 2.6, -10), ears=("up", 4), tail=(16, 2.2, -35), fur="#8a8478", belly=None, dark=None,
              paws=None, eye=EYE, nose=NOSE, more=None, k=1.0, room=(0, 0, 0)):
    """A four-legged animal. Sizes in voxels.

    length: chest to rump; back: the height of the back; body: (half width, half height, the
    chest's and the rump's share of them); leg: radius; spread: the legs' x; neck: (length, degrees
    up from level, radius); head: radii; snout: (length, radius, degrees); ears: (kind, size);
    tail: (length, radius, degrees from level, negative = hanging); room: voxels more above, in
    front and to each side, for what `more` adds."""
    # `k` scales the whole animal (the sizes above are for k = 1).
    length, back, leg = length * k, back * k, leg * k
    body = (body[0] * k, body[1] * k) + tuple(body[2:])
    spread = spread * k if spread is not None else None
    neck, head, snout = (neck[0] * k, neck[1], neck[2] * k), tuple(v * k for v in head), (snout[0] * k, snout[1] * k, snout[2])
    ears, tail = (ears[0], ears[1] * k), (tail[0] * k, tail[1] * k, tail[2])
    bw, bh = body[0], body[1]
    chest_k, rump_k = (body + (1.0, .9))[2:4]
    spread = spread if spread is not None else bw * .62
    yc = back - bh
    zf, zb = length / 2 - bw * .8, -length / 2 + bw * .8
    na = math.radians(neck[1])
    neck_a = np.array([0, yc + bh * .35, zf + bw * .3])
    neck_b = neck_a + neck[0] * np.array([0, math.sin(na), math.cos(na)])
    hc = neck_b + np.array([0, head[1] * .15, head[2] * .35])
    sa = math.radians(snout[2])
    snout_b = hc + np.array([0, -head[1] * .25, head[2] * .6]) + snout[0] * np.array([0, math.sin(sa), math.cos(sa)])
    ta = math.radians(tail[2])
    tail_a = np.array([0, yc + bh * .35, zb - bw * .7])
    tail_b = tail_a + tail[0] * np.array([0, math.sin(ta), -math.cos(ta)])
    top = max(back, hc[1] + head[1] + (ears[1] if ears[0] in ("up", "round", "horns") else 0), tail_b[1] + tail[1]) + 3
    front, rear = max(snout_b[2] + snout[1], hc[2] + head[2]) + 2, min(tail_b[2] - tail[1], zb - bw) - 2
    half = int(max(bw, spread + leg * 1.6, head[0] + (ears[1] if ears[0] in ("flop", "horns") else 1))) + 3
    top, front, half = top + room[0], front + room[1], half + room[2]
    zoff = (front + rear) / 2                                            # the block is centred on the animal
    s = Sculpt(2 * half, int(top) + 1, int(front - rear) + 2)

    def z(p):
        return (p[0], p[1], p[2] - zoff)
    belly, dark, paws = belly or shade(fur, 1.18), dark or shade(fur, .72), paws or fur
    # Legs first, so the body covers their tops.
    want = {}
    for zl, name, lean in ((zf, "Front", 0.0), (zb, "Back", -1.0)):
        knee_y = yc * .48
        hipj, knee, ankle = (spread, yc - bh * .15, zl), (spread, knee_y, zl + lean * leg * .9), (spread, leg + .5, zl + lean * leg * .2)
        for sx in (1, -1):
            a, kn, f = [(sx * p[0], p[1], p[2]) for p in (hipj, knee, ankle)]
            s.limb(z(a), z(kn), leg * (1.5 if name == "Back" else 1.25), leg, fur)
            s.limb(z(kn), z(f), leg, leg * .85, fur)
            s.limb(z(f), z((f[0], leg * .7, f[2] + leg * 1.1)), leg * .95, leg * .8, paws)
        sides(want, f"{name}Leg", hipj)
        sides(want, f"{name}Shin", knee)
        sides(want, f"{name}Paw", ankle)
    chest, rump = (0, yc, zf), (0, yc, zb)
    s.limb(z(rump), z(chest), bh * rump_k, bh * chest_k, fur, (bw / bh, 1, 1))
    s.limb(z(neck_a), z(neck_b), neck[2] * 1.25, neck[2], fur)
    s.ball(z(hc), head, fur)
    s.limb(z(hc + np.array([0, -head[1] * .25, head[2] * .5])), z(snout_b), snout[1] * 1.15, snout[1], fur, (1.1, 1, 1))
    s.limb(z(tail_a), z(tail_b), tail[1], tail[1] * .55, fur)
    # The underside lighter.
    s.paint((s.Y < yc - bh * .3) & (s.Y > yc - bh - .5) & (np.abs(s.X) < bw * .8) & (s.Z > zb - zoff - bw) & (s.Z < zf - zoff + bw), belly, [fur])
    s.paint((s.Y < hc[1] - head[1] * .35) & (s.Z > hc[2] - zoff), belly, [fur])
    # Ears.
    kind, e = ears
    for sx in (1, -1):
        base = hc + np.array([sx * head[0] * .62, head[1] * .7, -head[2] * .15])
        if kind == "up":
            s.limb(z(base), z(base + np.array([sx * e * .2, e, -e * .1])), e * .42, .5, dark)
        elif kind == "round":
            s.ball(z(base + np.array([sx * e * .1, e * .3, 0])), (e * .6, e * .6, e * .4), dark)
        elif kind == "flop":
            s.limb(z(base + np.array([sx * head[0] * .2, -head[1] * .2, 0])), z(base + np.array([sx * head[0] * .5, -e, 0])), e * .45, e * .35, dark, (1, 1, .6))
    # The face: eyes and a nose.
    ex, ey, ez = head[0] * .55, hc[1] + head[1] * .2, hc[2] + head[2] * .55 - zoff
    s.paint((np.abs(np.abs(s.X) - ex) < .9 * k) & (np.abs(s.Y - ey) < 1.0 * k) & (s.Z > ez - k), eye)
    s.paint((s.Z > snout_b[2] - zoff - 1.2 * k) & (s.Y > snout_b[1] - snout[1] * .2), nose)
    want.update(Hips=rump, Spine=(0, yc, (zf + zb) / 2), Neck=tuple(neck_a), Head=tuple(neck_b), Tail=tuple(tail_a), TailTip=tuple((tail_a + tail_b) / 2))
    out = dict(s=s, z=z, zoff=zoff, hc=hc, snout=snout_b, yc=yc, zf=zf, zb=zb, bw=bw, bh=bh, tail_a=tail_a, tail_b=tail_b, want={name: z(v) for name, v in want.items()}, fur=fur, dark=dark, belly=belly, k=k, feature=("fur", [fur, belly, dark]))
    if more:
        more(out)
    s.mirror()
    return out


def folded_wings(q, k, arm, web):
    """Wings folded over the back: an arm up and back from each shoulder, a membrane (or the
    feathers) hanging from it."""
    s, z = q["s"], q["z"]
    for sx in (1, -1):
        root = np.array([sx * 5 * k, q["yc"] + q["bh"] * .7, q["zf"] - 2 * k])
        elbow = root + np.array([sx * 9 * k, 15 * k, -6 * k])
        tip = elbow + np.array([sx * 6 * k, -4 * k, -24 * k])
        s.limb(z(root), z(elbow), 1.8 * k, 1.4 * k, arm)
        s.limb(z(elbow), z(tip), 1.4 * k, .8, arm)
        for t in np.linspace(0, 1, int(26 * k)):
            top = elbow + (tip - elbow) * t
            low = root + (np.array([sx * 7 * k, q["yc"] + 2 * k, q["zb"] + 2 * k]) - root) * t
            s.limb(z(top), z(low), .9, .9, web)


def rider(s, at, skin, hair, k=1.0):
    """A person's upper body growing out of an animal at `at` (a centaur's, a drider's). Returns
    where its neck starts and where its head is."""
    y, zz = at[1], at[2]
    s.limb((0, y, zz), (0, y + 20 * k, zz), 6.5 * k, 8 * k, skin, (1, 1, .6))
    for sx in (1, -1):
        s.limb((sx * 8.5 * k, y + 19 * k, zz), (sx * 10 * k, y + 8 * k, zz + 2 * k), 2.3 * k, 1.9 * k, skin)
        s.limb((sx * 10 * k, y + 8 * k, zz + 2 * k), (sx * 9 * k, y + 1 * k, zz + 8 * k), 1.9 * k, 1.6 * k, skin)
    s.limb((0, y + 21 * k, zz), (0, y + 25 * k, zz), 2.4 * k, 2.4 * k, skin)
    hc = (0, y + 30 * k, zz + .5 * k)
    s.ball(hc, 5 * k, skin)
    near = (s.X ** 2 + (s.Y - hc[1]) ** 2 + (s.Z - hc[2]) ** 2) < (5.6 * k) ** 2
    s.paint(near & ((s.Y > hc[1] + 2.2 * k) | (s.Z < hc[2] - 1.5 * k)), hair)
    s.paint((np.abs(np.abs(s.X) - 2 * k) < .8 * k) & (np.abs(s.Y - hc[1] - .3 * k) < .8 * k) & (s.Z > hc[2] + 3 * k), EYE)
    return (0, y + 22 * k, zz), hc


def finish(q, rig):
    """The mesh of a sculpt and the rig fit that goes into its model definition."""
    lo, hi = q["s"].bounds()
    return q["s"].mesh(), dict(template=rig, joints=nudges(rig, lo, hi, q["want"]))


# ---------------------------------------------------------------- the animals with four legs

def wolf():
    return quadruped(50, 36, (8, 9, 1.0, .85), leg=2.9, neck=(10, 42, 4.6), head=(6, 6, 6.5), snout=(9, 3, -12), ears=("up", 5), tail=(20, 2.8, -40), fur="#8d8a84", belly="#c9c5ba", dark="#5e5b58")


def dog():
    return quadruped(38, 27, (6.5, 7, 1.0, .9), leg=2.3, neck=(7, 45, 3.8), head=(5, 5, 5.5), snout=(6, 2.6, -8), ears=("flop", 5), tail=(12, 1.8, 40), fur="#a8763e", belly="#d9b98a", dark="#6e4a26")


def cat():
    return quadruped(26, 17, (4, 4.5, .95, .95), leg=1.6, neck=(4, 50, 2.6), head=(4, 3.8, 4), snout=(2, 1.6, -5), ears=("up", 3), tail=(16, 1.3, 62), fur="#8a8f98", belly="#c9ccd2", dark="#5a5e66")


def rat():
    return quadruped(26, 12, (5, 5, .9, 1.0), leg=1.4, neck=(2, 12, 3.4), head=(3.6, 3.4, 4.5), snout=(4, 1.6, -6), ears=("round", 2.6), tail=(24, 1.0, -8), fur="#6e6258", belly="#a89a8c", dark="#c9a09a",
                     more=lambda q: q["s"].limb(q["z"](q["tail_a"]), q["z"](q["tail_b"]), 1.0, .6, "#c9a09a"))


def bear(owl=False):
    def more(q):
        s, hc, sn = q["s"], q["hc"], q["snout"]
        if owl:
            s.paint((s.Z > hc[2] - q["zoff"] - 2) & (s.Y > hc[1] - 4), "#b8a078", [q["fur"], q["belly"]])                      # a feathered face
            s.paint((s.Z > sn[2] - q["zoff"] - 7) & (s.Y < sn[1] + 3.5) & (np.abs(s.X) < 3.4), "#e0a030")                       # the beak
            s.paint((np.abs(np.abs(s.X) - 4.6) < 1.5) & (np.abs(s.Y - hc[1] - 1.6) < 1.5) & (s.Z > hc[2] - q["zoff"] + 4), "#f2c84b")
            s.paint((np.abs(np.abs(s.X) - 4.6) < .7) & (np.abs(s.Y - hc[1] - 1.6) < .7) & (s.Z > hc[2] - q["zoff"] + 4), EYE)
    return quadruped(46, 38, (11, 12, 1.0, .95), leg=4.2, neck=(5, 28, 8), head=(7, 6.5, 7), snout=(5, 3.2, -8), ears=("round", 3), tail=(3, 2.4, -30), fur="#6b4a32", belly="#8a6646", dark="#4a3222", k=1.3, more=more)


def boar():
    def more(q):
        s, z, sn, k = q["s"], q["z"], q["snout"], q["k"]
        for sx in (1, -1):                                  # tusks
            s.limb(z((sx * 2.6 * k, sn[1] - k, sn[2] - 2 * k)), z((sx * 3.4 * k, sn[1] + 3 * k, sn[2] - k)), .9 * k, .5, WHITE)
        s.paint((np.abs(s.X) < 1.6 * k) & (s.Y > q["yc"] + q["bh"] * .75), "#3a302a", [q["fur"]])       # a bristly ridge
    return quadruped(36, 26, (8, 9, 1.05, .9), leg=2.4, neck=(3, 6, 7), head=(6, 6, 7), snout=(7, 3.2, -14), ears=("up", 3), tail=(6, 1, 25), fur="#5a4a40", belly="#74625a", dark="#3a302a", more=more, k=1.3)


def sheep():
    def more(q):
        s, z = q["s"], q["z"]
        rng = np.random.default_rng(7)
        for _ in range(60):                                 # tufts of wool all over the body
            c = (rng.uniform(-9, 9), q["yc"] + rng.uniform(-7, 9), rng.uniform(q["zb"] - 6, q["zf"] + 6))
            s.ball(z(c), rng.uniform(3, 4.8), "#e8e4d8" if rng.random() < .8 else "#d2cdbf")
        s.ball(z(q["hc"]), (4, 4.5, 5), "#3a3436")
        s.limb(z(q["hc"]), z(q["snout"]), 2.6, 2.2, "#3a3436")
        s.paint((np.abs(np.abs(s.X) - 2.2) < .9) & (np.abs(s.Y - q["hc"][1] - .8) < 1.0) & (s.Z > q["hc"][2] - q["zoff"] + 2.2), WHITE)
    return quadruped(40, 32, (9, 9.5, 1.0, 1.0), leg=2.3, neck=(4, 30, 4.5), head=(4, 4.5, 5), snout=(3, 2.4, -12), ears=("flop", 3), tail=(4, 2, -60), fur="#3a3436", belly="#3a3436", dark="#3a3436", eye="#3a3436", more=more, k=1.0)


def cow():
    def more(q):
        s, z, hc = q["s"], q["z"], q["hc"]
        rng = np.random.default_rng(3)
        for _ in range(9):                                  # patches
            c = (rng.uniform(-15, 15), q["yc"] + rng.uniform(-5, 16), rng.uniform(q["zb"] - 8, q["zf"] + 8))
            r = rng.uniform(7, 12)
            s.paint(((s.X - c[0]) ** 2 + (s.Y - c[1]) ** 2 + (s.Z - c[2] + q["zoff"]) ** 2) < r * r, "#3a3030", [q["fur"]])
        for sx in (1, -1):                                  # horns
            s.limb(z((sx * 5.5, hc[1] + 7, hc[2] - 3)), z((sx * 11, hc[1] + 12, hc[2] - 1)), 1.7, .7, WHITE)
        s.ball(z((0, q["yc"] - q["bh"] - 1, q["zb"] + 8)), (5.5, 4, 7), "#e0a8a0")                 # the udder
        s.paint((s.Z > q["snout"][2] - q["zoff"] - 4.5) & (s.Y < q["snout"][1] + 4), "#e0a8a0")
        s.limb(z(q["tail_b"]), z(q["tail_b"] - np.array([0, 5, 0])), 3, 2.2, "#3a3030")             # the tail's tuft
    return quadruped(78, 60, (15, 16, 1.0, 1.0), leg=4.2, neck=(10, 18, 8.5), head=(8, 8.5, 10), snout=(8, 5.5, -25), ears=("flop", 5), tail=(34, 2, -80), fur="#e8e2d4", belly="#e8e2d4", dark="#e8e2d4",
                     nose="#e0a8a0", more=more)


def horse(wings=False, horn=False):
    def more(q):
        s, z = q["s"], q["z"]
        if wings:
            folded_wings(q, 1.3, "#e6e0d2", "#f4f0e6")
        if horn:
            s.limb(z(q["hc"] + np.array([0, 5, 5])), z(q["hc"] + np.array([0, 17, 11])), 1.5, .5, "#f2d98a")
        a, b = np.array(q["want"]["Neck"]) + [0, 0, q["zoff"]], np.array(q["want"]["Head"]) + [0, 0, q["zoff"]]
        s.limb(z(a + [0, 7, -4]), z(b + [0, 7, -4]), 3.2, 2.6, "#3a2a1c", (.6, 1, 1))              # the mane
        s.limb(z(q["tail_a"]), z(q["tail_b"]), 3.4, 4.2, "#3a2a1c")
    return quadruped(72, 70, (12, 13.5, 1.0, 1.0), leg=4, neck=(24, 55, 7.2), head=(6.5, 7, 9), snout=(11, 4.8, -38), ears=("up", 4.5), tail=(34, 3.2, -72), fur="#8a5a34", belly="#8a5a34", dark="#3a2a1c",
                     paws="#2a2420", more=more, room=(14 if wings or horn else 0, 4 if horn else 0, 14 if wings else 0))


def big_cat(mane=False, stripes=False, wings=False):
    def more(q):
        s, z, hc, k = q["s"], q["z"], q["hc"], q["k"]
        if wings:
            folded_wings(q, 1.0 * k, "#8a6a4a", "#b89a6a")
        if mane:
            for a in np.linspace(0, 2 * math.pi, 14, endpoint=False):
                s.ball(z((math.cos(a) * 6.5 * k, hc[1] + (math.sin(a) * 6.5 - 1) * k, hc[2] - 4 * k)), 4.2 * k, "#8a5a2a")
            s.ball(z(hc + np.array([0, 0, 1.5 * k])), (5.6 * k, 5.4 * k, 5.6 * k), q["fur"])
            s.paint((np.abs(np.abs(s.X) - 3.3 * k) < .9 * k) & (np.abs(s.Y - hc[1] - 1.1 * k) < 1.0 * k) & (s.Z > hc[2] - q["zoff"] + 4 * k), EYE)
        if stripes:
            s.paint((np.sin(s.Z * .9 / k + s.Y * .35 / k) > .55) & (s.Y > q["yc"] - q["bh"] * .2), "#2a2224", [q["fur"]])
    return quadruped(50, 31, (7.5, 8, 1.0, .9), leg=3, neck=(6, 30, 5.5), head=(6, 5.5, 6), snout=(4, 3, -8), ears=("round", 2.6), tail=(26, 1.7, -22), fur="#c79a56", belly="#e6d2a8", dark="#9a7238",
                     more=more, k=1.35, room=(16, 0, 12) if wings else (0, 0, 0))


def dragon(k=1.0, wings=True):
    """A dragon on four legs: long neck and tail, horns, a crest and folded wings. `k` scales it
    (a wyrmling at 1). Without wings: a drake, a hydra, a sea monster."""
    def more(q):
        s, z, hc = q["s"], q["z"], q["hc"]
        fur, belly = q["fur"], q["belly"]
        for sx in (1, -1):
            s.limb(z(hc + np.array([sx * 3 * k, 3 * k, -3 * k])), z(hc + np.array([sx * 5 * k, 8 * k, -10 * k])), 1.5 * k, .5, "#e8e2cf")      # horns
        if wings:
            folded_wings(q, k, fur, belly)
        for t in np.linspace(.05, .95, int(9 * k)):                                              # a crest down the back
            p = np.array([0, q["yc"] + q["bh"], q["zf"]]) * (1 - t) + np.array([0, q["yc"] + q["bh"] * .8, q["zb"]]) * t
            s.limb(z(p), z(p + np.array([0, 3 * k, -1 * k])), 1.0 * k, .4, "#e8e2cf")
        s.paint((np.abs(np.abs(s.X) - 3.2 * k) < .9 * k) & (np.abs(s.Y - hc[1] - 1.2 * k) < .9 * k) & (s.Z > hc[2] - q["zoff"] + 3 * k), "#f2c84b")
    q = quadruped(46 * k, 34 * k, (8 * k, 8.5 * k, 1.0, .9), leg=3.2 * k, neck=(16 * k, 48, 4.6 * k), head=(4.6 * k, 4.4 * k, 6 * k), snout=(8 * k, 3 * k, -8), ears=("none", 0),
                     tail=(34 * k, 4.2 * k, -18), fur="#b8433a", belly="#d9805e", dark="#8a2e28", eye="#f2c84b", nose="#8a2e28", more=more)
    q["feature"] = ("skin", q["feature"][1])
    return q


# ---------------------------------------------------------------- other body plans

def spider(body="#3a3038", mark="#8a2e3a", claws=False, sting=False, drow=False):
    """A spider; with claws a crab, with a sting too a scorpion, with a drow's body a drider."""
    s = Sculpt(64, 62 if drow else 40, 72)
    ab, th = (0, 15, -10), (0, 12, 8)
    s.ball(ab, (10, 9, 13), body)
    s.ball(th, (7, 6, 8), body)
    s.paint((np.abs(s.X) < 2.5) & (s.Y > 20) & (s.Z < -4), mark)
    s.ball((0, 11, 15), (4, 3.5, 4), body)
    want = dict(Hips=ab, Spine=(0, 14, 0), Neck=(0, 13, 10), Head=(0, 12, 15), Tail=(0, 15, -16), TailTip=(0, 14, -21))
    for k, zl in enumerate((14, 9, 4, -1)):                  # four legs a side: out and up to a knee, then down
        for sx in (1, -1):
            root = (sx * 5, 12, zl)
            knee = (sx * (15 + k), 21 - k, zl + (6 - 4 * k))
            foot = (sx * (25 + k * 1.5), .8, zl + (12 - 8 * k))
            s.limb(root, knee, 1.7, 1.3, body)
            s.limb(knee, foot, 1.3, .8, body)
        if k in (0, 3):
            name = "Front" if k == 0 else "Back"
            sides(want, f"{name}Leg", (5, 12, zl))
            sides(want, f"{name}Shin", (15 + k, 21 - k, zl + (6 - 4 * k)))
            sides(want, f"{name}Paw", (25 + k * 1.5, 2, zl + (12 - 8 * k)))
    for sx in (1, -1):                                       # fangs and eyes
        s.limb((sx * 1.6, 9.5, 18), (sx * 1.2, 6, 19.5), 1.0, .5, "#1e181c")
    s.paint((np.abs(np.abs(s.X) - 1.5) < .9) & (np.abs(s.Y - 12.5) < .9) & (s.Z > 16.4), "#d94040")
    s.paint((np.abs(np.abs(s.X) - 3.2) < .7) & (np.abs(s.Y - 13.4) < .7) & (s.Z > 15.4), "#d94040")
    for sx in (1, -1) if claws else ():                      # an arm and its pincer
        s.limb((sx * 4, 11, 15), (sx * 10, 10, 23), 2, 1.7, body)
        s.ball((sx * 10, 10, 26), (2.8, 2.4, 3.4), body)
        s.limb((sx * 10, 10, 27), (sx * 8, 10, 33), 1.5, .6, body)
        s.limb((sx * 10, 10, 27), (sx * 12.5, 10, 33), 1.5, .6, body)
    if sting:                                                # the tail arching over the back
        for a, b, r in (((0, 16, -20), (0, 27, -27), 2.8), ((0, 27, -27), (0, 34, -19), 2.3), ((0, 34, -19), (0, 31, -12), 1.8)):
            s.limb(a, b, r, r * .85, body)
        s.limb((0, 31, -12), (0, 27, -9), 1.4, .4, "#1e181c")
        want.update(Tail=(0, 16, -20), TailTip=(0, 30, -24))
    if drow:
        neck, _ = rider(s, (0, 15, 9), "#4a4458", "#dedede")
        want.update(Neck=(0, 25, 9), Head=neck)
    s.mirror()
    return dict(s=s, want=want, feature=("body", [body]))


def chicken():
    s = Sculpt(26, 40, 40)
    white, red, yellow = "#ece6d6", "#c8322c", "#e0a030"
    s.ball((0, 15, -1), (7, 7.5, 10), white)
    s.limb((0, 17, -9), (0, 27, -15), 4.5, 1.5, "#d8d0bc", (.5, 1, 1))                            # tail feathers
    s.limb((0, 18, 6), (0, 29, 9), 4.2, 3, white)
    s.ball((0, 31, 10), 3.6, white)
    s.limb((0, 34, 9), (0, 36.5, 11), 1.6, 1.0, red, (.5, 1, 1))                                   # comb
    s.limb((0, 30.5, 13), (0, 30, 16), 1.3, .5, yellow)                                            # beak
    s.ball((0, 27.5, 12.5), (1.1, 1.8, 1.1), red)
    s.paint((np.abs(np.abs(s.X) - 2.6) < .8) & (np.abs(s.Y - 32) < .8) & (s.Z > 10.5), EYE)
    for sx in (1, -1):
        s.ball((sx * 6.2, 15.5, -2), (2, 5, 7.5), "#d8d0bc")                                        # wings
        s.limb((sx * 3, 9, 0), (sx * 3, 1.5, 0), 1.1, .9, yellow)
        s.limb((sx * 3, 1, -1), (sx * 3, .8, 4), 1.0, .8, yellow)
    s.mirror()
    want = dict(Hips=(0, 14, -3), Chest=(0, 18, 3), Neck=(0, 23, 7), Head=(0, 29, 9), Tail=(0, 17, -8))
    sides(want, "Wing", (6, 19, 3))
    sides(want, "WingTip", (6.5, 14, -6))
    sides(want, "Thigh", (3, 10, 0))
    sides(want, "Shank", (3, 5, 0))
    sides(want, "Toe", (3, 1, 0))
    return dict(s=s, want=want, feature=("feathers", [white, "#d8d0bc"]))


def flyer(kind="bat"):
    """A flying animal upright in the air, its wings spread: a bat, or a bird of prey."""
    bat, wasp = kind == "bat", kind == "wasp"
    span = dict(bat=30, bird=34, wasp=19)[kind]
    s = Sculpt(2 * span + 4, 40, 16)
    body, wing = dict(bat=("#4a3a4a", "#6a4a5a"), bird=("#6e4a2e", "#553820"), wasp=("#e0b030", "#cfe0e8"))[kind]
    s.ball((0, 14, 0), (4, 6.5, 3.6), body)
    s.ball((0, 23, 1), (3.6, 3.6, 3.4), body if bat else "#e6dcc8")
    if bat:
        for sx in (1, -1):
            s.limb((sx * 2.4, 25, 0), (sx * 4, 31, 0), 1.6, .5, body)                              # big ears
        s.paint((np.abs(s.X) < 1.3) & (np.abs(s.Y - 21.6) < .7) & (s.Z > 3), WHITE)                # teeth
    elif wasp:
        s.paint((np.sin(s.Y * 1.5) > .1) & (s.Y < 19), "#2a2228", [body])                          # stripes
        s.limb((0, 8, 0), (0, 2.5, -1), 1.2, .4, "#2a2228")                                         # the sting
        for sx in (1, -1):
            s.limb((sx * 1.6, 26, 2), (sx * 4, 31, 4), .7, .5, "#2a2228")
    else:
        s.limb((0, 22.5, 4), (0, 21, 7), 1.4, .5, "#e0a030")                                        # beak
        s.limb((0, 9, -1), (0, 2, -3), 3, 4.5, wing, (1, 1, .35))                                   # tail
    s.paint((np.abs(np.abs(s.X) - 1.7) < .8) & (np.abs(s.Y - 23.8) < .8) & (s.Z > 3), "#d94040" if bat else EYE)
    for sx in (1, -1):
        root, mid, tip = np.array([sx * 3, 19, 0]), np.array([sx * span * .55, 23, 0]), np.array([sx * span, 17, 0])
        s.limb(root, mid, 1.3, 1.0, body)
        s.limb(mid, tip, 1.0, .6, body)
        for t in np.linspace(0, 1, 2 * span):                  # the membrane or the feathers below the arm
            top = root + (mid - root) * min(1, t * 1.8) if t < .556 else mid + (tip - mid) * ((t - .556) / .444)
            drop = dict(bat=10, bird=12, wasp=7)[kind] * math.sin(math.pi * min(1, t * 1.05)) ** .6
            scallop = 2.2 * abs(math.sin(t * math.pi * (4 if bat else 9))) if t > .05 and not wasp else 0
            s.limb(top, top - np.array([0, drop - scallop, 0]), .8, .8, wing)
        s.limb((sx * 1.5, 8, 0), (sx * 1.8, 3.5, 1), .8, .6, body if bat else "#e0a030")           # feet
    s.mirror()
    want = dict(Hips=(0, 10, 0), Chest=(0, 17, 0), Head=(0, 22, 1))
    sides(want, "Wing", (3, 19, 0))
    sides(want, "WingTip", (span * .55, 23, 0))
    sides(want, "Foot", (1.6, 8, 0))
    return dict(s=s, want=want, feature=("fur" if bat else "body" if wasp else "feathers", [body] if wasp else [body, wing]))


def blob(color="#4fbf6a", cube=False):
    s = Sculpt(44, 30, 44)
    if cube:
        s.box((-18, 0, -18), (18, 28, 18), color)
        e = (np.abs(np.abs(s.X) - 17.5) < 1) | (np.abs(np.abs(s.Z) - 17.5) < 1)
        s.paint(((np.abs(np.abs(s.X) - 17.5) < 1) & (np.abs(np.abs(s.Z) - 17.5) < 1)) | (e & ((s.Y > 27) | (s.Y < 1))), shade(color, .8))
        s.paint((np.abs(s.X + 5) < 3) & (np.abs(s.Y - 9) < 4) & (s.Z > 17), "#e8e2cf")               # a bone inside
    else:
        s.ball((0, 6, 0), (19, 15, 19), color)
        s.carve(s.Y < 0)
        s.paint(s.Y < 3, shade(color, .78))
        s.paint(((s.X + 7) ** 2 + (s.Y - 15) ** 2 < 14) & (s.Z > 4), shade(color, 1.3))             # a highlight
        s.paint((np.abs(np.abs(s.X) - 5) < 1.2) & (np.abs(s.Y - 9.5) < 1.6) & (s.Z > 12), EYE)
    s.mirror() if cube else None
    return dict(s=s, want=dict(Hips=(0, 0, 0), Body=(0, 8, 0), Top=(0, 16, 0)), feature=("body", [color, shade(color, .78), shade(color, 1.3), shade(color, .8)]))


def ghost(color="#dfe8ee", eyes=EYE):
    s = Sculpt(44, 66, 26)
    s.limb((0, 34, 0), (0, 8, 0), 9, 6, color, (1, 1, .7))                                          # the shroud
    s.limb((0, 44, 0), (0, 34, 0), 7.5, 9, color, (1, 1, .7))
    s.ball((0, 53, 1), (6.5, 7.5, 6), color)
    for sx in (1, -1):
        s.limb((sx * 8, 42, 1), (sx * 17, 31, 5), 3, 2, color)
        s.limb((sx * 17, 31, 5), (sx * 18, 26, 8), 2, 1.2, color)
    s.carve((s.Y < 12) & (np.sin(s.X * 1.1) + np.sin(s.Z * 1.3) > .5 + (s.Y - 4) * .12))            # a ragged hem
    s.carve((s.Y < 5))
    s.paint(s.Y < 18, shade(color, .86))
    s.paint((np.abs(np.abs(s.X) - 2.6) < 1.3) & (np.abs(s.Y - 54.5) < 1.8) & (s.Z > 4), eyes)
    s.paint((np.abs(s.X) < 1.6) & (np.abs(s.Y - 49.5) < 1.6) & (s.Z > 4.5), eyes)
    s.mirror()
    want = dict(Hips=(0, 26, 0), Chest=(0, 38, 0), Head=(0, 47, 0), Trail=(0, 22, 0), TrailTip=(0, 12, 0))
    sides(want, "Limb", (8, 42, 1))
    return dict(s=s, want=want, feature=("body", [color, shade(color, .86)]))


def elemental(kind):
    """A whirl of flame, water, air or stone with two arms."""
    s = Sculpt(52, 70, 34)
    colors = dict(fire=("#e0522a", "#f2903c", "#f6d860"), water=("#2f6fb0", "#4f9ad0", "#bfe2f2"), air=("#b8c6d2", "#dfe8ee", "#f6fafc"), earth=("#6e5a44", "#8a7458", "#a89678"))[kind]
    rng = np.random.default_rng(dict(fire=1, water=2, air=3, earth=4)[kind])
    if kind == "earth":
        for c, r in (((0, 12, 0), (10, 12, 8)), ((0, 32, 0), (13, 12, 9)), ((0, 50, 1), (7, 7, 6.5))):
            s.ball(c, r, colors[1])
        for sx in (1, -1):
            s.limb((sx * 12, 38, 0), (sx * 20, 22, 3), 5, 6, colors[1])
            s.limb((sx * 5, 12, 0), (sx * 6, 2, 0), 5, 5.5, colors[0])
        for _ in range(40):                                  # facets and cracks
            c = (rng.uniform(-14, 14), rng.uniform(2, 56), rng.uniform(-9, 9))
            s.paint(((s.X - c[0]) ** 2 + (s.Y - c[1]) ** 2 + (s.Z - c[2]) ** 2) < rng.uniform(4, 16), colors[0] if rng.random() < .6 else colors[2])
        s.paint((np.abs(np.abs(s.X) - 2.8) < 1.2) & (np.abs(s.Y - 51.5) < 1.2) & (s.Z > 4), "#f6d860")
    else:
        for t in np.linspace(0, 1, 26):                      # a column of blobs that swirls as it rises
            y = 3 + 56 * t
            r = (12 * (1 - t) ** .7 + 3) if kind != "air" else (4 + 10 * math.sin(math.pi * t))
            a = t * (9 if kind == "air" else 5)
            s.ball((math.sin(a) * 3 * (1 - t), y, math.cos(a) * 2 * (1 - t)), (r, r * .8, r * .75), colors[0] if t < .45 else colors[1])
        for _ in range(26):
            c = (rng.uniform(-9, 9), rng.uniform(4, 58), rng.uniform(-3, 9))
            s.paint(((s.X - c[0]) ** 2 + (s.Y - c[1]) ** 2 + (s.Z - c[2]) ** 2) < rng.uniform(5, 14), colors[2] if c[1] > 24 else colors[1])
        for sx in (1, -1):
            s.limb((sx * 8, 34, 0), (sx * 19, 40 if kind == "fire" else 28, 4), 3.4, 1.6, colors[1])
        s.paint((np.abs(np.abs(s.X) - 2.6) < 1.2) & (np.abs(s.Y - 44) < 1.4) & (s.Z > 2), "#fff6d0" if kind == "fire" else "#1e2a3a")
    s.mirror()
    want = dict(Hips=(0, 20, 0), Chest=(0, 34, 0), Head=(0, 46, 0), Trail=(0, 16, 0), TrailTip=(0, 7, 0))
    sides(want, "Limb", (9, 36, 0))
    return dict(s=s, want=want)


def wisp():
    s = Sculpt(14, 30, 14)
    s.ball((0, 22, 0), 5.5, "#bfe2f2")
    s.ball((0, 22, 0), 3.2, "#f6fafc")
    s.limb((0, 17, 0), (0, 8, -1), 2.4, .6, "#bfe2f2")
    return dict(s=s, want=dict(Hips=(0, 16, 0), Chest=(0, 20, 0), Head=(0, 23, 0), Trail=(0, 14, 0), TrailTip=(0, 9, 0), Limb_L=(4, 22, 0), Limb_R=(-4, 22, 0)))


def deer(goat=False):
    def more(q):
        s, z, hc = q["s"], q["z"], q["hc"]
        for sx in (1, -1):
            a = hc + np.array([sx * 2.4, 4, -2])
            if goat:                                        # horns curving back, and a beard
                s.limb(z(a), z(a + np.array([sx * 1.5, 6, -4])), 1.3, 1.0, "#8a8070")
                s.limb(z(a + np.array([sx * 1.5, 6, -4])), z(a + np.array([sx * 3, 5, -10])), 1.0, .5, "#8a8070")
            else:                                           # antlers
                b = a + np.array([sx * 5, 10, -3])
                s.limb(z(a), z(b), 1.0, .8, "#d8cfae")
                s.limb(z(a + (b - a) * .5), z(a + (b - a) * .5 + np.array([sx * 1, 6, 4])), .8, .5, "#d8cfae")
                s.limb(z(b), z(b + np.array([-sx * 2, 5, 3])), .8, .5, "#d8cfae")
                s.limb(z(b), z(b + np.array([sx * 3, 3, -3])), .8, .5, "#d8cfae")
        if goat:
            s.limb(z(q["snout"] + np.array([0, -3, -3])), z(q["snout"] + np.array([0, -8, -4])), 1.4, .6, q["belly"])
        else:
            s.paint((s.Z < q["zb"] - q["zoff"] - 4) & (s.Y > q["yc"] - 2), WHITE, [q["fur"]])                                  # a white rump
    if goat:
        return quadruped(36, 30, (7, 8, 1.0, 1.0), leg=2.1, neck=(8, 50, 4), head=(4.2, 4.6, 5.5), snout=(5, 2.4, -20), ears=("flop", 3.5), tail=(4, 1.4, 50), fur="#d8d2c4", belly="#eee8dc", dark="#8a8478",
                         more=more, room=(8, 0, 4))
    return quadruped(50, 46, (7.5, 9, 1.0, .95), leg=2.3, neck=(16, 58, 4.2), head=(4.5, 5, 6), snout=(7, 2.6, -20), ears=("up", 4), tail=(4, 1.6, 30), fur="#a8743e", belly="#e0cba0", dark="#7a5228",
                     more=more, room=(18, 4, 8))


def elephant():
    def more(q):
        s, z, hc = q["s"], q["z"], q["hc"]
        s.limb(z(hc + np.array([0, -3, 10])), z(hc + np.array([0, -22, 14])), 5, 3.4, q["fur"])                               # the trunk
        s.limb(z(hc + np.array([0, -22, 14])), z(hc + np.array([0, -33, 18])), 3.4, 2.4, q["fur"])
        for sx in (1, -1):
            s.ball(z(hc + np.array([sx * 12, 1, -4])), (2.6, 11, 9), q["dark"])                                                # ears
            s.limb(z(hc + np.array([sx * 5, -7, 9])), z(hc + np.array([sx * 7, -15, 22])), 2, .8, WHITE)                       # tusks
    return quadruped(70, 78, (19, 21, 1.0, 1.0), leg=7, neck=(6, 15, 13), head=(11, 12, 12), snout=(3, 6, -50), ears=("none", 0), tail=(26, 1.6, -82), fur="#8a8a90", belly="#9a9aa0", dark="#6e6e76",
                     paws="#6e6e76", nose="#8a8a90", more=more, room=(0, 16, 4))


def rhino(frill=False):
    def more(q):
        s, z, hc, sn = q["s"], q["z"], q["hc"], q["snout"]
        s.limb(z(sn + np.array([0, 2, -4])), z(sn + np.array([0, 17, 2])), 3.2, .7, "#efe8d0")                                 # the horn
        if frill:                                           # a bone frill behind the head, two more horns
            s.ball(z(hc + np.array([0, 5, -7])), (13, 13, 3), q["dark"])
            for sx in (1, -1):
                s.limb(z(hc + np.array([sx * 4, 5, 2])), z(hc + np.array([sx * 6, 13, 13])), 1.8, .5, "#d8cfae")
        else:
            s.limb(z(sn + np.array([0, 3, -10])), z(sn + np.array([0, 10, -9])), 2.4, .7, "#efe8d0")
    return quadruped(64, 52, (15, 16, 1.0, 1.0), leg=6, neck=(4, -14, 12), head=(8, 8.5, 10), snout=(12, 6, -14), ears=("up", 3.5), tail=(14, 1.4, -75), fur="#8f8c86", belly="#a5a29c", dark="#6e6b66",
                     nose="#8f8c86", more=more, room=(20, 8, 6))


def crocodile():
    def more(q):
        s = q["s"]
        s.paint((np.sin(s.Z * .8) > .3) & (np.abs(s.X) < 6) & (s.Y > q["yc"] + q["bh"] * .55), q["dark"], [q["fur"]])          # ridges down the back
        s.paint((np.abs(s.Y - q["snout"][1] + 1) < .7) & (s.Z > q["hc"][2] - q["zoff"] + 3) & (np.floor(s.Z) % 2 == 0), WHITE)  # teeth
    return quadruped(60, 16, (10, 6.5, 1.0, 1.0), leg=3, spread=11, neck=(4, 0, 6), head=(6, 4, 7), snout=(16, 3.4, 0), ears=("none", 0), tail=(44, 6, -6), fur="#4f6a3a", belly="#c9c890", dark="#34482a",
                     eye="#f2c84b", nose="#34482a", more=more)


def frog():
    def more(q):
        s, z, hc = q["s"], q["z"], q["hc"]
        for sx in (1, -1):                                  # eyes on top of the head
            s.ball(z(hc + np.array([sx * 4.5, 5, 1])), 2.6, q["fur"])
            s.ball(z(hc + np.array([sx * 5.2, 5.4, 2.2])), 1.5, "#f2c84b")
            s.ball(z(hc + np.array([sx * 5.8, 5.4, 3])), .8, EYE)
        s.paint((np.abs(s.Y - hc[1] + 1.5) < .6) & (s.Z > hc[2] - q["zoff"] + 2), q["dark"])                                  # the mouth
    return quadruped(22, 15, (9, 8, .8, 1.1), leg=2.6, spread=9, neck=(1, 20, 7), head=(7.5, 5.5, 6), snout=(2, 4, 0), ears=("none", 0), tail=(1, 1, 0), fur="#5a9a3a", belly="#d8dc9a", dark="#3a6a26",
                     eye="#5a9a3a", nose="#5a9a3a", more=more, room=(6, 0, 0))


def ape():
    def more(q):
        s, hc = q["s"], q["hc"]
        s.paint((s.Z > hc[2] - q["zoff"] + 2.5) & (s.Y < hc[1] + 3) & (np.abs(s.X) < 4.6), "#6a5a52")                         # the bare face
        s.paint((np.abs(np.abs(s.X) - 2.4) < .9) & (np.abs(s.Y - hc[1] - 1) < .9) & (s.Z > hc[2] - q["zoff"] + 4), EYE)
    return quadruped(30, 42, (11, 12, 1.15, .8), leg=4.2, neck=(4, 60, 7), head=(6.5, 7, 6.5), snout=(3, 3.6, -15), ears=("round", 2), tail=(1, 1, 0), fur="#3a3230", belly="#5a4e48", dark="#2a2422",
                     nose="#3a3230", more=more)


def griffon():
    def more(q):
        s, hc = q["s"], q["hc"]
        folded_wings(q, 1.05 * q["k"], "#8a6a4a", "#b89a6a")
        s.paint((s.Z > q["zf"] - q["zoff"] + 2) & (s.Y > q["yc"] + 2), "#ece6d6", [q["fur"], q["belly"], q["dark"]])           # an eagle's white head and neck
        s.paint((s.Z > q["snout"][2] - q["zoff"] - 7) & (s.Y < hc[1] + 1), "#e0a030")                                           # the beak
    return quadruped(52, 40, (9, 10, 1.0, .9), leg=3.4, neck=(10, 50, 5.5), head=(5.5, 5.5, 6), snout=(6, 2.6, -25), ears=("up", 3), tail=(26, 1.6, -25), fur="#b88a4a", belly="#e6d2a8", dark="#8a6230",
                     nose="#e0a030", more=more, k=1.3, room=(18, 0, 14))


def centaur():
    def more(q):
        s, z = q["s"], q["z"]
        at = z((0, q["yc"] + q["bh"] * .55, q["zf"] + 4))
        neck, _ = rider(s, at, "#d09068", "#5d3a1a")
        q["want"]["Neck"], q["want"]["Head"] = (0, at[1] + 10, at[2]), neck
        s.limb(z(q["tail_a"]), z(q["tail_b"]), 3.2, 4, "#3a2a1c")
    return quadruped(62, 60, (11.5, 13, 1.0, 1.0), leg=3.8, neck=(1, 80, 5), head=(3, 3, 3), snout=(1, 1.5, 0), ears=("none", 0), tail=(30, 3, -72), fur="#8a5a34", belly="#8a5a34", dark="#3a2a1c",
                     paws="#2a2420", eye="#8a5a34", nose="#8a5a34", more=more, room=(44, 6, 4))


# ---------------------------------------------------------------- snakes, worms and what swims

def snake(color="#5a8a3a", hood=False):
    """A coiled snake, its head raised."""
    s = Sculpt(44, 40, 44)
    belly, dark = shade(color, 1.4), shade(color, .68)
    for r, y, rad in ((13, 3.6, 3.7), (9.5, 9.4, 3.4)):      # two coils
        for a in np.linspace(0, 2 * math.pi, 48, endpoint=False):
            s.ball((math.cos(a) * r, y, math.sin(a) * r), rad, color)
    ang = np.arctan2(s.Z, s.X)
    s.paint((np.sin(ang * 9) > .45) & (s.Y > 2), dark, [color])
    s.limb((0, 12, -7), (0, 26, 2), 3.4, 2.9, color)
    if hood:
        s.ball((0, 25, 1), (7, 6, 1.6), color)
    s.paint((s.Z > 0) & (s.Y > 13) & (np.abs(s.X) < 2.2), belly, [color])
    s.ball((0, 28, 5), (3.4, 2.8, 4.6), color)
    s.limb((0, 27.5, 7), (0, 27, 12), 2.6, 1.6, color)
    s.paint((np.abs(np.abs(s.X) - 2.4) < .8) & (np.abs(s.Y - 29) < .8) & (s.Z > 6), "#f2c84b")
    s.limb((0, 26.6, 12), (0, 25.5, 16), .6, .5, "#c8322c")                                          # the tongue
    return dict(s=s, want=dict(Hips=(0, 0, 0), Body=(0, 10, -4), Top=(0, 22, 0)), feature=("skin", [color, belly, dark]))


def worm(color="#7a4a8a"):
    """A great worm rearing out of the ground, its maw open."""
    s = Sculpt(40, 64, 44)
    dark = shade(color, .8)
    z = 0.0
    for j in range(9):                                       # segments, leaning forward as they rise
        y = 4 + j * 5.8
        z = 2.5 * math.sin(j * .45) + j * .6
        s.ball((0, y, z), (11 - j * .3, 4.4, 11 - j * .3), color if j % 2 == 0 else dark)
    top = (0, y + 4, z + 2)
    s.ball(top, (9, 5, 9), color)
    s.ball((0, y + 6, z + 5), (6.5, 4, 6.5), "#3a1a22")                                              # the maw
    for a in np.linspace(0, 2 * math.pi, 10, endpoint=False):                                        # a ring of teeth
        s.limb((math.cos(a) * 6.5, y + 7, z + 5 + math.sin(a) * 6.5), (math.cos(a) * 4, y + 10, z + 5 + math.sin(a) * 4), 1.2, .5, WHITE)
    return dict(s=s, want=dict(Hips=(0, 0, 0), Body=(0, 22, 2), Top=(0, 44, 5)), feature=("skin", [color, dark]))


def fish(color="#7a8a98"):
    """A shark: a fish seen swimming, its back fin up."""
    s = Sculpt(30, 32, 76)
    belly = "#e6e6e0"
    s.limb((0, 13, -20), (0, 13, 16), 4, 7.5, color, (.75, 1, 1))
    s.limb((0, 13, 16), (0, 12.5, 30), 7.5, 2, color, (.75, 1, 1))
    s.limb((0, 13, -20), (0, 13, -28), 4, 2, color, (.6, 1, 1))
    s.limb((0, 13, -28), (0, 23, -35), 2.6, 1, color, (.4, 1, 1))                                    # the tail fin
    s.limb((0, 13, -28), (0, 6, -33), 2.2, 1, color, (.4, 1, 1))
    s.limb((0, 19, 2), (0, 29, -5), 3.4, .8, color, (.4, 1, 1))                                      # the back fin
    for sx in (1, -1):
        s.limb((sx * 5, 10, 10), (sx * 12, 6, 4), 2.6, .8, color, (1, .45, 1))
    s.paint(s.Y < 10.5, belly, [color])
    s.paint((np.abs(s.Y - 10.8) < .7) & (s.Z > 21), "#3a2a30")                                        # the mouth
    s.paint((np.abs(np.abs(s.X) - 4) < 1.6) & (np.abs(s.Y - 14.5) < .9) & (np.abs(s.Z - 20) < .9), EYE)
    want = dict(Hips=(0, 13, -6), Chest=(0, 13, 8), Head=(0, 13, 18), Trail=(0, 13, -18), TrailTip=(0, 13, -27))
    sides(want, "Limb", (5, 10, 10))
    return dict(s=s, want=want, feature=("skin", [color]))


def octopus(color="#b0503a"):
    s = Sculpt(64, 30, 64)
    light = shade(color, 1.35)
    for a in np.linspace(0, 2 * math.pi, 8, endpoint=False) + .39:
        c, n = math.cos(a), math.sin(a)
        s.limb((c * 5, 5, n * 5), (c * 16, 2.6, n * 16), 2.8, 2, color)
        s.limb((c * 16, 2.6, n * 16), (c * 25 - n * 5, 1.6, n * 25 + c * 5), 2, .9, color)
    s.paint(s.Y < 2.5, light, [color])
    s.ball((0, 15, -2), (9, 11, 10), color)
    s.paint((np.abs(np.abs(s.X) - 4.5) < 1.5) & (np.abs(s.Y - 11) < 1.3) & (s.Z > 4), "#f2c84b")
    s.paint((np.abs(np.abs(s.X) - 4.5) < 1.5) & (np.abs(s.Y - 11) < .6) & (s.Z > 4), EYE)
    return dict(s=s, want=dict(Hips=(0, 0, 0), Body=(0, 8, 0), Top=(0, 18, -2)), feature=("skin", [color, light]))


# ---------------------------------------------------------------- two legs, plants and things

def raptor(kind="trex"):
    """A beast on two legs: a tyrant lizard, or a tall running bird with an axe of a beak."""
    if kind == "trex":
        s = Sculpt(40, 66, 100)
        skin, belly, dark = "#6a7a3a", "#c9c890", "#4a5628"
        s.limb((0, 34, -8), (0, 38, 12), 10, 11, skin, (.85, 1, 1))
        s.limb((0, 34, -8), (0, 28, -44), 8, 1.5, skin)
        s.limb((0, 40, 14), (0, 48, 24), 7, 6.5, skin)
        s.ball((0, 50, 28), (6, 6.5, 8), skin)
        s.limb((0, 50, 30), (0, 48, 42), 6, 4.5, skin, (.9, 1, 1))
        s.paint((s.Y < 30) & (s.Z > -30), belly, [skin])
        s.paint((s.Y < 45.5) & (s.Z > 22), belly, [skin])
        s.paint((np.abs(s.Y - 46) < .8) & (s.Z > 31) & (np.floor(s.Z) % 2 == 0), WHITE)                # teeth
        s.paint((np.sin(s.Z * .5) > .5) & (s.Y > 40) & (s.Z < 14), dark, [skin])
        s.paint((np.abs(np.abs(s.X) - 4.6) < 1.5) & (np.abs(s.Y - 53) < .9) & (np.abs(s.Z - 31) < .9), "#f2c84b")
        for sx in (1, -1):
            s.limb((sx * 8, 34, 14), (sx * 9, 28, 19), 1.8, 1.2, skin)
            s.limb((sx * 8, 32, -2), (sx * 9, 18, 3), 5.2, 3.6, skin)
            s.limb((sx * 9, 18, 3), (sx * 9, 6, -2), 3.4, 2.6, skin)
            s.limb((sx * 9, 5, -2), (sx * 9, 1.6, 7), 2.6, 2, dark)
        want = dict(Hips=(0, 34, -6), Chest=(0, 38, 10), Neck=(0, 42, 16), Head=(0, 49, 26), Tail=(0, 33, -12))
        sides(want, "Wing", (8, 34, 14))
        sides(want, "WingTip", (9, 28, 19))
        sides(want, "Thigh", (8, 32, -2))
        sides(want, "Shank", (9, 18, 3))
        sides(want, "Toe", (9, 3, 0))
        return dict(s=s, want=want, feature=("skin", [skin, dark]))
    s = Sculpt(30, 70, 60)
    feathers, wing, yellow = "#b8a070", "#8a7450", "#e0a030"
    s.ball((0, 34, 0), (8, 9, 12), feathers)
    s.limb((0, 34, -9), (0, 40, -19), 4.5, 1.5, wing, (.5, 1, 1))
    s.limb((0, 38, 8), (0, 56, 13), 3.4, 2.8, feathers)
    s.ball((0, 58, 14), 4.2, feathers)
    s.limb((0, 58, 17), (0, 54, 27), 3.8, 1.2, yellow, (.5, 1, 1))                                   # the beak
    s.paint((np.abs(np.abs(s.X) - 3.2) < 1.2) & (np.abs(s.Y - 59.5) < .9) & (np.abs(s.Z - 16) < .9), EYE)
    for sx in (1, -1):
        s.ball((sx * 7.4, 35, -1), (2, 6, 9), wing)
        s.limb((sx * 4, 28, 0), (sx * 4, 15, 2), 2, 1.4, yellow)
        s.limb((sx * 4, 15, 2), (sx * 4, 2, 0), 1.4, 1.1, yellow)
        s.limb((sx * 4, 1.2, -1), (sx * 4, 1, 6), 1.2, .9, yellow)
    want = dict(Hips=(0, 32, -3), Chest=(0, 37, 4), Neck=(0, 42, 9), Head=(0, 56, 13), Tail=(0, 35, -9))
    sides(want, "Wing", (7, 39, 3))
    sides(want, "WingTip", (7.5, 33, -8))
    sides(want, "Thigh", (4, 28, 0))
    sides(want, "Shank", (4, 15, 2))
    sides(want, "Toe", (4, 1.5, 0))
    return dict(s=s, want=want, feature=("feathers", [feathers, wing]))


def tree():
    """A tree that walks: a trunk with a face, two branches for arms, a crown of leaves."""
    s = Sculpt(64, 88, 48)
    bark, dark, leaf, leaf2 = "#6b4a32", "#4a3222", "#3f7a3a", "#5a9a4a"
    rng = np.random.default_rng(11)
    s.limb((0, 0, 0), (0, 46, 0), 8.5, 6.5, bark)
    for a in np.linspace(0, 2 * math.pi, 5, endpoint=False) + .3:
        s.limb((math.cos(a) * 5, 5, math.sin(a) * 5), (math.cos(a) * 14, 1.2, math.sin(a) * 14), 3, 1.6, bark)
    for sx in (1, -1):
        s.limb((sx * 6, 36, 0), (sx * 18, 31, 4), 3.4, 2.4, bark)
        s.limb((sx * 18, 31, 4), (sx * 22, 20, 8), 2.4, 1.4, bark)
        s.limb((sx * 22, 20, 8), (sx * 25, 16, 10), 1.2, .6, bark)
        s.limb((sx * 22, 20, 8), (sx * 20, 15, 11), 1.2, .6, bark)
    s.paint((np.sin(s.X * 1.4 + s.Y * .25) > .72) & (s.Y < 46), dark, [bark])
    for _ in range(30):
        c = (rng.uniform(-17, 17), rng.uniform(50, 74), rng.uniform(-12, 12))
        s.ball(c, rng.uniform(6, 9), leaf if rng.random() < .6 else leaf2)
    s.paint((np.abs(np.abs(s.X) - 3) < 1.4) & (np.abs(s.Y - 34) < 1.6) & (s.Z > 4), "#f2c84b")
    s.paint((np.abs(s.X) < 3) & (np.abs(s.Y - 26.5) < 1.3) & (s.Z > 4), "#1e1612")
    s.mirror()
    want = dict(Hips=(0, 20, 0), Chest=(0, 36, 0), Head=(0, 52, 0), Trail=(0, 12, 0), TrailTip=(0, 4, 0))
    sides(want, "Limb", (6, 36, 0))
    return dict(s=s, want=want, feature=("leaves", [leaf, leaf2]))


def shrub():
    s = Sculpt(40, 34, 40)
    leaf, leaf2 = "#3f7a3a", "#5a9a4a"
    rng = np.random.default_rng(5)
    for a in np.linspace(0, 2 * math.pi, 6, endpoint=False):
        s.limb((math.cos(a) * 3, 5, math.sin(a) * 3), (math.cos(a) * 10, 1, math.sin(a) * 10), 1.6, 1, "#6b4a32")
    for _ in range(34):
        c = (rng.uniform(-11, 11), rng.uniform(8, 24), rng.uniform(-11, 11))
        s.ball(c, rng.uniform(4, 6.5), leaf if rng.random() < .6 else leaf2)
    s.paint((np.abs(np.abs(s.X) - 4) < 1.4) & (np.abs(s.Y - 16) < 1.4) & (s.Z > 8), "#f2c84b")
    return dict(s=s, want=dict(Hips=(0, 0, 0), Body=(0, 10, 0), Top=(0, 20, 0)), feature=("leaves", [leaf, leaf2]))


def mushroom(cap="#8a5a9a"):
    s = Sculpt(40, 40, 40)
    stalk = "#e4dccb"
    s.limb((0, 0, 0), (0, 22, 0), 6, 4.5, stalk)
    s.ball((0, 24, 0), (16, 10, 16), cap)
    s.carve((s.Y < 21) & (s.X ** 2 + s.Z ** 2 > 36))
    s.paint((s.Y < 22.5) & (s.X ** 2 + s.Z ** 2 > 30), shade(cap, .7))
    rng = np.random.default_rng(2)
    for _ in range(9):
        a, r = rng.uniform(0, 2 * math.pi), rng.uniform(3, 13)
        s.paint(((s.X - math.cos(a) * r) ** 2 + (s.Z - math.sin(a) * r) ** 2 < 4) & (s.Y > 25), shade(cap, 1.45), [cap])
    return dict(s=s, want=dict(Hips=(0, 0, 0), Body=(0, 12, 0), Top=(0, 24, 0)), feature=("cap", [cap, shade(cap, .7), shade(cap, 1.45)]))


def mimic():
    """A chest with teeth."""
    s = Sculpt(44, 34, 40)
    wood, dark, metal = "#7a5230", "#4a3222", "#c9a03c"
    s.box((-16, 0, -10), (16, 13, 10), wood)
    s.box((-15, 13, -9), (15, 17, 9), "#3a1a22")                                                     # the mouth
    s.box((-16, 17, -10), (16, 22, 10), wood)
    s.limb((-15, 22, 0), (15, 22, 0), 9.5, 9.5, wood, (1, .5, 1))
    s.carve(s.Y < 0)
    s.paint((np.abs(np.abs(s.X) - 11) < 1.5) | (np.abs(np.abs(s.X) - 15.5) < .6), metal, [wood])
    s.paint((np.abs(s.Y - 6.5) < .6) | (np.abs(s.Y - 20) < .6), dark, [wood])
    s.paint((s.Z > 8) & (((np.abs(s.Y - 13.5) < .6) | (np.abs(s.Y - 16.5) < .6)) & (np.floor(s.X) % 3 != 0)), WHITE)        # teeth
    s.limb((3, 14, 6), (7, 9, 17), 2.6, 1.8, "#c8525a", (1, .5, 1))                                   # the tongue
    s.paint((np.abs(np.abs(s.X) - 6) < 1.5) & (np.abs(s.Y - 24) < 1.5) & (s.Z > 5), "#f2c84b")
    s.paint((np.abs(np.abs(s.X) - 6) < .6) & (np.abs(s.Y - 24) < 1.5) & (s.Z > 5), EYE)
    return dict(s=s, want=dict(Hips=(0, 0, 0), Body=(0, 13, 0), Top=(0, 22, 0)))


def rug():
    s = Sculpt(40, 14, 60)
    red, gold, blue = "#8f2f2f", "#c9a03c", "#2f4f8f"
    s.box((-17, 0, -26), (17, 3, 24), red)
    s.limb((-16, 5, 24), (16, 5, 24), 4.5, 4.5, red)                                                 # the end that rears up
    s.paint((np.abs(np.abs(s.X) - 15) < 1) | (np.abs(s.Z + 24) < 1), gold, [red])
    s.paint((np.abs(s.X) + np.abs(s.Z + 2) * .6 < 9) & (s.Y > 2), blue, [red])
    s.paint((np.abs(s.X) + np.abs(s.Z + 2) * .6 < 4) & (s.Y > 2), gold)
    s.paint((s.Z < -25) & (np.floor(s.X) % 2 == 0), gold)
    return dict(s=s, want=dict(Hips=(0, 0, -10), Body=(0, 2, 8), Top=(0, 6, 24)))


def flying_sword():
    s = Sculpt(22, 58, 8)
    s.box((-2.5, 14, -1), (2.5, 50, 1), "#aab4c2")
    for j in range(5):                                       # the point
        s.box((-2.5 + j * .5, 50 + j, -1), (2.5 - j * .5, 51 + j, 1), "#aab4c2")
    s.paint((np.abs(s.X) < .6) & (s.Y < 48), "#6c7686", ["#aab4c2"])
    s.box((-8, 11, -1.5), (8, 14, 1.5), "#c9a03c")
    s.box((-1.5, 3, -1.5), (1.5, 11, 1.5), "#5e3e24")
    s.ball((0, 2, 0), 2.4, "#c9a03c")
    want = dict(Hips=(0, 12, 0), Chest=(0, 26, 0), Head=(0, 40, 0), Trail=(0, 8, 0), TrailTip=(0, 3, 0))
    sides(want, "Limb", (7, 12, 0))
    return dict(s=s, want=want)



# file (under models/, without .glb): (its builder, the rig template that animates it)
CREATURES = {
    "wolf": (wolf, "rig_quadruped"), "dog": (dog, "rig_quadruped"), "cat": (cat, "rig_quadruped"), "rat": (rat, "rig_quadruped"), "bear": (bear, "rig_quadruped"), "boar": (boar, "rig_quadruped"),
    "sheep": (sheep, "rig_quadruped"), "cow": (cow, "rig_quadruped"), "horse": (horse, "rig_quadruped"), "big_cat": (big_cat, "rig_quadruped"), "lion": (lambda: big_cat(mane=True), "rig_quadruped"),
    "tiger": (lambda: big_cat(stripes=True), "rig_quadruped"), "dragon_wyrmling": (dragon, "rig_quadruped"), "dragon_young": (lambda: dragon(1.5), "rig_quadruped"),
    "spider": (spider, "rig_quadruped"), "chicken": (chicken, "rig_bird"), "bat": (flyer, "rig_flyer"), "bird": (lambda: flyer("bird"), "rig_flyer"),
    "slime": (blob, "rig_blob"), "gel_cube": (lambda: blob("#9fd6c8", cube=True), "rig_blob"), "ghost": (ghost, "rig_floating"), "wisp": (wisp, "rig_floating"),
    "fire_elemental": (lambda: elemental("fire"), "rig_floating"), "water_elemental": (lambda: elemental("water"), "rig_floating"), "air_elemental": (lambda: elemental("air"), "rig_floating"),
    "earth_elemental": (lambda: elemental("earth"), "rig_floating"),
    "deer": (deer, "rig_quadruped"), "goat": (lambda: deer(goat=True), "rig_quadruped"), "elephant": (elephant, "rig_quadruped"), "rhino": (rhino, "rig_quadruped"),
    "triceratops": (lambda: rhino(frill=True), "rig_quadruped"), "crocodile": (crocodile, "rig_quadruped"), "frog": (frog, "rig_quadruped"), "ape": (ape, "rig_quadruped"),
    "griffon": (griffon, "rig_quadruped"), "centaur": (centaur, "rig_quadruped"), "pegasus": (lambda: horse(wings=True), "rig_quadruped"), "unicorn": (lambda: horse(horn=True), "rig_quadruped"),
    "winged_lion": (lambda: big_cat(mane=True, wings=True), "rig_quadruped"), "owlbear": (lambda: bear(owl=True), "rig_quadruped"), "drake": (lambda: dragon(1.25, wings=False), "rig_quadruped"),
    "crab": (lambda: spider("#c8683a", "#c8683a", claws=True), "rig_quadruped"), "scorpion": (lambda: spider("#a8763e", "#a8763e", claws=True, sting=True), "rig_quadruped"),
    "drider": (lambda: spider(drow=True), "rig_quadruped"), "wasp": (lambda: flyer("wasp"), "rig_flyer"), "snake": (snake, "rig_blob"), "cobra": (lambda: snake("#a8904a", hood=True), "rig_blob"),
    "worm": (worm, "rig_blob"), "shark": (fish, "rig_floating"), "octopus": (octopus, "rig_blob"), "tyrant_lizard": (raptor, "rig_bird"), "axe_beak": (lambda: raptor("axebeak"), "rig_bird"),
    "tree": (tree, "rig_floating"), "shrub": (shrub, "rig_blob"), "mushroom": (mushroom, "rig_blob"), "mimic": (mimic, "rig_blob"), "rug": (rug, "rig_blob"), "flying_sword": (flying_sword, "rig_floating"),
}


def build(name):
    """A creature's mesh, its rig fit, its height in world pixels (two voxels a pixel) and the
    colour feature its variants recolour (name, shades)."""
    make, rig = CREATURES[name]
    q = make()
    mesh, fit = finish(q, rig)
    lo, hi = q["s"].bounds()
    return mesh, fit, round(float(hi[1] - lo[1])) / 2, q.get("feature")
