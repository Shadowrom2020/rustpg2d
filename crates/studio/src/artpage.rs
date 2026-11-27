//! The Generate art page: what Meshy (meshy.ai, the creator's own account) is asked to draw or
//! model, what that costs, the jobs with their progress and results, and *Add to project*.
//! The workshops open it with their thing filled in (*Generate art…* in a prop's, item's,
//! character's or 3D model's tab).
//!
//! The jobs themselves are `editor_core::art`; nothing is spent without a click here, and
//! never more than the session's budget.

use std::collections::HashMap;

use eframe::egui;
use editor_core::art::{api_key, meshy, save_key, ArtKind, ArtRequest, ArtState, Job, Stage, ATTACKS, CLIP_CHOICES, EXTRAS};
use editor_core::Workspace;
use rustpg2d_engine::refactor::RefKind;

pub struct ArtPage {
    pub kind: ArtKind,
    /// What it becomes: a new id, or the item, prop or model it is made for.
    pub id: String,
    pub description: String,
    pub attack: String,
    /// Portraits: the character it is for ("" = none).
    pub portrait_for: String,
    pub female: bool,
    /// 3D characters and creatures: the figure sheet to model from ("" = a picture is drawn).
    pub from: String,
    pub extras: Vec<String>,
    /// *Draw* was clicked and waits for the second click (*Ask before spending*).
    confirm: bool,
    pub message: Option<String>,
    /// The results as egui shows them, by job: for which version.
    previews: HashMap<u32, (u32, egui::TextureHandle)>,
}

impl Default for ArtPage {
    fn default() -> ArtPage {
        ArtPage { kind: ArtKind::Icon, id: String::new(), description: String::new(), attack: "none".into(), portrait_for: String::new(), female: false, from: String::new(), extras: Vec::new(), confirm: false, message: None, previews: HashMap::new() }
    }
}

/// What a kind is made for: the definitions its id is picked from (`None`: a new id is typed).
fn made_for(kind: ArtKind) -> Option<RefKind> {
    match kind {
        ArtKind::Gear3d => Some(RefKind::Item),
        ArtKind::Prop3d => Some(RefKind::Prop),
        ArtKind::Clips3d => Some(RefKind::Model),
        _ => None,
    }
}

fn about(kind: ArtKind) -> &'static str {
    match kind {
        ArtKind::Icon => "An item's icon, 32 x 32, in the style of the project's own icons. Added as a sprite sheet and an item.",
        ArtKind::Portrait => "A portrait for dialogs. Added as a sprite sheet, and as the portrait of the character named.",
        ArtKind::Image => "A painted picture for the Show picture action.",
        ArtKind::Model3d => "A person: a turnaround picture, a 3D model of it, a skeleton and animations (walk, idle, the attack, hit, sit). Takes a few minutes.",
        ArtKind::Creature3d => "An animal or a monster: a picture and a 3D model of it, without a skeleton. Its tab offers the rig templates that make it move.",
        ArtKind::Gear3d => "The 3D model of an item, from its icon (a description guides the colours): what characters hold in their hands.",
        ArtKind::Prop3d => "The 3D model of a prop there is: a concept picture from its picture and the description, then a model stood on the ground, as wide as the prop's hitbox.",
        ArtKind::Clips3d => "More animations for a 3D model with its own skeleton: its file is rigged again and the clips fitted to its bones. No description needed.",
    }
}

fn has_key() -> bool {
    !api_key().is_empty() || std::env::var_os("RUSTPG2D_MESHY_FAKE").is_some()
}

impl ArtPage {
    /// The page set up for a thing of the project (a workshop's *Generate art…*).
    pub fn set_for(&mut self, kind: RefKind, id: &str, ws: &Workspace) {
        (self.confirm, self.message) = (false, None);
        match kind {
            RefKind::Prop => (self.kind, self.id) = (ArtKind::Prop3d, id.to_string()),
            RefKind::Item => (self.kind, self.id) = (ArtKind::Gear3d, id.to_string()),
            RefKind::Model => (self.kind, self.id) = (ArtKind::Clips3d, id.to_string()),
            RefKind::Character => {
                (self.kind, self.portrait_for) = (ArtKind::Portrait, id.to_string());
                self.id = format!("{id}_portrait");
                if self.description.trim().is_empty() {
                    self.description = ws.project.db.character(id).map(|c| c.name.clone()).unwrap_or_default();
                }
            }
            _ => {}
        }
    }

