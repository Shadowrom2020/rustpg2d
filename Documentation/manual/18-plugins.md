# Plugins

A **plugin** is an add-on for a game: its own scripts, [script hooks](08-scripting.md#script-hooks),
menu entries, HUD elements, settings and assets, kept in a folder of its own. Unlike assets from
the [asset library](11-asset-library.md), which are copied into the project and become its own,
a plugin stays apart: switch it off and it is gone from the game, add a newer version and it
replaces the old one, keeping your settings.

## Using plugins

*Assets › Plugins…* opens the **Plugins** window. It lists the project's plugins in the order the
game loads them.

- **Add from file…** adds a `.r2dplugin` file someone shared. **Add from library** lists the
  plugins of your asset library (the starter library brings some). A plugin the project already
  has is replaced when the new one has the same or a newer version; an older one is refused
  (remove the plugin first to go back).
- The **checkbox** switches a plugin on and off. Plugins that are off stay in the project but the
  game doesn't load them.
- The **arrows** change the load order: plugins load after the project's own scripts, in this
  order; their hooks run by priority like the project's.
- **Parameters**: the plugin's settings, each with a field that fits it (a number, a text, on /
  off, a sprite sheet, a sound, an item, a map, a colour). *default* goes back to the plugin's
  own value.
- **Export…** writes a `.r2dplugin` to share; **Send to library** copies it into your asset
  library, for your other projects; **Remove** deletes its folder from the project.

Every change is saved at once. *Edit › Problems* lists plugins whose folder is missing, that
need a newer R(ust)PG2D (their **script API** is newer than this version's: the game leaves them
out), and hooks or menu entries that run a function nobody has.

[Exported games](14-publishing.md) carry the plugins that are on.

## Making a plugin

A plugin is the folder `plugins/<id>/` of a project (the id: letters, digits, `_`, `-`):

```
plugins/hard_mode/
  plugin.ron            what it is and brings
  scripts/hard_mode.rhai its functions
  defs/sounds.ron       its own definitions (optional): sheets, sounds, items, …
  sounds/roar.ogg       their files, relative to the plugin's folder
```

`plugin.ron`:

```ron
(
    name: "Hard mode",
    version: "1.0",
    author: "Your name",
    description: "Enemies hit harder.",
    api: "0.27",
    parameters: [
        (name: "times", label: "Damage ×", kind: Number, default: Some(2.0), min: 1.0, max: 5.0,
         help: "How much harder enemies hit"),
    ],
    hooks: [(hook: "damage", function: "hard_mode_damage", priority: 10)],
    pause_entries: [Custom(label: "Hard mode", function: "hard_mode_info")],
    hud: {"badge": (kind: Text(text: "Hard mode"), anchor: TopRight, x: 10.0, y: 40.0)},
)
```

| Field | What it is |
|---|---|
| `name`, `version`, `author`, `description` | Shown in the Plugins window. `version` is dotted numbers (`1.2.0`): a higher one is an update. |
| `api` | The [script API](08-scripting.md) version the plugin needs, like `0.27`; empty = any. A plugin needing a newer API than the editor's and game's isn't loaded. |
| `parameters` | Settings the project fills in: `name` (what scripts ask for), `label`, `kind` (`Number`, `Text`, `Flag`, `Sprite`, `Sound`, `Item`, `Map`, `Color`), `default`, for numbers `min` / `max`, and a `help` sentence. |
| `hooks` | [Script hooks](08-scripting.md#script-hooks): hook, function, priority. |
| `title_entries`, `pause_entries` | [Menu entries](05-database.md#title-screen-and-menus) added to the title screen (before *Exit*) and the pause menu (before *Save and quit*), usually `Custom(label, function, detail)`. |
| `hud` | HUD elements shown while the plugin is on, by id, like the ones scripts set with `hud_text` / `hud_image` / `hud_bar` (`kind`: `Text(text)`, `Image(sheet, frame)`, `Bar(…)`; `anchor`, `x`, `y`: how far in from the anchor's edges, `size`, `color`). They aren't saved. |

**Scripts**: every `.rhai` file in the plugin's `scripts/` folder. All functions of the project and
its plugins share one name space, so start a plugin's function names with its id
(`hard_mode_damage`). A plugin reads its settings with `plugin_param("hard_mode", "times")`:

```rust
fn hard_mode_damage(m) {
    if m.target == "player" {
        m.amount *= plugin_param("hard_mode", "times");
    }
    m
}
```

**Definitions**: `defs/*.ron` in the plugin's folder, in the project's format, with files named
relative to the plugin's folder. The game adds the ones the project doesn't have; a definition the
project has with the same id stays the project's. The editor doesn't show them in the Database:
they belong to the plugin. Sprite sheets, sounds, materials and fonts find their files in the
plugin's folder; 3D models aren't supported in plugins yet.

**Not covered yet**: the plugin's texts aren't part of the project's translations.

## Sharing

**Export…** in the Plugins window writes `<id>.r2dplugin` (the `data.pack` format with the folder's
files under `<id>/`). *Send to library* puts the plugin into your asset library, where every
project can add it with *Add from library*. Raise `version` when you change a plugin, so projects
that have it can update.
