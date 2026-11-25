//! The *Generate art* window (Assets › Generate art…), the Meshy part of *Edit › Settings…*, the
//! references the requests use, and adding finished art to the project.

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::defs::{CharacterDef, ItemDef, SheetDef, SpriteRef};
use rustpg2d_engine::imageio::Rgba;

use super::{api_key, meshy, save_key, ArtKind, ArtRequest, Job, Locked, References, Stage, ATTACKS};
use crate::app::Editor;
use crate::ui::{self, ph};

/// The Art window's form.
pub struct ArtWindow {
    pub open: bool,
    pub kind: ArtKind,
    pub id: String,
    pub description: String,
    pub attack: String,
    /// Portraits: the character it is for ("" = none).
    pub portrait_for: String,
    /// 3D characters: a woman; the figure to model from ("" = a new turnaround picture).
    pub female: bool,
    pub from: String,
    pub extras: Vec<String>,
    /// A job waits for "Draw for N credits" (Ask before spending).
    pub confirm: bool,
    pub message: Option<String>,
    /// Preview textures by job id: (version, texture).
    previews: std::collections::HashMap<u32, (u32, egui::TextureHandle)>,
}

impl Default for ArtWindow {
    fn default() -> Self {
        ArtWindow { open: false, kind: ArtKind::Model3d, id: String::new(), description: String::new(), attack: "none".into(), portrait_for: String::new(), female: false, from: String::new(), extras: Vec::new(), confirm: false, message: None, previews: Default::default() }
    }
}

impl Editor {
    /// The pictures art requests refer to: the project's item icons (the style of new icons).
    pub fn art_references(&self) -> References {
        let db = &self.project.db;
        let icons = db.sheet("items").and_then(|d| {
            let im = Rgba::decode(&std::fs::read(self.project.root.join(&d.image)).ok()?).ok()?;
            let n = (im.w / 32).min(8);
            let mut strip = Rgba::new(n * 32 * 4, 32 * 4);
            for y in 0..strip.h {
                for x in 0..strip.w {
                    let c = im.get((x / 4) as i32, (y / 4) as i32);
                    let a = c[3] as u32;
                    let mix = |i: usize| ((c[i] as u32 * a + 235 * (255 - a)) / 255) as u8;
                    strip.set(x as i32, y as i32, [mix(0), mix(1), mix(2), 255]);
                }
            }
            Some(meshy::data_uri(&strip.encode().ok()?))
        });
        References { icons, ..Default::default() }
    }

    /// The references of one request: 3D characters from a figure get its front, side and back
    /// frames, an item's 3D model its icon.
    pub fn art_references_for(&self, req: &ArtRequest) -> anyhow::Result<References> {
        let mut refs = self.art_references();
        let db = &self.project.db;
        let sheet_image = |id: &str| -> anyhow::Result<(Rgba, u32, u32)> {
            let d = db.sheet(id).ok_or_else(|| anyhow::anyhow!("no sprite sheet \"{id}\""))?;
            let im = Rgba::decode(&std::fs::read(self.project.root.join(&d.image))?)?;
            Ok((im, d.frame_w.max(1), d.frame_h.max(1)))
        };
        match req.kind {
            ArtKind::Model3d | ArtKind::Creature3d if !req.from.is_empty() => {
                let (im, fw, fh) = sheet_image(&req.from)?;
                if im.h / fh < 8 || im.w / fw < 1 {
                    anyhow::bail!("\"{}\" isn't a figure with standing frames (rows 4-7)", req.from);
                }
                // Standing facing down, left and up (the character layout's idle rows).
                refs.views = [4, 5, 7].iter().map(|row| framed(&im, 0, row * fh, fw, fh, 640)).collect::<anyhow::Result<_>>()?;
            }
            ArtKind::Clips3d => {
                let m = db.model(&req.id).ok_or_else(|| anyhow::anyhow!("no 3D model \"{}\" to add clips to", req.id))?;
                if m.rig.is_some() || !self.assets.models.info(&m.id).is_some_and(|i| i.rigged) {
                    anyhow::bail!("{} has no skeleton of its own (Meshy rigs people): give it a rig template", req.id);
                }
                refs.model = Some(meshy::data_uri_of(&std::fs::read(self.project.root.join(&m.file))?, "application/octet-stream"));
            }
            ArtKind::Prop3d => {
                let prop = db.prop(&req.id).ok_or_else(|| anyhow::anyhow!("no prop \"{}\" to make a 3D model of: make the prop first (its hitbox is the model's footprint)", req.id))?;
                if let Ok((im, fw, fh)) = sheet_image(&prop.sprite.sheet) {
                    let cols = (im.w / fw).max(1);
                    let (x, y) = ((prop.sprite.frame % cols) * fw, (prop.sprite.frame / cols) * fh);
                    refs.icon = framed(&im, x, y, fw * prop.sprite.w.max(1), fh * prop.sprite.h.max(1), 512).ok();
                }
                if refs.icon.is_none() && req.description.trim().len() < 8 {
                    anyhow::bail!("prop \"{}\" has no picture: describe it in a few words", req.id);
                }
            }
            ArtKind::Gear3d => {
                let item = db.item(&req.id).ok_or_else(|| anyhow::anyhow!("no item \"{}\" to make a 3D model of", req.id))?;
                let (im, fw, fh) = sheet_image(&item.icon.sheet).map_err(|_| anyhow::anyhow!("item \"{}\" has no icon to model it from", req.id))?;
                let cols = (im.w / fw).max(1);
                let (x, y) = ((item.icon.frame % cols) * fw, (item.icon.frame / cols) * fh);
                refs.icon = Some(framed(&im, x, y, fw * item.icon.w.max(1), fh * item.icon.h.max(1), 512)?);
            }
            _ => {}
        }
        Ok(refs)
    }

