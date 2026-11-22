"""The figure kit's core: the kit's skeleton (re-proportioned per build), a mesh of plain-coloured
triangles bound to its bones, voxel blocks with merged faces, and the `.glb` reader / writer.

Nothing here knows what a figure looks like: `figure.py` builds bodies and pieces with it."""
import json
import os
import struct

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
RIG = f"{HERE}/rig/human.glb"
X, Y, Z = np.array([1.0, 0, 0]), np.array([0, 1.0, 0]), np.array([0, 0, 1.0])
NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}
DT = {5126: "<f4", 5123: "<u2", 5121: "u1", 5125: "<u4", 5120: "i1", 5122: "<i2"}
# What the engine draws in one piece (`render/model.rs`: DRAW_VERTICES, DRAW_INDICES).
MAX_VERTICES, MAX_TRIANGLES = 32_768, 32_768
GENERATOR = "rustpg2d figure kit"


def read_glb(path):
    b = open(path, "rb").read()
    jl = struct.unpack("<I", b[12:16])[0]
    j = json.loads(b[20:20 + jl])
    if len(b) <= 20 + jl:
        return j, b""
    bl = struct.unpack("<I", b[20 + jl:24 + jl])[0]
    return j, b[28 + jl:28 + jl + bl]


def accessor(j, bin_, i):
    a = j["accessors"][i]
    bv = j["bufferViews"][a["bufferView"]]
    n = NCOMP[a["type"]]
    off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    dt = np.dtype(DT[a["componentType"]])
    assert bv.get("byteStride", 0) in (0, dt.itemsize * n), "interleaved buffer"
    return np.frombuffer(bin_, dtype=dt, count=a["count"] * n, offset=off).reshape(a["count"], n).copy()


class Bin:
    """The binary chunk of a `.glb` being written, with its views and accessors."""

    def __init__(self):
        self.data = bytearray()
        self.views = []
        self.accs = []

    def add(self, arr, type_, comp, target=None, minmax=False):
        arr = np.ascontiguousarray(arr)
        self.data += b"\0" * ((4 - len(self.data) % 4) % 4)
        view = {"buffer": 0, "byteOffset": len(self.data), "byteLength": arr.nbytes}
        if target:
            view["target"] = target
        self.data += arr.tobytes()
        self.views.append(view)
        acc = {"bufferView": len(self.views) - 1, "componentType": comp, "count": len(arr), "type": type_}
        if minmax:
            acc["min"] = arr.min(0).tolist() if arr.ndim > 1 else [float(arr.min())]
            acc["max"] = arr.max(0).tolist() if arr.ndim > 1 else [float(arr.max())]
        self.accs.append(acc)
        return len(self.accs) - 1


def write_glb(path, j, data):
    js = json.dumps(j, separators=(",", ":"), sort_keys=True).encode()
    js += b" " * ((4 - len(js) % 4) % 4)
    data = bytes(data) + b"\0" * ((4 - len(data) % 4) % 4)
    total = 12 + 8 + len(js) + 8 + len(data)
    with open(path, "wb") as f:
        f.write(struct.pack("<4sII", b"glTF", 2, total) + struct.pack("<I4s", len(js), b"JSON") + js + struct.pack("<I4s", len(data), b"BIN\0") + data)


def qmat(q):
    x, y, z, w = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def norm(v):
    n = np.linalg.norm(v)
    return v / n if n > 1e-12 else v


def frame(axis, ref=Z):
    """Axes of a block along `axis`: (across, along, front), the front as near `ref` as it gets."""
    y = norm(np.asarray(axis, float))
    z = ref - np.dot(ref, y) * y
    if np.linalg.norm(z) < 1e-6:
        z = Y - np.dot(Y, y) * y
    z = norm(z)
    return np.cross(y, z), y, z


def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


