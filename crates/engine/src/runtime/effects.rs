//! Conditions at runtime (until 0.27 status effects): applying them on the player and on
//! characters (after their saving throw, unless they are immune), removing them, ongoing damage
//! and healing every round (through resistances), what they do to moving and acting, and drawing
//! (tints, the HUD badges, badges over characters). The conditions and their rules are in
//! [`crate::srd::conditions`].

use gfx::prelude::*;

use super::text::{draw_text, measure_text};
use super::Game;
use crate::srd::conditions::{self, ActiveCondition, Condition, ConditionSpec};

impl Game {
    /// Applies a condition on `target`: "player", "self" (the trigger's owner) or a character's name.
    pub(super) fn apply_effect(&mut self, target: &str, owner: Option<u32>, spec: &ConditionSpec) {
        if target == "player" {
            self.apply_player_effect(spec);
        } else if let Some(i) = self.entity_index(target, owner) {
            self.apply_entity_effect(i, spec);
        } else {
            eprintln!("[events] Apply condition: no character '{target}' on this map");
        }
    }

    pub(super) fn apply_player_effect(&mut self, spec: &ConditionSpec) {
        if self.game_over.is_some() || spec.is_nothing() {
            return;
        }
        // A class feature makes the hero immune (Purity of Body: poisoned; Aura of Courage).
        if let (Some(c), Some(t)) = (spec.condition, self.hero_traits()) {
            if t.condition_immunities.iter().any(|i| i == c.id()) || t.aura.as_ref().is_some_and(|a| a.2 == c.id()) {
                return;
            }
        }
        // The saving throw keeps it off (the roll shows on screen and in the journal).
        if let Some(sv) = &spec.save {
            // (Against being poisoned: Antitoxin and a dwarf's resilience give advantage.)
            let mode = if spec.condition == Some(Condition::Poisoned) || spec.ongoing.as_ref().is_some_and(|o| o.damage_type == Some(crate::srd::DamageType::Poison)) { self.poison_save_mode(super::turns::Who::Player) } else { crate::srd::Mode::Normal };
            if self.world.check(&sv.ability, sv.dc, mode, true).is_some_and(|r| r.success == Some(true)) {
                return;
            }
        }
        if let Some(spec) = self.effect_hook("player", spec) {
            conditions::apply(&mut self.world.effects, &spec);
            if conditions::rules(&self.world.effects).dead {
                self.world.player_hp = Some(0);
                self.player_defeated();
            }
        }
    }

    /// Applies a condition on character `i`: its stat block's condition immunities and saving
    /// throw first (without a stat block: every score 10, no proficiency).
    pub(super) fn apply_entity_effect(&mut self, i: usize, spec: &ConditionSpec) {
        if spec.is_nothing() {
            return;
        }
        let e = &self.loaded.entities[i];
        let who = if e.name.is_empty() { self.project.db.character(&e.def).map_or(e.def.clone(), |d| d.name.clone()) } else { e.name.clone() };
        let block = self.stat_block(e).cloned().unwrap_or_default();
        if spec.condition.is_some_and(|c| block.immune_to(c)) && spec.ongoing.is_none() {
            return;
        }
        if let Some(sv) = &spec.save {
            if let Some(mut rec) = block.save(&sv.ability, sv.dc) {
                rec.label = format!("{who}: {}", rec.label);
                let saved = rec.success == Some(true);
                self.world.log_roll(rec);
                if saved {
                    return;
                }
            }
        }
        let target = self.loaded.entities[i].name.clone();
        let Some(mut spec) = self.effect_hook(&target, spec) else { return };
        if spec.condition.is_some_and(|c| block.immune_to(c)) {
            spec.condition = None;
        }
        let dead = match self.loaded.entities.get_mut(i) {
            Some(e) => {
                conditions::apply(&mut e.effects, &spec);
                conditions::rules(&e.effects).dead && e.max_hp > 0
            }
            None => false,
        };
        if dead {
            self.loaded.entities[i].hp = 0;
            self.defeat(i);
        }
    }

