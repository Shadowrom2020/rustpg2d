# 14. Publishing

**File › Export game (standalone)…** saves the project and writes a game that runs without the
editor, for **Linux**, **Windows**, **macOS** and the **Web**. Tick the platforms in the dialog; every
editor exports for all four, also without the other systems.

Before writing anything, the export checks the project: the start map must exist, and missing files
or an unset player character are reported. An earlier export in the same place is replaced; other
folders are never touched.

## What you get

```
releases/<game-name>/                   Linux: the game, its data/ folder, README.txt, rustpg2d-LICENSE.txt
releases/<game-name>-windows/           Windows: <game-name>.exe, data\, README.txt, the license
releases/<game-name>-macos/             macOS: <Game Name>.app, README.txt, the license
releases/<game-name>-web/               Web: index.html and its files
releases/<game-name>-linux.zip          (optional, to share; also -windows, -macos, -web)
releases/<game-name>-x86_64.AppImage    (optional, Linux package)
releases/<game-name>.flatpak            (optional, Linux package)
```

Each bundle holds only the images and sounds the game uses, never the project's save games. The game
executable is the R(ust)PG2D game runner; it runs the data next to it and titles its window with the
game's name.

## Linux

Players unpack the folder (or the zip) and start the game. It runs on x86_64 with **glibc 2.34 or
newer** (Debian 12, Ubuntu 22.04, Fedora 35, RHEL 9 and later) and needs OpenGL, X11 or Wayland and
ALSA. Save games go to `~/.local/share/<game-name>/saves`.

Two optional packages, built from the bundle:

- **AppImage**: one file that runs on most distributions. Players make it executable
  (`chmod +x`) and start it; without FUSE, `--appimage-extract-and-run` works. The editor writes it
  itself; only the official AppImage runtime is downloaded once (checked against a pinned SHA-256).
- **Flatpak bundle**: a sandboxed single file. Players install it with
  `flatpak install --user <game>.flatpak` and start it from the app menu. Building it needs the
  `flatpak` command on your machine. Set the **app id** (default `org.rustpg2d.<game>`) and the
  **runtime** in the dialog; keep the app id the same for updates, or players' saves stay with the
  old app. The game gets a window, sound and devices (the GPU and gamepads), nothing else.

Both get a desktop entry and an **icon**: the player character, drawn large, or a PNG you pick.

## Windows

Players unzip the folder and double-click `<game-name>.exe` (Windows 10 / 11, 64-bit). Save games go
to `%APPDATA%\<game-name>\saves`. The game isn't code-signed, so Windows SmartScreen shows "Windows
protected your PC" the first time: *More info*, *Run anyway* (the README in the bundle says so).
Gamepads work through Windows' own support. Without OpenGL (a virtual machine without 3D acceleration)
the game says so in a message box.

## macOS

The bundle is an application, `<Game Name>.app`, for macOS 11 or newer on Apple Silicon and Intel. Its
icon is the one chosen under *Icon*, else the player character. Players unzip it, drag it into
*Applications* and open it. Save games go to `~/Library/Application Support/<game-name>/saves`.

The export signs the app ad hoc (it passes `codesign --verify`), but it isn't signed with an Apple
developer certificate nor notarized. So macOS may refuse the first start of a downloaded copy ("Apple
could not verify…"): the player opens *System Settings › Privacy & Security* and chooses *Open
Anyway* (on macOS 14 and older: right-click the app, *Open*), or runs
`xattr -dr com.apple.quarantine "/Applications/<Game Name>.app"`. The README in the bundle explains it.

## Web and itch.io

The web export writes `releases/<game-name>-web/` (`index.html`, the game runner as WebAssembly,
macroquad's loader, R(ust)PG2D's browser storage, and `data.pack` with all of the game's files) and,
with *.zip*, `<game-name>-web.zip` with those files at its root.

**On itch.io**: create a project with *Kind of project: HTML*, upload the zip, and tick *This file
will be played in the browser*. A window size of 1280 × 720 fits the game well.

To try a web export locally, serve the folder (browsers don't run WebAssembly from `file://` pages),
e.g. run `python3 -m http.server` in it and open http://localhost:8000.

In the browser:

- it runs in current Firefox, Chrome, Brave, Edge and Safari with WebGL: a loading screen, then the
  title screen;
- save games, settings and achievements stay in the player's browser (per site and game folder);
  clearing the site data deletes them;
- sound starts with the first click or key press (browsers block it before);
- gamepads work as on the desktop (moving, buttons, rebinding, the button pictures in hints); a
  browser only shows a pad to the page after one of its buttons was pressed there, so the first
  press wakes it up;
- the title screen has no *Exit* (players close the tab); the developer console and script reloading
  aren't available;
- the download is about 12 MB.

## From a source build

The release downloads ship every game runner. In a source build, build the runners first
(`cargo build --release --workspace`, plus the web and Windows runners described in the
[README](../../README.md#building-from-source)). The export picks `target/release/rustpg2d-game` and
warns when only a debug build exists. It refuses a runner built from another engine version than the
editor and names the command that rebuilds it, because such a game could fail to read the project.

## Your game, your license

Games made with R(ust)PG2D are licensed however you decide; R(ust)PG2D sets no terms for your maps,
art, scripts or story. The game executable is the R(ust)PG2D game runner, released under the MIT
License, so every export includes the engine's license as `rustpg2d-LICENSE.txt`. That's all the MIT
License asks for. The example project's art and music are MIT-licensed too, so you may use them in
your games.
