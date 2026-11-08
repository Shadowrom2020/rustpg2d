//! Healing companions: a *Heal* spell or a healing item (one whose *on use* heals) asks whom to
//! heal while companions are on the map: a short pause with a "Heal whom?" bar listing the
//! player and the companions (Left / Right and E, a click on a character or its HUD row; Esc
//! cancels and nothing is spent). Also the companions' rows under the player's HUD panel and
//! healing characters by name (`Action::HealCharacter`, `heal_npc`).

use macroquad::prelude::*;

use super::input::{self, Act};
use super::{Game, RtEntity};
use crate::defs::UseTarget;
use crate::events::runner::ActionCtx;
use super::Source;
use crate::events::{Action, GameEvent};
use crate::skills::SpellEffect;
use super::text::{draw_text, measure_text};

/// What the chosen character is healed with.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum HealSource {
    Item(String),
    Spell(String),
}

/// The open "Heal whom?" choice.
#[derive(Clone, Debug)]
pub(super) struct HealPick {
    source: HealSource,
    /// The candidates: `None` = the player, else a companion's entity uid.
    choices: Vec<Option<u32>>,
    sel: usize,
    /// Mouse position when last looked at: pointing at a candidate selects it once it moves.
    mouse: Vec2,
}

/// Where the "heal whom?" bar's parts are (screen pixels).
struct BarLayout {
    panel: Rect,
    title: String,
    names: Vec<String>,
    buttons: Vec<Rect>,
}

const HINT: &str = "Click or Left / Right + E: heal   Esc: cancel";

/// Size of a companion's HUD row (screen pixels).
const ROW_W: f32 = 176.0;
const ROW_H: f32 = 34.0;

impl Game {
    /// Companions on this map that can be healed (entity indices).
    pub(super) fn healable_companions(&self) -> Vec<usize> {
        (0..self.loaded.entities.len()).filter(|&i| self.loaded.entities[i].companion.is_some() && self.loaded.entities[i].active && self.loaded.entities[i].max_hp > 0).collect()
    }

    /// Whether an item heals when used (a *Heal player* among its *on use* actions).
    fn heals(&self, item: &str) -> bool {
        fn any_heal(actions: &[Action]) -> bool {
            actions.iter().any(|a| match a {
                Action::Heal { amount } => *amount > 0,
                Action::If { then, otherwise, .. } => any_heal(then) || any_heal(otherwise),
                _ => false,
            })
        }
        let ev = GameEvent::UseItem { item: item.to_string() };
        self.project.db.item(item).is_some_and(|d| d.on_use.iter().any(|t| t.on.matches(&ev) && any_heal(&t.actions)))
    }

    /// Starts the target choice for a healing item or a Heal spell when there is someone besides
    /// the player to heal. Returns true when the use is taken care of (the choice is open, or the
    /// item went straight to its only possible target / can't be used); false = heal the player
    /// at once, as without companions.
    pub(super) fn start_heal_pick(&mut self, source: HealSource) -> bool {
        let use_on = match &source {
            HealSource::Item(id) => {
                if !self.heals(id) {
                    return false;
                }
                self.project.db.item(id).map_or(UseTarget::Player, |d| d.use_on)
            }
            HealSource::Spell(_) => UseTarget::Choose,
        };
        if use_on == UseTarget::Player {
            return false;
        }
        let mut choices: Vec<Option<u32>> = self.healable_companions().into_iter().map(|i| Some(self.loaded.entities[i].uid)).collect();
        if use_on == UseTarget::Choose {
            choices.insert(0, None);
        }
        match choices.len() {
            0 => {
                let name = match &source {
                    HealSource::Item(id) => self.project.db.item(id).map_or(id.clone(), |d| d.name.clone()),
                    HealSource::Spell(id) => id.clone(),
                };
                self.ui.toast(format!("{name} is for companions - none is here."));
                true
            }
            // Only the player (no companion here): as before.
            1 if choices[0].is_none() => false,
            1 => {
                self.apply_heal(&source, choices[0]);
                true
            }
            _ => {
                // The most hurt one first (by share of HP), so E heals whoever needs it most.
                let share = |c: &Option<u32>| match c {
                    None => self.hp() as f32 / self.player_max_hp().max(1) as f32,
                    Some(uid) => self.loaded.entities.iter().find(|e| e.uid == *uid).map_or(1.0, |e| e.hp as f32 / e.max_hp.max(1) as f32),
                };
                let sel = (0..choices.len()).min_by(|a, b| share(&choices[*a]).total_cmp(&share(&choices[*b]))).unwrap_or(0);
                self.heal_pick = Some(HealPick { source, choices, sel, mouse: input::mouse_pos() });
                true
            }
        }
    }

