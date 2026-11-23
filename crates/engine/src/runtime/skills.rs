//! The SRD sheet page and spells at runtime: the Sheet (K: ability scores, saving throws, the 18
//! skills, proficiencies, features) and Spells (L: the spellbook: known and prepared spells by
//! level, the slots, casting outside fights) tabs of the character window, learning spells, the
//! light spell's glow and the scripts' magic targeting (`aim()`). Slots, preparation and casting
//! are `spells.rs` and `turns/casting.rs`.

use std::collections::HashMap;

use macroquad::prelude::*;

use super::input::{self, Act};
use super::turns::Who;
use super::{skin, Game};
use crate::defs::LightSpec;
use crate::render::wrap_text;
use crate::srd::abilities::{self, Ability, SKILLS};
use super::text::{draw_text};

const LIST_W: f32 = 300.0;
const PAD: f32 = 16.0;
const ROW: f32 = 40.0;

/// A monster's spellcasting used today: slots spent by level, innate spells' uses by spell.
pub(super) type MonsterSpent = ([u32; 9], HashMap<String, u32>);

/// Runtime state of the systems (not saved except through `WorldState`).
#[derive(Default)]
pub(super) struct Skills {
    open: bool,
    /// 0 = skills, 1 = magic.
    tab: usize,
    sel: [usize; 2],
    /// A "Light" spell around the player: (light, seconds left).
    pub(super) light: Option<(LightSpec, f32)>,
    /// Magic targeting: waiting for the player to click a target.
    aiming: Option<Aim>,
    /// Party members concentrating on a spell: uid -> (its effects' source, seconds left).
    pub(super) npc_concentration: HashMap<u32, (String, f32)>,
    /// Monsters' spellcasting used today (by map and uid): slots spent by level, innate spells'
    /// uses. Not saved; dawn gives them back.
    pub(super) monster_spent: HashMap<(String, u32), MonsterSpent>,
    /// The spellbook's slot level for the selected spell (0 = the lowest).
    slot: u32,
}

/// What the player is aiming (a script's `aim()`).
#[derive(Debug)]
struct Aim {
    /// Script function called with the target (tile coordinates).
    script: String,
}

/// Top of the list and details below the header line.
const LIST_TOP: f32 = 56.0;

fn row_rect(r: Rect, i: usize) -> Rect {
    Rect::new(r.x + PAD, r.y + LIST_TOP + i as f32 * ROW, LIST_W - PAD, ROW - 4.0)
}

impl Game {
    /// The Skills tab (the SRD page) is always there.
    pub(super) fn skills_on(&self) -> bool {
        true
    }

    /// The Spells tab: the project has magic, or the hero casts or knows spells.
    pub(super) fn magic_on(&self) -> bool {
        self.project.config.skills.magic_on() || self.spellcasting(Who::Player).is_some() || !self.world.spells.is_empty()
    }

    pub(super) fn spell_light(&self) -> Option<LightSpec> {
        self.skills.light.as_ref().map(|l| l.0.clone())
    }

    /// Once per frame: the light spell's timer, concentration outside fights.
    pub(super) fn update_skills(&mut self, dt: f32) {
        self.refresh_armor_training();
        self.refresh_load();
        self.refresh_senses();
        self.update_magic_items();
        if let Some(l) = &mut self.skills.light {
            l.1 -= dt;
            if l.1 <= 0.0 {
                self.skills.light = None;
            }
        }
        if self.fight.is_none() {
            self.tick_concentration(dt);
            self.tick_shapes(dt, None);
            self.tick_areas(dt, None);
        }
        self.drop_concentration_when_incapacitated();
    }

    /// A new game or a loaded one: no glow, no aiming, no party member concentrating.
    pub(super) fn init_skills(&mut self, _new_game: bool) {
        self.skills.light = None;
        self.skills.npc_concentration.clear();
        self.skills.monster_spent.clear();
        self.cancel_aim();
    }

    /// Learns a spell (the *Learn spell* action, `learn_spell`): only a caster whose class list
    /// has it (the strict SRD); a known caster or a wizard adds it to what it knows. Returns
    /// false if it can't be learned.
    pub fn learn_spell(&mut self, id: &str) -> bool {
        let Some(d) = self.project.db.spell(id).cloned() else {
            eprintln!("[game] unknown spell '{id}'");
            return false;
        };
        let name = crate::lang::tr(&d.name);
        if self.world.spells.contains(id) {
            self.ui.toast(crate::lang::tr_ui("You already know {}").replace("{}", &name));
            return false;
        }
        if !self.on_list(Who::Player, &d) {
            self.ui.toast(crate::lang::tr_ui("You can't make sense of it.").into_owned());
            return false;
        }
        self.add_known(Who::Player, &[id.to_string()]);
        self.audio.play(&self.project.config.sfx.pickup);
        self.ui.toast(crate::lang::tr_ui("Learned {}").replace("{}", &name));
        true
    }

    /// Casts a spell outside a fight (the Spells tab, the action bar's Cast, `cast_spell`); a heal with
    /// companions around asks whom first. Returns true when it was cast (or the choice opened).
    pub fn cast_spell(&mut self, id: &str) -> bool {
        self.cast_spell_at(id, None)
    }

