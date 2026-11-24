"""blender -b -P smooth.py -- JOBS.json: smooths built kit models in place (the painted figures' shapes).

JOBS.json is a list of {"file": PATH, "mode": "body"|"head_piece", "head": PATH}: build.py writes it.
- body: heads, outfits (the bodies), cloaks, wings and creatures: remeshed and smoothed strongly,
  so blocks become round limbs, heads and muzzles;
- head_piece: hair, beards and hats, smoothed together with a plain head of their size (`head`),
  so a cap follows a round skull; only the part over the piece is kept.
Every model is checked against its voxels: where the smoothing melted a thin part away (a brim, a
wing, a tail tip) it is done again gently, and failing that the voxel model is kept.
The bones and their weights are carried over from the voxels; every face keeps the exact plain
colour of the nearest voxel face, so the engine's palette and colour features work unchanged.
Prints one `SMOOTHED file mode vertices triangles` line per model (mode: strong, gentle, voxel)."""
import json
import os
import sys

import bmesh
import bpy
from mathutils.bvhtree import BVHTree

FACES = 2500


def weld(me):
    """Closes the cracks of merged voxel faces: splits open edges at the open vertices lying on them
    (T-junctions), merges doubles, then fills the holes left where a piece lies against the body
    (the inside of a hair cap), so the voxel remesh sees a closed volume."""
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    key = lambda v: (round(v[0], 4), round(v[1], 4), round(v[2], 4))  # noqa: E731
    for _ in range(30):
        # Open vertices by the axis line they lie on (voxel edges run along the axes).
        lines = {}
        for v in {v for e in bm.edges if e.is_boundary for v in e.verts}:
            k = key(v.co)
            for a in range(3):
                rest = tuple(k[i] for i in range(3) if i != a)
                lines.setdefault((a, rest), []).append(v)
        changed = False
        for e in [e for e in bm.edges if e.is_boundary]:
            if not e.is_valid:
                continue
            p, q = e.verts[0].co, e.verts[1].co
            d = q - p
            axis = max(range(3), key=lambda i: abs(d[i]))
            if any(abs(d[i]) > 1e-5 for i in range(3) if i != axis):
                continue
            k = key(p)
            rest = tuple(k[i] for i in range(3) if i != axis)
            lo, hi = sorted((p[axis], q[axis]))
            inner = [v for v in lines.get((axis, rest), []) if lo + 1e-5 < v.co[axis] < hi - 1e-5 and v not in e.verts]
            if inner:
                v = min(inner, key=lambda v: abs(v.co[axis] - p[axis]))
                t = (v.co[axis] - p[axis]) / d[axis]
                _, nv = bmesh.utils.edge_split(e, e.verts[0], t)
                nv.co = v.co.copy()
                changed = True
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        if not changed:
            break
    bmesh.ops.holes_fill(bm, edges=[e for e in bm.edges if e.is_boundary], sides=0)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()


def voxel_size(me):
    """The kit voxel's edge in this model's units: its shortest edge."""
    lens = [(me.vertices[e.vertices[0]].co - me.vertices[e.vertices[1]].co).length for e in me.edges]
    return min(x for x in lens if x > 1e-6)


def tree(me):
    return BVHTree.FromPolygons([v.co.copy() for v in me.vertices], [list(p.vertices) for p in me.polygons])


def apply(o, kind, **kw):
    md = o.modifiers.new(kind.lower(), kind)
    for k, v in kw.items():
        setattr(md, k, v)
    bpy.context.view_layer.objects.active = o
    bpy.ops.object.modifier_apply(modifier=md.name)


def load(path):
    """The armature and the mesh of a kit file (the importer's bone shapes removed)."""
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.context.scene.objects if o not in before]
    for o in [o for o in new if o.type == "MESH" and o.name.startswith("Icosphere")]:
        new.remove(o)
        bpy.data.objects.remove(o, do_unlink=True)
    arm = next((o for o in new if o.type == "ARMATURE"), None)
    mesh = next(o for o in new if o.type == "MESH")
    return arm, mesh


