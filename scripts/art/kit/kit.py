"""What the kit builds: the builds (proportions), the heads of the races, the pieces that fit
them, and the figures of the library's cast, each with the model definition the library gets
(`defs/models.ron`)."""
import creatures as C
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
    "giant": head("Giant", "large", hair="short", outfit="loincloth", playable=False),
    "fiend": head("Fiend", "large", race=dict(horns=True, tail=True), skins=[], skin="#8f2f2a", outfit="loincloth", playable=False),
    "imp": head("Imp", "small", race=dict(horns=True, tail=True), skins=[], skin="#a83a32", outfit="rags", playable=False),
    "kobold": head("Kobold", "small", race=dict(dragon=True, tail=True), skins=[], skin="#a8573a", outfit="rags", playable=False),
}


def cast(name, base, outfit="clothes", hair=None, beard=None, headwear=None, cloak=False, scale=1.0, **colors):
    """`cloak`: True, or the wings worn in its place (`wings_feather`, `wings_bat`); `scale`: how
    much taller than the head's own figure (giants)."""
    return dict(name=name, base=base, outfit=outfit, hair=hair, beard=beard, headwear=headwear, cloak=cloak, scale=scale, colors=colors)


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
STONE, PALE_SKIN, FUR = "#8a8c90", "#e6e0dc", "#6a5a4a"
# People-shaped monsters: figures of the cast, and the library's characters that get them
# (MONSTER_CAST: model id -> character ids).
CAST.update({
    "assassin": cast("Assassin", "hero", "leather_armor", "short", headwear="hood", cloak=True, hair_=BLACK, clothing="#2a2a30"),
    "berserker": cast("Berserker", "hero", "loincloth", "mohawk", "long", hair_=GINGER),
    "druid": cast("Druid", "hero", "robe", "long", "long", hair_=BROWN, clothing="#4a6a3a"),
    "gladiator": cast("Gladiator", "hero", "leather_armor", "short", headwear="helmet", clothing=RED),
    "scout": cast("Scout", "heroine", "leather_armor", "ponytail", cloak=True, hair_=BROWN, clothing=GREEN),
    "tribal_warrior": cast("Tribal warrior", "hero", "loincloth", "long", hair_=BLACK, skin="#b5794c"),
    "lycanthrope": cast("Lycanthrope", "hero", "rags", "long", "short", hair_=DARK),
    "vampire": cast("Vampire", "hero", "doublet", "short", cloak=True, hair_=BLACK, skin=PALE_SKIN, clothing="#5a1a22"),
    "vampire_spawn": cast("Vampire spawn", "heroine", "rags", "long", hair_=BLACK, skin=PALE_SKIN),
    "wight": cast("Wight", "hero", "chain_mail", "long", hair_=WHITE, skin="#9aa8a4"),
    "mummy": cast("Mummy", "hero", "rags", skin="#cfc4a0", clothing="#b8ad8a"),
    "mummy_lord": cast("Mummy lord", "hero", "robe", headwear="hood", skin="#cfc4a0", clothing="#c9a03c", scale=1.05),
    "lich": cast("Lich", "skeleton", "robe", headwear="hood", clothing="#3a2a4a"),
    "doppelganger": cast("Doppelganger", "hero", skin="#a8b0b4", clothing=GREY),
    "medusa": cast("Medusa", "heroine", "dress", "long", hair_="#3f8f4a", skin="#9ab08a", clothing="#6a6a4a"),
    "green_hag": cast("Green hag", "heroine", "rags", "long", hair_="#2a3a22", skin="#6a8a52"),
    "night_hag": cast("Night hag", "heroine", "robe", "long", hair_=WHITE, skin="#4a4a7a", clothing="#2a2a30"),
    "sea_hag": cast("Sea hag", "heroine", "rags", "long", hair_="#2e5a4a", skin="#6a9a9a"),
    "dryad": cast("Dryad", "elf_f", "dress", "long", hair_="#2e8b57", skin="#b89a6a", clothing="#4a7a3a"),
    "harpy": cast("Harpy", "heroine", "rags", "long", cloak="wings_feather", hair_=DARK, wings="#8a6a4a"),
    "deva": cast("Deva", "hero", "robe", "long", cloak="wings_feather", hair_=BLOND, clothing=PALE, scale=1.1),
    "planetar": cast("Planetar", "hero", "plate_armor", cloak="wings_feather", skin="#8fb09a", scale=1.5),
    "solar": cast("Solar", "hero", "plate_armor", "long", cloak="wings_feather", hair_=BLOND, skin="#e8c070", scale=1.8),
    "erinyes": cast("Erinyes", "tiefling_f", "plate_armor", "long", cloak="wings_feather", hair_=BLACK, wings="#3a3038"),
    "succubus": cast("Succubus", "tiefling_f", "dress", "long", cloak="wings_bat", hair_=BLACK, clothing="#5a1a22", wings="#7a2e2e"),
    "satyr": cast("Satyr", "tiefling", "loincloth", "short", "short", hair_=BROWN, skin="#c49a72"),
    "rakshasa": cast("Rakshasa", "orc", "doublet", skin="#d98a3a", clothing=PURPLE),
    "oni": cast("Oni", "fiend", "robe", "long", hair_=WHITE, skin="#4a6aa0", clothing="#3a2a4a", scale=1.4),
    "minotaur": cast("Minotaur", "fiend", "loincloth", skin="#6b4630", scale=1.4),
    "gargoyle": cast("Gargoyle", "fiend", "loincloth", cloak="wings_bat", skin=STONE, wings=STONE),
    "quasit": cast("Quasit", "imp", "rags", skin="#5a8a4a", scale=.6),
    "imp_winged": cast("Imp (winged)", "imp", "rags", cloak="wings_bat", wings="#7a2e2e", scale=.6),
    "homunculus": cast("Homunculus", "imp", "rags", cloak="wings_bat", skin="#9a8a7a", wings="#6a5a4a", scale=.6),
    "sprite": cast("Sprite", "halfling_f", "dress", "short", cloak="wings_feather", hair_=BLOND, clothing=GREEN, wings="#cfe0e8", scale=.5),
    "dretch": cast("Dretch", "goblin", "rags", skin="#8a8f6a"),
    "bearded_devil": cast("Bearded devil", "tiefling", "loincloth", None, "long", hair_="#5a1a22", skin="#a83a32"),
    "barbed_devil": cast("Barbed devil", "fiend", "loincloth", skin="#8a5a2a", scale=1.1),
    "bone_devil": cast("Bone devil", "skeleton", "bones", cloak="wings_bat", wings="#b8ad8a", scale=1.4),
    "chain_devil": cast("Chain devil", "tiefling", "chain_mail", skin="#8a8590"),
    "horned_devil": cast("Horned devil", "fiend", "loincloth", cloak="wings_bat", wings="#5a2020", scale=1.5),
    "ice_devil": cast("Ice devil", "fiend", "loincloth", skin="#cfe0e8", scale=1.5),
    "pit_fiend": cast("Pit fiend", "fiend", "loincloth", cloak="wings_bat", skin="#a02a22", wings="#5a2020", scale=2.0),
    "balor": cast("Balor", "fiend", "loincloth", cloak="wings_bat", skin="#5a1a14", wings="#2a1a1a", scale=2.2),
    "glabrezu": cast("Glabrezu", "fiend", "loincloth", skin="#7a3a4a", scale=1.9),
    "hezrou": cast("Hezrou", "troll", "loincloth", skin="#4a6a3a", scale=1.5),
    "nalfeshnee": cast("Nalfeshnee", "troll", "loincloth", cloak="wings_bat", skin=FUR, wings="#4a3a30", scale=1.8),
    "marilith": cast("Marilith", "tiefling_f", "plate_armor", "long", hair_=BLACK, skin="#6a8a5a", scale=1.5),
    "vrock": cast("Vrock", "fiend", "loincloth", cloak="wings_feather", skin="#6a5a50", wings="#4a4038", scale=1.5),
    "hill_giant": cast("Hill giant", "giant", "loincloth", "short", hair_=BROWN, skin="#c49a72", scale=2.0),
    "stone_giant": cast("Stone giant", "giant", "loincloth", skin="#9a9c9e", scale=2.3),
    "frost_giant": cast("Frost giant", "giant", "chain_mail", "long", "long", hair_=WHITE, skin="#a8c4e0", scale=2.6),
    "fire_giant": cast("Fire giant", "giant", "plate_armor", "mohawk", "long", hair_="#d35400", skin="#3a3434", scale=2.8),
    "cloud_giant": cast("Cloud giant", "giant", "robe", "long", hair_=WHITE, skin="#c8d8e8", clothing=PALE, scale=3.0),
    "storm_giant": cast("Storm giant", "giant", "chain_mail", "long", "long", hair_=WHITE, skin="#8a7ab0", scale=3.3),
    "ettin": cast("Ettin", "orc", "loincloth", "mohawk", hair_=BLACK, skin="#a08a6a", scale=1.9),
    "ogre_zombie": cast("Ogre zombie", "troll", "loincloth", skin="#7a8a6a", scale=1.4),
    "flesh_golem": cast("Flesh golem", "giant", "rags", skin="#8a9a7a", scale=1.35),
    "clay_golem": cast("Clay golem", "giant", "loincloth", skin="#a8704a", scale=1.45),
    "stone_golem": cast("Stone golem", "giant", "loincloth", skin=STONE, scale=1.6),
    "iron_golem": cast("Iron golem", "giant", "plate_armor", headwear="helmet", skin="#6c7686", scale=1.7),
    "shield_guardian": cast("Shield guardian", "giant", "plate_armor", headwear="helmet", skin="#8a7a5a", scale=1.4),
    "animated_armor": cast("Animated armor", "hero", "plate_armor", headwear="helmet", skin="#6c7686"),
    "bugbear": cast("Bugbear", "orc", "leather_armor", "long", hair_=BROWN, skin="#8a5a2e", scale=1.1),
    "gnoll": cast("Gnoll", "orc", "leather_armor", "mohawk", hair_=DARK, skin="#a89a6a"),
    "grimlock": cast("Grimlock", "orc", "loincloth", skin=STONE),
    "lizardfolk": cast("Lizardfolk", "dragonborn", "loincloth", skin="#4a8a4a"),
    "sahuagin": cast("Sahuagin", "dragonborn", "loincloth", skin="#3a7a8a"),
    "merrow": cast("Merrow", "dragonborn", "loincloth", skin="#4a8a7a", scale=1.3),
    "merfolk": cast("Merfolk", "hero", "loincloth", "long", hair_="#2e8b57", skin="#7ab0b0"),
    "dragon_veteran": cast("Half-dragon veteran", "dragonborn", "plate_armor"),
    "kobold_warrior": cast("Kobold warrior", "kobold", "leather_armor"),
    "drow": cast("Drow", "elf", "chain_mail", "long", hair_=WHITE, skin="#4a4458"),
    "duergar": cast("Duergar", "dwarf", "chain_mail", None, "long", hair_=WHITE, skin=STONE),
    "deep_gnome": cast("Deep gnome", "halfling", "leather_armor", skin=STONE),
    "azer": cast("Azer", "dwarf", "smith_apron", "mohawk", "long", hair_="#d35400", skin="#b87a3a"),
})
_MONSTER_CAST = {
    "assassin": "assassin", "berserker": "berserker", "druid": "druid", "gladiator": "gladiator", "scout": "scout", "tribal_warrior": "tribal-warrior",
    "lycanthrope": "werewolf-human wererat-human werebear-human wereboar-human weretiger-human",
    "vampire": "vampire-vampire", "vampire_spawn": "vampire-spawn", "wight": "wight", "mummy": "mummy", "mummy_lord": "mummy-lord",
    "lich": "lich", "doppelganger": "doppelganger", "medusa": "medusa", "green_hag": "green-hag", "night_hag": "night-hag", "sea_hag": "sea-hag", "dryad": "dryad", "harpy": "harpy", "deva": "deva",
    "planetar": "planetar", "solar": "solar", "erinyes": "erinyes", "succubus": "succubus-incubus", "satyr": "satyr", "rakshasa": "rakshasa", "oni": "oni", "minotaur": "minotaur",
    "gargoyle": "gargoyle", "quasit": "quasit", "imp_winged": "imp", "homunculus": "homunculus", "sprite": "sprite", "dretch": "dretch", "bearded_devil": "bearded-devil",
    "barbed_devil": "barbed-devil", "bone_devil": "bone-devil", "chain_devil": "chain-devil", "horned_devil": "horned-devil", "ice_devil": "ice-devil", "pit_fiend": "pit-fiend", "balor": "balor",
    "glabrezu": "glabrezu", "hezrou": "hezrou", "nalfeshnee": "nalfeshnee", "marilith": "marilith", "vrock": "vrock", "hill_giant": "hill-giant", "stone_giant": "stone-giant",
    "frost_giant": "frost-giant", "fire_giant": "fire-giant", "cloud_giant": "cloud-giant", "storm_giant": "storm-giant", "ettin": "ettin", "ogre_zombie": "ogre-zombie", "flesh_golem": "flesh-golem",
    "clay_golem": "clay-golem", "stone_golem": "stone-golem", "iron_golem": "iron-golem", "shield_guardian": "shield-guardian", "animated_armor": "animated-armor", "bugbear": "bugbear",
    "gnoll": "gnoll", "grimlock": "grimlock", "lizardfolk": "lizardfolk", "sahuagin": "sahuagin", "merrow": "merrow", "merfolk": "merfolk", "dragon_veteran": "half-red-dragon-veteran",
    "kobold_warrior": "kobold", "drow": "drow", "duergar": "duergar", "deep_gnome": "deep-gnome-svirfneblin", "azer": "azer",
}
MONSTER_CAST = {cid: model for model, ids in _MONSTER_CAST.items() for cid in ids.split()}
# Races whose figures are the kit's bodies under other ids than the race's own: (men, women).
# Their bodies have the race's size already, so the race's own size factor goes back to 1.
RACE_BODIES = {"halfling": ("halfling", "halfling_f"), "gnome": ("halfling", "halfling_f"), "tiefling": ("tiefling", "tiefling_f")}
# Clothing whose look isn't told by its slot and name: item id -> the style it shows.
ITEM_WEAR = dict(guard_uniform="tabard", outfit_guard="tabard", baker_whites="apron", miller_smock="apron", smith_apron="smith_apron", outfit_blacksmith_f="smith_apron",
                 villager_dress="dress", farm_dress="dress", innkeeper_dress="dress", merchant_dress="dress", outfit_merchant_f="dress", outfit_elder_f="dress", outfit_noble="dress",
                 goblin_rags="rags", outfit_goblin_f="rags", elder_garb="robe", outfit_wizard="robe", outfit_wizard_f="robe", outfit_nomad="robe", outfit_nomad_f="robe",
                 crier_coat="doublet", outfit_merchant="doublet", outfit_noble_m="doublet", outfit_bandit="leather_armor", outfit_bandit_f="leather_armor")