class Rig:
    """The kit's skeleton (`rig/human.glb`). `proportions()` changes bone lengths for a build and
    puts the feet back on the floor; the clips fit every build because the engine fits shared
    clips to a model's own bone lengths."""

    def __init__(self):
        j, _ = read_glb(RIG)
        self.nodes = []
        for n in j["nodes"]:
            self.nodes.append(dict(name=n.get("name", ""), t=np.array(n.get("translation", [0, 0, 0]), float), r=np.array(n.get("rotation", [0, 0, 0, 1]), float),
                                   s=np.array(n.get("scale", [1, 1, 1]), float), children=list(n.get("children", []))))
        self.joints = list(j["skins"][0]["joints"])
        self.mesh_node = next(i for i, n in enumerate(j["nodes"]) if n.get("name") == j["extras"]["mesh_node"])
        self.idx = {n["name"]: i for i, n in enumerate(self.nodes)}
        self.jidx = {self.nodes[n]["name"]: k for k, n in enumerate(self.joints)}
        self.roots = list(j["scenes"][0]["nodes"])
        self._world = None
        self.ankle0 = (self.p("LeftFoot")[1] + self.p("RightFoot")[1]) / 2

    def world(self):
        if self._world is None:
            out = [None] * len(self.nodes)

            def go(i, parent):
                n = self.nodes[i]
                m = np.eye(4)
                m[:3, :3] = qmat(n["r"]) * n["s"]
                m[:3, 3] = n["t"]
                out[i] = parent @ m
                for c in n["children"]:
                    go(c, out[i])
            for r in self.roots:
                go(r, np.eye(4))
            self._world = out
        return self._world

    def p(self, name):
        return self.world()[self.idx[name]][:3, 3].copy()

    def scale(self, names, f, axis=None):
        for name in names:
            t = self.nodes[self.idx[name]]["t"]
            if axis is None:
                t *= f
            else:
                t[axis] *= f
        self._world = None

    def proportions(self, legs=1.0, torso=1.0, arms=1.0, shoulders=1.0, hips=1.0, neck=1.0):
        s = ["Left", "Right"]
        self.scale([f"{a}Leg" for a in s] + [f"{a}Foot" for a in s], legs)
        self.scale(["Spine02", "Spine01", "Spine", "neck"], torso)
        self.scale(["Head"], neck)
        self.scale([f"{a}ForeArm" for a in s] + [f"{a}Hand" for a in s], arms)
        self.scale([f"{a}Arm" for a in s], shoulders)
        self.scale([f"{a}UpLeg" for a in s], hips, axis=0)
        ankle = (self.p("LeftFoot")[1] + self.p("RightFoot")[1]) / 2
        self.nodes[self.idx["Hips"]]["t"][1] += self.ankle0 - ankle
        self._world = None
        return self

    def json_nodes(self):
        out = []
        for n in self.nodes:
            d = {"name": n["name"]}
            if np.abs(n["t"]).max() > 0:
                d["translation"] = [round(float(v), 6) for v in n["t"]]
            if not np.allclose(n["r"], [0, 0, 0, 1]):
                d["rotation"] = [round(float(v), 6) for v in n["r"]]
            if not np.allclose(n["s"], 1):
                d["scale"] = [round(float(v), 6) for v in n["s"]]
            if n["children"]:
                d["children"] = n["children"]
            out.append(d)
        return out


