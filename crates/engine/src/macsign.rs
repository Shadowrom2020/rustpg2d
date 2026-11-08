//! Ad-hoc code signature of an exported macOS game (`release::export`), made on any platform the
//! way `codesign --sign - <Name>.app` does it: `Contents/_CodeSignature/CodeResources` seals the
//! bundle's files (SHA-1 `files`, SHA-256 `files2`), and every slice of the game executable gets
//! a new code directory that hashes its pages, `Info.plist`, the (empty) requirements and
//! `CodeResources`. Without it the linker's signature of the bare executable doesn't match the
//! bundle around it ("code has no resources but signature indicates they must be present"), which
//! macOS reports as a damaged app once the game was downloaded. Ad-hoc means no identity: macOS
//! still asks the player once (Privacy & Security, Open Anyway), as for any app not notarized.
//!
//! The executable is a thin 64-bit Mach-O or a universal binary of them (`lipo`); a slice
//! without a signature (x86_64 builds) gets an `LC_CODE_SIGNATURE` load command in the free space
//! after the load commands.

use std::path::Path;

use anyhow::{bail, ensure, Context, Result};

use crate::hash::{base64, sha1, sha256};

const PAGE_SHIFT: u8 = 12;
const PAGE: usize = 1 << PAGE_SHIFT;
const MH_MAGIC_64: u32 = 0xfeed_facf;
const FAT_MAGIC: u32 = 0xcafe_babe;
const LC_SEGMENT_64: u32 = 0x19;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const MH_EXECUTE: u32 = 2;
const CSMAGIC_EMBEDDED_SIGNATURE: u32 = 0xfade_0cc0;
const CSMAGIC_CODEDIRECTORY: u32 = 0xfade_0c02;
const CSMAGIC_REQUIREMENTS: u32 = 0xfade_0c01;
const CSMAGIC_BLOBWRAPPER: u32 = 0xfade_0b01;
const CS_ADHOC: u32 = 0x2;
const CS_EXECSEG_MAIN_BINARY: u64 = 0x1;
/// Code directory version with the executable segment fields.
const CD_VERSION: u32 = 0x20400;
/// Special slots: 1 = Info.plist, 2 = requirements, 3 = CodeResources.
const SPECIAL_SLOTS: usize = 3;

/// Signs `<app>/Contents/MacOS/<exe_name>` and seals the bundle; `identifier` is the bundle id.
pub fn sign_app(app: &Path, identifier: &str, exe_name: &str) -> Result<()> {
    let contents = app.join("Contents");
    let resources = code_resources(&contents, exe_name)?;
    let sig_dir = contents.join("_CodeSignature");
    std::fs::create_dir_all(&sig_dir)?;
    std::fs::write(sig_dir.join("CodeResources"), &resources)?;
    let info = std::fs::read(contents.join("Info.plist")).context("reading Info.plist")?;
    let exe_path = contents.join("MacOS").join(exe_name);
    let exe = std::fs::read(&exe_path).with_context(|| format!("reading {}", exe_path.display()))?;
    let signed = sign_macho(&exe, identifier, &sha256(&info), &sha256(&resources))?;
    std::fs::write(&exe_path, signed)?;
    Ok(())
}

/// `CodeResources`: every file of the bundle except the executable, Info.plist, PkgInfo and the
/// signature itself, with codesign's default rules (files outside `Contents/Resources` and
/// `Contents/MacOS` would be nested code, which exports don't have).
fn code_resources(contents: &Path, exe_name: &str) -> Result<Vec<u8>> {
    let mut files = Vec::new();
    let mut stack = vec![contents.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)?.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(contents)?.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            if p.is_dir() {
                if rel != "_CodeSignature" {
                    stack.push(p);
                }
                continue;
            }
            if rel == "Info.plist" || rel == "PkgInfo" || rel == format!("MacOS/{exe_name}") || rel.ends_with(".DS_Store") {
                continue;
            }
            ensure!(rel.starts_with("Resources/"), "{rel}: only Contents/Resources may hold files besides the executable");
            files.push((rel, std::fs::read(&p)?));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>files</key>\n\t<dict>\n");
    for (rel, data) in &files {
        x += &format!("\t\t<key>{}</key>\n\t\t<data>\n\t\t{}\n\t\t</data>\n", esc(rel), base64(&sha1(data)));
    }
    x += "\t</dict>\n\t<key>files2</key>\n\t<dict>\n";
    for (rel, data) in &files {
        x += &format!("\t\t<key>{}</key>\n\t\t<dict>\n\t\t\t<key>hash2</key>\n\t\t\t<data>\n\t\t\t{}\n\t\t\t</data>\n\t\t</dict>\n", esc(rel), base64(&sha256(data)));
    }
    x += "\t</dict>\n";
    x += RULES;
    x += "</dict>\n</plist>\n";
    Ok(x.into_bytes())
}

