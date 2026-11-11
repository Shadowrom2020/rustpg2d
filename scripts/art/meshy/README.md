# Character art with Meshy

The complete-figure character sheets of the example and the starter library (people, races,
animals, monsters) are drawn by Meshy's image-to-image API from the shipped sheets as style
references, then converted to the game's 16-row sheet layout here. Nothing in the game or the
editor depends on these scripts; for single assets the editor has the same pipeline built in
(*Assets › Generate art…*, `crates/editor/src/art/`).

## Setup

- The API key goes in `~/.config/meshy/api_key` (or `$MESHY_API_KEY`); the scripts read it and
  never print or write it.
- Generated pictures, references and job files go to `work/` next to the scripts (git-ignored),
  or `$ART_WORK`.
- Python 3 with Pillow and NumPy.

## Steps

```sh
cd scripts/art/meshy
python3 meshy.py                      # balance
python3 refs.py hero villager_f       # references from the shipped sheets (walk + attack grids)
# add the characters to cast.py (size class, attack action or None, description)
python3 jobs.py                       # work/jobs_walk.json, work/jobs_attack.json
python3 gen.py work/jobs_walk.json    # 4 x 4 walk grids (3 credits each with nano-banana-2-lite)
python3 gen.py work/jobs_attack.json  # attack grids, the walk grid as the character reference
python3 sprites.py --all              # work/out/<name>.png: 4 x 16 game sheets
```

`gen.py` keeps every task id in `work/gen_state.json` and skips finished pictures, so a rerun
never pays twice; delete `work/gen/<job>.png` (and its entry) to draw one again. `jobs.py NAME…`
writes the jobs of some characters only.

**People without what they hold** (0.17: the game draws weapons, tools and shields at the figures'
hand marks): with `ART_CAST=noweapon ART_WORK=$PWD/work_noweapon` the scripts use
`cast_noweapon.py` (every armed figure, its own size class = its earlier standing height), empty
fists in the walk prompt, attacks as arm motions, and `ref_villager_walk` / `ref_villager_f_walk`
as style references (`python3 refs.py villager villager_f`). After adding the sheets, estimate
their hands (sprite editor: *Hands* › *Estimate all again*, or `pxopen …; pxhands estimateall;
pxsave`) and swap the side attack rows of archers (`pxhands swap`); re-pick colour features with
`../features.py` (`pick NAME IMAGE OUT_DIR`, then `write SHEETS.RON ID SHADES.json`).

Then add the sheets with the editor or its agent server (`write_asset`, `upsert_def` with
`kind: Body`, `complete: true`, `scale: 2` — the troll: 96 x 144 frames at `scale: 3`), the
characters / races, send them to the starter library (`library_send`) and regenerate
`crates/editor/assets/starter_history.ron` (`cargo run -p rustpg2d-editor --example
starter_history`).

## What the converter does

- Each grid cell keeps its figure: the largest shape and the parts near it (weapons); pieces of
  neighbouring cells (touching the sides, or wholly above / below the figure) are dropped, and
  pixels the background removal made see-through inside the figure are filled.
- One scale per size class (`cast.py SIZE`: standing height, frame size), feet on (w / 2, h - 3).
- The first side row is "left" (`SIDE_FACES_RIGHT` lists sheets drawn the other way), "right" is
  its mirror; single frames turned the other way are flipped.
- Faded frames (the model sometimes fades one) are replaced by their sister frame (walk) or
  dropped (attack); frames much smaller than the rest of their row likewise.
- Idle frame 2 breathes, hurt leans back, characters without an attack sheet stand in the attack
  rows; one 48-colour palette per sheet and a soft shadow.

Look at every sheet before using it (`work/out`): the model occasionally invents details (a
second weapon in one pose); replace such a frame with a good one of the same row.
