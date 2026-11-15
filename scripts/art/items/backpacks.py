"""blender -b --factory-startup -P scripts/art/items/backpacks.py -- OUTDIR: the example's three backpacks as GLB
(Y up, front/pockets +Z, the back panel -Z against the wearer). Plain coloured materials, low poly."""
import bpy, sys, math, os
out = sys.argv[sys.argv.index('--') + 1]

def clear():
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete()
    for m in list(bpy.data.materials):
        bpy.data.materials.remove(m)

def mat(name, rgb, emit=0.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes['Principled BSDF']
    b.inputs['Base Color'].default_value = (*rgb, 1.0)
    b.inputs['Roughness'].default_value = 0.8
    if emit:
        b.inputs['Emission Color'].default_value = (*rgb, 1.0)
        b.inputs['Emission Strength'].default_value = emit
    return m

def box(name, size, loc, m, bevel=0.0, seg=1):
    # Blender is Z up; glTF export turns it to Y up: (x, y, z) here = (x, -z?, y). We model in
    # Blender axes: X right, Y back (-> glTF -Z), Z up (-> glTF Y). So the front is Blender -Y.
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    o = bpy.context.active_object
    o.name = name
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)
    if bevel:
        b = o.modifiers.new('bevel', 'BEVEL')
        b.width = bevel
        b.segments = seg
    o.data.materials.append(m)
    return o

def cyl(name, r, depth, loc, rot, m, verts=10):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=r, depth=depth, location=loc, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    o.data.materials.append(m)
    return o

def straps(m, w, h, depth):
    # Two shoulder straps on the back panel (Blender +Y side).
    for x in (-w * 0.25, w * 0.25):
        box('strap', (w * 0.12, 0.02, h * 0.85), (x, depth / 2 + 0.012, h * 0.5), m, 0.004)

def pack(kind):
    clear()
    if kind == 'small':
        body, flap, trim, strap = mat('canvas', (0.42, 0.30, 0.18)), mat('flap', (0.30, 0.20, 0.11)), mat('buckle', (0.72, 0.62, 0.35)), mat('strap', (0.22, 0.14, 0.08))
        w, h, d = 0.30, 0.36, 0.16
        box('body', (w, d, h), (0, 0, h / 2), body, 0.03, 2)
        box('flap', (w * 1.02, d * 1.04, h * 0.32), (0, 0, h * 0.86), flap, 0.025, 2)
        box('pocket', (w * 0.62, d * 0.35, h * 0.34), (0, -d / 2 - d * 0.12, h * 0.32), flap, 0.02, 2)
        box('buckle', (0.04, 0.02, 0.04), (0, -d / 2 - 0.012, h * 0.74), trim)
        straps(strap, w, h, d)
    elif kind == 'big':
        body, flap, trim, strap, roll = mat('canvas', (0.30, 0.38, 0.22)), mat('flap', (0.22, 0.29, 0.16)), mat('buckle', (0.70, 0.60, 0.33)), mat('strap', (0.22, 0.14, 0.08)), mat('bedroll', (0.55, 0.22, 0.16))
        w, h, d = 0.38, 0.52, 0.22
        box('body', (w, d, h), (0, 0, h / 2), body, 0.035, 2)
        box('flap', (w * 1.02, d * 1.04, h * 0.25), (0, 0, h * 0.89), flap, 0.03, 2)
        box('pocket', (w * 0.7, d * 0.32, h * 0.3), (0, -d / 2 - d * 0.1, h * 0.3), flap, 0.02, 2)
        for x in (-w / 2 - 0.035, w / 2 + 0.035):
            box('side', (0.07, d * 0.6, h * 0.35), (x, 0, h * 0.35), flap, 0.02, 2)
        box('buckle', (0.05, 0.02, 0.05), (0, -d / 2 - 0.014, h * 0.8), trim)
        cyl('bedroll', 0.075, w * 1.15, (0, 0, h + 0.07), (0, math.pi / 2, 0), roll, 12)
        for x in (-w * 0.3, w * 0.3):
            cyl('tie', 0.081, 0.025, (x, 0, h + 0.07), (0, math.pi / 2, 0), strap, 12)
        straps(strap, w, h, d)
    else:
        body, flap, trim, strap, gem = mat('cloth', (0.28, 0.16, 0.42)), mat('flap', (0.18, 0.10, 0.30)), mat('gold', (0.85, 0.68, 0.25)), mat('strap', (0.16, 0.10, 0.20)), mat('gem', (0.35, 0.85, 1.0), 4.0)
        w, h, d = 0.32, 0.40, 0.17
        box('body', (w, d, h), (0, 0, h / 2), body, 0.04, 2)
        box('flap', (w * 1.03, d * 1.05, h * 0.34), (0, 0, h * 0.85), flap, 0.03, 2)
        box('trim', (w * 1.05, d * 1.07, 0.02), (0, 0, h * 0.68), trim)
        box('trim2', (0.02, d * 1.07, h * 0.66), (0, 0, h * 0.34), trim)
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.045, location=(0, -d / 2 - 0.03, h * 0.82))
        g = bpy.context.active_object
        g.scale = (1, 0.6, 1.25)
        g.data.materials.append(gem)
        box('pocket', (w * 0.5, d * 0.3, h * 0.28), (0, -d / 2 - d * 0.1, h * 0.28), flap, 0.02, 2)
        straps(strap, w, h, d)
    # One mesh (modifiers applied), then export. Blender's front is -Y = glTF +Z.
    bpy.ops.object.select_all(action='SELECT')
    bpy.context.view_layer.objects.active = bpy.data.objects['body']
    bpy.ops.object.convert(target='MESH')
    bpy.ops.object.join()
    bpy.context.active_object.name = f'backpack_{kind}'
    bpy.ops.object.select_all(action='SELECT')
    path = os.path.join(out, f'backpack_{kind}.glb')
    bpy.ops.export_scene.gltf(filepath=path, export_format='GLB', export_yup=True, export_apply=True, use_selection=True)
    print('wrote', path)

for k in ('small', 'big', 'magic'):
    pack(k)
