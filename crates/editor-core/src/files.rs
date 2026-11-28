//! Native file dialogs (non-blocking) and project/asset file helpers.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rustpg2d_engine::defs::Database;
use rustpg2d_engine::map::{Map, Zone, ZoneKind};
use rustpg2d_engine::project::{Project, ProjectConfig};

fn to_slash(p: &Path) -> String {
    p.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// Returns `file`'s path relative to the project root. Files outside the project are
/// copied into `<root>/<subdir>/` first (never overwriting a different file).
pub fn import_asset(root: &Path, file: &Path, subdir: &str) -> Result<String> {
    let root_c = root.canonicalize().context("project folder")?;
    let file_c = file.canonicalize().with_context(|| format!("reading {}", file.display()))?;
    if let Ok(rel) = file_c.strip_prefix(&root_c) {
        return Ok(to_slash(rel));
    }
    let dir = root_c.join(subdir);
    std::fs::create_dir_all(&dir)?;
    let name = file_c.file_name().context("file has no name")?;
    let stem = Path::new(name).file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let ext = Path::new(name).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let data = std::fs::read(&file_c)?;
    let mut dest = dir.join(name);
    let mut n = 2;
    while dest.exists() && std::fs::read(&dest)? != data {
        dest = dir.join(format!("{stem}_{n}{ext}"));
        n += 1;
    }
    if !dest.exists() {
        std::fs::write(&dest, &data)?;
    }
    Ok(to_slash(dest.strip_prefix(&root_c)?))
}

/// Creates an empty project (config, empty definitions, one map with a spawn point, a script file).
pub fn create_project(dir: &Path) -> Result<()> {
    if dir.join("project.ron").exists() {
        bail!("{} already contains a project", dir.display());
    }
    let has_files = std::fs::read_dir(dir).map(|mut it| it.any(|e| e.is_ok_and(|e| !e.file_name().to_string_lossy().starts_with('.')))).unwrap_or(false);
    if has_files {
        bail!("{} is not empty; choose (or create) an empty folder for the new project", dir.display());
    }
    std::fs::create_dir_all(dir)?;
    let name = dir.file_name().map_or("New project".into(), |n| n.to_string_lossy().into_owned());
    let project = Project {
        root: dir.to_path_buf(),
        config: ProjectConfig { inventory_size: 24, ..ProjectConfig::new(&name) },
        db: Database::default(),
    };
    project.save_config()?;
    project.save_defs()?;
    let mut map = Map::new("start", 40, 30);
    let uid = map.alloc_uid();
    map.zones.push(Zone { uid, name: "start".into(), x: 20, y: 15, w: 1, h: 1, kind: ZoneKind::Spawn, triggers: vec![], loot: None });
    project.save_map(&map)?;
    std::fs::create_dir_all(project.scripts_dir())?;
    std::fs::write(
        project.scripts_dir().join("main.rhai"),
        "// Functions here can be called with the 'Run script' action.\n\nfn hello() {\n    say(\"Hello!\");\n}\n",
    )?;
    for d in ["materials", "sprites", "sounds"] {
        std::fs::create_dir_all(dir.join(d))?;
    }
    Ok(())
}

/// Creates a project file at `path` (`.r2dproject` is added when missing) with an empty project
/// (see [`create_project`]) named after the file. Returns the file's path.
pub fn create_project_file(path: &Path) -> Result<PathBuf> {
    let ext = crate::project_file::EXT;
    let path = if path.extension().is_some_and(|e| e == ext) { path.to_path_buf() } else { path.with_file_name(format!("{}.{ext}", path.file_name().context("no file name")?.to_string_lossy())) };
    let mut pf = crate::project_file::ProjectFile::create(&path)?;
    create_project(&pf.work)?;
    // The working copy's folder name isn't the game's.
    let mut p = Project::load(&pf.work)?;
    p.config.name = path.file_stem().map_or("New project".into(), |n| n.to_string_lossy().into_owned());
    p.save_config()?;
    pf.sync_now()?;
    Ok(pf.path.clone())
}

// ---------------------------------------------------------------- recent projects

const RECENT_MAX: usize = 8;

thread_local! {
    /// The editor's settings folder for what runs on this thread (tests: each has its own).
    static CONFIG_HERE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// From here on, what runs on this thread keeps the editor's settings in `dir` (`None`: where
/// they usually are). For tests that write settings: each test runs on a thread of its own.
pub fn config_dir_here(dir: Option<PathBuf>) {
    CONFIG_HERE.with_borrow_mut(|c| *c = dir);
}

/// `$RUSTPG2D_EDITOR_CONFIG`, else `$XDG_CONFIG_HOME/rustpg2d`, else `~/.config/rustpg2d`
/// (Windows: `%APPDATA%\rustpg2d`).
pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(p) = CONFIG_HERE.with_borrow(|c| c.clone()) {
        return Some(p);
    }
    if let Some(p) = std::env::var_os("RUSTPG2D_EDITOR_CONFIG") {
        return Some(PathBuf::from(p));
    }
    // (This crate's own tests never read or write the settings of whoever runs them.)
    if cfg!(test) {
        return None;
    }
    let base = rustpg2d_engine::paths::config_base()?;
    let dir = base.join("rustpg2d");
    rustpg2d_engine::legacy::adopt(&dir.join("recent.ron"), &base.join(rustpg2d_engine::legacy::OLD_NAME).join("recent.ron"));
    Some(dir)
}

/// The editor's own settings (`<config dir>/editor.ron`): the AI agent server's choices and the
/// Meshy art settings (the API key is kept apart, in `meshy.key`).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct EditorSettings {
    /// Start the agent server when the editor starts.
    pub mcp_autostart: bool,
    pub mcp_port: u16,
    /// Meshy model for Generate art.
    pub meshy_model: String,
    /// Credits a session may spend on art.
    pub meshy_budget: u32,
    /// The Art window asks before each spend.
    pub meshy_ask: bool,
    /// Test-play from here: each project's test setup (by its project file, else its folder).
    pub test_setups: std::collections::BTreeMap<String, crate::testsetup::TestSetup>,
    /// Clean up unused files: per project (its file, else its folder) the entries the creator
    /// kept (`model:ID`, `file:PATH`), not offered again when the project closes.
    pub cleanup_kept: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    /// The sources of packs the creator added: web addresses of an `index.ron`, or folders.
    pub pack_sources: Vec<String>,
    /// *Share…*: who shares, under which licence, with which credit line and into which
    /// folder, as entered last.
    pub share_author: String,
    pub share_licence: String,
    pub share_attribution: String,
    pub share_folder: String,
    /// The version of the editor whose news were looked at (or put away) last: "" = never.
    pub seen_version: String,
    /// Where the Help tab was left: the chapter's file and the heading's anchor ("" = never).
    pub help_at: (String, String),
}

