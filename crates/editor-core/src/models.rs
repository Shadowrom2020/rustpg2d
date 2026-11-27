//! 3D models: importing `.glb` files as models (their clips matched to the animations by name),
//! extra clip files, replacing a model's file, rig templates, item models and their icons.

use rustpg2d_engine::defs::{ClipMap, Database, JointNudge, ModelDef, RigFit};
use gfx::math::Vec3;
use rustpg2d_engine::model::{guess_clips, Model};

use crate::workspace::Workspace;

/// Gives model `d` the rig template `template` ("" = none), keeping its turn (joints moved for
/// another template start over). Without clips of its own (or with the old template's) it plays
/// the template's.
pub fn choose_template(d: &mut ModelDef, db: &Database, template: &str) {
    let old = d.rig.as_ref().map(|r| r.template.clone()).unwrap_or_default();
    if template.is_empty() {
        d.rig = None;
        return;
    }
    let old_clips = db.model(&old).map(|t| t.clips.clone());
    let (turn, joints) = d.rig.as_ref().map_or((0.0, Vec::new()), |r| (r.turn, if r.template == template { r.joints.clone() } else { Vec::new() }));
    d.rig = Some(RigFit { template: template.to_string(), turn, joints });
    if d.clips.is_empty() || old_clips.as_ref() == Some(&d.clips) {
        d.clips = db.model(template).map(|t| t.clips.clone()).unwrap_or_default();
    }
}

/// Animations the clips kind adds to a model (Meshy library actions): the weapon animations items
/// name and sneaking. (name, action, the clip's name in Meshy's file without `_` and in lower
/// case, frames a second, hit point).
pub const CLIP_CHOICES: [(&str, u32, &str, f32, Option<f32>); 11] = [
    ("attack_1h", 219, "righthandswordslash", 12.0, None),
    ("attack_2h", 128, "heavyhammerswing", 12.0, Some(0.84)),
    ("attack_thrust", 240, "thrustslash", 12.0, Some(0.29)),
    ("attack_bow", 224, "archeryshot", 12.0, Some(0.6)),
    ("attack_cast", 129, "magesoellcast", 12.0, Some(0.55)),
    ("attack_unarmed", 192, "rightjabfromguard", 12.0, Some(0.33)),
    ("idle_1h", 89, "combatstance", 8.0, None),
    ("idle_2h", 85, "axestance", 8.0, None),
    ("block_shield", 138, "block1", 12.0, Some(0.15)),
    ("block_weapon", 147, "swordparry", 12.0, Some(0.3)),
    ("sneak", 559, "sneakywalk", 12.0, None),
];

impl Workspace {
    /// Compacts model file `rel` of the project in place (`model::compact`; kept when that fails
    /// or isn't smaller) and returns its bytes.
    pub fn compact_file(&self, rel: &str, keep_mesh: bool) -> anyhow::Result<Vec<u8>> {
        let path = self.project.root.join(rel);
        let bytes = std::fs::read(&path)?;
        match rustpg2d_engine::model::compact(&bytes, keep_mesh) {
            Ok(small) if small.len() < bytes.len() => {
                std::fs::write(&path, &small)?;
                Ok(small)
            }
            Ok(_) => Ok(bytes),
            Err(e) => {
                eprintln!("[models] {rel} not compacted: {e:#}");
                Ok(bytes)
            }
        }
    }

    /// Compacts every model file of the project: models, their clip files (without meshes), items'
    /// models. Returns what it saved.
    pub fn compact_models(&mut self) -> String {
        let db = &self.project.db;
        let mut files: Vec<(String, bool)> = Vec::new();
        for m in &db.models {
            files.push((m.file.clone(), true));
            files.extend(m.clip_files.iter().map(|f| (f.clone(), false)));
        }
        files.extend(db.items.iter().filter_map(|i| i.model.as_ref()).map(|m| (m.file.clone(), true)));
        files.sort();
        files.dedup();
        let size = |f: &str| std::fs::metadata(self.project.root.join(f)).map_or(0, |m| m.len());
        let before: u64 = files.iter().map(|(f, _)| size(f)).sum();
        for (f, keep) in &files {
            if let Err(e) = self.compact_file(f, *keep) {
                eprintln!("[models] {f}: {e:#}");
            }
        }
        let after: u64 = files.iter().map(|(f, _)| size(f)).sum();
        self.reload_assets();
        format!("{} model files: {} KB -> {} KB", files.len(), before / 1024, after / 1024)
    }

