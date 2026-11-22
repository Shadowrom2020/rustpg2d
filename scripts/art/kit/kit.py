"""What the kit builds: the builds (proportions), the heads of the races, the pieces that fit
them, and the figures of the library's cast, each with the model definition the library gets
(`defs/models.ron`)."""
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
BLACK, DARK, BROWN, BLOND, GINGER, WHITE = "#1e1612", "#3a2418", "#5d3a1a", "#e0c060", "#c0562a", "#dedede"
BLUE, RED, GREEN, EARTH, PURPLE, YELLOW, GREY, PALE, TEAL, ORANGE = CLOTHS[0], CLOTHS[1], CLOTHS[2], CLOTHS[3], CLOTHS[4], CLOTHS[5], "#5a6068", CLOTHS[7], CLOTHS[8], CLOTHS[9]


def head(name, build, female=False, race=None, skins=SKINS, skin=None, outfit="clothes", hair=None, beard=None, playable=True):
    return dict(name=name, build=build, female=female, race=race or {}, skins=skins, skin=skin, outfit=outfit, hair=hair, beard=beard, headwear=None, cloak=False, playable=playable, colors={})


# The heads the kit builds: a race's own head and neck on a build. Each is a figure of its own
# (the playable ones are the races' bodies) and the file other figures of that kind reuse.
HEADS = {
    "hero": head("Human", "medium", hair="short"),
    "heroine": head("Human (female)", "medium", True, hair="long"),
    "elf": head("Elf", "medium", race=dict(ears=True), skin="#f2c49b", hair="long"),
    "elf_f": head("Elf (female)", "medium", True, dict(ears=True), skin="#f2c49b", hair="ponytail"),
    "tiefling": head("Tiefling", "medium", race=dict(horns=True, tail=True), skins=REDS, skin="#b0483e", hair="short"),
    "tiefling_f": head("Tiefling (female)", "medium", True, dict(horns=True, tail=True), REDS, "#b0483e", hair="long"),
    "dwarf": head("Dwarf", "dwarf", hair="short", beard="long"),
    "dwarf_f": head("Dwarf (female)", "dwarf", True, hair="bun"),
    "halfling": head("Halfling", "small", hair="short"),
    "halfling_f": head("Halfling (female)", "small", True, hair="ponytail"),
    "orc": head("Half-orc", "large", race=dict(tusks=True), skins=GREENS, skin="#7d9a62", hair="mohawk"),
    "orc_f": head("Half-orc (female)", "large", True, dict(tusks=True), GREENS, "#7d9a62", hair="ponytail"),
    "dragonborn": head("Dragonborn", "large", race=dict(dragon=True, tail=True), skins=[], skin="#b8433a"),
    "dragonborn_f": head("Dragonborn (female)", "large", True, dict(dragon=True, tail=True), [], "#b8433a"),
    "goblin": head("Goblin", "small", race=dict(goblin=True), skins=GREENS, skin="#6f9a4a", outfit="rags"),
    "goblin_scout": head("Goblin scout", "small", True, dict(goblin=True), GREENS, "#6f9a4a", outfit="rags"),
    "troll": head("Troll", "large", race=dict(troll=True), skins=[], skin="#5f7a80", outfit="loincloth"),
    "skeleton": head("Skeleton", "medium", race=dict(skull=True), skins=[], skin="#e8e2cf", outfit="bones", playable=False),
    "skeleton_f": head("Skeleton f", "medium", True, dict(skull=True), [], "#e8e2cf", outfit="bones", playable=False),
}


def cast(name, base, outfit="clothes", hair=None, beard=None, headwear=None, cloak=False, **colors):
    return dict(name=name, base=base, outfit=outfit, hair=hair, beard=beard, headwear=headwear, cloak=cloak, colors=colors)


