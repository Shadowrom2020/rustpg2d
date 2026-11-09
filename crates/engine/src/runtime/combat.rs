//! Real-time combat: player attacks, hostile characters, knockback, defeat and game over.

use macroquad::prelude::*;

use super::{input, Arrival, Game, Mover, Source};
use crate::defs::{CharacterDef, Ranged, SpriteRef};
use crate::entity::Facing;
use crate::events::GameEvent;
use crate::state::WorldState;
use super::text::{draw_text, measure_text};

const SWING_TIME: f32 = 0.18;
/// Enemy invulnerability after a hit.
const HIT_INVULN: f32 = 0.3;
/// Player invulnerability after being hurt.
const HURT_INVULN: f32 = 1.0;
const KNOCKBACK: f32 = 190.0;
const GAME_OVER_TIME: f32 = 2.5;
const EFFECT_TIME: f32 = 0.35;

/// A flying arrow, bolt, ...
pub(super) struct Projectile {
    pos: Vec2,
    vel: Vec2,
    dist_left: f32,
    traveled: f32,
    damage: i32,
    from_player: bool,
    /// Entity that fired it (never hit by its own shot).
    shooter: Option<u32>,
    sprite: SpriteRef,
    /// A status effect for whoever it hits.
    effect: Option<crate::effects::EffectSpec>,
}

pub(super) struct PlayerCombat {
    /// 0 disables combat (no HUD, no attacking, can't be hurt).
    pub max_hp: i32,
    /// Bare-handed damage.
    pub damage: i32,
    /// Base defense (armor adds to it).
    pub defense: i32,
    pub cooldown: f32,
    pub swing: f32,
    pub invuln: f32,
    pub knock: Vec2,
    /// Last melee swing: direction, reach (px) and half-angle (radians), for drawing.
    swing_dir: Vec2,
    swing_reach: f32,
    swing_arc: f32,
}

impl PlayerCombat {
    pub fn new(def: Option<&CharacterDef>) -> Self {
        let (max_hp, damage, defense) = def.map_or((0, 0, 0), |d| (d.max_hp.max(0), d.damage.max(0), d.defense));
        PlayerCombat {
            max_hp,
            damage,
            defense,
            cooldown: 0.0,
            swing: 0.0,
            invuln: 0.0,
            knock: Vec2::ZERO,
            swing_dir: vec2(0.0, 1.0),
            swing_reach: 16.0,
            swing_arc: 1.0,
        }
    }
    pub fn enabled(&self) -> bool {
        self.max_hp > 0
    }
    pub fn reset(&mut self) {
        self.cooldown = 0.0;
        self.swing = 0.0;
        self.invuln = 0.0;
        self.knock = Vec2::ZERO;
    }
}

fn inflate(r: Rect, by: f32) -> Rect {
    Rect::new(r.x - by, r.y - by, r.w + 2.0 * by, r.h + 2.0 * by)
}

/// Decays a knockback velocity; returns the movement for this frame (`u` = art pixel size).
fn decay(knock: &mut Vec2, dt: f32, u: f32) -> Vec2 {
    let step = *knock * dt;
    *knock *= (1.0 - 9.0 * dt).max(0.0);
    if knock.length() < 8.0 * u {
        *knock = Vec2::ZERO;
    }
    step
}

impl Game {
    pub(super) fn hp(&self) -> i32 {
        self.world.player_hp.unwrap_or(self.player_max_hp())
    }

    /// Advances the game-over screen; returns true while it is showing. Once the red fade is
    /// done the defeat menu opens (continue from the last save, start over, quit to the main
    /// menu); the screen stays red behind it.
    pub(super) fn update_game_over(&mut self, dt: f32) -> bool {
        let Some(t) = &mut self.game_over else { return false };
        *t += dt;
        if *t >= GAME_OVER_TIME {
            *t = GAME_OVER_TIME;
            self.open_defeat_menu();
        }
        true
    }

    /// The defeat menu's *Start over*: a new game from the start, as the same hero (name, sex
    /// and look stay).
    pub(super) fn start_over(&mut self) {
        let (name, sex, look) = (self.world.player_name.clone(), self.world.player_sex, self.world.player_appearance.clone());
        self.new_game();
        self.world.player_name = name;
        self.world.player_sex = sex;
        self.world.player_appearance = look;
    }

    /// Starts over with a fresh world at the project's start map / spawn.
    pub(super) fn new_game(&mut self) {
        self.reset_world();
        self.ui_hidden = false;
        self.reset_cutscene();
        let (map, spawn) = (self.project.config.start_map.clone(), self.project.config.start_spawn.clone());
        self.transition(&map, Arrival::Spawn(spawn));
    }

