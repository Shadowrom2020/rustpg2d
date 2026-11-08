//! Quick map switching and the Transition tool (two-way links between maps, e.g. on roads).

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::map::{Map, Zone, ZoneKind};

use crate::app::{Editor, Pending, Selection};

/// Where the Transition tool is.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkStage {
    /// The exit area is drawn; the target map is being chosen.
    Choose,
    /// On the target map: drag the other end of the transition.
    PickArrival,
}

/// The first end of a transition being set up.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkSource {
    /// An area just drawn with the Transition tool (tiles: x, y, w, h).
    Area((u32, u32, u32, u32)),
    /// An existing transition zone (by name).
    Zone(String),
}

#[derive(Clone, Debug)]
pub struct LinkDraft {
    pub from_map: String,
    pub source: LinkSource,
    pub target: String,
    pub stage: LinkStage,
}

/// Quick switcher (Ctrl+M) state.
#[derive(Default)]
pub struct QuickSwitch {
    pub open: bool,
    pub query: String,
}

/// Inspector requests for a transition zone (handled after the panel).
#[derive(Clone, Debug)]
pub enum TransitionReq {
    /// Make the counterpart (map, zone) point back at this zone.
    LinkBack { zone: String, map: String, other: String },
    /// Draw the other end on `map`.
    DrawOther { zone: String, map: String },
    /// Clear this zone's link (and the counterpart's, if it pointed here).
    Unlink { zone: String },
}

thread_local! {
    static TRANS_REQ: std::cell::RefCell<Option<TransitionReq>> = const { std::cell::RefCell::new(None) };
}

pub fn request_transition(r: TransitionReq) {
    TRANS_REQ.with(|q| *q.borrow_mut() = Some(r));
}

