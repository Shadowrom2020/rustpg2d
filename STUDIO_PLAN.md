# The Studio: what is left, task by task

This file is for an agent that picks up **one task** of the editor rewrite (ROADMAP.md,
*0.36.0 — The Studio*) and finishes it in one session. It says what to build, where, and how to
know it works. It was written on 2026-11-28, after part 10; the maintainer's decisions of that
day are in section 3.

Read sections 1 and 2 once. Then take the **first task in section 4 that is not ticked**, unless
you were told which one. Do that task and nothing else.

---

## 1. Rules for every task

**Where you work**

- Branch `feature/studio`. Check with `git branch --show-current` and `git status` before you
  change anything. Never commit to `master`.
- The new editor is `crates/studio` (the window) over `crates/editor-core` (its logic, no UI).
  `crates/editor` is the old 0.35 editor: **read it, never change it, never build it**.
- Engine code (`crates/engine`, `crates/gfx`, `crates/game`) is changed only when a task says so.
  If you think a task needs an engine change that it doesn't name, stop and say so in your report.

**What you must not touch**

- The user's own data: `~/Test`, `~/releases`, `~/.local/share/rustpg2d`, `~/.config/rustpg2d`,
  `~/.config/rust2d`, `~/.local/share/rust2d`, `example_project/saves/`. Never delete, rename or
  overwrite any of them.
- Every run of the editor or the game uses scratch folders (see *Running the editor* below).
  Without those variables the editor reads and writes the user's real settings and library.
- Never run `cargo fmt` or `rustfmt`. The code has long lines on purpose.
- Never start a Meshy job without `RUSTPG2D_MESHY_FAKE=<dir>`; a UI test never clicks *Draw it*.
- `example_project/` and `crates/editor/assets/starter_library/` are content. No task here
  changes them.

**Style of the code**

- Match what is around you: lines up to about 160 characters, `///` on items, short comments
  that say *why*.
- Texts the creator reads are plain sentences. No emoji and no arrows (`→`, `▲`, `✔`) in any
  string the editor draws: the font lacks them, and a UI test panics on a missing glyph.
- Every control has a name a test can find. A button whose text is not unique gets one with
  `widget_info` (see `Named` in `crates/studio/src/pluginspage.rs`).
- Something that changes the project is one step of the history: call `ws.begin("What was
  done")` before changing, or `ws.begin_unnamed(..)` inside a function a command may wrap.

**Before you commit** (all three must be clean; fix what they find, do not skip)

```sh
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

**Commit and push**

- One commit a task. Message: a summary line, a blank line, bullets. Last line:
  `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>` (or your own model's line if you
  were given one).
- `git push origin feature/studio`. The draft pull request #356 runs CI. Wait for it:
  `gh pr checks 356`. *Build, lint, test* must pass; if it fails, read the log
  (`gh api --allow-escape-sequences repos/Shadowrom2020/rustpg2d/actions/jobs/<JOB>/logs`), fix,
  push again.

**Notes you keep true** (part of every task, in the same commit)

- `ROADMAP.md`: the part the task belongs to gets a line under **Progress**, and its test names
  and what you saw in the window under **Verify**. In part 12's **parity table**, change the
  task's row to `there` with the date.
- `CHANGELOG.md`, section *Unreleased › Editor*: one entry in words a creator understands.
- `CLAUDE.md`: a new script word goes into the long script paragraph of *The Studio branch*; a
  new file goes into the workspace map.
- This file: tick the task (`- [x]`) and add one line under it: what you did not do, if anything.

**Your report** says, in this order: what works now; what you tested and how (test names, what
you did in the window); what you did **not** test; anything you found that is wrong but outside
the task. Do not write "should work". If you did not run it, say so.

**When to stop and ask instead of going on**

- The task needs something this file marks *ask first*.
- A check fails and you do not understand why after two honest tries.
- You would have to delete or rewrite code the task doesn't name.

---

## 2. How this code base does things

### Running the editor for a test

There is no display server to click in. The editor is driven by a script in an environment
variable; it prints lines you read, and `shot` writes a PNG you look at.

```sh
S=<your scratch folder>          # never the repository, never the home folder
rm -rf $S/game && rsync -a --exclude saves example_project/ $S/game/
cd $S && RUSTPG2D_EDITOR_CONFIG=$S/cfg RUSTPG2D_LIBRARY=$S/lib RUSTPG2D_SETTINGS_DIR=$S/settings \
  HOME=$S/home RUSTPG2D_REPO=0 RUSTPG2D_FOLDER_PROJECTS=1 RUSTPG2D_MUTE=1 \
  RUSTPG2D_STUDIO_SCRIPT="wait 2; tab def:item:potion; wait 0.5; status; shot $S/a.png; quit" \
  timeout 240 /home/shadowrom/Projects/rust2d/target/debug/rustpg2d-studio game
