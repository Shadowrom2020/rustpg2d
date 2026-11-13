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

/// The keystream's byte at `pos` (positions count from the start of the index / the data).
fn key_byte(pos: u64, salt: u64) -> u8 {
    // SplitMix64 of the 8-byte block, one byte of it per position.
    let mut z = (pos / 8).wrapping_add(salt).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> ((pos % 8) * 8)) as u8
}

/// Scrambles (or unscrambles: the same operation) `buf`, which starts at position `start`.
fn scramble(buf: &mut [u8], start: u64, salt: u64) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b ^= key_byte(start + i as u64, salt);
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
}
