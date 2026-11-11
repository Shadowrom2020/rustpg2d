"""Minimal Meshy API client. The key is read from $MESHY_API_KEY or ~/.config/meshy/api_key
(never printed or written anywhere)."""
import json, os, sys, urllib.request, urllib.error

BASE = "https://api.meshy.ai"


def key():
    k = os.environ.get("MESHY_API_KEY")
    if not k:
        with open(os.path.expanduser("~/.config/meshy/api_key")) as f:
            k = f.read().strip()
    return k


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method, headers={"Authorization": "Bearer " + key(), "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return json.loads(r.read().decode() or "{}")
    except urllib.error.HTTPError as e:
        raise SystemExit(f"HTTP {e.code} {path}: {e.read().decode()[:300]}")


def download(url, path):
    with urllib.request.urlopen(url, timeout=300) as r, open(path, "wb") as f:
        f.write(r.read())


if __name__ == "__main__":
    print(call("GET", "/openapi/v1/balance"))