    /// Saves the Meshy settings (with the agent server's).
    pub fn save_art_settings(&self) {
        let s = &self.art_settings;
        crate::files::save_settings(&crate::files::EditorSettings { meshy_model: s.model.clone(), meshy_budget: s.budget, meshy_ask: s.ask, ..crate::files::load_settings() });
    }

    /// Starts a job for a request (the cost confirmed by the caller).
    pub fn start_art(&mut self, mut req: ArtRequest) -> anyhow::Result<u32> {
        if req.kind == ArtKind::Clips3d {
            // The women's clips (picking crops) for a woman's model.
            req.female = self.project.db.model(&req.id).is_some_and(|m| m.female);
        }
        let refs = self.art_references_for(&req)?;
        let settings = self.art_settings.clone();
        let id = self.art.start(req, &settings, refs)?;
        self.set_status(format!("Drawing art (job {id}) with Meshy"));
        Ok(id)
    }

    pub fn redo_art(&mut self, job: u32, picture: &str) -> anyhow::Result<()> {
        let req = self.art.job(job).ok_or_else(|| anyhow::anyhow!("no job {job}"))?.locked().req.clone();
        let refs = self.art_references_for(&req)?;
        let settings = self.art_settings.clone();
        self.art.redo(job, picture, &settings, refs)
    }

