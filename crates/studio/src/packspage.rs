//! Packs on the Library tab: the packs that are installed, a pack file looked at before it is
//! installed (what it is, what it brings, what in it runs, what the library has differently),
//! the sources the creator added with what they offer; and the *Share…* window, which writes a
//! selection of the project as a pack.
//!
//! What all of that does is `editor_core::packs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use eframe::egui;
use editor_core::packs::{Preview, SameId};
use editor_core::Workspace;
use rustpg2d_engine::library::AssetRef;
use rustpg2d_engine::packs::{IndexEntry, PackManifest, SourceIndex};

/// A pack that was looked at and waits for *Install*.
pub struct Pending {
    pub bytes: Vec<u8>,
    pub preview: Preview,
    /// The index it came from ("" = a file).
    pub source: String,
    pub same: SameId,
    /// An update: what the creator changed and takes the pack's version of all the same, and
    /// what the pack no longer has and is deleted.
    pub take: Vec<AssetRef>,
    pub delete: Vec<AssetRef>,
}

/// Something fetched beside the editor (a source's index, a pack's file).
struct Fetch<T> {
    what: String,
    rx: Receiver<Result<T, String>>,
}

impl<T: Send + 'static> Fetch<T> {
    fn start(ctx: &egui::Context, what: String, job: impl FnOnce() -> Result<T, String> + Send + 'static) -> Fetch<T> {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
        Fetch { what, rx }
    }

    /// The answer, once it is there.
    fn poll(&self) -> Option<Result<T, String>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(format!("{} gave no answer", self.what))),
        }
    }
}

#[derive(Default)]
pub struct PacksPage {
    pub pending: Option<Pending>,
    /// The sources (from the editor's settings; read when first shown).
    sources: Option<Vec<String>>,
    add_source: String,
    /// What each source offers, as last read.
    indexes: HashMap<String, Result<SourceIndex, String>>,
    reading: Option<Fetch<SourceIndex>>,
    getting: Option<(String, Fetch<Vec<u8>>)>,
    /// What the last install, read or download said.
    pub said: Option<Result<String, String>>,
    /// Packs' preview pictures as egui shows them: by pack id and version ("" = the pack looked at).
    pictures: HashMap<String, Vec<egui::TextureHandle>>,
    /// The project's assets that are older than the library's version of their pack (looked
    /// for when the page is first shown and after something was installed).
    pub project: Option<Vec<editor_core::packs::ProjectUpdate>>,
    /// Of those changed in the project too: the ones the library's version is taken of.
    pub project_take: Vec<AssetRef>,
}

/// PNGs as pictures egui can show (what can't be read is left out; none larger than 512 pixels a side).
fn textures(ctx: &egui::Context, key: &str, pngs: impl Iterator<Item = Vec<u8>>) -> Vec<egui::TextureHandle> {
    pngs.enumerate()
        .filter_map(|(k, png)| {
            let mut im = rustpg2d_engine::imageio::Rgba::decode(&png).ok()?;
            let longest = im.w.max(im.h);
            if longest > 512 {
                im = im.resized(im.w * 512 / longest, im.h * 512 / longest);
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([im.w as usize, im.h as usize], &im.px);
            // (Pixel art stays crisp; a larger picture is shown smaller: smoothed.)
            let options = if longest <= 96 { egui::TextureOptions::NEAREST } else { egui::TextureOptions::LINEAR };
            Some(ctx.load_texture(format!("pack picture {key} {k}"), image, options))
        })
        .collect()
}

/// A pack's pictures in a row, each in a box of `side` points.
fn picture_row(ui: &mut egui::Ui, pictures: &[egui::TextureHandle], side: f32, title: &str) {
    for (k, t) in pictures.iter().enumerate() {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, format!("Picture {} of {title}", k + 1)));
        ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(38, 40, 46));
        let size = t.size_vec2();
        let scale = ((side - 6.0) / size.x).min((side - 6.0) / size.y);
        ui.painter().image(t.id(), egui::Rect::from_center_size(rect.center(), size * scale), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    }
}

fn size_text(bytes: u64) -> String {
    if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / (1 << 20) as f64)
    } else {
        format!("{} kB", bytes.div_ceil(1024))
    }
}

fn byline(m: &PackManifest) -> String {
    let mut parts = vec![format!("version {}", m.version)];
    if !m.author.is_empty() {
        parts.push(format!("by {}", m.author));
    }
    if !m.licence.is_empty() {
        parts.push(m.licence.clone());
    }
    parts.join(" · ")
}

impl PacksPage {
    fn sources(&mut self) -> &mut Vec<String> {
        self.sources.get_or_insert_with(|| editor_core::files::load_settings().pack_sources)
    }

    fn save_sources(&mut self) {
        let sources = self.sources().clone();
        editor_core::files::save_settings(&editor_core::files::EditorSettings { pack_sources: sources, ..editor_core::files::load_settings() });
    }

    /// Looks at a pack's bytes: it waits for *Install* then.
    pub fn look(&mut self, ws: &mut Workspace, bytes: Vec<u8>, source: &str) {
        match ws.pack_preview(&bytes) {
            Ok(preview) => {
                let same = SameId::Keep;
                (self.pending, self.said) = (Some(Pending { bytes, preview, source: source.to_string(), same, take: Vec::new(), delete: Vec::new() }), None);
                self.pictures.remove("");
            }
            Err(e) => self.said = Some(Err(e)),
        }
    }

