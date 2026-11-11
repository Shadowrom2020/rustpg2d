"""The characters to draw: name -> (size class, attack action or None, description), and the
size classes: standing height in sheet pixels, frame size, image pixels per world pixel, shadow
radius. Sheets are 2x art (64 x 96 frames for people); the troll is 3x art in 96 x 144 frames,
drawn at race size 1.5."""

CAST = {
    "elf": ("elf", "bow", "a young elf man, tall and slender, long straight silver-blond hair, pointed ears, a forest-green tunic with a pale leaf-pattern trim and a brown belt, grey-green trousers, soft brown boots, a wooden longbow in his left hand."),
    "elf_f": ("elf_f", "bow", "a young elf woman, tall and slender, long straight golden hair, pointed ears, a forest-green tunic with a pale leaf-pattern trim and a brown belt, grey-green leggings, soft brown boots, a wooden longbow in her left hand."),
    "dwarf": ("dwarf", "chop", "a dwarf man, very short and broad (two thirds of a human's height), a long braided red beard down to his belt, a round iron helmet, a brown leather jerkin over a rust-red shirt, a wide belt with a big iron buckle, heavy boots, a war axe in his right hand."),
    "dwarf_f": ("dwarf_f", "chop", "a dwarf woman, very short and broad (two thirds of a human's height), auburn hair in two thick braids, a brown leather jerkin over a mustard-yellow shirt, a wide belt, heavy boots, a war hammer in her right hand."),
    "orc": ("orc", "slash", "an orc warrior man, big and muscular, grey-green skin, small tusks, black hair in a topknot, crude dark leather armour with iron studs and fur on the shoulders, a jagged iron sword in his right hand."),
    "orc_f": ("orc_f", "slash", "an orc warrior woman, muscular, grey-green skin, small tusks, black braided hair, crude dark leather armour with iron studs, a jagged iron sword in her right hand."),
    "troll": ("troll", "club", "a huge troll, hulking and hunched, mossy blue-grey skin, a big nose, small eyes, long arms, a ragged loincloth and a fur shoulder wrap, a heavy wooden club in his right hand."),
    "orc_chief": ("orc", "chop", "the orc chief, huge and muscular, dark green skin, big tusks, a horned iron helmet, heavy spiked iron armour, a red cape, a massive double-bladed battle axe in his right hand."),
    "dwarf_merchant": ("dwarf", None, "a dwarf merchant, very short and broad (two thirds of a human's height), a long braided grey beard, a green cap, a brown leather apron over a blue shirt, a small pick-axe at his belt, empty hands."),
    "elf_herbalist": ("elf_f", None, "an elven herbalist woman, tall and slender, long chestnut hair with a crown of flowers, pointed ears, a pale green robe down to the ankles, a basket of herbs in her left hand."),
}

# class: (standing height px, (frame w, frame h), sheet scale, shadow radius x)
SIZE = {
    "elf": (86, (64, 96), 2, 13), "elf_f": (83, (64, 96), 2, 12),
    "dwarf": (70, (64, 96), 2, 15), "dwarf_f": (68, (64, 96), 2, 14),
    "orc": (88, (80, 96), 2, 17), "orc_f": (84, (80, 96), 2, 16),
    "troll": (126, (96, 144), 3, 26),
}
# Sheets whose first side row faces right (checked by eye after generating).
SIDE_FACES_RIGHT = set()
