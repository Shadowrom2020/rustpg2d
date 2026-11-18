//! The SRD skills page and magic at runtime: mana, learning and casting spells, and the Skills
//! (K: ability scores, saving throws, the 18 skills, proficiencies) and Spells (L) tabs of the
//! character window (the mana bar is part of the HUD's status frame, `combat.rs`).

use std::collections::HashMap;

use macroquad::prelude::*;

use super::input::{self, Act};
use super::{skin, Game};
use crate::defs::LightSpec;
use crate::render::wrap_text;
use crate::skills::{SpellEffect, SPELL_PREFIX};
use crate::srd::abilities::{self, Ability, SKILLS};
use super::text::{draw_text};

const LIST_W: f32 = 300.0;
const PAD: f32 = 16.0;
const ROW: f32 = 40.0;

/// Runtime state of the systems (not saved except through `WorldState`).
#[derive(Default)]
pub(super) struct Skills {
    open: bool,
    /// 0 = skills, 1 = magic.
    tab: usize,
    sel: [usize; 2],
    /// Spell cooldowns left, by spell id.
    cooldown: HashMap<String, f32>,
    /// A "Light" spell around the player: (light, seconds left).
    light: Option<(LightSpec, f32)>,
    /// Magic targeting: waiting for the player to click a target.
    aiming: Option<Aim>,
    /// A script spell being cast: its cost is paid when the script's aim gets a target.
    casting: Option<String>,
}

/// What the player is aiming.
#[derive(Debug)]
struct Aim {
    /// The spell paid for when the target is chosen ("" = none, e.g. aim() from a trigger).
    spell: String,
    /// Script function called with the target (tile coordinates); `None` = a bolt spell.
    script: Option<String>,
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
    pub(super) fn magic_on(&self) -> bool {
        self.project.config.skills.magic_on()
    }

    pub fn max_mana(&self) -> u32 {
        if !self.magic_on() {
            return 0;
        }
        self.project.config.skills.max_mana + self.level_mana()
    }
    fn mana_regen(&self) -> f32 {
        self.project.config.skills.mana_regen
    }
    pub(super) fn spell_light(&self) -> Option<LightSpec> {
        self.skills.light.as_ref().map(|l| l.0.clone())
    }

    /// Once per frame: mana regeneration, cooldowns, the light spell's timer.
    pub(super) fn update_skills(&mut self, dt: f32) {
        if !self.magic_on() {
            return;
        }
        let max = self.max_mana() as f32;
        self.world.mana = (self.world.mana + self.mana_regen() * dt).min(max);
        for v in self.skills.cooldown.values_mut() {
            *v -= dt;
        }
        self.skills.cooldown.retain(|_, v| *v > 0.0);
        if let Some(l) = &mut self.skills.light {
            l.1 -= dt;
            if l.1 <= 0.0 {
                self.skills.light = None;
            }
        }
    }

    /// New game: full mana. Loading: keeps the save's value.
    pub(super) fn init_skills(&mut self, new_game: bool) {
        self.skills.cooldown.clear();
        self.skills.light = None;
        self.cancel_aim();
        if new_game {
            self.world.mana = self.max_mana() as f32;
        } else {
            self.world.mana = self.world.mana.min(self.max_mana() as f32);
        }
    }

    /// Why a spell can't be learned / cast right now.
    fn spell_requirement(&self, id: &str) -> Option<String> {
        let d = self.project.db.spell(id)?;
        if !d.skill.trim().is_empty() && !self.world.is_proficient(&d.skill) {
            return Some(format!("needs proficiency in {}", abilities::proficiency_name(&abilities::proficiency_id(&d.skill))));
        }
        None
    }