/// codesign's default resource rules (`rules` for macOS before 10.9, `rules2` after).
const RULES: &str = "\t<key>rules</key>
\t<dict>
\t\t<key>^Resources/</key>
\t\t<true/>
\t\t<key>^Resources/.*\\.lproj/</key>
\t\t<dict>
\t\t\t<key>optional</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>1000</real>
\t\t</dict>
\t\t<key>^Resources/.*\\.lproj/locversion.plist$</key>
\t\t<dict>
\t\t\t<key>omit</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>1100</real>
\t\t</dict>
\t\t<key>^Resources/Base\\.lproj/</key>
\t\t<dict>
\t\t\t<key>weight</key>
\t\t\t<real>1010</real>
\t\t</dict>
\t\t<key>^version.plist$</key>
\t\t<true/>
\t</dict>
\t<key>rules2</key>
\t<dict>
\t\t<key>.*\\.dSYM($|/)</key>
\t\t<dict>
\t\t\t<key>weight</key>
\t\t\t<real>11</real>
\t\t</dict>
\t\t<key>^(.*/)?\\.DS_Store$</key>
\t\t<dict>
\t\t\t<key>omit</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>2000</real>
\t\t</dict>
\t\t<key>^(Frameworks|SharedFrameworks|PlugIns|Plug-ins|XPCServices|Helpers|MacOS|Library/(Automator|Spotlight|LoginItems))/</key>
\t\t<dict>
\t\t\t<key>nested</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>10</real>
\t\t</dict>
\t\t<key>^.*</key>
\t\t<true/>
\t\t<key>^Info\\.plist$</key>
\t\t<dict>
\t\t\t<key>omit</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>20</real>
\t\t</dict>
\t\t<key>^PkgInfo$</key>
\t\t<dict>
\t\t\t<key>omit</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>20</real>
\t\t</dict>
\t\t<key>^Resources/</key>
\t\t<dict>
\t\t\t<key>weight</key>
\t\t\t<real>20</real>
\t\t</dict>
\t\t<key>^Resources/.*\\.lproj/</key>
\t\t<dict>
\t\t\t<key>optional</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>1000</real>
\t\t</dict>
\t\t<key>^Resources/.*\\.lproj/locversion.plist$</key>
\t\t<dict>
\t\t\t<key>omit</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>1100</real>
\t\t</dict>
\t\t<key>^Resources/Base\\.lproj/</key>
\t\t<dict>
\t\t\t<key>weight</key>
\t\t\t<real>1010</real>
\t\t</dict>
\t\t<key>^[^/]+$</key>
\t\t<dict>
\t\t\t<key>nested</key>
\t\t\t<true/>
\t\t\t<key>weight</key>
\t\t\t<real>10</real>
\t\t</dict>
\t\t<key>^embedded\\.provisionprofile$</key>
\t\t<dict>
\t\t\t<key>weight</key>
\t\t\t<real>20</real>
\t\t</dict>
\t\t<key>^version\\.plist$</key>
\t\t<dict>
\t\t\t<key>weight</key>
\t\t\t<real>20</real>
\t\t</dict>
\t</dict>
";

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}
fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn le64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}
fn put_le32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn put_le64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn align(v: usize, to: usize) -> usize {
    v.div_ceil(to) * to
}

