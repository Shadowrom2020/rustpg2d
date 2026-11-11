"""Writes WORK/jobs_walk.json and WORK/jobs_attack.json for the cast (cheapest model)."""
import json, os
from cast import CAST
from common import WORK
from prompts import walk, attack

MODEL = "nano-banana-2-lite"
REF_WALK = [os.path.join(WORK, "ref_hero_walk.png"), os.path.join(WORK, "ref_villager_f_walk.png")]
REF_ATTACK = os.path.join(WORK, "ref_guard_attack.png")

w = {f"walk_{n}": {"prompt": walk(d), "model": MODEL, "refs": REF_WALK, "aspect": "2:3"} for n, (_, _, d) in CAST.items()}
a = {f"attack_{n}": {"prompt": attack(d, act), "model": MODEL, "refs": [os.path.join(WORK, "gen", f"walk_{n}.png"), REF_ATTACK], "aspect": "2:3"} for n, (_, act, d) in CAST.items() if act}
json.dump(w, open(os.path.join(WORK, "jobs_walk.json"), "w"), indent=1)
json.dump(a, open(os.path.join(WORK, "jobs_attack.json"), "w"), indent=1)
print(len(w), "walk jobs,", len(a), "attack jobs")
