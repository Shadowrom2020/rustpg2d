//! The World view: every map as a card with its picture, the ways between them (portals,
//! transitions, edges) as arrows, and what is wrong with them said on the spot: a map nothing
//! leads to, a way that leads nowhere. A click opens a map; cards can be dragged about, and
//! lie where they were left the next time (noted with the creator's folders and tags).

use std::collections::HashMap;

use eframe::egui;
use editor_core::world::{Way, WorldMap};
use editor_core::Workspace;

/// Points of a card per tile: a card's longer side is about 220 points, whatever the map's
/// size (a room isn't a dot beside the wilderness).
fn scale_of(m: &WorldMap) -> f32 {
    (220.0 / m.width.max(m.height).max(1) as f32).min(7.0)
}

#[derive(Default)]
pub struct WorldView {
    maps: Vec<WorldMap>,
    textures: HashMap<String, egui::TextureHandle>,
    /// Where each card's picture has its top-left corner (the view's own points).
    pos: HashMap<String, egui::Pos2>,
    /// What the survey was made of: made again when it changes.
    key: String,
    /// The view's shift and zoom.
    pan: egui::Vec2,
    zoom: f32,
}

fn way_color(way: Way) -> egui::Color32 {
    match way {
        Way::Portal => egui::Color32::from_rgb(200, 120, 255),
        Way::Transition => egui::Color32::from_rgb(255, 160, 60),
        Way::Edge => egui::Color32::from_rgb(110, 220, 130),
    }
}

impl WorldView {
    /// `maps=N unreached=[…] problems=N` for the script's `status`.
    pub fn status(&self) -> String {
        let unreached: Vec<&str> = self.maps.iter().filter(|m| !m.reached).map(|m| m.name.as_str()).collect();
        let (links, problems) = self.maps.iter().flat_map(|m| &m.links).fold((0, 0), |(l, p), link| (l + 1, p + usize::from(!link.problem.is_empty())));
        format!("maps={} links={links} unreached=[{}] problems={problems}", self.maps.len(), unreached.join(","))
    }

    /// Looks at the project again when it changed since the last look.
    fn refresh(&mut self, ctx: &egui::Context, ws: &Workspace) {
        let key = format!("{}|{}|{}", ws.maps.join(","), ws.timeline.status(), ws.is_dirty());
        if key == self.key && !self.maps.is_empty() {
            return;
        }
        self.key = key;
        self.maps = ws.survey_world();
        self.textures.clear();
        for m in &self.maps {
            if m.picture.len() == (m.width * m.height * 4) as usize {
                let image = egui::ColorImage::from_rgba_unmultiplied([m.width as usize, m.height as usize], &m.picture);
                self.textures.insert(m.name.clone(), ctx.load_texture(format!("world {}", m.name), image, egui::TextureOptions::NEAREST));
            }
        }
        // Cards that have no place yet: a column for each step from the start map, the maps
        // nothing leads to last. Where the creator left a card, it lies.
        self.pos.retain(|name, _| self.maps.iter().any(|m| m.name == *name));
        for (name, meta) in ws.meta.of_kind(rustpg2d_engine::refactor::RefKind::Map) {
            if let Some([x, y]) = meta.card {
                self.pos.insert(name.to_string(), egui::pos2(x, y));
            }
        }
        let mut columns: std::collections::BTreeMap<u32, Vec<&WorldMap>> = std::collections::BTreeMap::new();
        for m in &self.maps {
            columns.entry(m.distance.unwrap_or(u32::MAX)).or_default().push(m);
        }
        let mut x = 30.0;
        for (_, column) in columns {
            let mut y = 40.0;
            let mut widest: f32 = 0.0;
            for m in column {
                let size = egui::vec2(m.width as f32, m.height as f32) * scale_of(m);
                self.pos.entry(m.name.clone()).or_insert(egui::pos2(x, y));
                y += size.y + 56.0;
                widest = widest.max(size.x);
            }
            x += widest.max(120.0) + 110.0;
        }
    }