    pub(super) fn heal_picking(&self) -> bool {
        self.heal_pick.is_some()
    }

    /// The choice's input (it pauses the world). Returns true while it is open.
    pub(super) fn update_heal_pick(&mut self) -> bool {
        let Some(pick) = &self.heal_pick else { return false };
        let n = pick.choices.len();
        let mut sel = pick.sel;
        if input::act_pressed(Act::Cancel) || input::mouse_pressed(MouseButton::Right) {
            self.heal_pick = None;
            self.ui.toast("Cancelled".into());
            return true;
        }
        if input::act_pressed(Act::Left) || input::act_pressed(Act::Up) {
            sel = (sel + n - 1) % n;
        }
        if input::act_pressed(Act::Right) || input::act_pressed(Act::Down) {
            sel = (sel + 1) % n;
        }
        let mut chosen = input::act_pressed(Act::Confirm) || input::act_pressed(Act::Attack);
        // The mouse: pointing at a candidate selects it, a click heals it at once.
        let (m, moved) = (input::mouse_pos(), input::mouse_pos() != pick.mouse);
        let under = self.heal_choice_at(m);
        if let Some(k) = under.filter(|_| moved || input::mouse_pressed(MouseButton::Left)) {
            sel = k;
            chosen |= input::mouse_pressed(MouseButton::Left);
        }
        let pick = self.heal_pick.as_mut().expect("checked above");
        pick.sel = sel;
        pick.mouse = m;
        if chosen {
            let (source, target) = (pick.source.clone(), pick.choices[sel]);
            self.heal_pick = None;
            self.apply_heal(&source, target);
        }
        true
    }

    /// The choice under a screen point: a name in the bar, a HUD row, a companion or the player
    /// on the map.
    fn heal_choice_at(&self, p: Vec2) -> Option<usize> {
        let pick = self.heal_pick.as_ref()?;
        let bar = self.heal_bar(pick);
        if let Some(k) = bar.buttons.iter().position(|r| r.contains(p)) {
            return Some(k);
        }
        if bar.panel.contains(p) {
            return None;
        }
        for (k, r) in self.companion_rows() {
            if r.contains(p) {
                let uid = self.loaded.entities[k].uid;
                return pick.choices.iter().position(|c| *c == Some(uid));
            }
        }
        let w = self.mouse_world_at(p);
        if let Some(i) = self.entity_under(w) {
            let uid = self.loaded.entities[i].uid;
            if let Some(k) = pick.choices.iter().position(|c| *c == Some(uid)) {
                return Some(k);
            }
        }
        let ts = self.ts();
        let me = Rect::new(self.player.pos.x - ts / 2.0, self.player.pos.y - ts * 1.5, ts, ts * 1.5);
        me.contains(w).then(|| pick.choices.iter().position(Option::is_none)).flatten()
    }

    /// Heals `target` (None = the player) with the item or spell.
    fn apply_heal(&mut self, source: &HealSource, target: Option<u32>) {
        match source {
            HealSource::Spell(id) => {
                let Some(d) = self.project.db.spell(id).cloned() else { return };
                let SpellEffect::Heal { amount } = d.effect else { return };
                if self.world.mana < d.mana as f32 {
                    self.ui.toast("Not enough mana".into());
                    return;
                }
                self.pay_spell(&d);
                match target.and_then(|uid| self.uid_index(uid)) {
                    Some(i) => self.heal_entity(i, amount),
                    None => self.heal(amount),
                }
                self.ui.toast(d.name.to_string());
            }
            HealSource::Item(item) => match target.and_then(|uid| self.uid_index(uid)) {
                None => self.use_item_now(item),
                Some(i) => self.use_item_on(item, i),
            },
        }
    }

