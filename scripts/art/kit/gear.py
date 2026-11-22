"""The kit's gear: weapons, shields, tools, lights and backpacks as voxel models, one voxel per
pixel of the drawn item (an item `size` world pixels long is 2 x size voxels long).

Every item stands along +Y with its handle at the bottom, is thin along Z, and its front (a
blade's edge, an axe's head, a bow's back, a shield's face) is where `front` says. Each builder
returns the voxels and how the item is held: the hold point, the direction from it to the far
end, and the front (see `ItemModel` in the engine)."""
import math

import numpy as np

from figure import CLOTH, LEATHER, LEATHER_DARK, METAL, METAL_DARK, RAG, STRAW, TRIM, rnd
from kitlib import X, Y, Z, StaticMesh, Vox

WOOD, WOOD_DARK, STRING, FLAME, GLOW, MAGIC = "#8a5a2c", "#5e3c1e", "#d8d0c0", "#f2903c", "#f6d860", "#6a4a7a"


def grid():
    return Vox(np.zeros(3), X, Y, Z, 1.0, None)


def rod(v, y0, y1, color, w=2):
    """A shaft along Y, `w` voxels square, centred."""
    v.box((-(w // 2), w - w // 2), (y0, y1), (-(w // 2), w - w // 2), color)


def sword(length, blade=4, guard=METAL_DARK, pommel=METAL_DARK, fuller=None):
    v = grid()
    v.box((-1, 2), (0, 3), (-1, 1), pommel)
    rod(v, 3, 10, LEATHER_DARK)
    v.box((-5, 5), (10, 12), (-1, 1), guard)
    for y in range(12, length):
        left = length - y                                   # the tip narrows
        half = min(blade, left + 1) / 2
        for x in range(-rnd(half), rnd(half + .01)):
            v.set(x, y, -1 if blade < 5 else 0, fuller if fuller and x in (-1, 0) and left > 6 else METAL)
            if blade >= 5:
                v.set(x, y, -1, fuller if fuller and x in (-1, 0) and left > 6 else METAL)
    return v, (0, 6, 0), (0, 1, 0), (1, 0, 0)


def spear(length):
    v = grid()
    rod(v, 0, length - 12, WOOD)
    rod(v, length - 14, length - 12, METAL_DARK, 4)
    for y in range(length - 12, length):
        t = (y - (length - 12)) / 12
        half = 3.2 * math.sin(math.pi * min(1, t * 1.15 + .12))
        for x in range(-rnd(half), rnd(half + .01)):
            v.set(x, y, -1, METAL)
            v.set(x, y, 0, METAL)
    return v, (0, rnd(length * .27), 0), (0, 1, 0), (1, 0, 0)


def staff(length):
    v = grid()
    rod(v, 0, length, WOOD)
    for y0 in (0, length - 6):
        rod(v, y0, y0 + 6, WOOD_DARK, 4 if y0 else 2)
    rod(v, length // 2 - 5, length // 2 + 5, LEATHER_DARK)
    return v, (0, length // 2, 0), (0, 1, 0), (0, 0, 1)


def axe(length):
    v = grid()
    rod(v, 0, length, WOOD)
    rod(v, 4, 12, LEATHER_DARK)
    top = length - 3
    for x in range(1, 10):                                   # the blade: wider toward its edge
        half = 2 + x * .75
        for y in range(rnd(top - 6 - half), rnd(top - 6 + half)):
            v.set(x, y, -1, METAL if x < 8 else "#d5dce6")
            v.set(x, y, 0, METAL if x < 8 else "#d5dce6")
    v.box((-3, 1), (top - 9, top - 3), (-1, 1), METAL_DARK)
    return v, (0, 8, 0), (0, 1, 0), (1, 0, 0)


def hammer(length):
    v = grid()
    rod(v, 0, length, WOOD)
    v.box((-6, 7), (length - 9, length - 1), (-3, 3), METAL_DARK)
    for x in (-6, 6):
        v.box((x, x + 1), (length - 9, length - 1), (-3, 3), METAL)
    return v, (0, 8, 0), (0, 1, 0), (1, 0, 0)


def pickaxe(length):
    v = grid()
    rod(v, 0, length, WOOD)
    for x in range(-14, 15):                                 # a curved head, its points down
        y = length - 4 - rnd((abs(x) / 14) ** 2 * 7)
        thick = 3 if abs(x) < 9 else 2 if abs(x) < 13 else 1
        v.box((x, x + 1), (y - thick, y), (-1, 1), METAL)
    v.box((-2, 3), (length - 8, length - 3), (-2, 2), METAL_DARK)
    return v, (0, 10, 0), (0, 1, 0), (1, 0, 0)


def hoe(length):
    v = grid()
    rod(v, 0, length, WOOD)
    v.box((1, 4), (length - 3, length), (-1, 1), METAL_DARK)
    v.box((4, 10), (length - 8, length), (-3, 3), METAL)
    v.box((4, 10), (length - 8, length - 2), (-2, 2), METAL)
    for k in (-3, 2):
        v.box((4, 10), (length - 2, length), (k, k + 1), METAL_DARK)
    return v, (0, rnd(length * .28), 0), (0, 1, 0), (1, 0, 0)


def broom(length):
    v = grid()
    rod(v, 0, length - 18, WOOD)
    rod(v, length - 20, length - 16, RAG, 4)
    for y in range(length - 16, length):
        half = 3 + (y - (length - 16)) * .28
        for x in range(-rnd(half), rnd(half)):
            for z in range(-2, 2):
                v.set(x, y, z, STRAW if (x + y) % 5 else "#b8943f")
    return v, (0, rnd(length * .25), 0), (0, 1, 0), (1, 0, 0)


def fishing_rod(length):
    v = grid()
    rod(v, 0, 10, LEATHER_DARK, 3)
    rod(v, 10, rnd(length * .6), WOOD)
    rod(v, rnd(length * .6), length, WOOD_DARK, 1)
    v.box((1, 4), (11, 15), (-1, 1), METAL_DARK)             # the reel
    v.set(1, length - 1, 0, STRING)
    return v, (0, 5, 0), (0, 1, 0), (1, 0, 0)


def torch(length):
    v = grid()
    rod(v, 0, length - 12, WOOD)
    rod(v, length - 18, length - 10, RAG, 4)
    for y in range(length - 10, length):                     # the flame
        t = (y - (length - 10)) / 10
        half = 2.6 * (1 - t) + .6
        for x in range(-rnd(half), rnd(half)):
            for z in range(-rnd(half), rnd(half)):
                v.set(x, y, z, GLOW if abs(x) + abs(z) < half * .8 and t > .15 else FLAME)
    return v, (0, 10, 0), (0, 1, 0), (1, 0, 0)


def lantern(length):
    v = grid()
    h = length - 8
    v.box((-4, 4), (0, 2), (-4, 4), METAL_DARK)
    v.box((-3, 3), (2, h - 2), (-3, 3), GLOW)
    for x, z in ((-4, -4), (3, -4), (-4, 3), (3, 3)):
        v.box((x, x + 1), (2, h - 2), (z, z + 1), METAL_DARK)
    v.box((-4, 4), (h - 2, h), (-4, 4), METAL_DARK)
    v.box((-2, 2), (h, h + 2), (-2, 2), METAL_DARK)
    for y in range(h + 2, length):                           # the ring it hangs by
        for x in (-2, 1) if y < length - 1 else range(-2, 2):
            v.set(x, y, 0, METAL)
    return v, (0, length - 1, 0), (0, -1, 0), (0, 0, 1)


def bow(length, bulge=9, wood=WOOD):
    """Its back bulges toward +X, the string runs straight between the tips."""
    v = grid()
    mid = length // 2
    for y in range(length):
        x = rnd(bulge * math.sin(math.pi * y / (length - 1)))
        thick = 2 if 4 < y < length - 5 else 1
        for d in range(thick):
            v.set(x - d, y, -1, wood)
            v.set(x - d, y, 0, wood)
        if 0 < y < length - 1:
            v.set(0, y, 0, STRING)
    for y in range(mid - 4, mid + 4):
        v.set(bulge, y, -1, LEATHER_DARK)
        v.set(bulge, y, 0, LEATHER_DARK)
        v.set(bulge - 1, y, -1, LEATHER_DARK)
        v.set(bulge - 1, y, 0, LEATHER_DARK)
    return v, (bulge - 1, mid, 0), (0, 1, 0), (1, 0, 0)


def crossbow(length):
    v = grid()
    v.box((-1, 2), (0, length - 4), (-1, 2), WOOD)
    v.box((-2, 3), (0, 8), (-1, 2), WOOD_DARK)               # the butt
    for x in range(-15, 16):                                 # the prod, bent back toward the butt
        y = length - 6 - rnd((abs(x) / 15) ** 2 * 6)
        v.box((x, x + 1), (y, y + 2), (0, 2), METAL_DARK)
    for x in range(-14, 15):
        v.set(x, length - 14, 1, STRING)
    v.box((0, 1), (length - 14, length - 2), (1, 2), METAL)   # the bolt
    return v, (0, 12, 0), (0, 1, 0), (1, 0, 0)


def shield(size):
    """A round wooden shield: its face toward +Z, the strap side at the back."""
    v = grid()
    r = size / 2
    for x in range(-rnd(r), rnd(r)):
        for y in range(-rnd(r), rnd(r)):
            d = math.hypot(x + .5, y + .5)
            if d > r:
                continue
            rim = d > r - 2
            plank = WOOD_DARK if (x + 100) % 6 == 0 else WOOD
            v.box((x, x + 1), (y, y + 1), (0, 2), METAL_DARK if rim else plank)
            if d < 3.5:
                v.set(x, y, 2, METAL)
                if d < 2:
                    v.set(x, y, 3, METAL)
    return v, (0, 0, 0), (0, 1, 0), (0, 0, 1)


def backpack(height, width, depth, color=LEATHER, flap=LEATHER_DARK, roll=None, trim=None):
    """Its flat side (toward the wearer's back) at -Z."""
    v = grid()
    h = height - (5 if roll else 0)
    for x in range(-width // 2, width // 2):
        for y in range(h):
            for z in range(depth):
                ex, ey, ez = min(x + width // 2, width // 2 - 1 - x), min(y, h - 1 - y), depth - 1 - z
                if ex + ey < 1 or (ex + ez < 1) or (ey + ez < 1 and y > h // 2):
                    continue
                front = z == depth - 1
                v.set(x, y, z, flap if y >= h * .62 else (trim if trim and front and ex == 2 else color))
    v.box((-1, 1), (rnd(h * .5), rnd(h * .62)), (depth, depth + 1), trim or METAL)      # the buckle
    v.box((-width // 4, width // 4), (2, rnd(h * .4)), (depth, depth + 1), flap)       # a front pocket
    if roll:
        for x in range(-width // 2 - 1, width // 2 + 1):
            for y in range(h, h + 5):
                for z in range(1, 6):
                    if min(y - h, h + 4 - y) + min(z - 1, 5 - z) >= 1:
                        v.set(x, y, z, roll if abs(x) not in (width // 2 - 2,) else LEATHER_DARK)
    return v, (0, rnd(h * .68), 0), (0, 1, 0), (0, 0, 1)


# file (under models/, without .glb): its builder. Lengths are 2 x the item's size in world pixels.
GEAR = {
    "sword": lambda: sword(48, 4),
    "iron_sword": lambda: sword(52, 4, fuller="#8d97a6"),
    "fine_iron_sword": lambda: sword(56, 5, guard=TRIM, pommel=TRIM, fuller="#d5dce6"),
    "spear": lambda: spear(96),
    "quarterstaff": lambda: staff(92),
    "axe": lambda: axe(52),
    "hammer": lambda: hammer(40),
    "pickaxe": lambda: pickaxe(48),
    "hoe": lambda: hoe(72),
    "broom": lambda: broom(72),
    "fishing_rod": lambda: fishing_rod(88),
    "torch": lambda: torch(48),
    "lantern": lambda: lantern(24),
    "bow": lambda: bow(60),
    "crude_bow": lambda: bow(56, 7, WOOD_DARK),
    "crossbow": lambda: crossbow(48),
    "wooden_shield": lambda: shield(36),
    "backpack_small": lambda: backpack(22, 14, 7),
    "backpack_big": lambda: backpack(30, 18, 9, roll=CLOTH),
    "backpack_magic": lambda: backpack(24, 14, 7, MAGIC, "#4a3258", trim=TRIM),
}


def build(name):
    """An item's mesh and how it is held: (mesh, dict(grip, axis, front))."""
    v, grip, axis, front = GEAR[name]()
    m = StaticMesh()
    v.emit(m)
    return m, dict(grip=[float(c) for c in grip], axis=[float(c) for c in axis], front=[float(c) for c in front])