    /// Teaches a spell (scrolls, books, the "Learn spell" action). Returns false if it can't be
    /// learned (unknown, magic off, requirement not met).
    pub fn learn_spell(&mut self, id: &str) -> bool {
        if !self.magic_on() {
            self.ui.toast("You can't make sense of it.".into());
            return false;
        }
        let Some(d) = self.project.db.spell(id).cloned() else {
            eprintln!("[game] unknown spell '{id}'");
            return false;
        };
        if self.world.spells.contains(id) {
            self.ui.toast(format!("You already know {}", d.name));
            return false;
        }
        if let Some(why) = self.spell_requirement(id) {
            self.ui.toast(format!("Too hard for you yet: {why}"));
            return false;
        }
        self.world.spells.insert(id.to_string());
        self.audio.play(&self.project.config.sfx.pickup);
        self.ui.toast(format!("Learned {}  (K: magic)", d.name));
        true
    }

    pub(super) fn add_mana(&mut self, n: i32) {
        let max = self.max_mana() as f32;
        self.world.mana = (self.world.mana + n as f32).clamp(0.0, max);
    }

    /// Casts a learned spell: mana, cooldown, then its effect.
    pub fn cast_spell(&mut self, id: &str) -> bool {
        if !self.magic_on() || self.runner.busy() {
            return false;
        }
        let Some(d) = self.project.db.spell(id).cloned() else { return false };
        if !self.world.spells.contains(id) {
            self.ui.toast(format!("You don't know {}", d.name));
            return false;
        }
        if let Some(why) = self.spell_requirement(id) {
            self.ui.toast(format!("{}: {why}", d.name));
            return false;
        }
        if self.skills.cooldown.get(id).is_some_and(|c| *c > 0.0) {
            return false;
        }
        if self.world.mana < d.mana as f32 {
            self.ui.toast("Not enough mana".into());
            return false;
        }
        // Bolts are aimed with the mouse first; mana and cooldown are taken when they fly.
        if matches!(d.effect, SpellEffect::Bolt { .. }) {
            self.skills.aiming = Some(Aim { spell: id.to_string(), script: None });
            show_mouse(false);
            self.ui.toast(format!("{}: click an enemy to aim  (right-click cancels, X: straight ahead)", d.name));
            return true;
        }
        // A script that starts aiming (aim()) pays when the target is chosen.
        if let SpellEffect::Script { function } = &d.effect {
            self.sync_script_player();
            let actions = self.scripts.call(&mut self.world, function, None);
            if actions.iter().any(|a| matches!(a, crate::events::Action::Aim { .. })) {
                self.skills.casting = Some(id.to_string());
            } else {
                self.pay_spell(&d);
                self.ui.toast(d.name.to_string());
            }
            self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
            return true;
        }
        // With companions around, a Heal spell asks whom to heal (paid once chosen).
        if matches!(d.effect, SpellEffect::Heal { .. }) && self.start_heal_pick(super::healing::HealSource::Spell(id.to_string())) {
            return true;
        }
        self.pay_spell(&d);
        match &d.effect {
            SpellEffect::Heal { amount } => self.heal(*amount),
            SpellEffect::Bolt { .. } => {}
            SpellEffect::Blast { damage, radius } => self.damage_around(*damage, *radius, true),
            SpellEffect::Light { radius, seconds } => self.player_light_for(*radius, *seconds, crate::events::spell_light_color(), false),
            SpellEffect::Script { .. } => {}
        }
        self.ui.toast(d.name.to_string());
        true
    }

    /// Spends a spell's mana, starts its cooldown and plays the cast sound.
    pub(super) fn pay_spell(&mut self, d: &crate::skills::SpellDef) {
        self.world.mana -= d.mana as f32;
        if d.cooldown > 0.0 {
            self.skills.cooldown.insert(d.id.clone(), d.cooldown);
        }
        self.audio.play(&self.project.config.sfx.attack);
    }

