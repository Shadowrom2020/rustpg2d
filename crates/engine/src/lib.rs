//! R(ust)PG2D engine: tile maps, entities, events, scripting, rendering and the game runtime.

pub mod achievements;
pub mod audio;
pub mod autotile;
pub mod crafting;
pub mod defs;
pub mod effects;
pub mod entity;
pub mod events;
pub mod fog;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub mod glibc_compat;
pub mod imageio;
pub mod inventory;
pub mod lang;
pub mod legacy;
// Asset library, old-project migration and game export: editor features (zip), not in the web game.
#[cfg(not(target_arch = "wasm32"))]
pub mod library;
pub mod map;
#[cfg(not(target_arch = "wasm32"))]
pub mod migrate;
pub mod path;
pub mod progression;
pub mod project;
pub mod quest;
pub mod refactor;
pub mod release;
pub mod render;
pub mod runtime;
pub mod script;
pub mod script_refs;
pub mod settings;
pub mod skills;
pub mod state;
pub mod sys;
pub mod structure;

pub use macroquad;

/// Fingerprint of the engine sources this binary was built from. The editor compares it
/// with `rustpg2d-game --engine-build` before a play-test to catch a stale game binary.
pub const ENGINE_BUILD: &str = env!("RUSTPG2D_ENGINE_BUILD");
