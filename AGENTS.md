# AGENTS.md

Guidelines for anyone (people and coding agents) working on R(ust)PG2D: a Rust 2D top-down RPG
engine (`crates/engine`), the game runner (`crates/game`) and the editor (`crates/editor`).

## Branching concept

```
feature/<name>, bugfix/<name>   o---o
                               /     \ PR
develop        ---o-----------o-------o---------------o-------------------------o---
                                       \             / back-merge PR           / back-merge PR
release/0.2.0                           o---o       /                         /
                                             \ PR  /                         /
master         ---o---------------------------o---o-------------------------o---
                                         tag v0.2.0 \                      / PR, tag v0.2.1
hotfix/0.2.1                                         o--------------------o
```

| Branch | Created from | Pull request into | Purpose |
|---|---|---|---|
| `master` | – | – | Released versions only. Every merge is a release: CI tags it `vX.Y.Z` and publishes it. |
| `develop` | `master` (once) | – | Integration branch: everything finished for the next release. Always builds and passes CI. |
| `feature/<name>` | `develop` | `develop` | One feature, e.g. `feature/quest-journal`. |
| `bugfix/<name>` | `develop` | `develop` | A fix that ships with the next regular release, e.g. `bugfix/gate-spawn`. |
| `release/X.Y.Z` | `develop` | `master` | Prepares a release: version bump, CHANGELOG section, small fixes found while preparing it. No new features. |
| `hotfix/X.Y.Z` | `master` | `master` | An urgent fix of the released version that can't wait for the next release; raises PATCH. |
| `dependabot/*` | – | `develop` | Dependency updates opened by Dependabot. |

Note: this repository's main branch is called `master` (not `main`).

### Rules

1. **`master` and `develop` are protected** (GitHub rulesets, for everyone including admins):
   no direct pushes, no force pushes, no deletion. Changes arrive by **pull request** only, merged
   with a **merge commit** (squash and rebase merges are disabled) once the required CI checks
   pass. No review is required (one developer); `master` additionally requires the branch to be
   up to date. Tags `v*` can't be moved or deleted.
2. **CI decides which branch may go where** (`cargo xtask check-branch`, the *Branch policy*
   check): `develop` takes `feature/*`, `bugfix/*`, `release/X.Y.Z`, `hotfix/X.Y.Z`, `master`
   (back-merges) and `dependabot/*`; `master` takes only `release/X.Y.Z` and `hotfix/X.Y.Z`.
3. Keep a branch focused on one topic; GitHub deletes it after the merge.
4. **Releases** are cut on demand from `develop`, **hotfixes** from `master`. Their branch name
   carries the version, and CI (*Version* check, `cargo xtask check-version`) refuses the pull
   request unless the version is valid SemVer, above the latest release, equal to
   `[workspace.package] version` in `Cargo.toml`, has a `## X.Y.Z — date` section in
   CHANGELOG.md and no `## Unreleased` is left; a hotfix may only raise PATCH.
