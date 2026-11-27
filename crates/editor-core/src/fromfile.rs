//! New definitions from files: a 3D model or a picture dropped into the editor becomes a prop
//! that shows it, a picture a sprite sheet, a sound file a sound. The file is copied into the
//! project (its folder for that kind of art), the definition is made around it.

use std::path::Path;

use rustpg2d_engine::refactor::RefKind;

use crate::Workspace;

/// What a file is, by its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// A 3D model (`.glb`).
    Model,
    /// A picture (`.png`).
    Picture,
    /// A sound (`.ogg`, `.wav`).
    Sound,
}

impl FileKind {
    pub fn of(file: &Path) -> Option<FileKind> {
        match file.extension()?.to_str()?.to_lowercase().as_str() {
            "glb" => Some(FileKind::Model),
            "png" => Some(FileKind::Picture),
            "ogg" | "wav" => Some(FileKind::Sound),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Model => "a 3D model",
            FileKind::Picture => "a picture",
            FileKind::Sound => "a sound",
        }
    }

    /// The kinds of definitions such a file can become, the likeliest first.
    pub fn makes(self) -> &'static [RefKind] {
        match self {
            FileKind::Model => &[RefKind::Prop],
            FileKind::Picture => &[RefKind::Prop, RefKind::Sheet],
            FileKind::Sound => &[RefKind::Sound],
        }
    }
}

/// What a new definition of `kind` made from `file` will be, in words (None: that kind can't
/// be made from such a file).
pub fn what_it_makes(kind: RefKind, file: &Path) -> Option<&'static str> {
    match (kind, FileKind::of(file)?) {
        (RefKind::Prop, FileKind::Model) => Some("A prop that shows this 3D model, standing on the ground in the middle of its footprint, one tile wide. Its pictures for lists are drawn from the model."),
        (RefKind::Prop, FileKind::Picture) => Some("A prop with this picture, standing on its lower edge, and the sprite sheet that holds the picture. A large picture is shown smaller (the sheet's scale)."),
        (RefKind::Sheet, FileKind::Picture) => Some("A sprite sheet of this picture as one frame: set the frame's size in its form when it holds several."),
        (RefKind::Sound, FileKind::Sound) => Some("A sound that plays this file."),
        _ => None,
    }
}

/// "old_oak-tree 2.png" -> "Old oak tree 2": a file's name as a name to start with.
pub fn name_from_file(file: &Path) -> String {
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut name: String = stem.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(f) = name.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    name
}

/// How many image pixels a world pixel of a picture this wide gets: pixel art (up to three
/// tiles wide) is shown as it is, larger pictures at most about three tiles wide.
fn scale_for(width: u32) -> u32 {
    width.div_ceil(96).clamp(1, 16)
}

/// How wide a picture stands on the ground, in its pixels: the span of what is painted in its
/// lowest part (a tree's trunk, not its crown; a crate's whole width). The picture's width
/// when nothing is painted.
fn foot_width(p: &rustpg2d_engine::imageio::Rgba) -> u32 {
    let painted = |x: u32, y: u32| p.get(x as i32, y as i32)[3] > 40;
    let rows: Vec<u32> = (0..p.h).filter(|&y| (0..p.w).any(|x| painted(x, y))).collect();
    let (Some(&top), Some(&bottom)) = (rows.first(), rows.last()) else { return p.w };
    // (The lowest eighth of what is painted, a few rows at least.)
    let from = bottom.saturating_sub(((bottom - top) / 8).max(2)).max(top);
    let xs: Vec<u32> = (0..p.w).filter(|&x| (from..=bottom).any(|y| painted(x, y))).collect();
    match (xs.first(), xs.last()) {
        (Some(a), Some(b)) => b - a + 1,
        _ => p.w,
    }
}