def shape(m, head, vox, strength):
    """A smoothed copy of mesh object `m` (with `head` for head pieces) at `strength` 1 (strong) or 0
    (gentle); None when nothing is left."""
    s = m.copy()
    s.data = m.data.copy()
    bpy.context.scene.collection.objects.link(s)
    for md in list(s.modifiers):
        s.modifiers.remove(md)
    s.vertex_groups.clear()
    s.parent = None
    s.matrix_world = m.matrix_world.copy()
    if head is not None:
        bm = bmesh.new()
        bm.from_mesh(head.data)
        bm.from_mesh(m.data)
        bm.to_mesh(s.data)
        bm.free()
    weld(s.data)
    bpy.ops.object.select_all(action="DESELECT")
    s.select_set(True)
    if strength == 1 and head is not None:
        apply(s, "REMESH", mode="VOXEL", voxel_size=vox * 0.49)
        apply(s, "LAPLACIANSMOOTH", lambda_factor=1.5, iterations=50, use_volume_preserve=True, use_normalized=True)
    elif strength == 1:
        apply(s, "REMESH", mode="VOXEL", voxel_size=vox * 0.36)
        apply(s, "LAPLACIANSMOOTH", lambda_factor=1.5, iterations=30, use_volume_preserve=True, use_normalized=True)
    else:
        apply(s, "REMESH", mode="VOXEL", voxel_size=vox * 0.2)
        apply(s, "LAPLACIANSMOOTH", lambda_factor=0.6, iterations=10, use_volume_preserve=True, use_normalized=True)
    if head is not None:
        # Only the part over the piece, lifted a little off the head.
        th, tm = tree(head.data), tree(m.data)
        bm = bmesh.new()
        bm.from_mesh(s.data)
        far = [f for f in bm.faces if (tm.find_nearest(f.calc_center_median())[3] or 9e9) > (th.find_nearest(f.calc_center_median())[3] or 9e9) - vox * 0.13]
        bmesh.ops.delete(bm, geom=far, context="FACES")
        for v in bm.verts:
            v.co += v.normal * vox * 0.18
        bm.to_mesh(s.data)
        bm.free()
    if len(s.data.polygons) == 0:
        bpy.data.objects.remove(s, do_unlink=True)
        return None
    apply(s, "DECIMATE", ratio=min(1.0, FACES / max(1, len(s.data.polygons))))
    return s


def strays(m, s, vox):
    """The share of the voxel model's vertices outside the smoothed surface and farther than 2.5
    voxels from it: a part melted away. (Vertices inside it are hidden anyway: the legs under a
    skirt.)"""
    t = tree(s.data)
    far = 0
    for v in m.data.vertices:
        loc, normal, _, dist = t.find_nearest(v.co)
        if loc is None or (dist > 2.5 * vox and (v.co - loc).dot(normal) > 0):
            far += 1
    return far / max(1, len(m.data.vertices))


def finish(m, s, arm):
    """Colours, weights and the armature of the voxel model `m` onto smoothed `s`."""
    s.data.materials.clear()
    for mt in m.data.materials:
        s.data.materials.append(mt)
    t = tree(m.data)
    src = [p.material_index for p in m.data.polygons]
    for p in s.data.polygons:
        hit = t.find_nearest(p.center)
        if hit[2] is not None:
            p.material_index = src[hit[2]]
    bpy.context.view_layer.objects.active = s
    bpy.ops.object.shade_smooth()
    for g in m.vertex_groups:
        s.vertex_groups.new(name=g.name)
    dt = s.modifiers.new("dt", "DATA_TRANSFER")
    dt.object = m
    dt.use_vert_data = True
    dt.data_types_verts = {"VGROUP_WEIGHTS"}
    dt.vert_mapping = "POLYINTERP_NEAREST"
    dt.layers_vgroup_select_src = "ALL"
    dt.layers_vgroup_select_dst = "NAME"
    bpy.ops.object.modifier_apply(modifier="dt")
    if arm is not None:
        s.parent = arm
        am = s.modifiers.new("Armature", "ARMATURE")
        am.object = arm


def smooth(job):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    head = None
    if job["mode"] == "head_piece":
        _, head = load(job["head"])
        bpy.data.objects.remove(_, do_unlink=True) if _ else None
        for md in list(head.modifiers):
            head.modifiers.remove(md)
    arm, m = load(job["file"])
    names = {mt.name: mt.name.split(".")[0] for mt in m.data.materials}
    vox = voxel_size(m.data)
    if head is not None:
        # (The head's rest pose: the same skeleton, so model space matches.)
        head.parent = None
    got, how = None, "voxel"
    for strength, label in ((1, "strong"), (0, "gentle")):
        s = shape(m, head, vox, strength)
        if s is not None and strays(m, s, vox) < 0.02:
            got, how = s, label
            break
        if s is not None:
            bpy.data.objects.remove(s, do_unlink=True)
    if got is None:
        return "voxel", len(m.data.vertices), sum(len(p.vertices) - 2 for p in m.data.polygons)
    if head is not None:
        bpy.data.objects.remove(head, do_unlink=True)
    finish(m, got, arm)
    bpy.data.objects.remove(m, do_unlink=True)
    got.name = os.path.splitext(os.path.basename(job["file"]))[0]
    for mt in got.data.materials:
        mt.name = names.get(mt.name, mt.name)
    bpy.ops.object.select_all(action="DESELECT")
    got.select_set(True)
    if arm is not None:
        arm.select_set(True)
    bpy.ops.export_scene.gltf(filepath=job["file"], export_format="GLB", use_selection=True, export_animations=False, export_yup=True, export_apply=False)
    return how, len(got.data.vertices), sum(len(p.vertices) - 2 for p in got.data.polygons)


def main():
    jobs = json.load(open(sys.argv[sys.argv.index("--") + 1]))
    for job in jobs:
        how, nv, nt = smooth(job)
        print("SMOOTHED", job["file"], how, nv, nt, flush=True)


if __name__ == "__main__":
    main()