    pub fn request(&self) -> ArtRequest {
        ArtRequest { kind: self.kind, id: self.id.trim().to_string(), description: self.description.clone(), attack: self.attack.clone(), portrait_for: self.portrait_for.trim().to_string(), by_agent: false, female: self.female, from: self.from.trim().to_string(), extras: self.extras.clone() }
    }

    /// Starts the job the form describes. Returns its number.
    pub fn draw(&mut self, ws: &mut Workspace) -> Result<u32, String> {
        self.confirm = false;
        match ws.start_art(self.request()) {
            Ok(id) => {
                self.message = None;
                Ok(id)
            }
            Err(e) => {
                self.message = Some(format!("{e:#}"));
                Err(format!("{e:#}"))
            }
        }
    }

    /// Adds a finished job's art to the project (as `id`, else as the id it was asked for).
    pub fn apply(&mut self, ws: &mut Workspace, job: u32, id: Option<&str>) -> Result<String, String> {
        let asked = ws.art.job(job).map(|j| j.lock().unwrap_or_else(|e| e.into_inner()).req.clone()).ok_or_else(|| format!("there is no job {job}"))?;
        let id = id.filter(|i| !i.is_empty()).unwrap_or(&asked.id).to_string();
        match ws.apply_art(job, &id) {
            Ok(said) => {
                ws.begin(&format!("Add generated art: {id}"));
                ws.refresh_refs();
                ws.stale = true;
                self.message = None;
                Ok(said)
            }
            Err(e) => {
                self.message = Some(format!("{e:#}"));
                Err(format!("{e:#}"))
            }
        }
    }

    /// The script's line: `kind=… id=… cost=N left=N key=yes|no jobs=N [said=…]`, then a line a job.
    pub fn status(&self, ws: &Workspace) -> String {
        let cost = ArtState::cost(&self.request(), &ws.art_settings.model);
        let mut out = format!("kind={} id={} cost={cost} left={} key={} jobs={}", self.kind.name(), if self.id.is_empty() { "-" } else { &self.id }, ws.art.budget_left(&ws.art_settings), if has_key() { "yes" } else { "no" }, ws.art.jobs.len());
        if let Some(m) = &self.message {
            out += &format!(" said={m}");
        }
        for j in &ws.art.jobs {
            out += &format!("\n[art] {}", j.lock().unwrap_or_else(|e| e.into_inner()).line());
        }
        out
    }