```

- Build first (`cargo build -p rustpg2d-studio`): the script runs the binary, not your sources.
- Script words are in `crates/studio/src/main.rs` (`fn run_script`). A line that is no script
  word is a command of the editor (`map.open village`, `edit.undo`, `file.save_all`).
- `click X Y`, `click X Y right`, `hover X Y`, `drag X1 Y1 X2 Y2`, `wheel DY`, `type TEXT`,
  `press Enter` are real input. Find X and Y in a screenshot (crop it with PIL when small).
- The shell is zsh: a glob that matches nothing (`rm -rf $S/x*`) aborts the whole line.
  `sleep` is blocked; wait with `python3 -c 'import time; time.sleep(5)'`.
- An empty project to test on: script `project empty $S/p1` (or `project new $S/p1` for one with
  a hero and grass; that needs a library with the starter set).

### Adding something to the editor: the usual four places

1. **Logic** in `crates/editor-core/src/<area>.rs` as `impl Workspace { pub fn … }`, returning
   `Result<String, String>` (what was done, or why not, in a sentence). A unit test beside it on a
   project made with `crate::files::create_project(&dir)` in `std::env::temp_dir()`.
   A test that writes the editor's settings calls `crate::files::config_dir_here(Some(dir))` first.
2. **UI** in `crates/studio/src/<name>.rs`: a struct with `show(&mut self, ui, ws, …)`,
   `status(&self, ws) -> String` (one line for scripts) and `word(&mut self, ws, args) ->
   Result<(), String>` (the script's words). Copy the shape of `pluginspage.rs` (a block on a
   settings page), `itemshop.rs` (a block at the top of a definition's tab) or `cleanupwin.rs`
   (a window).
3. **Wiring** in `crates/studio/src/shell.rs`: a field of `Shell`, the same field borrowed in
   `Viewer` (two places: the struct and where `Viewer { … }` is built in `Shell::ui`), the call
   where it is shown (`Viewer::definition` for a definition's tab, `Viewer::settings` for a
   settings page, `impl TabViewer for Viewer` for a tab of its own), and a `*_word` function.
   What a tab's UI wants from the window goes through the `Ask` enum (handled in `Shell::ui`).
4. **Script word** in `crates/studio/src/main.rs`: `mod <name>;` at the top, and one line in
   `run_script`: `"word" => println!("[word] {}", self.shell.word_word(&cmd[1..])),`.

A **new tab** also needs: a variant of `Tab`, its `title` and `name`, `tab_named`, the list in
`show_tab` (`find_tab_from`), and `help_topic` (which manual chapter F1 opens).

### UI tests

`egui_kittest`, in a `#[cfg(test)] mod ui_tests` at the end of the UI file. The harness draws
your `show` and you click by label:

```rust
let mut h = Harness::builder().with_size(egui::vec2(1000.0, 700.0)).build_ui_state(|ui, s: &mut State| s.page.show(ui, &mut s.ws), state);
h.run();
h.get_by_label("Add calendar").click();
h.run_steps(3);
assert_eq!(h.state().ws.plugins_status(), "…");
```

- `Workspace::new(dir, editor_core::Gpu::No)`: no graphics device. Anything that needs loaded
  models or textures does nothing there; test that part in the window and say so.
- A file dialog never opens in a test: `crate::pick::answer_next(path)` says what the next one
  answers.
- `get_by_label` panics when two things have the label. Use `query_all_by_label(..).next()` or
  give your control its own name.

### Things that already exist (use them, do not write them again)