    /// Shares animations of model `source` with model `target` (Database › 3D models › *Copy clips
    /// from*): the clip files they come from become `target`'s shared clips (fitted to its bones
    /// when it loads, `Model::add_clips_retargeted`) and the animations are mapped like the
    /// source's, hit points included. `anims` empty = every animation whose clip is in one of the
    /// source's clip files (clips in its model file can't be shared); animations `target` maps
    /// already are kept unless `replace`. Saves; returns what it did.
    pub fn share_clips(&mut self, target: &str, source: &str, anims: &[String], replace: bool) -> Result<String, String> {
        let db = &self.project.db;
        if target == source {
            return Err("a model can't copy clips from itself".into());
        }
        let src = db.model(source).ok_or_else(|| format!("no model \"{source}\""))?.clone();
        let mut d = db.model(target).ok_or_else(|| format!("no model \"{target}\""))?.clone();
        if d.rig.is_some() || !self.assets.models.info(target).is_some_and(|i| i.rigged) {
            return Err(format!("{target} has no skeleton of its own: it plays its rig template's clips"));
        }
        let read = |rel: &str| std::fs::read(self.project.root.join(rel)).map_err(|e| format!("{rel}: {e}"));
        let model = Model::load(&read(&d.file)?).map_err(|e| format!("{}: {e:#}", d.file))?;
        // The source's clip files: their clips, and whether the target has their bones.
        let mut files: Vec<(String, Vec<String>, bool)> = Vec::new();
        for f in src.clip_paths() {
            let bytes = read(f)?;
            let names = rustpg2d_engine::model::clip_names(&bytes).map_err(|e| format!("{f}: {e:#}"))?;
            let (hit, of) = model.bones_matched(&bytes).map_err(|e| format!("{f}: {e:#}"))?;
            files.push((f.clone(), names, of > 0 && hit * 10 >= of * 9));
        }
        let (mut added, mut kept, mut missing) = (Vec::new(), Vec::new(), Vec::new());
        for c in &src.clips {
            if !anims.is_empty() && !anims.contains(&c.anim) {
                continue;
            }
            let Some((file, _, fits)) = files.iter().find(|(_, names, _)| names.contains(&c.clip)) else {
                if !anims.is_empty() {
                    missing.push(format!("{} (in {source}'s own model file)", c.anim));
                }
                continue;
            };
            if !fits {
                missing.push(format!("{} ({target} lacks the bones of {file})", c.anim));
                continue;
            }
            if d.clip_of(&c.anim).is_some() && !replace {
                kept.push(c.anim.clone());
                continue;
            }
            if !d.clip_paths().any(|p| p == file) {
                d.shared_clips.push(file.clone());
            }
            d.clips.retain(|o| o.anim != c.anim);
            d.clips.push(c.clone());
            added.push(c.anim.clone());
        }
        for a in anims.iter().filter(|a| !src.clips.iter().any(|c| c.anim == **a)) {
            missing.push(format!("{a} ({source} has none)"));
        }
        if let Some(m) = self.project.db.models.iter_mut().find(|m| m.id == target) {
            *m = d;
        }
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        let mut msg = if added.is_empty() { format!("{target} took no animations from {source}") } else { format!("{target} plays {source}'s {}", added.join(", ")) };
        if !kept.is_empty() {
            msg += &format!("; kept its own {}", kept.join(", "));
        }
        if !missing.is_empty() {
            msg += &format!("; not copied: {}", missing.join(", "));
        }
        Ok(msg)
    }