# Models and characters of the library the kit replaced without a figure of the same id; the
# lycanthropes' hybrid forms and their figures went in 0.35 (two forms: human and animal).
RETIRED = ["hero_realistic", "elf_realistic", "elf_f_realistic", "werewolf-hybrid", "wererat-hybrid", "werebear-hybrid", "wereboar-hybrid", "weretiger-hybrid",
           "werewolf", "wererat", "werebear", "wereboar", "weretiger"]
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


DRAGONS = dict(black="#3a3a44", blue="#3b62c4", brass="#b89a4e", bronze="#a8703c", copper="#c0683c", gold="#d9a824", green="#3f8f4a", red="#b8433a", silver="#a9b4c0", white="#e4e6ea")


def creature(name, file, size=1.0, **colors):
    return dict(name=name, file=file, size=size, colors=colors)


# The creatures' models: a file of creatures.py, how much bigger than the file's own size, and the
# colour of its feature (fur, skin, body, feathers) when it isn't the file's own.
CREATURE_MODELS = {
    "wolf": creature("Wolf", "wolf"), "dire_wolf": creature("Dire wolf", "wolf", 1.35, fur="#6a6660"), "winter_wolf": creature("Winter wolf", "wolf", 1.5, fur="#dfe4ea"),
    "hell_hound": creature("Hell hound", "wolf", 1.15, fur="#8a3226"), "dog": creature("Dog", "dog"), "hyena": creature("Hyena", "dog", 1.2, fur="#a89a6a"), "cat": creature("Cat", "cat"),
    "rat": creature("Rat", "rat"), "giant_rat": creature("Giant rat", "rat", 2.2), "bear": creature("Bear", "bear"), "black_bear": creature("Black bear", "bear", .9, fur="#2e2a2c"),
    "polar_bear": creature("Polar bear", "bear", 1.15, fur="#e8e6e0"), "boar": creature("Boar", "boar"), "giant_boar": creature("Giant boar", "boar", 1.6), "sheep": creature("Sheep", "sheep"),
    "cow": creature("Cow", "cow"), "horse": creature("Horse", "horse"), "pony": creature("Pony", "horse", .72, fur="#a8763e"), "warhorse": creature("Warhorse", "horse", 1.08, fur="#4a3a30"),
    "nightmare": creature("Nightmare", "horse", 1.08, fur="#221c22"), "unicorn": creature("Unicorn", "horse", 1.0, fur="#eeeae2"), "panther": creature("Panther", "big_cat", 1.0, fur="#2a262c"),
    "lion": creature("Lion", "lion"), "tiger": creature("Tiger", "tiger", 1.0, fur="#d98a3a"), "saber_tooth": creature("Saber-toothed tiger", "tiger", 1.25, fur="#c79a56"),
    "spider": creature("Spider", "spider", .5), "wolf_spider": creature("Giant wolf spider", "spider", 1.0, body="#5a4a3a"), "giant_spider": creature("Giant spider", "spider", 1.8),
    "phase_spider": creature("Phase spider", "spider", 1.8, body="#4a6a8a"), "chicken": creature("Chicken", "chicken"), "bat": creature("Bat", "bat"), "giant_bat": creature("Giant bat", "bat", 2.2),
    "raven": creature("Raven", "bird", .7, feathers="#2a2630"), "hawk": creature("Hawk", "bird", .8), "eagle": creature("Eagle", "bird", 1.1), "owl": creature("Owl", "bird", .9, feathers="#8a7a66"),
    "giant_eagle": creature("Giant eagle", "bird", 2.4), "giant_owl": creature("Giant owl", "bird", 2.2, feathers="#8a7a66"), "vulture": creature("Vulture", "bird", 1.2, feathers="#3a3430"),
    "slime": creature("Slime", "slime"), "black_pudding": creature("Black pudding", "slime", 2.6, body="#2a2630"), "gray_ooze": creature("Gray ooze", "slime", 1.4, body="#8a8c90"),
    "ochre_jelly": creature("Ochre jelly", "slime", 2.6, body="#c9a03c"), "gelatinous_cube": creature("Gelatinous cube", "gel_cube", 3.4), "ghost": creature("Ghost", "ghost"),
    "specter": creature("Specter", "ghost", 1.0, body="#9fb2c8"), "wraith": creature("Wraith", "ghost", 1.05, body="#3a3644"), "shadow": creature("Shadow", "ghost", .95, body="#221e2a"),
    "wisp": creature("Will-o'-wisp", "wisp"), "fire_elemental": creature("Fire elemental", "fire_elemental", 1.6), "water_elemental": creature("Water elemental", "water_elemental", 1.6),
    "air_elemental": creature("Air elemental", "air_elemental", 1.6), "earth_elemental": creature("Earth elemental", "earth_elemental", 1.6), "magmin": creature("Magmin", "fire_elemental", .6),
    "mephit_fire": creature("Magma mephit", "fire_elemental", .55), "mephit_water": creature("Ice mephit", "water_elemental", .55), "mephit_air": creature("Dust mephit", "air_elemental", .55),
}
CREATURE_MODELS.update({
    "deer": creature("Deer", "deer"), "elk": creature("Elk", "deer", 1.25, fur="#8a6a4a"), "giant_elk": creature("Giant elk", "deer", 1.7, fur="#8a6a4a"), "goat": creature("Goat", "goat"),
    "giant_goat": creature("Giant goat", "goat", 1.5, fur="#8a8478"), "camel": creature("Camel", "horse", 1.0, fur="#c9a66a"), "elephant": creature("Elephant", "elephant", 1.9),
    "mammoth": creature("Mammoth", "elephant", 2.3, fur="#6b4a32"), "rhinoceros": creature("Rhinoceros", "rhino", 1.5), "triceratops": creature("Triceratops", "triceratops", 2.2, fur="#7a8a5a"),
    "crocodile": creature("Crocodile", "crocodile", 1.2), "giant_crocodile": creature("Giant crocodile", "crocodile", 2.4), "lizard": creature("Lizard", "crocodile", .35, fur="#7a8a4a"),
    "giant_lizard": creature("Giant lizard", "crocodile", 1.3, fur="#7a8a4a"), "basilisk": creature("Basilisk", "crocodile", 1.5, fur="#6a6a3a"), "bulette": creature("Bulette", "crocodile", 2.2, fur="#8a8f98"),
    "behir": creature("Behir", "crocodile", 3.0, fur="#3b62c4"), "frog": creature("Frog", "frog", .5), "giant_frog": creature("Giant frog", "frog", 1.8), "giant_toad": creature("Giant toad", "frog", 2.4, fur="#7a6a3a"),
    "ape": creature("Ape", "ape"), "baboon": creature("Baboon", "ape", .6, fur="#8a7a5a"), "giant_ape": creature("Giant ape", "ape", 2.3), "griffon": creature("Griffon", "griffon", 1.3),
    "hippogriff": creature("Hippogriff", "griffon", 1.25, fur="#8a6a4a"), "centaur": creature("Centaur", "centaur", 1.15), "pegasus": creature("Pegasus", "pegasus", 1.0, fur="#eeeae2"),
    "unicorn": creature("Unicorn", "unicorn", 1.0, fur="#eeeae2"), "manticore": creature("Manticore", "winged_lion", 1.4, fur="#a85a3a"), "chimera": creature("Chimera", "winged_lion", 1.5),
    "sphinx": creature("Sphinx", "winged_lion", 1.8, fur="#d9b45a"), "owlbear": creature("Owlbear", "owlbear", 1.3), "wyvern": creature("Wyvern", "dragon_young", 2.25, skin="#6a5a3a"),
    "pseudodragon": creature("Pseudodragon", "dragon_wyrmling", .4, skin="#a8573a"), "drake": creature("Drake", "drake"), "hydra": creature("Hydra", "drake", 2.4, skin="#4a6a4a"),
    "tarrasque": creature("Tarrasque", "drake", 4.0, skin="#6a5a48"), "plesiosaurus": creature("Plesiosaurus", "drake", 1.9, skin="#3a7a8a"), "dragon_turtle": creature("Dragon turtle", "drake", 3.2, skin="#3a7a6a"),
    "crab": creature("Crab", "crab", .5), "giant_crab": creature("Giant crab", "crab", 1.3), "chuul": creature("Chuul", "crab", 2.4, body="#8a3a3a"), "scorpion": creature("Scorpion", "scorpion", .4),
    "giant_scorpion": creature("Giant scorpion", "scorpion", 1.8), "ankheg": creature("Ankheg", "scorpion", 2.0, body="#7a6a3a"), "rust_monster": creature("Rust monster", "scorpion", 1.2, body="#a85a2a"),
    "ettercap": creature("Ettercap", "spider", 1.5, body="#5a3a6a"), "drider": creature("Drider", "drider", 1.9), "giant_wasp": creature("Giant wasp", "wasp", 1.3), "wasps": creature("Wasps", "wasp", .6),
    "snake": creature("Snake", "snake", .6), "constrictor_snake": creature("Constrictor snake", "snake", 1.3, skin="#8a7a4a"), "giant_snake": creature("Giant snake", "snake", 2.2, skin="#8a7a4a"),
    "cobra": creature("Cobra", "cobra", .6), "giant_cobra": creature("Giant cobra", "cobra", 1.4), "naga": creature("Naga", "cobra", 2.4, skin="#3f8f6a"), "couatl": creature("Couatl", "cobra", 1.6, skin="#d9a824"),
    "purple_worm": creature("Purple worm", "worm", 3.6), "grick": creature("Grick", "worm", .9, skin="#8a8c90"), "remorhaz": creature("Remorhaz", "worm", 2.8, skin="#9ac0d8"),
    "centipede": creature("Giant centipede", "worm", .6, skin="#a85a2a"), "shark": creature("Shark", "shark", 1.6), "giant_shark": creature("Giant shark", "shark", 3.0), "quipper": creature("Quipper", "shark", .6, skin="#6a9a8a"),
    "killer_whale": creature("Killer whale", "shark", 3.0, skin="#2a2a30"), "sea_horse": creature("Sea horse", "shark", .5, skin="#d9a24a"), "octopus": creature("Octopus", "octopus", .7),
    "giant_octopus": creature("Giant octopus", "octopus", 2.0), "kraken": creature("Kraken", "octopus", 6.0, skin="#7a3a4a"), "aboleth": creature("Aboleth", "octopus", 3.0, skin="#5a8a7a"),
    "otyugh": creature("Otyugh", "octopus", 2.6, skin="#7a6a4a"), "darkmantle": creature("Darkmantle", "octopus", .9, skin="#3a3644"), "roper": creature("Roper", "octopus", 2.6, skin="#8a8c90"),
    "tyrannosaurus": creature("Tyrannosaurus", "tyrant_lizard", 2.6), "axe_beak": creature("Axe beak", "axe_beak", 1.3), "cockatrice": creature("Cockatrice", "chicken", 1.0, feathers="#8a9a5a"),
    "treant": creature("Treant", "tree", 2.4), "awakened_tree": creature("Awakened tree", "tree", 2.0, leaves="#6a8a3a"), "awakened_shrub": creature("Awakened shrub", "shrub"),
    "shambling_mound": creature("Shambling mound", "shrub", 2.6, leaves="#4a5a2a"), "shrieker": creature("Shrieker", "mushroom", 1.0, cap="#c9b8d8"), "violet_fungus": creature("Violet fungus", "mushroom", 1.6),
    "mimic": creature("Mimic", "mimic", 1.2), "rug_of_smothering": creature("Rug of smothering", "rug", 2.2), "flying_sword": creature("Flying sword", "flying_sword", .8),
    "gibbering_mouther": creature("Gibbering mouther", "slime", 1.8, body="#b07a6a"), "lemure": creature("Lemure", "slime", 1.6, body="#a8705a"), "cloaker": creature("Cloaker", "bat", 3.4, fur="#3a3038"),
    "stirge": creature("Stirge", "wasp", .7, body="#a85a4a"), "gorgon": creature("Gorgon", "cow", 1.25, fur="#6c7686"), "salamander": creature("Salamander", "cobra", 2.4, skin="#d35400"),
})
for _c, _hex in DRAGONS.items():
    CREATURE_MODELS[f"{_c}_dragon_wyrmling"] = creature(f"{_c.capitalize()} dragon wyrmling", "dragon_wyrmling", 1.0, skin=_hex)
    CREATURE_MODELS[f"young_{_c}_dragon"] = creature(f"Young {_c} dragon", "dragon_young", 1.95, skin=_hex)
    CREATURE_MODELS[f"adult_{_c}_dragon"] = creature(f"Adult {_c} dragon", "dragon_young", 3.3, skin=_hex)
    CREATURE_MODELS[f"ancient_{_c}_dragon"] = creature(f"Ancient {_c} dragon", "dragon_young", 4.95, skin=_hex)
