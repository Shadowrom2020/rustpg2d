"""Rig templates for unrigged 3D models (animals and creatures; Meshy rigs only people): one
`.glb` per body plan with a skeleton, a plain stand-in mesh bound to it and the clips Walk, Idle,
Attack and Hurt. The editor fits a template's skeleton to a model (*Database > 3D models > Rig
template*, `Model::fit_rig`), so the stand-in only gives the proportions the fit stretches from.

usage: blender -b -P templates.py -- OUT_DIR [NAME...]

Blender's axes: the creature faces -Y (the glTF front, +Z, after export), +Z is up. Bones point
from their joint to the next; joint names avoid the words the engine reads as a person's bones
(hand, arm with left/right...). Clips are keyed every two frames at 24 fps from functions of the
clip's progress t (0..1): {bone: (rotation degrees XYZ, location, scale)}."""
import math
import os
import sys

import bpy
from mathutils import Vector

FPS = 24


def wave(t, k=1.0, phase=0.0):
    return math.sin(2 * math.pi * (k * t + phase))


def curve(t, keys):
    """Piecewise linear through (t, value) keys."""
    for (t0, v0), (t1, v1) in zip(keys, keys[1:]):
        if t0 <= t <= t1:
            return v0 + (v1 - v0) * (t - t0) / max(t1 - t0, 1e-6)
    return keys[-1][1]


def build(name, bones, parts, clips, color, out_dir):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.fps = FPS
    data = bpy.data.armatures.new(name)
    arm = bpy.data.objects.new("Armature", data)
    scene.collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="EDIT")
    for bone, head, tail, parent in bones:
        eb = data.edit_bones.new(bone)
        eb.head, eb.tail = head, tail
        eb.roll = 0.0
        if parent:
            eb.parent = data.edit_bones[parent]
            eb.use_connect = (Vector(data.edit_bones[parent].tail) - Vector(head)).length < 1e-4
    bpy.ops.object.mode_set(mode="OBJECT")

    mat = bpy.data.materials.new(name + "_skin")
    mat.use_nodes = True
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = color
    objs = []
    for bone, kind, loc, size in parts:
        if kind == "ball":
            bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=8, radius=1, location=loc)
        else:
            bpy.ops.mesh.primitive_cube_add(size=2, location=loc)
        o = bpy.context.object
        o.scale = size
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
        g = o.vertex_groups.new(name=bone)
        g.add([v.index for v in o.data.vertices], 1.0, "REPLACE")
        o.data.materials.append(mat)
        objs.append(o)
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    body = bpy.context.object
    body.name = name + "_body"
    body.modifiers.new("Armature", "ARMATURE").object = arm
    body.parent = arm

    arm.animation_data_create()
    for pb in arm.pose.bones:
        pb.rotation_mode = "XYZ"
    for clip, seconds, fn in clips:
        act = bpy.data.actions.new(clip)
        act.use_fake_user = True
        arm.animation_data.action = act
        frames = max(2, 2 * round(seconds * FPS / 2))
        for f in list(range(0, frames, 2)) + [frames]:
            pose = fn(f / frames)
            for pb in arm.pose.bones:
                v = tuple(pose.get(pb.name, ()))
                rot, loc, scale = v + ((0, 0, 0), (0, 0, 0), (1, 1, 1))[len(v):]
                pb.rotation_euler = [math.radians(a) for a in rot]
                pb.location = loc
                pb.scale = scale
                for path in ("rotation_euler", "location", "scale"):
                    pb.keyframe_insert(path, frame=f)
    arm.animation_data.action = None
    path = os.path.join(out_dir, f"rig_{name}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", export_animation_mode="ACTIONS", export_yup=True, export_force_sampling=True, export_frame_step=2)
    print("WROTE", path)


def P(**bones):
    """A pose: bone=(rot) or bone=(rot, loc) or bone=(rot, loc, scale)."""
    out = {}
    for b, v in bones.items():
        rot = v[0]
        loc = v[1] if len(v) > 1 else (0, 0, 0)
        scale = v[2] if len(v) > 2 else (1, 1, 1)
        out[b] = (rot, loc, scale)
    return out


# ------------------------------------------------------------------ four legs (wolf, dog, boar, deer)

