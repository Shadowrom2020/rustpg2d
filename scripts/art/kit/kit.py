"""What the kit builds: the builds (proportions), the bodies of the races, and the pieces that fit
them, each with the model definition the library gets (`defs/models.ron`)."""
import figure as F

FEMALE = dict(bulk=.88, rig=dict(legs=.97, torso=.96, arms=.95, shoulders=.86))
# Proportions. Heads are 10 voxels except the small build's 8, so head pieces come in two sizes.
BUILDS = {
    "medium": dict(),
    "dwarf": dict(bulk=1.32, wide=1.24, deep=1.28, rig=dict(legs=.6, torso=.86, arms=.8, shoulders=1.14, hips=1.15)),
    "small": dict(bulk=.8, wide=.76, deep=.82, head=.8, rig=dict(legs=.54, torso=.6, arms=.57, shoulders=.7, hips=.72, neck=.7)),
    "large": dict(bulk=1.2, wide=1.16, deep=1.15, rig=dict(legs=1.06, torso=1.07, arms=1.05, shoulders=1.14, hips=1.06)),
}
SKINS = ["#f6d0b0", "#f2c49b", "#e8b88a", "#d09068", "#b5794c", "#8d5a3b", "#5a3a24"]
HAIRS = ["#1e1612", "#3a2418", "#5d3a1a", "#8a5a2e", "#e0c060", "#d35400", "#c0392b", "#dedede", "#3b3b9e", "#2e8b57"]
CLOTHS = ["#2f4f8f", "#8f2f2f", "#2f6f3f", "#6f4f2f", "#5f2f6f", "#c8a030", "#303038", "#d8d0c0", "#2f7f8f", "#b05a20"]
GREENS = ["#7d9a62", "#6a8a52", "#8aa06a", "#5e7a54", "#96a078"]
REDS = ["#b0483e", "#8f3a4a", "#a0584e", "#7a3a5a", "#c06a4a"]
# id: (name, build, female, what the race adds, skin colours offered, default skin, default hair, default beard)
BODIES = {
    "hero": ("Human", "medium", False, {}, SKINS, None, "short", None),
    "heroine": ("Human (female)", "medium", True, {}, SKINS, None, "long", None),
    "elf": ("Elf", "medium", False, dict(ears=True), SKINS, "#f2c49b", "long", None),
    "elf_f": ("Elf (female)", "medium", True, dict(ears=True), SKINS, "#f2c49b", "ponytail", None),
    "tiefling": ("Tiefling", "medium", False, dict(horns=True, tail=True), REDS, "#b0483e", "short", None),
    "tiefling_f": ("Tiefling (female)", "medium", True, dict(horns=True, tail=True), REDS, "#b0483e", "long", None),
    "dwarf": ("Dwarf", "dwarf", False, {}, SKINS, None, "short", "long"),
    "dwarf_f": ("Dwarf (female)", "dwarf", True, {}, SKINS, None, "bun", None),
    "halfling": ("Halfling", "small", False, {}, SKINS, None, "short", None),
    "halfling_f": ("Halfling (female)", "small", True, {}, SKINS, None, "ponytail", None),
    "orc": ("Half-orc", "large", False, dict(tusks=True), GREENS, "#7d9a62", "mohawk", None),
    "orc_f": ("Half-orc (female)", "large", True, dict(tusks=True), GREENS, "#7d9a62", "ponytail", None),
    "dragonborn": ("Dragonborn", "large", False, dict(dragon=True, tail=True), [], "#b8433a", None, None),
    "dragonborn_f": ("Dragonborn (female)", "large", True, dict(dragon=True, tail=True), [], "#b8433a", None, None),
}
# Races whose figures are the kit's bodies under other ids than the race's own: (men, women).
# Their bodies have the race's size already, so the race's own size factor goes back to 1.
RACE_BODIES = {"halfling": ("halfling", "halfling_f"), "gnome": ("halfling", "halfling_f"), "tiefling": ("tiefling", "tiefling_f")}
# The animations of every body: (animation, clip of rig/human.glb, frames per second).
CLIPS = [("walk", "Walking", 12), ("idle", "Idle_3", 8), ("hurt", "Hit_Reaction", 12), ("sit", "Chair_Sit_Idle_M", 8), ("attack", "Right_Hand_Sword_Slash", 12), ("sneak", "Sneaky_Walk", 12),
         ("craft", "Heavy_Hammer_Swing", 12), ("chop", "Axe_Breathe_and_Look_Around", 12), ("attack_1h", "Right_Hand_Sword_Slash", 12), ("attack_2h", "Heavy_Hammer_Swing", 12),
         ("attack_thrust", "Thrust_Slash", 12), ("attack_bow", "Draw_and_Shoot_from_Back", 12), ("attack_cast", "Right_Jab_from_Guard", 12), ("attack_unarmed", "Right_Jab_from_Guard", 12),
         ("idle_1h", "Axe_Breathe_and_Look_Around", 8), ("idle_2h", "Axe_Breathe_and_Look_Around", 8), ("block_shield", "Sword_Parry_Backward_1", 12), ("block_weapon", "Sword_Parry", 12),
         ("block_2h", "Sword_Parry", 12)]