impl Workspace {
    /// A new definition of `kind` called `name` from `file` (copied into the project). One
    /// step of the history, saved when the project is (only the files are in its folder at
    /// once).
    /// Returns its id.
    pub fn make_from_file(&mut self, kind: RefKind, file: &Path, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("it needs a name".into());
        }
        let what = FileKind::of(file).ok_or_else(|| format!("{} is no 3D model (.glb), picture (.png) or sound (.ogg, .wav)", file.display()))?;
        if what_it_makes(kind, file).is_none() {
            return Err(format!("{} can't become a {}: it can become {}", what.label(), kind.label().to_lowercase(), what.makes().iter().map(|k| format!("a {}", k.label().to_lowercase())).collect::<Vec<_>>().join(" or ")));
        }
        if !file.is_file() {
            return Err(format!("there is no file {}", file.display()));
        }
        let root = self.project.root.clone();
        let label = format!("New {} from {}: {name}", kind.label().to_lowercase(), file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default());
        let set = |ws: &mut Workspace, kind: RefKind, id: &str, change: &dyn Fn(&mut serde_json::Value)| -> Result<(), String> {
            let mut value = ws.def_value(kind, id).ok_or_else(|| format!("{id} could not be made"))?;
            change(&mut value);
            ws.set_def_value(kind, id, value)
        };
        let id = match (kind, what) {
            (RefKind::Sound, _) => {
                let rel = crate::files::import_asset(&root, file, "sounds").map_err(|e| format!("{e:#}"))?;
                let id = self.make_def(kind, name, None)?;
                set(self, kind, &id, &|v| v["file"] = rel.clone().into())?;
                id
            }
            (_, FileKind::Picture) => {
                let bytes = std::fs::read(file).map_err(|e| format!("reading {}: {e}", file.display()))?;
                let picture = rustpg2d_engine::imageio::Rgba::decode(&bytes).map_err(|e| format!("{}: {e:#}", file.display()))?;
                let rel = crate::files::import_asset(&root, file, "sprites").map_err(|e| format!("{e:#}"))?;
                let scale = scale_for(picture.w);
                // The sheet: the whole picture as one frame.
                let sheet = if kind == RefKind::Sheet {
                    self.make_def(RefKind::Sheet, name, None)?
                } else {
                    let id = rustpg2d_engine::refactor::unique_id(&format!("{}_picture", crate::defs::id_from_name(name)), |c| self.project.db.has(RefKind::Sheet, c));
                    self.new_def(RefKind::Sheet, &id)?;
                    id
                };
                set(self, RefKind::Sheet, &sheet, &|v| {
                    v["image"] = rel.clone().into();
                    v["frame_w"] = picture.w.into();
                    v["frame_h"] = picture.h.into();
                    v["scale"] = scale.into();
                })?;
                if kind == RefKind::Sheet {
                    sheet
                } else {
                    // The prop: as wide on the ground as the foot of its picture, half as deep
                    // (the creator drags the corners in the workshop to what really stands there).
                    let id = self.make_def(RefKind::Prop, name, None)?;
                    let wide = ((foot_width(&picture) as f32 / scale as f32 / 4.0).round() * 4.0).clamp(8.0, 256.0);
                    let deep = ((wide / 2.0 / 4.0).round() * 4.0).max(8.0);
                    set(self, RefKind::Prop, &id, &|v| {
                        v["sprite"]["sheet"] = sheet.clone().into();
                        v["sprite"]["frame"] = 0.into();
                        v["hitbox"] = serde_json::json!([wide, deep]);
                    })?;
                    id
                }
            }
            (_, FileKind::Model) => {
                let bytes = std::fs::read(file).map_err(|e| format!("reading {}: {e}", file.display()))?;
                let (glb, size) = rustpg2d_engine::model::stand_on_ground(&bytes).map_err(|e| format!("{}: {e:#}", file.display()))?;
                let id = self.make_def(RefKind::Prop, name, None)?;
                std::fs::create_dir_all(root.join("models")).map_err(|e| e.to_string())?;
                let mut rel = format!("models/{id}_3d.glb");
                let mut n = 2;
                while root.join(&rel).exists() {
                    rel = format!("models/{id}_3d_{n}.glb");
                    n += 1;
                }
                std::fs::write(root.join(&rel), glb).map_err(|e| format!("writing {rel}: {e}"))?;
                // One tile wide, as deep as the model is (`show_prop_model` sees to the depth).
                set(self, RefKind::Prop, &id, &|v| v["hitbox"] = serde_json::json!([32.0, 16.0]))?;
                self.show_prop_model(&id, &rel, size).map_err(|e| format!("{e:#}"))?;
                id
            }
            (_, FileKind::Sound) => return Err("a sound file becomes a sound".into()),
        };
        self.begin(&label);
        if what != FileKind::Model {
            // (`show_prop_model` loaded the art again already.)
            self.reload_assets();
        }
        self.refresh_refs();
        self.stale = true;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    #[test]
    fn files_are_told_apart_and_named() {
        assert_eq!(FileKind::of(Path::new("/x/Old_Oak-tree 2.PNG")), Some(FileKind::Picture));
        assert_eq!(FileKind::of(Path::new("barrel.glb")), Some(FileKind::Model));
        assert_eq!(FileKind::of(Path::new("door.ogg")), Some(FileKind::Sound));
        assert_eq!(FileKind::of(Path::new("notes.txt")), None);
        assert_eq!(name_from_file(Path::new("/x/old_oak-tree 2.png")), "Old oak tree 2");
        assert!(what_it_makes(RefKind::Prop, Path::new("a.glb")).is_some() && what_it_makes(RefKind::Sheet, Path::new("a.glb")).is_none());
        assert!(what_it_makes(RefKind::Sound, Path::new("a.png")).is_none());
        assert_eq!((scale_for(32), scale_for(96), scale_for(97), scale_for(512)), (1, 1, 2, 6));
    }

