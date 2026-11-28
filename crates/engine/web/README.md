# Web game files

Written by *File › Export game › Web* (`release::export_web`):

- `index.html` — the page (`{title}` = the game's name): a canvas and a module script that loads
  the game.
- `rustpg2d-game.wasm` and `rustpg2d-game.js` — the game runner for browsers and its loader, not
  kept here: `cargo build --release --target wasm32-unknown-unknown -p rustpg2d-game`, then
  `cargo xtask-web` writes the pair into `target/web/` (wasm-bindgen's step).
- `data.pack` — the game's files.

The browser's services the game uses (storage for save games, the clock, the page's address,
gamepads, downloads, fetching `data.pack`) are Rust code in `crates/gfx/src/web.rs`; until 0.35
they were JavaScript files of this folder, loaded by macroquad's loader.
