# AGENTS.md

Guidelines for anyone (people and coding agents) working on R(ust)PG2D: a Rust 2D top-down RPG
engine (`crates/engine`), the game runner (`crates/game`) and the editor (`crates/editor`).

## Branching concept

GitHub Flow: one long-lived branch, `master`, and short-lived branches that merge into it by
pull request. A release is a pull request too; merging it tags and publishes the version.

```
feature/<name>, bugfix/<name>   o---o       o---o
                               /     \ PR  /     \ PR
master         ---o-----------o-------o---o-------o-----o---
                                       \         / PR     tag v0.2.0
release/0.2.0                           o-------o
```

| Branch | Created from | Pull request into | Purpose |
|---|---|---|---|
| `master` | – | – | The trunk: everything finished. Always builds and passes CI. Releases are its tagged commits (`vX.Y.Z`). |
| `feature/<name>` | `master` | `master` | One feature, e.g. `feature/quest-journal`. |
| `bugfix/<name>` | `master` | `master` | One fix, e.g. `bugfix/gate-spawn`; an urgent one is followed by a patch release. |
| `release/X.Y.Z` | `master` | `master` | Prepares a release: version bump, CHANGELOG section, small fixes found while preparing it. No new features. Merging it publishes the release. |
| `dependabot/*` | – | `master` | Dependency updates opened by Dependabot. |

Note: this repository's main branch is called `master` (not `main`). Until 0.18.2 it used
git-flow with a `develop` branch, hotfix branches and back-merge pull requests; those are gone.

### Rules

1. **`master` is protected** (a GitHub ruleset, for everyone including admins): no direct pushes,
   no force pushes, no deletion. Changes arrive by **pull request** only, merged with a **merge
   commit** (squash and rebase merges are disabled) once the required CI checks pass and the
   branch is up to date. No review is required (one developer).
2. **CI decides which branch may go in** (`cargo xtask check-branch`, the *Branch policy*
   check): `master` takes `feature/*`, `bugfix/*`, `release/X.Y.Z` and `dependabot/*`.
3. Keep a branch focused on one topic; GitHub deletes it after the merge.
4. **Releases** are cut on demand from `master`. The branch name carries the version, and CI
   (*Version* check, `cargo xtask check-version`) refuses the pull request unless the version is
   valid SemVer, above the latest release, equal to `[workspace.package] version` in
   `Cargo.toml`, has a `## X.Y.Z — date` section in CHANGELOG.md and no `## Unreleased` is left.
   A patch release (fixes only) is an ordinary `release/X.Y.Z` that raises PATCH.
