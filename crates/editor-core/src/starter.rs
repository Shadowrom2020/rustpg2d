//! The starter asset library: no longer built into the editor, but a file of its own
//! (`engine::starter_pack`, `rustpg2d-starter-library-X.Y.Z.r2dlib`, a download of every release).
//!
//! Where it comes from, in this order:
//! 1. `RUSTPG2D_STARTER_PACK=<file>` (tests; `installed` = only the installed pack, to test
//!    installing);
//! 2. repository mode: the folder `crates/editor/assets/starter_library` of the checkout;
//! 3. the pack installed in `<data>/rustpg2d/starter/` (the folder beside the asset library);
//! 4. a development build: the folder in the source tree it was built from, when it is there.
//!
//! *Assets › Starter asset library…* (the starter library window) downloads the pack of the editor's own
//! version from the GitHub release (checked against its `SHA256SUMS`) or installs one from a file.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use rustpg2d_engine::library::{self, LibraryFiles};
use rustpg2d_engine::project::Project;
use rustpg2d_engine::starter_pack::{self, StarterPack};

/// The folder the starter library is made from, in the source tree.
const SOURCE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../editor/assets/starter_library");
/// Its history beside it.
const SOURCE_HISTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../editor/assets/starter_history.ron");
/// The installed pack's file name (one pack: installing replaces it).
const INSTALLED: &str = "starter-library.r2dlib";
/// Where releases are downloaded from (`RUSTPG2D_STARTER_URL` replaces it, without the trailing
/// `/v<version>`… : the folder that holds `SHA256SUMS` and the pack).
const RELEASES: &str = "https://github.com/Shadowrom2020/rustpg2d/releases/download";

/// Where a starter library came from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// `RUSTPG2D_STARTER_PACK`.
    Env(PathBuf),
    /// The checkout's folder (repository mode, or a development build).
    Folder(PathBuf),
    /// The installed pack.
    Installed(PathBuf),
}

/// A loaded starter library.
pub struct Starter {
    pub pack: StarterPack,
    pub source: Source,
}

impl Starter {
    /// Its files as the library code takes them.
    pub fn files(&self) -> Vec<(&str, &[u8])> {
        self.pack.library_files()
    }

    /// "0.24.0 (installed)", for messages and `status`.
    pub fn describe(&self) -> String {
        let from = match &self.source {
            Source::Env(_) => "file",
            Source::Folder(_) => "source folder",
            Source::Installed(_) => "installed",
        };
        format!("{} ({from})", self.pack.version)
    }
}

/// The loaded starter library (`None` inside: looked for, none there).
static CURRENT: Mutex<Option<Option<Arc<Starter>>>> = Mutex::new(None);

/// The folder the installed pack lives in: `starter/` beside the asset library.
pub fn install_dir() -> PathBuf {
    let lib = library::library_dir();
    lib.parent().map_or_else(|| PathBuf::from("starter"), |p| p.join("starter"))
}

fn installed_path() -> PathBuf {
    install_dir().join(INSTALLED)
}

/// The pack at `path` if this editor can read it.
fn read_pack(path: &Path) -> Result<StarterPack> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let pack = StarterPack::from_bytes(&bytes).with_context(|| format!("reading {}", path.display()))?;
    pack.check_format()?;
    Ok(pack)
}

fn read_folder(dir: &Path, history: &Path) -> Result<StarterPack> {
    StarterPack::from_dir(dir, history, &format!("{}-dev", env!("CARGO_PKG_VERSION")))
}