impl Default for EditorSettings {
    fn default() -> Self {
        EditorSettings { mcp_autostart: false, mcp_port: crate::DEFAULT_PORT, meshy_model: "nano-banana-2-lite".into(), meshy_budget: 60, meshy_ask: true, test_setups: Default::default(), cleanup_kept: Default::default(), pack_sources: Vec::new(), share_author: String::new(), share_licence: String::new(), share_attribution: String::new(), share_folder: String::new(), seen_version: String::new(), help_at: Default::default() }
    }
}

/// Whether an editor kept settings on this computer before (else this is its first start).
pub fn settings_exist() -> bool {
    config_dir().is_some_and(|d| d.join("editor.ron").is_file())
}

pub fn load_settings() -> EditorSettings {
    config_dir().and_then(|d| std::fs::read_to_string(d.join("editor.ron")).ok()).and_then(|s| ron::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_settings(s: &EditorSettings) {
    let Some(dir) = config_dir() else { return };
    match rustpg2d_engine::project::ron_text(s) {
        Ok(text) => {
            if let Err(e) = rustpg2d_engine::sys::write(&dir.join("editor.ron"), text.as_bytes()) {
                eprintln!("[editor] cannot save the editor settings: {e}");
            }
        }
        Err(e) => eprintln!("[editor] cannot serialize the editor settings: {e}"),
    }
}

/// Copies `path` to `<name>.bak` beside it, once (an existing backup is kept): before the editor
/// saves a file it could only read in part. The backup's path, or `None` when copying failed.
pub fn back_up_once(path: &Path) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(".bak");
    let bak = path.with_file_name(name);
    if bak.exists() {
        return Some(bak);
    }
    match std::fs::copy(path, &bak) {
        Ok(_) => Some(bak),
        Err(e) => {
            eprintln!("[editor] cannot back up {}: {e}", path.display());
            None
        }
    }
}

/// Recently opened projects that still exist, newest first.
pub fn recent_projects() -> Vec<PathBuf> {
    let Some(file) = config_dir().map(|d| d.join("recent.ron")) else { return Vec::new() };
    let list: Vec<PathBuf> = std::fs::read_to_string(file).ok().and_then(|s| ron::from_str(&s).ok()).unwrap_or_default();
    list.into_iter().filter(|p| p.join("project.ron").exists() || crate::project_file::is_project_file(p)).collect()
}

/// Takes a project off the recent list (a folder that was converted into a project file).
pub fn forget_recent(path: &Path) {
    let Some(dir) = config_dir() else { return };
    let path = crate::project_file::canonical(path);
    let mut list = recent_projects();
    let before = list.len();
    list.retain(|p| *p != path);
    if list.len() != before {
        if let Ok(s) = rustpg2d_engine::project::ron_text(&list) {
            let _ = rustpg2d_engine::sys::write(&dir.join("recent.ron"), s.as_bytes());
        }
    }
}

/// Puts a project at the top of the recent list.
pub fn remember_recent(root: &Path) {
    let Some(dir) = config_dir() else { return };
    let root = crate::project_file::canonical(root);
    let mut list = recent_projects();
    list.retain(|p| *p != root);
    list.insert(0, root);
    list.truncate(RECENT_MAX);
    if let Ok(s) = rustpg2d_engine::project::ron_text(&list) {
        if let Err(e) = rustpg2d_engine::sys::write(&dir.join("recent.ron"), s.as_bytes()) {
            eprintln!("[editor] cannot save the recent projects list: {e}");
        }
    }
}

/// The example project shipped with a packaged R(ust)PG2D: AppImage / Flatpak
/// `<prefix>/share/rustpg2d/example_project` (the editor being
/// `<prefix>/lib/rustpg2d/rustpg2d-editor`), the Windows zip `example_project` next to the editor,
/// the macOS app `Contents/Resources/example_project`.
pub fn bundled_example() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let packaged = dir.parent().and_then(|p| p.parent()).map(|p| p.join("share").join("rustpg2d").join("example_project"));
    let mac_app = dir.join("..").join("Resources").join("example_project");
    [Some(dir.join("example_project")), packaged, Some(mac_app)].into_iter().flatten().find(|p| p.join("project.ron").is_file())
}

