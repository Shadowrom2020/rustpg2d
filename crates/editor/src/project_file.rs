//! Projects in one file: `<name>.r2dproject`, in the format of an exported game's `data.pack`
//! (`rustpg2d_engine::pack`). The editor unpacks a project file into a working copy (a folder
//! under `<data>/rustpg2d/work`) and everything works on that folder as on a project folder; the
//! project file is written again whenever the working copy changed (checked every two seconds,
//! on a thread; after a save; before the project is closed or the editor quits), atomically.
//!
//! The working copy remembers which state of the project file it came from and what it looked
//! like when it was last packed (`.r2dwork`). Opening a project file reuses a working copy that
//! still belongs to it (fast for big projects, and work after the last pack - a crash - is
//! recovered); a working copy of an older state of the file is set aside, not deleted. If the
//! project file changed elsewhere while it was open, the editor writes its work to a separate
//! copy instead of overwriting it.
//!
//! Project folders are converted into project files when opened, unless
//! `RUSTPG2D_FOLDER_PROJECTS=1` (development: the repository's example is a folder).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use anyhow::{bail, Context, Result};
use rustpg2d_engine::pack;

/// A project file's extension.
pub const EXT: &str = "r2dproject";
/// The working copy's record (hidden: it never goes into the pack).
const RECORD: &str = ".r2dwork";
/// Seconds between checks of the working copy.
const CHECK_EVERY: f64 = 2.0;

/// Whether project folders open as they are (`RUSTPG2D_FOLDER_PROJECTS=1`), not converted.
pub fn folders_allowed() -> bool {
    std::env::var("RUSTPG2D_FOLDER_PROJECTS").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Whether `path` is a project file.
pub fn is_project_file(path: &Path) -> bool {
    path.is_file() && pack::is_pack_file(path)
}

/// The project file a folder is converted into: `<folder>.r2dproject` beside it.
pub fn converted_name(dir: &Path) -> PathBuf {
    let name = dir.file_name().map_or("project".into(), |n| n.to_string_lossy().into_owned());
    dir.with_file_name(format!("{name}.{EXT}"))
}

/// Where working copies live: `$RUSTPG2D_PROJECT_WORK`, else `<editor config>/work` when
/// `RUSTPG2D_EDITOR_CONFIG` is set (tests), else `<data>/rustpg2d/work`.
fn work_base() -> PathBuf {
    if let Some(p) = std::env::var_os("RUSTPG2D_PROJECT_WORK") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("RUSTPG2D_EDITOR_CONFIG") {
        return PathBuf::from(p).join("work");
    }
    rustpg2d_engine::paths::data_base().unwrap_or_else(std::env::temp_dir).join("rustpg2d").join("work")
}

/// The working copy of the project file `path`: named after it, plus a hash of its full path.
fn work_dir_for(path: &Path) -> PathBuf {
    let full = path.canonicalize().unwrap_or_else(|_| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    let hash = rustpg2d_engine::hash::hex(&rustpg2d_engine::hash::sha256(full.to_string_lossy().as_bytes())[..8]);
    let stem: String = path.file_stem().map_or("project".into(), |s| s.to_string_lossy().into_owned()).chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    work_base().join(format!("{stem}-{hash}"))
}

/// A file's size and modification time (nanoseconds since 1970); `(0, 0)` if it is missing.
fn file_meta(path: &Path) -> (u64, u64) {
    std::fs::metadata(path).map_or((0, 0), |m| (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64)))
}

/// A fingerprint of the working copy: its files' paths, sizes and modification times (FNV-1a,
/// stable across builds: it is stored in the record).
fn tree_stamp(dir: &Path) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for rel in pack::dir_files(dir).unwrap_or_default() {
        let (len, modified) = file_meta(&dir.join(&rel));
        eat(rel.as_bytes());
        eat(&len.to_le_bytes());
        eat(&modified.to_le_bytes());
    }
    h
}

/// What a working copy knows about its project file (`.r2dwork`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Record {
    /// The project file.
    file: PathBuf,
    /// Its size and modification time when it was unpacked or last written.
    file_len: u64,
    file_modified: u64,
    /// The working copy's [`tree_stamp`] at that moment.
    tree: u64,
    /// A pack written and about to replace `file` (written before the rename, which keeps size
    /// and time): if the editor stops right after the rename, this is what the file now holds.
    pending: Option<Box<Record>>,
}

impl Record {
    fn load(work: &Path) -> Option<Record> {
        ron::from_str(&std::fs::read_to_string(work.join(RECORD)).ok()?).ok()
    }

