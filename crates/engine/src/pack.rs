//! Game packs: an exported game's files in one file, `data.pack`, next to the program (in a macOS
//! app: its Resources), instead of a `data` folder. The game reads the index when it starts and a
//! file from its offset when it needs it (`sys` serves them where it would read `data/…`); the web
//! game downloads the pack whole.
//!
//! Layout: [`MAGIC`], the index's length (u64, little endian), the index (postcard: relative path,
//! offset from the start of the file data, length), then the files one after another. Index and
//! files are scrambled with a keystream that depends on the byte's position, so a text or image
//! editor shows noise. That keeps casual eyes out; it is not protection.

#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use anyhow::bail;
use anyhow::{Context, Result};

/// The pack's file name.
pub const PACK_FILE: &str = "data.pack";
/// Start of a pack (the version is part of it).
const MAGIC: &[u8; 8] = b"R2DPAK1\0";

/// The keystream's 8 bytes of block `block` (bytes `8 * block` to `8 * block + 7`).
fn key_block(block: u64, salt: u64) -> u64 {
    // SplitMix64 of the block number.
    let mut z = block.wrapping_add(salt).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Scrambles (or unscrambles: the same operation) `buf`, which starts at position `start`
/// (positions count from the start of the index / the data).
fn scramble(buf: &mut [u8], start: u64, salt: u64) {
    let mut i = 0;
    while i < buf.len() {
        let pos = start + i as u64;
        let key = key_block(pos / 8, salt).to_le_bytes();
        let key = &key[(pos % 8) as usize..];
        let n = key.len().min(buf.len() - i);
        for (b, k) in buf[i..i + n].iter_mut().zip(key) {
            *b ^= k;
        }
        i += n;
    }
}

const INDEX_SALT: u64 = 0x5241_5047_3244_0001;
const DATA_SALT: u64 = 0x5241_5047_3244_0002;

/// One file in the index: (relative path with `/`, offset in the data, length).
type Entry = (String, u64, u64);

/// A pack of `files` (relative paths with `/` and their contents).
pub fn write(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut index: Vec<Entry> = Vec::with_capacity(files.len());
    let mut offset = 0u64;
    for (path, data) in files {
        index.push((path.clone(), offset, data.len() as u64));
        offset += data.len() as u64;
    }
    let mut idx = postcard::to_stdvec(&index).context("packing the file list")?;
    scramble(&mut idx, 0, INDEX_SALT);
    let mut out = Vec::with_capacity(16 + idx.len() + offset as usize);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(idx.len() as u64).to_le_bytes());
    out.extend_from_slice(&idx);
    let mut pos = 0u64;
    for (_, data) in files {
        let start = out.len();
        out.extend_from_slice(data);
        scramble(&mut out[start..], pos, DATA_SALT);
        pos += data.len() as u64;
    }
    Ok(out)
}

/// The index of a pack whose first bytes are `head` (at least the header and the index): the
/// entries and where the file data starts.
fn read_index(head: &[u8]) -> Result<(Vec<Entry>, u64)> {
    let rest = head.strip_prefix(MAGIC.as_slice()).context("not an R(ust)PG2D data.pack (or from another version)")?;
    let len = u64::from_le_bytes(rest.get(..8).context("data.pack is cut short")?.try_into().unwrap());
    let mut idx = rest.get(8..8 + len as usize).context("data.pack is cut short")?.to_vec();
    scramble(&mut idx, 0, INDEX_SALT);
    let index: Vec<Entry> = postcard::from_bytes(&idx).context("reading data.pack's file list")?;
    Ok((index, 16 + len))
}

/// Every file of a pack in memory (the web game, tests).
pub fn read_all(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let (index, data_start) = read_index(bytes)?;
    index
        .into_iter()
        .map(|(path, off, len)| {
            let a = (data_start + off) as usize;
            let mut data = bytes.get(a..a + len as usize).with_context(|| format!("data.pack is cut short in {path}"))?.to_vec();
            scramble(&mut data, off, DATA_SALT);
            Ok((path, data))
        })
        .collect()
}

