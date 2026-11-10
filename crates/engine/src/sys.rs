//! The game's access to files and the clock, the same on every platform:
//!
//! - **Native**: `std::fs` and the system clock.
//! - **Web** (`wasm32`): there is no file system. The exported game's files are fetched before
//!   the game starts ([`web::mount`], from `data/files.ron`) and kept in memory; what the game
//!   writes (save games, settings, achievements, thumbnails) is kept in memory too and stored in
//!   the browser's `localStorage` (via `web/rustpg2d.js`), from where [`web::mount`] restores it
//!   next time. Paths are the same relative paths as natively (`data/maps/village.map`).
//!
//! The editor keeps using `std::fs` directly; this is for code the game runs.

use std::io;
use std::path::{Path, PathBuf};

/// Reads a whole file.
pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    imp::read(path)
}

/// Reads a whole file as UTF-8 text.
pub fn read_to_string(path: &Path) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Writes a file, creating its folder (natively) if needed.
pub fn write(path: &Path, data: &[u8]) -> io::Result<()> {
    imp::write(path, data)
}

/// Whether a file (or, natively, a folder) exists.
pub fn exists(path: &Path) -> bool {
    imp::exists(path)
}

/// The files directly in a folder (not its subfolders), in no particular order.
pub fn files_in(dir: &Path) -> io::Result<Vec<PathBuf>> {
    imp::files_in(dir)
}

/// Deletes a file.
pub fn remove_file(path: &Path) -> io::Result<()> {
    imp::remove_file(path)
}

/// When a file was last changed (seconds since 1970); 0 if unknown (always on the web).
pub fn modified_secs(path: &Path) -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        0
    }
}

/// Seconds since 1970 (save dates, achievement times); 0 if the clock is unavailable.
pub fn unix_now() -> u64 {
    imp::unix_now()
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::io;
    use std::path::{Path, PathBuf};

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    pub fn write(path: &Path, data: &[u8]) -> io::Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        // Whole file or nothing: written beside the file, then renamed over it, so a crash (or a
        // full disk) leaves at most a `<name>.tmp`, never a truncated save, map or defs file.
        let tmp = tmp_path(path);
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    /// `<file>.tmp` next to the file: the real extension stays inside the name, so listings of
    /// `*.ron` / `*.map` (the editor's file watcher, exports) never pick it up.
    fn tmp_path(path: &Path) -> PathBuf {
        let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        name.push(".tmp");
        path.with_file_name(name)
    }

    pub fn exists(path: &Path) -> bool {
        path.exists()
    }

    pub fn files_in(dir: &Path) -> io::Result<Vec<PathBuf>> {
        Ok(std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect())
    }

    pub fn remove_file(path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    pub fn unix_now() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::io;
    use std::path::{Path, PathBuf};

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        super::web::FILES.with(|f| f.borrow().get(&super::web::key(path)).cloned()).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.display().to_string()))
    }

    pub fn write(path: &Path, data: &[u8]) -> io::Result<()> {
        let k = super::web::key(path);
        super::web::store(&k, Some(data));
        super::web::FILES.with(|f| f.borrow_mut().insert(k, data.to_vec()));
        Ok(())
    }

    pub fn exists(path: &Path) -> bool {
        let k = super::web::key(path);
        let dir = format!("{k}/");
        super::web::FILES.with(|f| f.borrow().keys().any(|p| *p == k || p.starts_with(&dir)))
    }

    pub fn files_in(dir: &Path) -> io::Result<Vec<PathBuf>> {
        let prefix = format!("{}/", super::web::key(dir));
        Ok(super::web::FILES.with(|f| {
            f.borrow().keys().filter(|p| p.starts_with(&prefix) && !p[prefix.len()..].contains('/')).map(PathBuf::from).collect()
        }))
    }

    pub fn remove_file(path: &Path) -> io::Result<()> {
        let k = super::web::key(path);
        super::web::store(&k, None);
        super::web::FILES.with(|f| f.borrow_mut().remove(&k)).map(|_| ()).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.display().to_string()))
    }

    pub fn unix_now() -> u64 {
        // SAFETY: a plain function of the page's JS (web/rustpg2d.js).
        unsafe { super::web::rustpg2d_now() as u64 }
    }
}