| You need | It is |
|---|---|
| A sentence for an event, a condition, an action | `editor_core::events::trigger_summary`, `condition_summary`, `action_summary` |
| The script functions of the project, what a script calls | `rustpg2d_engine::script_refs`, `ws.refs` (`editor_core::refs::Refs`) |
| A picture of a definition, a sprite, a figure in a pose | `crate::thumbs::Thumbs` (`of`, `sprite`, `pose`), `crate::charshop::draw_figure` |
| A *Browse…* button | `crate::pick::browse(ui, Key::…, label, title, want, start)` |
| Going to a definition, a map, a place in a script | `Ask::Open(Tab::…)`, `Shell::go_to(&Target)` |
| A pannable, zoomable canvas with cards and lines | `crates/studio/src/worldview.rs` |
| A strip of time with marks, a playhead, play / pause | `crates/studio/src/timeline.rs` |
| What the project doesn't use | `editor_core::cleanup` (`ws.cleanup_find()`) |
| Whether anything is unsaved; saving everything | `Shell::unsaved()`, `Shell::run("file.save_all")` |

---

## 3. Decisions of 2026-11-28 (the maintainer's)

| Question | Decision | What follows |
|---|---|---|
| A minimap in a map's tab | **Dropped.** The World view shows every map. | Nothing to build. |
| Tiled and Aseprite import | **Dropped.** | Nothing to build. Their code leaves with the old editor (part 11); their sections leave the manual (task 14). `SheetDef.tags` stays. |
| The events graph | **Build it.** | Tasks 3 and 4. |
| The clean-up offered when a project closes | **Bring it back.** | Tasks 1 and 2. |
| A new file extension for packs | **No: `.r2dpack` is reused.** | Nothing to do. |

---

## 4. Tasks, in order

Sizes: **S** = under 150 lines, **M** = 150 to 400, **L** = more. Every task ends with the gate,
the notes and the commit of section 1.

### - [x] Task 1 (M): unsaved changes are asked about when the window closes

Done 2026-11-28. Also: `closewindow` (the window's close button, for scripts); the question is a modal (Escape = *Stay*); the project file is written on exit.

**Why.** Closing the editor's window with unsaved changes loses them without a word. The 0.35
editor asked. Task 2 needs the same question.

**Read first.** `crates/studio/src/main.rs` (`impl eframe::App for Studio`, the `"quit"` script
word), `crates/studio/src/shell.rs` (`unsaved`, `switch_project`, `close_project`, `save_scripts`),
`crates/studio/src/cleanupwin.rs` (how a small window is shown and answered).

**Do.**
1. In `shell.rs` add `pub enum Leaving { Quit, Close, Open(PathBuf), New(PathBuf, String, bool) }`
   and a field `pub leaving: Option<Leaving>`: what the creator asked for that needs the project
   to be left.
2. In `Studio::ui` (`main.rs`): when `ctx.input(|i| i.viewport().close_requested())` and
   `self.shell.unsaved()` and no script asked to quit, send
   `egui::ViewportCommand::CancelClose` and set `shell.leaving = Some(Leaving::Quit)`.
3. Draw a window (in `Shell::ui`, after the tabs) while `leaving` is set and something is
   unsaved: *"There are changes that aren't saved."* with three buttons: **Save and go on**
   (`self.run("file.save_all")`, then do what was asked), **Don't save** (do what was asked
   without saving), **Stay** (forget it). For `Quit`, "do what was asked" is: set a flag
   `quit_now` and send `ViewportCommand::Close`; with the flag set, step 2 lets the close through.
4. `switch_project`, `new_project` and `close_project` today refuse with *"there are unsaved
   changes: save first"*. Where they are called from the UI (`Ask::OpenProject`, `Ask::NewProject`,
   `Ask::CloseProject` in `Shell::ui`), set `leaving` instead when something is unsaved. Keep the
   functions' own refusal: the script's `project …` words rely on it.
5. *Don't save* must really drop the changes: for a project that is left, open the next one
   without saving (the functions refuse while `unsaved()`; add a parameter or a
   `discard_changes()` that reloads nothing and clears the dirty marks: look at what
   `Workspace::close_project` and `Shell::around_another` do, and choose the smallest change).
