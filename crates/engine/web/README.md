# Web game files

Written next to `rustpg2d-game.wasm` by *File › Export game › Web* (`release::export_web`):

- `index.html` — the page (`{title}` = the game's name).
- `mq_js_bundle.js` — macroquad 0.4.16's JS loader (WebGL, input, audio, file loading), copied
  unchanged from the crate's `js/` folder. macroquad / miniquad are MIT OR Apache-2.0
  (Copyright (c) 2020 Fedor Logachev). Update it together with the macroquad version.
- `rustpg2d.js` — R(ust)PG2D's plugin (version 2): browser storage, clock, the page's query string
  and gamepads (the Gamepad API's first connected pad, standard layout) (`engine/src/sys.rs`).