    /// Starts magic targeting for a script (the Aim action / `aim()`): the chosen target goes
    /// to `function(x, y)`. Inside a spell's script the spell is paid for then.
    pub(super) fn start_aim(&mut self, function: &str) {
        let spell = self.skills.casting.take().unwrap_or_default();
        let name = self.project.db.spell(&spell).map_or("Magic".to_string(), |d| d.name.clone());
        self.skills.aiming = Some(Aim { spell, script: Some(function.to_string()) });
        show_mouse(false);
        self.ui.toast(format!("{name}: click a target  (right-click cancels, X: straight ahead)"));
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
        let Some(Aim { spell, script }) = self.skills.aiming.take() else { return };
        show_mouse(true);
        let def = self.project.db.spell(&spell).cloned();
        if let Some(d) = &def {
            if self.world.mana < d.mana as f32 {
                self.ui.toast("Not enough mana".into());
                return;
            }
        }
        let origin = self.cast_origin();
        let ts = self.ts();
        let Some(function) = script else {
            // A bolt spell.
            let Some(d) = def else { return };
            let SpellEffect::Bolt { damage, projectile, speed, range } = &d.effect else { return };
            self.pay_spell(&d);
            let dir = aim.and_then(|t| (t - origin).try_normalize()).unwrap_or_else(|| Vec2::from(self.player.facing.dir()));
            if let Some(f) = crate::entity::Facing::from_vec(dir.x, dir.y) {
                self.player.facing = f;
            }
            let r = crate::defs::Ranged { projectile: projectile.clone(), speed: *speed, range: *range, ammo: String::new() };
            self.shoot(origin, dir, &r, *damage, true, None, None);
            self.ui.toast(d.name);
            return;
        };
        if let Some(d) = &def {
            self.pay_spell(d);
            self.ui.toast(d.name.clone());
        }
        // Straight ahead: five tiles in the facing direction.
        let target = aim.unwrap_or_else(|| origin + Vec2::from(self.player.facing.dir()) * 5.0 * ts);
        if let Some(f) = crate::entity::Facing::from_vec(target.x - origin.x, target.y - origin.y) {
            self.player.facing = f;
        }
        let args = vec![rhai::Dynamic::from((target.x / ts) as f64), rhai::Dynamic::from((target.y / ts) as f64)];
        let actions = self.scripts.call_args(&mut self.world, &function, args, None);
        self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
    }