CLIP_FILE = "models/kit_clips.glb"
OUTFIT_NAMES = dict(clothes="Clothes", leather_armor="Leather armour", chain_mail="Chain mail", plate_armor="Plate armour", robe="Robe")


def build_of(name, female):
    b = {k: (dict(v) if isinstance(v, dict) else v) for k, v in BUILDS[name].items()}
    if female:
        rig = b.setdefault("rig", {})
        for k, v in FEMALE["rig"].items():
            rig[k] = rig.get(k, 1.0) * v
        b["bulk"] = b.get("bulk", 1.0) * FEMALE["bulk"]
        b["female"] = True
    return b


def feature(name, shades, used, colors=None, default=None):
    """A colour feature of the shades the piece really has (None when it has none of them)."""
    shades = [s for s in shades if s in used]
    if not shades:
        return None
    f = {"name": name, "shades": [list(F.rgb(s)) for s in shades]}
    if colors:
        f["colors"] = [list(F.rgb(c)) for c in colors]
    if default:
        f["default"] = list(F.rgb(default))
    return f


def head_size(build):
    return F.rnd(5 * BUILDS[build].get("head", 1.0) * F.R)


def tag(build, female):
    return f"{build}_{'f' if female else 'm'}"


def everything():
    """Every model of the kit: (id, a function building its mesh, its definition without the
    measures the build adds)."""
    figs = {}

    def fig(build, female):
        key = (build, female)
        if key not in figs:
            figs[key] = F.Figure(build_of(build, female))
        return figs[key]
    out = []
    heads = {}                       # head size -> the bodies that can wear its hair and helmets
    for bid, (name, build, female, race, skins, skin, hair, beard) in BODIES.items():
        parts = [f"kit_clothes_{tag(build, female)}"]
        n = head_size(build)
        if not race.get("dragon"):
            heads.setdefault(n, []).append(bid)
            if hair:
                parts.append(f"kit_hair_{hair}_{n}")
            if beard:
                parts.append(f"kit_beard_{beard}_{n}")
        d = dict(id=bid, name=name, shared_clips=[CLIP_FILE], clips=[dict(anim=a, clip=c, fps=float(f)) for a, c, f in CLIPS], playable=True, parts=parts,
                 _features=[("skin", [F.SKIN, F.MOUTH, F.BELLY], skins, skin)], _stand=(build, female))
        if female:
            d["female"] = True
        out.append((bid, (lambda b=build, f=female, r=race: fig(b, f).body(**r)), d))
    for build in BUILDS:
        for female in (False, True):
            t = tag(build, female)
            fits = [b for b, v in BODIES.items() if v[1] == build and v[2] == female]
            for style, label in OUTFIT_NAMES.items():
                d = dict(id=f"kit_{style}_{t}", name=f"{label} ({t.replace('_', ' ')})", part=dict(slot="outfit", style=style, fits=fits),
                         _features=[("skin", [F.SKIN], None, None), ("clothing", [F.CLOTH, F.CLOTH_DARK], CLOTHS, None)])
                out.append((d["id"], (lambda b=build, f=female, s=style: fig(b, f).outfit(s)), d))
            d = dict(id=f"kit_cloak_{t}", name=f"Cloak ({t.replace('_', ' ')})", part=dict(slot="cloak", style="cloak", fits=fits), _features=[])
            out.append((d["id"], (lambda b=build, f=female: fig(b, f).cloak()), d))
    for n, fits in sorted(heads.items()):
        build = "small" if n == head_size("small") else "medium"
        for style in F.HAIR_STYLES:
            d = dict(id=f"kit_hair_{style}_{n}", name=f"{style.capitalize()} hair ({n})", part=dict(slot="hair", style=f"hair_{style}", fits=fits, offer=True), _features=[("hair", [F.HAIR], HAIRS, None)])
            out.append((d["id"], (lambda b=build, s=style: fig(b, False).hair(s)), d))
        for kind in F.BEARDS:
            d = dict(id=f"kit_beard_{kind}_{n}", name=f"{kind.capitalize()} beard ({n})", part=dict(slot="beard", style=f"beard_{kind}", fits=fits, offer=True), _features=[("hair", [F.HAIR], HAIRS, None)])
            out.append((d["id"], (lambda b=build, k=kind: fig(b, False).beard(k)), d))
        for kind in F.HEADWEAR:
            d = dict(id=f"kit_{kind}_{n}", name=f"{kind.capitalize()} ({n})", part=dict(slot="headwear", style=kind, fits=fits, hides=["hair"]),
                     _features=[("clothing", [F.CLOTH, F.CLOTH_DARK], CLOTHS, None)])
            out.append((d["id"], (lambda b=build, k=kind: fig(b, False).headwear(k)), d))
    return out, fig