# The library's people: a head file, what they wear and their colours (hair, clothing, skin).
CAST = {
    "villager": cast("Villager", "hero", hair="short", clothing=GREEN),
    "villager_f": cast("Villager f", "heroine", "dress", "bun", hair_=BROWN, clothing=RED),
    "elder": cast("Elder", "hero", "robe", "short", "long", hair_=WHITE, clothing=PURPLE),
    "elder_f": cast("Elder f", "heroine", "dress", "bun", hair_=WHITE, clothing=PURPLE),
    "guard": cast("Guard", "hero", "tabard", "short", headwear="helmet", clothing=BLUE),
    "guard_f": cast("Guard f", "heroine", "tabard", "ponytail", headwear="helmet", clothing=BLUE),
    "outlaw": cast("Outlaw", "hero", "leather_armor", "short", "short", "hood", True, hair_=BLACK, clothing="#4a5a44"),
    "bandit": cast("Bandit", "hero", "leather_armor", "mohawk", "short", hair_=DARK, clothing=EARTH),
    "bandit_f": cast("Bandit f", "heroine", "leather_armor", "ponytail", hair_=BLACK, clothing=EARTH),
    "bandit_chief": cast("Bandit chief", "hero", "leather_armor", None, "short", cloak=True, hair_=BLACK, clothing=RED),
    "orc_chief": cast("Orc chief", "orc", "plate_armor", "mohawk", cloak=True, hair_=BLACK),
    "merchant": cast("Merchant", "hero", "doublet", "short", "short", clothing=PURPLE),
    "merchant_f": cast("Merchant f", "heroine", "dress", "long", hair_=BLACK, clothing=YELLOW),
    "herbalist": cast("Herbalist", "heroine", "apron", "ponytail", hair_=BLOND, clothing=GREEN),
    "baker": cast("Baker", "hero", "apron", "short", "short", "chef_hat", clothing=PALE),
    "innkeeper": cast("Innkeeper", "heroine", "apron", "bun", hair_=GINGER, clothing=ORANGE),
    "priest": cast("Priest", "hero", "robe", clothing=PALE),
    "miller": cast("Miller", "hero", "apron", "short", "short", hair_="#8a5a2e", clothing="#8a7a5c"),
    "fisher": cast("Fisher", "hero", "clothes", "short", headwear="straw_hat", hair_=BLACK, clothing=YELLOW),
    "town_crier": cast("Town crier", "hero", "doublet", "short", hair_=BLACK, clothing=RED),
    "blacksmith": cast("Blacksmith", "hero", "smith_apron", None, "long", hair_=BLACK, clothing=GREY),
    "blacksmith_f": cast("Blacksmith f", "heroine", "smith_apron", "ponytail", hair_=DARK, clothing=GREY),
    "farmer": cast("Farmer", "hero", "clothes", "short", headwear="straw_hat", hair_=BLOND, clothing=EARTH),
    "farmer_f": cast("Farmer f", "heroine", "dress", "ponytail", headwear="straw_hat", hair_=BLOND, clothing=YELLOW),
    "child_boy": cast("Child boy", "halfling", hair="short", clothing=BLUE),
    "child_girl": cast("Child girl", "halfling_f", "dress", "ponytail", hair_=BLOND, clothing=RED),
    "dwarf_merchant": cast("Dwarf merchant", "dwarf", "doublet", "short", "long", hair_=WHITE, clothing=YELLOW),
    "elf_herbalist": cast("Elf herbalist", "elf_f", "robe", "long", hair_="#e6dcc0", clothing=GREEN),
    "wizard": cast("Wizard", "hero", "robe", "short", "long", "wizard_hat", hair_=WHITE, clothing=BLUE),
    "wizard_f": cast("Wizard f", "heroine", "robe", "long", headwear="wizard_hat", hair_=DARK, clothing=PURPLE),
    "nomad": cast("Nomad", "hero", "robe", "short", "short", "hood", hair_=BLACK, clothing=ORANGE),
    "nomad_f": cast("Nomad f", "heroine", "robe", "long", headwear="hood", hair_=BLACK, clothing=TEAL),
    "noble_m": cast("Noble m", "hero", "doublet", "short", cloak=True, clothing=TEAL),
    "noble": cast("Noble", "heroine", "dress", "bun", hair_=BLOND, clothing=PURPLE),
}
# Races whose figures are the kit's bodies under other ids than the race's own: (men, women).
# Their bodies have the race's size already, so the race's own size factor goes back to 1.
RACE_BODIES = {"halfling": ("halfling", "halfling_f"), "gnome": ("halfling", "halfling_f"), "tiefling": ("tiefling", "tiefling_f")}
# Clothing whose look isn't told by its slot and name: item id -> the style it shows.
ITEM_WEAR = dict(guard_uniform="tabard", outfit_guard="tabard", baker_whites="apron", miller_smock="apron", smith_apron="smith_apron", outfit_blacksmith_f="smith_apron",
                 villager_dress="dress", farm_dress="dress", innkeeper_dress="dress", merchant_dress="dress", outfit_merchant_f="dress", outfit_elder_f="dress", outfit_noble="dress",
                 goblin_rags="rags", outfit_goblin_f="rags", elder_garb="robe", outfit_wizard="robe", outfit_wizard_f="robe", outfit_nomad="robe", outfit_nomad_f="robe",
                 crier_coat="doublet", outfit_merchant="doublet", outfit_noble_m="doublet", outfit_bandit="leather_armor", outfit_bandit_f="leather_armor")