    /// Adds a finished job's art to the project as `id`: the image and its sprite sheet (people's
    /// figures with estimated hand marks), and for characters a character, for icons an item, for
    /// props a prop (unless one exists). Returns what was added.
    pub fn apply_art(&mut self, job: u32, id: &str) -> anyhow::Result<String> {
        if self.library_mode {
            anyhow::bail!("add art to a project, not the library");
        }
        let j = self.art.job(job).ok_or_else(|| anyhow::anyhow!("no job {job}"))?.locked().clone();
        let png = j.result.clone().filter(|_| j.stage == Stage::Done).ok_or_else(|| anyhow::anyhow!("job {job} isn't finished"))?;
        let id = id.trim();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            anyhow::bail!("the id must be letters, digits and _");
        }
        if j.req.kind.is_3d() {
            return self.apply_art_3d(&j, id);
        }
        let db = &self.project.db;
        if db.sheet(id).is_some() {
            anyhow::bail!("the project already has a sprite sheet \"{id}\"; pick another id");
        }
        let mut rel = format!("sprites/{id}.png");
        let mut n = 2;
        while self.project.root.join(&rel).exists() {
            rel = format!("sprites/{id}_{n}.png");
            n += 1;
        }
        std::fs::create_dir_all(self.project.root.join("sprites"))?;
        std::fs::write(self.project.root.join(&rel), &png)?;
        let (fw, fh) = j.frame;
        let sheet = match j.req.kind {
            ArtKind::Portrait => SheetDef { scale: 2, ..SheetDef::new(id, &rel, fw, fh) },
            ArtKind::Icon | ArtKind::Image => SheetDef::new(id, &rel, fw, fh),
            ArtKind::Model3d | ArtKind::Creature3d | ArtKind::Gear3d | ArtKind::Clips3d | ArtKind::Prop3d => unreachable!("3D jobs are added by apply_art_3d"),
        };
        let mut added = vec![format!("sprite sheet {id} ({rel})")];
        let db = &mut self.project.db;
        db.upsert(sheet);
        let name = id.replace('_', " ");
        let name = name[..1].to_uppercase() + &name[1..];
        match j.req.kind {
            ArtKind::Portrait if !j.req.portrait_for.is_empty() => match db.characters.iter_mut().find(|c| c.id == j.req.portrait_for) {
                Some(c) => {
                    c.portrait = SpriteRef::new(id, 0);
                    added.push(format!("the portrait of {}", j.req.portrait_for));
                }
                None => added.push(format!("(no character \"{}\" for the portrait)", j.req.portrait_for)),
            },
            ArtKind::Icon if db.item(id).is_none() => {
                db.upsert(ItemDef { name: name.clone(), icon: SpriteRef::new(id, 0), ..ItemDef::fresh(id) });
                added.push(format!("item {id}"));
            }
            _ => {}
        }
        db.reindex();
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        if let Some(job) = self.art.job(job) {
            job.locked().applied = Some(id.to_string());
        }
        let msg = format!("Added {}", added.join(", "));
        self.set_status(msg.clone());
        Ok(msg)
    }

    /// Adds a finished 3D job: a 3D character's model (the rigged file with its clips file) and a
    /// character using it, a creature's model, an item's 3D model. Returns what was added.
    fn apply_art_3d(&mut self, j: &Job, id: &str) -> anyhow::Result<String> {
        let file = |name: &str| j.made.files.iter().find(|(f, _)| *f == name).map(|(_, b)| b.clone()).ok_or_else(|| anyhow::anyhow!("job {} has no {name}", j.id));
        let write = |ed: &Editor, stem: &str, bytes: &[u8]| -> anyhow::Result<String> {
            std::fs::create_dir_all(ed.project.root.join("models"))?;
            let mut rel = format!("models/{stem}.glb");
            let mut n = 2;
            while ed.project.root.join(&rel).exists() {
                rel = format!("models/{stem}_{n}.glb");
                n += 1;
            }
            std::fs::write(ed.project.root.join(&rel), bytes)?;
            Ok(rel)
        };
        let mut added = Vec::new();
        match j.req.kind {
            ArtKind::Gear3d => {
                let rel = write(self, id, &file("model.glb")?)?;
                added.push(self.use_item_model(&j.req.id, &rel));
            }
            ArtKind::Prop3d => {
                let (glb, size) = rustpg2d_engine::model::stand_on_ground(&file("model.glb")?)?;
                let rel = write(self, &format!("{}_3d", j.req.id), &glb)?;
                added.push(self.use_prop_model(&j.req.id, &rel, size)?);
            }
            ArtKind::Clips3d => {
                let model = j.req.id.clone();
                if self.project.db.model(&model).is_none() {
                    anyhow::bail!("the 3D model \"{model}\" is gone");
                }
                let rel = write(self, &format!("{model}_clips"), &file("clips.glb")?)?;
                let bytes = self.compact_file(&rel, false)?;
                let names = rustpg2d_engine::model::clip_names(&bytes)?;
                added.push(self.add_shared_clips(&model, &rel, &names, &j.req.extras));
            }
            kind => {
                if self.project.db.model(id).is_some() {
                    anyhow::bail!("the project already has a 3D model \"{id}\"; pick another id");
                }
                let rigged = kind == ArtKind::Model3d;
                let rel = write(self, id, &file(if rigged { "rig.glb" } else { "model.glb" })?)?;
                let before = self.project.db.models.len();
                added.push(self.use_model_file("", false, &rel));
                let Some(model) = self.project.db.models.get(before).map(|m| m.id.clone()) else { anyhow::bail!("{}", added.join("; ")) };
                if rigged {
                    let clips = write(self, &format!("{id}_clips"), &file("clips.glb")?)?;
                    added.push(self.use_model_file(&model, true, &clips));
                }
                let db = &mut self.project.db;
                if let Some(m) = db.models.iter_mut().find(|m| m.id == model) {
                    m.female = j.req.female;
                    let name = id.replace('_', " ");
                    m.name = name[..1].to_uppercase() + &name[1..];
                }
                if rigged && db.character(id).is_none() {
                    let name = id.replace('_', " ");
                    let name = name[..1].to_uppercase() + &name[1..];
                    let mut c = CharacterDef { description: j.req.description.trim().to_string(), ..crate::builder::default_character(db, id, &name) };
                    c.appearance = rustpg2d_engine::defs::Appearance { model: model.clone(), ..Default::default() };
                    db.upsert(c);
                    added.push(format!("character {id}"));
                }
                db.reindex();
                self.defs_dirty = true;
                self.save_defs();
                self.reload_assets();
            }
        }
        if let Some(job) = self.art.job(j.id) {
            job.locked().applied = Some(id.to_string());
        }
        let msg = added.join("; ");
        self.set_status(msg.clone());
        Ok(msg)
    }

    /// Tells about jobs that finished or failed (the status line), once each.
    pub fn art_tick(&mut self) {
        let mut news = Vec::new();
        for j in &self.art.jobs {
            let mut j = j.locked();
            let end = matches!(j.stage, Stage::Done | Stage::Failed(_));
            if end && !j.told {
                j.told = true;
                news.push(match &j.stage {
                    Stage::Failed(e) => format!("Art job {} ({}) failed: {e}", j.id, j.req.id),
                    _ => format!("Art job {} ({}) is ready: {} credits spent", j.id, j.req.id, j.credits()),
                });
            }
        }
        for n in news {
            self.set_status(n);
        }
    }

    /// *Edit › Settings…*: the Meshy account and its spending rules.
    pub fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.art.settings_open {
            return;
        }
        let mut open = true;
        let mut changed = false;
        egui::Window::new(ui::t(ph::GEAR, "Settings")).id(egui::Id::new("editor_settings")).open(&mut open).default_width(460.0).show(ctx, |ui| {
            ui.heading("Meshy (Generate art)");
            ui.label("Draws characters, icons, props, portraits and pictures with your Meshy account (meshy.ai). Your key is stored for your user only, apart from projects; agents never see it.");
            let has = !api_key().is_empty();
            ui.horizontal(|ui| {
                ui.label("API key");
                ui.add(egui::TextEdit::singleline(&mut self.art.key_field).password(true).hint_text(if has { "saved (type to replace)" } else { "msy_…" }).desired_width(220.0));
                if ui.button("Save").clicked() && !self.art.key_field.trim().is_empty() {
                    match save_key(&self.art.key_field) {
                        Ok(()) => self.set_status("Meshy key saved"),
                        Err(e) => self.set_status(format!("Saving the key failed: {e:#}")),
                    }
                    self.art.key_field.clear();
                    self.art.balance = None;
                }
                if has && ui.button("Remove").clicked() {
                    let _ = save_key("");
                    self.art.balance = None;
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(has, egui::Button::new("Test key")).on_hover_text("Asks Meshy for your balance").clicked() {
                    self.art.balance = Some(meshy::balance(&api_key()).map_err(|e| format!("{e:#}")));
                }
                match &self.art.balance {
                    Some(Ok(b)) => {
                        ui.label(format!("{b} credits"));
                    }
                    Some(Err(e)) => {
                        ui.colored_label(Color32::from_rgb(235, 110, 100), e);
                    }
                    None => {}
                }
            });
            let s = &mut self.art_settings;
            egui::Grid::new("meshy_settings").num_columns(2).show(ui, |ui| {
                ui.label("Model");
                egui::ComboBox::from_id_salt("meshy_model").selected_text(meshy::MODELS.iter().find(|m| m.0 == s.model).map_or(s.model.as_str(), |m| m.1)).show_ui(ui, |ui| {
                    for (id, label, cost) in meshy::MODELS {
                        changed |= ui.selectable_value(&mut s.model, id.to_string(), format!("{label}: {cost} credits a picture")).changed();
                    }
                });
                ui.end_row();
                ui.label("Budget per session");
                changed |= ui.add(egui::DragValue::new(&mut s.budget).range(0..=5000).suffix(" credits")).on_hover_text("Generate art (and agents) stop when this session has spent as much").changed();
                ui.end_row();
                ui.label("Ask before spending");
                changed |= ui.checkbox(&mut s.ask, "the Art window asks before each job").changed();
                ui.end_row();
            });
            let spent = *self.art.spent.locked();
            ui.weak(format!("This session: {spent} of {} credits used. A character costs 1 picture (2 with an attack), anything else 1.", self.art_settings.budget));
        });
        if changed {
            self.save_art_settings();
        }
        if !open {
            self.art.settings_open = false;
        }
    }

    /// *Assets › Generate art…*.
    pub fn art_window(&mut self, ctx: &egui::Context) {
        let any_running = self.art.jobs.iter().any(|j| matches!(j.locked().stage, Stage::Drawing | Stage::Converting));
        if any_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if !self.art.window.open {
            return;
        }
        let mut open = true;
        let mut start = false;
        let mut edit_sheet: Option<String> = None;
        let mut redo: Option<(u32, &'static str)> = None;
        let mut apply: Option<(u32, String)> = None;
        let jobs: Vec<Job> = self.art.jobs.iter().rev().map(|j| j.locked().clone()).collect();
        let cost = super::ArtState::cost(&self.req_from_form(), &self.art_settings.model);
        let left = self.art.budget_left(&self.art_settings);
        let has_key = !api_key().is_empty() || std::env::var_os("RUSTPG2D_MESHY_FAKE").is_some();
        egui::Window::new(ui::t(ph::PAINT_BRUSH, "Generate art")).id(egui::Id::new("art_window")).open(&mut open).default_width(560.0).default_height(560.0).show(ctx, |ui| {
            if !has_key {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(230, 190, 90), "No Meshy API key yet.");
                    if ui.button("Settings…").clicked() {
                        self.art.settings_open = true;
                    }
                });
            }
            let w = &mut self.art.window;
            egui::Grid::new("art_form").num_columns(2).show(ui, |ui| {
                ui.label("What");
                ui.vertical(|ui| {
                    for three_d in [true, false] {
                        ui.horizontal(|ui| {
                            for k in ArtKind::ALL.into_iter().filter(|k| k.is_3d() == three_d) {
                                ui.selectable_value(&mut w.kind, k, k.label());
                            }
                        });
                    }
                });
                ui.end_row();
                ui.label("Id");
                ui.add(egui::TextEdit::singleline(&mut w.id).hint_text(match w.kind {
                    ArtKind::Gear3d => "an item id: iron_sword",
                    ArtKind::Prop3d => "a prop id: market_stall",
                    ArtKind::Clips3d => "a 3D model id: hero",
                    _ => "orc_shaman",
                }).desired_width(180.0));
                ui.end_row();
                ui.label("Description");
                ui.add(egui::TextEdit::multiline(&mut w.description).desired_rows(3).desired_width(380.0).hint_text("an orc shaman, grey-green skin, a bone necklace, a ragged red cloak, a staff with a skull"));
                ui.end_row();
                if w.kind == ArtKind::Model3d {
                    ui.label("Attack");
                    egui::ComboBox::from_id_salt("art_attack3d").selected_text(ATTACKS.iter().find(|a| a.0 == w.attack).map_or("?", |a| a.1)).show_ui(ui, |ui| {
                        for (id, label) in ATTACKS.iter().filter(|a| a.0 != "bite") {
                            ui.selectable_value(&mut w.attack, id.to_string(), *label);
                        }
                    });
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut w.female, "a woman").on_hover_text("Picks the women's sitting clip and marks the model as a woman's (the creator)");
                    ui.end_row();
                    ui.label("Extra animations");
                    ui.horizontal(|ui| {
                        for e in super::EXTRAS {
                            let mut on = w.extras.iter().any(|x| x == e);
                            if ui.checkbox(&mut on, e).on_hover_text("3 credits more: a clip for this activity (else it plays idle)").changed() {
                                w.extras.retain(|x| x != e);
                                if on {
                                    w.extras.push(e.to_string());
                                }
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("From figure");
                    ui.add(egui::TextEdit::singleline(&mut w.from).hint_text("(draw a turnaround picture)").desired_width(180.0)).on_hover_text("A sprite sheet in the four-facing layout (an old figure): its standing frames (front, side, back) become the model, no picture is drawn");
                    ui.end_row();
                }
                if w.kind == ArtKind::Clips3d {
                    ui.label("Animations");
                    ui.horizontal_wrapped(|ui| {
                        ui.set_max_width(380.0);
                        for e in super::CLIP_CHOICES.iter().map(|c| c.0).chain(super::EXTRAS) {
                            let mut on = w.extras.iter().any(|x| x == e);
                            if ui.checkbox(&mut on, e).on_hover_text("3 credits: a clip from Meshy's animation library, mapped to this name").changed() {
                                w.extras.retain(|x| x != e);
                                if on {
                                    w.extras.push(e.to_string());
                                }
                            }
                        }
                    });
                    ui.end_row();
                }
                if w.kind == ArtKind::Creature3d {
                    ui.label("From figure");
                    ui.add(egui::TextEdit::singleline(&mut w.from).hint_text("(draw a picture)").desired_width(180.0)).on_hover_text("A sprite sheet in the four-facing layout (an old figure): its standing frames (front, side, back) become the model, no picture is drawn");
                    ui.end_row();
                }
                if w.kind == ArtKind::Portrait {
                    ui.label("For character");
                    ui.add(egui::TextEdit::singleline(&mut w.portrait_for).hint_text("(none)").desired_width(180.0)).on_hover_text("A character id: Add to project makes the picture its dialog portrait");
                    ui.end_row();
                }
            });
            match w.kind {
                ArtKind::Model3d => ui.weak("A turnaround picture, a 3D model of it, a skeleton and animations (walk, idle, the attack, hit, sit). Takes a few minutes."),
                ArtKind::Creature3d => ui.weak("A picture and a 3D model of it, without a skeleton: give it a rig template to animate it."),
                ArtKind::Gear3d => ui.weak("A 3D model of the item from its icon (the description, if any, guides the colours). It is held in hands of 3D characters."),
                ArtKind::Prop3d => ui.weak("A 3D model of the prop with this id: a concept picture from its picture (and the description, if any), then a model, stood on the ground, as wide as the prop's hitbox (which grows as deep as the model; make the prop first). Placed copies turn with R; it casts a shadow."),
                ArtKind::Clips3d => ui.weak("More animations for the rigged 3D model with this id (no description needed): its file is rigged again (5 credits) and the clips are fitted to its own skeleton. Other models with the same bones can then share them (Database › 3D models › Copy clips)."),
                _ => ui.label(""),
            };
            ui.horizontal(|ui| {
                let label = if w.confirm { format!("Draw for {cost} credits - sure?") } else { format!("Draw ({cost} credits)") };
                if ui.add_enabled(has_key && cost <= left, egui::Button::new(ui::t(ph::PAINT_BRUSH, &label))).on_hover_text(format!("{left} credits left in this session's budget")).clicked() {
                    if self.art_settings.ask && !w.confirm {
                        w.confirm = true;
                    } else {
                        start = true;
                        w.confirm = false;
                    }
                }
                if w.confirm && ui.button("Cancel").clicked() {
                    w.confirm = false;
                }
                ui.weak(format!("{left} credits left this session"));
            });
            if let Some(m) = &w.message {
                ui.colored_label(Color32::from_rgb(230, 190, 90), m);
            }
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for j in &jobs {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(format!("{} \"{}\"{}", j.req.kind.label(), j.req.id, if j.req.by_agent { "  (agent)" } else { "" }));
                            match &j.stage {
                                Stage::Drawing => {
                                    let p: Vec<String> = j.pictures.iter().map(|p| format!("{} {}", p.name, if p.status == "IN_PROGRESS" { format!("{}%", p.progress) } else { p.status.to_lowercase() })).collect();
                                    ui.spinner();
                                    ui.label(p.join(", "));
                                }
                                Stage::Converting => {
                                    ui.spinner();
                                    ui.label("converting");
                                }
                                Stage::Done => {
                                    ui.label(format!("done, {} credits", j.credits()));
                                }
                                Stage::Failed(e) => {
                                    ui.colored_label(Color32::from_rgb(235, 110, 100), e);
                                }
                            }
                        });
                        ui.weak(&j.req.description);
                        if let Some(png) = &j.result {
                            let tex = self.art.window.previews.get(&j.id).filter(|(v, _)| *v == j.version).map(|(_, t)| t.clone());
                            let tex = tex.or_else(|| {
                                let im = Rgba::decode(png).ok()?;
                                let img = crate::widgets::color_image(ui.ctx(), im.w as usize, im.h as usize, &im.px);
                                let t = ui.ctx().load_texture(format!("art_{}", j.id), img, egui::TextureOptions::NEAREST);
                                self.art.window.previews.insert(j.id, (j.version, t.clone()));
                                Some(t)
                            });
                            if let Some(t) = tex {
                                preview(ui, &t, j);
                            }
                        }
                        ui.horizontal(|ui| {
                            if !matches!(j.stage, Stage::Drawing | Stage::Converting) {
                                for p in &j.pictures {
                                    let tip = if j.req.kind.is_3d() { format!("Make the {} again, with the steps after it ({} credits)", p.name, super::ArtState::redo_cost(j, p.name)) } else { format!("Draw the {} picture again ({} credits)", p.name, meshy::model_cost(&self.art_settings.model)) };
                                    if ui.button(format!("Redo {}", p.name)).on_hover_text(tip).clicked() {
                                        redo = Some((j.id, p.name));
                                    }
                                }
                            }
                            if j.stage == Stage::Done {
                                match &j.applied {
                                    Some(a) => {
                                        ui.label(format!("added as {a}"));
                                        if ui.button(ui::t(ph::PENCIL_SIMPLE, "Edit in sprite editor")).on_hover_text("Database › Sprite sheets with the sheet open in the sprite editor").clicked() {
                                            edit_sheet = Some(a.clone());
                                        }
                                    }
                                    None => {
                                        if ui.button(ui::t(ph::PLUS, "Add to project")).clicked() {
                                            apply = Some((j.id, j.req.id.clone()));
                                        }
                                    }
                                }
                            }
                        });
                    });
                }
            });
        });
        if start {
            let req = self.req_from_form();
            match self.start_art(req) {
                Ok(_) => self.art.window.message = None,
                Err(e) => self.art.window.message = Some(format!("{e:#}")),
            }
        }
        if let Some((job, pic)) = redo {
            if let Err(e) = self.redo_art(job, pic) {
                self.art.window.message = Some(format!("{e:#}"));
            }
        }
        if let Some((job, id)) = apply {
            if let Err(e) = self.apply_art(job, &id) {
                self.art.window.message = Some(format!("{e:#}"));
            }
        }
        if let Some(sheet) = edit_sheet {
            self.edit_sheet(&sheet);
        }
        if !open {
            self.art.window.open = false;
        }
    }

    fn req_from_form(&self) -> ArtRequest {
        let w = &self.art.window;
        ArtRequest { kind: w.kind, id: w.id.trim().to_string(), description: w.description.clone(), attack: w.attack.clone(), portrait_for: w.portrait_for.trim().to_string(), by_agent: false, female: w.female, from: w.from.trim().to_string(), extras: w.extras.clone() }
    }
}