/// Signs a thin Mach-O or every slice of a universal binary.
pub fn sign_macho(bin: &[u8], identifier: &str, info_hash: &[u8; 32], resources_hash: &[u8; 32]) -> Result<Vec<u8>> {
    ensure!(bin.len() >= 8, "not a macOS program");
    if be32(bin, 0) == FAT_MAGIC {
        let n = be32(bin, 4) as usize;
        ensure!(n > 0 && bin.len() >= 8 + 20 * n, "damaged universal binary");
        let mut slices = Vec::new();
        for k in 0..n {
            let e = 8 + 20 * k;
            let (cpu, sub, off, size, al) = (be32(bin, e), be32(bin, e + 4), be32(bin, e + 8) as usize, be32(bin, e + 12) as usize, be32(bin, e + 16));
            ensure!(off + size <= bin.len() && al < 32, "damaged universal binary");
            slices.push((cpu, sub, al, sign_thin(&bin[off..off + size], identifier, info_hash, resources_hash)?));
        }
        // Same layout as lipo: each slice at its alignment (2^14 for arm64 / x86_64).
        let mut out = vec![0u8; 8 + 20 * n];
        out[0..4].copy_from_slice(&FAT_MAGIC.to_be_bytes());
        out[4..8].copy_from_slice(&(n as u32).to_be_bytes());
        for (k, (cpu, sub, al, data)) in slices.iter().enumerate() {
            let off = align(out.len(), 1 << al);
            out.resize(off, 0);
            let e = 8 + 20 * k;
            for (i, v) in [*cpu, *sub, off as u32, data.len() as u32, *al].into_iter().enumerate() {
                out[e + 4 * i..e + 4 * i + 4].copy_from_slice(&v.to_be_bytes());
            }
            out.extend_from_slice(data);
        }
        Ok(out)
    } else {
        sign_thin(bin, identifier, info_hash, resources_hash)
    }
}