    /// The `effect_apply` script hook: the condition as it starts (`condition`, `seconds`), or
    /// None when a hook cancelled it.
    fn effect_hook(&mut self, target: &str, spec: &ConditionSpec) -> Option<ConditionSpec> {
        if !self.hooked("effect_apply") {
            return Some(spec.clone());
        }
        use crate::hooks::{flag, map, num, text};
        let id = spec.condition.map_or("", |c| c.id());
        let m = self.hook("effect_apply", map([("target", target.into()), ("condition", id.into()), ("effect", id.into()), ("seconds", (spec.seconds as f64).into()), ("cancel", false.into())]));
        if flag(&m, "cancel") {
            return None;
        }
        let condition = match text(&m, "condition", id) {
            c if c == id => spec.condition,
            c => Condition::parse(&c).filter(|c| *c != Condition::Any),
        };
        Some(ConditionSpec { condition, seconds: num(&m, "seconds", spec.seconds as f64).max(0.0) as f32, ..spec.clone() })
    }

    /// Removes a condition (`None` / `Any` = all, ongoing damage too) from `target`.
    pub(super) fn cure_effect(&mut self, target: &str, owner: Option<u32>, kind: Option<Condition>) {
        let kind = kind.unwrap_or(Condition::Any);
        if target == "player" {
            conditions::remove(&mut self.world.effects, kind);
        } else if let Some(i) = self.entity_index(target, owner) {
            conditions::remove(&mut self.loaded.entities[i].effects, kind);
        } else {
            eprintln!("[events] Remove condition: no character '{target}' on this map");
        }
    }

    /// Once per frame (not while the game is frozen): conditions run down; ongoing damage hurts
    /// (through resistances) and ongoing healing heals, once a round.
    pub(super) fn tick_effects(&mut self, dt: f32) {
        if !self.world.effects.is_empty() {
            for (amount, kind) in conditions::tick(&mut self.world.effects, dt) {
                match kind {
                    Some(t) => {
                        let n = self.player_defenses().apply(amount, Some(t));
                        self.effect_damage(n);
                    }
                    None if self.combat.enabled() => {
                        let max = self.player_max_hp();
                        self.world.player_hp = Some((self.hp() + amount).min(max));
                    }
                    None => {}
                }
            }
        }
        for i in 0..self.loaded.entities.len() {
            if self.loaded.entities[i].effects.is_empty() {
                continue;
            }
            let ticks = conditions::tick(&mut self.loaded.entities[i].effects, dt);
            if ticks.is_empty() {
                continue;
            }
            let defenses = self.stat_block(&self.loaded.entities[i]).map(|s| s.defenses()).unwrap_or_default();
            for (amount, kind) in ticks {
                let e = &mut self.loaded.entities[i];
                if !e.active || e.max_hp <= 0 || e.hp <= 0 {
                    break;
                }
                let Some(t) = kind else {
                    e.hp = (e.hp + amount).min(e.max_hp);
                    continue;
                };
                let n = defenses.apply(amount, Some(t));
                if n <= 0 {
                    continue;
                }
                let before = e.hp;
                e.hp -= n;
                e.flash = 0.15;
                if e.bar <= 0.0 {
                    e.bar_lag = before.max(0) as f32 / e.max_hp as f32;
                }
                e.bar = super::juice::BAR_TIME;
                let kind = if e.companion.is_some() { super::juice::NumberKind::Hurt } else { super::juice::NumberKind::Hit };
                let head = (e.body.pos, self.ts() * 1.1);
                self.damage_number(head, n, kind);
                let e = &self.loaded.entities[i];
                if e.hp <= 0 {
                    self.defeat(i);
                    break;
                } else {
                    self.hp_dropped(i, before);
                }
            }
        }
    }