def quadruped(out_dir):
    bones = [("Hips", (0, 0.32, 0.62), (0, 0.05, 0.64), None), ("Spine", (0, 0.05, 0.64), (0, -0.25, 0.66), "Hips"),
             ("Neck", (0, -0.25, 0.66), (0, -0.42, 0.80), "Spine"), ("Head", (0, -0.42, 0.80), (0, -0.66, 0.76), "Neck"),
             ("Tail", (0, 0.32, 0.62), (0, 0.52, 0.62), "Hips"), ("TailTip", (0, 0.52, 0.62), (0, 0.72, 0.54), "Tail")]
    parts = [("Hips", "ball", (0, 0.18, 0.62), (0.15, 0.22, 0.15)), ("Spine", "ball", (0, -0.12, 0.64), (0.17, 0.22, 0.17)),
             ("Neck", "ball", (0, -0.34, 0.73), (0.08, 0.1, 0.1)), ("Head", "box", (0, -0.53, 0.79), (0.08, 0.14, 0.08)),
             ("Tail", "box", (0, 0.44, 0.62), (0.04, 0.1, 0.04)), ("TailTip", "box", (0, 0.63, 0.58), (0.03, 0.1, 0.03))]
    for side, x in (("L", 0.12), ("R", -0.12)):
        for end, y, parent in (("Front", -0.22, "Spine"), ("Back", 0.3, "Hips")):
            bones += [(f"{end}Leg_{side}", (x, y, 0.58), (x, y + 0.02, 0.32), parent), (f"{end}Shin_{side}", (x, y + 0.02, 0.32), (x, y, 0.06), f"{end}Leg_{side}"),
                      (f"{end}Paw_{side}", (x, y, 0.06), (x, y - 0.06, 0.02), f"{end}Shin_{side}")]
            parts += [(f"{end}Leg_{side}", "box", (x, y + 0.01, 0.45), (0.04, 0.04, 0.14)), (f"{end}Shin_{side}", "box", (x, y + 0.01, 0.19), (0.032, 0.032, 0.13)),
                      (f"{end}Paw_{side}", "box", (x, y - 0.03, 0.03), (0.04, 0.05, 0.03))]

    def legs(t, swing, bend):
        # Diagonal pairs move together (a trot); a shin bends while its leg comes forward.
        out = {}
        for leg, s in (("FrontLeg_L", 1), ("BackLeg_R", 1), ("FrontLeg_R", -1), ("BackLeg_L", -1)):
            out[leg] = ((swing * s * wave(t), 0, 0),)
            out[leg.replace("Leg", "Shin")] = ((bend * max(0.0, s * wave(t, 1, 0.25)), 0, 0),)
        return out

    def walk(t):
        p = legs(t, 28, 35)
        p.update(P(Hips=((0, 0, 0), (0, 0, 0.015 * wave(t, 2))), Neck=((4 * wave(t, 2), 0, 0),), Tail=((0, 0, 14 * wave(t)),), TailTip=((0, 0, 10 * wave(t, 1, 0.15)),)))
        return p

    def idle(t):
        return P(Spine=((1.5 * wave(t), 0, 0),), Neck=((3 * wave(t), 0, 0),), Head=((0, 0, 14 * wave(t)),), Tail=((0, 0, 8 * wave(t)),), TailTip=((0, 0, 8 * wave(t, 1, 0.2)),))

    def attack(t):
        a = curve(t, [(0, 0), (0.35, -1), (0.55, 1), (0.75, 1), (1, 0)])
        p = P(Hips=((0, 0, 0), (0, 0.12 * a, -0.03 * abs(a))), Neck=((-18 * a, 0, 0),), Head=((-10 * max(a, 0), 0, 0),),
              FrontLeg_L=((-20 * max(a, 0), 0, 0),), FrontLeg_R=((-20 * max(a, 0), 0, 0),), Tail=((12 * a, 0, 0),))
        return p

    def hurt(t):
        a = curve(t, [(0, 0), (0.2, 1), (1, 0)])
        return P(Hips=((0, 0, 0), (0, -0.07 * a, 0)), Spine=((-6 * a, 0, 0),), Neck=((20 * a, 0, 0),), Head=((0, 0, 12 * a),), Tail=((-20 * a, 0, 0),))

    build("quadruped", bones, parts, [("Walk", 0.8, walk), ("Idle", 2.0, idle), ("Attack", 0.8, attack), ("Hurt", 0.5, hurt)], (0.55, 0.5, 0.45, 1), out_dir)


# ------------------------------------------------------------------ bird (chicken, crow, owl standing)

