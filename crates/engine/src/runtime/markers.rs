//! Quest markers: where the current stage of each active quest leads (`QuestDef::targets`: a
//! placed character, prop or zone by name on a map). On screen a bobbing marker over the target;
//! off screen an arrow at the screen's edge pointing to it; a target on another map is shown at
//! the exit leading there (a portal, a transition or a map edge). The world map pins the same
//! spots. Players switch them off in *Options › Quest markers*.

use macroquad::prelude::*;

use super::Game;
use crate::entity::EntityKind;
use crate::map::ZoneKind;
use crate::quest::QuestStatus;

/// A spot a quest leads to on the current map.
#[derive(Clone, Debug)]
pub(super) struct Marker {
    /// The quest's id and title.
    pub quest: String,
    pub title: String,
    /// World pixels: the point the marker hangs over.
    pub pos: Vec2,
    /// The target's map, when the marker stands at the exit leading there.
    pub via: Option<String>,
}

const GOLD_MARK: Color = Color::new(1.0, 0.82, 0.25, 1.0);

/// A diamond with a dark rim, centred on `c`.
fn diamond(c: Vec2, r: f32, fill: Color) {
    let rim = Color::new(0.15, 0.1, 0.05, fill.a);
    let p = |k: f32| [vec2(c.x, c.y - k), vec2(c.x + k * 0.7, c.y), vec2(c.x, c.y + k), vec2(c.x - k * 0.7, c.y)];
    let (o, i) = (p(r + 2.0), p(r));
    draw_triangle(o[0], o[1], o[2], rim);
    draw_triangle(o[0], o[2], o[3], rim);
    draw_triangle(i[0], i[1], i[2], fill);
    draw_triangle(i[0], i[2], i[3], fill);
}

