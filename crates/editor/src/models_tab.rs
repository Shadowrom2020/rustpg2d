//! 3D models in the editor: importing `.glb` files as models (their clips matched to the
//! animations by name), extra clip files, replacing a model's file. The form is
//! `database::model_form`.

use rustpg2d_engine::defs::ModelDef;
use rustpg2d_engine::model::{guess_clips, Model};

use crate::app::Editor;
use crate::database::Tab;

impl Editor {
    /// Uses `rel` (a `.glb` under the project) as a new model (`id` empty), as model `id`'s file,
    /// or (`clips`) as a file with more clips for it. Returns the status message.
    pub fn use_model_file(&mut self, id: &str, clips: bool, rel: &str) -> String {
        let model = match std::fs::read(self.project.root.join(rel)).map_err(anyhow::Error::from).and_then(|b| Model::load(&b)) {
            Ok(m) => m,
            Err(e) => return format!("{rel} can't be used as a model: {e:#}"),
        };
        let names: Vec<String> = model.clips.iter().map(|c| c.name.clone()).collect();
        let db = &mut self.project.db;
        let msg = if id.is_empty() {
            let stem = std::path::Path::new(rel).file_stem().unwrap_or_default().to_string_lossy().to_lowercase().replace([' ', '-'], "_");
            let base: String = stem.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            let new_id = rustpg2d_engine::refactor::unique_id(if base.is_empty() { "model" } else { &base }, |c| db.model(c).is_some());
            let mut name = new_id.replace('_', " ");
            if let Some(f) = name.get_mut(0..1) {
                f.make_ascii_uppercase();
            }
            let clips = guess_clips(&names);
            let mapped = clips.iter().map(|c| format!("{}={}", c.anim, c.clip)).collect::<Vec<_>>().join(", ");
            db.models.push(ModelDef { name, clips, ..ModelDef::new(&new_id, rel) });
            self.db.tab = Tab::Models;
            self.db.sel = db.models.len() - 1;
            format!(
                "Imported model {new_id}: {} triangles, {} clips{}{}",
                model.indices.len() / 3,
                names.len(),
                if mapped.is_empty() { String::new() } else { format!(" ({mapped})") },
                if model.joints.len() < 2 { " - no skeleton: give it a rig template" } else { "" }
            )
        } else {
            let Some(d) = db.models.iter_mut().find(|d| d.id == id) else { return format!("No model {id}") };
            if clips {
                if !d.clip_files.iter().any(|f| f == rel) {
                    d.clip_files.push(rel.to_string());
                }
                // Animations still without a clip take one of the new file's.
                let added: Vec<_> = guess_clips(&names).into_iter().filter(|c| d.clip_of(&c.anim).is_none()).collect();
                let n = added.len();
                d.clips.extend(added);
                format!("{id} plays the {} clip(s) of {rel}{}", names.len(), if n > 0 { format!("; {n} animation(s) mapped") } else { String::new() })
            } else {
                d.file = rel.to_string();
                format!("{id} now uses {rel}")
            }
        };
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        msg
    }

    /// Gives item `id` the 3D model `rel` (a `.glb` under the project): its hold point and handle
    /// direction estimated from its shape. Returns the status message.
    pub fn use_item_model(&mut self, id: &str, rel: &str) -> String {
        let model = match std::fs::read(self.project.root.join(rel)).map_err(anyhow::Error::from).and_then(|b| Model::load(&b)) {
            Ok(m) => m,
            Err(e) => return format!("{rel} can't be used as a model: {e:#}"),
        };
        let mut pts = Vec::new();
        model.skin(&model.pose(None, 0.0), &mut pts);
        let points: Vec<_> = pts.into_iter().map(|p| p.0).collect();
        let (grip, axis) = rustpg2d_engine::model::estimate_grip(&points, &model.indices);
        let Some(item) = self.project.db.items.iter_mut().find(|d| d.id == id) else { return format!("No item {id}") };
        let size = item.model.as_ref().map_or(22.0, |m| m.size);
        item.model = Some(rustpg2d_engine::defs::ItemModel { file: rel.to_string(), grip, axis, size });
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        format!("{id} has the 3D model {rel} ({} triangles); check where it is held in its form", model.indices.len() / 3)
    }

    /// Renders item `id`'s inventory icon from its 3D model into the next free 32 x 32 cell of
    /// `sprites/icons_3d.png` (sheet `icons_3d`, made when missing) and makes it the item's icon.
    pub fn render_item_icon(&mut self, id: &str) -> String {
        use rustpg2d_engine::defs::{SheetDef, SpriteRef};
        use rustpg2d_engine::imageio::Rgba;
        const CELL: u32 = 32;
        const COLS: u32 = 8;
        let Some(spec) = self.project.db.item(id).and_then(|d| d.model.clone()) else { return format!("{id} has no 3D model") };
        let Some(icon) = self.assets.models.render_item_icon(&spec, CELL) else { return format!("{id}'s model isn't loaded") };
        let rel = "sprites/icons_3d.png";
        let path = self.project.root.join(rel);
        let mut img = std::fs::read(&path).ok().and_then(|b| Rgba::decode(&b).ok()).unwrap_or_else(|| Rgba::new(CELL * COLS, CELL));
        // The first empty cell, else a new row.
        let cells = (img.w / CELL) * (img.h / CELL);
        let empty = |img: &Rgba, k: u32| (0..CELL).all(|y| (0..CELL).all(|x| img.get(((k % COLS) * CELL + x) as i32, ((k / COLS) * CELL + y) as i32)[3] == 0));
        let k = match (0..cells).find(|k| empty(&img, *k)) {
            Some(k) => k,
            None => {
                let mut bigger = Rgba::new(img.w, img.h + CELL);
                for y in 0..img.h as i32 {
                    for x in 0..img.w as i32 {
                        bigger.set(x, y, img.get(x, y));
                    }
                }
                img = bigger;
                cells
            }
        };
        for y in 0..CELL {
            for x in 0..CELL {
                let i = ((y * CELL + x) * 4) as usize;
                let px = [icon.bytes[i], icon.bytes[i + 1], icon.bytes[i + 2], icon.bytes[i + 3]];
                img.set(((k % COLS) * CELL + x) as i32, ((k / COLS) * CELL + y) as i32, px);
            }
        }
        let png = match img.encode() {
            Ok(p) => p,
            Err(e) => return format!("Writing the icon failed: {e:#}"),
        };
        if let Err(e) = std::fs::create_dir_all(path.parent().unwrap()).and_then(|_| std::fs::write(&path, png)) {
            return format!("Writing {rel} failed: {e}");
        }
        let db = &mut self.project.db;
        if db.sheet("icons_3d").is_none() {
            db.sheets.push(SheetDef::new("icons_3d", rel, CELL, CELL));
        }
        if let Some(d) = db.items.iter_mut().find(|d| d.id == id) {
            d.icon = SpriteRef::new("icons_3d", k);
        }
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        format!("{id}'s icon is rendered from its model (icons_3d frame {k})")
    }
}
