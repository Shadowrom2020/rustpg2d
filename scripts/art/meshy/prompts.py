"""Prompts for the character sheets (4 x 4 grids: rows front / left / right / back)."""

COMMON = ("Pixel art in exactly the art style of the references (same shading, dark brown outline, proportions, "
          "face style, same figure size and pixel size as in the second reference). "
          "Layout exactly like the second reference image: a grid of 4 rows and 4 columns, every cell the same size, one "
          "full-body figure per cell, nothing overlapping or crossing into another cell. Row 1: facing the viewer (front). "
          "Row 2: facing left (side view). Row 3: facing right (side view). Row 4: facing away (back view). "
          "Exactly the same character, colours and size in every frame, feet at the same height in every cell. "
          "Plain flat background, no text, no labels, no ground shadows. ")
WALK = "The 4 columns are a walk cycle with clear steps: standing, left foot well forward, standing, right foot well forward. "
ATTACK = ("Pixel art sprite sheet. The character is exactly the character of the first reference image (same face, clothes, "
          "colours, proportions, size and pixel style). Layout like the second reference image: a grid of exactly 4 rows and "
          "4 columns, every cell the same size, one full-body figure per cell, nothing overlapping or crossing into another "
          "cell. Row 1: facing the viewer (front). Row 2: facing left (side view). Row 3: facing right (side view). Row 4: "
          "facing away (back view). Feet at the same height in every cell. Plain flat background, no text, no labels, no "
          "ground shadows, no motion lines or effects. ")

ACTIONS = {
    "bow": "The 4 columns are a bow shot: 1 raising the bow and drawing an arrow, 2 the bow fully drawn, aiming in the facing direction, 3 the arrow released and the string snapping forward, 4 lowering the bow.",
    "chop": "The 4 columns are a heavy overhead blow: 1 the weapon raised high behind the head, 2 swinging it down in front in the facing direction, 3 the weapon low in front after the blow, 4 back to standing.",
    "slash": "The 4 columns are a sword slash: 1 the sword raised to the side ready to strike, 2 slashing across in the facing direction, 3 follow-through with the sword low on the other side, 4 back to standing.",
    "club": "The 4 columns are a club smash: 1 the club raised high over the head, 2 smashing it down in front in the facing direction, 3 the club down at the feet in front, 4 lifting it back up.",
}


def walk(desc):
    return COMMON + WALK + "The character: " + desc


def attack(desc, action):
    return ATTACK + ACTIONS[action] + " The character: " + desc