/// A pack opened from disk: its index in memory, files read from the file when asked.
#[cfg(not(target_arch = "wasm32"))]
pub struct Pack {
    path: PathBuf,
    files: BTreeMap<String, (u64, u64)>,
    data_start: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl Pack {
    /// Opens a pack and reads its index.
    pub fn open(path: &Path) -> Result<Pack> {
        use std::io::Read;
        let mut f = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut head = [0u8; 16];
        f.read_exact(&mut head).with_context(|| format!("{} is cut short", path.display()))?;
        if &head[..8] != MAGIC {
            bail!("{} is not an R(ust)PG2D data.pack (or from another version)", path.display());
        }
        let len = u64::from_le_bytes(head[8..].try_into().unwrap());
        let mut bytes = head.to_vec();
        bytes.resize(16 + len as usize, 0);
        f.read_exact(&mut bytes[16..]).with_context(|| format!("{} is cut short", path.display()))?;
        let (index, data_start) = read_index(&bytes)?;
        Ok(Pack { path: path.to_path_buf(), files: index.into_iter().map(|(p, o, l)| (p, (o, l))).collect(), data_start })
    }

    /// A file's contents (`rel`: its relative path with `/`); None if the pack has no such file.
    pub fn read(&self, rel: &str) -> Option<std::io::Result<Vec<u8>>> {
        use std::io::{Read, Seek, SeekFrom};
        let &(off, len) = self.files.get(rel)?;
        Some((|| {
            let mut f = std::fs::File::open(&self.path)?;
            f.seek(SeekFrom::Start(self.data_start + off))?;
            let mut data = vec![0u8; len as usize];
            f.read_exact(&mut data)?;
            scramble(&mut data, off, DATA_SALT);
            Ok(data)
        })())
    }

    /// Whether the pack has a file `rel`, or files in a folder `rel`.
    pub fn exists(&self, rel: &str) -> bool {
        let dir = format!("{}/", rel.trim_end_matches('/'));
        self.files.contains_key(rel) || self.files.range(dir.clone()..).next().is_some_and(|(k, _)| k.starts_with(&dir))
    }

    /// The files directly in folder `rel` ("" = the top), as relative paths.
    pub fn files_in(&self, rel: &str) -> Vec<String> {
        let dir = if rel.is_empty() { String::new() } else { format!("{}/", rel.trim_end_matches('/')) };
        self.files.keys().filter(|k| k.starts_with(&dir) && !k[dir.len()..].contains('/')).cloned().collect()
    }

    /// How many files the pack holds.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// The files under `dir` for a pack: relative paths with `/` (sorted) and their contents. Hidden
/// files and folders (a leading `.`) are left out.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_dir_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for rel in dir_files(dir)? {
        let data = std::fs::read(dir.join(&rel)).with_context(|| format!("reading {}", dir.join(&rel).display()))?;
        out.push((rel, data));
    }
    Ok(out)
}

/// The relative paths (with `/`, sorted) of the files under `dir`, without hidden ones.
#[cfg(not(target_arch = "wasm32"))]
pub fn dir_files(dir: &Path) -> Result<Vec<String>> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
        for e in std::fs::read_dir(dir).with_context(|| format!("reading the folder {}", dir.display()))? {
            let e = e?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            let ty = e.file_type()?;
            if ty.is_dir() {
                walk(&e.path(), &rel, out)?;
            } else if ty.is_file() || e.path().is_file() {
                out.push(rel);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, "", &mut out)?;
    out.sort();
    Ok(out)
}

/// Writes `bytes` to `path` through a temporary file beside it that is then renamed, so `path`
/// is either the old or the new file, never half of one.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let name = path.file_name().context("no file name")?.to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.writing"));
    let mut f = std::fs::File::create(&tmp).with_context(|| format!("writing {}", tmp.display()))?;
    f.write_all(bytes).with_context(|| format!("writing {}", tmp.display()))?;
    f.sync_all().with_context(|| format!("writing {}", tmp.display()))?;
    drop(f);
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

/// Packs the files under `dir` (see [`read_dir_files`]) into the pack file `dest` (written
/// atomically). Returns how many files it holds.
#[cfg(not(target_arch = "wasm32"))]
pub fn pack_dir(dir: &Path, dest: &Path) -> Result<usize> {
    let files = read_dir_files(dir)?;
    write_atomic(dest, &write(&files)?)?;
    Ok(files.len())
}