# The library's monsters and animals that get one of these models: model id -> character ids.
# (The figures of the cast further down are people-shaped monsters: MONSTER_CAST.)
_MONSTERS = {
    "winter_wolf": "winter-wolf", "dire_wolf": "worg dire-wolf werewolf-wolf", "hell_hound": "hell-hound death-dog", "dog": "blink-dog jackal mastiff", "hyena": "hyena giant-hyena",
    "rat": "rat swarm-of-rats weasel badger", "giant_rat": "giant-rat giant-rat-diseased wererat-rat giant-weasel giant-badger", "black_bear": "black-bear", "bear": "brown-bear werebear-bear",
    "polar_bear": "polar-bear", "boar": "boar", "giant_boar": "giant-boar wereboar-boar", "horse": "draft-horse riding-horse", "warhorse": "warhorse", "pony": "pony mule", "camel": "camel",
    "nightmare": "nightmare", "unicorn": "unicorn", "pegasus": "pegasus", "lion": "lion lamia", "panther": "panther", "tiger": "tiger weretiger-tiger", "saber_tooth": "saber-toothed-tiger",
    "spider": "spider swarm-of-spiders swarm-of-insects swarm-of-beetles", "giant_spider": "giant-spider", "wolf_spider": "giant-wolf-spider giant-fire-beetle", "phase_spider": "phase-spider",
    "bat": "swarm-of-bats", "giant_bat": "giant-bat vampire-bat", "stirge": "stirge", "eagle": "eagle", "giant_eagle": "giant-eagle giant-vulture roc", "hawk": "hawk blood-hawk", "owl": "owl",
    "giant_owl": "giant-owl", "raven": "raven swarm-of-ravens", "vulture": "vulture", "black_pudding": "black-pudding", "gray_ooze": "gray-ooze", "ochre_jelly": "ochre-jelly",
    "gelatinous_cube": "gelatinous-cube", "specter": "specter vampire-mist", "wraith": "wraith", "shadow": "shadow", "wisp": "will-o-wisp", "fire_elemental": "fire-elemental efreeti",
    "water_elemental": "water-elemental", "air_elemental": "air-elemental invisible-stalker djinni", "earth_elemental": "earth-elemental xorn", "magmin": "magmin", "mephit_fire": "magma-mephit",
    "mephit_water": "steam-mephit ice-mephit", "mephit_air": "dust-mephit", "deer": "deer", "elk": "elk", "giant_elk": "giant-elk", "goat": "goat", "giant_goat": "giant-goat",
    "elephant": "elephant", "mammoth": "mammoth", "rhinoceros": "rhinoceros", "triceratops": "triceratops", "crocodile": "crocodile", "giant_crocodile": "giant-crocodile", "lizard": "lizard",
    "giant_lizard": "giant-lizard", "basilisk": "basilisk", "bulette": "bulette", "behir": "behir", "frog": "frog", "giant_frog": "giant-frog", "giant_toad": "giant-toad", "ape": "ape",
    "baboon": "baboon", "giant_ape": "giant-ape", "griffon": "griffon", "hippogriff": "hippogriff", "centaur": "centaur", "manticore": "manticore", "chimera": "chimera",
    "sphinx": "androsphinx gynosphinx", "owlbear": "owlbear", "wyvern": "wyvern", "pseudodragon": "pseudodragon", "hydra": "hydra", "tarrasque": "tarrasque", "plesiosaurus": "plesiosaurus",
    "dragon_turtle": "dragon-turtle", "crab": "crab", "giant_crab": "giant-crab", "chuul": "chuul", "scorpion": "scorpion", "giant_scorpion": "giant-scorpion", "ankheg": "ankheg",
    "rust_monster": "rust-monster", "ettercap": "ettercap", "drider": "drider", "giant_wasp": "giant-wasp", "wasps": "swarm-of-wasps", "snake": "poisonous-snake flying-snake swarm-of-poisonous-snakes",
    "constrictor_snake": "constrictor-snake", "giant_snake": "giant-constrictor-snake", "giant_cobra": "giant-poisonous-snake", "naga": "guardian-naga spirit-naga", "couatl": "couatl",
    "purple_worm": "purple-worm", "grick": "grick", "remorhaz": "remorhaz", "centipede": "giant-centipede swarm-of-centipedes", "shark": "reef-shark hunter-shark", "giant_shark": "giant-shark",
    "quipper": "quipper swarm-of-quippers", "killer_whale": "killer-whale", "sea_horse": "sea-horse giant-sea-horse", "octopus": "octopus", "giant_octopus": "giant-octopus", "kraken": "kraken",
    "aboleth": "aboleth", "otyugh": "otyugh", "darkmantle": "darkmantle", "roper": "roper", "tyrannosaurus": "tyrannosaurus-rex", "axe_beak": "axe-beak", "cockatrice": "cockatrice",
    "treant": "treant", "awakened_tree": "awakened-tree", "awakened_shrub": "awakened-shrub", "shambling_mound": "shambling-mound", "shrieker": "shrieker", "violet_fungus": "violet-fungus",
    "mimic": "mimic", "rug_of_smothering": "rug-of-smothering", "flying_sword": "flying-sword", "gibbering_mouther": "gibbering-mouther", "lemure": "lemure", "cloaker": "cloaker",
    "gorgon": "gorgon", "salamander": "salamander",
}
MONSTERS = {cid: model for model, ids in _MONSTERS.items() for cid in ids.split()}
for _c in DRAGONS:
    MONSTERS[f"{_c}-dragon-wyrmling"] = f"{_c}_dragon_wyrmling"
    for _age in ("young", "adult", "ancient"):
        MONSTERS[f"{_age}-{_c}-dragon"] = f"{_age}_{_c}_dragon"


