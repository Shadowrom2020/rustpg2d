#!/bin/sh
# The renderer comparison set (ROADMAP 0.36.0, part 1): the same autoplay runs, one picture each,
# with every frame taking a sixtieth of a second (RUSTPG2D_FIXED_STEP) so two runners draw the
# same frames.
# usage: shots.sh GAME_BINARY OUT_DIR [REPO]
BIN=$1; OUT=$2; REPO=${3:-$(pwd)}
WORK=$(mktemp -d "${TMPDIR:-/tmp}/shots.XXXXXX")
mkdir -p "$OUT" "$WORK/settings"
cp -r "$REPO/example_project" "$WORK/proj" && rm -rf "$WORK/proj/saves"
export RUSTPG2D_MUTE=1 RUSTPG2D_FIXED_STEP=0.016666668 RUSTPG2D_SETTINGS_DIR="$WORK/settings" RUSTPG2D_PHOTOS_DIR="$WORK/photos"
run() { # NAME SCRIPT ARGS…
  name=$1; script=$2; shift 2
  RUSTPG2D_AUTOPLAY="drag 2 2 2 2; $script; status; shot $OUT/$name.png; quit" timeout 90 "$BIN" "$WORK/proj" "$@" 2>&1 | grep '^\[autoplay\]' | tail -1 | cut -c1-200 > "$OUT/$name.txt"
  [ -f "$OUT/$name.png" ] && echo "ok   $name" || echo "FAIL $name"
}
run title        "wait 3"
run village_day  "settime 12; wait 2"                       --no-title --map village --pos 816,400
run village_dusk "settime 19; wait 2"                       --no-title --map village --pos 816,400
run village_night "settime 23; wait 2"                      --no-title --map village --pos 1296,560
run forest       "settime 10; wait 2"                       --no-title --map forest --pos 304,656
run fight        "seed 5; wait 2; waitturn 30; wait 1"      --no-title --map forest --pos 304,656
run mine         "wait 2"                                   --no-title --map mine
run tavern       "wait 2"                                   --no-title --map tavern
run house        "wait 2"                                   --no-title --map house --spawn start
run charwin      "wait 1; press c; wait 1"                  --no-title --map house --spawn start
run sheet        "wait 1; press c; wait 0.5; wintab sheet; wait 1" --no-title --map house --spawn start
run spells       "wait 1; press k; wait 1"                  --no-title --map house --spawn start
run journal      "wait 1; press j; wait 1"                  --no-title --map house --spawn start
run mapscreen    "wait 1; press m; wait 1.5"                --no-title --map village --pos 816,400
run pause        "wait 1; press escape; wait 1"             --no-title --map house --spawn start
run options      "wait 1; press escape; wait 0.5; press down; press down; press down; press e; wait 1" --no-title --map house --spawn start
run rain         "wait 1; console set_weather(\"rain\", 0.8); wait 3" --dev --no-title --map village --pos 816,400
run photo        "settime 15; wait 1; press p; wait 1"      --no-title --map village --pos 816,400
rm -rf "$WORK"