def bird(out_dir):
    bones = [("Hips", (0, 0.06, 0.42), (0, -0.04, 0.58), None), ("Chest", (0, -0.04, 0.58), (0, -0.1, 0.74), "Hips"),
             ("Neck", (0, -0.1, 0.74), (0, -0.14, 0.86), "Chest"), ("Head", (0, -0.14, 0.86), (0, -0.34, 0.88), "Neck"),
             ("Tail", (0, 0.06, 0.42), (0, 0.34, 0.36), "Hips")]
    parts = [("Hips", "ball", (0, 0.04, 0.48), (0.14, 0.18, 0.13)), ("Chest", "ball", (0, -0.06, 0.64), (0.13, 0.13, 0.13)),
             ("Neck", "ball", (0, -0.12, 0.8), (0.06, 0.06, 0.07)), ("Head", "ball", (0, -0.2, 0.89), (0.07, 0.09, 0.07)),
             ("Tail", "box", (0, 0.24, 0.38), (0.08, 0.1, 0.02))]
    for side, x in (("L", 1), ("R", -1)):
        bones += [(f"Wing_{side}", (0.12 * x, -0.04, 0.66), (0.15 * x, 0.18, 0.52), "Chest"), (f"WingTip_{side}", (0.15 * x, 0.18, 0.52), (0.15 * x, 0.38, 0.42), f"Wing_{side}"),
                  (f"Thigh_{side}", (0.06 * x, 0.02, 0.4), (0.06 * x, 0.0, 0.2), "Hips"), (f"Shank_{side}", (0.06 * x, 0.0, 0.2), (0.06 * x, 0.0, 0.03), f"Thigh_{side}"),
                  (f"Toe_{side}", (0.06 * x, 0.0, 0.03), (0.06 * x, -0.1, 0.0), f"Shank_{side}")]
        parts += [(f"Wing_{side}", "box", (0.14 * x, 0.07, 0.6), (0.02, 0.12, 0.08)), (f"WingTip_{side}", "box", (0.15 * x, 0.28, 0.47), (0.015, 0.1, 0.05)),
                  (f"Thigh_{side}", "box", (0.06 * x, 0.01, 0.3), (0.03, 0.03, 0.1)), (f"Shank_{side}", "box", (0.06 * x, 0.0, 0.11), (0.012, 0.012, 0.09)),
                  (f"Toe_{side}", "box", (0.06 * x, -0.05, 0.015), (0.03, 0.05, 0.012))]

    def walk(t):
        s = wave(t)
        return P(Thigh_L=((30 * s, 0, 0),), Thigh_R=((-30 * s, 0, 0),), Shank_L=((-20 * max(s, 0), 0, 0),), Shank_R=((-20 * max(-s, 0), 0, 0),),
                 Hips=((0, 6 * s, 0), (0, 0, 0.012 * wave(t, 2))), Neck=((8 * wave(t, 2), 0, 0),), Tail=((0, 0, 6 * s),))

    def idle(t):
        return P(Neck=((4 * wave(t), 0, 0),), Head=((0, 0, 25 * wave(t, 1)),), Tail=((5 * wave(t, 2), 0, 0),), Wing_L=((0, 0, 3 * wave(t)),), Wing_R=((0, 0, -3 * wave(t)),))

    def attack(t):
        a = curve(t, [(0, 0), (0.3, -1), (0.5, 1), (0.7, 1), (1, 0)])
        o = max(a, 0)
        return P(Chest=((-25 * a, 0, 0),), Neck=((-25 * o, 0, 0),), Head=((-15 * o, 0, 0),), Wing_L=((0, 0, 35 * o),), Wing_R=((0, 0, -35 * o),), Tail=((15 * a, 0, 0),))

    def hurt(t):
        a = curve(t, [(0, 0), (0.2, 1), (1, 0)])
        return P(Hips=((0, 0, 0), (0, -0.05 * a, 0.03 * a)), Chest=((15 * a, 0, 0),), Neck=((15 * a, 0, 0),), Wing_L=((0, 0, 40 * a),), Wing_R=((0, 0, -40 * a),))

    build("bird", bones, parts, [("Walk", 0.6, walk), ("Idle", 2.0, idle), ("Attack", 0.7, attack), ("Hurt", 0.5, hurt)], (0.6, 0.55, 0.45, 1), out_dir)


# ------------------------------------------------------------------ flyer (bat, bird in flight, wyvern)