/// A region of a picture, cut to its opaque part, enlarged (nearest) to about `size` px on its
/// longer side and centred on white with a margin: a PNG data URI for Meshy. Half-transparent
/// pixels (a figure's ground shadow) count as background, or they would be modelled.
fn framed(im: &Rgba, x0: u32, y0: u32, w: u32, h: u32, size: u32) -> anyhow::Result<String> {
    let (mut l, mut t, mut r, mut b) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if im.get((x0 + x) as i32, (y0 + y) as i32)[3] >= 128 {
                (l, t, r, b) = (l.min(x), t.min(y), r.max(x + 1), b.max(y + 1));
            }
        }
    }
    if r <= l || b <= t {
        anyhow::bail!("the picture is empty");
    }
    let k = (size * 8 / 10 / (r - l).max(b - t)).max(1);
    let mut out = Rgba::new(size, size);
    let (ox, oy) = ((size - (r - l) * k) / 2, (size - (b - t) * k) / 2);
    for y in 0..size {
        for x in 0..size {
            let (sx, sy) = (x as i64 - ox as i64, y as i64 - oy as i64);
            let c = if sx >= 0 && sy >= 0 && (sx as u32) < (r - l) * k && (sy as u32) < (b - t) * k { im.get((x0 + l) as i32 + sx as i32 / k as i32, (y0 + t) as i32 + sy as i32 / k as i32) } else { [0; 4] };
            out.set(x as i32, y as i32, if c[3] >= 128 { [c[0], c[1], c[2], 255] } else { [255; 4] });
        }
    }
    Ok(meshy::data_uri(&out.encode()?))
}

/// The result (the picture; 3D jobs: what they were made from).
fn preview(ui: &mut egui::Ui, t: &egui::TextureHandle, _j: &Job) {
    let size = t.size_vec2();
    let k = (220.0 / size.y).min(400.0 / size.x).min(4.0);
    ui.add(egui::Image::new((t.id(), size * k)));
}
