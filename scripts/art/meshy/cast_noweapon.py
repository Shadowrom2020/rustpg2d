"""The figures re-rendered without what they hold (0.17: held items are drawn by the game at the
figures' hand grips): name -> (size class, attack motion or None, description). Every figure has
its own size class: the standing height of its earlier sheet, so it stays as tall as before."""

CAST = {
    "hero": ("hero", "slash", "a young man with messy brown hair, a blue tunic with a brown belt, dark grey trousers, brown leather boots."),
    "heroine": ("heroine", "slash", "a young woman with long straight brown hair, a light blue tunic with a brown belt, grey arm guards, dark trousers, brown boots."),
    "guard": ("guard", "thrust", "a town guard man, a round iron helmet, a dark blue padded uniform with a brown belt, dark trousers, brown boots."),
    "guard_f": ("guard_f", "thrust", "a town guard woman, a round iron helmet, auburn hair, a dark blue padded uniform with a brown belt, dark trousers, brown boots."),
    "goblin": ("goblin", "bow", "a small goblin man, green skin, long pointed ears, a big nose, ragged brown leather clothes, bare feet."),
    "goblin_scout": ("goblin_scout", "bow", "a goblin woman scout, dark green skin, long pointed ears, long black hair, a ragged dark green and brown leather outfit."),
    "bandit_chief": ("bandit_chief", "chop", "a burly bandit chief, black hair and a full black beard, a red shirt under a brown leather vest, a wide belt, dark trousers, black boots."),
    "outlaw": ("outlaw", "slash", "an outlaw man, short brown hair, a green tunic with a brown belt, dark trousers, brown boots."),
    "bandit": ("bandit", "slash", "a bandit man, a black hood, a black cloth mask over the lower face, black leather clothes, dark boots."),
    "bandit_f": ("bandit_f", "slash", "a bandit woman, a dark green hood, a red cloth mask over the lower face, a dark green and black leather outfit, dark boots."),
    "skeleton": ("skeleton", "slash", "a walking skeleton, bare yellowed bones, empty eye sockets."),
    "skeleton_f": ("skeleton_f", "slash", "a skeleton woman, bare yellowed bones, long ragged dark hair."),
    "fisher": ("fisher", None, "a fisherman, an old man with a grey beard, a dark blue wool cap, a yellow oilskin coat, brown boots."),
    "town_crier": ("town_crier", None, "a town crier, a black top hat, a long red coat with gold buttons, dark trousers, black boots."),
    "farmer": ("farmer", None, "a farmer, a wide straw hat, brown hair, a beige linen shirt, blue trousers, brown boots."),
    "blacksmith_f": ("blacksmith_f", None, "a blacksmith woman, red hair tied back, a dark brown leather apron over a dark shirt, brown boots."),
    "wizard": ("wizard", "cast", "an old wizard, a long white beard, a tall pointed blue hat, a long blue robe with a belt."),
    "wizard_f": ("wizard_f", "cast", "a sorceress, long dark hair, a tall pointed purple hat, a long purple robe."),
    "elf": ("elf", "bow", "a young elf man, tall and slender, long straight silver-blond hair, pointed ears, a forest-green tunic with a pale leaf-pattern trim and a brown belt, grey-green trousers, soft brown boots."),
    "elf_f": ("elf_f", "bow", "a young elf woman, tall and slender, long straight golden hair, pointed ears, a forest-green tunic with a pale leaf-pattern trim and a brown belt, grey-green leggings, soft brown boots."),
    "dwarf": ("dwarf", "chop", "a dwarf man, very short and broad (two thirds of a human's height), a long braided red beard down to his belt, a round iron helmet, a brown leather jerkin over a rust-red shirt, a wide belt with a big iron buckle, heavy boots."),
    "dwarf_f": ("dwarf_f", "chop", "a dwarf woman, very short and broad (two thirds of a human's height), auburn hair in two thick braids, a brown leather jerkin over a mustard-yellow shirt, a wide belt, heavy boots."),
    "orc": ("orc", "slash", "an orc warrior man, big and muscular, grey-green skin, small tusks, black hair in a topknot, crude dark leather armour with iron studs and fur on the shoulders."),
    "orc_f": ("orc_f", "slash", "an orc warrior woman, muscular, grey-green skin, small tusks, black braided hair, crude dark leather armour with iron studs."),
    "orc_chief": ("orc_chief", "chop", "the orc chief, huge and muscular, dark green skin, big tusks, a horned iron helmet, heavy spiked iron armour, a red cape."),
    "troll": ("troll", "club", "a huge troll, hulking and hunched, mossy blue-grey skin, a big nose, small eyes, long arms, a ragged loincloth and a fur shoulder wrap."),
}

# class: (standing height px, (frame w, frame h), sheet scale, shadow radius x)
P = (64, 96)
SIZE = {
    "hero": (80, P, 2, 13), "heroine": (77, P, 2, 12), "guard": (79, P, 2, 13), "guard_f": (77, P, 2, 12),
    "goblin": (73, P, 2, 12), "goblin_scout": (85, P, 2, 12), "bandit_chief": (81, P, 2, 14), "outlaw": (80, P, 2, 13),
    "bandit": (80, P, 2, 13), "bandit_f": (79, P, 2, 12), "skeleton": (85, P, 2, 12), "skeleton_f": (84, P, 2, 12),
    "fisher": (81, P, 2, 13), "town_crier": (84, P, 2, 13), "farmer": (84, P, 2, 13), "blacksmith_f": (77, P, 2, 12),
    "wizard": (78, P, 2, 13), "wizard_f": (81, P, 2, 12), "elf": (87, P, 2, 13), "elf_f": (85, P, 2, 12),
    "dwarf": (70, P, 2, 15), "dwarf_f": (68, P, 2, 14), "orc": (88, (80, 96), 2, 17), "orc_f": (82, (80, 96), 2, 16),
    "orc_chief": (86, (80, 96), 2, 17), "troll": (125, (96, 144), 3, 26),
}
# Sheets whose first side row faces right (checked by eye after generating).
SIDE_FACES_RIGHT = set()