    #[test]
    fn a_picture_a_model_and_a_sound_become_definitions() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-fromfile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let project = dir.join("game");
        copy(&root.join("example_project"), &project);
        let mut ws = Workspace::new(project.clone(), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        let before = ws.project.db.clone();
        // A picture from outside the project, four tiles wide: a prop standing on it and its sheet.
        let outside = dir.join("Stone_arch.png");
        let mut img = rustpg2d_engine::imageio::Rgba::new(128, 96);
        for y in 0..96 {
            for x in 0..128 {
                // (An arch: a beam on two legs that stand at its ends.)
                if y < 24 || !(16..112).contains(&x) {
                    img.set(x, y, [200, 190, 180, 255]);
                }
            }
        }
        std::fs::write(&outside, img.encode().unwrap()).unwrap();
        let id = ws.make_from_file(RefKind::Prop, &outside, &name_from_file(&outside)).unwrap();
        ws.history_frame(None);
        assert_eq!(id, "stone_arch");
        let prop = ws.project.db.prop(&id).unwrap().clone();
        assert_eq!((prop.name.as_str(), prop.sprite.sheet.as_str(), prop.hitbox), ("Stone arch", "stone_arch_picture", [64.0, 32.0]));
        let sheet = ws.project.db.sheets.iter().find(|s| s.id == "stone_arch_picture").unwrap();
        assert_eq!((sheet.image.as_str(), sheet.frame_w, sheet.frame_h, sheet.scale), ("sprites/Stone_arch.png", 128, 96, 2));
        assert!(project.join("sprites/Stone_arch.png").is_file());
        assert_eq!(ws.timeline.done.len(), 1, "{:?}", ws.timeline.done.iter().map(|c| &c.label).collect::<Vec<_>>());
        assert_eq!(ws.timeline.done[0].label, "New prop from Stone_arch.png: Stone arch");
        // Undone, the definitions are as they were (the copied file stays: unused files are cleaned up elsewhere).
        ws.undo().unwrap();
        assert!(ws.project.db == before);
        // A tree stands on its trunk, not on the width of its crown.
        let mut tree = rustpg2d_engine::imageio::Rgba::new(64, 80);
        for y in 0..80 {
            for x in 0..64 {
                if y < 50 || (26..38).contains(&x) {
                    tree.set(x, y, [60, 130, 60, 255]);
                }
            }
        }
        assert_eq!(foot_width(&tree), 12);
        assert_eq!(foot_width(&rustpg2d_engine::imageio::Rgba::new(40, 40)), 40, "nothing painted: its width");
        let oak = dir.join("oak.png");
        std::fs::write(&oak, tree.encode().unwrap()).unwrap();
        let id = ws.make_from_file(RefKind::Prop, &oak, "Young oak").unwrap();
        ws.history_frame(None);
        assert_eq!(ws.project.db.prop(&id).unwrap().hitbox, [12.0, 8.0]);
        // The same picture as a sheet of its own.
        let sheet = ws.make_from_file(RefKind::Sheet, &outside, "Arch tiles").unwrap();
        ws.history_frame(None);
        assert!(ws.project.db.sheets.iter().any(|s| s.id == sheet && s.frame_w == 128) && ws.project.db.prop("arch_tiles").is_none());
        // A 3D model of the project: a prop showing it, as deep as the model is.
        let glb = std::fs::read_dir(project.join("models")).unwrap().flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with("_3d.glb"))).expect("the example has a prop's model");
        let model = dir.join("crate.glb");
        std::fs::copy(&glb, &model).unwrap();
        let id = ws.make_from_file(RefKind::Prop, &model, "Old crate").unwrap();
        ws.history_frame(None);
        let prop = ws.project.db.prop(&id).unwrap();
        assert_eq!(id, "old_crate");
        assert!(!prop.model.is_empty() && ws.project.db.model(&prop.model).is_some_and(|m| m.file == "models/old_crate_3d.glb" && m.height > 4.0));
        assert!(project.join("models/old_crate_3d.glb").is_file() && prop.hitbox[0] == 32.0 && prop.hitbox[1] >= 16.0);
        // It is a change like the others: nothing was saved for it, and undone it is gone.
        assert!(ws.is_dirty() && rustpg2d_engine::project::Project::load(&project).unwrap().db.prop("old_crate").is_none());
        ws.undo().unwrap();
        ws.history_frame(None);
        assert!(ws.project.db.prop("old_crate").is_none() && !ws.project.db.models.iter().any(|m| m.file == "models/old_crate_3d.glb"));
        ws.redo().unwrap();
        ws.history_frame(None);
        assert!(ws.project.db.prop("old_crate").is_some_and(|p| !p.model.is_empty()));
        // A sound; and what can't be.
        let ogg = std::fs::read_dir(project.join("sounds")).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "ogg")).expect("the example has sounds");
        let sound = dir.join("creak.ogg");
        std::fs::copy(&ogg, &sound).unwrap();
        let id = ws.make_from_file(RefKind::Sound, &sound, "Door creak").unwrap();
        assert!(ws.project.db.sounds.iter().any(|s| s.id == id && s.file == "sounds/creak.ogg"));
        assert!(ws.make_from_file(RefKind::Sound, &outside, "X").is_err());
        assert!(ws.make_from_file(RefKind::Prop, &dir.join("missing.png"), "X").is_err());
        assert!(ws.make_from_file(RefKind::Prop, &outside, " ").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
