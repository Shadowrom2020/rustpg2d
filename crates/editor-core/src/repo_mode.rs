//! Repository mode: the editor started in a checkout of R(ust)PG2D (the working folder is the
//! repository's root) works on the repository itself. Without a project argument it opens
//! `example_project/` as a folder (not converted into a project file), and the asset library is
//! the starter library `crates/editor/assets/starter_library/`, so *Send to library*, the Asset
//! Library window and the agent server's `library_send` change the repository's files.
//! `RUSTPG2D_LIBRARY` still picks another library, `RUSTPG2D_REPO=0` turns the mode off.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The example project, relative to the repository's root.
pub const EXAMPLE: &str = "example_project";
/// The starter library, relative to the repository's root.
pub const STARTER: &str = "crates/editor/assets/starter_library";

/// The repository's root when the editor runs in repository mode.
pub fn root() -> Option<&'static Path> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        if std::env::var("RUSTPG2D_REPO").is_ok_and(|v| v == "0") {
            return None;
        }
        std::env::current_dir().ok().filter(|d| is_repository(d)).map(|d| d.canonicalize().unwrap_or(d))
    })
    .as_deref()
}

/// Whether `dir` is a checkout of R(ust)PG2D: the workspace with the example project and the
/// starter library.
pub fn is_repository(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file() && dir.join(EXAMPLE).join("project.ron").is_file() && dir.join(STARTER).join("project.ron").is_file()
}

/// Turns the mode on for this process and the ones it starts (the Asset Library window): the
/// starter library becomes the asset library unless `RUSTPG2D_LIBRARY` names one. Call before
/// anything opens the library (`window_conf` runs before `main`); later calls do nothing.
pub fn init() {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        let Some(root) = root() else { return };
        if std::env::var_os("RUSTPG2D_LIBRARY").is_none() {
            std::env::set_var("RUSTPG2D_LIBRARY", root.join(STARTER));
        }
        println!("[editor] repository mode: {} (example project and starter library of this checkout)", root.display());
    });
}

/// The example project to open when no project was given.
pub fn example() -> Option<PathBuf> {
    root().map(|r| r.join(EXAMPLE))
}

/// What the editor opens when it starts: the project named on its command line, else the
/// example of the checkout it was started in (repository mode), else nothing (the home screen).
/// Never an `example_project` that merely lies beside the program or in the current folder: the
/// Windows zip has one next to the editor, and opening it would convert the bundled example.
pub fn project_to_open(given: Option<PathBuf>) -> Option<PathBuf> {
    pick_project(given, root())
}

fn pick_project(given: Option<PathBuf>, root: Option<&Path>) -> Option<PathBuf> {
    given.or_else(|| root.map(|r| r.join(EXAMPLE)))
}

/// Whether `path` is the repository's example project (opened as a folder, as it is kept in git).
pub fn is_example(path: &Path) -> bool {
    root().is_some_and(|r| same(path, &r.join(EXAMPLE)))
}

/// Whether the asset library in use is the repository's starter library.
pub fn library_is_starter() -> bool {
    root().is_some_and(|r| same(&rustpg2d_engine::library::library_dir(), &r.join(STARTER)))
}

/// Whether `dir` is the editor's own starter library source: the folder the starter set is
/// built from never gets the (possibly older) embedded starter set applied to itself, which
/// would bring back assets deleted there.
pub fn is_starter_source(dir: &Path) -> bool {
    same(dir, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../editor/assets/starter_library")) || root().is_some_and(|r| same(dir, &r.join(STARTER)))
}

fn same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checkout_is_a_repository_and_its_starter_is_the_source() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(is_repository(&repo));
        assert!(!is_repository(&repo.join(EXAMPLE)));
        assert!(is_starter_source(&repo.join(STARTER)));
        assert!(!is_starter_source(&repo.join(EXAMPLE)));
    }

    /// Outside repository mode no project named is the home screen, even with an
    /// `example_project` in the current folder (the Windows zip's layout).
    #[test]
    fn without_a_project_only_a_checkout_opens_its_example() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-start-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(EXAMPLE)).unwrap();
        std::fs::write(dir.join(EXAMPLE).join("project.ron"), "()").unwrap();
        assert_eq!(pick_project(None, None), None);
        assert_eq!(pick_project(Some(dir.join("mine")), None), Some(dir.join("mine")));
        assert_eq!(pick_project(None, Some(&dir)), Some(dir.join(EXAMPLE)));
        assert_eq!(pick_project(Some(dir.join("mine")), Some(&dir)), Some(dir.join("mine")));
        // (The tests run in the crate's folder, which is no checkout's root.)
        assert_eq!(project_to_open(None), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