impl Game {
    /// Where the active quests' current stages lead on this map, as markers.
    pub(super) fn quest_markers(&self) -> Vec<Marker> {
        let m = &self.loaded.map;
        let ts = self.ts();
        let mut out = Vec::new();
        let mut quests: Vec<(&String, &crate::quest::QuestProgress)> = self.world.quests.iter().filter(|(_, p)| p.status == QuestStatus::Active).collect();
        quests.sort_by_key(|q| std::cmp::Reverse(q.1.order));
        for (id, p) in quests {
            let Some(def) = self.project.db.quests.iter().find(|q| q.id == *id) else { continue };
            let Some(t) = def.target_of(p.stage) else { continue };
            let title = self.world.expand_text(&def.title);
            let (map, name) = (t.map.trim(), t.name.trim());
            if map == m.name {
                // The target here: a placed entity (over its head), else a zone (its middle).
                let entity = self.loaded.entities.iter().find(|e| e.active && e.name == name).map(|e| {
                    // Over a character's head (its model's standing height), a prop's top.
                    let h = if e.kind == EntityKind::Character {
                        let look = self.look(&e.appearance);
                        self.project.db.model(&look.model).map_or(ts * 1.4, |m| m.height) * self.look_size(&e.appearance)
                    } else {
                        ts
                    };
                    e.body.pos - vec2(0.0, h + ts * 0.25)
                });
                let zone = || m.zones.iter().find(|z| z.name == name).map(|z| z.pixel_rect(ts).center());
                if let Some(pos) = entity.or_else(zone) {
                    out.push(Marker { quest: id.clone(), title, pos, via: None });
                }
            } else {
                // Elsewhere: the nearest exit leading to its map.
                let me = self.player.pos;
                let exits = m.zones.iter().filter_map(|z| match &z.kind {
                    ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. } if target_map == map => Some(z.pixel_rect(ts).center()),
                    _ => None,
                });
                let edges = crate::map::Edge::ALL.into_iter().filter_map(|e| {
                    let link = m.edges.get(e)?;
                    (link.target_map == map).then(|| m.edges.strip_rect(e, m.width, m.height, ts)).flatten().map(|r| r.center())
                });
                if let Some(pos) = exits.chain(edges).min_by(|a, b| a.distance(me).total_cmp(&b.distance(me))) {
                    out.push(Marker { quest: id.clone(), title, pos, via: Some(map.to_string()) });
                }
            }
        }
        out
    }

    /// The markers in the world (screen space, over the world; `cam` = the world's camera):
    /// bobbing over targets on screen, arrows at the screen's edge for the others.
    pub(super) fn draw_quest_markers(&self, cam: &Camera2D) {
        if !self.settings.quest_markers || self.on_title() || self.cutscene_active() {
            return;
        }
        let markers = self.quest_markers();
        if markers.is_empty() {
            return;
        }
        let (sw, sh) = (screen_width(), screen_height());
        let margin = 26.0;
        let t = get_time() as f32;
        for (k, mk) in markers.iter().enumerate() {
            let p = cam.world_to_screen(mk.pos);
            if p.x > margin && p.x < sw - margin && p.y > margin && p.y < sh - margin {
                let bob = (t * 3.0 + k as f32).sin() * 3.0;
                diamond(vec2(p.x, p.y - 10.0 + bob), 9.0, GOLD_MARK);
            } else {
                // Off screen: an arrow at the edge, pointing from the screen's middle to it.
                let c = vec2(sw / 2.0, sh / 2.0);
                let dir = (p - c).normalize_or_zero();
                if dir == Vec2::ZERO {
                    continue;
                }
                let s = ((sw / 2.0 - margin) / dir.x.abs().max(1e-3)).min((sh / 2.0 - margin) / dir.y.abs().max(1e-3));
                let tip = c + dir * s;
                let n = vec2(-dir.y, dir.x);
                let rim = Color::new(0.15, 0.1, 0.05, 1.0);
                draw_triangle(tip + dir * 3.0, tip - dir * 20.0 + n * 12.0, tip - dir * 20.0 - n * 12.0, rim);
                draw_triangle(tip, tip - dir * 17.0 + n * 9.0, tip - dir * 17.0 - n * 9.0, GOLD_MARK);
                diamond(tip - dir * 30.0, 6.0, GOLD_MARK);
            }
        }
    }

    /// The pins on the world map (`to_screen`: map tiles to screen pixels; `view`: the map's
    /// visible rectangle).
    pub(super) fn draw_quest_pins(&self, view: Rect, to_screen: impl Fn(Vec2) -> Vec2) {
        if !self.settings.quest_markers {
            return;
        }
        let ts = self.ts();
        for mk in self.quest_markers() {
            let p = to_screen(mk.pos / ts);
            if !view.contains(p) {
                continue;
            }
            diamond(p - vec2(0.0, 8.0), 8.0, GOLD_MARK);
            // The quest's title above the pin (exit labels sit below their exits).
            let label: String = mk.title.chars().take(28).collect();
            let w = super::text::measure_text(&label, None, 16, 1.0).width;
            draw_rectangle(p.x - w / 2.0 - 4.0, p.y - 40.0, w + 8.0, 18.0, Color::new(0.0, 0.0, 0.0, 0.6));
            super::text::draw_text(&label, p.x - w / 2.0, p.y - 27.0, 16.0, GOLD_MARK);
        }
    }

    /// `markers=` for `status`: quest@x,y (tiles) or quest>map@x,y (at the exit to that map).
    pub(super) fn markers_status(&self) -> Vec<String> {
        if !self.settings.quest_markers {
            return Vec::new();
        }
        let ts = self.ts();
        self.quest_markers()
            .into_iter()
            .map(|m| {
                let (x, y) = ((m.pos.x / ts) as i32, (m.pos.y / ts) as i32);
                match m.via {
                    Some(map) => format!("{}>{map}@{x},{y}", m.quest),
                    None => format!("{}@{x},{y}", m.quest),
                }
            })
            .collect()
    }
}