    /// A pack file picked or dropped.
    pub fn open_file(&mut self, ws: &mut Workspace, file: &Path) {
        match std::fs::read(file) {
            Ok(bytes) => self.look(ws, bytes, ""),
            Err(e) => self.said = Some(Err(format!("reading {}: {e}", file.display()))),
        }
    }

    /// Whether a dropped file is one this page takes (a pack, or a file of 0.35).
    pub fn takes(file: &Path) -> bool {
        file.extension().and_then(|e| e.to_str()).is_some_and(|e| ["r2dpack", "r2dplugin", "r2dlib"].contains(&e.to_lowercase().as_str()))
    }

    /// Installs the pack that waits.
    pub fn install(&mut self, ws: &mut Workspace) {
        let Some(p) = self.pending.take() else { return };
        self.project = None;
        // (An update goes asset by asset, with what was ticked; a copy beside is the old way.)
        self.said = Some(if p.preview.update.is_some() && p.same != SameId::Copy { ws.update_pack(&p.bytes, &p.take, &p.delete, &p.source) } else { ws.install_pack(&p.bytes, p.same, &p.source) });
    }

    /// The script's line: `installed=N sources=N pending=TITLE VERSION new/same/differing|- said=…`.
    pub fn status(&mut self, ws: &mut Workspace) -> String {
        let pending = self.pending.as_ref().map_or("-".to_string(), |p| format!("{} {} {}/{}/{} {:?}", p.preview.manifest.title.replace(' ', "_"), p.preview.manifest.version, p.preview.new.len(), p.preview.same.len(), p.preview.differing.len(), p.same));
        let said = match &self.said {
            None => "-".to_string(),
            Some(Ok(m)) => m.chars().take(90).collect(),
            Some(Err(e)) => format!("failed: {}", e.chars().take(120).collect::<String>()),
        };
        format!("installed={} sources={} pending={pending} said={said}", ws.installed_packs().len(), self.sources().len(), )
    }