# The size class the library gives these monsters (percent of their model: the SRD's Tiny 50,
# Small 75, Large 150, Huge 200, Gargantuan 300). A model's height is its drawn height divided by
# the class of the first monster that uses it, so the two together give the size meant here.
_SIZES = {
    50: "badger crab flying-snake frog hawk homunculus imp lizard owl poisonous-snake pseudodragon quasit quipper rat raven scorpion sea-horse spider sprite stirge weasel will-o-wisp",
    75: "awakened-shrub baboon blood-hawk cockatrice darkmantle deep-gnome-svirfneblin dretch dust-mephit eagle flying-sword giant-centipede giant-fire-beetle giant-rat giant-rat-diseased "
    "ice-mephit jackal kobold magma-mephit magmin octopus steam-mephit",
    150: "aboleth air-elemental androsphinx ankheg axe-beak black-pudding bone-devil brown-bear bulette camel centaur chimera chuul clay-golem cloaker constrictor-snake crocodile dire-wolf "
    "djinni draft-horse drider earth-elemental efreeti elk ettin fire-elemental gelatinous-cube giant-bat giant-boar giant-eagle giant-goat giant-hyena giant-lizard giant-octopus giant-"
    "owl giant-scorpion giant-sea-horse giant-spider giant-toad giant-vulture glabrezu gorgon griffon guardian-naga gynosphinx hezrou hippogriff horned-devil hunter-shark ice-devil "
    "iron-golem lamia lion manticore marilith merrow minotaur nalfeshnee nightmare ochre-jelly ogre-zombie oni otyugh owlbear pegasus phase-spider pit-fiend planetar plesiosaurus polar-"
    "bear rhinoceros riding-horse roper rug-of-smothering saber-toothed-tiger salamander shambling-mound shield-guardian solar spirit-naga stone-golem tiger unicorn vrock warhorse "
    "water-elemental winter-wolf worg wyvern young-black-dragon young-blue-dragon young-brass-dragon young-bronze-dragon young-copper-dragon young-gold-dragon young-green-dragon young-"
    "red-dragon young-silver-dragon young-white-dragon",
    200: "adult-black-dragon adult-blue-dragon adult-brass-dragon adult-bronze-dragon adult-copper-dragon adult-gold-dragon adult-green-dragon adult-red-dragon adult-silver-dragon adult-"
    "white-dragon awakened-tree balor behir cloud-giant elephant fire-giant frost-giant giant-ape giant-constrictor-snake giant-crocodile giant-elk giant-shark hill-giant hydra killer-"
    "whale mammoth remorhaz stone-giant storm-giant treant triceratops tyrannosaurus-rex",
    300: "ancient-black-dragon ancient-blue-dragon ancient-brass-dragon ancient-bronze-dragon ancient-copper-dragon ancient-gold-dragon ancient-green-dragon ancient-red-dragon ancient-"
    "silver-dragon ancient-white-dragon dragon-turtle kraken purple-worm roc tarrasque",
}
SIZE_CLASS = {cid: pct for pct, ids in _SIZES.items() for cid in ids.split()}