# Models and characters of the library the kit replaced without a figure of the same id.
RETIRED = ["hero_realistic", "elf_realistic", "elf_f_realistic"]
# The animations of every figure: (animation, clip of rig/human.glb, frames per second).
CLIPS = [("walk", "Walking", 12), ("idle", "Idle_3", 8), ("hurt", "Hit_Reaction", 12), ("sit", "Chair_Sit_Idle_M", 8), ("attack", "Right_Hand_Sword_Slash", 12), ("sneak", "Sneaky_Walk", 12),
         ("craft", "Heavy_Hammer_Swing", 12), ("chop", "Axe_Breathe_and_Look_Around", 12), ("attack_1h", "Right_Hand_Sword_Slash", 12), ("attack_2h", "Heavy_Hammer_Swing", 12),
         ("attack_thrust", "Thrust_Slash", 12), ("attack_bow", "Draw_and_Shoot_from_Back", 12), ("attack_cast", "Right_Jab_from_Guard", 12), ("attack_unarmed", "Right_Jab_from_Guard", 12),
         ("idle_1h", "Axe_Breathe_and_Look_Around", 8), ("idle_2h", "Axe_Breathe_and_Look_Around", 8), ("block_shield", "Sword_Parry_Backward_1", 12), ("block_weapon", "Sword_Parry", 12),
         ("block_2h", "Sword_Parry", 12)]
CLIP_FILE = "models/kit_clips.glb"
OUTFIT_NAMES = dict(clothes="Clothes", leather_armor="Leather armour", chain_mail="Chain mail", plate_armor="Plate armour", robe="Robe", apron="Apron", smith_apron="Smith's apron", dress="Dress",
                    tabard="Tabard", doublet="Doublet", rags="Rags", loincloth="Loincloth", bones="Bones")
# Outfits not built for every build and sex: style -> (builds or None = all, sexes or None = both).
OUTFIT_ONLY = dict(dress=(None, (True,)), loincloth=(("medium", "large"), None), bones=(("medium",), None))
HEADWEAR_NAMES = dict(helmet="Helmet", hood="Hood", straw_hat="Straw hat", chef_hat="Baker's hat", wizard_hat="Wizard's hat")


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
    """A colour feature of the shades the model really has. A figure also names features its
    pieces have (no shade of its own) to give them its default colour. None = nothing to say."""
    shades = [s for s in shades if s in used]
    if not shades and not default:
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


def figures():
    """Every figure: the heads, then the cast (each with its head's build, sex and race)."""
    out = {}
    for fid, h in HEADS.items():
        out[fid] = dict(h, base=fid)
    for fid, c in CAST.items():
        h = HEADS[c["base"]]
        colors = {k.rstrip("_"): v for k, v in c["colors"].items()}
        out[fid] = dict(h, name=c["name"], base=c["base"], outfit=c["outfit"], hair=c["hair"], beard=c["beard"], headwear=c["headwear"], cloak=c["cloak"], playable=False, colors=colors)
    return out


