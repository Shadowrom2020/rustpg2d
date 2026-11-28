# Compatibility fixtures

Files written by 0.35; `tests/compat.rs` loads them, so a change that stops them from loading fails
the tests. Projects of versions before 0.35 are not read any more (0.35 converts them; this
version refuses them with the message `project::MADE_BEFORE_035`).

| Fixture | Origin |
|---|---|
| `project-0.35.0/` | `project.ron` and `defs/` of the example project at tag v0.35.0-alpha.1 (`git show v0.35.0-alpha.1:example_project/…`): they load with no entry left out (`definitions_of_0_35_0_load`); items still have their `held` pictures and spells `effect: Parts`, fields this version drops when it writes them |
| `maps/village-0.35.0.map` | the example's village at tag v0.35.0-alpha.1 (map format version 3) |
| `saves/save-0.35.0.ron` | a quick save written by the game of tag v0.35.0-alpha.1 (built from `git archive`), in the house with `fixture_flag`, `fixture_var` = 7 and two potions (save layout 2); the smoke run (`scripts/ci/smoke.sh`) loads it too |
| `saves/minimal.ron` | only the fields of a save without a default |
| `project-0.27.0/project.ron` | the example's `project.ron` at tag v0.27.0, without a rules version: an older project, refused (`projects_before_0_35_are_refused`) |
| `model_two_bones.glb` | a two-bone skinned model for the model loader's tests (not a version's file) |

Add a fixture when a format changes (take the file from the last release's tag), never edit one.
