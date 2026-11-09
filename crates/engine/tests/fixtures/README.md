# Compatibility fixtures

Files written by earlier versions; `tests/compat.rs` loads them, so a change that stops them from
loading fails the tests.

| Fixture | Origin |
|---|---|
| `project-0.9.0/` | `project.ron` and `defs/` of the example project at tag v0.9.0 (`git show v0.9.0:example_project/…`); four characters still carry a pre-drawn `sheet` |
| `maps/house-0.9.0.map`, `shop-0.9.0.map`, `village-0.9.0.map` | example maps at v0.9.0 (format version 2) |
| `maps/mine-0.10.0.map` | the mine at v0.10.0 |
| `maps/v1-synthetic.map` | a format version 1 map built by the `reads_version_1` test (v1 never shipped; the file freezes the postcard layout of `TileLayer` / `Chunk`, which v2 uses too). `RUSTPG2D_WRITE_FIXTURES=1 cargo test -p rustpg2d-engine reads_version_1` rewrites it |
| `saves/save-0.11.0.ron` | a quick save written by 0.11.0 (the save layout is unchanged since 0.10.0; no `version` field yet) |
| `saves/minimal.ron` | only the fields without a default: the oldest save layout |

Add a fixture when a format changes (take the file from the last release's tag), never edit one.
