//! 3D models in the editor: importing `.glb` files as models (their clips matched to the
//! animations by name), extra clip files, replacing a model's file. The form is
//! `database::model_form`.

use rustpg2d_engine::defs::{Database, JointNudge, ModelDef, RigFit};
use macroquad::math::Vec3;
use rustpg2d_engine::model::{guess_clips, Model};

use crate::app::Editor;
use crate::database::Tab;

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

impl Editor {
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
            self.db.tab = Tab::Models;
            self.db.sel = db.models.len() - 1;
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
            m.front = snap(macroquad::math::Quat::from_axis_angle(a, std::f32::consts::FRAC_PI_2) * f);
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

impl Editor {
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
