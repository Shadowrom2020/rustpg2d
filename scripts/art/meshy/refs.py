"""Builds the reference images from the shipped 2x character sheets (64 x 96 frames, 16 rows):
the walk cycle grid (rows 0-3) and the attack grid (rows 8-11), enlarged 4x on a flat
background, as WORK/ref_<sheet>_walk.png / ref_<sheet>_attack.png."""
import os, sys
from PIL import Image
from common import STARTER, WORK


def grid(sheet, first_row, out, bg=(140, 140, 150)):
    im = Image.open(os.path.join(STARTER, "sprites", sheet + ".png")).convert("RGBA")
    fw, fh = im.width // 4, im.height // 16
    part = im.crop((0, first_row * fh, 4 * fw, (first_row + 4) * fh))
    flat = Image.new("RGBA", part.size, bg + (255,))
    flat.alpha_composite(part)
    flat.convert("RGB").resize((part.width * 4, part.height * 4), Image.NEAREST).save(out)
    return out


if __name__ == "__main__":
    for s in sys.argv[1:] or ["hero", "villager_f"]:
        print(grid(s, 0, os.path.join(WORK, f"ref_{s}_walk.png")))
    print(grid("guard", 8, os.path.join(WORK, "ref_guard_attack.png")))