    /// Center of the enemy under the mouse while aiming.
    fn aim_target(&self) -> Option<Vec2> {
        let i = self.entity_under(self.mouse_world())?;
        let e = &self.loaded.entities[i];
        (e.hostile && e.max_hp > 0).then(|| e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0))
    }

    /// Aiming a bolt: left click fires at the enemy under the cursor (or at the clicked point),
    /// X fires straight ahead, right click / Esc cancel. Returns (key consumed, mouse consumed).
    pub(super) fn update_aim(&mut self, key_consumed: bool, mouse_consumed: bool) -> (bool, bool) {
        if !self.aiming() {
            return (false, false);
        }
        if self.modal() || self.runner.busy() {
            // A dialog or window opened meanwhile: the spell waits (the cursor stays normal there).
            return (false, false);
        }
        if !key_consumed && input::act_pressed(Act::Cancel) {
            self.cancel_aim();
            self.ui.toast("Spell cancelled".into());
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
            self.ui.toast("Spell cancelled".into());
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
    pub(super) fn damage_around(&mut self, damage: i32, radius: f32, cast: bool) {
        self.damage_at(self.player.pos, damage, radius, cast);
    }

    /// Damages every character within `radius` tiles of `center` (world pixels); `cast` = the
    /// player's own (see [`Self::damage_around`]).
    pub(super) fn damage_at(&mut self, center: Vec2, damage: i32, radius: f32, cast: bool) {
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
            self.strike_entity(i, dmg, center, 1.2, super::combat::Blow { attacker, ..Default::default() });
        }
        self.effects.push((center, 0.0));
    }

    pub(super) fn player_light_for(&mut self, radius: f32, seconds: f32, color: [u8; 3], flicker: bool) {
        self.skills.light = Some((LightSpec { intensity: 0.9, ..LightSpec::new(radius, color, flicker) }, seconds.max(0.1)));
    }

    /// Whether a quick bar entry is a spell (and its id).
    pub(super) fn quick_spell(entry: &str) -> Option<&str> {
        entry.strip_prefix(SPELL_PREFIX)
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

    /// Rows of the current tab: the SRD skills' ids or spell ids.
    fn skill_rows(&self) -> Vec<String> {
        if self.skills.tab == 0 {
            SKILLS.iter().map(|(id, _, _)| id.to_string()).collect()
        } else {
            // Learned first, then the rest (greyed out).
            let mut v: Vec<(bool, String)> = self.project.db.spells.iter().map(|d| (!self.world.spells.contains(&d.id), d.id.clone())).collect();
            v.sort_by_key(|(unknown, _)| *unknown);
            v.into_iter().map(|(_, id)| id).collect()
        }
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
        let sel = &mut self.skills.sel[tab];
        if input::act_pressed(Act::Down) {
            *sel = (*sel + 1).min(rows.len().saturating_sub(1));
        }
        if input::act_pressed(Act::Up) {
            *sel = sel.saturating_sub(1);
        }
        if click {
            for i in 0..rows.len() {
                if row_rect(r, i).contains(m) {
                    *sel = i;
                }
            }
        }
        let sel = self.skills.sel[tab].min(rows.len().saturating_sub(1));
        let Some(id) = rows.get(sel).cloned() else { return (true, true) };
        let enter = input::act_pressed(Act::Confirm) || (click && Self::button_rect(r).contains(m));
        if tab == 1 {
            if enter && self.cast_spell(&id) {
                self.skills.open = false;
            }
            // 1-8 puts the spell on the quick bar.
            if let Some(k) = super::inventory_ui::quick_key_pressed() {
                if self.world.spells.contains(&id) {
                    self.assign_quick(k, &format!("{SPELL_PREFIX}{id}"));
                    self.ui.toast(format!("{} on quick slot {}", self.project.db.spell(&id).map_or(id.clone(), |d| d.name.clone()), k + 1));
                }
            }
        }
        (true, true)
    }

    fn button_rect(r: Rect) -> Rect {
        Rect::new(r.x + LIST_W + 26.0, r.y + r.h - 70.0, 200.0, 30.0)
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
        let head = format!("Mana {:.0} / {}   -   spells known: {}", self.world.mana, self.max_mana(), self.world.spells.len());
        draw_text(&head, r.x + PAD, r.y + 32.0, 22.0, skin::dim());
        let help = "{key:up}/{key:down}: select   {key:confirm}: cast   1-8: put on the quick bar   {key:next_tab}: next tab   {key:spells} / {key:cancel}: close";
        super::glyphs::draw_hint_in(help, r.x + PAD, r.y + r.h - 14.0, 16.0, r.w - 2.0 * PAD, skin::disabled());
        draw_line(r.x + LIST_W + 8.0, r.y + LIST_TOP, r.x + LIST_W + 8.0, r.y + r.h - 40.0, 1.0, skin::line(0.4));

        let rows = self.skill_rows();
        let tab = self.skills.tab;
        let sel = self.skills.sel[tab].min(rows.len().saturating_sub(1));
        if rows.is_empty() {
            draw_text("No spells in this game.", r.x + PAD, r.y + LIST_TOP + 26.0, 22.0, skin::dim());
            return;
        }
        for (i, id) in rows.iter().enumerate() {
            let rr = row_rect(r, i);
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
            let known = self.world.spells.contains(id);
            let col = if known { skin::normal() } else { skin::disabled() };
            if !d.icon.sheet.is_empty() {
                self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(rr.x + 4.0, rr.y + 4.0, 28.0, 28.0), Color::new(1.0, 1.0, 1.0, if known { 1.0 } else { 0.4 }));
            }
            draw_text(&d.name, rr.x + 38.0, rr.y + 17.0, 19.0, col);
            let sub = if known {
                match self.world.quickbar.iter().position(|q| q == &format!("{SPELL_PREFIX}{id}")) {
                    Some(k) => format!("{} mana  -  quick slot {}", d.mana, k + 1),
                    None => format!("{} mana", d.mana),
                }
            } else {
                "not learned".to_string()
            };
            draw_text(&sub, rr.x + 38.0, rr.y + 32.0, 14.0, skin::disabled());
        }

        // Details.
        let x = r.x + LIST_W + 26.0;
        let w = r.x + r.w - PAD - x;
        let mut y = r.y + LIST_TOP + 24.0;
        let id = &rows[sel];
        let Some(d) = self.project.db.spell(id) else { return };
        let known = self.world.spells.contains(id);
        draw_text(&d.name, x, y, 28.0, skin::highlight());
        y += 26.0;
        let mut info = format!("{} mana", d.mana);
        if d.cooldown > 0.0 {
            info += &format!("  -  {:.1}s cooldown", d.cooldown);
        }
        if !d.skill.is_empty() {
            info += &format!("  -  needs {}", abilities::proficiency_name(&abilities::proficiency_id(&d.skill)));
        }
        draw_text(&info, x, y, 18.0, skin::dim());
        for line in wrap_text(&self.world.expand_text(&d.description), 20, w) {
            y += 26.0;
            draw_text(&line, x, y, 20.0, skin::normal());
        }
        y += 30.0;
        let effect = match &d.effect {
            SpellEffect::Heal { amount } => format!("Heals {} heart{}", *amount as f32 / 2.0, if *amount == 2 { "" } else { "s" }),
            SpellEffect::Bolt { damage, range, .. } => format!("Bolt: {damage} damage, {range:.0} tiles"),
            SpellEffect::Blast { damage, radius } => format!("Blast: {damage} damage within {radius:.0} tiles"),
            SpellEffect::Light { radius, seconds } => format!("Light of {radius:.0} tiles for {seconds:.0}s"),
            SpellEffect::Script { .. } => String::new(),
        };
        if !effect.is_empty() {
            draw_text(&effect, x, y, 18.0, skin::text(skin::Text::Dim, Color::new(0.85, 0.8, 0.6, 1.0)));
        }
        let b = Self::button_rect(r);
        let ready = known && self.world.mana >= d.mana as f32 && !self.skills.cooldown.contains_key(id) && self.spell_requirement(id).is_none();
        if !skin::button_picture(b, ready && b.contains(input::mouse_pos())) {
            draw_rectangle(b.x, b.y, b.w, b.h, if ready { Color::new(0.4, 0.5, 0.9, 0.35) } else { Color::new(1.0, 1.0, 1.0, 0.06) });
            draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.5, if ready { Color::new(0.6, 0.7, 1.0, 1.0) } else { skin::disabled() });
        }
        let label = if !known { "Not learned" } else if self.world.mana < d.mana as f32 { "Not enough mana" } else { "Cast  (Enter)" };
        draw_text(label, b.x + 12.0, b.y + 21.0, 18.0, if ready { skin::normal() } else { skin::disabled() });
        let mana = format!("Mana {:.0} / {}", self.world.mana, self.max_mana());
        draw_text(&mana, b.x + b.w + 16.0, b.y + 21.0, 18.0, Color::new(0.6, 0.7, 1.0, 1.0));
    }

    /// The Skills tab: the hero's ability scores with modifiers and saving throws, the 18 skills
    /// with their bonuses (proficient ones marked), the proficiency bonus and other proficiencies.
    fn draw_srd_page(&self) {
        let r = self.skills_rect();
        let w = &self.world;
        let prof = abilities::proficiency_bonus(w.player_level());
        let head = format!("Level {}   -   Proficiency bonus {}", w.player_level(), abilities::signed(prof as i64));
        draw_text(&head, r.x + PAD, r.y + 32.0, 22.0, skin::dim());
        super::glyphs::draw_hint_in("{key:next_tab}: next tab   {key:skills} / {key:cancel}: close", r.x + PAD, r.y + r.h - 14.0, 16.0, r.w - 2.0 * PAD, skin::disabled());
        let gold = skin::highlight();
        // Abilities: score, modifier, save.
        let (ax, mut y) = (r.x + PAD, r.y + LIST_TOP + 18.0);
        draw_text("Ability", ax, y, 17.0, skin::disabled());
        draw_text("Saving throw", ax + 190.0, y, 17.0, skin::disabled());
        for a in Ability::ALL {
            y += 34.0;
            let score = w.abilities.score(a);
            draw_text(a.short(), ax, y, 22.0, gold);
            draw_text(format!("{score}"), ax + 56.0, y, 22.0, skin::normal());
            draw_text(format!("({})", abilities::signed(abilities::modifier(score) as i64)), ax + 92.0, y, 20.0, skin::dim());
            let save = w.check_bonus(a.id(), true).unwrap_or(0);
            let proficient = w.is_proficient(&format!("save:{}", a.id()));
            draw_text(abilities::signed(save), ax + 190.0, y, 20.0, if proficient { gold } else { skin::dim() });
            if proficient {
                draw_circle(ax + 180.0, y - 6.0, 3.5, gold);
            }
        }
        // Passive Perception and the other proficiencies under the abilities.
        y += 44.0;
        let passive = 10 + w.check_bonus("perception", false).unwrap_or(0);
        draw_text(format!("Passive Perception {passive}"), ax, y, 18.0, skin::normal());
        // (Weapon and armor proficiencies show on the character sheet.)
        let others: Vec<String> = w.proficiencies.iter().filter(|p| abilities::skill(p).is_none() && abilities::gear(p).is_none() && !p.starts_with("save:")).map(|p| abilities::proficiency_name(p)).collect();
        if !others.is_empty() {
            y += 30.0;
            draw_text("Tools and other proficiencies", ax, y, 17.0, skin::disabled());
            for line in wrap_text(&others.join(", "), 18, LIST_W - PAD) {
                y += 22.0;
                draw_text(&line, ax, y, 18.0, skin::normal());
            }
        }
        // Skills: one row each, the bonus, the ability and the name (a dot = proficient).
        draw_line(r.x + LIST_W + 8.0, r.y + LIST_TOP, r.x + LIST_W + 8.0, r.y + r.h - 40.0, 1.0, skin::line(0.4));
        let cx = r.x + LIST_W + 30.0;
        let step = ((r.h - LIST_TOP - 60.0) / SKILLS.len() as f32).clamp(20.0, 28.0);
        for (i, (id, name, a)) in SKILLS.iter().enumerate() {
            let cy = r.y + LIST_TOP + 18.0 + i as f32 * step;
            let proficient = w.is_proficient(id);
            let bonus = w.check_bonus(id, false).unwrap_or(0);
            if proficient {
                draw_circle(cx + 4.0, cy - 6.0, 3.5, gold);
            }
            draw_text(abilities::signed(bonus), cx + 14.0, cy, 19.0, if proficient { gold } else { skin::dim() });
            draw_text(a.short(), cx + 56.0, cy, 15.0, skin::disabled());
            draw_text(name, cx + 100.0, cy, 19.0, skin::normal());
        }
    }

    /// For automated play-throughs.
    pub fn skills_status(&self) -> String {
        format!(
            "spells={:?} mana={:.0}/{} light={} aiming={:?}",
            self.world.spells,
            self.world.mana,
            self.max_mana(),
            self.skills.light.is_some(),
            self.skills.aiming
        )
    }
}