    /// A fresh world (the start map is entered by the caller).
    pub(super) fn reset_world(&mut self) {
        self.world = WorldState { player_hp: Some(self.combat.max_hp), hour: self.project.config.start_hour, ..Default::default() };
        self.clock_seen = None;
        self.slot = None;
        self.order = None;
        self.game_over = None;
        self.autosave_pending = false;
        self.prepare_world();
        self.inv = Default::default();
        self.combat.reset();
        self.runner.clear();
        self.ui = super::Ui::default();
        self.cache.clear();
        self.loaded.map.name.clear();
    }

    pub(super) fn update_combat(&mut self, dt: f32, frozen: bool, key_consumed: bool) {
        // (Timers stop at 0 instead of counting down forever.)
        let c = &mut self.combat;
        c.cooldown = (c.cooldown - dt).max(0.0);
        c.swing = (c.swing - dt).max(0.0);
        c.invuln = (c.invuln - dt).max(0.0);
        for e in &mut self.loaded.entities {
            e.invuln = (e.invuln - dt).max(0.0);
            e.flash = (e.flash - dt).max(0.0);
        }
        for fx in &mut self.effects {
            fx.1 += dt;
        }
        self.effects.retain(|fx| fx.1 < EFFECT_TIME);
        if !self.combat.enabled() || frozen {
            return;
        }
        if !key_consumed && input::act_pressed(input::Act::SwitchWeapon) {
            self.cycle_weapon();
        }
        if !key_consumed && self.combat.cooldown <= 0.0 && input::act_pressed(input::Act::Attack) {
            self.order = None;
            self.attack_at(None);
        }
        self.enemy_shooting(dt);
        self.companion_contact_damage();
        // Contact damage from hostile characters.
        if self.combat.invuln <= 0.0 {
            let me = inflate(self.player.rect(), 1.0);
            // Stunned characters don't hurt; strength / weakness change the damage.
            let hit = self
                .loaded
                .entities
                .iter()
                .position(|e| e.active && e.hostile && e.damage > 0 && !e.ai.harmless() && !crate::effects::stunned(&e.effects) && e.body.rect().overlaps(&me));
            if let Some(i) = hit {
                let e = &self.loaded.entities[i];
                let (dmg, from) = (crate::effects::scale_damage(&e.effects, e.damage), e.body.pos);
                if let Some(fx) = self.touch_effect(i) {
                    self.apply_player_effect(&fx);
                }
                self.hurt_player(dmg, from);
            }
        }
    }