6. Script words: the existing `quit` must **never** ask (every scripted run ends with it; a
   question would hang the run until its timeout). Add `leave` to print `[leave] -|quit|close|open`
   and `leave save|discard|stay` to answer the window.

**Done when.** With unsaved changes: closing the window, *Close this project* and opening another
project each show the question; each answer does what it says. Without unsaved changes nothing
is asked.

**Check.** A UI test of the question's window alone (three answers). In the window, on a scratch
copy: `field item potion /value 99; project close; leave; leave stay; status` (still open,
`dirty=true`), then `project close`-by-click is not scriptable: use the *Close this project*
button's position from a screenshot, then `leave discard; status` (no project, and the scratch
copy's `defs/items.ron` still has the old value), and once more with `leave save` (the file has
the new value). Scripted `quit` with unsaved changes ends the run at once.

**Don't.** Don't ask on `quit` from a script. Don't save without being asked.

### - [x] Task 2 (S): the clean-up is offered when a project is left

Done 2026-11-28. Not done: the look through the project runs in the frame (about two seconds on the example, only when leaving), not on a thread.

**Why.** Decision of 2026-11-28. 0.35 did it (`crates/editor/src/cleanup.rs`, `offer_cleanup`).

**Needs.** Task 1.

**Read first.** `crates/editor-core/src/cleanup.rs` (all of it), `crates/studio/src/cleanupwin.rs`,
`crates/editor/src/cleanup.rs` lines 120 to 165 (`offer_cleanup`: the rule to copy).

**Do.**
1. In `editor-core/src/cleanup.rs` add `pub fn cleanup_offer(&mut self) -> Option<Cleanup>`: the
   unused things **that were not kept before**, all ticked; `None` when there are none. Unlike
   `cleanup_find` it must not save: it looks at the project as it is on disk (0.35 loaded the
   project again from its folder: `Project::load(&self.project.root)`, then
   `unused::find_in(&disk)`), because after *Don't save* the editor's own copy still holds what
   was dropped.