    /// The script's words: `file PATH` (a pack file looked at), `same keep|replace|copy`,
    /// `take KIND ID` / `delete KIND ID` (an update: the pack's version of what was changed
    /// here; what the pack no longer has), `install`, `cancel`, `add ADDRESS` (a source),
    /// `remove ADDRESS`, `project` (the project's assets older than the library's packs) /
    /// `project PACK [kind:id…]` (takes the library's version, also of those named).
    pub fn word(&mut self, ws: &mut Workspace, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "file" => self.open_file(ws, Path::new(&args[1..].join(" "))),
            // (*Take the one this editor has*: the starter library looked at as a pack.)
            "starter" => {
                let bytes = editor_core::starter::as_file().ok_or("no starter library was found")?.map_err(|e| format!("{e:#}"))?;
                self.look(ws, bytes, "");
            }
            "same" => {
                let same = match arg(1) {
                    "keep" => SameId::Keep,
                    "replace" => SameId::Replace,
                    "copy" => SameId::Copy,
                    _ => return Err("same keep|replace|copy".into()),
                };
                self.pending.as_mut().ok_or("no pack is looked at")?.same = same;
            }
            "take" | "delete" => {
                let kind = rustpg2d_engine::refactor::RefKind::parse(arg(1)).ok_or("take|delete KIND ID")?;
                let p = self.pending.as_mut().ok_or("no pack is looked at")?;
                let list = if arg(0) == "take" { &mut p.take } else { &mut p.delete };
                list.push((kind, arg(2).to_string()));
            }
            "install" => self.install(ws),
            "project" => {
                // `project` lists what is older in the project; `project PACK [kind:id…]` updates it.
                match arg(1) {
                    "" => {
                        let list = ws.project_updates();
                        for u in &list {
                            println!("[packs] {}", u.line());
                        }
                        self.project = Some(list);
                    }
                    pack => {
                        let take: Vec<AssetRef> = args[2..].iter().filter_map(|w| w.split_once(':').and_then(|(k, id)| Some((rustpg2d_engine::refactor::RefKind::parse(k)?, id.to_string())))).collect();
                        self.said = Some(ws.update_project_from(pack, &take));
                        self.project = None;
                    }
                }
            }
            "cancel" => self.pending = None,
            "add" => {
                let address = arg(1).to_string();
                if !address.is_empty() && !self.sources().contains(&address) {
                    self.sources().push(address);
                    self.save_sources();
                }
            }
            "remove" => {
                self.sources().retain(|s| s != arg(1));
                self.save_sources();
            }
            other => return Err(format!("packs {other}: file, same, install, cancel, add, remove")),
        }
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) {
        // What was fetched beside the editor has arrived?
        if let Some(r) = self.reading.as_ref().and_then(|f| f.poll().map(|r| (f.what.clone(), r))) {
            self.indexes.insert(r.0, r.1);
            self.reading = None;
        }
        if let Some((source, r)) = self.getting.as_ref().and_then(|(s, f)| f.poll().map(|r| (s.clone(), r))) {
            self.getting = None;
            match r {
                Ok(bytes) => self.look(ws, bytes, &source),
                Err(e) => self.said = Some(Err(e)),
            }
        }
        match &self.said {
            Some(Ok(m)) => {
                ui.colored_label(egui::Color32::from_rgb(140, 220, 140), m.as_str());
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, e.as_str());
            }
            None => {}
        }
        self.pending_ui(ui, ws);
        self.project_ui(ui, ws);
        // A pack file from the disk.
        ui.horizontal(|ui| {
            ui.strong("Install a pack");
            if let Some(file) = crate::pick::browse(ui, crate::pick::Key::PackFile, "From a file…", "A pack file (.r2dpack), or an asset package, plugin or starter library file of 0.35", || crate::pick::Want::File("Packs", &["r2dpack", "r2dplugin", "r2dlib"]), None) {
                self.open_file(ws, &file);
            }
            ui.weak("or drop the file on the window. It is shown first: what it is, what it brings.");
        });
        ui.add_space(6.0);
        // What is installed.
        let installed = ws.installed_packs();
        ui.strong(match installed.len() {
            0 => "No packs are installed".to_string(),
            1 => "1 pack is installed".to_string(),
            n => format!("{n} packs are installed"),
        });
        // The starter library, when it isn't a pack here yet.
        if !installed.iter().any(|p| p.manifest.id == rustpg2d_engine::packs::STARTER_ID) && self.pending.as_ref().is_none_or(|p| p.preview.manifest.id != rustpg2d_engine::packs::STARTER_ID) {
            ui.group(|ui| {
                ui.label(egui::RichText::new("The starter library").strong());
                ui.weak("Characters, props, items, ground, sounds and music to start from. Installed as a pack, it is listed here and kept up to date like any other; what you changed of it stays yours.");
                let busy = self.getting.is_some();
                ui.horizontal(|ui| {
                    if let Some(found) = editor_core::starter::current() {
                        if ui.add_enabled(!busy, egui::Button::new(format!("Take the one this editor has ({})", found.describe()))).on_hover_text("Shown first, as every pack: what it would add and update").clicked() {
                            self.getting = Some((String::new(), Fetch::start(ui.ctx(), "the starter library".into(), || editor_core::starter::as_file().unwrap_or_else(|| Err(anyhow::anyhow!("no starter library was found"))).map_err(|e| format!("{e:#}")))));
                        }
                    }
                    if ui.add_enabled(!busy, egui::Button::new("Download it")).on_hover_text(format!("The starter library of R(ust)PG2D {} from its release, checked against the release's SHA256SUMS; shown before anything is installed", env!("CARGO_PKG_VERSION"))).clicked() {
                        self.getting = Some(("the R(ust)PG2D release".into(), Fetch::start(ui.ctx(), "the starter library".into(), || editor_core::starter::fetch().map_err(|e| format!("{e:#}")))));
                    }
                });
            });
        }
        let mut forget = None;
        for p in &installed {
            let key = format!("{} {}", p.manifest.id, p.manifest.version);
            if !self.pictures.contains_key(&key) {
                let files = ws.pack_previews(&p.manifest.id);
                self.pictures.insert(key.clone(), textures(ui.ctx(), &key, files.iter().filter_map(|f| std::fs::read(f).ok())));
            }
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    picture_row(ui, &self.pictures[&key], 44.0, &p.manifest.title);
                    ui.label(egui::RichText::new(&p.manifest.title).strong());
                    ui.weak(byline(&p.manifest));
                });
                if !p.manifest.description.is_empty() {
                    ui.label(&p.manifest.description);
                }
                ui.horizontal(|ui| {
                    ui.weak(format!("{} assets{}{}", p.assets.len(), if p.plugins.is_empty() { String::new() } else { format!(" · plugins: {}", p.plugins.join(", ")) }, if p.source.is_empty() { " · from a file".to_string() } else { format!(" · from {}", p.source) }));
                    if ui.small_button("Forget").on_hover_text("Takes the pack off this list. What it brought stays in the library: projects may use it.").clicked() {
                        forget = Some(p.manifest.id.clone());
                    }
                });
            });
        }
        if let Some(id) = forget {
            self.said = Some(ws.forget_pack(&id).map(|()| format!("Forgot the pack {id}")));
        }
        ui.add_space(6.0);
        self.sources_ui(ui, &installed);
    }

    /// The project's assets that came from a pack as an older version than the library has now.
    fn project_ui(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) {
        if !ws.project_open {
            return;
        }
        let list = self.project.get_or_insert_with(|| ws.project_updates()).clone();
        if list.is_empty() {
            return;
        }
        let names = |v: &[AssetRef]| {
            let shown: Vec<String> = v.iter().take(8).map(|(k, id)| format!("{} {id}", k.label().to_lowercase())).collect();
            format!("{}{}", shown.join(", "), if v.len() > 8 { format!(" and {} more", v.len() - 8) } else { String::new() })
        };
        let mut update = None;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new("In this project").strong());
            ui.weak("Assets the project took from a pack, of which the library has a newer version now.");
            for u in &list {
                ui.add_space(4.0);
                ui.label(format!("{}: the project's came as version {}, the library has {}.", u.title, u.from.join(" and "), u.to));
                if !u.plan.updated.is_empty() {
                    ui.label(format!("    Updated ({}), which the project didn't change: {}", u.plan.updated.len(), names(&u.plan.updated)));
                }
                if !u.plan.edited.is_empty() {
                    ui.colored_label(ui.visuals().warn_fg_color, format!("    Changed in the project and by the pack ({}): the project's stay unless you tick them.", u.plan.edited.len()));
                    for a in &u.plan.edited {
                        let mut on = self.project_take.contains(a);
                        if ui.checkbox(&mut on, format!("Take the library's {} {}", a.0.label().to_lowercase(), a.1)).changed() {
                            self.project_take.retain(|x| x != a);
                            if on {
                                self.project_take.push(a.clone());
                            }
                        }
                    }
                }
                if !u.plan.added.is_empty() {
                    ui.weak(format!("    They need {} the project doesn't have yet: {}", u.plan.added.len(), names(&u.plan.added)));
                }
                if ui.button(format!("Update the project's from {}", u.title)).on_hover_text("One step of the history. Pictures, sounds and models that are overwritten are kept in the project's editor/backup folder first.").clicked() {
                    update = Some(u.pack.clone());
                }
            }
        });
        ui.add_space(6.0);
        if let Some(pack) = update {
            let take = std::mem::take(&mut self.project_take);
            self.said = Some(ws.update_project_from(&pack, &take));
            self.project = None;
        }
    }

    /// The pack that was looked at: what it is and what installing it does.
    fn pending_ui(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) {
        let Some(p) = self.pending.as_mut() else { return };
        let (mut install, mut cancel) = (false, false);
        let m = &p.preview.manifest;
        let pictures = self.pictures.entry(String::new()).or_insert_with(|| textures(ui.ctx(), "looked at", p.preview.pictures.iter().cloned())).clone();
        egui::Frame::group(ui.style()).stroke(egui::Stroke::new(1.0_f32, ui.visuals().selection.bg_fill)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&m.title).heading());
                ui.weak(byline(m));
            });
            if !pictures.is_empty() {
                ui.horizontal(|ui| picture_row(ui, &pictures, 72.0, &m.title));
            }
            let starter = m.id == rustpg2d_engine::packs::STARTER_ID;
            if m.legacy && !starter {
                ui.weak("A file of R(ust)PG2D 0.35: it has no pack manifest of its own.");
            }
            if !m.description.is_empty() {
                ui.label(&m.description);
            }
            if !m.attribution.is_empty() {
                ui.weak(format!("Credit: {}", m.attribution));
            }
            ui.label(format!("It holds {}.", if p.preview.holds.is_empty() { "nothing".to_string() } else { p.preview.holds.join(", ") }));
            // (The starter library is the editor's own: its script functions need no warning.)
            if !p.preview.code.is_empty() && !starter {
                let shown: Vec<&str> = p.preview.code.iter().take(6).map(String::as_str).collect();
                let more = p.preview.code.len().saturating_sub(6);
                ui.colored_label(ui.visuals().warn_fg_color, format!("Some of it runs in your games: {}{}. Install it only from someone you trust.", shown.join(", "), if more > 0 { format!(" and {more} more") } else { String::new() })).on_hover_text(p.preview.code.join("\n"));
            }
            let (new, same, differing) = (p.preview.new.len(), p.preview.same.len(), p.preview.differing.len());
            let names = |v: &[AssetRef]| {
                let shown: Vec<String> = v.iter().take(8).map(|(k, id)| format!("{} {id}", k.label().to_lowercase())).collect();
                format!("{}{}", shown.join(", "), if v.len() > 8 { format!(" and {} more", v.len() - 8) } else { String::new() })
            };
            match &p.preview.update {
                // An update: asset by asset.
                Some(u) => {
                    match &p.preview.installed {
                        Some(v) if *v == m.version => ui.label(format!("Version {v} is installed already.")),
                        Some(v) => ui.label(format!("Version {v} is installed: this is {}.", m.version)),
                        // (The starter library, until now part of every library without being a pack.)
                        None => ui.label("Your library has what it had of this already. Installed as a pack, it is kept up to date like the others."),
                    };
                    if u.is_empty() && p.preview.installed.is_some() {
                        ui.weak("Your library has everything of it as it is: there is nothing to do.");
                    } else if u.is_empty() {
                        ui.weak("Everything of it is there as it is: installing only puts it on the list.");
                    }
                    if !u.added.is_empty() {
                        ui.label(format!("New in it ({}): {}", u.added.len(), names(&u.added)));
                    }
                    if !u.updated.is_empty() {
                        ui.label(format!("Updated ({}), which you didn't change: {}", u.updated.len(), names(&u.updated)));
                    }
                    if !u.edited.is_empty() {
                        ui.colored_label(ui.visuals().warn_fg_color, format!("Changed by you and by the pack ({}): yours stay unless you tick them.", u.edited.len()));
                        for a in &u.edited {
                            let mut on = p.take.contains(a);
                            if ui.checkbox(&mut on, format!("Take the pack's {} {}", a.0.label().to_lowercase(), a.1)).on_hover_text("Your version is overwritten (the library is copied first)").changed() {
                                p.take.retain(|x| x != a);
                                if on {
                                    p.take.push(a.clone());
                                }
                            }
                        }
                    }
                    if !u.retired.is_empty() {
                        ui.label(format!("No longer in the pack ({}): they stay unless you tick them.", u.retired.len()));
                        for r in &u.retired {
                            let a = &r.asset;
                            let mut on = p.delete.contains(a);
                            let label = format!("Delete the {} {}{}", a.0.label().to_lowercase(), a.1, if r.untouched { "" } else { " (you changed it)" });
                            let free = r.used_by.is_empty();
                            if ui.add_enabled(free, egui::Checkbox::new(&mut on, label)).on_disabled_hover_text(format!("Still needed by {}", r.used_by.join(", "))).changed() {
                                p.delete.retain(|x| x != a);
                                if on {
                                    p.delete.push(a.clone());
                                }
                            }
                        }
                    }
                    ui.weak("The library's folder is copied first; what was done says where to.");
                }
                // A first install: what the library has under the same ids.
                None => {
                    ui.label(format!("New to the library: {new} · there as they are: {same} · there under the same id, but different: {differing}."));
                    if differing > 0 {
                        ui.weak(format!("Different: {}", names(&p.preview.differing)));
                        ui.horizontal(|ui| {
                            ui.label("Those");
                            ui.radio_value(&mut p.same, SameId::Keep, "stay as the library has them").on_hover_text("What you have is kept; the pack's other assets use the library's");
                            ui.radio_value(&mut p.same, SameId::Replace, "are replaced by the pack's").on_hover_text("The library's versions are overwritten (the library is copied first)");
                            ui.radio_value(&mut p.same, SameId::Copy, "come in beside them, as copies").on_hover_text("Under new ids: both versions are there");
                        });
                    }
                }
            }
            if !p.preview.missing.is_empty() {
                ui.colored_label(ui.visuals().error_fg_color, format!("It needs {}: install that first.", p.preview.missing.iter().map(|n| if n.version.is_empty() { n.id.clone() } else { format!("{} {} or newer", n.id, n.version) }).collect::<Vec<_>>().join(" and ")));
            }
            ui.horizontal(|ui| {
                let nothing = p.preview.installed.is_some() && p.preview.update.as_ref().is_some_and(|u| u.is_empty());
                install = ui.add_enabled(p.preview.missing.is_empty() && !nothing, egui::Button::new(egui::RichText::new(if p.preview.installed.is_some() { "Update it" } else { "Install it" }).strong())).clicked();
                cancel = ui.button("Not now").clicked();
                ui.weak(size_text(p.bytes.len() as u64));
            });
        });
        ui.add_space(6.0);
        if install {
            self.install(ws);
        } else if cancel {
            self.pending = None;
        }
    }

    /// The sources: each with what it offers, read when asked.
    fn sources_ui(&mut self, ui: &mut egui::Ui, installed: &[editor_core::packs::Installed]) {
        ui.strong("Sources");
        ui.weak("A source is a place packs are offered at: a web address of an index.ron, or a folder with one (what Share… writes). Downloads are checked against the size and SHA-256 its index names.");
        let sources = self.sources().clone();
        let busy = self.reading.is_some() || self.getting.is_some();
        let (mut read, mut remove, mut get): (Option<String>, Option<String>, Option<(String, IndexEntry)>) = (None, None, None);
        for source in &sources {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let title = match self.indexes.get(source) {
                        Some(Ok(index)) => index.title.clone(),
                        _ => source.clone(),
                    };
                    ui.label(egui::RichText::new(title).strong());
                    if ui.add_enabled(!busy, egui::Button::new(if self.indexes.contains_key(source) { "Read again" } else { "See what it offers" })).clicked() {
                        read = Some(source.clone());
                    }
                    if ui.small_button("Remove").on_hover_text("Takes the source off this list (what was installed from it stays)").clicked() {
                        remove = Some(source.clone());
                    }
                    if self.reading.as_ref().is_some_and(|f| f.what == *source) {
                        ui.spinner();
                    }
                });
                ui.weak(source);
                match self.indexes.get(source) {
                    Some(Err(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, e.as_str());
                    }
                    Some(Ok(index)) => {
                        let offered = index.newest();
                        if offered.is_empty() {
                            ui.weak("It offers nothing this editor can use.");
                        }
                        for e in offered {
                            ui.horizontal(|ui| {
                                let have = installed.iter().find(|p| p.manifest.id == e.manifest.id).map(|p| p.manifest.version.clone());
                                ui.label(&e.manifest.title);
                                ui.weak(format!("{} · {} · {}", byline(&e.manifest), if e.holds.is_empty() { "?".to_string() } else { e.holds.join(", ") }, size_text(e.size)));
                                let (label, tip) = match &have {
                                    Some(v) if *v == e.manifest.version => ("Installed", "This version is installed: look at it again"),
                                    Some(_) => ("Update…", "A newer version than the one installed: downloaded, checked, and shown before anything changes"),
                                    None => ("Install…", "Downloaded, checked against the index, and shown before anything is installed"),
                                };
                                if let Some(v) = have.filter(|v| *v != e.manifest.version) {
                                    ui.colored_label(ui.visuals().warn_fg_color, format!("installed: {v}"));
                                }
                                if ui.add_enabled(!busy, egui::Button::new(label)).on_hover_text(tip).clicked() {
                                    get = Some((source.clone(), e.clone()));
                                }
                            });
                        }
                    }
                    None => {}
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.add_source).hint_text("https://…/index.ron, or a folder").desired_width(360.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "A source's address"));
            let address = self.add_source.trim().to_string();
            if ui.add_enabled(!address.is_empty() && !sources.contains(&address), egui::Button::new("Add source")).clicked() {
                self.sources().push(address.clone());
                self.save_sources();
                self.add_source.clear();
                read = Some(address);
            }
            if let Some(folder) = crate::pick::browse(ui, crate::pick::Key::PackSource, "A folder…", "A folder with an index.ron and its packs", || crate::pick::Want::Folder, None) {
                self.add_source = folder.display().to_string();
            }
        });
        if let Some((_, f)) = &self.getting {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Downloading {}…", f.what));
            });
        }
        if let Some(source) = remove {
            self.sources().retain(|s| *s != source);
            self.indexes.remove(&source);
            self.save_sources();
        }
        if let Some(source) = read {
            let address = source.clone();
            self.reading = Some(Fetch::start(ui.ctx(), source, move || editor_core::packs::read_index(&address)));
        }
        if let Some((source, entry)) = get {
            let (address, title) = (source.clone(), entry.manifest.title.clone());
            self.getting = Some((source, Fetch::start(ui.ctx(), title, move || editor_core::packs::download(&entry, &address))));
        }
    }
}