    /// Uses a healing item on character `i`: its *on use* actions run with the character as
    /// "self", and *Heal player* heals it instead.
    fn use_item_on(&mut self, item: &str, i: usize) {
        let uid = self.loaded.entities[i].uid;
        let actions = self.collect(&GameEvent::UseItem { item: item.to_string() }, &Source::Item(item.to_string()));
        if actions.is_empty() {
            self.ui.toast("Nothing happens.".into());
            return;
        }
        for (a, _) in actions {
            let a = redirect_heals(a);
            self.runner.push(&[a], &ActionCtx { owner: Some(uid) });
        }
        if self.project.db.item(item).is_some_and(|d| d.consumable) {
            self.world.take_item(item, 1);
        }
    }

    /// Heals character `i` by `amount` (negative hurts it); a companion's HP is saved.
    pub(super) fn heal_entity(&mut self, i: usize, amount: i32) {
        let e = &self.loaded.entities[i];
        if e.max_hp <= 0 {
            return;
        }
        if amount < 0 {
            let from = e.body.pos;
            self.damage_entity(i, -amount, from, 0.0);
            return;
        }
        let name = self.entity_display_name(e.uid).unwrap_or_else(|| e.name.clone());
        let e = &mut self.loaded.entities[i];
        e.hp = (e.hp + amount).min(e.max_hp);
        self.ui.toast(format!("{name} +{amount} HP"));
        self.companion_healed(i);
    }

    /// `Action::HealCharacter`: `"player"` heals the player.
    pub(super) fn heal_character(&mut self, target: &str, amount: i32, owner: Option<u32>) {
        if target.eq_ignore_ascii_case("player") {
            self.heal(amount);
        } else if let Some(i) = self.entity_index(target, owner) {
            self.heal_entity(i, amount);
        } else {
            eprintln!("[events] Heal character: no character '{target}' here");
        }
    }

    /// The companions' HUD rows: (entity index, screen rect), under the player's panel.
    pub(super) fn companion_rows(&self) -> Vec<(usize, Rect)> {
        if self.ui_hidden || !self.combat.enabled() {
            return Vec::new();
        }
        let top = self.hud_bottom.get() + 6.0;
        self.healable_companions().into_iter().enumerate().map(|(k, i)| (i, Rect::new(10.0, top + k as f32 * (ROW_H + 4.0), ROW_W, ROW_H))).collect()
    }

    /// A click on a companion's HUD row opens its window (gear and bag). Returns true if so.
    pub(super) fn click_companion_row(&mut self) -> bool {
        if !input::mouse_pressed(MouseButton::Left) {
            return false;
        }
        let m = input::mouse_pos();
        let Some((i, _)) = self.companion_rows().into_iter().find(|(_, r)| r.contains(m)) else { return false };
        self.open_companion_window(i);
        true
    }