2. When the project is left (task 1's "do what was asked", for `Quit`, `Close`, `Open`, `New`)
   and `cleanup_offer()` gives something, show the clean-up window first, with a line on top:
   *"Before you go: this project has files nothing uses."* Its three ways out (*Delete the
   ticked*, *Keep all*, the window's close button) all go on with leaving. *Keep all* remembers
   them, so the question does not come back for the same files.
3. Offer it once a leaving: a flag like 0.35's `cleanup_passed`.
4. `cleanup` (the script word) prints `…:leaving` while the window is the one shown on leaving.

**Done when.** A project with an unused sound: *Close this project* shows the clean-up; *Keep
all* closes the project and the next close asks nothing; a project without unused things closes
without the window.

**Check.** Core test for `cleanup_offer` (kept things are not offered; nothing is saved by it).
In the window on a scratch copy: `defs.make sound file:<a .wav> Old bell; file.save_all`, close
by the button, `cleanup`, `cleanup keep`, `status`.

**Don't.** Don't offer it on a scripted `quit`. Don't delete anything without the creator's click.

### - [x] Task 3 (M): the events graph, its logic

Done 2026-11-28. `events_graph` returns a `Graph` (nodes, edges, a `Card` a node, where each function is) instead of a pair: the tab needs the cards. 0.35's own summaries were not kept: the event sheet's say more.

**Why.** Decision of 2026-11-28: build it. This task is the part without UI.

**Read first.** `crates/editor/src/graph.rs` lines 1 to 450: `Owner`, `NodeKey`, `EdgeKind`,
`Edge`, `FnInfo`, `script_info`, `owners`, `Facts`, `build`. That is the logic; it has no UI in
it except `EdgeKind::color`. Also `crates/editor-core/src/events.rs` (the summaries).

**Do.**
1. New file `crates/editor-core/src/graph.rs`, `pub mod graph;` in `lib.rs`. Copy lines 1 to 450
   of the old file and remove what is UI: the `egui` import, `EdgeKind::color` (the window picks
   colours), `NodeKey::column` stays (it is layout data, a number).
2. The old file has its own `action_summary` and `cond_summary` (lines 195 to 246). Look at what
   they print, then use `editor_core::events::action_summary` / `condition_summary` instead if
   they give the same information; they take the action as JSON (`serde_json::to_value(a)`) and
   a `Names` argument (see how `crates/studio/src/sheet.rs` calls them). Keep the old ones only
   if the graph needs a line the new ones can't give, and say which in a comment.
3. Add to `impl Workspace`: `pub fn events_graph(&self, map: &str, all_fns: bool, focus:
   Option<graph::Owner>) -> Result<(Vec<NodeKey>, Vec<Edge>), String>`: the map from the open
   documents (`self.doc`, `self.parked`) or read from its file, the functions from
   `graph::script_info` over the project's script files (`self.project.script_files()`), then
   `graph::build`.
4. A text form for scripts and tests: `pub fn dump(nodes: &[NodeKey], edges: &[Edge]) ->
   Vec<String>`, one line a node (`owner entity 12`, `trigger entity 12 #0`, `fn open_gate`,
   `map house`) and one an edge (`calls  trigger entity 12 #0 [row 1] -> fn open_gate`). The old
   `graph_dump` (line 825) shows a format; keep it close.
5. Tests: a small map made in the test (an entity with a trigger that runs a script function,
   sets a flag and teleports; a zone trigger whose condition reads the flag; a script file with
   the function, which emits an event another trigger listens for). Assert the five edge kinds
   appear once each, and that `focus` on the entity drops what it does not lead to.
   Also one test on `example_project`'s village: more than 20 nodes, no panic.

**Done when.** `cargo test -p rustpg2d-editor-core graph` passes and the dump of the village
reads sensibly (paste ten lines of it into your report).

**Don't.** No UI. No new data in map files: the graph is computed.

### - [x] Task 4 (L): the events graph, its tab

Done 2026-11-28. The script's `graph focus` takes `entity UID`, `zone UID` or `-` (not `map`: the map itself can't be selected on its tab). Extra words: `graph find`, `graph click NODE`, `graph view X Y ZOOM`.

**Needs.** Task 3.

**Read first.** `crates/studio/src/worldview.rs` (all: cards on a canvas, pan, zoom, lines between
cards: your model), `crates/editor/src/graph.rs` lines 452 to 566 (`GraphState`, `layout`: four
columns, each node under the one before it) and 585 to 783 (`events_ui`: what the old one showed),
`crates/studio/src/shell.rs` (how `Tab::World` is wired).

**Do.**
1. `Tab::Graph(String)` (the map's name): title `Events: village`, name `graph:village`.
   Opened by a button **Events graph** in a map's tab bar (`mapview.rs`: add `Ask::Graph` there
   and handle it in `Viewer::map`) and by a command `map.graph` for the map in front.
2. `crates/studio/src/graphview.rs`, `GraphView` kept in `Shell` by map name. Each frame: ask
   `ws.events_graph` again only when the project changed (cache by a key as `WorldView::refresh`
   does: `ws.timeline.status()`, `ws.is_dirty()`, `ws.scripts_rev`).
3. Draw: four columns (who, their events, script functions, maps). A node is a card:
   - who: the thing's name and kind (the map itself, a zone, a placed thing);
   - an event: *When …* as its heading, then its *If* and *Do* lines from the summaries, at most
     eight, then "… and N more";
   - a function: its name and the lines `FnInfo::lines` has;
   - a map: its name.
   Lines between cards in five colours, one a kind of edge, with a legend above the canvas
   (as the World view has for portal / transition / edge). An edge that starts at a row starts
   at that row's height.
4. Layout: port `layout` from the old file. Cards can be dragged; their places live for the
   session only (not saved).
5. Clicks: a *who* card selects it on its map and shows the map's tab (`Shell::go_to` with
   `Target::Entity` / `Target::Zone` / `Target::Map`); an event card does the same for its owner
   (its Inspector edits the event: **no editing inside the graph in this task**); a function
   card opens the script at the function (`Target::Script(path, at)`); a map card opens the map.
6. Above the canvas: **Everything on the map | Only what is selected** (`focus`), and **Functions:
   those the map reaches | all**. A search field that dims the cards that don't match.
7. Script word `graph` (its tab in front): prints `MAP nodes=N edges=N focus=-|…`; `graph dump`
   prints the lines of task 3; `graph focus entity UID|zone UID|map|-`; `graph functions all|map`.
