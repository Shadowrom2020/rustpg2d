"""Runs Meshy image-to-image jobs. Every job's task id is kept in WORK/gen_state.json, so a rerun
never pays twice; results land in WORK/gen/<name>.png (existing ones are skipped).

usage: gen.py JOBS.json [names...]
JOBS.json: {name: {"prompt", "model", "refs": [paths], "aspect": "3:4", "nobg": false}}"""
import base64, io, json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import meshy
from common import WORK
from PIL import Image

OUT = os.path.join(WORK, "gen")
os.makedirs(OUT, exist_ok=True)
SP = os.path.join(WORK, "gen_state.json")
MAX = 8  # tasks in flight (the plan limits queued tasks: HTTP 429 beyond)


def uri(path, cache={}):
    if path not in cache:
        im = Image.open(path)
        buf = io.BytesIO()
        if im.mode == "RGB" or path.endswith(".jpg"):
            im.convert("RGB").save(buf, "JPEG", quality=90)
            cache[path] = "data:image/jpeg;base64," + base64.b64encode(buf.getvalue()).decode()
        else:
            im.save(buf, "PNG", optimize=True)
            cache[path] = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()
    return cache[path]


def main():
    st = json.load(open(SP)) if os.path.exists(SP) else {}
    jobs = json.load(open(sys.argv[1]))
    only = sys.argv[2:]
    names = [n for n in jobs if not only or n in only]
    todo = [n for n in names if not os.path.exists(os.path.join(OUT, n + ".png"))]
    flying = [n for n in todo if n in st]
    waiting = [n for n in todo if n not in st]
    while flying or waiting:
        while waiting and len(flying) < MAX:
            n = waiting.pop(0)
            j = jobs[n]
            body = {"ai_model": j["model"], "prompt": j["prompt"], "reference_image_urls": [uri(p) for p in j["refs"]], "remove_background": not j.get("nobg")}
            if j.get("aspect"):
                body["aspect_ratio"] = j["aspect"]
            try:
                st[n] = meshy.call("POST", "/openapi/v1/image-to-image", body)["result"]
            except SystemExit as e:
                if "429" in str(e):
                    waiting.insert(0, n)
                    break
                raise
            json.dump(st, open(SP, "w"), indent=1)
            flying.append(n)
        time.sleep(8)
        for n in list(flying):
            t = meshy.call("GET", f"/openapi/v1/image-to-image/{st[n]}")
            if t["status"] in ("SUCCEEDED", "FAILED", "CANCELED"):
                flying.remove(n)
                if t["status"] == "SUCCEEDED":
                    meshy.download(t["image_urls"][0], os.path.join(OUT, n + ".png"))
                    print("ok", n, t.get("consumed_credits"), flush=True)
                else:
                    print("FAILED", n, t.get("task_error"), flush=True)
                    st.pop(n, None)
                    json.dump(st, open(SP, "w"), indent=1)
    print("balance", meshy.call("GET", "/openapi/v1/balance")["balance"])


if __name__ == "__main__":
    main()