    /// Ongoing damage: no defense, no knockback, no invulnerability.
    fn effect_damage(&mut self, n: i32) {
        if n <= 0 || !self.combat.enabled() || self.game_over.is_some() {
            return;
        }
        let hp = (self.hp() - n).max(0);
        self.world.player_hp = Some(hp);
        let head = (self.player.pos, self.ts() * 1.1);
        self.damage_number(head, n, super::juice::NumberKind::Hurt);
        if hp == 0 {
            self.player_defeated();
        }
    }

    /// The hero can't act (incapacitated, stunned, paralyzed, petrified, unconscious): no moving,
    /// attacking or using things.
    pub(super) fn player_stunned(&self) -> bool {
        !conditions::can_act(&self.world.effects)
    }

    /// Movement speed factor of character `i` (0 = it can't move).
    pub(super) fn npc_speed_factor(&self, i: usize) -> f32 {
        let fx = &self.loaded.entities[i].effects;
        if fx.is_empty() {
            1.0
        } else {
            conditions::rules(fx).speed
        }
    }

    /// What a hostile character's touch gives: its definition's, or its melee weapon's.
    pub(super) fn touch_effect(&self, i: usize) -> Option<ConditionSpec> {
        let e = &self.loaded.entities[i];
        self.project
            .db
            .character(&e.def)
            .and_then(|d| d.touch_effect.clone())
            .or_else(|| e.weapon.as_ref().filter(|w| w.ranged.is_none()).and_then(|w| w.effect.clone()))
    }

    /// The tint of someone with these conditions (None = normal colors).
    pub(super) fn effect_tint(list: &[ActiveCondition]) -> Option<Color> {
        if list.is_empty() {
            return None;
        }
        let on = |c| conditions::has(list, c);
        if on(Condition::Petrified) {
            return Some(Color::new(0.62, 0.6, 0.56, 1.0));
        }
        if on(Condition::Stunned) || on(Condition::Paralyzed) {
            return Some(Color::new(0.7, 0.7, 0.75, 1.0));
        }
        if list.iter().any(|a| a.ongoing.as_ref().is_some_and(|o| o.damage_type == Some(crate::srd::DamageType::Fire))) {
            return Some(Color::new(1.0, 0.7, 0.45, 1.0));
        }
        if on(Condition::Poisoned) {
            return Some(Color::new(0.6, 1.0, 0.55, 1.0));
        }
        None
    }

    /// The see-through-ness of someone with these conditions (invisible: faint).
    pub(super) fn effect_alpha(list: &[ActiveCondition]) -> f32 {
        if conditions::has(list, Condition::Invisible) {
            0.3
        } else {
            1.0
        }
    }

    /// Someone prone or unconscious: its model lies on the ground (as a downed companion).
    pub(super) fn draw_lying(&self, a: &crate::defs::Appearance, pos: Vec2, tint: Color) -> bool {
        if !a.has_look() {
            return false;
        }
        let (_, fh) = self.character_frame_size(a);
        let u = self.u();
        draw_ellipse(pos.x, pos.y - 2.0 * u, fh * 0.45, 4.0 * u, 0.0, Color::new(0.0, 0.0, 0.0, 0.3 * tint.a));
        match self.model_cell(a, &super::paperdoll::Gear::default(), (crate::entity::Facing::Down, super::paperdoll::Anim::Idle(0))) {
            Some(cell) => {
                self.assets.models.draw_rotated(&cell, pos - vec2(0.0, 6.0 * u), -std::f32::consts::FRAC_PI_2, self.look_size(a), tint);
                true
            }
            None => false,
        }
    }