8. F1 on the tab: `07-events.md`.

**Done when.** The village's graph shows; a click on the elder's card shows the village with the
elder selected; a click on a function opens its script at that function.

**Check.** A UI test with a small made-up map (cards by their labels, a click gives the right
`Target`). In the window: `map.open village; map.graph; wait 1; graph; shot`, look at the
picture: cards do not lie on top of each other, lines end at cards, texts are readable at the
default zoom. Say honestly how it looks; a graph that is a tangle is not done.

**Don't.** Don't edit events in the graph. Don't store positions in the project.

### - [x] Task 5 (S/M): the model workshop takes more clips

Done 2026-11-28. Not done: the list of models isn't filtered by whether their bones fit (that is read from the files on *Take them*, which says what it couldn't take).

**Why.** Parity row *3D models: partly*. The logic exists; the window lacks the buttons.

**Read first.** `crates/studio/src/modelshop.rs` (all), `crates/editor-core/src/models.rs`:
`use_model_file(id, clips, rel)` (with `clips = true`: another file of clips for model `id`),
`share_clips(target, source, anims, replace)` (the clips of another model with the same
skeleton), `add_shared_clips`.

**Do.**
1. In the model workshop, under the animations: **Add a file of clips…** (`pick::browse`, `.glb`;
   copy it into `models/` with `editor_core::files::import_asset`, then `use_model_file(id, true,
   &rel)`), and **Take clips from another model…**: a list of the project's other models, the
   animations to take (ticks, all by default), *Replace the ones it has* (off by default), then
   `share_clips`. Show what the function says.