5. **Merging into master releases automatically** (`.github/workflows/release.yml`): it takes
   the R(ust)PG2D AppImage, Flatpak, Windows zip and macOS app that CI built for the merged pull
   request (master only takes up-to-date branches, so the merge commit has exactly that tree; it
   builds them again only when they can't be found, e.g. expired), tags the merge commit
   `vX.Y.Z`, publishes a GitHub Release (notes = the CHANGELOG section, with the packages and
   SHA256SUMS) and opens the pull request that merges `master` back into `develop`, about a
   minute after the merge. Merge that one too: while `develop` has nothing `master` lacks, its
   checks only report (the merge is the tested tree).
6. Push and open pull requests only when asked to; merge only when asked to.
7. Before starting work, check which branch is checked out (`git branch --show-current`) and
   that the working tree is clean. If the current branch is `master` or `develop`, create a
   branch first.

### Versions: Semantic Versioning 2.0

Versions are `MAJOR.MINOR.PATCH` ([semver.org](https://semver.org)), in `[workspace.package]
version` of the root `Cargo.toml`, tagged `vX.Y.Z`:

- **MAJOR** — incompatible changes: projects, maps, definitions or save games of the previous
  version no longer load (the compatibility rules below make this rare), or a script API is
  removed or changes meaning.
- **MINOR** — new features (the usual release from `develop`); backwards compatible.
- **PATCH** — fixes only (hotfixes; a release from `develop` with only bug fixes).
- **Pre-releases** `X.Y.Z-rc.N` are allowed (`release/0.3.0-rc.1`); GitHub marks them as
  pre-releases. Build metadata (`+…`) is not used.
- While MAJOR is 0, MINOR releases may still contain breaking changes; say so in the CHANGELOG.

### Commands

```sh
# Start a feature or bugfix
git checkout develop && git pull --ff-only
git checkout -b feature/<name>               # or bugfix/<name>
# ... commit, then publish and open the pull request
git push -u origin feature/<name>
gh pr create --base develop --fill
gh pr checks --watch                         # wait for CI
gh pr merge --merge                          # (--auto: merge as soon as CI passes)

# Cut a release (MINOR for features, PATCH for fixes only)
git checkout develop && git pull --ff-only
git checkout -b release/0.2.0
#   set [workspace.package] version = "0.2.0" in Cargo.toml (cargo build updates Cargo.lock),
#   rename "## Unreleased" in CHANGELOG.md to "## 0.2.0 — YYYY-MM-DD", update "Current version" in README.md
git commit -am "R(ust)PG2D 0.2.0"
git push -u origin release/0.2.0
gh pr create --base master --title "R(ust)PG2D 0.2.0" --fill
gh pr merge --merge                          # after CI: Release workflow tags, publishes, opens the back-merge PR
gh pr merge <back-merge PR> --merge          # master -> develop

# Hotfix a released version
git checkout master && git pull --ff-only
git checkout -b hotfix/0.2.1
#   fix, bump version to 0.2.1, add "## 0.2.1 — YYYY-MM-DD" to CHANGELOG.md
git push -u origin hotfix/0.2.1
gh pr create --base master --fill            # then merge it and the back-merge PR as above
```

## Continuous integration

`.github/workflows/ci.yml` runs on every pull request into `develop` / `master` and on pushes to
`master` (Ubuntu 22.04, so the binaries need only glibc ≤ 2.35). *Build, lint, test* and
*Runtime smoke* only report success, without running, on a tree that was already tested: a push to
`master` with its pull request's tree, and the back-merge `master` → `develop` while the merge is
`master`'s tree. Every job uses the Rust version
pinned in `rust-toolchain.toml` (also `rust-version` in `Cargo.toml` and the `toolchain:` inputs
of the workflows; an xtask test keeps them equal), so a new stable release never breaks a pull
request by surprise: bump the three together in a `feature/` branch. Pushes to `develop` don't run it
again: the pull request already tested the merge. Its jobs are the required checks:

| Check | What it does | Runs for |
|---|---|---|
| *Branch policy* | `cargo xtask check-branch HEAD BASE` (seconds: xtask without the engine) | every PR (required: develop, master) |
| *Build, lint, test* | `cargo clippy --all-targets --all-features -D warnings`, `cargo test` (workspace, `--locked`, unoptimized: fast to compile) | every PR (required: develop, master) |
| *Runtime smoke* | `scripts/ci/smoke.sh`: the game and the editor on a copy of the example project under Xvfb (title, a map, the clock, an old save, quick save, a map change + autosave, the editor opening a map, the MCP smoke client) | every PR (advisory) |
| *Version* | `cargo xtask check-version BRANCH` | PRs into master (required there) |
| *Release builds* | `builds.yml`, all at the same time: macOS editor + game per architecture (two `macos-latest` jobs), Linux editor + game + the packaging tool (`xtask` with `dist`), the web game, Windows editor + game (MinGW-w64) | PRs into master, manual runs |
| *Packages* | assembles the release builds without compiling: universal macOS binaries (`llvm-lipo`), `xtask dist` (AppImage, Flatpak, Windows zip, unsigned macOS app), smoke test of game and editor under Xvfb, downloads as artifact (kept 30 days: the release publishes them) | PRs into master (required there) |
| *Windows* | clippy and tests on `windows-latest` (MSVC) | PRs into master, manual runs |
| *macOS* | clippy for editor and game, the engine's tests on `macos-latest` | PRs into master, manual runs |
| *macOS app* | signs the app ad hoc (`codesign --deep`), verifies, zips it (`macos-app.yml`) | PRs into master, manual runs |

A pull request into `develop` therefore costs one compile-and-test job; the platform and package
builds happen when a release or hotfix branch asks to enter `master`. To try them earlier, run
*Actions › CI › Run workflow* on the branch (all platform jobs, about 15 minutes), or *Actions ›
Release › Run workflow* (builds the downloads as artifacts, or with *from_commit* = a pull
request's head commit fetches and checks that pull request's packages; it never tags or
publishes).

Dependabot opens monthly grouped updates (cargo, GitHub Actions) against `develop`; macroquad,
egui-macroquad, egui and egui-phosphor are pinned and updated by hand.

`cargo xtask` (crates/xtask) runs the same tasks locally: `check-version BRANCH`,
`check-branch HEAD BASE`, `release-notes X.Y.Z` (no engine: builds in seconds), and
`cargo xtask-dist [--out DIR]` (xtask with the `dist` feature; needs `cargo build --release`
first; refuses binaries that need glibc > 2.35).

## Checks before merging

CI runs them on every pull request; run them locally before pushing:

```sh
cargo build --workspace                      # also rebuilds rustpg2d-game for editor play-tests
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

- `example_project/` and the editor's starter asset library (`crates/editor/assets/starter_library/`)
  are plain files, edited with the editor (or through its AI agent server). An asset the example
  gains or changes goes to the starter library too (*Send to library*, or `library_send` with
  `replace`); the tests check that both agree and that the German translation is complete.
  Never delete or commit `example_project/saves/` (it is ignored; it may hold the user's saves).
- Build the game in release mode (`cargo build --release --workspace`) before testing
  *File → Export game* or `cargo xtask-dist`.

## Commit messages

- A short summary line, then a blank line and bullet points for the notable changes.
- Release commits on release branches: `R(ust)PG2D <version>`.

## Project notes

- Map and definition files must stay loadable by older and newer versions: append new enum
  variants at the end, and give new fields `#[serde(default)]`. `crates/engine/tests/fixtures/`
  holds files of earlier versions that the tests load; add the last release's file there when a
  format changes.
- Tiles are always 32 px (`project::TILE_SIZE`).
- Features are verified end to end with scripted runs: `RUSTPG2D_AUTOPLAY` for the game and
  `RUSTPG2D_EDITOR_SCRIPT` for the editor (see `crates/game/src/main.rs` and
  `crates/editor/src/main.rs` for the commands), plus screenshots. Run destructive tests on a
  copy of `example_project`, and set `RUSTPG2D_EDITOR_CONFIG` / `RUSTPG2D_LIBRARY` to scratch folders
  so the user's recent-projects list and asset library are not touched.
