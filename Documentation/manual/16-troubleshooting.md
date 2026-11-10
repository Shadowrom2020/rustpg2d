# 16. Troubleshooting

**The editor or the game doesn't start, or shows a message about OpenGL.**
R(ust)PG2D needs a graphics driver with OpenGL 3. Update the graphics driver; in a virtual machine,
turn on 3D acceleration.

**Windows says "Windows protected your PC" / macOS says "Apple could not verify…".**
R(ust)PG2D and the games it exports aren't signed with a publisher certificate. Windows: *More info ›
Run anyway*. macOS: *System Settings › Privacy & Security › Open Anyway*. See
[Getting started](01-getting-started.md#installing) and [Publishing](14-publishing.md).

**The file dialogs don't open on Linux.**
They need a running xdg-desktop-portal (standard on GNOME and KDE desktops). You can always type the
path instead. On macOS a file dialog is modal: the editor waits until it closes.

**A banner says a file could only be read in part.**
An entry of a definitions file or a map object couldn't be read (a typo from a hand edit, or written
by a newer version). The editor left it out and backed the file up next to itself (`items.ron.bak`,
`village.map.bak`). Fix the entry, then reload. See
[The editor](02-the-editor.md#files-the-editor-can-only-read-in-part).

**A banner says files changed on disk.**
Another program (or an AI agent) changed project files while you had unsaved edits to them. Choose
*Reload* to take the new files or *Keep mine* to keep your edits.

**The export refuses a game runner "built from another engine".**
In a source build, the runner for that platform is older than the editor; a game exported with it
might not read your maps. Rebuild it with the command the message names (e.g.
`cargo build --release --target wasm32-unknown-unknown -p rustpg2d-game` for the web). The release
downloads always ship matching runners.

**A web export shows "The game could not start".**
The lines under it name the cause. Serve the folder over HTTP (browsers don't run WebAssembly from
`file://`), and export again with a matching web runner (above).

**Enemies I placed don't come back.**
Characters placed directly on a map stay defeated. Use an [enemy spawner](04-objects-and-zones.md#enemy-spawners)
for enemies that return.

**A trigger runs twice / two conversations show at once.**
Every trigger whose conditions hold runs, and conditions are checked before any action. Give such
triggers conditions that exclude each other ([Events › How triggers run](07-events.md#how-triggers-run)).

**A script doesn't see the answer to a choice.**
`choice` and `say` are queued and run after your function returns; continue with `then_call`
([Scripting](08-scripting.md#the-one-rule-immediate-and-queued)).

**Music has a short gap at the loop.**
Use Ogg Vorbis or WAV at 44.1 kHz; the engine makes the loop seamless, and a track whose length is a
multiple of 4096 samples plays exactly as it is.

**My asset library didn't get the newest starter assets.**
The library only gains assets it doesn't have yet, so your edits are never overwritten. To start over
from the newest starter set, back up the library folder, delete it and open the editor.