/// The web game's files: fetched from the server, written ones kept in `localStorage`.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::Path;

    thread_local! {
        pub(super) static FILES: RefCell<BTreeMap<String, Vec<u8>>> = const { RefCell::new(BTreeMap::new()) };
    }

    // web/rustpg2d.js (a miniquad plugin).
    extern "C" {
        pub(super) fn rustpg2d_now() -> f64;
        fn rustpg2d_storage_set(key: *const u8, key_len: usize, value: *const u8, value_len: usize);
        fn rustpg2d_storage_remove(key: *const u8, key_len: usize);
        fn rustpg2d_storage_dump() -> usize;
        fn rustpg2d_storage_take(dest: *mut u8);
        fn rustpg2d_query_dump() -> usize;
        fn rustpg2d_language_dump() -> usize;
        fn rustpg2d_query_take(dest: *mut u8);
        fn rustpg2d_pad_buttons() -> u32;
        fn rustpg2d_pad_axis(i: u32) -> f32;
        fn rustpg2d_pad_id_dump() -> usize;
        fn rustpg2d_pad_id_take(dest: *mut u8);
    }

    /// macroquad's loader checks that a JS plugin's crate is there (web/rustpg2d.js, version 2).
    #[no_mangle]
    pub extern "C" fn rustpg2d_crate_version() -> u32 {
        2
    }

    /// The first connected gamepad (the browser's Gamepad API): its buttons in the standard
    /// layout as bits 0-15 (the order of `runtime::input::Pad::ALL`) and its left stick (x, y;
    /// y grows downwards).
    pub fn pad_state() -> (u32, f32, f32) {
        // SAFETY: plain values from the JS plugin.
        unsafe { (rustpg2d_pad_buttons(), rustpg2d_pad_axis(0), rustpg2d_pad_axis(1)) }
    }

    /// The first connected gamepad's name ("" = none).
    pub fn pad_id() -> String {
        // SAFETY: the JS copies `len` bytes into the buffer it was asked for.
        unsafe {
            let len = rustpg2d_pad_id_dump();
            let mut buf = vec![0u8; len];
            if len > 0 {
                rustpg2d_pad_id_take(buf.as_mut_ptr());
            }
            String::from_utf8_lossy(&buf).into_owned()
        }
    }

    /// A path as the key of the file table: components joined with `/`, without `.`.
    pub fn key(path: &Path) -> String {
        path.components().filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        }).collect::<Vec<_>>().join("/")
    }

    /// Stores (or with `None` removes) a written file in `localStorage` (base64).
    pub(super) fn store(key: &str, data: Option<&[u8]>) {
        // SAFETY: pointers into live buffers, lengths in bytes; the JS copies them.
        unsafe {
            match data {
                Some(d) => {
                    let v = super::base64(d);
                    rustpg2d_storage_set(key.as_ptr(), key.len(), v.as_ptr(), v.len());
                }
                None => rustpg2d_storage_remove(key.as_ptr(), key.len()),
            }
        }
    }

    /// Adds fetched files to the table, then the ones written in earlier visits.
    pub fn mount(files: Vec<(String, Vec<u8>)>) {
        FILES.with(|f| {
            let mut f = f.borrow_mut();
            for (k, v) in files {
                f.insert(k, v);
            }
            // SAFETY: the JS fills exactly `len` bytes.
            let dump = unsafe {
                let len = rustpg2d_storage_dump();
                let mut buf = vec![0u8; len];
                rustpg2d_storage_take(buf.as_mut_ptr());
                buf
            };
            for line in String::from_utf8_lossy(&dump).lines() {
                if let Some((k, v)) = line.split_once('\t') {
                    if let Some(bytes) = super::unbase64(v) {
                        f.insert(k.to_string(), bytes);
                    }
                }
            }
        });
    }

    /// The browser's language (`navigator.language`, e.g. "de-DE").
    pub fn browser_language() -> String {
        // SAFETY: as in `mount`.
        let raw = unsafe {
            let len = rustpg2d_language_dump();
            let mut buf = vec![0u8; len];
            rustpg2d_query_take(buf.as_mut_ptr());
            buf
        };
        String::from_utf8_lossy(&raw).into_owned()
    }

    /// The page's query string without `?`, URL-decoded (`?autoplay=…` drives automated runs).
    pub fn query() -> String {
        // SAFETY: as in `mount`.
        let raw = unsafe {
            let len = rustpg2d_query_dump();
            let mut buf = vec![0u8; len];
            rustpg2d_query_take(buf.as_mut_ptr());
            buf
        };
        String::from_utf8_lossy(&raw).into_owned()
    }


}

/// Standard base64 (the web game keeps binary files as text in `localStorage`); [`crate::hash`]'s.
pub fn base64(data: &[u8]) -> String {
    crate::hash::base64(data)
}

pub fn unbase64(s: &str) -> Option<Vec<u8>> {
    crate::hash::base64_decode(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_files() {
        let dir = std::env::temp_dir().join(format!("rustpg2d_sys_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("saves/quick.ron");
        write(&f, b"()").unwrap();
        assert!(exists(&f) && exists(&dir.join("saves")));
        assert_eq!(read_to_string(&f).unwrap(), "()");
        // Written whole or not at all: the temporary file is gone, overwriting keeps one file.
        write(&f, b"(x: 1)").unwrap();
        assert_eq!(read_to_string(&f).unwrap(), "(x: 1)");
        assert_eq!(files_in(&dir.join("saves")).unwrap(), vec![f.clone()]);
        assert!(files_in(&dir).unwrap().is_empty(), "files only, not folders");
        remove_file(&f).unwrap();
        assert!(!exists(&f));
        assert!(unix_now() > 1_700_000_000);
        for data in [&b""[..], b"a", b"ab", b"abc", b"\x00\xff\x10PNG\r\n"] {
            assert_eq!(unbase64(&base64(data)).as_deref(), Some(data));
        }
        assert_eq!(base64(b"Man"), "TWFu");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