/// The user's own copy of the bundled example: the project file
/// `~/rustpg2d/example_project.r2dproject` (Windows: `%USERPROFILE%\rustpg2d\…`), packed from
/// the bundled folder on first use. A folder copy made by an older R(ust)PG2D (maybe edited) is
/// returned instead: opening it converts it into that same project file.
pub fn example_copy() -> Result<PathBuf> {
    let src = bundled_example().context("This R(ust)PG2D has no bundled example game: only the downloaded editor carries one (built from source, open example_project in the checkout instead).")?;
    let home = rustpg2d_engine::paths::home().context("no home folder (HOME / USERPROFILE is not set)")?;
    copy_example(&src, &home)
}

/// [`example_copy`] of the example in `src` for the home folder `home`.
fn copy_example(src: &Path, home: &Path) -> Result<PathBuf> {
    let folder = home.join("rustpg2d").join("example_project");
    let file = crate::project_file::converted_name(&folder);
    if crate::project_file::is_project_file(&file) {
        return Ok(file);
    }
    if folder.join("project.ron").exists() {
        return Ok(folder);
    }
    // A copy made before the rename (rust2d) is opened where it is: it may hold the user's edits.
    let old = home.join(rustpg2d_engine::legacy::OLD_NAME).join("example_project");
    if old.join("project.ron").exists() {
        return Ok(old);
    }
    std::fs::create_dir_all(home.join("rustpg2d"))?;
    rustpg2d_engine::pack::pack_dir(src, &file).with_context(|| format!("copying the example project to {}", file.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rustpg2d-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn import_copies_outside_files_without_clobbering() {
        let root = temp_dir("import-root");
        let outside = temp_dir("import-src");
        std::fs::write(outside.join("grass.png"), b"one").unwrap();
        assert_eq!(import_asset(&root, &outside.join("grass.png"), "tilesets").unwrap(), "tilesets/grass.png");
        // Same content again: reused.
        assert_eq!(import_asset(&root, &outside.join("grass.png"), "tilesets").unwrap(), "tilesets/grass.png");
        // Different file, same name: copied under a new name.
        std::fs::write(outside.join("grass.png"), b"two").unwrap();
        assert_eq!(import_asset(&root, &outside.join("grass.png"), "tilesets").unwrap(), "tilesets/grass_2.png");
        // Files already inside the project are referenced in place.
        std::fs::create_dir_all(root.join("art/x")).unwrap();
        std::fs::write(root.join("art/x/hero.png"), b"h").unwrap();
        assert_eq!(import_asset(&root, &root.join("art/x/hero.png"), "sprites").unwrap(), "art/x/hero.png");
        assert_eq!(std::fs::read(root.join("tilesets/grass.png")).unwrap(), b"one");
    }

    #[test]
    fn new_project_loads() {
        let dir = temp_dir("newproj");
        create_project(&dir).unwrap();
        let p = Project::load(&dir).unwrap();
        assert_eq!(p.list_maps(), vec!["start".to_string()]);
        assert!(p.load_map("start").unwrap().spawn_point("start").is_some());
        assert!(create_project(&dir).is_err(), "must not overwrite an existing project");
    }

    /// The home screen's *The example game* (script `project example`): the bundled example
    /// packed once into `~/rustpg2d`, the same file again after that; a build without a bundled
    /// example says so.
    #[test]
    fn the_example_is_copied_into_the_home_folder_once() {
        let src = temp_dir("example-src");
        create_project(&src).unwrap();
        let home = temp_dir("example-home");
        let copy = copy_example(&src, &home).unwrap();
        assert_eq!(copy, home.join("rustpg2d").join("example_project.r2dproject"));
        assert!(crate::project_file::is_project_file(&copy));
        let written = std::fs::metadata(&copy).unwrap().modified().unwrap();
        assert_eq!(copy_example(&src, &home).unwrap(), copy);
        assert_eq!(std::fs::metadata(&copy).unwrap().modified().unwrap(), written, "the copy is not written again");
        // (The tests' program has no example beside it.)
        assert!(bundled_example().is_none());
        assert!(example_copy().unwrap_err().to_string().contains("no bundled example game"));
    }
}
