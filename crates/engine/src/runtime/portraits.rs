//! Dialog portraits: the speaker's head and shoulders at the left of the dialog box, a close-up
//! render of their model (in their colours). Nothing to set up per character: the speaker is
//! found from the dialog's speaker text (`ProjectConfig::dialog_portraits` turns it off).

use macroquad::prelude::*;

use super::Game;
use crate::defs::{Appearance, SpriteRef};
use crate::entity::EntityKind;

/// Whose portrait a dialog shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Speaker {
    Player,
    /// A character on the map (entity uid).
    Entity(u32),
    /// A character definition (a speaker who isn't on this map).
    Def(String),
}

/// Does a character's name fit a speaker's? Letter case is ignored, and a name fits when all
/// words of the shorter one appear in the longer one ("Elder" speaks as "Elder Rowan", "guard"
/// as "Town Guard").
fn names_match(a: &str, b: &str) -> bool {
    let words = |s: &str| s.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>();
    let (a, b) = (words(a), words(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.iter().all(|w| long.contains(w))
}

impl Game {
    /// Finds who speaks: `raw` is the dialog's speaker as written ("{self}", "{player}", a
    /// name), `shown` the expanded text, `owner` the trigger's owner. In order: none for
    /// narration; the entity a placeholder names; the player; the trigger's owner if the name
    /// is its own; a character on this map with that name (entity or definition name, see
    /// `names_match`; the nearest one); a character definition with that name.
    pub(super) fn find_speaker(&self, raw: &str, shown: &str, owner: Option<u32>) -> Option<Speaker> {
        let name = shown.trim();
        if !self.project.config.dialog_portraits || name.is_empty() {
            return None;
        }
        let is_char = |uid: u32| self.loaded.entities.iter().any(|e| e.uid == uid && e.kind == EntityKind::Character);
        if raw.contains("{self}") {
            return owner.filter(|u| is_char(*u)).map(Speaker::Entity);
        }
        if let Some(i) = raw.find("{uid:") {
            let uid = raw[i + 5..].split('}').next().and_then(|n| n.parse::<u32>().ok());
            return uid.filter(|u| is_char(*u)).map(Speaker::Entity);
        }
        if raw.trim() == "{player}" || name.eq_ignore_ascii_case(self.world.player_name.trim()) {
            return Some(Speaker::Player);
        }
        let db = &self.project.db;
        let named = |e: &super::RtEntity| e.kind == EntityKind::Character && (names_match(&e.name, name) || db.character(&e.def).is_some_and(|d| names_match(&d.name, name)));
        if let Some(uid) = owner.filter(|u| self.loaded.entities.iter().any(|e| e.uid == *u && named(e))) {
            return Some(Speaker::Entity(uid));
        }
        let near = self
            .loaded
            .entities
            .iter()
            .filter(|e| e.active && named(e))
            .min_by(|a, b| a.body.pos.distance(self.player.pos).total_cmp(&b.body.pos.distance(self.player.pos)));
        if let Some(e) = near {
            return Some(Speaker::Entity(e.uid));
        }
        db.characters.iter().find(|d| names_match(&d.name, name) && d.appearance.has_look()).map(|d| Speaker::Def(d.id.clone()))
    }

    /// Shows the portrait of whoever `raw` / `shown` names in the dialog just opened.
    pub(super) fn set_dialog_portrait(&mut self, raw: &str, shown: &str, owner: Option<u32>) {
        self.portrait = self.find_speaker(raw, shown, owner);
        if self.portrait.is_some() {
            self.ui.with_portrait();
        }
    }

    /// The definition id behind a speaker (moods and drawn portraits belong to definitions).
    fn speaker_def(&self, s: &Speaker) -> Option<String> {
        match s {
            Speaker::Player => Some(self.project.config.player.clone()).filter(|p| !p.is_empty()),
            Speaker::Entity(uid) => self.loaded.entities.iter().find(|e| e.uid == *uid).map(|e| e.def.clone()),
            Speaker::Def(id) => Some(id.clone()),
        }
    }

    /// The drawn portrait a speaker shows now, if it has one: its mood's, the placed
    /// character's own, its definition's (the player: the project's player portrait first).
    fn portrait_image(&self, s: &Speaker) -> Option<SpriteRef> {
        let def = self.speaker_def(s).and_then(|id| self.project.db.character(&id));
        if let Some(d) = def {
            if let Some(m) = self.moods.get(&d.id).and_then(|m| d.moods.get(m)).filter(|m| !m.is_empty()) {
                return Some(m.clone());
            }
        }
        match s {
            Speaker::Player if !self.project.config.player_portrait.is_empty() => return Some(self.project.config.player_portrait.clone()),
            Speaker::Entity(uid) => {
                if let Some(p) = self.loaded.entities.iter().find(|e| e.uid == *uid).and_then(|e| e.portrait.clone()).filter(|p| !p.is_empty()) {
                    return Some(p);
                }
            }
            _ => {}
        }
        def.map(|d| d.portrait.clone()).filter(|p| !p.is_empty())
    }

    /// A character's portrait for the HUD rows: its drawn one, else its head and shoulders.
    pub(super) fn draw_entity_portrait(&self, uid: u32, r: Rect) {
        let s = Speaker::Entity(uid);
        if let Some(p) = self.portrait_image(&s) {
            if self.assets.draw_sprite_fit(&p, r) {
                return;
            }
        }
        if let Some(a) = self.speaker_look(&s) {
            self.draw_bust(&a, r);
        }
    }

    /// `Action::Mood`: the mood of a character on the map (or a definition by id / name).
    pub(super) fn set_mood(&mut self, target: &str, mood: &str, owner: Option<u32>) {
        // A dialog speaker as written: "{self}", "{uid:N}", "{player}" or a name.
        let uid = match target.trim() {
            "{self}" => owner,
            t if t.starts_with("{uid:") => t[5..].trim_end_matches('}').parse().ok(),
            _ => None,
        };
        let target = if target.trim() == "{player}" { self.project.config.player.clone() } else { target.to_string() };
        let def = uid
            .and_then(|u| self.loaded.entities.iter().find(|e| e.uid == u).map(|e| e.def.clone()))
            .or_else(|| self.entity_index(&target, owner).map(|i| self.loaded.entities[i].def.clone()))
            .or_else(|| {
                let db = &self.project.db;
                db.character(&target).or_else(|| db.characters.iter().find(|d| names_match(&d.name, &target))).map(|d| d.id.clone())
            });
        let Some(def) = def else {
            eprintln!("[events] Set mood: no character '{target}'");
            return;
        };
        if mood.trim().is_empty() {
            self.moods.remove(&def);
        } else {
            self.moods.insert(def, mood.trim().to_string());
        }
    }

    /// The look a portrait shows.
    fn speaker_look(&self, s: &Speaker) -> Option<Appearance> {
        match s {
            Speaker::Player => self.player_appearance(),
            Speaker::Entity(uid) => {
                let e = self.loaded.entities.iter().find(|e| e.uid == *uid)?;
                e.appearance.has_look().then(|| e.appearance.clone())
            }
            Speaker::Def(id) => self.project.db.character(id).map(|d| d.appearance.clone()),
        }
    }

    /// Head and shoulders of a character, scaled into `r` (bottom-aligned, centered).
    pub(super) fn draw_bust(&self, a: &Appearance, r: Rect) {
        // A close-up render, square, scaled into the box.
        if let Some(cell) = self.model_bust(a) {
            let side = r.w.min(r.h);
            self.assets.models.draw_in(&cell, Rect::new(r.x + (r.w - side) / 2.0, r.bottom() - side, side, side), WHITE);
        }
    }

    /// Draws the open dialog's portrait (after the dialog box).
    pub(super) fn draw_dialog_portrait(&self) {
        let (Some(r), Some(s)) = (self.ui.portrait_rect, &self.portrait) else { return };
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.45));
        let drawn = self.portrait_image(s).is_some_and(|p| self.assets.draw_sprite_fit(&p, Rect::new(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0)));
        if let Some(a) = self.speaker_look(s).filter(|_| !drawn) {
            // Head and shoulders, scaled to the square.
            self.draw_bust(&a, Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 4.0));
        }
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.85, 0.72, 0.38, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_names() {
        assert!(names_match("Elder", "Elder Rowan"));
        assert!(names_match("guard", "Town Guard"));
        assert!(names_match("Tilda", "tilda"));
        assert!(!names_match("Tilda", "Tomas"));
        assert!(!names_match("", "Sign"));
        assert!(!names_match("Old Man", "Old Woman"));
    }
}