fn find() -> Option<Starter> {
    let env = std::env::var_os("RUSTPG2D_STARTER_PACK").map(PathBuf::from);
    let only_installed = env.as_deref() == Some(Path::new("installed"));
    if let Some(p) = env.filter(|_| !only_installed) {
        return match read_pack(&p) {
            Ok(pack) => Some(Starter { pack, source: Source::Env(p) }),
            Err(e) => {
                eprintln!("[starter] RUSTPG2D_STARTER_PACK: {e:#}");
                None
            }
        };
    }
    if !only_installed {
        if let Some(root) = crate::repo_mode::root() {
            let dir = root.join(crate::repo_mode::STARTER);
            match read_folder(&dir, &root.join("crates/editor/assets/starter_history.ron")) {
                Ok(pack) => return Some(Starter { pack, source: Source::Folder(dir) }),
                Err(e) => eprintln!("[starter] {e:#}"),
            }
        }
    }
    let installed = installed_path();
    if installed.is_file() {
        match read_pack(&installed) {
            Ok(pack) => return Some(Starter { pack, source: Source::Installed(installed) }),
            Err(e) => eprintln!("[starter] the installed starter library can't be used: {e:#}"),
        }
    }
    let dir = Path::new(SOURCE_DIR);
    if !only_installed && dir.join("project.ron").is_file() {
        if let Ok(pack) = read_folder(dir, Path::new(SOURCE_HISTORY)) {
            return Some(Starter { pack, source: Source::Folder(dir.to_path_buf()) });
        }
    }
    None
}

/// The starter library, loaded once (`None`: not installed).
pub fn current() -> Option<Arc<Starter>> {
    let mut c = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    if c.is_none() {
        *c = Some(find().map(Arc::new));
    }
    c.clone().flatten()
}

/// Looks for the starter library again (after installing one).
pub fn reload() -> Option<Arc<Starter>> {
    *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    current()
}

/// Whether the starter library is the checkout's folder in repository mode (nothing to install).
pub fn is_repo_folder() -> bool {
    crate::repo_mode::root().is_some() && current().is_some_and(|s| matches!(s.source, Source::Folder(_)))
}

/// For `status`: `none`, or the version and where it came from.
pub fn status() -> String {
    current().map_or_else(|| "none".to_string(), |s| s.describe().replace(' ', ""))
}

/// Opens the asset library at `dir` with the starter library's assets (as `library::open_library`
/// does; none for the starter's own source folder, whose assets are edited there).
pub fn open_library(dir: &Path) -> Result<Project> {
    let starter = (!crate::repo_mode::is_starter_source(dir)).then(current).flatten();
    let files = starter.as_ref().map(|s| s.files());
    library::open_library(dir, files.as_deref().map(|f| f as &LibraryFiles))
}

/// Installs the starter library file `bytes` (checked first) as the editor's starter library.
/// Returns its version. (The asset library gets its new assets from [`apply_to_library`], on the
/// main thread.)
pub fn install(bytes: &[u8]) -> Result<String> {
    let pack = StarterPack::from_bytes(bytes)?;
    pack.check_format()?;
    if crate::repo_mode::root().is_some() {
        bail!("in repository mode the starter library is the checkout's folder: nothing to install");
    }
    let dir = install_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    // (Written beside and renamed: an interrupted install leaves the old pack.)
    let tmp = dir.join(format!("{INSTALLED}.part"));
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, installed_path()).context("installing the starter library")?;
    reload();
    Ok(pack.version)
}

/// After installing: the asset library gets the assets it doesn't have yet (a new library is
/// filled with all of them); changed ones wait for *Update asset library*.
pub fn apply_to_library() -> Result<()> {
    open_library(&library::library_dir()).map(|_| ())
}