    /// One row per companion: head, name and an HP bar; highlighted while chosen to be healed.
    pub(super) fn draw_companion_rows(&self) {
        let chosen = self.heal_pick.as_ref().and_then(|p| p.choices[p.sel]);
        let gold = Color::new(0.85, 0.72, 0.38, 1.0);
        for (i, r) in self.companion_rows() {
            let e = &self.loaded.entities[i];
            let lit = chosen == Some(e.uid);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.05, 0.05, 0.1, 0.78));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, if lit { 2.5 } else { 1.5 }, if lit { Color::new(0.5, 1.0, 0.55, 1.0) } else { gold });
            let head = Rect::new(r.x + 3.0, r.y + 3.0, r.h - 6.0, r.h - 6.0);
            draw_rectangle(head.x, head.y, head.w, head.h, Color::new(0.0, 0.0, 0.0, 0.45));
            self.draw_entity_portrait(e.uid, head);
            let name = self.entity_display_name(e.uid).unwrap_or_else(|| e.name.clone());
            let tx = head.right() + 6.0;
            draw_text(&name, tx, r.y + 14.0, 16.0, Color::new(1.0, 0.86, 0.45, 1.0));
            // Its orders, when not the usual (attack, following).
            if let Some(l) = &e.companion {
                let mut tag = match l.order {
                    crate::state::Order::Attack => String::new(),
                    o => o.label().to_uppercase(),
                };
                if l.post.is_some() {
                    tag = if tag.is_empty() { "STAY".into() } else { format!("{tag} STAY") };
                }
                if !tag.is_empty() {
                    let nw = measure_text(&name, None, 16, 1.0).width;
                    draw_text(&tag, tx + nw + 6.0, r.y + 13.0, 12.0, Color::new(0.55, 0.8, 1.0, 1.0));
                }
            }
            let (bw, by) = (r.right() - tx - 6.0, r.y + 20.0);
            let share = (e.hp as f32 / e.max_hp.max(1) as f32).clamp(0.0, 1.0);
            draw_rectangle(tx - 1.0, by - 1.0, bw + 2.0, 9.0, Color::new(0.0, 0.0, 0.0, 0.7));
            let col = if share > 0.5 { Color::new(0.3, 0.8, 0.35, 1.0) } else if share > 0.25 { Color::new(0.9, 0.7, 0.2, 1.0) } else { Color::new(0.9, 0.2, 0.2, 1.0) };
            draw_rectangle(tx, by, bw * share, 7.0, col);
            // Ammunition of a bow the player gave it (red when out).
            if let Some((kind, n)) = self.companion_ammo(i) {
                let icon = self.project.db.item(&kind).map(|d| d.icon.clone());
                let ax = r.right() + 4.0;
                draw_rectangle(ax, r.y, 54.0, r.h, Color::new(0.05, 0.05, 0.1, 0.78));
                if let Some(s) = icon {
                    self.assets.draw_frame_in(&s.sheet, s.frame, Rect::new(ax + 2.0, r.y + 7.0, 20.0, 20.0), WHITE);
                }
                draw_text(format!("x{n}"), ax + 24.0, r.y + 22.0, 16.0, if n == 0 { Color::new(1.0, 0.35, 0.35, 1.0) } else { WHITE });
            }
            let (hp, col) = if self.is_down(i) { ("DOWN".to_string(), Color::new(1.0, 0.35, 0.35, 1.0)) } else { (format!("{}/{}", e.hp, e.max_hp), LIGHTGRAY) };
            let w = measure_text(&hp, None, 14, 1.0).width;
            draw_text(&hp, r.right() - w - 6.0, r.y + 14.0, 14.0, col);
        }
    }

    /// The bar's layout: a title, one button per candidate (name and HP), the hint; at the
    /// bottom of the screen (toasts use the top).
    fn heal_bar(&self, pick: &HealPick) -> BarLayout {
        let label = |c: &Option<u32>| match c {
            None => format!("{} {}/{}", self.world.player_name.trim(), self.hp(), self.player_max_hp()),
            Some(uid) => self.loaded.entities.iter().find(|e| e.uid == *uid).map_or("?".into(), |e| format!("{} {}/{}", self.entity_display_name(e.uid).unwrap_or_else(|| e.name.clone()), e.hp, e.max_hp)),
        };
        let what = match &pick.source {
            HealSource::Item(id) => self.project.db.item(id).map_or(id.clone(), |d| d.name.clone()),
            HealSource::Spell(id) => self.project.db.spell(id).map_or(id.clone(), |d| d.name.clone()),
        };
        let title = format!("{what}: heal whom?");
        let names: Vec<String> = pick.choices.iter().map(label).collect();
        let widths: Vec<f32> = names.iter().map(|n| measure_text(n, None, 20, 1.0).width + 24.0).collect();
        let row_w = widths.iter().sum::<f32>() + 8.0 * (names.len() as f32 - 1.0);
        let w = row_w.max(measure_text(HINT, None, 16, 1.0).width).max(measure_text(&title, None, 22, 1.0).width) + 32.0;
        let h = 96.0;
        let panel = Rect::new((screen_width() - w) / 2.0, screen_height() - h - 24.0, w, h);
        let mut cx = panel.x + (w - row_w) / 2.0;
        let buttons = widths
            .iter()
            .map(|bw| {
                let r = Rect::new(cx, panel.y + 36.0, *bw, 28.0);
                cx += bw + 8.0;
                r
            })
            .collect();
        BarLayout { panel, title, names, buttons }
    }

    /// The "Heal whom?" bar and a ring around the chosen character.
    pub(super) fn draw_heal_pick(&self) {
        let Some(pick) = &self.heal_pick else { return };
        let t = get_time() as f32;
        let BarLayout { panel: p, title, names, buttons } = self.heal_bar(pick);
        draw_rectangle(p.x, p.y, p.w, p.h, Color::new(0.05, 0.05, 0.1, 0.88));
        draw_rectangle_lines(p.x, p.y, p.w, p.h, 2.5, Color::new(0.85, 0.72, 0.38, 1.0));
        let tw = measure_text(&title, None, 22, 1.0).width;
        draw_text(&title, p.x + (p.w - tw) / 2.0, p.y + 26.0, 22.0, Color::new(1.0, 0.86, 0.45, 1.0));
        for (k, (n, r)) in names.iter().zip(&buttons).enumerate() {
            let lit = k == pick.sel;
            draw_rectangle(r.x, r.y, r.w, r.h, if lit { Color::new(0.2, 0.45, 0.25, 0.95) } else { Color::new(0.15, 0.15, 0.2, 0.9) });
            if lit {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.5, 1.0, 0.55, 1.0));
            }
            draw_text(n, r.x + 12.0, r.y + 20.0, 20.0, WHITE);
        }
        let hw = measure_text(HINT, None, 16, 1.0).width;
        draw_text(HINT, p.x + (p.w - hw) / 2.0, p.y + p.h - 10.0, 16.0, LIGHTGRAY);
        // The chosen character on the map.
        let pos = match pick.choices[pick.sel] {
            None => Some(self.player.pos),
            Some(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map(|e: &RtEntity| e.body.pos),
        };
        if let Some(p) = pos {
            let s = self.camera().world_to_screen(p);
            let scale = self.view_scale();
            let pulse = 0.6 + 0.3 * (t * 6.0).sin();
            draw_ellipse_lines(s.x, s.y - 2.0 * scale, 16.0 * scale, 7.0 * scale, 0.0, 2.5, Color::new(0.5, 1.0, 0.55, pulse));
            let top = self.camera().world_to_screen(p - vec2(0.0, self.ts() * 1.6));
            let bob = (t * 5.0).sin() * 3.0;
            draw_triangle(vec2(top.x - 8.0, top.y - 12.0 + bob), vec2(top.x + 8.0, top.y - 12.0 + bob), vec2(top.x, top.y + bob), Color::new(0.5, 1.0, 0.55, 0.95));
        }
    }

    /// `status()`: the chosen target while choosing (`heal:<name>`), else `-`.
    pub(super) fn heal_pick_status(&self) -> String {
        let Some(p) = &self.heal_pick else { return "-".into() };
        match p.choices[p.sel] {
            None => "heal:player".into(),
            Some(uid) => format!("heal:{}", self.loaded.entities.iter().find(|e| e.uid == uid).map_or("?".into(), |e| e.name.clone())),
        }
    }
}