/// A 64-bit Mach-O with a new embedded signature.
fn sign_thin(slice: &[u8], identifier: &str, info_hash: &[u8; 32], resources_hash: &[u8; 32]) -> Result<Vec<u8>> {
    ensure!(slice.len() >= 32 && le32(slice, 0) == MH_MAGIC_64, "not a 64-bit macOS program (Mach-O)");
    let mut m = slice.to_vec();
    let ncmds = le32(&m, 16) as usize;
    let sizeofcmds = le32(&m, 20) as usize;
    let filetype = le32(&m, 12);
    ensure!(32 + sizeofcmds <= m.len(), "damaged Mach-O header");
    // Load commands of interest: __TEXT, __LINKEDIT, LC_CODE_SIGNATURE; the first section's file offset.
    let (mut text, mut linkedit, mut sig) = (None, None, None);
    let mut first_section = m.len();
    let mut at = 32;
    for _ in 0..ncmds {
        ensure!(at + 8 <= 32 + sizeofcmds, "damaged load commands");
        let (cmd, size) = (le32(&m, at), le32(&m, at + 4) as usize);
        ensure!(size >= 8 && at + size <= 32 + sizeofcmds, "damaged load commands");
        if cmd == LC_SEGMENT_64 {
            let name = &m[at + 8..at + 24];
            if name.starts_with(b"__TEXT\0") {
                text = Some(at);
            } else if name.starts_with(b"__LINKEDIT\0") {
                linkedit = Some(at);
            }
            let nsects = le32(&m, at + 64) as usize;
            for s in 0..nsects {
                let off = le32(&m, at + 72 + 80 * s + 48) as usize;
                if off > 0 {
                    first_section = first_section.min(off);
                }
            }
        } else if cmd == LC_CODE_SIGNATURE {
            sig = Some(at);
        }
        at += size;
    }
    let (Some(text), Some(linkedit)) = (text, linkedit) else { bail!("Mach-O without __TEXT or __LINKEDIT") };
    let (le_off, le_size) = (le64(&m, linkedit + 40) as usize, le64(&m, linkedit + 48) as usize);
    // The signature goes where the old one was, else after __LINKEDIT (the end of the file).
    let dataoff = match sig {
        Some(s) => {
            let off = le32(&m, s + 8) as usize;
            ensure!(off <= m.len() && off >= le_off, "damaged code signature command");
            off
        }
        None => {
            ensure!(le_off + le_size == m.len(), "__LINKEDIT isn't at the end of the executable");
            ensure!(32 + sizeofcmds + 16 <= first_section, "no room for a code signature load command");
            let s = 32 + sizeofcmds;
            put_le32(&mut m, s, LC_CODE_SIGNATURE);
            put_le32(&mut m, s + 4, 16);
            put_le32(&mut m, 16, ncmds as u32 + 1);
            put_le32(&mut m, 20, sizeofcmds as u32 + 16);
            sig = Some(s);
            align(m.len(), 16)
        }
    };
    let sig = sig.unwrap();
    m.truncate(dataoff);
    m.resize(dataoff, 0);
    let n_code = dataoff.div_ceil(PAGE);
    let ident_len = identifier.len() + 1;
    let cd_len = 88 + ident_len + (SPECIAL_SLOTS + n_code) * 32;
    let requirements: [u8; 12] = [0xfa, 0xde, 0x0c, 0x01, 0, 0, 0, 12, 0, 0, 0, 0];
    debug_assert_eq!(be32(&requirements, 0), CSMAGIC_REQUIREMENTS);
    let wrapper: [u8; 8] = [0xfa, 0xde, 0x0b, 0x01, 0, 0, 0, 8];
    debug_assert_eq!(be32(&wrapper, 0), CSMAGIC_BLOBWRAPPER);
    let header = 12 + 3 * 8;
    let total = header + cd_len + requirements.len() + wrapper.len();
    let datasize = align(total, 16);
    // Header fields first: the first page's hash covers them.
    put_le32(&mut m, sig + 8, dataoff as u32);
    put_le32(&mut m, sig + 12, datasize as u32);
    let le_size = dataoff + datasize - le_off;
    put_le64(&mut m, linkedit + 48, le_size as u64);
    let vm_page = 0x4000;
    if (le64(&m, linkedit + 32) as usize) < le_size {
        put_le64(&mut m, linkedit + 32, align(le_size, vm_page) as u64);
    }
    // Code directory.
    let mut cd = Vec::with_capacity(cd_len);
    let hash_offset = 88 + ident_len + SPECIAL_SLOTS * 32;
    for v in [CSMAGIC_CODEDIRECTORY, cd_len as u32, CD_VERSION, CS_ADHOC, hash_offset as u32, 88, SPECIAL_SLOTS as u32, n_code as u32, dataoff as u32] {
        cd.extend_from_slice(&v.to_be_bytes());
    }
    cd.extend_from_slice(&[32, 2, 0, PAGE_SHIFT]); // hash size, SHA-256, platform, page size
    cd.extend_from_slice(&[0; 16]); // spare2, scatterOffset, teamOffset, spare3
    cd.extend_from_slice(&0u64.to_be_bytes()); // codeLimit64
    let (text_off, text_size) = (le64(&m, text + 40), le64(&m, text + 48));
    cd.extend_from_slice(&text_off.to_be_bytes());
    cd.extend_from_slice(&text_size.to_be_bytes());
    cd.extend_from_slice(&(if filetype == MH_EXECUTE { CS_EXECSEG_MAIN_BINARY } else { 0 }).to_be_bytes());
    debug_assert_eq!(cd.len(), 88);
    cd.extend_from_slice(identifier.as_bytes());
    cd.push(0);
    // Special slots, highest first: CodeResources (3), requirements (2), Info.plist (1).
    cd.extend_from_slice(resources_hash);
    cd.extend_from_slice(&sha256(&requirements));
    cd.extend_from_slice(info_hash);
    for page in m.chunks(PAGE) {
        cd.extend_from_slice(&sha256(page));
    }
    debug_assert_eq!(cd.len(), cd_len);
    // Superblob: code directory, requirements, empty CMS wrapper (ad-hoc).
    let mut blob = Vec::with_capacity(datasize);
    for v in [CSMAGIC_EMBEDDED_SIGNATURE, total as u32, 3, 0, header as u32, 2, (header + cd_len) as u32, 0x10000, (header + cd_len + requirements.len()) as u32] {
        blob.extend_from_slice(&v.to_be_bytes());
    }
    blob.extend_from_slice(&cd);
    blob.extend_from_slice(&requirements);
    blob.extend_from_slice(&wrapper);
    blob.resize(datasize, 0);
    m.extend_from_slice(&blob);
    Ok(m)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal Mach-O executable: header, __TEXT with one section, __LINKEDIT at the end.
    pub(crate) fn tiny_macho(with_sig: bool) -> Vec<u8> {
        let mut m = vec![0u8; 0x2000];
        put_le32(&mut m, 0, MH_MAGIC_64);
        put_le32(&mut m, 4, 0x0100_000c); // arm64
        put_le32(&mut m, 12, MH_EXECUTE);
        let mut at = 32;
        // __TEXT: 72 + one 80-byte section.
        put_le32(&mut m, at, LC_SEGMENT_64);
        put_le32(&mut m, at + 4, 152);
        m[at + 8..at + 14].copy_from_slice(b"__TEXT");
        put_le64(&mut m, at + 40, 0);
        put_le64(&mut m, at + 48, 0x1000);
        put_le32(&mut m, at + 64, 1);
        put_le32(&mut m, at + 72 + 48, 0x400); // section file offset
        at += 152;
        put_le32(&mut m, at, LC_SEGMENT_64);
        put_le32(&mut m, at + 4, 72);
        m[at + 8..at + 18].copy_from_slice(b"__LINKEDIT");
        put_le64(&mut m, at + 32, 0x1000);
        put_le64(&mut m, at + 40, 0x1000);
        put_le64(&mut m, at + 48, 0x1000);
        at += 72;
        let mut ncmds = 2;
        if with_sig {
            put_le32(&mut m, at, LC_CODE_SIGNATURE);
            put_le32(&mut m, at + 4, 16);
            put_le32(&mut m, at + 8, 0x1800);
            put_le32(&mut m, at + 12, 0x800);
            put_le64(&mut m, 32 + 152 + 32, 0x2000); // vmsize
            put_le64(&mut m, 32 + 152 + 48, 0x1000); // filesize up to the end
            at += 16;
            ncmds += 1;
        }
        put_le32(&mut m, 16, ncmds);
        put_le32(&mut m, 20, (at - 32) as u32);
        for (i, b) in m[0x400..0x1000].iter_mut().enumerate() {
            *b = i as u8;
        }
        m
    }

    /// Checks what `codesign --verify` checks of the code directory: page hashes, slots, offsets.
    fn verify(m: &[u8], ident: &str, info: &[u8; 32], res: &[u8; 32]) {
        let mut at = 32;
        let mut sig = None;
        for _ in 0..le32(m, 16) {
            if le32(m, at) == LC_CODE_SIGNATURE {
                sig = Some((le32(m, at + 8) as usize, le32(m, at + 12) as usize));
            }
            at += le32(m, at + 4) as usize;
        }
        let (off, size) = sig.expect("LC_CODE_SIGNATURE");
        assert_eq!(off + size, m.len(), "the signature ends the file");
        let b = &m[off..];
        assert_eq!(be32(b, 0), CSMAGIC_EMBEDDED_SIGNATURE);
        assert_eq!(be32(b, 8), 3);
        let cd = &b[be32(b, 16) as usize..];
        assert_eq!(be32(cd, 0), CSMAGIC_CODEDIRECTORY);
        assert_eq!(be32(cd, 12), CS_ADHOC);
        assert_eq!(be32(cd, 32) as usize, off, "code limit = signature offset");
        let ho = be32(cd, 16) as usize;
        let id = &cd[be32(cd, 20) as usize..];
        assert_eq!(&id[..ident.len() + 1], format!("{ident}\0").as_bytes());
        assert_eq!(&cd[ho - 32..ho], info, "slot 1 = Info.plist");
        assert_eq!(&cd[ho - 96..ho - 64], res, "slot 3 = CodeResources");
        let n = be32(cd, 28) as usize;
        assert_eq!(n, off.div_ceil(PAGE));
        for (k, page) in m[..off].chunks(PAGE).enumerate() {
            assert_eq!(&cd[ho + 32 * k..ho + 32 * k + 32], &sha256(page), "page {k}");
        }
        let req = &b[be32(b, 24) as usize..];
        assert_eq!(&cd[ho - 64..ho - 32], &sha256(&req[..12]), "slot 2 = requirements");
    }

    #[test]
    fn signs_thin_and_universal() {
        let (info, res) = (sha256(b"info"), sha256(b"res"));
        // With a linker signature: replaced in place.
        let a = sign_macho(&tiny_macho(true), "org.rustpg2d.t", &info, &res).unwrap();
        verify(&a, "org.rustpg2d.t", &info, &res);
        assert_eq!(le32(&a, 16), 3, "no extra load command");
        // Without (x86_64 builds): the load command is added.
        let b = sign_macho(&tiny_macho(false), "org.rustpg2d.t", &info, &res).unwrap();
        verify(&b, "org.rustpg2d.t", &info, &res);
        assert_eq!(le32(&b, 16), 3);
        // Signing again gives the same bytes.
        assert_eq!(sign_macho(&b, "org.rustpg2d.t", &info, &res).unwrap(), b);
        // Universal: each slice signed, offsets aligned to 2^14.
        let (s1, s2) = (tiny_macho(false), tiny_macho(true));
        let mut fat = vec![0u8; 0x4000];
        fat[0..8].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2]);
        for (k, (off, s)) in [(0x4000usize, &s1), (0x8000, &s2)].into_iter().enumerate() {
            let e = 8 + 20 * k;
            for (i, v) in [0x0100_0007u32, 3, off as u32, s.len() as u32, 14].into_iter().enumerate() {
                fat[e + 4 * i..e + 4 * i + 4].copy_from_slice(&v.to_be_bytes());
            }
            fat.resize(off, 0);
            fat.extend_from_slice(s);
        }
        let u = sign_macho(&fat, "x", &info, &res).unwrap();
        for k in 0..2 {
            let (off, size) = (be32(&u, 8 + 20 * k + 8) as usize, be32(&u, 8 + 20 * k + 12) as usize);
            assert_eq!(off % 0x4000, 0);
            verify(&u[off..off + size], "x", &info, &res);
        }
        assert!(sign_macho(b"MZ\x90\0not mach-o here.......................", "x", &info, &res).is_err());
    }

    #[test]
    fn seals_the_bundle() {
        let tmp = std::env::temp_dir().join(format!("rustpg2d-macsign-{}", std::process::id()));
        let c = tmp.join("A.app/Contents");
        std::fs::create_dir_all(c.join("MacOS")).unwrap();
        std::fs::create_dir_all(c.join("Resources/data/maps")).unwrap();
        std::fs::write(c.join("Info.plist"), "<plist/>").unwrap();
        std::fs::write(c.join("PkgInfo"), "APPL????").unwrap();
        std::fs::write(c.join("Resources/data/maps/a&b.map"), "abc").unwrap();
        std::fs::write(c.join("MacOS/a"), tiny_macho(true)).unwrap();
        sign_app(&tmp.join("A.app"), "org.rustpg2d.a", "a").unwrap();
        let cr = std::fs::read(c.join("_CodeSignature/CodeResources")).unwrap();
        let text = String::from_utf8(cr.clone()).unwrap();
        assert!(text.contains("<key>Resources/data/maps/a&amp;b.map</key>\n\t\t<data>\n\t\tqZk+NkcGgWq6PiVxeFDCbJzQ2J0=\n"), "SHA-1 of abc");
        assert!(text.contains("<key>hash2</key>\n\t\t\t<data>\n\t\t\tungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=\n"), "SHA-256 of abc");
        assert!(!text.contains("PkgInfo</key>") && !text.contains("Info.plist</key>") && !text.contains("MacOS/a"));
        verify(&std::fs::read(c.join("MacOS/a")).unwrap(), "org.rustpg2d.a", &sha256(b"<plist/>"), &sha256(&cr));
        // Anything outside Resources would be nested code.
        std::fs::write(c.join("stray.txt"), "x").unwrap();
        assert!(sign_app(&tmp.join("A.app"), "org.rustpg2d.a", "a").is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