    /// The script's words: `kind KIND`, `id ID`, `describe TEXT…`, `for CHARACTER`, `attack NAME`,
    /// `extras A,B`, `draw` (the cost confirmed), `redo JOB PICTURE`, `add JOB [ID]`.
    pub fn word(&mut self, ws: &mut Workspace, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "kind" => self.kind = ArtKind::parse(arg(1)).ok_or("kind icon|portrait|image|model|creature|gear|prop|clips")?,
            "id" => self.id = arg(1).to_string(),
            "describe" => self.description = args[1..].join(" "),
            "for" => self.portrait_for = arg(1).to_string(),
            "attack" => self.attack = arg(1).to_string(),
            "extras" => self.extras = arg(1).split(',').filter(|e| !e.is_empty()).map(str::to_string).collect(),
            "draw" => {
                self.draw(ws)?;
            }
            "redo" => ws.redo_art(arg(1).parse().map_err(|_| "redo JOB PICTURE")?, arg(2)).map_err(|e| format!("{e:#}"))?,
            "add" => {
                self.apply(ws, arg(1).parse().map_err(|_| "add JOB [ID]")?, Some(arg(2)))?;
            }
            other => return Err(format!("art {other}: kind, id, describe, for, attack, extras, draw, redo, add")),
        }
        Ok(())
    }

    /// The page. Returns a definition to open (what a job was added as).
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) -> Option<(RefKind, String)> {
        let mut open = None;
        ui.label(egui::RichText::new("Generate art").heading());
        ui.label("Meshy (meshy.ai) draws icons, portraits and pictures and models characters, creatures, items and props, with your own account. A job costs credits of that account: what it costs is on its button, and nothing is spent without a click here.");
        ui.add_space(4.0);
        self.account(ui, ws);
        ui.separator();
        self.form(ui, ws);
        ui.separator();
        // The jobs, the newest first.
        let jobs: Vec<Job> = ws.art.jobs.iter().rev().map(|j| j.lock().unwrap_or_else(|e| e.into_inner()).clone()).collect();
        if jobs.iter().any(|j| matches!(j.stage, Stage::Drawing | Stage::Converting)) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        if jobs.is_empty() {
            ui.weak("No jobs yet. What is drawn appears here, with its result and Add to project.");
        }
        let (mut redo, mut add) = (None, None);
        for j in &jobs {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("{} · {}{}", j.req.kind.label(), j.req.id, if j.req.by_agent { "  (asked for by an agent)" } else { "" }));
                    match &j.stage {
                        Stage::Drawing => {
                            let steps: Vec<String> = j.pictures.iter().map(|p| format!("{} {}", p.name, if p.status == "IN_PROGRESS" { format!("{} %", p.progress) } else { p.status.to_lowercase() })).collect();
                            ui.spinner();
                            ui.label(steps.join(", "));
                        }
                        Stage::Converting => {
                            ui.spinner();
                            ui.label("converting");
                        }
                        Stage::Done => {
                            ui.label(format!("done · {} credits", j.credits()));
                        }
                        Stage::Failed(e) => {
                            ui.colored_label(ui.visuals().error_fg_color, e);
                        }
                    }
                });
                if !j.req.description.is_empty() {
                    ui.weak(&j.req.description);
                }
                if let Some(t) = j.result.as_ref().and_then(|png| self.preview(ui.ctx(), j, png)) {
                    let size = t.size_vec2();
                    let k = (220.0 / size.y).min(420.0 / size.x).min(4.0);
                    ui.add(egui::Image::new((t.id(), size * k))).widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, format!("The result of job {}", j.id)));
                }
                ui.horizontal(|ui| {
                    if !matches!(j.stage, Stage::Drawing | Stage::Converting) {
                        for p in &j.pictures {
                            let cost = if j.req.kind.is_3d() { ArtState::redo_cost(j, p.name) } else { meshy::model_cost(&ws.art_settings.model) };
                            if ui.button(format!("Again: {} ({cost} credits)", p.name)).on_hover_text(if j.req.kind.is_3d() { "This step again, with the steps after it" } else { "This picture again" }).clicked() {
                                redo = Some((j.id, p.name));
                            }
                        }
                    }
                    if j.stage == Stage::Done {
                        match &j.applied {
                            Some(a) => {
                                ui.label(format!("added as {a}"));
                            }
                            None => {
                                if ui.button(egui::RichText::new("Add to project").strong()).on_hover_text("Its files are written into the project and the definitions that show it are made").clicked() {
                                    add = Some(j.id);
                                }
                            }
                        }
                    }
                });
            });
        }
        if let Some((job, picture)) = redo {
            if let Err(e) = ws.redo_art(job, picture) {
                self.message = Some(format!("{e:#}"));
            }
        }
        if let Some(job) = add {
            let asked = jobs.iter().find(|j| j.id == job).map(|j| j.req.clone());
            if let (Ok(_), Some(req)) = (self.apply(ws, job, None), asked) {
                // What it became is opened: the thing it was made for, else the new sheet.
                open = Some(match req.kind {
                    ArtKind::Gear3d => (RefKind::Item, req.id),
                    ArtKind::Prop3d => (RefKind::Prop, req.id),
                    ArtKind::Clips3d | ArtKind::Model3d | ArtKind::Creature3d => (RefKind::Model, req.id),
                    ArtKind::Portrait if !req.portrait_for.is_empty() => (RefKind::Character, req.portrait_for),
                    ArtKind::Icon => (RefKind::Item, req.id),
                    _ => (RefKind::Sheet, req.id),
                });
            }
        }
        open.filter(|(k, id)| ws.project.db.has(*k, id))
    }

    /// The result of a job as a picture egui can show (made again when the job's changed).
    fn preview(&mut self, ctx: &egui::Context, j: &Job, png: &[u8]) -> Option<egui::TextureHandle> {
        if let Some((v, t)) = self.previews.get(&j.id) {
            if *v == j.version {
                return Some(t.clone());
            }
        }
        let mut im = rustpg2d_engine::imageio::Rgba::decode(png).ok()?;
        // (egui takes textures up to 2048 pixels a side.)
        let longest = im.w.max(im.h);
        if longest > 2048 {
            im = im.resized(im.w * 2048 / longest, im.h * 2048 / longest);
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([im.w as usize, im.h as usize], &im.px);
        let t = ctx.load_texture(format!("art job {}", j.id), image, egui::TextureOptions::NEAREST);
        self.previews.insert(j.id, (j.version, t.clone()));
        Some(t)
    }

    /// The Meshy account: the key, the model that draws, the session's budget.
    fn account(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) {
        let has = !api_key().is_empty();
        let mut changed = false;
        egui::CollapsingHeader::new(if has { "Your Meshy account" } else { "Your Meshy account: no key yet" }).id_salt("meshy account").default_open(!has_key()).show(ui, |ui| {
            ui.weak("The key is kept for your user only, apart from projects; agents never see it.");
            ui.horizontal(|ui| {
                let l = ui.label("API key");
                ui.add(egui::TextEdit::singleline(&mut ws.art.key_field).password(true).hint_text(if has { "saved (type to replace it)" } else { "msy_…" }).desired_width(240.0)).labelled_by(l.id);
                if ui.add_enabled(!ws.art.key_field.trim().is_empty(), egui::Button::new("Save key")).clicked() {
                    match save_key(&ws.art.key_field) {
                        Ok(()) => ws.set_status("The Meshy key is saved"),
                        Err(e) => ws.set_status(format!("The key could not be saved: {e:#}")),
                    }
                    ws.art.key_field.clear();
                    ws.art.balance = None;
                }
                if has && ui.button("Remove key").clicked() {
                    let _ = save_key("");
                    ws.art.balance = None;
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(has, egui::Button::new("Ask for the balance")).on_hover_text("Asks Meshy how many credits the account has (this costs none)").clicked() {
                    ws.art.balance = Some(meshy::balance(&api_key()).map_err(|e| format!("{e:#}")));
                }
                match &ws.art.balance {
                    Some(Ok(b)) => {
                        ui.label(format!("{b} credits"));
                    }
                    Some(Err(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                    None => {}
                }
            });
            let s = &mut ws.art_settings;
            egui::Grid::new("meshy settings").num_columns(2).show(ui, |ui| {
                let l = ui.label("Pictures are drawn by");
                egui::ComboBox::from_id_salt("meshy model")
                    .selected_text(meshy::MODELS.iter().find(|m| m.0 == s.model).map_or(s.model.as_str(), |m| m.1))
                    .show_ui(ui, |ui| {
                        for (id, label, cost) in meshy::MODELS {
                            changed |= ui.selectable_value(&mut s.model, id.to_string(), format!("{label}: {cost} credits a picture")).changed();
                        }
                    })
                    .response
                    .labelled_by(l.id);
                ui.end_row();
                let l = ui.label("Budget of a session");
                changed |= ui.add(egui::DragValue::new(&mut s.budget).range(0..=5000).suffix(" credits")).on_hover_text("Jobs (yours and agents') stop when this much was spent since the editor started").labelled_by(l.id).changed();
                ui.end_row();
                ui.label("");
                changed |= ui.checkbox(&mut s.ask, "Ask a second time before spending").changed();
                ui.end_row();
            });
        });
        if changed {
            ws.save_art_settings();
        }
    }

    /// What to make: its kind, what it is for, its description, the options of its kind; the button with its cost.
    fn form(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Make");
            for k in ArtKind::ALL {
                if ui.selectable_label(self.kind == k, k.label()).on_hover_text(about(k)).clicked() && self.kind != k {
                    // (An id picked for another kind of thing means nothing to this one.)
                    if made_for(self.kind) != made_for(k) {
                        self.id.clear();
                    }
                    (self.kind, self.confirm) = (k, false);
                }
            }
        });
        ui.weak(about(self.kind));
        let db = &ws.project.db;
        egui::Grid::new("art form").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            match made_for(self.kind) {
                // Made for something there is: picked, not typed.
                Some(kind) => {
                    let l = ui.label(format!("For the {}", kind.label().to_lowercase()));
                    let list: Vec<(String, String)> = editor_core::defs::list(db, kind).into_iter().filter(|(id, _)| kind != RefKind::Model || db.model(id).is_some_and(|m| m.part.is_none() && m.rig.is_none() && !id.starts_with("rig_"))).collect();
                    let shown = list.iter().find(|(id, _)| *id == self.id).map_or_else(|| if self.id.is_empty() { "pick one".to_string() } else { format!("{} (not there)", self.id) }, |(id, name)| if name == id || name.is_empty() { id.clone() } else { format!("{name}  ({id})") });
                    egui::ComboBox::from_id_salt("art for")
                        .selected_text(shown)
                        .width(280.0)
                        .show_ui(ui, |ui| {
                            for (id, name) in &list {
                                let label = if name == id || name.is_empty() { id.clone() } else { format!("{name}  ({id})") };
                                if ui.selectable_label(*id == self.id, label).clicked() {
                                    self.id = id.clone();
                                }
                            }
                        })
                        .response
                        .labelled_by(l.id);
                }
                None => {
                    let l = ui.label("Its id").on_hover_text("What the new sprite sheet or model is filed under: lower-case letters, digits and _");
                    ui.add(egui::TextEdit::singleline(&mut self.id).hint_text("orc_shaman").desired_width(200.0)).labelled_by(l.id);
                }
            }
            ui.end_row();
            if self.kind != ArtKind::Clips3d {
                let l = ui.label("Description");
                ui.add(egui::TextEdit::multiline(&mut self.description).desired_rows(3).desired_width(420.0).hint_text("an orc shaman, grey-green skin, a bone necklace, a ragged red cloak, a staff with a skull")).labelled_by(l.id);
                ui.end_row();
            }
            if self.kind == ArtKind::Portrait {
                let l = ui.label("Portrait of").on_hover_text("Add to project makes the picture this character's dialog portrait");
                let name = db.character(&self.portrait_for).map_or_else(|| if self.portrait_for.is_empty() { "nobody (a picture only)".to_string() } else { self.portrait_for.clone() }, |c| c.name.clone());
                egui::ComboBox::from_id_salt("art portrait of")
                    .selected_text(name)
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.portrait_for, String::new(), "nobody (a picture only)");
                        for (id, name) in editor_core::defs::list(db, RefKind::Character) {
                            ui.selectable_value(&mut self.portrait_for, id.clone(), if name.is_empty() { id } else { name });
                        }
                    })
                    .response
                    .labelled_by(l.id);
                ui.end_row();
            }
            if self.kind == ArtKind::Model3d {
                let l = ui.label("Attack");
                egui::ComboBox::from_id_salt("art attack")
                    .selected_text(ATTACKS.iter().find(|a| a.0 == self.attack).map_or("?", |a| a.1))
                    .show_ui(ui, |ui| {
                        for (id, label) in ATTACKS {
                            ui.selectable_value(&mut self.attack, id.to_string(), label);
                        }
                    })
                    .response
                    .labelled_by(l.id);
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut self.female, "A woman").on_hover_text("The women's sitting clip, and the model is offered to players who choose a woman");
                ui.end_row();
            }
            if matches!(self.kind, ArtKind::Model3d | ArtKind::Clips3d) {
                ui.label("Animations").on_hover_text("3 credits each: a clip from Meshy's animation library under this name");
                ui.horizontal_wrapped(|ui| {
                    ui.set_max_width(440.0);
                    let offered: Vec<&str> = if self.kind == ArtKind::Clips3d { CLIP_CHOICES.iter().map(|c| c.0).chain(EXTRAS).collect() } else { EXTRAS.to_vec() };
                    for e in offered {
                        let mut on = self.extras.iter().any(|x| x == e);
                        if ui.checkbox(&mut on, e.replace('_', " ")).changed() {
                            self.extras.retain(|x| x != e);
                            if on {
                                self.extras.push(e.to_string());
                            }
                            self.confirm = false;
                        }
                    }
                });
                ui.end_row();
            }
        });
        let req = self.request();
        let cost = ArtState::cost(&req, &ws.art_settings.model);
        let left = ws.art.budget_left(&ws.art_settings);
        // What stands in the way, said before the button is tried.
        let problem = if !has_key() {
            Some("Your Meshy account's key is missing: enter it above.".to_string())
        } else if cost > left {
            Some(format!("{cost} credits are more than the {left} left of this session's budget (above)."))
        } else {
            req.check().err().map(|e| format!("{e:#}"))
        };
        ui.horizontal(|ui| {
            let label = if self.confirm { format!("Yes, spend {cost} credits") } else { format!("Draw it: {cost} credits") };
            if ui.add_enabled(problem.is_none(), egui::Button::new(egui::RichText::new(label).strong())).clicked() {
                if ws.art_settings.ask && !self.confirm {
                    self.confirm = true;
                } else {
                    let _ = self.draw(ws);
                }
            }
            if self.confirm && ui.button("No").clicked() {
                self.confirm = false;
            }
            ui.weak(format!("{left} credits left of this session's budget"));
        });
        if let Some(p) = problem.as_ref().or(self.message.as_ref()) {
            ui.colored_label(ui.visuals().warn_fg_color, p);
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use editor_core::art::{ArtSettings, Made3d, Picture};
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use std::sync::{Arc, Mutex};

    fn copy(from: &std::path::Path, to: &std::path::Path) {
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

    struct State {
        ws: Workspace,
        page: ArtPage,
        opened: Vec<(RefKind, String)>,
    }

    // (Nothing here clicks *Draw it*: with a real key on this machine that would spend credits.
    // A job that is finished is put there as Meshy's answer would leave it.)
    #[test]
    fn a_workshop_fills_the_form_and_a_finished_job_is_added_to_the_project() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-artpage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.art_settings = ArtSettings { model: "nano-banana-2-lite".into(), budget: 100, ask: true };
        // A finished icon job: its picture as the converter left it.
        let mut png = rustpg2d_engine::imageio::Rgba::new(32, 32);
        png.set(10, 10, [200, 200, 220, 255]);
        let req = ArtRequest { kind: ArtKind::Icon, id: "moon_charm".into(), description: "a silver moon charm".into(), attack: "none".into(), portrait_for: String::new(), by_agent: false, female: false, from: String::new(), extras: Vec::new() };
        let job = Job { id: 1, req, model: "nano-banana-2-lite".into(), cost: 3, pictures: vec![Picture { name: "picture", task: None, status: "SUCCEEDED".into(), progress: 100, png: None, credits: 3 }], stage: Stage::Done, result: Some(png.encode().unwrap()), frame: (32, 32), applied: None, version: 1, told: true, made: Made3d::default() };
        ws.art.jobs.push(Arc::new(Mutex::new(job)));
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                s.ws.history_frame(None);
                if let Some(open) = s.page.show(ui, &mut s.ws) {
                    s.opened.push(open);
                }
            },
            State { ws, page: ArtPage::default(), opened: Vec::new() },
        );
        h.run();
        // Opened from a prop's tab: the kind and the prop are filled in, and the button says what it costs.
        {
            let s = h.state_mut();
            s.page.set_for(RefKind::Prop, "sign_tavern", &s.ws);
        }
        h.run();
        assert_eq!((h.state().page.kind, h.state().page.id.as_str()), (ArtKind::Prop3d, "sign_tavern"));
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "For the prop");
        assert!(h.query_all_by_label("Draw it: 33 credits").next().is_some(), "a picture (3) and a model (30)");
        // From a character's: a portrait of that character.
        {
            let s = h.state_mut();
            s.page.set_for(RefKind::Character, "elder", &s.ws);
        }
        h.run();
        assert_eq!((h.state().page.kind, h.state().page.id.as_str(), h.state().page.portrait_for.as_str()), (ArtKind::Portrait, "elder_portrait", "elder"));
        // The finished job is listed with its result; added, the project has the icon's sheet
        // and an item with it, as one step, and that item's tab is asked for.
        assert!(h.query_all_by_label_contains("Item icon · moon_charm").next().is_some());
        h.get_by_label("The result of job 1");
        h.get_by_label("Add to project").click();
        h.run();
        h.run();
        let s = h.state();
        let item = s.ws.project.db.item("moon_charm").expect("the item is made");
        assert_eq!(item.icon.sheet, "moon_charm");
        assert!(s.ws.project.db.sheets.iter().any(|d| d.id == "moon_charm" && s.ws.project.path(&d.image).is_file()));
        assert_eq!(s.opened, [(RefKind::Item, "moon_charm".to_string())]);
        assert!(h.query_all_by_label("added as moon_charm").next().is_some() && h.query_all_by_label("Add to project").next().is_none());
        assert!(s.ws.timeline.done.last().is_some_and(|c| c.label.contains("generated art")), "{:?}", s.ws.timeline.done.iter().map(|c| &c.label).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(dir);
    }
}