/// Actions of an item used by / on someone else than the player: *Heal player* heals that
/// character ("self"), effects applied to / cured on "player" go to it instead.
pub(super) fn redirect_heals(a: Action) -> Action {
    match a {
        Action::Heal { amount } => Action::HealCharacter { target: "self".into(), amount },
        Action::ApplyEffect { target, effect } if target == "player" => Action::ApplyEffect { target: "self".into(), effect },
        Action::CureEffect { target, kind } if target == "player" => Action::CureEffect { target: "self".into(), kind },
        Action::If { conditions, then, otherwise } => Action::If { conditions, then: then.into_iter().map(redirect_heals).collect(), otherwise: otherwise.into_iter().map(redirect_heals).collect() },
        a => a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heals_are_redirected_inside_ifs() {
        let a = Action::If { conditions: vec![], then: vec![Action::Heal { amount: 4 }], otherwise: vec![Action::Heal { amount: -1 }, Action::Autosave] };
        let Action::If { then, otherwise, .. } = redirect_heals(a) else { panic!() };
        assert_eq!(then, vec![Action::HealCharacter { target: "self".into(), amount: 4 }]);
        assert_eq!(otherwise[0], Action::HealCharacter { target: "self".into(), amount: -1 });
        assert_eq!(otherwise[1], Action::Autosave);
    }
}