    /// Attacks with the equipped weapon (or bare hands). `aim` (world point) sets the
    /// direction; otherwise the player's facing is used.
    pub(super) fn attack_at(&mut self, aim: Option<Vec2>) {
        // Both hands on the rungs.
        if !self.combat.enabled() || self.combat.cooldown > 0.0 || self.player_stunned() || self.climbing() {
            return;
        }
        let weapon = self.attack_weapon();
        let damage = weapon.damage;
        if damage <= 0 {
            return;
        }
        let cooldown = weapon.cooldown.max(0.05);
        if let Some(r) = weapon.ranged.clone() {
            if !r.ammo.is_empty() {
                if self.world.item_count(&r.ammo) == 0 {
                    let ammo = self.project.db.item(&r.ammo).map_or(r.ammo.clone(), |d| d.name.clone());
                    self.ui.toast(format!("Out of {ammo}!"));
                    self.combat.cooldown = 0.5;
                    return;
                }
                self.world.take_item(&r.ammo, 1);
            }
            let origin = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0 + 2.0);
            let dir = match aim {
                Some(t) => (t - origin).try_normalize().unwrap_or(vec2(1.0, 0.0)),
                None => Vec2::from(self.player.facing.dir()),
            };
            self.combat.cooldown = cooldown;
            self.audio.play(&self.project.config.sfx.attack);
            self.shoot(origin, dir, &r, damage, true, None, weapon.effect);
            return;
        }
        // Melee: everything within reach inside the swing arc.
        let ts = self.ts();
        let center = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        let dir = aim
            .and_then(|t| (t - center).try_normalize())
            .unwrap_or_else(|| Vec2::from(self.player.facing.dir()));
        if let Some(f) = Facing::from_vec(dir.x, dir.y) {
            self.player.facing = f;
        }
        let reach = weapon.reach.max(0.3) * ts;
        let arc = weapon.arc.clamp(5.0, 180.0).to_radians();
        self.combat.cooldown = cooldown;
        self.combat.swing = SWING_TIME;
        self.combat.swing_dir = dir;
        self.combat.swing_reach = reach;
        self.combat.swing_arc = arc;
        self.audio.play(&self.project.config.sfx.attack);
        let targets: Vec<usize> = (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                // Only hostile characters (never companions or other friendly characters).
                if !(e.active && e.hostile && e.max_hp > 0 && e.invuln <= 0.0) {
                    return false;
                }
                let to = (e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0)) - center;
                let radius = e.body.hitbox.x.max(e.body.hitbox.y) / 2.0 + 2.0;
                // Touching enemies are always hit; others must be in range and inside the arc.
                to.length() < ts * 0.5 || (to.length() - radius <= reach && to.angle_between(dir).abs() <= arc)
            })
            .collect();
        let from = self.player.pos;
        // No enemy in the swing: a gathering prop in it takes a hit (veins, trees).
        if targets.is_empty() {
            let prop = (0..self.loaded.entities.len()).filter(|&i| self.loaded.entities[i].kind == crate::entity::EntityKind::Prop && self.loaded.entities[i].active).find(|&i| {
                let e = &self.loaded.entities[i];
                let to = (e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0)) - center;
                let radius = e.body.hitbox.x.max(e.body.hitbox.y) / 2.0 + 2.0;
                self.project.db.prop(&e.def).is_some_and(|d| d.gather.is_some()) && (to.length() < ts * 0.5 || (to.length() - radius <= reach && to.angle_between(dir).abs() <= arc))
            });
            if let Some(i) = prop {
                self.gather_hit(i);
            }
        }
        for i in targets {
            if let Some(fx) = &weapon.effect {
                crate::effects::apply(&mut self.loaded.entities[i].effects, fx);
            }
            self.damage_entity(i, damage, from, weapon.knockback);
        }
    }

    /// Whether a melee swing with the current weapon would reach entity `i` (for mouse attacks).
    pub(super) fn melee_reaches(&self, i: usize) -> bool {
        let ts = self.ts();
        let w = self.attack_weapon();
        let e = &self.loaded.entities[i];
        let center = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        let to = (e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0)) - center;
        let radius = e.body.hitbox.x.max(e.body.hitbox.y) / 2.0 + 2.0;
        to.length() - radius <= w.reach.max(0.3) * ts * 0.9
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn shoot(&mut self, origin: Vec2, dir: Vec2, r: &Ranged, damage: i32, from_player: bool, shooter: Option<u32>, effect: Option<crate::effects::EffectSpec>) {
        self.projectiles.push(Projectile {
            pos: origin + dir * 4.0,
            vel: dir * r.speed.max(10.0),
            dist_left: r.range.max(0.5) * self.ts(),
            traveled: 0.0,
            damage,
            from_player,
            shooter,
            sprite: r.projectile.clone(),
            effect,
        });
    }

    /// Hostile characters with a ranged weapon shoot when the player is in range and in sight.
    fn enemy_shooting(&mut self, dt: f32) {
        let ts = self.ts();
        let player = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        // The player and companions that can be hurt: each shooter aims at the nearest in sight.
        let mut aims = vec![player];
        aims.extend(self.companion_targets().into_iter().map(|(_, p)| p));
        let mut shots = Vec::new();
        for i in 0..self.loaded.entities.len() {
            let e = &mut self.loaded.entities[i];
            if !e.active || !e.hostile || e.ai.harmless() || crate::effects::stunned(&e.effects) {
                continue;
            }
            let Some(w) = &e.weapon else { continue };
            let Some(r) = &w.ranged else { continue };
            e.attack_cd -= dt;
            if e.attack_cd > 0.0 || e.knock != Vec2::ZERO {
                continue;
            }
            let origin = e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0 + 2.0);
            let range = r.range * ts;
            let (r, dmg, uid, cd, fx) = (r.clone(), crate::effects::scale_damage(&e.effects, w.damage), e.uid, w.cooldown, w.effect);
            let mut near: Vec<Vec2> = aims.iter().copied().filter(|a| origin.distance(*a) <= range).collect();
            near.sort_by(|a, b| origin.distance(*a).total_cmp(&origin.distance(*b)));
            let Some(target) = near.into_iter().find(|a| self.line_clear(origin, *a)) else { continue };
            let e = &mut self.loaded.entities[i];
            e.attack_cd = cd.max(0.3) + rand::gen_range(0.0, 0.6);
            let dir = (target - origin).normalize_or_zero();
            e.body.facing = Facing::from_vec(dir.x, dir.y).unwrap_or(e.body.facing);
            shots.push((origin, dir, r, dmg, uid, fx));
        }
        for (origin, dir, r, dmg, uid, fx) in shots {
            self.audio.play(&self.project.config.sfx.attack);
            self.shoot(origin, dir, &r, dmg, false, Some(uid), fx);
        }
    }

    /// Moves projectiles in small steps; they stop at walls and hit the first target.
    pub(super) fn update_projectiles(&mut self, dt: f32) {
        let ts = self.ts();
        let companions = self.companion_targets();
        let mut i = 0;
        while i < self.projectiles.len() {
            let p = &self.projectiles[i];
            let travel = p.vel.length() * dt;
            let steps = (travel / 3.0).ceil().max(1.0) as usize;
            let step = p.vel * dt / steps as f32;
            let mut alive = true;
            for _ in 0..steps {
                let p = &mut self.projectiles[i];
                p.pos += step;
                p.dist_left -= step.length();
                p.traveled += step.length();
                let pos = p.pos;
                if p.dist_left <= 0.0 {
                    alive = false;
                    break;
                }
                let hit_box = Rect::new(pos.x - 2.0, pos.y - 2.0, 4.0, 4.0);
                let p = &self.projectiles[i];
                if p.from_player {
                    let target = self.loaded.entities.iter().position(|e| {
                        e.active && e.hostile && e.max_hp > 0 && e.invuln <= 0.0 && Some(e.uid) != p.shooter && {
                            // Characters are hit anywhere on their body, not just the feet.
                            let r = e.body.rect();
                            Rect::new(r.x, r.y - ts * 0.6, r.w, r.h + ts * 0.6).overlaps(&hit_box)
                        }
                    });
                    if let Some(t) = target {
                        let (dmg, from, fx) = (p.damage, pos - p.vel.normalize_or_zero() * 8.0, p.effect);
                        if let Some(fx) = fx {
                            crate::effects::apply(&mut self.loaded.entities[t].effects, &fx);
                        }
                        self.damage_entity(t, dmg, from, 1.0);
                        alive = false;
                        break;
                    }
                } else {
                    // Companions are hit like the player.
                    let companion = companions.iter().map(|(c, _)| *c).find(|&c| {
                        let r = self.loaded.entities[c].body.rect();
                        self.loaded.entities[c].invuln <= 0.0 && Rect::new(r.x, r.y - ts * 0.8, r.w, r.h + ts * 0.8).overlaps(&hit_box)
                    });
                    if let Some(c) = companion {
                        let (dmg, from, fx) = (p.damage, pos - p.vel.normalize_or_zero() * 8.0, p.effect);
                        if let Some(fx) = fx {
                            crate::effects::apply(&mut self.loaded.entities[c].effects, &fx);
                        }
                        self.damage_entity(c, dmg, from, 1.0);
                        alive = false;
                        break;
                    }
                    let r = self.player.rect();
                    let body = Rect::new(r.x, r.y - ts * 0.8, r.w, r.h + ts * 0.8);
                    if body.overlaps(&hit_box) {
                        if self.combat.invuln <= 0.0 {
                            let (dmg, from, fx) = (p.damage, pos - p.vel.normalize_or_zero() * 8.0, p.effect);
                            if let Some(fx) = fx {
                                self.apply_player_effect(&fx);
                            }
                            self.hurt_player(dmg, from);
                        }
                        alive = false;
                        break;
                    }
                }
                // Walls are checked after targets, so characters standing on a solid tile can still be hit.
                let leaving = self.projectiles[i].traveled < ts * 0.5; // shooter may stand on a solid tile
                let tile = ((pos.x / ts).floor() as i32, (pos.y / ts).floor() as i32);
                // Characters stuck inside a solid tile can still be hit.
                let occupied = || {
                    let tr = Rect::new(tile.0 as f32 * ts, tile.1 as f32 * ts, ts, ts);
                    self.loaded.entities.iter().any(|e| {
                        let r = e.body.rect();
                        e.active && e.max_hp > 0 && Rect::new(r.x, r.y - ts * 0.6, r.w, r.h + ts * 0.6).overlaps(&tr)
                    })
                };
                if !leaving && self.tile_solid(tile.0, tile.1) && !occupied() {
                    self.effects.push((pos, EFFECT_TIME * 0.6));
                    alive = false;
                    break;
                }
            }
            if alive {
                i += 1;
            } else {
                self.projectiles.swap_remove(i);
            }
        }
    }

    /// Applies damage (minus the target's defense, at least 1) with knockback.
    pub(super) fn damage_entity(&mut self, i: usize, dmg: i32, from: Vec2, knockback: f32) {
        let u = self.u();
        let e = &mut self.loaded.entities[i];
        let before = e.hp;
        let hit = (dmg - e.defense - crate::effects::defense_bonus(&e.effects)).max(1);
        e.hp -= hit;
        e.last_hit = hit;
        e.invuln = HIT_INVULN;
        e.flash = 0.15;
        e.body.move_target = None;
        let away = (e.body.pos - from).try_normalize().unwrap_or(vec2(0.0, 1.0));
        e.knock = away * KNOCKBACK * u * knockback.max(0.0);
        if e.hp <= 0 {
            self.defeat(i);
        } else {
            self.audio.play(&self.project.config.sfx.hit);
            self.hp_dropped(i, before);
        }
    }

    /// `DefeatEnemies`: defeats every living hostile character on the map. Returns how many.
    pub(super) fn defeat_enemies(&mut self) -> usize {
        let hostile: Vec<usize> = (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                e.active && e.hostile && e.max_hp > 0 && e.kind == crate::entity::EntityKind::Character
            })
            .collect();
        // A scripted defeat gives no experience: the player didn't fight.
        for &i in &hostile {
            self.loaded.entities[i].hp = 0;
            self.defeat_with(i, false);
        }
        hostile.len()
    }

    pub(super) fn defeat(&mut self, i: usize) {
        self.defeat_with(i, true);
    }

    /// Defeats character `i`; `xp`: the player gains its experience.
    fn defeat_with(&mut self, i: usize, xp: bool) {
        if self.loaded.entities[i].companion.is_some() {
            // Companions aren't defeated: they fall (and wait to be healed), or go home.
            self.knock_down(i);
            return;
        }
        if xp {
            let n = self.xp_of(&self.loaded.entities[i].def);
            self.give_xp(n as i64);
        }
        // The bestiary counts every defeated character kind.
        if self.loaded.entities[i].kind == crate::entity::EntityKind::Character {
            let def = self.loaded.entities[i].def.clone();
            *self.world.bestiary.entry(def).or_insert(0) += 1;
        }
        let e = &mut self.loaded.entities[i];
        e.active = false;
        e.knock = Vec2::ZERO;
        let (uid, pos, spawned_by) = (e.uid, e.body.pos - vec2(0.0, e.body.hitbox.y), e.spawned_by);
        let (feet, loot) = (e.body.pos, e.loot.clone());
        self.effects.push((pos, 0.0));
        self.persist(i);
        self.drop_loot(&loot, feet);
        self.drop_npc_items(i);
        if let Some(zone) = spawned_by {
            self.spawned_defeated(zone);
        }
        self.audio.play(&self.project.config.sfx.defeat);
        self.fire(GameEvent::Defeat, Source::Entity(uid));
    }

    /// Rolls a defeated character's loot table and puts what drops on the ground around
    /// `feet` (saved like items the player dropped).
    fn drop_loot(&mut self, loot: &[crate::defs::LootEntry], feet: Vec2) {
        let drops = crate::defs::roll_loot(loot, || rand::gen_range(0.0, 1.0), |lo, hi| rand::gen_range(lo, hi + 1));
        let u = self.u();
        // Several different items fan out a little so each can be seen and picked up.
        const SPOTS: [(f32, f32); 6] = [(0.0, 0.0), (10.0, 4.0), (-10.0, 4.0), (0.0, 9.0), (10.0, -5.0), (-10.0, -5.0)];
        for (k, (item, count)) in drops.into_iter().enumerate() {
            let (dx, dy) = SPOTS[k % SPOTS.len()];
            self.place_drop(&item, count, feet + vec2(dx, dy) * u);
        }
    }

    pub(super) fn hurt_player(&mut self, dmg: i32, from: Vec2) {
        if !self.combat.enabled() || self.game_over.is_some() {
            return;
        }
        // Armor absorbs damage, but at least 1 always gets through.
        let taken = (dmg - self.player_defense()).max(1);
        let hp = (self.hp() - taken).max(0);
        self.world.player_hp = Some(hp);
        self.combat.invuln = HURT_INVULN;
        let away = (self.player.pos - from).try_normalize().unwrap_or(vec2(0.0, 1.0));
        self.combat.knock = away * KNOCKBACK * self.u();
        self.audio.play(&self.project.config.sfx.hurt);
        self.hurt_feedback();
        if hp == 0 {
            self.player_defeated();
        }
    }

    /// The player's HP reached 0: the game-over screen, then the defeat menu.
    pub(super) fn player_defeated(&mut self) {
        self.game_over = Some(0.0);
        self.runner.clear();
        self.audio.stop_music();
    }

    /// Positive amounts heal, negative amounts hurt (without knockback).
    pub(super) fn heal(&mut self, amount: i32) {
        if !self.combat.enabled() {
            return;
        }
        if amount < 0 {
            let p = self.player.pos;
            self.hurt_player(-amount, p);
            self.combat.knock = Vec2::ZERO;
            return;
        }
        let hp = (self.hp() + amount).min(self.player_max_hp());
        self.world.player_hp = Some(hp);
        self.ui.toast(format!("+{amount} HP"));
    }

    /// Applies knockback to the player; returns true while it overrides input.
    pub(super) fn apply_player_knockback(&mut self, dt: f32) -> bool {
        if self.combat.knock == Vec2::ZERO {
            return false;
        }
        let u = self.u();
        let step = decay(&mut self.combat.knock, dt, u);
        self.player.pos = self.try_move(&self.player, step, Mover::Player, false);
        self.player.moving = false;
        true
    }

    pub(super) fn entity_knockback(&mut self, i: usize, dt: f32) -> bool {
        let u = self.u();
        let e = &mut self.loaded.entities[i];
        if e.knock == Vec2::ZERO {
            return false;
        }
        let step = decay(&mut e.knock, dt, u);
        let (body, uid) = (e.body.clone(), e.uid);
        let p = self.try_move(&body, step, Mover::Npc(uid), false);
        let e = &mut self.loaded.entities[i];
        e.body.pos = p;
        e.body.moving = false;
        true
    }

    // ------------------------------------------------------------ drawing

    /// World-space combat visuals: sword swing, enemy health bars, defeat puffs.
    pub(super) fn draw_combat_world(&self) {
        let ts = self.ts();
        let u = self.u();
        if self.combat.swing > 0.0 {
            // The swing shows the weapon's real reach and arc.
            let d = self.combat.swing_dir;
            let base = d.y.atan2(d.x);
            let arc = self.combat.swing_arc.min(1.4);
            let center = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0 + 2.0 * u);
            let p = 1.0 - self.combat.swing / SWING_TIME;
            let (a0, a1) = (base - arc, base - arc + 2.0 * arc * p);
            let steps = 8;
            let r = self.combat.swing_reach.max(ts * 0.6);
            for k in 0..steps {
                let t0 = a0 + (a1 - a0) * k as f32 / steps as f32;
                let t1 = a0 + (a1 - a0) * (k + 1) as f32 / steps as f32;
                let alpha = 0.3 + 0.7 * (k as f32 / steps as f32);
                draw_line(
                    center.x + t0.cos() * r,
                    center.y + t0.sin() * r,
                    center.x + t1.cos() * r,
                    center.y + t1.sin() * r,
                    2.0 * u,
                    Color::new(1.0, 1.0, 0.85, alpha),
                );
            }
            // The weapon itself (out of the hand meanwhile) sweeps along the arc. Item icons
            // point up-right, so they are turned by the arc's angle + 45 degrees.
            let icon = self.world.equipment.item(crate::defs::EquipSlot::Weapon).and_then(|id| self.project.db.item(id)).map(|d| d.icon.clone());
            if let Some(icon) = icon {
                let at = center + vec2(a1.cos(), a1.sin()) * (r * 0.5).max(ts * 0.45);
                self.assets.draw_frame_scaled(&icon.sheet, icon.frame, at, a1 + std::f32::consts::FRAC_PI_4, 1.0, WHITE);
            }
        }
        for e in self.loaded.entities.iter().filter(|e| e.active && e.max_hp > 0 && e.hp < e.max_hp && !e.companion.as_ref().is_some_and(|l| l.down.is_some())) {
            let w = 14.0 * u;
            let (x, y) = (e.body.pos.x - w / 2.0, e.body.pos.y - ts - 6.0 * u);
            draw_rectangle(x - u, y - u, w + 2.0 * u, 4.0 * u, Color::new(0.0, 0.0, 0.0, 0.7));
            draw_rectangle(x, y, w * e.hp as f32 / e.max_hp as f32, 2.0 * u, Color::new(0.9, 0.2, 0.2, 1.0));
        }
        // Status effects: small colored squares above the health bar.
        for e in self.loaded.entities.iter().filter(|e| e.active && !e.effects.is_empty()) {
            // Above the head of a full-height (1.5 tiles) character.
            self.draw_npc_effects(e.body.pos.x, e.body.pos.y - ts * 1.5 - 2.0 * u, &e.effects);
        }
        for p in &self.projectiles {
            let angle = p.vel.y.atan2(p.vel.x);
            if !self.assets.draw_frame_rotated(&p.sprite.sheet, p.sprite.frame, p.pos, angle, WHITE) {
                draw_line(p.pos.x, p.pos.y, p.pos.x - p.vel.x * 0.03, p.pos.y - p.vel.y * 0.03, 1.5 * u, WHITE);
            }
        }
        for (pos, age) in &self.effects {
            let t = age / EFFECT_TIME;
            let c = Color::new(1.0, 1.0, 1.0, 1.0 - t);
            draw_circle_lines(pos.x, pos.y, (3.0 + t * 10.0) * u, 1.5 * u, c);
            for k in 0..6 {
                let a = k as f32 * std::f32::consts::TAU / 6.0;
                let d = (4.0 + t * 12.0) * u;
                draw_rectangle(pos.x + a.cos() * d - u, pos.y + a.sin() * d - u, 2.0 * u, 2.0 * u, c);
            }
        }
    }

    /// Hearts in the top-left corner.
    /// The player status in the top-left corner: a framed panel with a small portrait, the
    /// hero's name, the hearts (1 heart = 2 HP), the mana bar (with magic) and the ammo of a
    /// ranged weapon.
    pub(super) fn draw_hud(&self) {
        if !self.combat.enabled() {
            return;
        }
        const HEART: f32 = 18.0;
        const STEP: f32 = 21.0;
        const PAD: f32 = 8.0;
        let (x0, y0) = (10.0, 10.0);
        let hp = self.hp();
        let hearts = (self.player_max_hp() + 1) / 2;
        let name = self.world.player_name.trim().to_string();
        let def = self.player_defense();
        let magic = self.magic_on();
        let levels = self.progression_on();
        let level_text = format!("Lv {}", self.world.player_level());
        let ammo = self.hud_ammo();
        // Sizes: the text column is as wide as its widest line.
        let portrait = Rect::new(x0 + PAD, y0 + PAD, 44.0, 60.0);
        let tx = portrait.right() + 10.0;
        let name_w = measure_text(&name, None, 22, 1.0).width;
        let def_text = (def > 0).then(|| format!("DEF {def}"));
        let def_w = def_text.as_ref().map_or(0.0, |t| measure_text(t, None, 16, 1.0).width + 14.0);
        let hearts_w = hearts as f32 * STEP;
        let bar_w = hearts_w.max(120.0);
        let level_w = measure_text(&level_text, None, 15, 1.0).width + 6.0;
        let badges_w = self.effect_badges_width();
        let col_w = (name_w + def_w)
            .max(hearts_w)
            .max(badges_w)
            .max(if magic { bar_w + 34.0 } else { 0.0 })
            .max(if levels { bar_w + level_w } else { 0.0 })
            .max(if ammo.is_some() { 80.0 } else { 0.0 });
        let mut h = 24.0 + HEART + 6.0;
        if magic {
            h += 16.0;
        }
        if levels {
            h += 14.0;
        }
        if !self.world.effects.is_empty() {
            h += 20.0;
        }
        if ammo.is_some() {
            h += 22.0;
        }
        let frame = Rect::new(x0, y0, tx + col_w + PAD + 2.0 - x0, h.max(portrait.h) + 2.0 * PAD);
        self.hud_bottom.set(frame.bottom());

        // Frame: dark backing, a gold border with an inner line and corner studs.
        let gold = Color::new(0.85, 0.72, 0.38, 1.0);
        let dim_gold = Color::new(0.55, 0.45, 0.22, 1.0);
        draw_rectangle(frame.x, frame.y, frame.w, frame.h, Color::new(0.05, 0.05, 0.1, 0.78));
        draw_rectangle_lines(frame.x, frame.y, frame.w, frame.h, 2.5, gold);
        draw_rectangle_lines(frame.x + 4.0, frame.y + 4.0, frame.w - 8.0, frame.h - 8.0, 1.0, dim_gold);
        for (cx, cy) in [(frame.x, frame.y), (frame.right(), frame.y), (frame.x, frame.bottom()), (frame.right(), frame.bottom())] {
            draw_poly(cx, cy, 4, 5.0, 45.0, gold);
            draw_poly(cx, cy, 4, 2.5, 45.0, Color::new(0.95, 0.88, 0.6, 1.0));
        }

        // Portrait: the hero as dressed now, on a darker plate.
        draw_rectangle(portrait.x, portrait.y, portrait.w, portrait.h, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_rectangle_lines(portrait.x, portrait.y, portrait.w, portrait.h, 1.5, dim_gold);
        if let Some(a) = self.player_appearance() {
            let (fw, fh) = self.character_frame_size(&a);
            let scale = ((portrait.w - 4.0) / fw).min((portrait.h - 4.0) / fh);
            let (w, hh) = (fw * scale, fh * scale);
            self.draw_character_in(&a, &self.world.equipment, Rect::new(portrait.x + (portrait.w - w) / 2.0, portrait.bottom() - 2.0 - hh, w, hh));
        }

        // Name, defense.
        let mut y = y0 + PAD;
        draw_text(&name, tx + 1.0, y + 19.0, 22.0, Color::new(0.0, 0.0, 0.0, 0.7));
        draw_text(&name, tx, y + 18.0, 22.0, Color::new(1.0, 0.86, 0.45, 1.0));
        if let Some(t) = &def_text {
            let w = measure_text(t, None, 16, 1.0).width;
            draw_text(t, tx + col_w - w, y + 17.0, 16.0, LIGHTGRAY);
        }
        y += 24.0;

        // Hearts.
        let (full, empty) = (Color::new(0.9, 0.15, 0.2, 1.0), Color::new(0.2, 0.2, 0.25, 0.9));
        for i in 0..hearts {
            let x = tx + i as f32 * STEP;
            draw_heart(x, y, HEART + 2.0, Color::new(0.0, 0.0, 0.0, 0.6));
            let left = hp - i * 2;
            draw_heart(x + 1.0, y + 1.0, HEART, if left >= 1 { full } else { empty });
            if left == 1 {
                draw_heart_right_half(x + 1.0, y + 1.0, HEART, empty);
            }
        }
        y += HEART + 6.0;

        // Status effects: a badge each, with the seconds left.
        y += self.draw_effect_badges(tx, y - 1.0);

        // Mana bar.
        if magic {
            let max = self.max_mana().max(1) as f32;
            let (bx, bh) = (tx, 9.0);
            draw_rectangle(bx - 1.0, y - 1.0, bar_w + 2.0, bh + 2.0, Color::new(0.0, 0.0, 0.0, 0.7));
            let fill = bar_w * (self.world.mana / max).clamp(0.0, 1.0);
            draw_rectangle(bx, y, fill, bh, Color::new(0.3, 0.45, 1.0, 1.0));
            draw_rectangle(bx, y, fill, 3.0, Color::new(0.55, 0.7, 1.0, 1.0));
            draw_rectangle_lines(bx - 1.0, y - 1.0, bar_w + 2.0, bh + 2.0, 1.0, Color::new(0.6, 0.7, 1.0, 0.8));
            draw_text(format!("{:.0}", self.world.mana), bx + bar_w + 6.0, y + 9.0, 15.0, Color::new(0.75, 0.82, 1.0, 1.0));
            y += 16.0;
        }

        // Level and the experience bar (progress towards the next level).
        if levels {
            let (have, need) = self.xp_progress();
            let (bx, bh) = (tx, 6.0);
            draw_rectangle(bx - 1.0, y - 1.0, bar_w + 2.0, bh + 2.0, Color::new(0.0, 0.0, 0.0, 0.7));
            let fill = bar_w * (have as f32 / need.max(1) as f32).clamp(0.0, 1.0);
            draw_rectangle(bx, y, fill, bh, Color::new(0.85, 0.68, 0.2, 1.0));
            draw_rectangle(bx, y, fill, 2.0, Color::new(1.0, 0.88, 0.5, 1.0));
            draw_rectangle_lines(bx - 1.0, y - 1.0, bar_w + 2.0, bh + 2.0, 1.0, Color::new(0.85, 0.72, 0.38, 0.8));
            draw_text(&level_text, bx + bar_w + 6.0, y + 7.0, 15.0, Color::new(1.0, 0.86, 0.45, 1.0));
            y += 14.0;
        }

        // Ammo of an equipped bow / crossbow.
        if let Some((sprite, n)) = ammo {
            if let Some(s) = &sprite {
                self.assets.draw_frame_in(&s.sheet, s.frame, Rect::new(tx - 2.0, y - 2.0, 20.0, 20.0), WHITE);
            }
            let col = if n == 0 { Color::new(1.0, 0.4, 0.4, 1.0) } else { WHITE };
            draw_text(format!("x{n}"), tx + 22.0, y + 13.0, 18.0, col);
        }
    }

    /// Icon and count of the ammo the equipped ranged weapon uses (None = no such weapon).
    fn hud_ammo(&self) -> Option<(Option<SpriteRef>, u32)> {
        let db = &self.project.db;
        let d = self.world.equipment.item(crate::defs::EquipSlot::Weapon).and_then(|id| db.item(id))?;
        let ammo = d.weapon.as_ref()?.ranged.as_ref()?.ammo.clone();
        if ammo.is_empty() {
            return None;
        }
        Some((db.item(&ammo).map(|a| a.icon.clone()), self.world.item_count(&ammo)))
    }

    pub(super) fn draw_game_over(&self) {
        let Some(t) = self.game_over else { return };
        let (w, h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.25, 0.0, 0.0, (t / 0.8).min(0.75)));
        // The defeat menu shows the message once it is open.
        if self.defeat_menu_open() {
            return;
        }
        let msg = "You were defeated";
        let m = measure_text(msg, None, 48, 1.0);
        draw_text(msg, (w - m.width) / 2.0, h / 2.0, 48.0, WHITE);
    }
}

/// The right half of a heart (drawn in the empty color to show half hearts).
fn draw_heart_right_half(x: f32, y: f32, s: f32, c: Color) {
    let r = s * 0.28;
    draw_circle(x + s * 0.7, y + s * 0.32, r, c);
    draw_rectangle(x + s * 0.5, y + s * 0.15, s * 0.2, s * 0.3, c);
    draw_triangle(vec2(x + s * 0.5, y + s * 0.42), vec2(x + s * 0.96, y + s * 0.42), vec2(x + s * 0.5, y + s * 0.92), c);
}

fn draw_heart(x: f32, y: f32, s: f32, c: Color) {
    let r = s * 0.28;
    draw_circle(x + s * 0.3, y + s * 0.32, r, c);
    draw_circle(x + s * 0.7, y + s * 0.32, r, c);
    draw_triangle(vec2(x + s * 0.04, y + s * 0.42), vec2(x + s * 0.96, y + s * 0.42), vec2(x + s * 0.5, y + s * 0.92), c);
}