2. Script words of `model`: `model clips FILE`, `model share SOURCE [ANIM…] [replace]`.
3. Both functions save the definitions at once (they are 0.35's). Leave that as it is and say so
   in the hover text (*"written at once, with the file"*), as `itemshop.rs` does.

**Done when.** A model without an `attack` clip gets one from another model of the same rig, and
the workshop's check no longer lists it as missing.

**Check.** UI test for the list and the ticks (the functions themselves need loaded models:
test those in the window). In the window on a scratch copy of the example: pick two characters'
models that share a rig (`model check` prints what each lacks), `model share …`, `model check`.

**Don't.** No joints dragged by hand in this task.

### - [ ] Task 6 (M/L): an animation is seen and played in its tab

**Why.** Parity row *Animation editor: open*. Today an animation is only its form.

**Read first.** `crates/editor/src/anim_editor.rs` (404 lines: `paint_preview`, `timeline`,
`paint_dummy` are what to bring over), `crates/engine/src/animations.rs` (`AnimationDef`, `Cel`,
`Cel::pose(t, frames)`, `Emitter::particles(t)`, `AnimationDef::length`, `moments`),
`crates/studio/src/artshop.rs` (`SheetStage`: how a sheet's frame is drawn from
`thumbs`), `crates/studio/src/timeline.rs` (a time strip).

**Do.**
1. `crates/studio/src/animshop.rs`, `AnimStage`, shown in `Viewer::definition` for
   `RefKind::Animation` (as `ItemStage` is for items).
2. Left: a stage (about 260 x 220) with a grey dummy figure standing on a line (port
   `paint_dummy`) and the animation drawn over it at the playhead: each cel's frame at its pose
   (place, size, turn, opacity from `Cel::pose`), each emitter's particles as small squares in
   their colour, a flash as a tint over the stage.
3. Under it: **Play**, **Pause**, **From the start**, *Loop* (on by default), and a strip of
   time as long as the animation with one row a kind (cels, emitters, flashes, sounds): a bar
   for what lasts, a tick for a moment; the playhead is dragged. Sounds are **not** played in
   this task: their ticks show their names.
4. Editing stays in the form below. The stage reads the definition each frame, so a change in
   the form shows at once.
5. Script word `anim` (the animation's tab in front): `ID length=S at=S playing|paused cels=N
   emitters=N flashes=N sounds=N`; `anim time SECS`, `anim play`, `anim pause`.

**Done when.** One of the example's animations (`slash`, `fire`, `ice`: tab `def:animation:slash`) plays
on the stage and looks like it does in the game.

**Check.** UI test without pictures (the strip's rows and the playhead: `anim time 0.2` moves
it). In the window: open an animation, `anim time` at three moments, a screenshot each; compare
with the same animation in the game (`RUSTPG2D_AUTOPLAY` and a `shot`, see CLAUDE.md,
*Verification*). Say what differs.

**Don't.** Don't write a second form for cels: the schema's form is the editor.

### - [ ] Task 7 (M): the test setup has a form

**Why.** Parity row *Play: partly*. A test setup (the hero's level and class, items, flags,
variables, quests, the hour, the party) can only be made by playing and pressing *Keep*.

**Read first.** `crates/editor-core/src/testsetup.rs` (`TestSetup`, `test_setup`,
`set_test_setup`, `write_test_save`), `crates/studio/src/play.rs` (the Play tab's bar and its
State panel: where the setup is shown today), `crates/editor/src/testplay.rs` (0.35's window).

**Do.**
1. A button **Test setup…** in the Play tab's bar opens a window with the setup's fields:
   level (a number), class (one of the project's classes or none), items (a list: an item
   picked, a count), flags (a list of names, offered from `ws.refs.all_flags`), variables (name
   and number, offered from `ws.refs.vars`), quests (quest, stage, completed), the hour (none =
   the project's start hour) and the day, the party (names of placed characters).
2. Every change calls `ws.set_test_setup(..)` (it is kept for the project in the editor's
   settings: not a step of the history). **Forget it** clears it.
3. Script word `testsetup`: prints `TestSetup::summary()`; `testsetup level N`, `testsetup class
   ID|-`, `testsetup item ID N`, `testsetup flag NAME on|off`, `testsetup var NAME N`,
   `testsetup quest ID STAGE [done]`, `testsetup hour H|-`, `testsetup clear`.

**Done when.** A setup made in the form (level 3, a potion, a flag) is what the game starts with:
`play village 20 20 setup` then `status` shows the level, the item and the flag in the `[game]`
line.

**Check.** UI test of the form (a value in, the same value out of `ws.test_setup()`; use
`config_dir_here`). In the window as above.

### - [ ] Task 8 (S): the pixel editor adds and duplicates frames

**Read first.** `crates/editor-core/src/pixels.rs` (`PixelDoc`), `crates/editor-core/src/frame_ops.rs`
(0.35's sheet operations: `duplicate`), `crates/studio/src/pixeledit.rs`, `crates/studio/src/artshop.rs`
(`SheetStage`: the frame picked).

**Do.** In `PixelDoc`: `add_frame()` (the picture grows by one frame at the end of the last row,
or by a row when the row is full; empty) and `duplicate_frame()` (the same, filled with the
frame being painted). Both are steps of the picture's own undo. Buttons **+ Frame** and
**Duplicate** in the pixel editor, words `pixels addframe` and `pixels dupframe`. Not for a
material's texture (`seamless`): its frames are an animation of the ground with their own rule.
A sheet wider or taller than 2048 px is refused with a sentence (egui can't show it).

**Check.** Core test (sizes before and after, the pixels of the copy). In the window: a sheet
with 4 frames becomes 5; saved; the sheet's tab shows 5.

### - [ ] Task 9 (M): a region of tiles is copied with its ground

**Read first.** `crates/editor-core/src/mapedit.rs`: `Clip`, `copy_objects`, `paste_objects`, and
`save_as_structure(rect, name, ground)` (how ground and blocked tiles of a rectangle are taken:
`StructureDef::capture` / `place` in `crates/engine/src/structure.rs`).

**Do.** When what is selected is a region of tiles (`Selection::Region`), *Copy* also takes the
region's ground and blocked tiles into the `Clip` (a `StructureDef` captured from the rectangle
is the simplest carrier); *Paste* puts them down with the things, as one step. Copying single
things stays as it is (no ground). `mapact copy` / `paste` are the script's words already.

**Check.** Core test: paint a 3 x 3 patch, select the region, copy, paste elsewhere, the ground
is there and undo takes it away in one step. In the window with `mapdrag` and `mapact`.

### - [ ] Task 10 (S): a character starts from another one

**Read first.** `crates/studio/src/charshop.rs` (*Change…* and the model cards),
`crates/editor/src/models.rs` (0.35's template chooser: what it copied: model, colours, pieces).

**Do.** In the character workshop: **Start from…**, a searched list of the project's other
characters with their pictures; picking one copies its look (model, pieces, colours) onto this
character as one step, nothing else. Word `char from ID`.

**Check.** UI test; in the window, a villager that looks like the guard after one click, and
one undo brings its own look back.

### - [ ] Task 11 (S): the Help tab opens where it was left

**Do.** `EditorSettings` gets `help_at: (String, String)` (the chapter's file, the heading's
anchor). The Help tab writes it when the chapter changes (not every frame), and a new editor
opens the tab there. Use `config_dir_here` in the test.

### - [ ] Task 12 (S): more problems can be mended from the list

**Read first.** `crates/editor-core/src/problems.rs` (what kinds of entries there are: run
`check.problems` on the example and on an empty project and read the texts).

**Do.** For two kinds of entries that are not broken references, offer the fix that is one
obvious step: *No player character is set* gets **Choose one…** (the project's characters; sets
`config.player`); *The start map '…' does not exist* gets **Choose one…** (the maps; sets
`config.start_map`). Nothing else.
If you find another entry with one obvious fix, name it in your report; don't build it.

### - [ ] Task 13 (ask first): old things leave (ROADMAP part 11)

**Not for a small model on its own.** It deletes engine code that reads old projects
(`migrate.rs`, `convert_tiles.rs`, `old_tiles.rs`, `srd_convert.rs`, `legacy.rs`, the upgrade
chains), writes one conversion 0.35 to 0.36, and removes `crates/editor`. A mistake there is a
project that no longer opens.

What a small model **can** do first, as its own task: write the list the maintainer must see
before anything is removed. Put it into ROADMAP part 11 under a new heading *For the
maintainer's word*: each of *pictures held in the hand without a model*, *what is left of
sprite-sheet figures and props*, *the top-down view*, *Meshy's 2D kinds* with: where it is in
the code (files, the fields), what in `example_project` and the starter library still uses it
(grep, with counts), and what a creator loses. No removal. (The Tiled and Aseprite imports are
decided already: dropped.)

### - [ ] Task 14 (L, in pieces): the manual describes this editor (ROADMAP part 12)

One chapter a session. The chapters are `Documentation/manual/NN-*.md`; the editor shows them
in its Help tab, and tests keep their links valid and their script table complete.

**For each chapter.**
1. Read the chapter. Mark every sentence that names a menu, a window, a tool or a key of the
   0.35 editor.
2. For each, find the same thing in the new editor **by running it** (section 2) or by reading
   `crates/studio/src`. Write what is there now, in the words the editor itself uses (its button
   texts, its tab names). Something the new editor doesn't have is removed from the chapter and
   listed in your report, not described.
3. Sections about *Import… Tiled* and *Aseprite* are removed (chapter 10 and wherever else
   `grep -n -i "tiled\|aseprite" Documentation/manual/*.md` finds them).
4. Don't change what describes the **game** (how fights work, what a spell does): only the
   editor's side.
5. Screenshots are a later task: leave the picture lines as they are.
6. `cargo test -p rustpg2d-editor-core manual` and `cargo test -p rustpg2d-studio helpview`
   must pass (links, and every character drawable: a new sign needs a line in
   `manual::drawable`, or better, don't use it).

Order: 02 (the editor), 03 (maps), 04 (objects and zones), 05 (the Database), 06 (characters),
07 (events), 10 (art and sound), 11 (the asset library), 12 (languages), 13 (AI agents), 14
(publishing), 18 (plugins), 20 (visual scripts), 01 (getting started) last. Chapters 08, 09,
15, 16, 17 and 19 are about the game and scripts: check only their mentions of the editor.

---

## 5. Not in this plan

- **Releasing** (0.35.0 final, 0.36.0): only when the maintainer asks.
- **The merge of `feature/studio` into `master`**: when the parity table has no open row, and
  the maintainer says so.
- **CI and packaging for the new editor**, `architecture.md`, the README's screenshots: part
  12, after the manual.
- **Anything Meshy** that spends credits.