    /// The tab. Returns the map a click asked to open.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) -> Option<String> {
        if self.zoom == 0.0 {
            self.zoom = 1.0;
        }
        self.refresh(ui.ctx(), ws);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("The world").strong());
            let unreached = self.maps.iter().filter(|m| !m.reached).count();
            let problems = self.maps.iter().flat_map(|m| &m.links).filter(|l| !l.problem.is_empty()).count();
            ui.weak(format!("{} maps", self.maps.len()));
            if unreached > 0 {
                ui.colored_label(ui.visuals().warn_fg_color, format!("{unreached} nothing leads to"));
            }
            if problems > 0 {
                ui.colored_label(ui.visuals().error_fg_color, format!("{problems} ways that lead nowhere"));
            }
            ui.separator();
            for (way, label) in [(Way::Portal, "portal"), (Way::Transition, "transition"), (Way::Edge, "edge")] {
                ui.colored_label(way_color(way), label);
            }
            ui.separator();
            if ui.button("Tidy").on_hover_text("Puts every card back in its column: by how many maps lie between it and the start").clicked() {
                for name in self.pos.keys().cloned().collect::<Vec<_>>() {
                    ws.set_card_place(&name, None);
                }
                self.pos.clear();
                self.key.clear();
            }
            ui.weak("Click a map to open it. Drag a card to move it, the background to move the view; the wheel zooms.");
        });
        // A world of one map: how it becomes more.
        if self.maps.len() <= 1 {
            ui.horizontal_wrapped(|ui| {
                ui.weak("This game is one map so far. Make another (+ New map in the Project browser, or Ctrl+N), then draw a transition zone at an edge of one (the Zones tool): the editor asks where on the other map it leads, and the way shows here as a line.");
            });
        }
        ui.separator();
        let rect = ui.available_rect_before_wrap();
        let back = ui.interact(rect, egui::Id::unique("world view"), egui::Sense::drag());
        if back.dragged() {
            self.pan += back.drag_delta();
        }
        if back.contains_pointer() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.zoom = (self.zoom * 1.1f32.powf((scroll / 50.0).clamp(-3.0, 3.0))).clamp(0.25, 4.0);
            }
        }
        let painter = ui.painter_at(rect);
        let (pan, zoom) = (self.pan, self.zoom);
        let at = |p: egui::Pos2| rect.min + pan + p.to_vec2() * zoom;
        let start = ws.project.config.start_map.clone();
        let start = &start;
        let mut open = None;
        let mut dropped: Option<String> = None;
        let font = egui::FontId::proportional(13.0);

        // The cards.
        let mut moved: Option<(String, egui::Vec2)> = None;
        for m in &self.maps {
            let Some(pos) = self.pos.get(&m.name).copied() else { continue };
            let size = egui::vec2(m.width as f32, m.height as f32) * scale_of(m) * zoom;
            let card = egui::Rect::from_min_size(at(pos), size);
            let r = ui.interact(card.expand(3.0), egui::Id::unique(("world card", &m.name)), egui::Sense::click_and_drag());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Map {}", m.name)));
            if r.clicked() {
                open = Some(m.name.clone());
            }
            if r.dragged() {
                moved = Some((m.name.clone(), r.drag_delta() / zoom));
            }
            if r.drag_stopped() {
                dropped = Some(m.name.clone());
            }
            let problems: Vec<String> = m.links.iter().filter(|l| !l.problem.is_empty()).map(|l| format!("{}: {}", if l.zone.is_empty() { "An edge" } else { &l.zone }, l.problem)).collect();
            let frame = if !problems.is_empty() {
                ui.visuals().error_fg_color
            } else if !m.reached {
                ui.visuals().warn_fg_color
            } else if r.hovered() {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_gray(110)
            };
            painter.rect_filled(card.expand(3.0), 3.0, egui::Color32::from_gray(24));
            if let Some(t) = self.textures.get(&m.name) {
                painter.image(t.id(), card, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
            painter.rect_stroke(card.expand(3.0), 3.0, egui::Stroke::new(if r.hovered() { 2.0 } else { 1.0 }, frame), egui::StrokeKind::Outside);
            let mut title = m.name.clone();
            if m.name == *start {
                title += "  (the game starts here)";
            }
            painter.text(card.left_top() - egui::vec2(2.0, 6.0), egui::Align2::LEFT_BOTTOM, title, font.clone(), egui::Color32::from_gray(230));
            let mut note = format!("{} x {}", m.width, m.height);
            if !m.reached {
                note += " · nothing leads here";
            }
            painter.text(card.left_bottom() + egui::vec2(-2.0, 6.0), egui::Align2::LEFT_TOP, note, egui::FontId::proportional(11.0), if m.reached { egui::Color32::from_gray(150) } else { ui.visuals().warn_fg_color });
            if !problems.is_empty() {
                r.on_hover_text(problems.join("\n"));
            } else if !m.reached {
                r.on_hover_text("No portal, transition or edge of another map leads here.");
            }
        }
        if let Some((name, d)) = moved {
            if let Some(p) = self.pos.get_mut(&name) {
                *p += d;
            }
        }
        // A card let go lies there from now on.
        if let Some((name, p)) = dropped.and_then(|n| self.pos.get(&n).map(|p| (n, *p))) {
            ws.set_card_place(&name, Some([p.x.round(), p.y.round()]));
        }

        // The ways: from where each starts on its map to where it arrives on the other.
        for m in &self.maps {
            let Some(from_card) = self.pos.get(&m.name).copied() else { continue };
            for l in &m.links {
                let a = at(from_card + egui::vec2(l.from.0, l.from.1) * scale_of(m));
                let color = if l.problem.is_empty() { way_color(l.way) } else { ui.visuals().error_fg_color };
                let b = match (self.pos.get(&l.to_map), self.maps.iter().find(|x| x.name == l.to_map)) {
                    (Some(to_card), Some(other)) => {
                        let (tx, ty) = l.to.unwrap_or((other.width as f32 / 2.0, other.height as f32 / 2.0));
                        at(*to_card + egui::vec2(tx, ty) * scale_of(other))
                    }
                    // A way that leads nowhere: a stub with a cross.
                    _ => {
                        let b = a + egui::vec2(26.0, -18.0);
                        painter.line_segment([b + egui::vec2(-4.0, -4.0), b + egui::vec2(4.0, 4.0)], egui::Stroke::new(2.0, color));
                        painter.line_segment([b + egui::vec2(-4.0, 4.0), b + egui::vec2(4.0, -4.0)], egui::Stroke::new(2.0, color));
                        b
                    }
                };
                painter.line_segment([a, b], egui::Stroke::new(1.5, color.gamma_multiply(0.85)));
                painter.circle_filled(a, 3.0, color);
                // The head, a little before the end.
                let dir = (b - a).normalized();
                if dir.is_finite() && (b - a).length() > 24.0 {
                    let (tip, n) = (b - dir * 4.0, egui::vec2(-dir.y, dir.x));
                    painter.add(egui::Shape::convex_polygon(vec![tip, tip - dir * 9.0 + n * 4.5, tip - dir * 9.0 - n * 4.5], color, egui::Stroke::NONE));
                }
            }
        }
        open
    }
}
