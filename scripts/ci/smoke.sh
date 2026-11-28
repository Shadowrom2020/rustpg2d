#!/usr/bin/env bash
# Runtime smoke test: the game and the editor really run on a scratch copy of the example project
# (CI's "Runtime smoke" job; runs the same way on a developer machine). It checks `status` fields
# of scripted runs: title screen, a map with the player, the clock, loading an old save game,
# quick-saving, a map change with its autosave, a turn-based fight, the agent server of the editor
# without a window driven by its smoke client (crates/studio/examples/mcp_smoke.rs), and the editor
# in a window opening a map.
#
# Needs target/debug/rustpg2d-game, rustpg2d-studio and examples/mcp_smoke:
#   cargo build -p rustpg2d-game -p rustpg2d-studio && cargo build -p rustpg2d-studio --example mcp_smoke
#   scripts/ci/smoke.sh            # BIN=<dir> for other binaries, WORK=<dir> to keep the logs
# Without a display (CI) every run goes through xvfb-run.
set -uo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BIN=${BIN:-$ROOT/target/debug}
WORK=${WORK:-${RUNNER_TEMP:-/tmp}/rustpg2d-smoke}
PORT=${PORT:-7743}
FIXTURES=$ROOT/crates/engine/tests/fixtures
rm -rf "$WORK"
mkdir -p "$WORK/proj" "$WORK/home" "$WORK/settings" "$WORK/cfg" "$WORK/lib"
(cd "$ROOT/example_project" && find . -path ./saves -prune -o -type f -print | cpio -pdm "$WORK/proj" 2>/dev/null)
# (RUSTPG2D_REPO=0: started in the checkout, the editor would otherwise be in repository mode.)
export RUSTPG2D_MUTE=1 RUSTPG2D_SETTINGS_DIR=$WORK/settings RUSTPG2D_EDITOR_CONFIG=$WORK/cfg RUSTPG2D_LIBRARY=$WORK/lib HOME=$WORK/home RUSTPG2D_REPO=0
if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && command -v xvfb-run >/dev/null; then
  X=(xvfb-run -a -s "-screen 0 1280x720x24")
  # No graphics card under Xvfb: the software renderer (lavapipe) is fine for a test.
  export RUSTPG2D_ALLOW_SOFTWARE=1
else
  X=()
fi
failed=0
# game SCRIPT ARGS... : runs the game with an autoplay script; prints its last [autoplay] line.
game() {
  local script=$1; shift
  RUSTPG2D_AUTOPLAY="$script" timeout 120 "${X[@]}" "$BIN/rustpg2d-game" "$WORK/proj" "$@" 2>&1 | tee -a "$WORK/game.log" | grep '^\[autoplay\]' | tail -1
}
# expect NAME LINE PATTERN... : every pattern must occur in the line.
expect() {
  local name=$1 line=$2; shift 2
  for pat in "$@"; do
    if ! grep -qF -- "$pat" <<<"$line"; then
      echo "FAIL $name: no '$pat' in: ${line:0:300}"
      failed=$((failed + 1))
      return
    fi
  done
  echo "PASS $name"
}

line=$(game "wait 2; status; quit")
expect "title screen" "$line" "menu=title"

# Quitting ends the game with exit code 0 (a crash while closing, e.g. freeing fonts after the
# graphics context, would show here as 139).
if RUSTPG2D_AUTOPLAY="wait 1; quit" timeout 120 "${X[@]}" "$BIN/rustpg2d-game" "$WORK/proj" >> "$WORK/game.log" 2>&1; then
  echo "PASS the game quits cleanly"
else
  echo "FAIL the game quits cleanly: exit code $?"
  failed=$((failed + 1))
fi

line=$(game "wait 2; status; quit" --no-title --map house --spawn start)
expect "a map with the player" "$line" "map=house" "hp=Some(" "fps="

# A new game starts on day 0 at 08:00; twenty hours later it is 04:00 of day 1.
line=$(game "wait 1; settime +20; wait 1; status; quit" --no-title --map house --spawn start)
expect "the clock advances a day" "$line" "day=1" "time=04:0"

# The fixture save (written by 0.11.0, no `version` yet) loads; quick-saving (F5) rewrites it.
mkdir -p "$WORK/proj/saves"
cp "$FIXTURES/saves/save-0.11.0.ron" "$WORK/proj/saves/quick.ron"
line=$(game "wait 1; press f9; wait 2; status; press f5; wait 1; quit" --no-title --map house --spawn start)
expect "an old save game loads" "$line" "map=house" "fixture_flag" '"potion": 2'
if grep -q "version: 2" "$WORK/proj/saves/quick.ron" 2>/dev/null; then echo "PASS quick save written"; else echo "FAIL quick save written: saves/quick.ron was not rewritten with version: 2"; failed=$((failed + 1)); fi