def everything():
    """Every model of the kit: (id, a function building its mesh or None for a figure that reuses
    a head's file, its definition without the measures the build adds)."""
    figs = {}

    def fig(build, female):
        key = (build, female)
        if key not in figs:
            figs[key] = F.Figure(build_of(build, female))
        return figs[key]
    out = []
    people = figures()
    heads = {}                       # head size -> the figures that can wear its hair and hats
    for fid, f in people.items():
        build, female = f["build"], f["female"]
        t = tag(build, female)
        n = head_size(build)
        parts = [f"kit_{f['outfit']}_{t}"]
        if not f["race"].get("dragon"):
            heads.setdefault(n, []).append(fid)
            parts += [f"kit_hair_{f['hair']}_{n}"] if f["hair"] else []
            parts += [f"kit_beard_{f['beard']}_{n}"] if f["beard"] else []
            parts += [f"kit_{f['headwear']}_{n}"] if f["headwear"] else []
        parts += [f"kit_cloak_{t}"] if f["cloak"] else []
        colors = f["colors"]
        d = dict(id=fid, name=f["name"], _file=f["base"], shared_clips=[CLIP_FILE], clips=[dict(anim=a, clip=c, fps=float(r)) for a, c, r in CLIPS], parts=parts,
                 _features=[("skin", [F.SKIN, F.MOUTH, F.BELLY], f["skins"] if f["base"] == fid else None, colors.get("skin", f["skin"])), ("hair", [], None, colors.get("hair")),
                            ("clothing", [], None, colors.get("clothing"))],
                 _stand=(build, female), _head=(build, female, f["race"]))
        if f["playable"]:
            d["playable"] = True
        if female:
            d["female"] = True
        out.append((fid, (lambda b=build, s=female, r=f["race"]: fig(b, s).body(**r)) if f["base"] == fid else None, d))
    for build in BUILDS:
        for female in (False, True):
            t = tag(build, female)
            fits = [fid for fid, f in people.items() if f["build"] == build and f["female"] == female]
            for style, label in OUTFIT_NAMES.items():
                builds, sexes = OUTFIT_ONLY.get(style, (None, None))
                if (builds and build not in builds) or (sexes and female not in sexes):
                    continue
                d = dict(id=f"kit_{style}_{t}", name=f"{label} ({t.replace('_', ' ')})", part=dict(slot="outfit", style=style, fits=fits),
                         _features=[("skin", [F.SKIN], None, None), ("clothing", [F.CLOTH, F.CLOTH_DARK], CLOTHS, None)])
                make = (lambda b=build, s=female: fig(b, s).bones()) if style == "bones" else (lambda b=build, s=female, st=style: fig(b, s).outfit(st))
                out.append((d["id"], make, d))
            d = dict(id=f"kit_cloak_{t}", name=f"Cloak ({t.replace('_', ' ')})", part=dict(slot="cloak", style="cloak", fits=fits), _features=[])
            out.append((d["id"], (lambda b=build, s=female: fig(b, s).cloak()), d))
    for n, fits in sorted(heads.items()):
        build = "small" if n == head_size("small") else "medium"
        for style in F.HAIR_STYLES:
            d = dict(id=f"kit_hair_{style}_{n}", name=f"{style.capitalize()} hair ({n})", part=dict(slot="hair", style=f"hair_{style}", fits=fits, offer=True), _features=[("hair", [F.HAIR], HAIRS, None)])
            out.append((d["id"], (lambda b=build, s=style: fig(b, False).hair(s)), d))
        for kind in F.BEARDS:
            d = dict(id=f"kit_beard_{kind}_{n}", name=f"{kind.capitalize()} beard ({n})", part=dict(slot="beard", style=f"beard_{kind}", fits=fits, offer=True), _features=[("hair", [F.HAIR], HAIRS, None)])
            out.append((d["id"], (lambda b=build, k=kind: fig(b, False).beard(k)), d))
        for kind in F.HEADWEAR:
            d = dict(id=f"kit_{kind}_{n}", name=f"{HEADWEAR_NAMES[kind]} ({n})", part=dict(slot="headwear", style=kind, fits=fits, hides=["hair"]),
                     _features=[("hair", [F.HAIR], None, None), ("clothing", [F.CLOTH, F.CLOTH_DARK], CLOTHS, None)])
            out.append((d["id"], (lambda b=build, k=kind: fig(b, False).headwear(k)), d))
    return out, fig