def flyer(out_dir):
    bones = [("Hips", (0, 0.04, 0.45), (0, 0.0, 0.62), None), ("Chest", (0, 0.0, 0.62), (0, -0.02, 0.76), "Hips"),
             ("Head", (0, -0.02, 0.76), (0, -0.08, 0.92), "Chest")]
    parts = [("Hips", "ball", (0, 0.03, 0.54), (0.08, 0.07, 0.1)), ("Chest", "ball", (0, 0.0, 0.69), (0.09, 0.08, 0.08)), ("Head", "ball", (0, -0.04, 0.84), (0.07, 0.07, 0.07))]
    for side, x in (("L", 1), ("R", -1)):
        bones += [(f"Wing_{side}", (0.07 * x, 0.0, 0.7), (0.4 * x, 0.02, 0.76), "Chest"), (f"WingTip_{side}", (0.4 * x, 0.02, 0.76), (0.72 * x, 0.04, 0.6), f"Wing_{side}"),
                  (f"Foot_{side}", (0.04 * x, 0.04, 0.47), (0.05 * x, 0.06, 0.3), "Hips")]
        parts += [(f"Wing_{side}", "box", (0.24 * x, 0.02, 0.66), (0.17, 0.008, 0.12)), (f"WingTip_{side}", "box", (0.56 * x, 0.03, 0.6), (0.16, 0.008, 0.1)),
                  (f"Foot_{side}", "box", (0.045 * x, 0.05, 0.38), (0.012, 0.012, 0.08))]

    def flap(t, k, amp):
        s = wave(t, k)
        # About each wing's local X (mirrored on the two sides: the same sign flaps both).
        tip = 0.6 * amp * wave(t, k, -0.1)
        return {"Wing_L": ((amp * s, 0, 0),), "Wing_R": ((amp * s, 0, 0),), "WingTip_L": ((tip, 0, 0),), "WingTip_R": ((tip, 0, 0),)}

    def walk(t):
        p = P(**flap(t, 2, 40))
        p.update(P(Hips=((10, 0, 0), (0, 0, 0.04 * wave(t, 2, 0.25)))))
        return p

    def idle(t):
        p = P(**flap(t, 2, 30))
        p.update(P(Hips=((0, 0, 0), (0, 0, 0.05 * wave(t, 2, 0.25))), Head=((0, 0, 10 * wave(t)),)))
        return p

    def attack(t):
        a = curve(t, [(0, 0), (0.3, -1), (0.55, 1), (0.75, 1), (1, 0)])
        p = P(**flap(t, 2, 20))
        p.update(P(Hips=((35 * max(a, 0) - 15 * max(-a, 0), 0, 0), (0, 0.15 * a, -0.05 * max(a, 0))), Head=((-15 * max(a, 0), 0, 0),),
                   Foot_L=((-50 * max(a, 0), 0, 0),), Foot_R=((-50 * max(a, 0), 0, 0),)))
        return p

    def hurt(t):
        a = curve(t, [(0, 0), (0.2, 1), (1, 0)])
        return P(Hips=((-20 * a, 0, 0), (0, -0.08 * a, 0.03 * a)), Wing_L=((-50 * a, 0, 0),), Wing_R=((-50 * a, 0, 0),))

    build("flyer", bones, parts, [("Walk", 0.5, walk), ("Idle", 1.0, idle), ("Attack", 0.8, attack), ("Hurt", 0.5, hurt)], (0.35, 0.3, 0.32, 1), out_dir)


# ------------------------------------------------------------------ blob (slime, jelly, mushroom thing)

def blob(out_dir):
    bones = [("Hips", (0, 0, 0.0), (0, 0, 0.22), None), ("Body", (0, 0, 0.22), (0, 0, 0.46), "Hips"), ("Top", (0, 0, 0.46), (0, 0, 0.66), "Body")]
    parts = [("Hips", "ball", (0, 0, 0.14), (0.4, 0.4, 0.14)), ("Body", "ball", (0, 0, 0.32), (0.34, 0.34, 0.16)), ("Top", "ball", (0, 0, 0.5), (0.22, 0.22, 0.16))]

    def squash(s):
        return (1 + 0.5 * s, 1 - s, 1 + 0.5 * s)

    def walk(t):
        h = max(0.0, wave(t, 1, 0.0))
        sq = curve(t, [(0, 0.15), (0.12, -0.12), (0.4, -0.05), (0.5, 0.0), (0.88, 0.0), (1, 0.15)])
        return P(Hips=((0, 0, 0), (0, 0.12 * h, 0), squash(sq)), Top=((6 * wave(t), 0, 0),))

    def idle(t):
        return P(Hips=((0, 0, 0), (0, 0, 0), squash(0.05 * wave(t))), Top=((0, 0, 0), (0, 0, 0), squash(-0.06 * wave(t, 1, 0.1))))

    def attack(t):
        a = curve(t, [(0, 0), (0.35, -1), (0.55, 1), (0.75, 0.6), (1, 0)])
        return P(Hips=((0, 0, 0), (0, 0.1 * max(a, 0), -0.12 * a), squash(0.25 * max(-a, 0) - 0.15 * max(a, 0))), Body=((-20 * max(a, 0), 0, 0),))

    def hurt(t):
        a = curve(t, [(0, 0), (0.15, 1), (1, 0)])
        return P(Hips=((0, 0, 0), (0, 0, 0.06 * a), squash(0.3 * a)), Top=((15 * a, 0, 0),))

    build("blob", bones, parts, [("Walk", 0.8, walk), ("Idle", 1.6, idle), ("Attack", 0.8, attack), ("Hurt", 0.5, hurt)], (0.4, 0.7, 0.4, 1), out_dir)


