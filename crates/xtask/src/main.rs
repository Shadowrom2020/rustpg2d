//! Project tasks for development and CI (`cargo xtask <task>`, see `.cargo/config.toml`):
//!
//! - `dist [--out DIR] [--bin-dir DIR] …` (`cargo xtask-dist`, feature `dist`) — the R(ust)PG2D
//!   downloads (editor, game runners, example project): `rustpg2d-X.Y.Z-x86_64.AppImage`,
//!   `rustpg2d-X.Y.Z.flatpak`, the Windows zip and the macOS app (see `dist.rs`), plus
//!   `SHA256SUMS`. Expects `cargo build --release` first (binaries in `target/release`).
//! - `check-version BRANCH` — a release / hotfix branch (`release/X.Y.Z`, `hotfix/X.Y.Z`) names a
//!   valid SemVer version above the newest `v*` tag that matches Cargo.toml and the CHANGELOG.
//! - `check-branch HEAD BASE` — the branch model: which branches may open pull requests into
//!   `develop` and `master` (AGENTS.md).
//! - `release-notes X.Y.Z` — prints that version's CHANGELOG section (the GitHub Release text).
//!
//! Without the `dist` feature xtask has no engine dependency, so the checks CI runs on every pull
//! request compile in seconds.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

#[cfg(feature = "dist")]
mod dist;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let res = match args.first().map(String::as_str) {
        #[cfg(feature = "dist")]
        Some("dist") => dist::dist(&args[1..]),
        #[cfg(not(feature = "dist"))]
        Some("dist") => Err(anyhow::anyhow!("dist needs the engine: run `cargo xtask-dist` (xtask with --features dist)")),
        Some("check-version") => args.get(1).context("usage: check-version BRANCH").and_then(|b| check_version(&root(), b)),
        Some("check-branch") => match (args.get(1), args.get(2)) {
            (Some(h), Some(b)) => check_branch(h, b).map(|m| println!("{m}")),
            _ => Err(anyhow::anyhow!("usage: check-branch HEAD BASE")),
        },
        Some("release-notes") => {
            args.get(1).context("usage: release-notes X.Y.Z").and_then(|v| release_notes(&read(&root().join("CHANGELOG.md"))?, v)).map(|n| print!("{n}"))
        }
        _ => Err(anyhow::anyhow!("tasks: dist, check-version, check-branch, release-notes (see crates/xtask/src/main.rs)")),
    };
    if let Err(e) = res {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// The workspace root.
pub(crate) fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

pub(crate) fn read(p: &Path) -> Result<String> {
    std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))
}

// ------------------------------------------------------------------ SemVer

/// A SemVer 2.0 version (build metadata is accepted and ignored for ordering).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<String>,
}

