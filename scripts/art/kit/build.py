"""build.py [--out DIR] [--check] [ID...]: builds the kit's models (all, or the named ones).

Writes DIR/models/<id>.glb, DIR/models/kit_clips.glb, DIR/kit.json (the figures' and pieces'
model definitions) and DIR/gear.json (how each item model is held); the editor's `kit_build` tool
takes all of it into a project. Building everything also writes manifest.json beside this script:
the hash of the kit's sources and of every output, which the repository's test compares.
`--check` builds into a temporary folder and fails when the outputs differ from the manifest.
The same sources always give the same bytes."""
import hashlib
import json
import os
import shutil
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gear  # noqa: E402
import kit  # noqa: E402
from kitlib import RIG  # noqa: E402

SOURCES = ["kitlib.py", "figure.py", "kit.py", "gear.py", "build.py", "rig/human.glb"]


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def source_hash():
    h = hashlib.sha256()
    for f in SOURCES:
        h.update(f.encode() + b"\0" + open(f"{HERE}/{f}", "rb").read())
    return h.hexdigest()


def build(out, only=None):
    os.makedirs(f"{out}/models", exist_ok=True)
    models, fig = kit.everything()
    defs, files = [], {}
    for mid, make, d in models:
        if only and mid not in only:
            continue
        d = dict(d, file=f"models/{d.pop('_file', mid)}.glb")
        head = d.pop("_head", None)
        if make:
            path = f"{out}/models/{mid}.glb"
            st = make().export(path)
            used = st["colors"]
            files[f"models/{mid}.glb"] = dict(sha256=sha(path), vertices=st["vertices"], triangles=st["triangles"], colors=len(used))
        else:
            # A figure of the cast: it reuses a head's file, so that head's colours are its own.
            b, female, race = head
            used = ["#%02x%02x%02x" % c for c in fig(b, female).body(**race).prims]
        feats = [kit.feature(name, shades, used, colors, default) for name, shades, colors, default in d.pop("_features")]
        d["features"] = [f for f in feats if f]
        if "_stand" in d:
            stand = fig(*d.pop("_stand")).stand()
            d["stand"] = round(stand, 4)
            d["height"] = round(40 * stand / 1.70 * 2) / 2
        defs.append(d)
    # The gear: item models and how each is held (the items that use a file take these).
    holds = []
    for name in gear.GEAR:
        if only and name not in only:
            continue
        mesh, hold = gear.build(name)
        path = f"{out}/models/{name}.glb"
        st = mesh.export(path)
        files[f"models/{name}.glb"] = dict(sha256=sha(path), vertices=st["vertices"], triangles=st["triangles"], colors=len(st["colors"]))
        holds.append(dict(file=f"models/{name}.glb", **hold))
    json.dump(holds, open(f"{out}/gear.json", "w"), indent=1)
    if not only:
        shutil.copy(RIG, f"{out}/{kit.CLIP_FILE}")
        files[kit.CLIP_FILE] = dict(sha256=sha(f"{out}/{kit.CLIP_FILE}"))
    json.dump(defs, open(f"{out}/kit.json", "w"), indent=1)
    return defs, files


def main():
    args = sys.argv[1:]
    check = "--check" in args
    out = args[args.index("--out") + 1] if "--out" in args else None
    only = [a for k, a in enumerate(args) if not a.startswith("--") and (k == 0 or args[k - 1] != "--out")]
    manifest = f"{HERE}/manifest.json"
    if check:
        with tempfile.TemporaryDirectory() as tmp:
            _, files = build(tmp)
        old = json.load(open(manifest))
        if old["sources"] != source_hash() or old["files"] != files:
            diff = sorted(k for k in set(files) | set(old["files"]) if files.get(k) != old["files"].get(k))
            raise SystemExit(f"the kit's outputs differ from manifest.json ({len(diff)} files: {', '.join(diff[:6])}…): run build.py and apply.py")
        print("the kit matches its manifest:", len(files), "files")
        return
    if not out:
        raise SystemExit(__doc__)
    defs, files = build(out, only)
    if not only:
        json.dump(dict(sources=source_hash(), source_files=SOURCES, files=files), open(manifest, "w"), indent=1, sort_keys=True)
    size = sum(os.path.getsize(f"{out}/{f}") for f in files)
    print(f"built {len(defs)} figure and piece definitions and {len(files)} files into {out}: {size // 1024} KB, largest {max(v.get('vertices', 0) for v in files.values())} vertices")


if __name__ == "__main__":
    main()