    /// Adds clip file `rel` (clips `names`, rigged again by Meshy for model `id`) to the model's
    /// shared clips (fitted to its own bones) and maps the animations `anims` asked for to its
    /// clips (*Generate art › 3D model clips*). Saves; returns what it did.
    pub fn add_shared_clips(&mut self, id: &str, rel: &str, names: &[String], anims: &[String]) -> String {
        let squash = |n: &str| n.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
        let Some(d) = self.project.db.models.iter_mut().find(|m| m.id == id) else { return format!("No model {id}") };
        if !d.shared_clips.iter().any(|f| f == rel) {
            d.shared_clips.push(rel.to_string());
        }
        let mut mapped = Vec::new();
        for a in anims {
            let clip = match CLIP_CHOICES.iter().find(|c| c.0 == a) {
                // The shortest name holding it: Block1, not Block10.
                Some(c) => names.iter().filter(|n| squash(n).contains(c.2)).min_by_key(|n| n.len()).map(|n| ClipMap { anim: a.clone(), clip: n.clone(), fps: c.3, hit: c.4 }),
                None => guess_clips(names).into_iter().find(|c| c.anim == *a),
            };
            if let Some(c) = clip {
                d.clips.retain(|o| o.anim != c.anim);
                d.clips.push(c);
                mapped.push(a.clone());
            }
        }
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        format!("{id} plays the {} clip(s) of {rel} (fitted to its bones){}", names.len(), if mapped.is_empty() { String::new() } else { format!(": {}", mapped.join(", ")) })
    }

    /// Fits rig template `template` (None: keep, "" or "-": none) to model `id`, with `turn`
    /// (None: keep) and the moved joints `joints` (None: keep; offsets in parts of the height).
    /// Saves; returns the fitted joints (positions in parts of the height, from the feet).
    pub fn fit_rig(&mut self, id: &str, template: Option<&str>, turn: Option<f32>, joints: Option<Vec<JointNudge>>) -> Result<String, String> {
        let db = &self.project.db;
        let d = db.model(id).ok_or_else(|| format!("no model \"{id}\""))?;
        if let Some(t) = template.filter(|t| !t.is_empty() && *t != "-") {
            if t == id {
                return Err("a model can't be its own rig template".into());
            }
            db.model(t).ok_or_else(|| format!("no model \"{t}\" to use as rig template"))?;
            if !self.assets.models.info(t).is_some_and(|i| i.rigged) {
                return Err(format!("{t} has no skeleton of its own: a rig template must be rigged"));
            }
        }
        if self.assets.models.info(id).is_some_and(|i| i.rigged) {
            return Err(format!("{id} has its own skeleton: rig templates are for unrigged models"));
        }
        let mut d = d.clone();
        if let Some(t) = template {
            choose_template(&mut d, db, if t == "-" { "" } else { t });
        }
        if let Some(rig) = &mut d.rig {
            if let Some(turn) = turn {
                rig.turn = turn;
            }
            if let Some(j) = joints {
                rig.joints = j;
            }
        } else if turn.is_some() || joints.is_some() {
            return Err(format!("{id} has no rig template yet: give one"));
        }
        let rig = d.rig.clone();
        if let Some(m) = self.project.db.models.iter_mut().find(|m| m.id == id) {
            *m = d;
        }
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        let Some(rig) = rig else { return Ok(format!("{id} has no rig template now")) };
        let view = self.assets.models.rig_view(id).ok_or_else(|| self.assets.models.errors.iter().find(|e| e.contains(&format!("model {id}:"))).cloned().unwrap_or_else(|| format!("{id} didn't load")))?;
        let h = (view.hi.y - view.lo.y).max(1e-5);
        let list: Vec<String> = view.joints.iter().map(|(n, p, _)| {
            let q = (*p - view.lo) / h;
            format!("{n} ({:.2}, {:.2}, {:.2})", q.x - (view.hi.x - view.lo.x) / h / 2.0, q.y, q.z - (view.hi.z - view.lo.z) / h / 2.0)
        }).collect();
        Ok(format!(
            "{id} uses rig template {} (turned {} deg, {} joint(s) moved). Joints (x right, y up from the feet, z front; parts of the height, from the middle): {}",
            rig.template,
            rig.turn,
            rig.joints.len(),
            list.join(", ")
        ))
    }