/// The base address of the release downloads of `version`.
fn release_url(version: &str) -> String {
    match std::env::var("RUSTPG2D_STARTER_URL") {
        Ok(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => format!("{RELEASES}/v{version}"),
    }
}

/// A download in progress (on its own thread).
pub struct Download {
    progress: Arc<Mutex<(u64, u64)>>,
    result: std::sync::mpsc::Receiver<Result<String>>,
}

impl Download {
    /// Downloads and installs the starter library of this editor's version.
    pub fn start() -> Download {
        let progress = Arc::new(Mutex::new((0u64, 0u64)));
        let (tx, rx) = std::sync::mpsc::channel();
        let (v, p) = (env!("CARGO_PKG_VERSION").to_string(), progress.clone());
        std::thread::spawn(move || {
            let _ = tx.send(download(&v, &p).and_then(|bytes| install(&bytes)));
        });
        Download { progress, result: rx }
    }

    /// (bytes so far, total; 0 = not known yet).
    pub fn progress(&self) -> (u64, u64) {
        *self.progress.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The outcome once it is over.
    pub fn poll(&self) -> Option<Result<String>> {
        self.result.try_recv().ok()
    }
}

/// The starter library file of this editor's version, fetched from its release and checked
/// against the release's `SHA256SUMS` (nothing is installed: the bytes are a pack to look at).
pub fn fetch() -> Result<Vec<u8>> {
    download(env!("CARGO_PKG_VERSION"), &Mutex::new((0, 0)))
}

/// The starter library that was found (the checkout's folder, an installed file) as the bytes
/// of a starter library file. `None`: none was found.
pub fn as_file() -> Option<Result<Vec<u8>>> {
    current().map(|s| s.pack.to_bytes())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_connect(Some(std::time::Duration::from_secs(20))).http_status_as_error(false).build().into()
}

/// Fetches the starter library of `version` and checks it against the release's `SHA256SUMS`.
fn download(version: &str, progress: &Mutex<(u64, u64)>) -> Result<Vec<u8>> {
    let base = release_url(version);
    let name = starter_pack::file_name(version);
    let get = |url: &str| -> Result<ureq::http::Response<ureq::Body>> {
        let resp = agent().get(url).call().map_err(|e| anyhow::anyhow!("can't reach {url}: {e}"))?;
        match resp.status().as_u16() {
            200 => Ok(resp),
            404 => bail!("R(ust)PG2D {version} has no starter library download ({url} doesn't exist): load the file instead"),
            s => bail!("{url} answered {s}"),
        }
    };
    let sums = get(&format!("{base}/SHA256SUMS"))?.body_mut().with_config().limit(1 << 20).read_to_string().context("reading SHA256SUMS")?;
    let Some(want) = starter_pack::sum_for(&sums, &name) else { bail!("SHA256SUMS of R(ust)PG2D {version} lists no {name}") };
    let mut resp = get(&format!("{base}/{name}"))?;
    let total = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0u64);
    let mut reader = resp.body_mut().with_config().limit(1 << 30).reader();
    let mut bytes = Vec::with_capacity(total as usize);
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf).context("downloading the starter library")?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        *progress.lock().unwrap_or_else(|e| e.into_inner()) = (bytes.len() as u64, total);
    }
    let got = rustpg2d_engine::hash::hex(&rustpg2d_engine::hash::sha256(&bytes));
    if got != want {
        bail!("the downloaded starter library is damaged (its SHA-256 doesn't match the release's): try again");
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_development_build_finds_the_source_folder() {
        // (Tests run in the checkout: the folder is there, and it is a library.)
        let s = current().expect("the starter library");
        assert!(s.files().iter().any(|(p, _)| *p == "project.ron"));
        assert!(!s.pack.history.is_empty());
    }

    #[test]
    fn starter_history_covers_the_starter() {
        use rustpg2d_engine::starter_update::{starter_bundle, StarterHistory};
        let s = current().expect("the starter library (the source folder)");
        let b = starter_bundle(&s.files()).unwrap();
        let h = StarterHistory::parse(&s.pack.history).unwrap();
        let missing = h.missing(&b);
        assert!(missing.is_empty(), "crates/editor/assets/starter_history.ron lacks {missing:?}: run `cargo run -p rustpg2d-editor-core --example starter_history`");
    }

    #[test]
    fn every_file_of_the_starter_belongs_to_an_asset() {
        // An update writes an asset's own files: a file an asset uses without owning it
        // would never reach a library that takes the starter as a pack.
        use rustpg2d_engine::library::{all_assets, asset_files};
        use rustpg2d_engine::starter_update::{own_files, starter_bundle};
        let s = current().expect("the starter library (the source folder)");
        let b = starter_bundle(&s.files()).unwrap();
        let owned: std::collections::BTreeSet<String> = all_assets(&b.db).iter().flat_map(|a| own_files(&b.db, a)).collect();
        let lost: Vec<String> = asset_files(&b.db).into_iter().filter(|f| !owned.contains(f)).collect();
        assert!(lost.is_empty(), "files no asset owns (starter_update::own_files): {lost:?}");
        let absent: Vec<&String> = owned.iter().filter(|f| !b.files.contains_key(*f)).collect();
        assert!(absent.is_empty(), "the starter library lacks files its assets name: {absent:?}");
    }
}
