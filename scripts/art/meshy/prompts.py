"""Prompts for the character sheets (4 x 4 grids: rows front / left / right / back). With
ART_CAST=noweapon the figures hold nothing (the game draws held items at their hands)."""
import os

EMPTY_HANDS = os.environ.get("ART_CAST") == "noweapon"
EMPTY = ("IMPORTANT: the character holds nothing at all: no weapon, no sword, no spear, no bow, no staff, no tool, no shield, "
         "nothing in the hands and nothing on the back. Both hands are empty and closed into loose fists, as if gripping a "
         "handle. ")

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


# The attack motions with empty fists (ART_CAST=noweapon): the game puts the weapon in the hand.
MOTIONS = {
    "slash": "The 4 columns are a sword-slash motion with an EMPTY right fist (no sword): 1 the right fist raised to the side ready to strike, 2 the right arm swinging across in the facing direction, 3 follow-through with the right fist low on the other side, 4 back to standing.",
    "chop": "The 4 columns are an overhead blow with EMPTY fists (no weapon): 1 both fists raised high behind the head, 2 both arms swinging down in front in the facing direction, 3 both fists low in front after the blow, 4 back to standing.",
    "club": "The 4 columns are a club smash with EMPTY fists (no club): 1 both fists raised high over the head, 2 both arms smashing down in front in the facing direction, 3 both fists down near the feet in front, 4 lifting the arms back up.",
    "thrust": "The 4 columns are a spear-thrust motion with EMPTY fists (no spear): 1 both fists pulled back at the hip, 2 both arms thrusting forward in the facing direction, 3 both arms fully extended forward, 4 back to standing.",
    "bow": "The 4 columns are an archery motion with EMPTY hands (no bow, no arrow): 1 raising the left fist forward, 2 the left arm fully extended forward in the facing direction and the right fist pulled back to the cheek, 3 the right hand opening and moving back, 4 lowering both arms.",
    "cast": "The 4 columns are casting a spell with EMPTY hands (no staff, no effects): 1 the right hand raised high, 2 the right arm thrust forward in the facing direction, palm open, 3 the arm fully extended, 4 back to standing.",
}


def walk(desc):
    return COMMON + WALK + (EMPTY if EMPTY_HANDS else "") + "The character: " + desc


def attack(desc, action):
    if EMPTY_HANDS:
        return ATTACK + EMPTY + MOTIONS[action] + " The character: " + desc
    return ATTACK + ACTIONS[action] + " The character: " + desc
