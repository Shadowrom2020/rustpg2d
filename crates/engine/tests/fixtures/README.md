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
| `saves/save-0.12.0.ron` | a quick save written by the 0.12.0 game (built from tag v0.12.0) with the town guard as a hurt companion: before companions had mana and learned spells |
| `project-0.16.0/` | `project.ron` and `defs/` of the example project at tag v0.16.0: before races (no `races.ron`, no `race` on characters, no body `cut`) |
| `project-0.20.0/`, `maps/village-0.20.0.map`, `maps/mine-0.20.0.map` | `project.ron`, `defs/` and two maps of the example project at tag v0.20.0, the last release with tiles (`tilesets.ron`, format version 2 tile maps): they convert into a painted project (`projects_of_0_20_0_convert`) |
| `quests/quests-0.21.0.ron` | the example's `defs/quests.ron` at tag v0.21.0: quests driven by triggers, before the quest designer (no giver, offer, steps, rewards, objects) |
| `saves/save-0.22.0.ron` | a quick save in Tilda's shop written by the game of the 0.22.0 save layout (built from `master` after #188, which changed no save fields): before homes and placed furniture |
| `maps/forest-0.25.0.map` | the example's forest at tag v0.25.0: placed characters before a faction of their own (`EntityInstance::faction`) |
| `saves/minimal.ron` | only the fields without a default: the oldest save layout |

Add a fixture when a format changes (take the file from the last release's tag), never edit one.