/// A zone name not used on `map` yet.
pub(crate) fn unique_zone_name(map: &Map, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 2;
    while map.zones.iter().any(|z| z.name == name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    name
}

/// Adds a transition zone to `map` leading to `target_map`:`target_zone`. Returns its name.
pub(crate) fn add_transition(map: &mut Map, rect: (u32, u32, u32, u32), target_map: &str, target_zone: &str) -> String {
    let name = unique_zone_name(map, &format!("to_{target_map}"));
    let uid = map.alloc_uid();
    map.zones.push(Zone {
        uid,
        name: name.clone(),
        x: rect.0,
        y: rect.1,
        w: rect.2,
        h: rect.3,
        kind: ZoneKind::Transition { target_map: target_map.into(), target_zone: target_zone.into() },
        triggers: vec![],
        loot: None,
    });
    name
}

/// Points the transition zone `zone` on `map` at `target_map`:`target_zone`.
pub(crate) fn set_target(map: &mut Map, zone: &str, target_map: &str, target_zone: &str) -> bool {
    match map.zones.iter_mut().find(|z| z.name == zone) {
        Some(z) => {
            z.kind = ZoneKind::Transition { target_map: target_map.into(), target_zone: target_zone.into() };
            true
        }
        None => false,
    }
}

impl Editor {
    /// Opens another map (asking about unsaved changes); with `spawn`, centers on that zone.
    pub fn goto_map(&mut self, map: &str, spawn: Option<&str>) {
        self.request(Pending::OpenAt(map.to_string(), spawn.unwrap_or_default().to_string()));
    }

    /// After opening a map: center on a zone by name and select it.
    pub fn focus_zone(&mut self, name: &str) {
        let ts = self.ts();
        let Some(doc) = &self.doc else { return };
        if let Some(z) = doc.map.zones.iter().find(|z| z.name == name) {
            let r = z.pixel_rect(ts);
            self.cam.center = macroquad::math::vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
            self.sel = Selection::Zone(z.uid);
        }
    }

    /// The previous / next map in the list (Ctrl+PgUp / Ctrl+PgDn).
    pub fn cycle_map(&mut self, step: i32) {
        if self.maps.is_empty() {
            return;
        }
        let cur = self.doc.as_ref().and_then(|d| self.maps.iter().position(|m| *m == d.map.name)).unwrap_or(0) as i32;
        let n = self.maps.len() as i32;
        let next = self.maps[(cur + step).rem_euclid(n) as usize].clone();
        self.goto_map(&next, None);
    }

    /// Transition tool: an area was dragged on the current map.
    pub fn link_area(&mut self, rect: (u32, u32, u32, u32)) {
        let Some(cur) = self.doc.as_ref().map(|d| d.map.name.clone()) else { return };
        match self.link.clone() {
            Some(d) if d.stage == LinkStage::PickArrival && d.target == cur => self.finish_link(d, rect),
            _ => {
                let target = self.maps.iter().find(|m| **m != cur).cloned().unwrap_or(cur.clone());
                self.link = Some(LinkDraft { from_map: cur, source: LinkSource::Area(rect), target, stage: LinkStage::Choose });
            }
        }
    }

    /// Step 2 of a transition: open the target map and wait for the other end to be drawn.
    pub fn link_next(&mut self, target: &str) {
        let Some(mut d) = self.link.clone() else { return };
        d.target = target.to_string();
        if self.doc.as_ref().is_some_and(|doc| doc.is_dirty()) {
            self.save_map();
        }
        d.stage = LinkStage::PickArrival;
        self.tool = crate::app::Tool::Link;
        self.link = Some(d);
        if self.doc.as_ref().is_none_or(|doc| doc.map.name != target) {
            self.open_map(target);
        }
    }

    /// Creates the other end on the open (target) map and links both ends.
    fn finish_link(&mut self, d: LinkDraft, rect: (u32, u32, u32, u32)) {
        let Some(doc) = &mut self.doc else { return };
        let before = crate::doc::Objects::of(&doc.map);
        let (a, b) = (d.from_map.clone(), doc.map.name.clone());
        // The zone on map A that this new end leads to (created now if it was just drawn).
        let res: anyhow::Result<(String, String)> = (|| {
            let new_b = unique_zone_name(&doc.map, &format!("to_{a}"));
            let a_zone = if a == b {
                let a_zone = match &d.source {
                    LinkSource::Area(r) => add_transition(&mut doc.map, *r, &b, &new_b),
                    LinkSource::Zone(z) => {
                        set_target(&mut doc.map, z, &b, &new_b);
                        z.clone()
                    }
                };
                add_transition(&mut doc.map, rect, &a, &a_zone);
                a_zone
            } else {
                let mut src = self.project.load_map(&a)?;
                let a_zone = match &d.source {
                    LinkSource::Area(r) => add_transition(&mut src, *r, &b, &new_b),
                    LinkSource::Zone(z) => {
                        if !set_target(&mut src, z, &b, &new_b) {
                            anyhow::bail!("'{a}' has no zone '{z}' anymore");
                        }
                        z.clone()
                    }
                };
                self.project.save_map(&src)?;
                add_transition(&mut doc.map, rect, &a, &a_zone);
                a_zone
            };
            Ok((a_zone, new_b))
        })();
        doc.record_objects(before, "transition");
        self.link = None;
        match res {
            Ok((a_zone, b_zone)) => {
                if let Some(z) = doc.map.zones.iter().find(|z| z.name == b_zone) {
                    self.sel = Selection::Zone(z.uid);
                }
                self.save_map();
                self.set_status(format!("Map transition: '{a}' {a_zone} \u{E0A0} '{b}' {b_zone}"));
            }
            Err(e) => self.set_status(format!("Could not link the maps: {e:#}")),
        }
        self.refresh_refs();
    }

    /// Carries out an inspector request for a transition zone on the open map.
    pub fn handle_transition_request(&mut self) {
        let Some(r) = TRANS_REQ.with(|q| q.borrow_mut().take()) else { return };
        let Some(cur) = self.doc.as_ref().map(|d| d.map.name.clone()) else { return };
        // Changes the zone `zone` on `map` (the open document or a map on disk).
        let edit = |ed: &mut Editor, map: &str, f: &mut dyn FnMut(&mut Map)| -> anyhow::Result<()> {
            if map == cur {
                let doc = ed.doc.as_mut().unwrap();
                let before = crate::doc::Objects::of(&doc.map);
                f(&mut doc.map);
                doc.record_objects(before, "transition");
                Ok(())
            } else {
                let mut m = ed.project.load_map(map)?;
                f(&mut m);
                ed.project.save_map(&m)
            }
        };
        let res: anyhow::Result<String> = match r {
            TransitionReq::LinkBack { zone, map, other } => {
                let c = cur.clone();
                edit(self, &cur, &mut |m| {
                    set_target(m, &zone, &map, &other);
                })
                .and_then(|_| {
                    edit(self, &map, &mut |m| {
                        set_target(m, &other, &c, &zone);
                    })
                })
                .map(|_| format!("Linked both ways: '{cur}' {zone} \u{E0A0} '{map}' {other}"))
            }
            TransitionReq::Unlink { zone } => {
                let target = self.doc.as_ref().and_then(|d| d.map.zones.iter().find(|z| z.name == zone)).and_then(|z| match &z.kind {
                    ZoneKind::Transition { target_map, target_zone } => Some((target_map.clone(), target_zone.clone())),
                    _ => None,
                });
                let c = cur.clone();
                let z2 = zone.clone();
                edit(self, &cur, &mut |m| {
                    set_target(m, &zone, "", "");
                })
                .and_then(|_| match &target {
                    Some((tm, tz)) if !tm.is_empty() => edit(self, tm, &mut |m| {
                        let back = m.zones.iter().any(|z| z.name == *tz && matches!(&z.kind, ZoneKind::Transition { target_map, target_zone } if *target_map == c && *target_zone == z2));
                        if back {
                            set_target(m, tz, "", "");
                        }
                    }),
                    _ => Ok(()),
                })
                .map(|_| format!("Unlinked {zone}"))
            }
            TransitionReq::DrawOther { zone, map } => {
                self.link = Some(LinkDraft { from_map: cur.clone(), source: LinkSource::Zone(zone), target: map.clone(), stage: LinkStage::Choose });
                self.link_next(&map);
                Ok(format!("Drag the other end of the transition on '{map}'"))
            }
        };
        match res {
            Ok(m) => self.set_status(m),
            Err(e) => self.set_status(format!("Transition: {e:#}")),
        }
        self.refresh_refs();
    }

    /// Windows: the transition draft and the quick map switcher.
    pub fn map_windows(&mut self, ctx: &egui::Context) {
        if let Some(mut d) = self.link.clone() {
            if d.stage == LinkStage::Choose {
                let mut open = true;
                let mut next = false;
                egui::Window::new("New map transition").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, [0.0, 60.0]).show(ctx, |ui| {
                    let what = match &d.source {
                        LinkSource::Area((x, y, w, h)) => format!("the area at {x},{y} ({w}×{h} tiles)"),
                        LinkSource::Zone(z) => format!("the zone {z}"),
                    };
                    ui.label(format!("Players leave '{}' through {what}.", d.from_map));
                    ui.horizontal(|ui| {
                        ui.label("Leads to map");
                        egui::ComboBox::from_id_salt("link_target").selected_text(&d.target).show_ui(ui, |ui| {
                            for m in &self.maps {
                                let label = if *m == d.from_map { format!("{m} (same map)") } else { m.clone() };
                                ui.selectable_value(&mut d.target, m.clone(), label);
                            }
                        });
                    });
                    ui.weak("Next, the target map opens: drag the other end there (e.g. where the road enters). A transition zone is created on both maps, each leading to the other; players arrive next to the other end.");
                    ui.horizontal(|ui| {
                        if ui.button(format!("Next: arrival on '{}' \u{E3D0}", d.target)).clicked() {
                            next = true;
                        }
                        if ui.button("Cancel").clicked() {
                            self.link = None;
                        }
                    });
                });
                if !open {
                    self.link = None;
                } else if next {
                    let target = d.target.clone();
                    self.link = Some(d);
                    self.link_next(&target);
                } else if self.link.is_some() {
                    self.link = Some(d);
                }
            } else {
                egui::Area::new(egui::Id::new("link_banner")).anchor(egui::Align2::CENTER_TOP, [0.0, 34.0]).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::YELLOW, format!("Drag the other end of the transition on '{}' (it leads back to '{}').", d.target, d.from_map));
                            if ui.button("Cancel").clicked() {
                                self.link = None;
                            }
                        });
                    });
                });
            }
        }

        if self.quick.open {
            let mut open = true;
            let mut pick: Option<String> = None;
            egui::Window::new("Go to map").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, [0.0, 80.0]).show(ctx, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.quick.query).hint_text("type to filter, Enter opens").desired_width(260.0));
                r.request_focus();
                let q = self.quick.query.to_lowercase();
                let hits: Vec<String> = self.maps.iter().filter(|m| q.is_empty() || m.to_lowercase().contains(&q)).cloned().collect();
                for m in &hits {
                    let cur = self.doc.as_ref().is_some_and(|d| d.map.name == *m);
                    if ui.selectable_label(cur, m).clicked() {
                        pick = Some(m.clone());
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    pick = hits.first().cloned();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.quick.open = false;
                }
            });
            if !open {
                self.quick.open = false;
            }
            if let Some(m) = pick {
                self.quick.open = false;
                self.goto_map(&m, None);
            }
        }
    }

    /// The map dropdown in the top bar.
    pub fn map_dropdown(&mut self, ui: &mut egui::Ui) {
        let cur = self.doc.as_ref().map_or("(no map)".to_string(), |d| format!("{}{}", d.map.name, if d.is_dirty() { " •" } else { "" }));
        let cur_name = self.doc.as_ref().map(|d| d.map.name.clone());
        let mut go = None;
        let mut new = false;
        egui::ComboBox::from_id_salt("map_switch").selected_text(egui::RichText::new(cur).strong()).show_ui(ui, |ui| {
            for m in &self.maps {
                if ui.selectable_label(cur_name.as_deref() == Some(m.as_str()), m).clicked() {
                    go = Some(m.clone());
                }
            }
            ui.separator();
            if ui.button("+ New map…").clicked() {
                new = true;
            }
        })
        .response
        .on_hover_text("Switch map (Ctrl+M: search · Ctrl+PgUp / PgDn: previous / next)");
        if let Some(m) = go.filter(|m| Some(m) != cur_name.as_ref()) {
            self.goto_map(&m, None);
        }
        if new {
            self.request(Pending::New);
        }
    }
}
