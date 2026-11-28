#!/bin/sh
# The manual's editor pictures (Documentation/images/editor-*.webp), taken again.
#   scripts/studio/manual-shots/run.sh SCRATCH NAME...     e.g. run.sh /tmp/shots map lighting
# SCRATCH gets a copy of the example (game/), scratch settings and the shots (out/NAME.png).
# A script's first line "#head FILE" names its header (default head.txt: the Problems divider
# dragged down; headc.txt closes the bottom panel). nodes.txt runs on what nodes_prepare.txt and
# nodes_build.txt made (a visual script the example doesn't have). Convert the shots to lossless
# WebP (PIL: save(path, 'WEBP', lossless=True, method=6)); some are cropped: compare with the
# picture in Documentation/images.
set -e
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
S=$1; shift
mkdir -p "$S/out" "$S/bin"
[ -d "$S/game" ] || rsync -a --exclude saves "$ROOT/example_project/" "$S/game/"
cp "$ROOT/target/debug/rustpg2d-studio" "$ROOT/target/debug/rustpg2d-game" "$S/bin/"
cd "$S"
for n in "$@"; do
  h=$(sed -n 's/^#head //p' "$HERE/$n.txt"); h=${h:-head.txt}
  script=$(cat "$HERE/$h" "$HERE/$n.txt" | grep -v '^#' | grep -v '^\s*$' | sed "s#\$S#$S#g" | paste -sd';')
  echo "=== $n"
  RUSTPG2D_EDITOR_CONFIG=$S/cfg RUSTPG2D_LIBRARY=$S/lib RUSTPG2D_SETTINGS_DIR=$S/settings HOME=$S/home RUSTPG2D_REPO=0 \
    RUSTPG2D_FOLDER_PROJECTS=1 RUSTPG2D_MUTE=1 RUSTPG2D_STUDIO_SCRIPT="$script" timeout 240 "$S/bin/rustpg2d-studio" game 2>&1 | tail -n 5
done