class Mesh:
    """Triangles in plain colours, each vertex bound to up to four bones. One primitive per
    colour: the engine gives a model of plain-coloured materials a palette texture, and colour
    features match its exact colours."""

    def __init__(self, rig):
        self.rig = rig
        self.prims = {}

    def w(self, spec):
        if isinstance(spec, str):
            spec = {spec: 1.0}
        items = sorted(((self.rig.jidx[n], float(v)) for n, v in spec.items() if v > 1e-4), key=lambda x: (-x[1], x[0]))[:4]
        tot = sum(v for _, v in items)
        return tuple((j, v / tot) for j, v in items)

    def quad(self, color, a, b, c, d, bone, along):
        """A flat face a-b-c-d seen from where `along` points."""
        pts = [np.asarray(v, float) for v in (a, b, c, d)]
        if np.dot(np.cross(pts[1] - pts[0], pts[2] - pts[0]), along) < 0:
            pts = [pts[0], pts[3], pts[2], pts[1]]
        if isinstance(color, str):
            color = rgb(color)
        self.prims.setdefault(tuple(color), []).append((pts, norm(np.asarray(along, float)), self.w(bone)))

    def empty(self):
        return not self.prims

    def bounds(self):
        pts = np.array([p for quads in self.prims.values() for q, _, _ in quads for p in q])
        return pts.min(0), pts.max(0)

    def export(self, path):
        rig = self.rig
        world = rig.world()
        out = Bin()
        ibm = np.array([np.linalg.inv(world[n]).T.flatten() for n in rig.joints], np.float32)
        ibm_acc = out.add(ibm, "MAT4", 5126)
        prims, mats = [], []
        nv = nt = 0
        for color in sorted(self.prims):
            index, pos, nor, jn, wt, idx = {}, [], [], [], [], []
            for pts, n, w in self.prims[color]:
                ids = []
                for p in pts:
                    key = (tuple(np.round(p, 5)), tuple(np.round(n, 3)), w)
                    i = index.get(key)
                    if i is None:
                        i = index[key] = len(pos)
                        pos.append(p)
                        nor.append(n)
                        jn.append([j for j, _ in w] + [0] * (4 - len(w)))
                        wt.append([v for _, v in w] + [0.0] * (4 - len(w)))
                    ids.append(i)
                idx += [ids[0], ids[1], ids[2], ids[0], ids[2], ids[3]]
            attrs = {"POSITION": out.add(np.array(pos, np.float32), "VEC3", 5126, 34962, True), "NORMAL": out.add(np.array(nor, np.float32), "VEC3", 5126, 34962),
                     "JOINTS_0": out.add(np.array(jn, np.uint16), "VEC4", 5123, 34962), "WEIGHTS_0": out.add(np.array(wt, np.float32), "VEC4", 5126, 34962)}
            prims.append({"attributes": attrs, "indices": out.add(np.array(idx, np.uint16), "SCALAR", 5123, 34963), "material": len(mats)})
            mats.append({"name": "#%02x%02x%02x" % color, "pbrMetallicRoughness": {"baseColorFactor": [round(c / 255, 6) for c in color] + [1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0}})
            nv += len(pos)
            nt += len(idx) // 3
        if nv > MAX_VERTICES or nt > MAX_TRIANGLES:
            raise SystemExit(f"{path}: {nv} vertices and {nt} triangles, more than the engine draws in one piece ({MAX_VERTICES} / {MAX_TRIANGLES})")
        nodes = rig.json_nodes()
        nodes[rig.mesh_node].update(mesh=0, skin=0)
        doc = {"asset": {"version": "2.0", "generator": GENERATOR}, "scene": 0, "scenes": [{"nodes": rig.roots}], "nodes": nodes,
               "meshes": [{"primitives": prims}], "materials": mats, "skins": [{"joints": rig.joints, "inverseBindMatrices": ibm_acc}],
               "buffers": [{"byteLength": len(out.data)}], "bufferViews": out.views, "accessors": out.accs}
        write_glb(path, doc, out.data)
        lo, hi = self.bounds()
        return dict(vertices=nv, triangles=nt, colors=["#%02x%02x%02x" % c for c in sorted(self.prims)], lo=[round(float(v), 4) for v in lo], hi=[round(float(v), 4) for v in hi])


class StaticMesh(Mesh):
    """Triangles in plain colours without bones: an item's model."""

    def __init__(self):
        self.rig = None
        self.prims = {}

    def w(self, spec):
        return ()

    def export(self, path):
        out = Bin()
        prims, mats = [], []
        nv = nt = 0
        for color in sorted(self.prims):
            index, pos, nor, idx = {}, [], [], []
            for pts, n, _ in self.prims[color]:
                ids = []
                for p in pts:
                    key = (tuple(np.round(p, 5)), tuple(np.round(n, 3)))
                    i = index.get(key)
                    if i is None:
                        i = index[key] = len(pos)
                        pos.append(p)
                        nor.append(n)
                    ids.append(i)
                idx += [ids[0], ids[1], ids[2], ids[0], ids[2], ids[3]]
            attrs = {"POSITION": out.add(np.array(pos, np.float32), "VEC3", 5126, 34962, True), "NORMAL": out.add(np.array(nor, np.float32), "VEC3", 5126, 34962)}
            prims.append({"attributes": attrs, "indices": out.add(np.array(idx, np.uint16), "SCALAR", 5123, 34963), "material": len(mats)})
            mats.append({"name": "#%02x%02x%02x" % color, "pbrMetallicRoughness": {"baseColorFactor": [round(c / 255, 6) for c in color] + [1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0}})
            nv += len(pos)
            nt += len(idx) // 3
        if nv > MAX_VERTICES or nt > MAX_TRIANGLES:
            raise SystemExit(f"{path}: {nv} vertices and {nt} triangles, more than the engine draws in one piece ({MAX_VERTICES} / {MAX_TRIANGLES})")
        doc = {"asset": {"version": "2.0", "generator": GENERATOR}, "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"name": "item", "mesh": 0}], "meshes": [{"primitives": prims}],
               "materials": mats, "buffers": [{"byteLength": len(out.data)}], "bufferViews": out.views, "accessors": out.accs}
        write_glb(path, doc, out.data)
        lo, hi = self.bounds()
        return dict(vertices=nv, triangles=nt, colors=["#%02x%02x%02x" % c for c in sorted(self.prims)], lo=[round(float(v), 4) for v in lo], hi=[round(float(v), 4) for v in hi])


class Vox:
    """A block of voxels bound to one bone (or a blend of two). Cell (i, j, k) is the box from
    origin + i·sx·ax + j·sy·ay + k·sz·az to the next cell."""
    DIRS = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)]

    def __init__(self, origin, ax, ay, az, size, bone):
        self.o = np.asarray(origin, float)
        self.ax, self.ay, self.az = (np.asarray(v, float) for v in (ax, ay, az))
        self.s = size if isinstance(size, (tuple, list)) else (size, size, size)
        self.bone = bone
        self.cells = {}

    def like(self):
        """An empty block on the same grid: a piece worn on this one."""
        return Vox(self.o, self.ax, self.ay, self.az, self.s, self.bone)

    def set(self, i, j, k, color):
        self.cells[(i, j, k)] = color

    def box(self, i, j, k, color):
        """i, j, k: (from, to) ranges, `to` exclusive."""
        for a in range(*i):
            for b in range(*j):
                for c in range(*k):
                    self.cells[(a, b, c)] = color

    def at(self, i, j, k):
        return self.o + i * self.s[0] * self.ax + j * self.s[1] * self.ay + k * self.s[2] * self.az

    def emit(self, mesh, hidden=()):
        """The faces between a cell and nothing, those of one colour in one plane merged into
        rectangles. `hidden`: cells of the block this one is worn on (no face against them)."""
        axes = (self.ax, self.ay, self.az)
        planes = {}
        for (i, j, k), color in self.cells.items():
            for d in self.DIRS:
                nb = (i + d[0], j + d[1], k + d[2])
                if nb in self.cells or nb in hidden:
                    continue
                u = next(n for n in range(3) if d[n])
                a, b = [n for n in range(3) if n != u]
                p = (i, j, k)
                planes.setdefault((d, p[u] + (1 if d[u] > 0 else 0), color), set()).add((p[a], p[b]))
        for (d, plane, color) in sorted(planes, key=lambda t: (t[0], t[1], str(t[2]))):
            cells = planes[(d, plane, color)]
            u = next(n for n in range(3) if d[n])
            a, b = [n for n in range(3) if n != u]
            while cells:
                a0, b0 = min(cells)
                w = 1
                while (a0 + w, b0) in cells:
                    w += 1
                h = 1
                while all((a0 + x, b0 + h) in cells for x in range(w)):
                    h += 1
                for x in range(w):
                    for y in range(h):
                        cells.discard((a0 + x, b0 + y))
                c = []
                for da, db in ((0, 0), (w, 0), (w, h), (0, h)):
                    q = [0, 0, 0]
                    q[u], q[a], q[b] = plane, a0 + da, b0 + db
                    c.append(self.at(*q))
                mesh.quad(color, c[0], c[1], c[2], c[3], self.bone, axes[u] * d[u])
