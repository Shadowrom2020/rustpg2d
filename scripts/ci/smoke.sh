#!/usr/bin/env bash
# Runtime smoke test: the game and the editor really run on a scratch copy of the example project
# (CI's "Runtime smoke" job; runs the same way on a developer machine). It checks `status` fields
# of scripted runs: title screen, a map with the player, the clock, loading an old save game,
# quick-saving, a map change with its autosave, the editor opening a map, and the agent server's
# smoke client (crates/editor/examples/mcp_smoke.rs).
#
# Needs target/debug/rustpg2d-game, rustpg2d-editor and examples/mcp_smoke:
#   cargo build -p rustpg2d-game -p rustpg2d-editor && cargo build -p rustpg2d-editor --example mcp_smoke
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
export RUSTPG2D_MUTE=1 RUSTPG2D_SETTINGS_DIR=$WORK/settings RUSTPG2D_EDITOR_CONFIG=$WORK/cfg RUSTPG2D_LIBRARY=$WORK/lib HOME=$WORK/home
if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && command -v xvfb-run >/dev/null; then
  X=(xvfb-run -a -s "-screen 0 1280x720x24")
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
if grep -q "version: 1" "$WORK/proj/saves/quick.ron" 2>/dev/null; then echo "PASS quick save written"; else echo "FAIL quick save written: saves/quick.ron was not rewritten with version: 1"; failed=$((failed + 1)); fi

# The fixture save moved to the village's east edge: load it, step over the edge, autosave.
sed -e 's/current_map: "house"/current_map: "village"/' -e 's/player_pos: ([^)]*)/player_pos: (2556.0, 1008.0)/' "$FIXTURES/saves/save-0.11.0.ron" > "$WORK/proj/saves/quick.ron"
rm -f "$WORK/proj/saves/autosave.ron"
line=$(game "wait 1; press f9; wait 1.5; hold right 0.12; wait 3; status; quit" --no-title --map house --spawn start)
expect "a map change with its autosave" "$line" "map=forest" "autosave=saved"
if grep -q 'current_map: "forest"' "$WORK/proj/saves/autosave.ron" 2>/dev/null; then echo "PASS autosave file"; else echo "FAIL autosave file: saves/autosave.ron missing or not on the forest"; failed=$((failed + 1)); fi

line=$(RUSTPG2D_EDITOR_SCRIPT="wait 2; open village; status; quit" timeout 120 "${X[@]}" "$BIN/rustpg2d-editor" "$WORK/proj" 2>&1 | tee -a "$WORK/editor.log" | grep '^\[status\]' | tail -1)
expect "the editor opens a map" "$line" "open=true" 'map=Some("village")'

# The agent server: the smoke client lists the tools, edits the project, writes art, captures a
# structure, sends assets to the scratch library and play-tests (it needs the game next to the editor).
RUSTPG2D_EDITOR_SCRIPT="mcp on $PORT; wait 300; quit" timeout 330 "${X[@]}" "$BIN/rustpg2d-editor" "$WORK/proj" > "$WORK/editor-mcp.log" 2>&1 &
editor=$!
for _ in $(seq 1 120); do
  curl -s -o /dev/null "http://127.0.0.1:$PORT/mcp" && break
  sleep 0.5
done
if timeout 300 "$BIN/examples/mcp_smoke" "$PORT" > "$WORK/mcp_smoke.log" 2>&1; then echo "PASS agent server smoke client"; else echo "FAIL agent server smoke client:"; cat "$WORK/mcp_smoke.log"; failed=$((failed + 1)); fi
kill "$editor" 2>/dev/null; wait "$editor" 2>/dev/null

if [ "$failed" -gt 0 ]; then
  echo "$failed check(s) failed (logs in $WORK)"
  exit 1
fi
echo "all smoke checks passed"
