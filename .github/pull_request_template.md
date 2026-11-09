## What and why

<!-- What does this change, and why? Link issues with "Fixes #123". -->

## Branch

- [ ] `feature/<name>` or `bugfix/<name>` into `develop`
- [ ] `release/X.Y.Z` into `master` (version bump in Cargo.toml + CHANGELOG section only)
- [ ] `hotfix/X.Y.Z` into `master` (urgent fix, PATCH bump)

## How it was tested

<!-- Scripted game / editor runs (RUSTPG2D_AUTOPLAY, RUSTPG2D_EDITOR_SCRIPT), screenshots, unit tests. -->

## Checklist

- [ ] `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace` pass
- [ ] CHANGELOG.md (*Unreleased*), README.md and Documentation/scripting.md updated where users see a change
- [ ] Old maps, definitions and save games still load (new enum variants appended, new fields have serde defaults)
- [ ] Example assets that changed are in `crates/editor/assets/starter_library/` too