# The fixture save moved to the village's east edge: load it, step over the edge, autosave.
sed -e 's/current_map: "house"/current_map: "village"/' -e 's/player_pos: ([^)]*)/player_pos: (2556.0, 1008.0)/' "$FIXTURES/saves/save-0.11.0.ron" > "$WORK/proj/saves/quick.ron"
rm -f "$WORK/proj/saves/autosave.ron"
line=$(game "wait 1; press f9; wait 1.5; hold right 0.12; wait 3; status; quit" --no-title --map house --spawn start)
expect "a map change with its autosave" "$line" "map=forest" "autosave=saved"
if grep -q 'current_map: "forest"' "$WORK/proj/saves/autosave.ron" 2>/dev/null; then echo "PASS autosave file"; else echo "FAIL autosave file: saves/autosave.ron missing or not on the forest"; failed=$((failed + 1)); fi

# A turn-based fight: a giant rat near Whisperwood's entrance notices the hero and one starts; the AI plays the
# enemy's turn (without pauses) until it is the player's turn again, and the player's attack is a roll.
# (waitturn, not fixed waits: on a slow runner the fight starts later.)
line=$(game "seed 5; wait 2; waitturn 30; turn fast; turn end; waitturn 30; turn attack giant-rat; wait 1; status; quit" --no-title --map forest --pos 304,656)
# (Two attack rolls: the rat's on its turn, then the player's.)
expect "a turn-based fight" "$line" "rolls=attack:" ",attack:"

# The new editor without a window (ROADMAP.md, 0.36.0): its agent server on the scratch project,
# driven by the smoke client (lists the tools, edits the project, writes art, captures a
# structure, sends assets to the scratch library, play-tests; needs the game next to it).
if [ -x "$BIN/rustpg2d-studio" ] && [ -x "$BIN/examples/mcp_smoke" ]; then
  cp -r "$WORK/proj" "$WORK/agent-proj"
  # (Under the display the games get: the editor itself opens no window, but its play-tests do.)
  RUSTPG2D_FOLDER_PROJECTS=1 RUSTPG2D_REPO=0 "${X[@]}" "$BIN/rustpg2d-studio" --headless "$WORK/agent-proj" --mcp "$PORT" --seconds 300 > "$WORK/studio-mcp.log" 2>&1 &
  studio=$!
  for _ in $(seq 1 120); do
    curl -s -o /dev/null "http://127.0.0.1:$PORT/mcp" && break
    sleep 0.5
  done
  if timeout 280 "$BIN/examples/mcp_smoke" "$PORT" headless > "$WORK/mcp_smoke.log" 2>&1; then echo "PASS agent server smoke client (no window)"; else echo "FAIL agent server smoke client (no window):"; cat "$WORK/mcp_smoke.log"; failed=$((failed + 1)); fi
  kill "$studio" 2>/dev/null; pkill -f "rustpg2d-studio --headless $WORK/agent-proj" 2>/dev/null; wait "$studio" 2>/dev/null
else
  echo "SKIP the agent server: no $BIN/rustpg2d-studio or $BIN/examples/mcp_smoke"
fi

# The new editor in a window: it opens the scratch project (a folder: converted into a project file
# beside it, which it works on), opens a map, and quits cleanly when its script says so.
if [ -x "$BIN/rustpg2d-studio" ]; then
  out=$(RUSTPG2D_STUDIO_SCRIPT="wait 2; map.open village; wait 1; status; quit" timeout 120 "${X[@]}" "$BIN/rustpg2d-studio" "$WORK/proj" 2>&1; echo "[exit] $?")
  printf '%s\n' "$out" >> "$WORK/studio.log"
  line=$(grep '^\[status\]' <<<"$out" | tail -1)
  expect "the editor opens a map" "$line" "project=R(ust)PG2D_example" "focus=map:village" "map=village"
  expect "the editor quits cleanly" "$(grep '^\[exit\]' <<<"$out")" "[exit] 0"
  if [ -s "$WORK/proj.r2dproject" ]; then echo "PASS the folder becomes a project file"; else echo "FAIL the folder becomes a project file: no $WORK/proj.r2dproject"; failed=$((failed + 1)); fi
else
  echo "SKIP the editor in a window: no $BIN/rustpg2d-studio"
fi

if [ "$failed" -gt 0 ]; then
  echo "$failed check(s) failed (logs in $WORK)"
  exit 1
fi
echo "all smoke checks passed"
