"""extract.py SOURCE.glb: writes rig/human.glb, the kit's skeleton and clips, from a rigged and
animated figure (the hero's Meshy rig, bought once). Only needed again when clips are added.

Kept: the node tree, the skin's joints, every clip's rotations (no finger tracks: kit hands have
no fingers) and the hips' movement. Dropped: the mesh, textures, bone translations and scales (a
build has its own bone lengths)."""
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from kitlib import DT, GENERATOR, RIG, Bin, accessor, read_glb, write_glb  # noqa: E402

j, b = read_glb(sys.argv[1])
out = Bin()
skin = j["skins"][0]
names = [n.get("name", "") for n in j["nodes"]]
hips = names.index("Hips")
cache = {}


def copy(i):
    if i not in cache:
        a = j["accessors"][i]
        cache[i] = out.add(accessor(j, b, i).astype(np.dtype(DT[a["componentType"]])), a["type"], a["componentType"], minmax=a["type"] == "SCALAR")
    return cache[i]


anims = []
for an in j["animations"]:
    chans, samps = [], []
    for ch in an["channels"]:
        node, path = ch["target"]["node"], ch["target"]["path"]
        if path == "scale" or (path == "translation" and node != hips):
            continue
        if "Hand" in names[node] and names[node] not in ("LeftHand", "RightHand"):
            continue
        s = an["samplers"][ch["sampler"]]
        samps.append({"input": copy(s["input"]), "output": copy(s["output"]), "interpolation": s.get("interpolation", "LINEAR")})
        chans.append({"sampler": len(samps) - 1, "target": {"node": node, "path": path}})
    anims.append({"name": an["name"], "channels": chans, "samplers": samps})
nodes = [{k: v for k, v in n.items() if k not in ("mesh", "skin")} for n in j["nodes"]]
mesh_node = next(n.get("name") for n in j["nodes"] if "mesh" in n)
doc = {"asset": {"version": "2.0", "generator": GENERATOR}, "scene": 0, "scenes": j["scenes"], "nodes": nodes, "skins": [{"joints": skin["joints"]}], "animations": anims,
       "extras": {"mesh_node": mesh_node}, "buffers": [{"byteLength": len(out.data)}], "bufferViews": out.views, "accessors": out.accs}
write_glb(RIG, doc, out.data)
print(RIG, os.path.getsize(RIG) // 1024, "KB,", len(anims), "clips:", ", ".join(a["name"] for a in anims))