/// Licences offered in *Share…* (any other can be typed).
const LICENCES: [(&str, &str); 5] = [
    ("CC0", "No rights reserved: anyone may use it for anything, without credit"),
    ("CC BY 4.0", "Anyone may use and change it, also in games they sell, when they credit you"),
    ("CC BY-SA 4.0", "As CC BY, and what is made from it must be shared under the same licence"),
    ("CC BY-NC 4.0", "As CC BY, but not in anything that is sold"),
    ("All rights reserved", "Nobody may use it without asking you"),
];

/// *Share…*: a selection of the project as a pack.
pub struct ShareWindow {
    pub manifest: PackManifest,
    /// The id follows the title until it is typed.
    id_typed: bool,
    pub assets: Vec<AssetRef>,
    pub plugins: Vec<String>,
    pub out: String,
    /// The project's plugins, to tick (id and name).
    offered: Vec<(String, String)>,
    /// What the selection brings with what it needs (worked out when the window opens, and
    /// again when a plugin is ticked).
    holds: Result<Vec<String>, String>,
    pub error: Option<String>,
    /// The file that was written.
    pub done: Option<PathBuf>,
}

impl ShareWindow {
    /// The window for `assets`, with who shares and where as entered last.
    pub fn of(ws: &Workspace, assets: Vec<AssetRef>) -> ShareWindow {
        let s = editor_core::files::load_settings();
        let title = match assets.as_slice() {
            [(kind, id)] => editor_core::defs::list(&ws.project.db, *kind).into_iter().find(|(i, _)| i == id).map(|(_, name)| name).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone()),
            _ => format!("{} assets", ws.project.config.name),
        };
        let manifest = PackManifest { author: s.share_author, licence: if s.share_licence.is_empty() { "CC BY 4.0".into() } else { s.share_licence }, attribution: s.share_attribution, ..PackManifest::new(&title) };
        let out = if s.share_folder.is_empty() { rustpg2d_engine::paths::home().map(|h| h.join("rustpg2d").join("shared").display().to_string()).unwrap_or_default() } else { s.share_folder };
        let holds = ws.make_pack(&manifest, &assets, &[], Vec::new()).map(|p| rustpg2d_engine::packs::holds(&p));
        let offered = rustpg2d_engine::plugins::installed(&ws.project.root).into_iter().map(|id| (rustpg2d_engine::plugins::read_def(&ws.project.root, &id).map(|d| d.name).ok().filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone()), id)).map(|(name, id)| (id, name)).collect();
        ShareWindow { manifest, id_typed: false, assets, plugins: Vec::new(), offered, out, holds, error: None, done: None }
    }

    /// Writes the pack. Remembers who shares and where.
    pub fn share(&mut self, ws: &mut Workspace) -> Result<PathBuf, String> {
        let previews = ws.pack_pictures(&self.assets);
        let r = ws.share_pack(&self.manifest, &self.assets, &self.plugins, previews, Path::new(self.out.trim()));
        match &r {
            Ok(file) => {
                (self.done, self.error) = (Some(file.clone()), None);
                let m = &self.manifest;
                editor_core::files::save_settings(&editor_core::files::EditorSettings { share_author: m.author.clone(), share_licence: m.licence.clone(), share_attribution: m.attribution.clone(), share_folder: self.out.trim().to_string(), ..editor_core::files::load_settings() });
            }
            Err(e) => self.error = Some(e.clone()),
        }
        r
    }

    /// The window. False when it was closed.
    pub fn show(&mut self, ctx: &egui::Context, ws: &mut Workspace) -> bool {
        let mut open = true;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return false;
        }
        egui::Window::new("Share as a pack").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, -30.0]).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.set_width(520.0);
            let names: Vec<String> = self.assets.iter().take(6).map(|(k, id)| format!("{} {id}", k.label().to_lowercase())).collect();
            ui.label(format!("{}{}", names.join(", "), if self.assets.len() > 6 { format!(" and {} more", self.assets.len() - 6) } else { String::new() }));
            match &self.holds {
                Ok(h) => ui.weak(format!("With what they need, the pack holds {}.", h.join(", "))),
                Err(e) => ui.colored_label(ui.visuals().error_fg_color, e.as_str()),
            };
            // The project's plugins can go along.
            if !self.offered.is_empty() {
                let mut changed = false;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Plugins too:");
                    for (id, name) in &self.offered {
                        let mut on = self.plugins.contains(id);
                        if ui.checkbox(&mut on, name.as_str()).on_hover_text(format!("The plugin {id} with its scripts and settings: who installs the pack is told that it runs")).changed() {
                            self.plugins.retain(|p| p != id);
                            if on {
                                self.plugins.push(id.clone());
                            }
                            changed = true;
                        }
                    }
                });
                if changed {
                    self.holds = ws.make_pack(&self.manifest, &self.assets, &self.plugins, Vec::new()).map(|p| rustpg2d_engine::packs::holds(&p));
                }
            }
            ui.add_space(6.0);
            let m = &mut self.manifest;
            egui::Grid::new("share form").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                let l = ui.label("Title");
                if ui.add(egui::TextEdit::singleline(&mut m.title).desired_width(320.0)).labelled_by(l.id).changed() && !self.id_typed {
                    m.id = rustpg2d_engine::packs::id_from(&m.title);
                }
                ui.end_row();
                let l = ui.label("Id").on_hover_text("What the pack is filed under everywhere: versions of one pack share it. Lower-case letters, digits, - _ and dots; your name first keeps it yours (anna.forest-props).");
                if ui.add(egui::TextEdit::singleline(&mut m.id).desired_width(320.0)).labelled_by(l.id).changed() {
                    self.id_typed = true;
                }
                ui.end_row();
                let l = ui.label("Version").on_hover_text("Raise it when you share it again: who has an older one is offered the update");
                ui.add(egui::TextEdit::singleline(&mut m.version).desired_width(100.0)).labelled_by(l.id);
                ui.end_row();
                let l = ui.label("By");
                ui.add(egui::TextEdit::singleline(&mut m.author).hint_text("your name").desired_width(320.0)).labelled_by(l.id);
                ui.end_row();
                let l = ui.label("Licence").on_hover_text("What others may do with it");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut m.licence).desired_width(180.0)).labelled_by(l.id);
                    egui::ComboBox::from_id_salt("share licence").selected_text("Pick one").show_ui(ui, |ui| {
                        for (name, means) in LICENCES {
                            if ui.selectable_label(m.licence == name, name).on_hover_text(means).clicked() {
                                m.licence = name.to_string();
                            }
                        }
                    })
                    .response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "Pick a licence"));
                });
                ui.end_row();
                let l = ui.label("Credit line").on_hover_text("What a game that uses it shows in its credits");
                ui.add(egui::TextEdit::singleline(&mut m.attribution).hint_text("Forest props by Anna").desired_width(320.0)).labelled_by(l.id);
                ui.end_row();
                let l = ui.label("Description");
                ui.add(egui::TextEdit::multiline(&mut m.description).desired_rows(2).desired_width(320.0)).labelled_by(l.id);
                ui.end_row();
                let l = ui.label("Into the folder").on_hover_text("The pack file is written there and listed in the index.ron there: the folder is a source, as it is or uploaded to any web space");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.out).desired_width(320.0)).labelled_by(l.id);
                    let start = Some(PathBuf::from(self.out.trim())).filter(|p| !p.as_os_str().is_empty());
                    if let Some(folder) = crate::pick::browse(ui, crate::pick::Key::ShareFolder, "Browse…", "The folder the pack is written into", || crate::pick::Want::Folder, start) {
                        self.out = folder.display().to_string();
                    }
                });
                ui.end_row();
            });
            ui.add_space(6.0);
            let problem = self.manifest.problem().or_else(|| self.out.trim().is_empty().then(|| "Say where the pack is written".to_string())).or_else(|| self.holds.as_ref().err().cloned());
            if let Some(p) = problem.as_ref().or(self.error.as_ref()) {
                ui.colored_label(ui.visuals().warn_fg_color, p);
            }
            if let Some(file) = &self.done {
                ui.colored_label(egui::Color32::from_rgb(140, 220, 140), format!("Written: {}", file.display()));
                ui.weak("Give the file to someone (they drop it on their editor), or upload the folder and give them its address as a source.");
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(problem.is_none(), egui::Button::new(egui::RichText::new(if self.done.is_some() { "Write it again" } else { "Write the pack" }).strong())).clicked() {
                    let _ = self.share(ws);
                }
                if ui.button(if self.done.is_some() { "Close" } else { "Cancel" }).clicked() {
                    open = false;
                }
            });
        });
        open
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use rustpg2d_engine::refactor::RefKind;

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

    struct State {
        ws: Workspace,
        page: PacksPage,
        share: Option<ShareWindow>,
    }

    /// The example as the project and an empty library of its own.
    fn scratch(tag: &str) -> (Workspace, PathBuf) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-packspage-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir.join("game"));
        std::fs::create_dir_all(dir.join("library")).unwrap();
        editor_core::files::create_project(&dir.join("library")).unwrap();
        let mut ws = Workspace::new(dir.join("game"), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&dir.join("library")).unwrap());
        (ws, dir)
    }

    #[test]
    fn a_selection_is_shared_in_the_window_and_the_pack_looked_at_and_installed() {
        let (ws, dir) = scratch("share");
        // (The sources and what was entered last come from the editor's settings: this test
        // leaves both alone, it fills the window's own fields and looks at a file.)
        let mut share = ShareWindow::of(&ws, vec![(RefKind::Prop, "chest".to_string())]);
        assert_eq!((share.manifest.title.as_str(), share.manifest.id.as_str(), share.manifest.version.as_str()), ("Chest", "chest", "1.0"));
        assert!(share.holds.as_ref().is_ok_and(|h| h.iter().any(|x| x == "1 prop")), "{:?}", share.holds);
        share.out = dir.join("shared").display().to_string();
        (share.manifest.author, share.manifest.licence) = ("Anna".into(), "CC BY 4.0".into());
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                s.page.pending_ui(ui, &mut s.ws);
                if let Some(Ok(m)) = &s.page.said {
                    ui.label(m.as_str());
                }
                if let Some(w) = &mut s.share {
                    w.show(ui.ctx(), &mut s.ws);
                }
            },
            State { ws, page: PacksPage::default(), share: Some(share) },
        );
        h.run();
        // The title typed anew: the id follows it.
        let title = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Title");
        title.focus();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Title").type_text("Sea chests");
        h.run();
        assert_eq!(h.state().share.as_ref().unwrap().manifest.id, "sea-chests");
        assert!(h.query_all_by_label_contains("With what they need, the pack holds").next().is_some());
        // (Written through the window's own function: its button would also save who shares
        // into the editor's settings, which a test must not touch.)
        let file = {
            let s = h.state_mut();
            let w = s.share.as_ref().unwrap();
            s.ws.share_pack(&w.manifest, &w.assets, &w.plugins, Vec::new(), Path::new(&w.out)).unwrap()
        };
        assert_eq!(file, dir.join("shared/sea-chests-1.0.r2dpack"));
        h.state_mut().share = None;
        // The file looked at: what it is and brings is said before anything is installed.
        {
            let s = h.state_mut();
            s.page.open_file(&mut s.ws, &file);
        }
        h.run();
        h.get_by_label("Sea chests");
        assert!(h.query_all_by_label_contains("by Anna · CC BY 4.0").next().is_some());
        assert!(h.query_all_by_label_contains("New to the library:").next().is_some());
        assert!(h.state().ws.lib.lib.as_ref().unwrap().db.prop("chest").is_none(), "nothing is installed yet");
        h.get_by_label("Install it").click();
        h.run();
        h.run();
        let s = h.state();
        assert!(s.ws.lib.lib.as_ref().unwrap().db.prop("chest").is_some() && s.page.pending.is_none());
        assert!(h.query_all_by_label_contains("Installed Sea chests 1.0").next().is_some());
        // The same file again: installed already; *Not now* leaves it.
        {
            let s = h.state_mut();
            s.page.open_file(&mut s.ws, &file);
        }
        h.run();
        assert!(h.query_all_by_label_contains("Version 1.0 is installed already").next().is_some());
        assert!(h.query_all_by_label_contains("there is nothing to do").next().is_some());
        h.get_by_label("Not now").click();
        h.run();
        assert!(h.state().page.pending.is_none());
        // A version 1.1 with a renamed chest, while the library's chest was changed here too:
        // the update says so and keeps the library's, unless the pack's is ticked.
        let v2 = {
            let s = h.state_mut();
            let mut value = s.ws.def_value(RefKind::Prop, "chest").unwrap();
            value["name"] = "Sea chest".into();
            s.ws.set_def_value(RefKind::Prop, "chest", value).unwrap();
            let lib = s.ws.lib.lib.as_mut().unwrap();
            lib.db.props.iter_mut().find(|p| p.id == "chest").unwrap().name = "My chest".into();
            lib.save_defs().unwrap();
            let m = PackManifest { author: "Anna".into(), version: "1.1".into(), ..PackManifest::new("Sea chests") };
            s.ws.share_pack(&m, &[(RefKind::Prop, "chest".to_string())], &[], Vec::new(), &dir.join("shared")).unwrap()
        };
        {
            let s = h.state_mut();
            s.page.open_file(&mut s.ws, &v2);
        }
        h.run();
        assert!(h.query_all_by_label_contains("Version 1.0 is installed: this is 1.1.").next().is_some());
        assert!(h.query_all_by_label_contains("Changed by you and by the pack (1)").next().is_some());
        h.get_by_label("Take the pack's prop chest").click();
        h.run();
        h.get_by_label("Update it").click();
        h.run();
        h.run();
        assert_eq!(h.state().ws.lib.lib.as_ref().unwrap().db.prop("chest").unwrap().name, "Sea chest");
        assert!(h.query_all_by_label_contains("Updated Sea chests from 1.0 to 1.1").next().is_some());
        // What is no pack says so.
        std::fs::write(dir.join("notes.r2dpack"), b"hello").unwrap();
        {
            let s = h.state_mut();
            s.page.open_file(&mut s.ws, &dir.join("notes.r2dpack"));
        }
        assert!(h.state().page.said.as_ref().is_some_and(|r| r.is_err()) && PacksPage::takes(Path::new("a.R2DPACK")) && !PacksPage::takes(Path::new("a.png")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