# ------------------------------------------------------------------ floating (ghost, wisp, floating eye)

def floating(out_dir):
    bones = [("Hips", (0, 0, 0.3), (0, 0, 0.6), None), ("Chest", (0, 0, 0.6), (0, 0, 0.8), "Hips"), ("Head", (0, 0, 0.8), (0, 0, 1.0), "Chest"),
             ("Trail", (0, 0, 0.3), (0, 0.04, 0.14), "Hips"), ("TrailTip", (0, 0.04, 0.14), (0, 0.1, 0.0), "Trail")]
    parts = [("Hips", "ball", (0, 0, 0.45), (0.2, 0.18, 0.17)), ("Chest", "ball", (0, 0, 0.7), (0.19, 0.17, 0.12)), ("Head", "ball", (0, 0, 0.88), (0.15, 0.15, 0.14)),
             ("Trail", "ball", (0, 0.02, 0.22), (0.13, 0.12, 0.1)), ("TrailTip", "ball", (0, 0.06, 0.07), (0.06, 0.06, 0.08))]
    for side, x in (("L", 1), ("R", -1)):
        bones += [(f"Limb_{side}", (0.17 * x, 0, 0.74), (0.32 * x, -0.04, 0.56), "Chest")]
        parts += [(f"Limb_{side}", "box", (0.25 * x, -0.02, 0.65), (0.05, 0.04, 0.1))]

    def drift(t, k=1):
        return {"Hips": ((0, 0, 0), (0, 0, 0.04 * wave(t, k))), "Trail": ((10 * wave(t, k, 0.2), 0, 8 * wave(t, k)),), "TrailTip": ((14 * wave(t, k, 0.4), 0, 10 * wave(t, k, 0.2)),)}

    def walk(t):
        p = P(**drift(t, 2))
        p["Hips"] = ((12, 0, 0), p["Hips"][1], (1, 1, 1))
        p.update(P(Limb_L=((10 * wave(t, 2), 0, 0),), Limb_R=((-10 * wave(t, 2), 0, 0),)))
        return p

    def idle(t):
        p = P(**drift(t, 1))
        p.update(P(Head=((0, 0, 10 * wave(t)),), Limb_L=((0, 0, 6 * wave(t)),), Limb_R=((0, 0, -6 * wave(t)),)))
        return p

    def attack(t):
        a = curve(t, [(0, 0), (0.3, -1), (0.55, 1), (0.75, 1), (1, 0)])
        o = max(a, 0)
        return P(Hips=((20 * a, 0, 0), (0, 0.12 * a, 0)), Limb_L=((-70 * o, 0, -20 * o),), Limb_R=((-70 * o, 0, 20 * o),), Head=((-10 * o, 0, 0),))

    def hurt(t):
        a = curve(t, [(0, 0), (0.2, 1), (1, 0)])
        return P(Hips=((-20 * a, 0, 0), (0, -0.08 * a, 0.03 * a)), Head=((15 * a, 0, 0),), Limb_L=((0, 0, 40 * a),), Limb_R=((0, 0, -40 * a),))

    build("floating", bones, parts, [("Walk", 1.0, walk), ("Idle", 2.0, idle), ("Attack", 0.8, attack), ("Hurt", 0.5, hurt)], (0.75, 0.78, 0.85, 1), out_dir)


TEMPLATES = {"quadruped": quadruped, "bird": bird, "flyer": flyer, "blob": blob, "floating": floating}

if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if not args:
        print(__doc__)
        sys.exit(1)
    out = args[0]
    os.makedirs(out, exist_ok=True)
    for name in args[1:] or list(TEMPLATES):
        TEMPLATES[name](out)