    fn save(&self, work: &Path) {
        match rustpg2d_engine::project::ron_text(self) {
            Ok(t) => {
                if let Err(e) = std::fs::write(work.join(RECORD), t) {
                    eprintln!("[editor] cannot write {}: {e}", work.join(RECORD).display());
                }
            }
            Err(e) => eprintln!("[editor] {e:#}"),
        }
    }

    /// Whether the project file is still the one this record saw.
    fn matches_file(&self, path: &Path) -> bool {
        (self.file_len, self.file_modified) == file_meta(path) && self.file_len > 0
    }

    /// The record that describes the project file `path` as it is now: this one, or its pending
    /// pack when that replaced the file.
    fn for_file(self, path: &Path) -> Option<Record> {
        if self.matches_file(path) {
            return Some(Record { pending: None, ..self });
        }
        self.pending.filter(|p| p.file == path && p.matches_file(path)).map(|p| *p)
    }
}

/// An open project file and its working copy.
pub struct ProjectFile {
    /// The project file.
    pub path: PathBuf,
    /// The working copy (the editor's `project.root`).
    pub work: PathBuf,
    record: Record,
    /// A pack being written on a thread: the record it leads to.
    job: Option<Receiver<Result<Record>>>,
    last_check: f64,
    /// Project files written since opening (for `status`).
    pub packs: u32,
    /// The last error writing the project file.
    pub error: Option<String>,
    /// A message for the status bar (opening, a conflict).
    pub note: Option<String>,
}

impl ProjectFile {
    /// Opens the project file `path`: its working copy, reused when it belongs to the file as it
    /// is now (with work done after the last pack: recovered), else unpacked afresh.
    pub fn open(path: &Path) -> Result<ProjectFile> {
        if !is_project_file(path) {
            bail!("{} is not an R(ust)PG2D project file", path.display());
        }
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let work = work_dir_for(&path);
        let mut note = None;
        let loaded = Record::load(&work);
        let current = loaded.clone().and_then(|r| r.for_file(&path)).filter(|_| work.join("project.ron").is_file());
        let record = match current {
            Some(r) => {
                if tree_stamp(&work) != r.tree {
                    note = Some(format!("Recovered changes made after {} was last written", path.display()));
                }
                r
            }
            None => {
                let old = loaded;
                if work.exists() {
                    // Work that never reached this project file (another state of it, or
                    // unpacked changes): set aside, not deleted.
                    let unpacked = old.as_ref().is_none_or(|r| tree_stamp(&work) != r.tree);
                    if unpacked {
                        let aside = work.with_file_name(format!("{}-set-aside-{}", work.file_name().unwrap_or_default().to_string_lossy(), now_secs()));
                        std::fs::rename(&work, &aside).with_context(|| format!("setting {} aside", work.display()))?;
                        note = Some(format!("The project file changed since it was last open here; the work there was set aside in {}", aside.display()));
                    } else {
                        std::fs::remove_dir_all(&work).with_context(|| format!("clearing {}", work.display()))?;
                    }
                }
                pack::unpack(&path, &work)?;
                if !work.join("project.ron").is_file() {
                    bail!("{} holds no project (project.ron is missing)", path.display());
                }
                let (file_len, file_modified) = file_meta(&path);
                let r = Record { file: path.clone(), file_len, file_modified, tree: tree_stamp(&work), pending: None };
                r.save(&work);
                r
            }
        };
        let mut pf = ProjectFile { path, work, record, job: None, last_check: f64::NEG_INFINITY, packs: 0, error: None, note };
        // Recovered work goes into the file at the next check.
        pf.last_check = f64::NEG_INFINITY;
        Ok(pf)
    }

