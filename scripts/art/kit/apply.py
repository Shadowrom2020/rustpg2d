"""apply.py PORT [BUILT_DIR] [--library]: sends the built kit to the project open in an editor
whose AI agent server listens on 127.0.0.1:PORT (start the editor with `mcp on PORT`).

- every model file and the shared clip file go into the project's `models/`;
- every model definition is written (the kit's bodies replace the models of the same ids);
- the project's armour, clothing, cloaks, helmets and hoods get the style they show when worn
  (`wear`), unless they already name one;
- with `--library`, the models are also sent to the editor's asset library.

Without BUILT_DIR the editor builds the kit itself (its `kit_build` tool: it must find the kit's
sources, so start it in the repository or with RUSTPG2D_KIT); with it, the files `build.py --out`
wrote are sent one by one."""
import json
import os
import re
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import kit  # noqa: E402


class Editor:
    """A session with the editor's agent server (MCP over Streamable HTTP)."""

    def __init__(self, port):
        self.url = f"http://127.0.0.1:{port}/mcp"
        self.sid = None
        self.post({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "kit", "version": "1"}}})
        self.post({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def post(self, body):
        h = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        if self.sid:
            h["Mcp-Session-Id"] = self.sid
        req = urllib.request.Request(self.url, data=json.dumps(body).encode(), headers=h, method="POST")
        with urllib.request.urlopen(req, timeout=600) as r:
            self.sid = r.headers.get("Mcp-Session-Id") or self.sid
            raw = r.read().decode()
        msgs = [json.loads(line[5:]) for line in raw.splitlines() if line.startswith("data:") and line[5:].strip()]
        return msgs or ([json.loads(raw)] if raw.strip() else [])

    def call(self, tool, **args):
        for m in self.post({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": tool, "arguments": args}}):
            if "error" in m:
                raise SystemExit(f"{tool}: {m['error']}")
            if "result" in m:
                text = "\n".join(c.get("text", "") for c in m["result"].get("content", []))
                if m["result"].get("isError"):
                    raise SystemExit(f"{tool} {args.get('def', {}).get('id', args.get('path', ''))}: {text}")
                return text
        return ""


def wear_of(entry):
    """The style an item shows when worn, from its definition's text (None = nothing)."""
    ident = re.search(r'id: "([^"]+)"', entry).group(1)
    slot = re.search(r"slot: (?:Some\()?([A-Z][a-z]+)", entry)
    slot = slot.group(1) if slot else ""
    cat = re.search(r"category: Some\((Light|Medium|Heavy)\)", entry)
    if slot == "Cloak":
        return ident, "cloak"
    if slot == "Head":
        return ident, "helmet" if "helm" in ident else "hood" if "hood" in ident else None
    if slot not in ("Body", "Clothing"):
        return ident, None
    if "robe" in ident:
        return ident, "robe"
    if slot == "Clothing":
        return ident, "clothes"
    if "plate" in ident or "splint" in ident:
        return ident, "plate_armor"
    if re.search("mail|chain|scale", ident):
        return ident, "chain_mail"
    return ident, {"Light": "leather_armor", "Medium": "chain_mail", "Heavy": "plate_armor"}.get(cat.group(1) if cat else "", None)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not args:
        raise SystemExit(__doc__)
    ed = Editor(args[0])
    info = ed.call("project_info")
    root = re.search(r" at (.+?) \(engine build", info)
    if len(args) > 1:
        # Files built before: sent one by one (slower: the editor reads its models again and again).
        built = args[1]
        defs = json.load(open(f"{built}/kit.json"))
        files = sorted(os.listdir(f"{built}/models"))
        for f in files:
            ed.call("write_asset", path=f"models/{f}", source=os.path.abspath(f"{built}/models/{f}"))
        # Pieces first: a body names the pieces it wears.
        for d in sorted(defs, key=lambda d: "part" not in d):
            ed.call("upsert_def", kind="models", **{"def": d})
        print(f"{len(files)} files and {len(defs)} model definitions sent")
    else:
        # The editor builds the kit itself and takes everything in one go.
        print(ed.call("kit_build").splitlines()[0][:200])
        defs = [dict(id=m[0]) for m in kit.everything()[0]]
    if root and os.path.exists(f"{root.group(1).strip()}/defs/items.ron"):
        text = open(f"{root.group(1).strip()}/defs/items.ron").read()
        n = 0
        for entry in re.split(r"(?m)^(?=    \(\n        id: )", text)[1:]:
            ident, wear = wear_of(entry)
            has = re.search(r'(?m)^        wear: "([^"]*)"', entry)
            # The kit's own table decides; the rule only fills in items without a style.
            wear = kit.ITEM_WEAR.get(ident, None if has else wear)
            if wear and not (has and has.group(1) == wear):
                ed.call("upsert_def", kind="items", merge=True, **{"def": {"id": ident, "wear": wear}})
                n += 1
        print(f"{n} items show a style when worn")
    else:
        print("the project's folder wasn't found in project_info: items left as they are")
    # What the kit replaced without a figure of the same id.
    for kind in ("characters", "models"):
        listed = ed.call("list_defs", kind=kind)
        for ident in kit.RETIRED:
            if re.search(rf"(?m)^{ident}:", listed):
                ed.call("delete_def", kind=kind, id=ident, force=True)
                print(f"removed {kind[:-1]} {ident}")
    races = ed.call("list_defs", kind="races")
    for race, (m, f) in kit.RACE_BODIES.items():
        if re.search(rf"(?m)^\W*{race}\b", races):
            ed.call("upsert_def", kind="races", merge=True, **{"def": {"id": race, "bodies_male": [m], "bodies_female": [f], "size": 1.0}})
    if "--library" in sys.argv:
        print(ed.call("library_send", assets=[{"kind": "models", "id": d["id"]} for d in defs], replace=True)[:200])


if __name__ == "__main__":
    main()