    fn cast_spell_at(&mut self, id: &str, slot: Option<u32>) -> bool {
        if self.runner.busy() {
            return false;
        }
        let Some(d) = self.project.db.spell(id).cloned() else { return false };
        if self.fight.is_some() {
            self.ui.toast(crate::lang::tr_ui("In a fight: use the action bar").into_owned());
            return false;
        }
        if d.heals() && self.can_cast(Who::Player, id) && self.start_heal_pick(super::healing::HealSource::Spell(id.to_string())) {
            return true;
        }
        match self.cast_out_of_fight(id, slot, false) {
            Ok(()) => true,
            Err(why) => {
                self.ui.toast(format!("{}: {}", crate::lang::tr(&d.name), crate::lang::tr_ui(why)));
                false
            }
        }
    }

    /// A healing spell on whom the heal choice picked (the slot spent then).
    pub(super) fn cast_heal_on(&mut self, id: &str, target: Who) -> bool {
        let Some(d) = self.project.db.spell(id).cloned() else { return false };
        let slot = if d.level == 0 {
            0
        } else {
            match self.lowest_slot(Who::Player, d.level) {
                Some(s) => s,
                None => {
                    self.ui.toast(crate::lang::tr_ui("No spell slot left").into_owned());
                    return false;
                }
            }
        };
        self.spend_slot(Who::Player, slot);
        self.resolve_spell(Who::Player, &d, slot, &[target], None);
        self.ui.toast(crate::lang::tr(&d.name));
        true
    }

    /// Starts magic targeting for a script (the Aim action / `aim()`): the chosen target goes
    /// to `function(x, y)`.
    pub(super) fn start_aim(&mut self, function: &str) {
        self.skills.aiming = Some(Aim { script: function.to_string() });
        show_mouse(false);
        self.ui.toast(crate::lang::tr_ui("Click a target  (right-click cancels, X: straight ahead)").into_owned());
    }

    pub(super) fn aiming(&self) -> bool {
        self.skills.aiming.is_some()
    }

    pub(super) fn cancel_aim(&mut self) {
        if self.skills.aiming.take().is_some() {
            show_mouse(true);
        }
    }

    /// Fires what is being aimed towards `aim` (world point; `None` = straight ahead).
    fn fire_aim(&mut self, aim: Option<Vec2>) {
        let Some(Aim { script }) = self.skills.aiming.take() else { return };
        show_mouse(true);
        let origin = self.cast_origin();
        let ts = self.ts();
        // Straight ahead: five tiles in the facing direction.
        let target = aim.unwrap_or_else(|| origin + Vec2::from(self.player.facing.dir()) * 5.0 * ts);
        if let Some(f) = crate::entity::Facing::from_vec(target.x - origin.x, target.y - origin.y) {
            self.player.facing = f;
        }
        let args = vec![rhai::Dynamic::from((target.x / ts) as f64), rhai::Dynamic::from((target.y / ts) as f64)];
        let actions = self.scripts.call_args(&mut self.world, &script, args, None);
        self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
    }