    /// A new project file at `path` (must not exist yet) with an empty working copy; the caller
    /// creates the project in [`ProjectFile::work`] and then calls [`ProjectFile::sync_now`].
    pub fn create(path: &Path) -> Result<ProjectFile> {
        if path.exists() {
            bail!("{} already exists", path.display());
        }
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()).join(path.file_name().context("no file name")?);
        let work = work_dir_for(&path);
        if work.exists() {
            let aside = work.with_file_name(format!("{}-set-aside-{}", work.file_name().unwrap_or_default().to_string_lossy(), now_secs()));
            std::fs::rename(&work, &aside).with_context(|| format!("setting {} aside", work.display()))?;
        }
        std::fs::create_dir_all(&work).with_context(|| format!("creating {}", work.display()))?;
        let record = Record { file: path.clone(), ..Default::default() };
        Ok(ProjectFile { path, work, record, job: None, last_check: f64::NEG_INFINITY, packs: 0, error: None, note: None })
    }

    /// Converts the project folder `dir` into `<dir>.r2dproject` beside it (the folder stays as
    /// it is). An existing project file of that name is kept and returned.
    pub fn convert_folder(dir: &Path) -> Result<(PathBuf, bool)> {
        let dest = converted_name(dir);
        if dest.exists() {
            if !is_project_file(&dest) {
                bail!("{} exists and is not a project file", dest.display());
            }
            return Ok((dest, false));
        }
        pack::pack_dir(dir, &dest).with_context(|| format!("converting {}", dir.display()))?;
        Ok((dest, true))
    }

    /// Whether a pack is being written.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Whether the working copy has changes the project file doesn't have yet.
    pub fn changed(&self) -> bool {
        tree_stamp(&self.work) != self.record.tree || !self.path.exists()
    }

    /// `projectfile=` for the editor script's `status`: path, then `saved`, `writing`, `pending`
    /// or `error`, and how many times it was written.
    pub fn status(&self) -> String {
        let state = if self.error.is_some() {
            "error"
        } else if self.busy() {
            "writing"
        } else if self.changed() {
            "pending"
        } else {
            "saved"
        };
        format!("{}:{state}:{}", self.path.display(), self.packs)
    }

    /// Checks soon (after a save).
    pub fn check_soon(&mut self) {
        self.last_check = f64::NEG_INFINITY;
    }

    /// Where the work goes: the project file, or a separate copy if the file changed elsewhere
    /// since this editor last read or wrote it (from then on the editor works on that copy).
    fn target(&mut self) -> PathBuf {
        if self.record.file_len == 0 || !self.path.exists() || self.record.matches_file(&self.path) {
            return self.path.clone();
        }
        let stem = self.path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let mut n = 1;
        let copy = loop {
            let c = self.path.with_file_name(format!("{stem} (changed {n}).{EXT}"));
            if !c.exists() {
                break c;
            }
            n += 1;
        };
        self.note = Some(format!("{} was changed by something else while it was open: your work is saved as {}", self.path.display(), copy.display()));
        self.path = copy.clone();
        copy
    }

    /// Called every frame: takes a finished pack's result; every two seconds starts writing the
    /// project file on a thread when the working copy changed.
    pub fn tick(&mut self, now: f64) {
        self.collect(false);
        if self.job.is_some() || now - self.last_check < CHECK_EVERY {
            return;
        }
        self.last_check = now;
        if !self.changed() {
            return;
        }
        let (work, dest, record) = (self.work.clone(), self.target(), self.record.clone());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(write_pack(&work, &dest, record));
        });
        self.job = Some(rx);
    }

    /// Takes the result of a pack written on the thread (`wait`: waits for it).
    fn collect(&mut self, wait: bool) {
        let Some(rx) = &self.job else { return };
        let res = if wait {
            rx.recv().unwrap_or_else(|_| Err(anyhow::anyhow!("the packing thread stopped")))
        } else {
            match rx.try_recv() {
                Ok(r) => r,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(_) => Err(anyhow::anyhow!("the packing thread stopped")),
            }
        };
        self.job = None;
        self.finish(res);
    }

    fn finish(&mut self, res: Result<Record>) {
        match res {
            Ok(r) => {
                self.record = r;
                self.packs += 1;
                self.error = None;
            }
            Err(e) => {
                let msg = format!("Writing the project file failed: {e:#}");
                eprintln!("[editor] {msg}");
                self.error = Some(msg);
            }
        }
    }

    /// Writes the project file now if the working copy changed (waits for a pack in progress
    /// first): before closing the project or quitting.
    pub fn sync_now(&mut self) -> Result<()> {
        self.collect(true);
        if self.changed() {
            let dest = self.target();
            let res = write_pack(&self.work, &dest, self.record.clone());
            let failed = res.as_ref().err().map(|e| format!("{e:#}"));
            self.finish(res);
            if let Some(e) = failed {
                bail!(e);
            }
        }
        Ok(())
    }
}

