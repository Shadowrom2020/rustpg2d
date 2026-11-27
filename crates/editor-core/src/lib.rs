//! The editor without its UI (ROADMAP.md, *0.36.0 — The Studio*, part 2): the workspace (a
//! project, its open maps, what changed), undo, the project file. The window (`crates/studio`),
//! the editor script and the agent server work through it.

pub mod doc;
pub mod art;
pub mod commands;
pub mod defs;
pub mod files;
pub mod frame_ops;
pub mod history;
pub mod library;
pub mod build;
pub mod events;
pub mod mapedit;
pub mod mapops;
pub mod mcp;
pub mod meta;
pub mod models;
pub mod problems;
pub mod project_file;
pub mod refs;
pub mod rename;
pub mod repo_mode;
pub mod schema;
pub mod scripts;
pub mod search;
pub mod starter;
pub mod testsetup;
pub mod timeline;
pub mod workspace;
pub mod world;

pub use workspace::{Gpu, Workspace};

/// The agent server's port unless another is chosen.
pub const DEFAULT_PORT: u16 = 7743;

/// Seconds since the program started (for "a moment ago": merged typing, status messages).
pub fn now() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}