    /// A blinded hero sees only a small circle around them (screen space, over the world): a
    /// mesh from clear in the middle to black, then black to beyond the screen's corners.
    pub(super) fn draw_blinded(&self) {
        if self.world.effects.is_empty() || !conditions::has(&self.world.effects, Condition::Blinded) || self.on_title() {
            return;
        }
        let c = self.view().above(self.player.pos, self.player.hitbox.y);
        let scale = self.view_scale();
        let (r0, r1) = (45.0 * scale, 150.0 * scale);
        let far = (screen_width() + screen_height()) * 2.0;
        let dark = 0.97;
        let rings = [(r0, 0.0), (r1, dark), (far, dark)];
        let n = 64;
        let mut vertices = Vec::new();
        for (r, a) in rings {
            for k in 0..n {
                let t = k as f32 / n as f32 * std::f32::consts::TAU;
                let color = [0, 0, 0, (a * 255.0) as u8];
                vertices.push(gfx::models::Vertex { position: vec3(c.x + t.cos() * r, c.y + t.sin() * r, 0.0), uv: vec2(0.0, 0.0), color, normal: vec4(0.0, 0.0, 1.0, 0.0) });
            }
        }
        let mut indices = Vec::new();
        for ring in 0..2u16 {
            for k in 0..n as u16 {
                let (a, b) = (ring * n as u16 + k, ring * n as u16 + (k + 1) % n as u16);
                let (c2, d) = (a + n as u16, b + n as u16);
                indices.extend_from_slice(&[a, b, c2, b, d, c2]);
            }
        }
        draw_mesh(&gfx::models::Mesh { vertices, indices, texture: None });
    }

    /// The player's conditions as a row of badges (label and seconds left) from `(x, y)`;
    /// returns the row's height (0 = none).
    pub(super) fn draw_effect_badges(&self, x: f32, y: f32) -> f32 {
        let mut bx = x;
        for a in &self.world.effects {
            let text = a.badge();
            let w = measure_text(&text, None, 14, 1.0).width + 8.0;
            let [r, g, b] = a.color();
            let c = Color::from_rgba(r, g, b, 255);
            draw_rectangle(bx, y, w, 16.0, Color::new(c.r * 0.45, c.g * 0.45, c.b * 0.45, 0.95));
            draw_rectangle_lines(bx, y, w, 16.0, 1.0, c);
            draw_text(&text, bx + 4.0, y + 12.0, 14.0, WHITE);
            bx += w + 4.0;
        }
        if self.world.effects.is_empty() {
            0.0
        } else {
            20.0
        }
    }

    /// Width of the badge row (for the status frame's size).
    pub(super) fn effect_badges_width(&self) -> f32 {
        self.world.effects.iter().map(|a| measure_text(&a.badge(), None, 14, 1.0).width + 12.0).sum()
    }

    /// Badges over a character with conditions (world space): their short names, side by side.
    pub(super) fn draw_npc_effects(&self, center_x: f32, bottom_y: f32, list: &[ActiveCondition]) {
        let _font = super::text::role(super::text::Role::Hud);
        let u = self.u();
        let size = 5.0 * u;
        let labels: Vec<(String, [u8; 3])> = list.iter().map(|a| (a.badge().split(' ').next().unwrap_or("").to_string(), a.color())).collect();
        let widths: Vec<f32> = labels.iter().map(|(t, _)| super::text::measure_plain(t, size).width + 2.0 * u).collect();
        let total = widths.iter().sum::<f32>() + u * (labels.len().saturating_sub(1)) as f32;
        let mut x = center_x - total / 2.0;
        for ((t, [r, g, b]), w) in labels.iter().zip(widths) {
            let c = Color::from_rgba(*r, *g, *b, 255);
            draw_rectangle(x, bottom_y - size - u, w, size + u, Color::new(c.r * 0.35, c.g * 0.35, c.b * 0.35, 0.85));
            super::text::draw_plain(t, x + u, bottom_y - 1.5 * u, size, c);
            x += w + u;
        }
    }

    /// For `status()`: the player's conditions and the characters with any.
    pub(super) fn effects_status(&self) -> String {
        let npcs: Vec<String> = self
            .loaded
            .entities
            .iter()
            .filter(|e| e.active && !e.effects.is_empty())
            .map(|e| format!("{}:{}", if e.name.is_empty() { &e.def } else { &e.name }, conditions::describe(&e.effects).join("+")))
            .collect();
        format!("effects={:?} npc_effects={:?}", conditions::describe(&self.world.effects), npcs)
    }
}