    /// Center of the enemy under the mouse while aiming.
    fn aim_target(&self) -> Option<Vec2> {
        let i = self.entity_under(self.mouse_world())?;
        let e = &self.loaded.entities[i];
        (e.hostile && e.max_hp > 0).then(|| e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0))
    }

    /// Aiming: left click fires at the enemy under the cursor (or at the clicked point), X fires
    /// straight ahead, right click / Esc cancel. Returns (key consumed, mouse consumed).
    pub(super) fn update_aim(&mut self, key_consumed: bool, mouse_consumed: bool) -> (bool, bool) {
        if !self.aiming() {
            return (false, false);
        }
        if self.modal() || self.runner.busy() {
            // A dialog or window opened meanwhile: the aim waits (the cursor stays normal there).
            return (false, false);
        }
        if !key_consumed && input::act_pressed(Act::Cancel) {
            self.cancel_aim();
            return (true, false);
        }
        if !key_consumed && input::act_pressed(Act::Attack) {
            self.fire_aim(None);
            return (true, false);
        }
        if mouse_consumed {
            return (false, false);
        }
        if input::mouse_pressed(MouseButton::Right) {
            self.cancel_aim();
            return (false, true);
        }
        if input::mouse_pressed(MouseButton::Left) {
            let aim = self.aim_target().unwrap_or_else(|| self.mouse_world());
            self.fire_aim(Some(aim));
            return (false, true);
        }
        (false, false)
    }
    /// The magic crosshair that replaces the mouse cursor while aiming (screen space).
    pub(super) fn draw_aim_cursor(&self) {
        if !self.aiming() || self.modal() {
            return;
        }
        let m = input::mouse_pos();
        let t = get_time() as f32;
        let on_enemy = self.aim_target().is_some();
        let col = if on_enemy { Color::new(1.0, 0.85, 0.3, 1.0) } else { Color::new(0.55, 0.75, 1.0, 1.0) };
        let r = if on_enemy { 14.0 } else { 11.0 } + (t * 6.0).sin() * 1.5;
        draw_circle_lines(m.x, m.y, r, 2.0, col);
        draw_circle_lines(m.x, m.y, r * 0.45, 1.5, Color::new(col.r, col.g, col.b, 0.7));
        for k in 0..4 {
            let a = t * 1.5 + k as f32 * std::f32::consts::FRAC_PI_2;
            let (c, s) = (a.cos(), a.sin());
            draw_line(m.x + c * (r + 3.0), m.y + s * (r + 3.0), m.x + c * (r + 9.0), m.y + s * (r + 9.0), 2.0, col);
        }
        draw_circle(m.x, m.y, 2.0, WHITE);
        if let Some(target) = self.aim_target() {
            // The enemy about to be hit gets a glowing ring in world space.
            let p = self.camera().world_to_screen(target);
            let scale = self.view_scale();
            draw_circle_lines(p.x, p.y, 18.0 * scale, 2.5, Color::new(1.0, 0.85, 0.3, 0.6 + 0.3 * (t * 8.0).sin()));
        }
    }

    /// Damages every character within `radius` tiles of the player (`cast`: the player's spell,
    /// which the characters hurt notice; a scripted blast isn't).
    pub(super) fn damage_around(&mut self, damage: i32, radius: f32, cast: bool, damage_type: Option<crate::srd::DamageType>) {
        self.damage_at(self.player.pos, damage, radius, cast, damage_type);
    }

    /// Damages every character within `radius` tiles of `center` (world pixels); `cast` = the
    /// player's own (see [`Self::damage_around`]).
    pub(super) fn damage_at(&mut self, center: Vec2, damage: i32, radius: f32, cast: bool, damage_type: Option<crate::srd::DamageType>) {
        let ts = self.ts();
        let dmg = damage;
        let targets: Vec<usize> = (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                e.active && e.hostile && e.max_hp > 0 && e.invuln <= 0.0 && e.body.pos.distance(center) <= radius.max(0.5) * ts
            })
            .collect();
        let attacker = cast.then_some(self.player.pos);
        for i in targets {
            self.strike_entity(i, dmg, center, 1.2, super::combat::Blow { attacker, damage_type, ..Default::default() });
        }
        self.effects.push((center, 0.0));
    }

    pub(super) fn player_light_for(&mut self, radius: f32, seconds: f32, color: [u8; 3], flicker: bool) {
        self.skills.light = Some((LightSpec { intensity: 0.9, ..LightSpec::new(radius, color, flicker) }, seconds.max(0.1)));
    }

    // ---------------------------------------------------------------- screen

    pub(super) fn skills_open(&self) -> bool {
        self.skills.open
    }

    /// 0 = the Skills tab, 1 = the Spells tab.
    pub(super) fn skills_tab(&self) -> usize {
        self.skills.tab
    }

    /// Shows the Skills (0) or Spells (1) tab of the character window.
    pub(super) fn show_skills(&mut self, tab: usize) {
        if self.skills_on() {
            self.skills.open = true;
            self.skills.tab = if self.magic_on() { tab.min(1) } else { 0 };
        }
    }

    pub(super) fn close_skills(&mut self) {
        self.skills.open = false;
    }

    /// The tab's area: the character window's content.
    fn skills_rect(&self) -> Rect {
        self.window_content()
    }

    /// Rows of the current tab: the sheet's features, or the spellbook's spells (by level).
    fn skill_rows(&self) -> Vec<String> {
        if self.skills.tab == 0 {
            // The sheet's rows: its features and traits (Up / Down read them).
            self.sheet_features().into_iter().map(|(name, _)| name).collect()
        } else {
            self.spellbook_rows()
        }
    }

    /// The spellbook's spells: those known, and those a preparing caster may prepare; by level.
    fn spellbook_rows(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.known_of(Who::Player);
        for id in self.preparable(Who::Player) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        let mut v: Vec<(u32, String, String)> = ids.into_iter().filter_map(|id| self.project.db.spell(&id).map(|d| (d.level, d.name.clone(), id.clone()))).collect();
        v.sort();
        v.into_iter().map(|(_, _, id)| id).collect()
    }

    /// Handles the open Skills / Spells tab (the keys that open, switch and close tabs are the
    /// character window's). Returns (key consumed, mouse consumed).
    pub(super) fn update_skills_ui(&mut self, allow: bool, _key_consumed: bool) -> (bool, bool) {
        if !allow || !self.skills_on() || !self.skills.open {
            return (false, false);
        }
        let r = self.skills_rect();
        let m = input::mouse_pos();
        let click = input::mouse_pressed(MouseButton::Left);
        if input::act_pressed(Act::Cancel) || (click && !self.window_rect().contains(m)) {
            self.skills.open = false;
            return (true, true);
        }
        let rows = self.skill_rows();
        let tab = self.skills.tab;
        let before = self.skills.sel[tab];
        let sel = &mut self.skills.sel[tab];
        if input::act_pressed(Act::Down) {
            *sel = (*sel + 1).min(rows.len().saturating_sub(1));
        }
        if input::act_pressed(Act::Up) {
            *sel = sel.saturating_sub(1);
        }
        // The wheel over the window moves the selection (the list follows it).
        let wheel = input::wheel();
        if wheel != 0.0 && self.window_rect().contains(m) {
            let sel = &mut self.skills.sel[tab];
            *sel = if wheel > 0.0 { sel.saturating_sub(1) } else { (*sel + 1).min(rows.len().saturating_sub(1)) };
        }
        if click {
            // (The rows shown: the list scrolls with the selection.)
            let (first, visible) = self.list_window(tab, rows.len());
            for k in 0..visible.min(rows.len().saturating_sub(first)) {
                let hit = if tab == 0 { self.sheet_feature_rect(k) } else { row_rect(r, k) };
                if hit.contains(m) {
                    self.skills.sel[tab] = first + k;
                }
            }
        }
        if self.skills.sel[tab] != before {
            self.skills.slot = 0;
        }
        // The sheet: R (gamepad X) takes a short rest.
        if tab == 0 && (input::pressed(KeyCode::R) || input::pad_pressed(input::Pad::West)) && self.combat.enabled() {
            self.skills.open = false;
            self.open_short_rest();
            return (true, true);
        }
        let sel = self.skills.sel[tab].min(rows.len().saturating_sub(1));
        let Some(id) = rows.get(sel).cloned() else { return (true, true) };
        let enter = input::act_pressed(Act::Confirm) || (click && Self::button_rect(r).contains(m));
        if tab == 1 {
            // Left / right: the slot level to cast it with.
            let level = self.project.db.spell(&id).map_or(1, |d| d.level);
            if level >= 1 {
                let slots: Vec<u32> = (level..=9).filter(|l| self.slots_left(Who::Player)[*l as usize - 1] > 0).collect();
                if !slots.is_empty() {
                    let cur = slots.iter().position(|l| *l == self.skills.slot).unwrap_or(0);
                    let step = if input::act_pressed(Act::Right) { 1 } else if input::act_pressed(Act::Left) { slots.len() - 1 } else { 0 };
                    self.skills.slot = slots[(cur + step) % slots.len()];
                }
            }
            if enter {
                let slot = (self.skills.slot >= 1).then_some(self.skills.slot);
                if self.cast_spell_at(&id, slot) {
                    self.skills.open = false;
                }
            }
            // P: prepare or unprepare it.
            if (input::pressed(KeyCode::P) || (click && Self::prepare_rect(r).contains(m))) && self.prepare_max(Who::Player) > 0 && level >= 1 {
                let on = !self.prepared_of(Who::Player).contains(&id);
                if let Err(why) = self.set_prepared(Who::Player, &id, on) {
                    self.ui.toast(crate::lang::tr_ui(why).into_owned());
                }
            }
        }
        (true, true)
    }

    /// The first row shown of tab `tab`'s list of `len` and how many rows fit: a long list
    /// scrolls with the selection (the Sheet's features, the Spells).
    fn list_window(&self, tab: usize, len: usize) -> (usize, usize) {
        let r = self.skills_rect();
        let visible = if tab == 0 {
            let bottom = r.bottom() - 132.0;
            (((bottom - self.sheet_feature_rect(0).y) / 22.0).floor() as usize).max(1)
        } else {
            (((r.h - 44.0 - LIST_TOP) / ROW).floor() as usize).max(1)
        };
        let sel = self.skills.sel[tab].min(len.saturating_sub(1));
        (sel.saturating_sub(visible - 1), visible)
    }

    fn button_rect(r: Rect) -> Rect {
        Rect::new(r.x + LIST_W + 26.0, r.y + r.h - 70.0, 200.0, 30.0)
    }

    fn prepare_rect(r: Rect) -> Rect {
        Rect::new(r.x + LIST_W + 240.0, r.y + r.h - 70.0, 200.0, 30.0)
    }

    pub(super) fn draw_skills_ui(&self) {
        if !self.skills.open {
            return;
        }
        // The window frame (panel, tabs, close button) is drawn by `draw_window_frame`.
        if self.skills.tab == 0 {
            self.draw_srd_page();
            return;
        }
        let r = self.skills_rect();
        let m = input::mouse_pos();
        // The slots, the DC and attack bonus, the prepared count.
        let (max, left) = (self.slot_max(Who::Player), self.slots_left(Who::Player));
        let mut head = String::new();
        if self.spellcasting(Who::Player).is_some() {
            let slots: Vec<String> = (0..9).filter(|k| max[*k] > 0).map(|k| format!("{}: {}/{}", k + 1, left[k], max[k])).collect();
            head = format!("{} {}   DC {}  {:+}", crate::lang::tr_ui("Slots"), if slots.is_empty() { "-".into() } else { slots.join("  ") }, self.spell_dc(Who::Player), self.spell_attack_bonus(Who::Player));
            let pm = self.prepare_max(Who::Player);
            if pm > 0 {
                head += &format!("   {} {}/{pm}", crate::lang::tr_ui("prepared"), self.prepared_of(Who::Player).len());
            }
            if let Some(c) = &self.world.concentration {
                let name = self.project.db.spell(c.0.split('@').next().unwrap_or("")).map_or(String::new(), |d| crate::lang::tr(&d.name));
                head += &format!("   {}: {name}", crate::lang::tr_ui("Concentration"));
            }
        }
        super::text::draw_plain(&head, r.x + PAD, r.y + 32.0, 19.0, skin::dim());
        let help = "{key:up}/{key:down}: select   {key:left}/{key:right}: slot   {key:confirm}: cast   P: prepare   {key:spells} / {key:cancel}: close";
        super::glyphs::draw_hint_in(help, r.x + PAD, skin::hint_baseline(r.y + r.h), 16.0, r.w - 2.0 * PAD, skin::disabled());
        draw_line(r.x + LIST_W + 8.0, r.y + LIST_TOP, r.x + LIST_W + 8.0, r.y + r.h - 40.0, 1.0, skin::line(0.4));

        let rows = self.skill_rows();
        let sel = self.skills.sel[1].min(rows.len().saturating_sub(1));
        if rows.is_empty() {
            let t = if self.spellcasting(Who::Player).is_some() { "No spells yet: your level-ups choose them." } else { "Your class casts no spells." };
            draw_text(t, r.x + PAD, r.y + LIST_TOP + 26.0, 20.0, skin::dim());
            return;
        }
        let prepared = self.prepared_of(Who::Player);
        let castable: Vec<String> = self.cast_options(Who::Player).into_iter().map(|o| o.0).collect();
        let (first, _) = self.list_window(1, rows.len());
        for (i, id) in rows.iter().enumerate().skip(first) {
            let rr = row_rect(r, i - first);
            if rr.y + rr.h > r.y + r.h - 44.0 {
                break;
            }
            let on = i == sel;
            if on {
                if !skin::cursor(rr, Color::new(0.85, 0.7, 0.3, 0.22)) {
                    draw_rectangle_lines(rr.x, rr.y, rr.w, rr.h, 2.0, skin::highlight());
                }
            } else if rr.contains(m) {
                draw_rectangle(rr.x, rr.y, rr.w, rr.h, Color::new(1.0, 1.0, 1.0, 0.08));
            }
            let Some(d) = self.project.db.spell(id) else { continue };
            let ready = castable.contains(id);
            let col = if ready { skin::normal() } else { skin::disabled() };
            if !d.icon.sheet.is_empty() {
                self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(rr.x + 4.0, rr.y + 4.0, 28.0, 28.0), Color::new(1.0, 1.0, 1.0, if ready { 1.0 } else { 0.4 }));
            }
            super::text::draw_plain(&crate::lang::tr(&d.name), rr.x + 38.0, rr.y + 17.0, 19.0, col);
            let mut sub = if d.level == 0 { crate::lang::tr_ui("Cantrip").into_owned() } else { format!("{} {}", crate::lang::tr_ui("Level"), d.level) };
            if prepared.contains(id) {
                sub += &format!("  -  {}", crate::lang::tr_ui("prepared"));
            }
            if d.ritual {
                sub += "  -  R";
            }
            if d.concentration {
                sub += "  -  C";
            }
            draw_text(&sub, rr.x + 38.0, rr.y + 32.0, 14.0, skin::disabled());
        }

        // Details.
        let x = r.x + LIST_W + 26.0;
        let w = r.x + r.w - PAD - x;
        let mut y = r.y + LIST_TOP + 24.0;
        let id = &rows[sel];
        let Some(d) = self.project.db.spell(id) else { return };
        super::text::draw_plain(&crate::lang::tr(&d.name), x, y, 28.0, skin::highlight());
        y += 26.0;
        let school = crate::lang::tr_ui(&d.school);
        let level = if d.level == 0 { format!("{school} {}", crate::lang::tr_ui("cantrip")) } else { format!("{} {}, {school}", crate::lang::tr_ui("Level"), d.level) };
        let mut info = vec![level, d.casting.label(), d.range.label()];
        if let Some(a) = d.area {
            info.push(a.label());
        }
        if !d.components.is_empty() {
            info.push(d.components.clone());
        }
        if !d.duration.is_empty() {
            info.push(d.duration.clone());
        }
        super::text::draw_plain(&info.join("  -  "), x, y, 17.0, skin::dim());
        y += 22.0;
        let summary = self.spell_summary(Who::Player, d, self.skills.slot.max(d.level));
        if !summary.is_empty() {
            super::text::draw_plain(&summary, x, y, 17.0, skin::text(skin::Text::Dim, Color::new(0.85, 0.8, 0.6, 1.0)));
            y += 6.0;
        }
        let bottom = Self::button_rect(r).y - 10.0;
        for line in wrap_text(&self.world.expand_text(&crate::lang::tr(&d.description)), 18, w) {
            y += 22.0;
            if y > bottom {
                break;
            }
            super::text::draw_plain(&line, x, y, 18.0, skin::normal());
        }
        let b = Self::button_rect(r);
        let ready = castable.contains(id);
        if !skin::button_picture(b, ready && b.contains(m)) {
            draw_rectangle(b.x, b.y, b.w, b.h, if ready { Color::new(0.4, 0.5, 0.9, 0.35) } else { Color::new(1.0, 1.0, 1.0, 0.06) });
            draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.5, if ready { Color::new(0.6, 0.7, 1.0, 1.0) } else { skin::disabled() });
        }
        let slot = if d.level == 0 { 0 } else { (self.skills.slot >= d.level).then_some(self.skills.slot).or_else(|| self.lowest_slot(Who::Player, d.level)).unwrap_or(d.level) };
        let label = if !ready {
            crate::lang::tr_ui(self.cast_refusal(Who::Player, id, "Not prepared")).into_owned()
        } else if slot >= 1 {
            format!("{}  ({} {slot})", crate::lang::tr_ui("Cast"), crate::lang::tr_ui("slot"))
        } else {
            crate::lang::tr_ui("Cast").into_owned()
        };
        draw_text(&label, b.x + 12.0, b.y + 21.0, 18.0, if ready { skin::normal() } else { skin::disabled() });
        if self.prepare_max(Who::Player) > 0 && d.level >= 1 {
            let p = Self::prepare_rect(r);
            draw_rectangle_lines(p.x, p.y, p.w, p.h, 1.5, skin::dim());
            let t = if prepared.contains(id) { "Unprepare (P)" } else { "Prepare (P)" };
            draw_text(t, p.x + 12.0, p.y + 21.0, 18.0, skin::normal());
        }
    }

    /// The Skills tab: the hero's ability scores with modifiers and saving throws, the 18 skills
    /// with their bonuses (proficient ones marked), the proficiency bonus and other proficiencies.
    /// The sheet's features and traits: the race's traits, then the background's feature (name,
    /// rules text).
    pub(super) fn sheet_features(&self) -> Vec<(String, String)> {
        // The class's and subclass's features at the hero's level (and the options picked).
        let mut v: Vec<(String, String)> = match (self.hero_class(), self.hero_traits()) {
            (Some(c), Some(t)) => t
                .features
                .iter()
                .filter_map(|id| c.feature(id))
                .filter(|f| f.choice.is_none() || !f.effects.is_empty() || !f.text.is_empty())
                .map(|f| {
                    // (A feature with uses shows those left: "Rage 2/3".)
                    let uses = f.effects.iter().find_map(|e| if let crate::classes::FeatureEffect::Resource { id, .. } = e { self.uses(super::turns::Who::Player, id) } else { None });
                    let name = crate::lang::tr(&f.name);
                    (uses.map_or(name.clone(), |(l, m)| format!("{name}  {l}/{m}")), crate::lang::tr(&f.text))
                })
                .collect(),
            _ => Vec::new(),
        };
        v.extend(self.player_race().map_or(Vec::new(), |r| r.traits.iter().map(|t| (t.name.clone(), t.description.clone())).collect::<Vec<_>>()));
        if let Some(b) = self.project.db.background(&self.world.player_background).filter(|b| !b.feature.is_empty()) {
            v.push((b.feature.clone(), b.feature_text.clone()));
        }
        // The magic items at work (attuned ones marked n/3), then those attuned but not worn.
        let n = self.world.attuned.len();
        let mark = |g: &Self, id: &str| if g.is_attuned(id) { format!("  ({} {n}/{})", crate::lang::tr_ui("attuned"), crate::srd::magic_items::MAX_ATTUNED) } else { String::new() };
        let working: Vec<String> = self.magic_in_effect(super::turns::Who::Player).iter().map(|d| d.id.clone()).collect();
        for id in working.iter().chain(self.world.attuned.iter().filter(|a| !working.contains(a))) {
            if let Some(d) = self.project.db.item(id) {
                v.push((format!("{}{}", crate::lang::tr(&d.name), mark(self, id)), crate::lang::tr(&d.description)));
            }
        }
        v
    }

    /// The sheet's columns: (left x, middle x, right x, top y).
    fn sheet_columns(&self) -> (f32, f32, f32, f32) {
        let r = self.skills_rect();
        (r.x + PAD, r.x + 282.0, r.x + 532.0, r.y + 78.0)
    }

    /// Where feature `k` is listed (the right column, under languages and proficiencies).
    fn sheet_feature_rect(&self, k: usize) -> Rect {
        let r = self.skills_rect();
        let (_, _, x, top) = self.sheet_columns();
        Rect::new(x - 6.0, top + 238.0 + k as f32 * 22.0, r.right() - x - PAD + 6.0, 21.0)
    }

    /// The character sheet (K): who the hero is, the abilities and saving throws, AC, initiative,
    /// speed, hit points and hit dice, passive Perception, the 18 skills, languages,
    /// proficiencies, and the features and traits (the selected one's text at the bottom).
    fn draw_srd_page(&self) {
        let r = self.skills_rect();
        let w = &self.world;
        let gold = skin::highlight();
        let (lx, mx, rx, top) = self.sheet_columns();
        // Who.
        let race = self.player_race();
        let mut who = vec![w.player_name.clone()];
        if let Some(r) = &race {
            who.push(r.name.clone());
        }
        if let Some(b) = self.project.db.background(&w.player_background) {
            who.push(b.name.clone());
        }
        if let (Some(c), Some(hc)) = (self.hero_class(), &w.class) {
            let sub = c.subclass(&hc.subclass).map_or(String::new(), |s| format!(" ({})", crate::lang::tr(&s.name)));
            who.push(format!("{}{sub}", crate::lang::tr(&c.name)));
        }
        super::text::draw_plain(&who.join("  -  "), r.x + PAD, r.y + 32.0, 22.0, gold);
        let prof = abilities::proficiency_bonus(w.player_level());
        let line = format!("Level {}   -   Proficiency bonus {}", w.player_level(), abilities::signed(prof as i64));
        draw_text(&line, r.x + PAD, r.y + 56.0, 17.0, skin::dim());
        if self.progression_on() {
            let p = &self.project.config.progression;
            let xp = format!("XP {} / {}", w.xp, p.xp_for_level(w.player_level() + 1));
            let tw = super::text::measure_text(&xp, None, 17, 1.0).width;
            draw_text(&xp, r.right() - PAD - tw, r.y + 56.0, 17.0, skin::dim());
        }
        super::glyphs::draw_hint_in("{key:up}/{key:down}: features   R: short rest   {key:next_tab}: next tab   {key:skills} / {key:cancel}: close", r.x + PAD, skin::hint_baseline(r.y + r.h), 16.0, r.w - 2.0 * PAD, skin::disabled());

        // Left: abilities with modifier and save, then the numbers of a stat block.
        let mut y = top;
        draw_text("Ability", lx, y, 15.0, skin::disabled());
        // (Smaller when a translation would reach the skills column: "Jet de sauvegarde".)
        let save_head = crate::lang::tr_ui("Saving throw");
        super::text::draw_plain(&save_head, lx + 170.0, y, super::text::fit_size(&save_head, 15.0, mx - lx - 180.0, 11.0), skin::disabled());
        for a in Ability::ALL {
            y += 28.0;
            let score = w.abilities.score(a);
            draw_text(a.short(), lx, y, 21.0, gold);
            draw_text(format!("{score}"), lx + 50.0, y, 21.0, skin::normal());
            draw_text(format!("({})", abilities::signed(abilities::modifier(score) as i64)), lx + 84.0, y, 18.0, skin::dim());
            let save = w.check_bonus(a.id(), true).unwrap_or(0);
            let proficient = w.is_proficient(&format!("save:{}", a.id()));
            draw_text(abilities::signed(save), lx + 178.0, y, 19.0, if proficient { gold } else { skin::dim() });
            if proficient {
                draw_circle(lx + 168.0, y - 6.0, 3.5, gold);
            }
        }
        y += 36.0;
        let hp_max = self.player_max_hp();
        let level = w.player_level();
        let cond = crate::srd::conditions::rules(&w.effects);
        // (The purse, when the project has coins: money isn't carried in the bag.)
        let money = if w.coins.is_empty() { None } else { Some(("Money", Self::format_cp(self.money_cp()))) };
        let facts = [
            ("Armor class", self.armor_class().to_string()),
            ("Initiative", abilities::signed(abilities::modifier(w.abilities.score(Ability::Dex)) as i64)),
            ("Speed", format!("{} ft", (self.hero_speed_ft() as f32 * cond.speed).round() as u32)),
            ("Hit points", if hp_max > 0 { format!("{} / {hp_max}", w.player_hp.unwrap_or(hp_max).max(0)) } else { "-".into() }),
            ("Hit dice", format!("{}/{} d{}", self.hit_dice_left(), level, self.hit_die())),
            // (Disadvantage -5, darkness 0, the fast pace -5: WorldState::passive.)
            ("Passive Perception", self.hero_passive_perception().to_string()),
        ];
        // What the hero carries (pounds / capacity), and today's food and water when they count.
        let load = ("Carried", format!("{} / {} lb", self.moves.carried.round() as i64, crate::srd::survival::capacity(w.abilities.score(Ability::Str)) as i64));
        let fed = self.project.config.rules.food_water.then(|| ("Food / water today", format!("{:.1} / {:.1}", w.survival.food, w.survival.water)));
        let loaded = self.moves.load != crate::srd::survival::Load::Light;
        for (label, value) in facts.into_iter().chain(money).chain(Some(load)).chain(fed) {
            draw_text(label, lx, y, 17.0, skin::dim());
            let vw = super::text::measure_text(&value, None, 19, 1.0).width;
            let warn = (label == "Carried" && loaded) || (label == "Food / water today" && w.survival.unfed > 0.0);
            draw_text(&value, mx - 24.0 - vw, y, 19.0, if warn { skin::highlight() } else { skin::normal() });
            y += 23.0;
        }
        // The conditions (exhaustion with its level) and what goes on every round.
        y += 6.0;
        draw_text("Conditions", lx, y, 15.0, skin::disabled());
        let names: Vec<String> = w
            .effects
            .iter()
            .map(|a| match a.condition {
                Some(crate::srd::Condition::Exhaustion) => format!("{} {}", crate::lang::tr_ui("Exhaustion"), a.level),
                Some(c) if c != crate::srd::Condition::Any => crate::lang::tr_ui(c.name()).into_owned(),
                _ => a.badge().split(' ').next().unwrap_or("").to_string(),
            })
            .collect();
        let text = if names.is_empty() { crate::lang::tr_ui("None").into_owned() } else { names.join(", ") };
        for l in wrap_text(&text, 16, mx - lx - 24.0).into_iter().take(2) {
            y += 19.0;
            super::text::draw_plain(&l, lx, y, 16.0, if names.is_empty() { skin::dim() } else { skin::highlight() });
        }

        // Middle: the 18 skills, the bonus, the ability and the name (a dot = proficient).
        draw_line(mx - 12.0, top - 10.0, mx - 12.0, r.bottom() - 132.0, 1.0, skin::line(0.4));
        let step = ((r.bottom() - 140.0 - top) / SKILLS.len() as f32).clamp(18.0, 24.0);
        for (i, (id, name, a)) in SKILLS.iter().enumerate() {
            let cy = top + 4.0 + i as f32 * step;
            let proficient = w.is_proficient(id);
            let bonus = w.check_bonus(id, false).unwrap_or(0);
            if proficient {
                draw_circle(mx + 4.0, cy - 6.0, 3.0, gold);
            }
            draw_text(abilities::signed(bonus), mx + 12.0, cy, 17.0, if proficient { gold } else { skin::dim() });
            draw_text(a.short(), mx + 46.0, cy, 13.0, skin::disabled());
            draw_text(name, mx + 82.0, cy, 17.0, skin::normal());
        }

        // Right: languages, proficiencies (tools, weapons, armor), features and traits.
        draw_line(rx - 12.0, top - 10.0, rx - 12.0, r.bottom() - 132.0, 1.0, skin::line(0.4));
        let cw = r.right() - rx - PAD;
        let langs: Vec<String> = w.languages.iter().map(|id| crate::backgrounds::language_label(id)).collect();
        draw_text("Languages", rx, top, 15.0, skin::disabled());
        let mut yy = top + 20.0;
        for l in wrap_text(&if langs.is_empty() { "-".to_string() } else { langs.join(", ") }, 16, cw).into_iter().take(3) {
            super::text::draw_plain(&l, rx, yy, 16.0, skin::normal());
            yy += 18.0;
        }
        let profs: Vec<String> = w.proficiencies.iter().filter(|p| abilities::skill(p).is_none() && !p.starts_with("save:")).map(|p| crate::lang::tr_ui(&abilities::proficiency_name(p)).into_owned()).collect();
        draw_text("Proficiencies", rx, top + 84.0, 15.0, skin::disabled());
        let mut yy = top + 104.0;
        for l in wrap_text(&if profs.is_empty() { "-".to_string() } else { profs.join(", ") }, 15, cw).into_iter().take(6) {
            super::text::draw_plain(&l, rx, yy, 15.0, skin::normal());
            yy += 17.0;
        }
        draw_text("Features and traits", rx, top + 226.0, 15.0, skin::disabled());
        let features = self.sheet_features();
        let sel = self.skills.sel[0].min(features.len().saturating_sub(1));
        let bottom = r.bottom() - 132.0;
        // (A long list scrolls with the selection.)
        let (first, _) = self.list_window(0, features.len());
        for (k, (name, _)) in features.iter().enumerate().skip(first) {
            let fr = self.sheet_feature_rect(k - first);
            if fr.bottom() > bottom {
                break;
            }
            if k == sel && !skin::cursor(fr, Color::new(0.85, 0.7, 0.3, 0.22)) {
                draw_rectangle_lines(fr.x, fr.y, fr.w, fr.h, 1.5, gold);
            }
            super::text::draw_plain(name, fr.x + 6.0, fr.y + 16.0, 17.0, if k == sel { gold } else { skin::normal() });
        }
        // The selected feature's rules at the bottom.
        draw_line(r.x + PAD, bottom + 6.0, r.right() - PAD, bottom + 6.0, 1.0, skin::line(0.4));
        if let Some((name, text)) = features.get(sel) {
            super::text::draw_plain(name, r.x + PAD, bottom + 28.0, 18.0, gold);
            let mut ty = bottom + 48.0;
            let lines = wrap_text(text, 15, r.w - 2.0 * PAD);
            let n = lines.len();
            for (k, l) in lines.into_iter().take(4).enumerate() {
                let l = if k == 3 && n > 4 { format!("{l}...") } else { l };
                super::text::draw_plain(&l, r.x + PAD, ty, 15.0, skin::dim());
                ty += 17.0;
            }
        }
    }

    /// For automated play-throughs.
    pub fn skills_status(&self) -> String {
        // (The character window's Sheet / Spells selections: `listsel=feature:K/ROWS,spell:K/ROWS`.)
        format!(
            "spells={:?} {} light={} aiming={:?} listsel=feature:{}/{},spell:{}/{}",
            self.world.spells,
            self.spells_status(),
            self.skills.light.is_some(),
            self.skills.aiming.as_ref().map(|a| a.script.clone()),
            self.skills.sel[0],
            self.sheet_features().len(),
            self.skills.sel[1],
            self.spellbook_rows().len()
        )
    }
}