    /// Uses `rel` (a `.glb` under the project) as a new model (`id` empty), as model `id`'s file,
    /// or (`clips`) as a file with more clips for it. Returns the status message.
    pub fn use_model_file(&mut self, id: &str, clips: bool, rel: &str) -> String {
        // Smaller first: textures as the game uses them; a clip file without its mesh.
        let bytes = match self.compact_file(rel, !clips) {
            Ok(b) => b,
            Err(e) => return format!("{rel} can't be used as a model: {e:#}"),
        };
        let names = match rustpg2d_engine::model::clip_names(&bytes) {
            Ok(n) => n,
            Err(e) => return format!("{rel} can't be used as a model: {e:#}"),
        };
        let model = if clips {
            None
        } else {
            match Model::load(&bytes) {
                Ok(m) => Some(m),
                Err(e) => return format!("{rel} can't be used as a model: {e:#}"),
            }
        };
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
            format!(
                "Imported model {new_id}: {} triangles, {} clips{}{}",
                model.as_ref().map_or(0, |m| m.indices.len() / 3),
                names.len(),
                if mapped.is_empty() { String::new() } else { format!(" ({mapped})") },
                if model.as_ref().is_some_and(|m| m.joints.len() < 2) { " - no skeleton: give it a rig template" } else { "" }
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

    /// Gives prop `prop` the still 3D model `rel` (a `.glb` under the project standing on the
    /// ground in its middle, `size` its width, height and depth in its units): a model definition
    /// `<prop>_3d` as tall as fits the prop's hitbox, the prop showing it, its pictures rendered
    /// from it. Returns the status message.
    pub fn use_prop_model(&mut self, prop: &str, rel: &str, size: gfx::math::Vec3) -> anyhow::Result<String> {
        let hitbox = self.project.db.prop(prop).map(|p| p.hitbox).ok_or_else(|| anyhow::anyhow!("the prop \"{prop}\" is gone"))?;
        self.compact_file(rel, true)?;
        // Drawn `height` px tall, its footprint is size.x / size.y * height wide: as wide as the
        // hitbox, which grows deeper when the model is (old props' hitboxes are often only their
        // feet), so collision and drawing order fit what is drawn.
        let fit = hitbox[0] / size.x.max(1e-3);
        let height = (size.y * fit).max(4.0).round();
        let depth = ((size.z * fit) / 2.0).round() * 2.0;
        let db = &mut self.project.db;
        let id = rustpg2d_engine::refactor::unique_id(&format!("{prop}_3d"), |c| db.model(c).is_some());
        let name = db.prop(prop).map_or(prop.to_string(), |p| p.name.clone());
        db.models.push(ModelDef { name: format!("{name} (3D)"), height, ..ModelDef::new(&id, rel) });
        if let Some(p) = db.props.iter_mut().find(|p| p.id == prop) {
            p.model = id.clone();
            p.hitbox[1] = p.hitbox[1].max(depth);
        }
        db.reindex();
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        let pictures = self.render_prop_pictures(Some(prop));
        Ok(format!("Prop {prop} shows the 3D model {id} ({height} px tall, on a {} x {} px footprint); {pictures}", hitbox[0], hitbox[1].max(depth)))
    }

    /// Gives item `id` the 3D model `rel` (a `.glb` under the project): its hold point and handle
    /// direction estimated from its shape. Returns the status message.
    pub fn use_item_model(&mut self, id: &str, rel: &str) -> String {
        let model = match self.compact_file(rel, true).and_then(|b| Model::load(&b)) {
            Ok(m) => m,
            Err(e) => return format!("{rel} can't be used as a model: {e:#}"),
        };
        let mut pts = Vec::new();
        model.skin(&model.pose(None, 0.0), &mut pts);
        let points: Vec<_> = pts.into_iter().map(|p| p.0).collect();
        let Some(item) = self.project.db.items.iter_mut().find(|d| d.id == id) else { return format!("No item {id}") };
        // A new model is held by the item's kind; a replaced one keeps its style and length.
        let (kind_hold, kind_size, at) = item.hold_by_kind(rustpg2d_engine::model::extents(&points));
        let (hold, size) = item.model.as_ref().map_or((kind_hold, kind_size), |m| (m.hold, m.size));
        let mut m = rustpg2d_engine::defs::ItemModel { hold, size, ..rustpg2d_engine::defs::ItemModel::new(rel) };
        hold_op(&mut m, &HoldOp::Estimate, &points, &model.indices, at);
        item.model = Some(m);
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

impl Workspace {
    /// Renders the pictures of props shown as 3D models (`only`: one prop, else every one):
    /// each state's model as the game draws it, facing south, into `sprites/prop_pictures/<id>.png`
    /// (a frame per state, twice the world's pixel density, the feet at the bottom middle), which
    /// become the prop's and its states' pictures: icons in the editor, sizes in the game. Saves.
    pub fn render_prop_pictures(&mut self, only: Option<&str>) -> String {
        let done = self.prop_pictures(only);
        if done.is_empty() {
            return "No prop with a loaded 3D model to render".into();
        }
        self.save_defs();
        self.reload_assets();
        format!("Rendered the pictures of {} props from their models: {}", done.len(), done.join(", "))
    }

    /// [`Self::render_prop_pictures`] without saving the definitions: the pictures are written
    /// and read, the props show them (a change like any other, undone with what made it).
    /// Returns the props done.
    pub fn prop_pictures(&mut self, only: Option<&str>) -> Vec<String> {
        use rustpg2d_engine::defs::{Appearance, SheetDef, SpriteRef};
        use rustpg2d_engine::entity::Facing;
        use rustpg2d_engine::imageio::Rgba;
        let db = self.project.db.clone();
        let mut done = Vec::new();
        for p in db.props.iter().filter(|p| only.is_none_or(|o| o == p.id)) {
            let n = p.states.len() + 1;
            let models: Vec<String> = (0..n).map(|k| p.model_at(k).into_owned()).collect();
            if models[0].is_empty() || !models.iter().all(|m| self.assets.models.has(m)) {
                continue;
            }
            // Each state: its pose cropped around the feet (left / right of them, above them).
            let mut pics: Vec<(Rgba, u32, u32)> = Vec::new();
            for m in &models {
                let look = Appearance { model: m.clone(), ..Default::default() };
                let gear = rustpg2d_engine::render::model::Gear::default();
                let Some((img, (cw, ch))) = self.assets.models.sheet(&db, &self.assets.sheets, &look, &gear, &["idle".to_string()], &[Facing::Down], 1, gfx::color::Color::new(0.0, 0.0, 0.0, 0.0)) else { break };
                let (fx, fy) = (cw / 2, ch * 3 / 4);
                // (Its solid pixels: the half-clear ground shadow doesn't widen it. A building's front
                // corner reaches below the feet: the picture ends where the model does.)
                let (mut lo_x, mut hi_x, mut top, mut bottom) = (fx, fx, fy, fy);
                for y in 0..img.height as u32 {
                    for x in 0..img.width as u32 {
                        if img.get_pixel(x, y).a > 0.8 {
                            lo_x = lo_x.min(x);
                            hi_x = hi_x.max(x + 1);
                            top = top.min(y);
                            bottom = bottom.max(y);
                        }
                    }
                }
                let half = (fx - lo_x).max(hi_x - fx) + 1;
                let (w, h) = (half * 2, bottom - top + 1);
                let mut out = Rgba::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        let (sx, sy) = (fx as i32 - half as i32 + x as i32, top as i32 + y as i32);
                        if sx >= 0 && (sx as u32) < img.width as u32 {
                            let c = img.get_pixel(sx as u32, sy as u32);
                            out.set(x as i32, y as i32, [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, (c.a * 255.0) as u8]);
                        }
                    }
                }
                pics.push((out, w, h));
            }
            if pics.len() != n {
                continue;
            }
            let (fw, fh) = (pics.iter().map(|p| p.1).max().unwrap_or(2), pics.iter().map(|p| p.2).max().unwrap_or(2));
            let mut sheet = Rgba::new(fw * n as u32, fh);
            for (k, (img, w, h)) in pics.iter().enumerate() {
                let (ox, oy) = (k as u32 * fw + (fw - w) / 2, fh - h);
                for y in 0..*h {
                    for x in 0..*w {
                        sheet.set((ox + x) as i32, (oy + y) as i32, img.get(x as i32, y as i32));
                    }
                }
            }
            let rel = format!("sprites/prop_pictures/{}.png", p.id);
            let path = self.project.root.join(&rel);
            let written = sheet.encode().map_err(|e| format!("{e:#}")).and_then(|png| std::fs::create_dir_all(path.parent().unwrap()).and_then(|_| std::fs::write(&path, png)).map_err(|e| e.to_string()));
            if let Err(e) = written {
                self.set_status(format!("Writing {rel} failed: {e}"));
                break;
            }
            let sid = format!("{}_picture", p.id);
            let def = SheetDef { scale: rustpg2d_engine::render::model::PX as u32, ..SheetDef::new(&sid, &rel, fw, fh) };
            self.assets.load_sheet(&self.project, &def);
            let db = &mut self.project.db;
            match db.sheets.iter_mut().find(|s| s.id == sid) {
                Some(s) => *s = def,
                None => db.sheets.push(def),
            }
            if let Some(d) = db.props.iter_mut().find(|d| d.id == p.id) {
                d.sprite = SpriteRef::new(&sid, 0);
                d.turns.clear();
                for (k, st) in d.states.iter_mut().enumerate() {
                    st.sprite = SpriteRef::new(&sid, k as u32 + 1);
                }
            }
            done.push(p.id.clone());
        }
        if !done.is_empty() {
            self.project.db.reindex();
            self.defs_dirty = true;
            self.stale = true;
        }
        done
    }
}

/// A change to how an item model is held: the item form's buttons and the `itemhold` script
/// command.
#[derive(Clone, Debug, PartialEq)]
pub enum HoldOp {
    /// Hold point, handle and front estimated from its shape for its hold style.
    Estimate,
    /// The handle runs along the next of the model's main directions (its length, width,
    /// thickness, each both ways), held as far along it as before.
    TurnHandle,
    /// The front turned a quarter about the handle.
    TurnFront,
    /// The other end is the handle.
    Flip,
    /// Held this far along its length (0 = the handle's end, 1 = the far end).
    At(f32),
    /// Another hold style (estimated again for it).
    Style(rustpg2d_engine::defs::HoldStyle),
}

/// How far along its length (from the handle's end) an item model is held, and its extent
/// along the handle: `(fraction, min, max)` of the points projected on the axis.
pub fn hold_fraction(m: &rustpg2d_engine::defs::ItemModel, pts: &[Vec3]) -> (f32, f32, f32) {
    let a = Vec3::from(m.axis).normalize_or(Vec3::Y);
    let (lo, hi) = pts.iter().map(|p| p.dot(a)).fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
    if lo > hi {
        return (0.0, 0.0, 0.0);
    }
    (((Vec3::from(m.grip).dot(a) - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0), lo, hi)
}

/// The front an item model shows: its own, else the automatic one.
pub fn front_of(m: &rustpg2d_engine::defs::ItemModel, pts: &[Vec3]) -> Vec3 {
    if m.front != [0.0; 3] {
        return Vec3::from(m.front);
    }
    Vec3::from(rustpg2d_engine::model::auto_front(m.axis, rustpg2d_engine::model::spread(pts).1))
}

/// How far along its length an item is held by its kind when its shape can't tell (bows: the
/// middle; [`rustpg2d_engine::defs::ItemDef::hold_by_kind`]), for these model points.
pub fn kind_at(d: &rustpg2d_engine::defs::ItemDef, pts: &[Vec3]) -> Option<f32> {
    d.hold_by_kind(rustpg2d_engine::model::extents(pts)).2
}

/// Applies `op` to an item model with these points and triangles; estimates hold it `at` this
/// far along when given (the item's kind says so, [`kind_at`]).
pub fn hold_op(m: &mut rustpg2d_engine::defs::ItemModel, op: &HoldOp, pts: &[Vec3], indices: &[u16], at: Option<f32>) {
    use rustpg2d_engine::model::estimate_hold;
    // Rounded to an axis when it is one (no -0.0000001 noise in the file).
    let snap = |v: Vec3| {
        let v = v.normalize_or(Vec3::Y);
        v.to_array().map(|c| if c.abs() < 1e-4 { 0.0 } else if (c.abs() - 1.0).abs() < 1e-4 { c.signum() } else { c })
    };
    match op {
        HoldOp::Estimate | HoldOp::Style(_) => {
            if let HoldOp::Style(s) = op {
                m.hold = *s;
            }
            (m.grip, m.axis, m.front) = estimate_hold(pts, indices, m.hold);
            if let Some(t) = at {
                hold_op(m, &HoldOp::At(t), pts, indices, None);
            }
        }
        HoldOp::TurnHandle => {
            // The model's main directions (its length first) and their opposites.
            let [d0, d1, d2] = rustpg2d_engine::model::main_axes(pts);
            let dirs = [d0, -d0, d1, -d1, d2, -d2];
            let a = Vec3::from(m.axis).normalize_or(Vec3::Y);
            let now = dirs.iter().position(|d| d.dot(a) > 0.999);
            let next = dirs[now.map_or(0, |i| (i + 1) % dirs.len())];
            let (t, _, _) = hold_fraction(m, pts);
            let front = front_of(m, pts);
            m.axis = snap(next);
            if m.hold != rustpg2d_engine::defs::HoldStyle::Strap {
                // On the line through the middle, as far along as before.
                let mid = rustpg2d_engine::model::spread(pts).0;
                let (_, plo, phi) = hold_fraction(m, pts);
                m.grip = (mid - next * mid.dot(next) + next * (plo + t * (phi - plo))).to_array();
            }
            // A front along the new handle can't be: the automatic one.
            m.front = if front.dot(next).abs() > 0.9 { [0.0; 3] } else { snap(front - next * front.dot(next)) };
        }
        HoldOp::TurnFront => {
            let a = Vec3::from(m.axis).normalize_or(Vec3::Y);
            let f = front_of(m, pts);
            m.front = snap(gfx::math::Quat::from_axis_angle(a, std::f32::consts::FRAC_PI_2) * f);
        }
        HoldOp::Flip => {
            let a = Vec3::from(m.axis).normalize_or(Vec3::Y);
            let (_, lo, hi) = hold_fraction(m, pts);
            let g = Vec3::from(m.grip);
            m.grip = (g + a * (lo + hi - 2.0 * g.dot(a))).to_array();
            m.axis = snap(-a);
        }
        HoldOp::At(t) => {
            let a = Vec3::from(m.axis).normalize_or(Vec3::Y);
            let (now, lo, hi) = hold_fraction(m, pts);
            m.grip = (Vec3::from(m.grip) + a * ((t.clamp(0.0, 1.0) - now) * (hi - lo))).to_array();
        }
    }
}

impl Workspace {
    /// `itemhold ITEM OP [ARG]`: what the item form's buttons do (`estimate`, `turnhandle`,
    /// `turnfront`, `flip`, `at FRACTION`, `style grip|upright|hang|strap`, `size PX`), saved.
    pub fn item_hold(&mut self, id: &str, op: &str, arg: &str) -> String {
        use rustpg2d_engine::defs::HoldStyle;
        let Some(file) = self.project.db.item(id).and_then(|d| d.model.as_ref()).map(|m| m.file.clone()) else { return format!("{id} has no 3D model") };
        let Some((pts, _)) = self.assets.models.item_points(&file) else { return format!("{file} isn't loaded") };
        let indices = self.assets.models.item_indices(&file);
        let op = match op {
            "estimate" => HoldOp::Estimate,
            "turnhandle" => HoldOp::TurnHandle,
            "turnfront" => HoldOp::TurnFront,
            "flip" => HoldOp::Flip,
            "at" => HoldOp::At(arg.parse().unwrap_or(0.1)),
            "style" => match HoldStyle::ALL.iter().find(|s| s.label().eq_ignore_ascii_case(arg)) {
                Some(s) => HoldOp::Style(*s),
                None => return format!("No hold style {arg} (grip, upright, hang, strap)"),
            },
            "size" => {
                let Some(m) = self.project.db.items.iter_mut().find(|d| d.id == id).and_then(|d| d.model.as_mut()) else { return String::new() };
                m.size = arg.parse().unwrap_or(m.size);
                return self.item_hold_saved(id);
            }
            _ => return format!("itemhold: unknown {op}"),
        };
        let Some(d) = self.project.db.items.iter_mut().find(|d| d.id == id) else { return String::new() };
        let at = kind_at(d, &pts);
        let Some(m) = d.model.as_mut() else { return String::new() };
        hold_op(m, &op, &pts, &indices, at);
        self.item_hold_saved(id)
    }

    fn item_hold_saved(&mut self, id: &str) -> String {
        self.defs_dirty = true;
        self.save_defs();
        self.assets.models.clear_poses();
        let m = self.project.db.item(id).and_then(|d| d.model.clone());
        m.map_or_else(String::new, |m| format!("{id}: {} grip {:?} axis {:?} front {:?} size {}", m.hold.label(), m.grip, m.axis, m.front, m.size))
    }
}