impl Version {
    fn parse(s: &str) -> Result<Version> {
        let core_pre = s.split_once('+').map_or(s, |(a, _)| a);
        let (core, pre) = match core_pre.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (core_pre, None),
        };
        let nums: Vec<&str> = core.split('.').collect();
        if nums.len() != 3 {
            bail!("'{s}' is not MAJOR.MINOR.PATCH");
        }
        let num = |n: &str| -> Result<u64> {
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) || (n.len() > 1 && n.starts_with('0')) {
                bail!("'{s}': '{n}' is not a number without leading zeros");
            }
            Ok(n.parse()?)
        };
        let pre: Vec<String> = match pre {
            None => Vec::new(),
            Some(p) => {
                let ids: Vec<String> = p.split('.').map(String::from).collect();
                if ids.iter().any(|i| i.is_empty() || !i.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
                    bail!("'{s}': bad pre-release '{p}'");
                }
                ids
            }
        };
        Ok(Version { major: num(nums[0])?, minor: num(nums[1])?, patch: num(nums[2])?, pre })
    }

    fn is_pre(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if self.is_pre() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

impl Ord for Version {
    /// SemVer precedence: numbers, then a pre-release sorts before the release; pre-release
    /// identifiers compare numerically when numeric, numbers before text.
    fn cmp(&self, o: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(o.major, o.minor, o.patch)).then_with(|| match (self.is_pre(), o.is_pre()) {
            (false, false) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => {
                for (a, b) in self.pre.iter().zip(&o.pre) {
                    let c = match (a.parse::<u64>(), b.parse::<u64>()) {
                        (Ok(x), Ok(y)) => x.cmp(&y),
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => a.cmp(b),
                    };
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                self.pre.len().cmp(&o.pre.len())
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// The version in `[workspace.package]` of the root Cargo.toml.
pub(crate) fn workspace_version(cargo_toml: &str) -> Result<String> {
    let mut in_pkg = false;
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_pkg = t == "[workspace.package]";
        } else if in_pkg {
            if let Some(v) = t.strip_prefix("version").map(str::trim_start).and_then(|r| r.strip_prefix('=')) {
                return Ok(v.trim().trim_matches('"').to_string());
            }
        }
    }
    bail!("no version in [workspace.package] of Cargo.toml")
}

/// The newest `vX.Y.Z` tag of the repository (None = no release yet).
fn newest_tag(root: &Path) -> Result<Option<Version>> {
    let out = Command::new("git").arg("-C").arg(root).args(["tag", "--list", "v*"]).output().context("running git")?;
    if !out.status.success() {
        bail!("git tag failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().filter_map(|t| Version::parse(t.trim().strip_prefix('v')?).ok()).max())
}

/// Whether the CHANGELOG has a `## X.Y.Z` heading.
fn has_section(changelog: &str, version: &str) -> bool {
    changelog.lines().any(|l| l.strip_prefix("## ").is_some_and(|r| r == version || r.starts_with(&format!("{version} "))))
}

/// Checks a release / hotfix branch against the repository (see `check_release`).
fn check_version(root: &Path, branch: &str) -> Result<()> {
    let cargo = read(&root.join("Cargo.toml"))?;
    let changelog = read(&root.join("CHANGELOG.md"))?;
    let latest = newest_tag(root)?;
    let v = check_release(branch, &workspace_version(&cargo)?, &changelog, latest.as_ref())?;
    println!("ok: {branch} releases {v} (latest release: {})", latest.map_or("none".into(), |l| format!("v{l}")));
    Ok(())
}

/// The rules for `release/X.Y.Z` and `hotfix/X.Y.Z`: valid SemVer, above the newest release
/// (a hotfix only raises PATCH of the newest MAJOR.MINOR), the same as Cargo.toml, a CHANGELOG
/// section for it and no `## Unreleased` left over.
fn check_release(branch: &str, cargo_version: &str, changelog: &str, latest: Option<&Version>) -> Result<Version> {
    let (kind, ver) = branch.split_once('/').context("expected release/X.Y.Z or hotfix/X.Y.Z")?;
    if kind != "release" && kind != "hotfix" {
        bail!("'{branch}' is not a release/X.Y.Z or hotfix/X.Y.Z branch");
    }
    let v = Version::parse(ver).with_context(|| format!("branch {branch}"))?;
    if let Some(l) = latest {
        if v <= *l {
            bail!("{v} is not above the latest release v{l}");
        }
        if kind == "hotfix" && (v.major, v.minor) != (l.major, l.minor) {
            bail!("a hotfix raises PATCH of the latest release v{l} (e.g. {}.{}.{}); {v} changes MAJOR or MINOR", l.major, l.minor, l.patch + 1);
        }
    }
    if cargo_version != v.to_string() {
        bail!("Cargo.toml says version {cargo_version}, the branch says {v}: set [workspace.package] version = \"{v}\"");
    }
    if !has_section(changelog, &v.to_string()) {
        bail!("CHANGELOG.md has no '## {v}' section");
    }
    if changelog.lines().any(|l| l.trim().eq_ignore_ascii_case("## unreleased")) {
        bail!("CHANGELOG.md still has an '## Unreleased' section: rename it to '## {v} — <date>'");
    }
    Ok(v)
}

/// The CHANGELOG section of `version` without its heading.
fn release_notes(changelog: &str, version: &str) -> Result<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in changelog.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if inside {
                break;
            }
            inside = rest == version || rest.starts_with(&format!("{version} "));
            continue;
        }
        if inside {
            out.push(line);
        }
    }
    if !inside && out.is_empty() {
        bail!("CHANGELOG.md has no '## {version}' section");
    }
    Ok(out.join("\n").trim().to_string() + "\n")
}

// ------------------------------------------------------------------ branch policy

/// Which branches may open pull requests into `develop` and `master`. Returns a short verdict.
fn check_branch(head: &str, base: &str) -> Result<String> {
    let named = |prefix: &str| head.strip_prefix(prefix).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c)));
    let versioned = |prefix: &str| head.strip_prefix(prefix).is_some_and(|v| Version::parse(v).is_ok());
    let ok = match base {
        "develop" => named("feature/") || named("bugfix/") || named("dependabot/") || versioned("release/") || versioned("hotfix/") || head == "master",
        "master" => versioned("release/") || versioned("hotfix/"),
        _ => true,
    };
    if !ok {
        bail!(
            "'{head}' may not be merged into '{base}'. develop takes feature/<name>, bugfix/<name>, release/X.Y.Z, hotfix/X.Y.Z, master (back-merge) and dependabot/*; master takes release/X.Y.Z and hotfix/X.Y.Z only"
        );
    }
    Ok(format!("ok: {head} -> {base}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Rust version is pinned in three places; they must agree (AGENTS.md, *Continuous
    /// integration*).
    #[test]
    fn toolchain_pins_agree() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let toml = std::fs::read_to_string(root.join("rust-toolchain.toml")).unwrap();
        let channel = toml.lines().find_map(|l| l.trim().strip_prefix("channel = ")).unwrap().trim_matches('"').to_string();
        assert!(channel.split('.').count() == 3, "rust-toolchain.toml pins a full version, not {channel}");
        let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let rust_version = cargo.lines().find_map(|l| l.trim().strip_prefix("rust-version = ")).unwrap().trim_matches('"').to_string();
        assert!(channel.starts_with(&format!("{rust_version}.")), "Cargo.toml rust-version {rust_version} vs toolchain {channel}");
        let mut seen = 0;
        for f in std::fs::read_dir(root.join(".github/workflows")).unwrap().flatten() {
            let text = std::fs::read_to_string(f.path()).unwrap();
            for line in text.lines().filter(|l| l.trim().starts_with("toolchain: ")) {
                assert_eq!(line.trim().trim_start_matches("toolchain: "), channel, "{}: {line}", f.path().display());
                seen += 1;
            }
            assert!(!text.contains("rust-toolchain@stable"), "{}: use @master with the pinned toolchain", f.path().display());
        }
        assert!(seen >= 3, "the workflows pin the toolchain");
    }

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn semver_parse_and_order() {
        assert_eq!(v("1.2.3").to_string(), "1.2.3");
        assert_eq!(v("0.2.0-rc.1+build.5").to_string(), "0.2.0-rc.1");
        for bad in ["1.2", "01.2.3", "1.2.3.4", "1.x.3", "1.2.3-", "1.2.3-a..b", ""] {
            assert!(Version::parse(bad).is_err(), "{bad}");
        }
        // The precedence example of the SemVer spec.
        let order = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta", "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0", "1.0.1", "1.1.0", "2.0.0"];
        for w in order.windows(2) {
            assert!(v(w[0]) < v(w[1]), "{} < {}", w[0], w[1]);
        }
        assert!(v("0.10.0") > v("0.9.9"));
    }

    #[test]
    fn workspace_version_is_read() {
        let toml = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.2.0\"\nedition = \"2021\"\n\n[dependencies]\nversion = \"9\"\n";
        assert_eq!(workspace_version(toml).unwrap(), "0.2.0");
        assert_eq!(workspace_version(&read(&root().join("Cargo.toml")).unwrap()).unwrap(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn release_rules() {
        let latest = v("0.1.0");
        let log = "# Changelog\n\n## 0.2.0 — 2026-11-06\n\n- New\n\n## 0.1.0 — 2026-11-05\n";
        assert_eq!(check_release("release/0.2.0", "0.2.0", log, Some(&latest)).unwrap(), v("0.2.0"));
        assert!(check_release("release/0.1.0", "0.1.0", log, Some(&latest)).unwrap_err().to_string().contains("not above"));
        assert!(check_release("release/0.2.0", "0.1.0", log, Some(&latest)).unwrap_err().to_string().contains("Cargo.toml"));
        assert!(check_release("release/0.3.0", "0.3.0", log, Some(&latest)).unwrap_err().to_string().contains("no '## 0.3.0'"));
        assert!(check_release("release/0.2", "0.2.0", log, Some(&latest)).is_err());
        assert!(check_release("feature/x", "0.2.0", log, Some(&latest)).is_err());
        let unreleased = format!("# Changelog\n\n## Unreleased\n\n{}", &log[13..]);
        assert!(check_release("release/0.2.0", "0.2.0", &unreleased, Some(&latest)).unwrap_err().to_string().contains("Unreleased"));
        // Hotfixes raise PATCH only.
        let fix = "## 0.1.1 — x\n## 0.2.0 — y\n";
        assert!(check_release("hotfix/0.1.1", "0.1.1", fix, Some(&latest)).is_ok());
        assert!(check_release("hotfix/0.2.0", "0.2.0", fix, Some(&latest)).unwrap_err().to_string().contains("PATCH"));
        // Pre-releases.
        let rc = "## 0.2.0-rc.1 — z\n";
        assert!(check_release("release/0.2.0-rc.1", "0.2.0-rc.1", rc, Some(&latest)).is_ok());
    }

    #[test]
    fn notes_of_a_version() {
        let log = "# Changelog\n\nintro\n\n## 0.2.0 — 2026-11-06\n\n### Added\n- A\n\n## 0.1.0 — 2026-11-05\n- B\n";
        assert_eq!(release_notes(log, "0.2.0").unwrap(), "### Added\n- A\n");
        assert_eq!(release_notes(log, "0.1.0").unwrap(), "- B\n");
        assert!(release_notes(log, "0.3.0").is_err());
        // The real changelog has the current release.
        let real = read(&root().join("CHANGELOG.md")).unwrap();
        assert!(release_notes(&real, "0.1.0").unwrap().contains("Autosave"));
    }

    #[test]
    fn branch_policy() {
        for (h, b) in [
            ("feature/linux-packages", "develop"),
            ("bugfix/gate-spawn", "develop"),
            ("dependabot/cargo/rhai-1.28", "develop"),
            ("release/0.2.0", "develop"),
            ("master", "develop"),
            ("release/0.2.0", "master"),
            ("hotfix/0.1.1", "master"),
            ("anything", "feature/x"),
        ] {
            assert!(check_branch(h, b).is_ok(), "{h} -> {b}");
        }
        for (h, b) in [("feature/x", "master"), ("develop", "master"), ("bugfix/x", "master"), ("release/next", "master"), ("fix-it", "develop"), ("feature/", "develop"), ("feature/a b", "develop")] {
            assert!(check_branch(h, b).is_err(), "{h} -> {b}");
        }
    }
}