/// Packs `work` into `dest` (through a temporary file that replaces it) and keeps the record in
/// `work` current at every step, so an editor stopped at any moment finds out what the file holds
/// (`current`: the record so far). Returns the new record.
fn write_pack(work: &Path, dest: &Path, current: Record) -> Result<Record> {
    use std::io::Write;
    let tree = tree_stamp(work);
    let bytes = pack::write(&pack::read_dir_files(work)?)?;
    let tmp = dest.with_file_name(format!(".{}.writing", dest.file_name().unwrap_or_default().to_string_lossy()));
    let mut f = std::fs::File::create(&tmp).with_context(|| format!("writing {}", tmp.display()))?;
    f.write_all(&bytes).and_then(|_| f.sync_all()).with_context(|| format!("writing {}", tmp.display()))?;
    drop(f);
    let (file_len, file_modified) = file_meta(&tmp);
    let done = Record { file: dest.to_path_buf(), file_len, file_modified, tree, pending: None };
    Record { pending: Some(Box::new(done.clone())), ..current }.save(work);
    std::fs::rename(&tmp, dest).with_context(|| format!("replacing {}", dest.display()))?;
    done.save(work);
    Ok(done)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rustpg2d_pf_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Working copies in a temporary folder, not the user's (one for all tests: the
    /// environment is shared by the tests running at the same time).
    fn work_in(_base: &Path) {
        std::env::set_var("RUSTPG2D_PROJECT_WORK", std::env::temp_dir().join(format!("rustpg2d_pf_work_{}", std::process::id())));
    }

    #[test]
    fn folders_convert_reopen_recover_and_never_overwrite_others_changes() {
        let base = scratch("cycle");
        work_in(&base);
        let folder = base.join("My Game");
        crate::files::create_project(&folder).unwrap();
        let (file, made) = ProjectFile::convert_folder(&folder).unwrap();
        assert!(made && file == base.join("My Game.r2dproject") && is_project_file(&file));
        assert!(folder.join("project.ron").is_file(), "the folder is left as it was");
        // Open: unpacked; nothing to write.
        let mut pf = ProjectFile::open(&file).unwrap();
        assert!(pf.work.join("maps/start.map").is_file() && !pf.changed() && pf.note.is_none());
        // A change in the working copy reaches the file.
        std::fs::write(pf.work.join("scripts/extra.rhai"), "fn x() {}").unwrap();
        assert!(pf.changed());
        pf.sync_now().unwrap();
        assert!(!pf.changed() && pf.packs == 1);
        let files = pack::read_all(&std::fs::read(&file).unwrap()).unwrap();
        assert!(files.iter().any(|(p, _)| p == "scripts/extra.rhai"));
        assert!(!files.iter().any(|(p, _)| p.starts_with('.')), "the record stays out");
        // Reopened: the same working copy; work after the last pack (a crash) is recovered.
        std::fs::write(pf.work.join("scripts/late.rhai"), "fn y() {}").unwrap();
        drop(pf);
        let mut pf = ProjectFile::open(&file).unwrap();
        assert!(pf.note.as_deref().is_some_and(|n| n.starts_with("Recovered")));
        assert!(pf.work.join("scripts/late.rhai").is_file() && pf.changed());
        pf.sync_now().unwrap();
        // The editor stopped right after replacing the file, before the record said so: the
        // pending entry describes the file, and the working copy is reused.
        let done = Record::load(&pf.work).unwrap();
        Record { file_len: 1, tree: 7, pending: Some(Box::new(done.clone())), ..done }.save(&pf.work);
        drop(pf);
        let mut pf = ProjectFile::open(&file).unwrap();
        assert!(pf.note.is_none() && !pf.changed() && pf.work.join("scripts/late.rhai").is_file());
        // The file changes elsewhere while open: the work goes to a copy instead.
        std::thread::sleep(std::time::Duration::from_millis(20));
        pack::pack_dir(&folder, &file).unwrap();
        std::fs::write(pf.work.join("scripts/mine.rhai"), "fn z() {}").unwrap();
        pf.sync_now().unwrap();
        let copy = base.join("My Game (changed 1).r2dproject");
        assert!(pf.path == copy && is_project_file(&copy));
        assert!(!pack::read_all(&std::fs::read(&file).unwrap()).unwrap().iter().any(|(p, _)| p == "scripts/mine.rhai"));
        // Opening the changed file again sets the old working copy aside and unpacks afresh.
        drop(pf);
        let pf = ProjectFile::open(&file).unwrap();
        assert!(!pf.work.join("scripts/extra.rhai").exists() && pf.note.is_none());
        // Converting again keeps the existing file.
        assert!(!ProjectFile::convert_folder(&folder).unwrap().1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn new_project_files_start_empty() {
        let base = scratch("new");
        work_in(&base);
        let file = base.join("sub/New.r2dproject");
        let mut pf = ProjectFile::create(&file).unwrap();
        crate::files::create_project(&pf.work).unwrap();
        pf.sync_now().unwrap();
        assert!(is_project_file(&file) && !pf.changed());
        assert!(ProjectFile::create(&file).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