5. **Merging a release pull request publishes it** (`.github/workflows/release.yml`, which runs on
   every push to master and acts only when the version in `Cargo.toml` has no tag yet): it takes
   the R(ust)PG2D AppImage, Flatpak, Windows zip and macOS app that CI built for the merged pull
   request (master only takes up-to-date branches, so the merge commit has exactly that tree; it
   builds them again only when they can't be found, e.g. expired), tags the merge commit
   `vX.Y.Z` and publishes a GitHub Release (notes = the CHANGELOG section, with the packages and
   SHA256SUMS). There is nothing to merge back.
6. Push and open pull requests only when asked to; merge only when asked to.
7. Before starting work, check which branch is checked out (`git branch --show-current`) and
   that the working tree is clean. If the current branch is `master`, create a branch first.

### Versions: Semantic Versioning 2.0

Versions are `MAJOR.MINOR.PATCH` ([semver.org](https://semver.org)), in `[workspace.package]
version` of the root `Cargo.toml`, tagged `vX.Y.Z`:

- **MAJOR** — incompatible changes: projects, maps, definitions or save games of the previous
  version no longer load (the compatibility rules below make this rare), or a script API is
  removed or changes meaning.
- **MINOR** — new features (the usual release); backwards compatible.
- **PATCH** — fixes only.
- **Pre-releases** `X.Y.Z-rc.N` are allowed (`release/0.3.0-rc.1`); GitHub marks them as
  pre-releases. Build metadata (`+…`) is not used.
- While MAJOR is 0, MINOR releases may still contain breaking changes; say so in the CHANGELOG.

### Commands

```sh
# Start a feature or bugfix
git checkout master && git pull --ff-only
git checkout -b feature/<name>               # or bugfix/<name>
# ... commit, then publish and open the pull request
git push -u origin feature/<name>
gh pr create --base master --fill
gh pr checks --watch                         # wait for CI
gh pr merge --merge                          # (--auto: merge as soon as CI passes)

# Cut a release (MINOR for features, PATCH for fixes only)
git checkout master && git pull --ff-only
git checkout -b release/0.2.0
#   set [workspace.package] version = "0.2.0" in Cargo.toml (cargo build updates Cargo.lock),
#   rename "## Unreleased" in CHANGELOG.md to "## 0.2.0 — YYYY-MM-DD", update "Current version" in README.md
git commit -am "R(ust)PG2D 0.2.0"
git push -u origin release/0.2.0
gh pr create --base master --title "R(ust)PG2D 0.2.0" --fill
gh pr merge --merge                          # after CI: the Release workflow tags and publishes
```

## Continuous integration

`.github/workflows/ci.yml` runs on every pull request into `master` and on pushes to `master`
(Ubuntu 22.04, so the binaries need only glibc ≤ 2.35). *Build, lint, test* and *Runtime smoke*
only report success, without running, on a push to `master` whose tree is the merged pull
request's (that pull request tested it). Every job uses the Rust version pinned in
`rust-toolchain.toml` (also `rust-version` in `Cargo.toml` and the `toolchain:` inputs of the
workflows; an xtask test keeps them equal), so a new stable release never breaks a pull request
by surprise: bump the three together in a `feature/` branch. Its jobs are the required checks
(a skipped job counts as passed):

| Check | What it does | Runs for |
|---|---|---|
| *Branch policy* | `cargo xtask check-branch HEAD BASE` (seconds: xtask without the engine) | every PR (required) |
| *Build, lint, test* | `cargo clippy --all-targets --all-features -D warnings`, `cargo test` (workspace, `--locked`, unoptimized: fast to compile) | every PR (required) |
| *Runtime smoke* | `scripts/ci/smoke.sh`: the game and the editor on a copy of the example project under Xvfb (title, a map, the clock, an old save, quick save, a map change + autosave, the editor opening a map, the MCP smoke client) | every PR (advisory) |
| *Version* | `cargo xtask check-version BRANCH` | release PRs (required) |
| *Release builds* | `builds.yml`, all at the same time: macOS editor + game per architecture (two `macos-latest` jobs), Linux editor + game + the packaging tool (`xtask` with `dist`), the web game, Windows editor + game (MinGW-w64) | release PRs, manual runs |
| *Packages* | assembles the release builds without compiling: universal macOS binaries (`llvm-lipo`), `xtask dist` (AppImage, Flatpak, Windows zip, unsigned macOS app), smoke test of game and editor under Xvfb, downloads as artifact (kept 30 days: the release publishes them) | release PRs (required) |
| *Windows* | clippy and tests on `windows-latest` (MSVC) | release PRs, manual runs |
| *macOS* | clippy for editor and game, the engine's tests on `macos-latest` | release PRs, manual runs |
| *macOS app* | signs the app ad hoc (`codesign --deep`), verifies, zips it (`macos-app.yml`) | release PRs, manual runs |

A feature or fix pull request therefore costs one compile-and-test job; the platform and package
builds happen when a release branch asks to enter `master`. To try them earlier, run
*Actions › CI › Run workflow* on the branch (all platform jobs, about 15 minutes), or *Actions ›
Release › Run workflow* (builds the downloads as artifacts, or with *from_commit* = a release
pull request's head commit fetches and checks that pull request's packages; it never tags or
publishes).

Dependabot opens monthly grouped updates (cargo, GitHub Actions) against `master`; macroquad,
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

- **Content first.** New features, fixes and workflows are built with the engine's own tools
  (events, Rhai scripts, definitions, structures, assets) and land in the editor's starter asset
  library, and in the example project when they should be seen in play. The Rust code changes only
  when the engine can't do it, and only after the maintainer agreed to the proposed (small,
  generic) addition. CLAUDE.md (*Content route*) describes the workflow.
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