def size_class(model):
    """The size class (1.0 = Medium) of the first monster that uses a model."""
    for table in (_MONSTERS, _MONSTER_CAST):
        if model in table:
            return SIZE_CLASS.get(table[model].split()[0], 100) / 100
    for cid, m in MONSTERS.items():
        if m == model:
            return SIZE_CLASS.get(cid, 100) / 100
    return 1.0


_SCULPTS = {}


def sculpt(file):
    """A creature file's mesh, rig fit, height in world pixels and colour feature (built once)."""
    if file not in _SCULPTS:
        _SCULPTS[file] = C.build(file)
    return _SCULPTS[file]


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
        out[fid] = dict(h, name=c["name"], base=c["base"], outfit=c["outfit"], hair=c["hair"], beard=c["beard"], headwear=c["headwear"], cloak=c["cloak"], scale=c["scale"], playable=False, colors=colors)
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
        parts += [f"kit_{'cloak' if f['cloak'] is True else f['cloak']}_{t}"] if f["cloak"] else []
        colors = f["colors"]
        d = dict(id=fid, name=f["name"], _file=f["base"], shared_clips=[CLIP_FILE], clips=[dict(anim=a, clip=c, fps=float(r)) for a, c, r in CLIPS], parts=parts,
                 _features=[("skin", [F.SKIN, F.MOUTH, F.BELLY], f["skins"] if f["base"] == fid else None, colors.get("skin", f["skin"])), ("hair", [], None, colors.get("hair")),
                            ("clothing", [], None, colors.get("clothing")), ("wings", [], None, colors.get("wings"))],
                 _stand=(build, female), _scale=f.get("scale", 1.0) / size_class(fid), _head=(build, female, f["race"]))
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
            for kind, label in (("feather", "Feathered wings"), ("bat", "Bat wings")):
                d = dict(id=f"kit_wings_{kind}_{t}", name=f"{label} ({t.replace('_', ' ')})", part=dict(slot="cloak", style=f"wings_{kind}", fits=fits), _features=[("wings", [F.WING, F.WING_DARK], None, None)])
                out.append((d["id"], (lambda b=build, s=female, k=kind: fig(b, s).wings(k)), d))
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
    # Creatures: a file per sculpt, and the models that use it at their size and in their colour.
    # (A file is built by the first model that uses it.)
    seen = set()
    for cid, c in CREATURE_MODELS.items():
        first = c["file"] not in seen
        seen.add(c["file"])
        d = dict(id=cid, name=c["name"], _file=c["file"], _creature=(c["file"], c["size"] / size_class(cid), c["colors"]))
        out.append((cid, (lambda f=c["file"]: sculpt(f)[0]) if first else None, d))
    return out, fig