/// Whether a pack's path is safe to unpack: relative, `/`-separated names, no `..` or empty parts.
#[cfg(not(target_arch = "wasm32"))]
fn safe_path(rel: &str) -> bool {
    !rel.is_empty() && !rel.contains('\\') && !rel.contains(':') && rel.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Unpacks the pack file `pack` into the folder `dir` (created; files there are overwritten).
/// Returns how many files it wrote.
#[cfg(not(target_arch = "wasm32"))]
pub fn unpack(pack: &Path, dir: &Path) -> Result<usize> {
    let bytes = std::fs::read(pack).with_context(|| format!("reading {}", pack.display()))?;
    let files = read_all(&bytes).with_context(|| format!("reading {}", pack.display()))?;
    for (rel, data) in &files {
        if !safe_path(rel) {
            bail!("{} holds a file outside its folder ({rel})", pack.display());
        }
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&path, data).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(files.len())
}

/// Whether `path` is a pack file (it starts with the pack's magic).
#[cfg(not(target_arch = "wasm32"))]
pub fn is_pack_file(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_round_trip_and_hide_their_text() {
        let files = vec![
            ("project.ron".to_string(), b"(name: \"Test Quest\")".to_vec()),
            ("defs/items.ron".to_string(), b"[(id: \"sword\", name: \"Sword\")]".to_vec()),
            ("maps/start.map".to_string(), vec![0u8; 3000]),
            ("empty.txt".to_string(), Vec::new()),
        ];
        let bytes = write(&files).unwrap();
        assert_eq!(read_all(&bytes).unwrap(), files);
        // Scrambled: neither the file list nor the texts are readable in the file.
        let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(!has(b"Test Quest") && !has(b"defs/items.ron") && !has(b"Sword"));
        assert!(read_all(b"not a pack").is_err());
        // From disk: the index, then single files.
        let dir = std::env::temp_dir().join(format!("rustpg2d_pack_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(PACK_FILE);
        std::fs::write(&path, &bytes).unwrap();
        let p = Pack::open(&path).unwrap();
        assert_eq!(p.len(), 4);
        assert_eq!(p.read("defs/items.ron").unwrap().unwrap(), files[1].1);
        assert_eq!(p.read("maps/start.map").unwrap().unwrap(), files[2].1);
        assert!(p.read("defs/props.ron").is_none());
        assert!(p.exists("defs") && p.exists("maps/start.map") && !p.exists("sounds"));
        assert_eq!(p.files_in("defs"), vec!["defs/items.ron".to_string()]);
        assert_eq!(p.files_in(""), vec!["empty.txt".to_string(), "project.ron".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_keystream_never_changes() {
        // Packs already shipped must keep reading: 16 zero bytes from position 3, scrambled.
        let mut buf = [0u8; 16];
        scramble(&mut buf, 3, DATA_SALT);
        assert_eq!(buf, [0x49, 0x39, 0x0c, 0x83, 0xbb, 0x1a, 0x68, 0x78, 0x71, 0x05, 0xe2, 0x0e, 0x79, 0x05, 0xd8, 0x54]);
    }

    #[test]
    fn scrambling_is_the_same_in_any_pieces() {
        let data: Vec<u8> = (0..100u8).collect();
        let mut whole = data.clone();
        scramble(&mut whole, 5, DATA_SALT);
        let mut parts = data.clone();
        let (a, b) = parts.split_at_mut(13);
        scramble(a, 5, DATA_SALT);
        scramble(b, 18, DATA_SALT);
        assert_eq!(whole, parts);
        assert_ne!(whole, data);
        scramble(&mut whole, 5, DATA_SALT);
        assert_eq!(whole, data);
    }

    #[test]
    fn folders_pack_and_unpack() {
        let base = std::env::temp_dir().join(format!("rustpg2d_packdir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("game");
        std::fs::create_dir_all(src.join("maps")).unwrap();
        std::fs::create_dir_all(src.join(".git")).unwrap();
        std::fs::write(src.join("project.ron"), "(name: \"G\")").unwrap();
        std::fs::write(src.join("maps/start.map"), [1u8, 2, 3]).unwrap();
        std::fs::write(src.join(".git/HEAD"), "x").unwrap();
        std::fs::write(src.join(".hidden"), "x").unwrap();
        let file = base.join("game.r2dproject");
        assert_eq!(pack_dir(&src, &file).unwrap(), 2);
        assert!(is_pack_file(&file) && !is_pack_file(&src.join("project.ron")));
        let out = base.join("out");
        assert_eq!(unpack(&file, &out).unwrap(), 2);
        assert_eq!(std::fs::read(out.join("maps/start.map")).unwrap(), [1u8, 2, 3]);
        assert!(!out.join(".git").exists() && !out.join(".hidden").exists());
        // A pack with a path leading out of its folder is refused.
        let evil = base.join("evil.r2dproject");
        std::fs::write(&evil, write(&[("../escape.txt".to_string(), b"x".to_vec())]).unwrap()).unwrap();
        assert!(unpack(&evil, &base.join("evil")).is_err());
        assert!(!base.join("escape.txt").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
