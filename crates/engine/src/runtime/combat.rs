//! Real-time combat: player attacks, hostile characters, knockback, defeat and game over.

use macroquad::prelude::*;

use crate::hooks;
use super::{input, Arrival, Game, Mover, Source};
use crate::defs::{CharacterDef, SpriteRef};
use crate::entity::Facing;
use crate::events::GameEvent;
use crate::state::WorldState;
use super::juice::NumberKind;
use super::text::{draw_text, measure_text};

pub(super) const SWING_TIME: f32 = 0.18;
/// Enemy invulnerability after a hit.
const HIT_INVULN: f32 = 0.3;
/// Player invulnerability after being hurt.
pub(super) const HURT_INVULN: f32 = 1.0;
pub(super) const KNOCKBACK: f32 = 190.0;
const GAME_OVER_TIME: f32 = 2.5;
/// Attack (or a click on a hostile character) starts a fight with one this near (tiles) and in sight.
pub(super) const ATTACK_SIGHT: f32 = 8.0;
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
    /// A condition for whoever it hits.
    effect: Option<crate::srd::ConditionSpec>,
    damage_type: Option<crate::srd::DamageType>,
    /// A critical shot of the player's (its damage already doubled).
    crit: bool,
    /// An enemy's shot: it may hit the player and companions (its shooter is hostile to them).
    at_player: bool,
    /// The shooter's faction: characters of factions hostile to it are hit too.
    faction: Option<u16>,
    /// Only the look of a fight's ranged attack (the roll already decided): hits nothing.
    visual: bool,
}

/// Who dealt a hit, for its feedback (hit-stop, the number's color).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Blow {
    /// The player's own hit (weapon or spell; not a companion's).
    pub by_player: bool,
    pub crit: bool,
    /// Another character's hit (not the player's side): a kill gives no experience and isn't
    /// the player's in the bestiary.
    pub by_npc: bool,
    /// Where the player's side attacked from (the player's or a companion's feet): an enemy the
    /// blow hurts notices it, however far it came from (`ai.rs` `notice_attack`).
    pub attacker: Option<Vec2>,
    /// Its damage type (resistances; `None` = untyped).
    pub damage_type: Option<crate::srd::DamageType>,
}

impl Projectile {
    /// The ground point below it (it flies `above` px over the ground), for depth sorting.
    pub(super) fn ground_y(&self, above: f32) -> f32 {
        self.pos.y + above
    }
}

pub(super) struct PlayerCombat {
    /// 0 disables combat (no HUD, no attacking, can't be hurt).
    pub max_hp: i32,
    /// Bare-handed damage.
    pub damage: i32,
    /// Base defense (armor adds to it).
    pub defense: i32,
    pub cooldown: f32,
    /// Seconds until a held E works a tree or rock again (Hold to repeat).
    pub hold_e: f32,
    pub swing: f32,
    pub invuln: f32,
    pub knock: Vec2,
    /// Sneaking (`stealth.rs`).
    pub sneak: bool,
    /// The attack animation: seconds left of it, and how long it lasts (the weapon's cooldown,
    /// 0.3 to 0.6 s; melee and shots alike).
    pub strike: f32,
    pub strike_len: f32,
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
            hold_e: 0.0,
            invuln: 0.0,
            knock: Vec2::ZERO,
            sneak: false,
            strike: 0.0,
            strike_len: 0.5,
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

/// Decays a knockback velocity; returns the movement for this frame (`u` = art pixel size).
/// One frame of knockback: a hard start that stops quickly (1.5× the speed, 1.5× the braking:
/// about the same distance as the old gentle slide).
fn decay(knock: &mut Vec2, dt: f32, u: f32) -> Vec2 {
    let step = *knock * dt * 1.5;
    *knock *= (1.0 - 13.5 * dt).max(0.0);
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
        if self.hooked("new_game") {
            self.hook("new_game", hooks::map([]));
        }
        self.ui_hidden = false;
        self.reset_cutscene();
        let (map, spawn) = (self.project.config.start_map.clone(), self.project.config.start_spawn.clone());
        self.transition(&map, Arrival::Spawn(spawn));
    }

    /// A fresh world (the start map is entered by the caller).
    pub(super) fn reset_world(&mut self) {
        self.world = WorldState { player_hp: Some(self.combat.max_hp), hour: self.project.config.start_hour, ..Default::default() };
        // The hero's SRD ability scores and proficiencies (the creator / race may change them).
        let rules = &self.project.config.rules;
        self.world.abilities = rules.abilities;
        self.world.proficiencies = rules.proficiencies.iter().map(|p| crate::srd::abilities::proficiency_id(p)).filter(|p| !p.is_empty()).collect();
        // (Full SRD hit points with those scores.)
        if self.combat.max_hp > 0 {
            self.world.player_hp = Some(self.player_max_hp());
        }
        self.clock_seen = None;
        self.slot = None;
        self.order = None;
        self.game_over = None;
        self.autosave_pending = false;
        self.prepare_world();
        self.inv = Default::default();
        self.combat.reset();
        self.fight = None;
        self.runner.clear();
        self.ui = super::Ui::default();
        self.cache.clear();
        self.loaded.map.name.clear();
        self.script_window = None;
    }

    pub(super) fn update_combat(&mut self, dt: f32, frozen: bool, key_consumed: bool) {
        // (Timers stop at 0 instead of counting down forever.)
        let c = &mut self.combat;
        c.cooldown = (c.cooldown - dt).max(0.0);
        c.swing = (c.swing - dt).max(0.0);
        c.strike = (c.strike - dt).max(0.0);
        c.invuln = (c.invuln - dt).max(0.0);
        for e in &mut self.loaded.entities {
            e.invuln = (e.invuln - dt).max(0.0);
            e.flash = (e.flash - dt).max(0.0);
            e.hurt = (e.hurt - dt).max(0.0);
            e.strike = (e.strike - dt).max(0.0);
            // The bar's lost part waits a moment, then drains to the HP left.
            let frac = if e.max_hp > 0 { e.hp.max(0) as f32 / e.max_hp as f32 } else { 0.0 };
            if e.bar < super::juice::BAR_TIME - 0.3 {
                e.bar_lag -= dt * 0.8;
            }
            e.bar_lag = e.bar_lag.max(frac);
            e.bar = (e.bar - dt).max(0.0);
        }
        for fx in &mut self.effects {
            fx.1 += dt;
        }
        self.effects.retain(|fx| fx.1 < EFFECT_TIME);
        // In a fight the turns take the input (`turns/`).
        if !self.combat.enabled() || frozen || self.fight.is_some() {
            return;
        }
        if !key_consumed && input::act_pressed(input::Act::SwitchWeapon) {
            self.cycle_weapon();
        }
        self.update_sneak(key_consumed);
        // (Options › Accessibility › Hold to repeat: holding Attack keeps swinging at a tree or a vein.)
        let attack = input::act_pressed(input::Act::Attack) || (self.settings.hold_repeat && input::act_down(input::Act::Attack));
        if !key_consumed && self.combat.cooldown <= 0.0 && attack {
            self.order = None;
            self.attack_at(None);
        }
    }

    /// Attack outside a fight: a hostile character near and in sight starts one (the hero struck
    /// first: those that hadn't noticed them are surprised); otherwise a swing at a gathering
    /// prop in reach (trees, veins). `aim` (world point) sets the direction.
    pub(super) fn attack_at(&mut self, aim: Option<Vec2>) {
        // Both hands on the rungs.
        if !self.combat.enabled() || self.combat.cooldown > 0.0 || self.player_stunned() || self.climbing() || self.fight.is_some() {
            return;
        }
        // Charmed: the hero won't attack.
        if crate::srd::conditions::has(&self.world.effects, crate::srd::Condition::Charmed) {
            return;
        }
        // Both hands full.
        if !self.world.player_carrying.is_empty() {
            self.ui.toast("Put down what you carry first.".into());
            self.combat.cooldown = 0.5;
            return;
        }
        if self.hostile_in_sight(ATTACK_SIGHT) && self.start_combat(true, true) {
            return;
        }
        let weapon = self.attack_weapon();
        let cooldown = weapon.cooldown.max(0.05);
        let ts = self.ts();
        let center = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        let dir = aim.and_then(|t| (t - center).try_normalize()).unwrap_or_else(|| Vec2::from(self.player.facing.dir()));
        if let Some(f) = Facing::from_vec(dir.x, dir.y) {
            self.player.facing = f;
        }
        self.combat.cooldown = cooldown;
        self.start_strike(cooldown);
        self.audio.play(&self.project.config.sfx.attack);
        if weapon.ranged.is_some() {
            return;
        }
        let reach = weapon.reach.max(0.3) * ts;
        let arc = weapon.arc.clamp(5.0, 180.0).to_radians();
        self.combat.swing = SWING_TIME;
        self.combat.swing_dir = dir;
        self.combat.swing_reach = reach;
        self.combat.swing_arc = arc;
        // A gathering prop in the swing takes a hit (veins, trees).
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

    /// A click on hostile character `i` starts a fight from here: near enough and in sight.
    pub(super) fn starts_fight_with(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        let eye = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        e.body.pos.distance(self.player.pos) <= ATTACK_SIGHT * self.ts() && self.line_clear(eye, e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0))
    }

    /// A hostile character that can fight within `tiles` and in a clear line.
    pub(super) fn hostile_in_sight(&self, tiles: f32) -> bool {
        let ts = self.ts();
        let eye = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        self.loaded.entities.iter().any(|e| {
            e.active && e.hostile && e.max_hp > 0 && e.hp > 0 && e.kind == crate::entity::EntityKind::Character && e.body.pos.distance(self.player.pos) <= tiles * ts && self.line_clear(eye, e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0))
        })
    }

    /// A fight's shot or bolt flying from `from` to `to` (only its look).
    pub(super) fn fly_visual(&mut self, from: Vec2, to: Vec2, sprite: SpriteRef) {
        let d = to - from;
        let dir = d.try_normalize().unwrap_or(vec2(1.0, 0.0));
        let speed = 14.0 * self.ts();
        self.projectiles.push(Projectile { pos: from, vel: dir * speed, dist_left: d.length(), traveled: 0.0, damage: 0, from_player: false, shooter: None, sprite, effect: None, damage_type: None, crit: false, at_player: false, faction: None, visual: true });
    }

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
                if p.visual {
                    continue;
                }
                if p.from_player {
                    let target = self.loaded.entities.iter().position(|e| {
                        e.active && e.hostile && e.max_hp > 0 && e.invuln <= 0.0 && Some(e.uid) != p.shooter && {
                            // Characters are hit anywhere on their body, not just the feet.
                            let r = e.body.rect();
                            Rect::new(r.x, r.y - ts * 0.6, r.w, r.h + ts * 0.6).overlaps(&hit_box)
                        }
                    });
                    if let Some(t) = target {
                        let (mut dmg, from, fx, damage_type) = (p.damage, pos - p.vel.normalize_or_zero() * 8.0, p.effect.clone(), p.damage_type);
                        // Shot by the player or a companion: the target notices where from.
                        let attacker = match p.shooter {
                            None => Some(self.player.pos),
                            Some(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map(|e| e.body.pos),
                        };
                        let mut blow = Blow { by_player: p.shooter.is_none(), crit: p.crit, attacker, damage_type, ..Default::default() };
                        // The player's own shot (not a companion's) at an enemy that hasn't noticed them.
                        if blow.by_player {
                            let (d, sneak) = self.sneak_attack(t, dmg);
                            (dmg, blow.crit) = (d, blow.crit || sneak);
                        }
                        if let Some(fx) = fx {
                            self.apply_entity_effect(t, &fx);
                        }
                        self.strike_entity(t, dmg, from, 1.0, blow);
                        alive = false;
                        break;
                    }
                } else {
                    // Characters of factions hostile to the shooter.
                    let foe = p.faction.and_then(|f| {
                        self.loaded.entities.iter().position(|e| {
                            e.active && e.kind == crate::entity::EntityKind::Character && e.companion.is_none() && e.max_hp > 0 && e.invuln <= 0.0 && Some(e.uid) != p.shooter && self.factions.hostile(Some(f), e.faction) && {
                                let r = e.body.rect();
                                Rect::new(r.x, r.y - ts * 0.6, r.w, r.h + ts * 0.6).overlaps(&hit_box)
                            }
                        })
                    });
                    if let Some(t) = foe {
                        let (dmg, from, fx, damage_type) = (p.damage, pos - p.vel.normalize_or_zero() * 8.0, p.effect.clone(), p.damage_type);
                        if let Some(fx) = fx {
                            self.apply_entity_effect(t, &fx);
                        }
                        self.npc_hits(t, dmg, from, 1.0, damage_type);
                        alive = false;
                        break;
                    }
                    if p.at_player {
                        // Companions are hit like the player.
                        let companion = companions.iter().map(|(c, _)| *c).find(|&c| {
                            let r = self.loaded.entities[c].body.rect();
                            self.loaded.entities[c].invuln <= 0.0 && Rect::new(r.x, r.y - ts * 0.8, r.w, r.h + ts * 0.8).overlaps(&hit_box)
                        });
                        if let Some(c) = companion {
                            let (dmg, from, fx, damage_type) = (p.damage, pos - p.vel.normalize_or_zero() * 8.0, p.effect.clone(), p.damage_type);
                            if let Some(fx) = fx {
                                self.apply_entity_effect(c, &fx);
                            }
                            self.strike_entity(c, dmg, from, 1.0, Blow { damage_type, ..Default::default() });
                            alive = false;
                            break;
                        }
                        let r = self.player.rect();
                        let body = Rect::new(r.x, r.y - ts * 0.8, r.w, r.h + ts * 0.8);
                        if body.overlaps(&hit_box) {
                            let (dmg, from, fx, damage_type) = (p.damage, pos - p.vel.normalize_or_zero() * 8.0, p.effect.clone(), p.damage_type);
                            if self.combat.invuln <= 0.0 {
                                if let Some(fx) = fx {
                                    self.apply_player_effect(&fx);
                                }
                                self.hurt_player_by(dmg, from, damage_type);
                            }
                            alive = false;
                            break;
                        }
                    }
                }
                // Walls are checked after targets, so characters standing on a solid tile can still be hit.
                // Low obstacles (fences, water, barrels) don't stop shots; walls, trees and tall props do.
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
                if !leaving && self.blocks_shot(tile.0, tile.1) && !occupied() {
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
        self.strike_entity(i, dmg, from, knockback, Blow::default());
    }

    /// [`Self::damage_entity`] with what dealt the blow: the player's own hits pause the world a
    /// moment (longer for a critical hit or a kill), critical ones shake it a little.
    pub(super) fn strike_entity(&mut self, i: usize, dmg: i32, from: Vec2, knockback: f32, blow: Blow) {
        // In a fight: the turns' damage (death saves, the order).
        let uid = self.loaded.entities[i].uid;
        if self.fighting(uid) {
            let by = blow.by_player.then_some(super::turns::Who::Player);
            self.combat_damage(super::turns::Who::Npc(uid), dmg, blow.damage_type, blow.crit, by);
            return;
        }
        let (u, ts) = (self.u(), self.ts());
        // Its damage type against its stat block's resistances (immune: nothing), and a helpless
        // target (paralyzed, unconscious) takes a critical hit.
        let defenses = self.stat_block(&self.loaded.entities[i]).map(|s| s.defenses()).unwrap_or_default();
        let e = &self.loaded.entities[i];
        let mut blow = blow;
        let mut dmg = dmg;
        if !e.effects.is_empty() && crate::srd::conditions::rules(&e.effects).hits_crit && !blow.crit {
            dmg *= 2;
            blow.crit = true;
        }
        let typed = defenses.apply(dmg, blow.damage_type);
        if typed <= 0 && dmg > 0 {
            return;
        }
        let mut hit = (typed - e.defense).max(1);
        // Script hook: the damage can change (0 = none).
        if self.hooked("damage") {
            let (name, def) = (e.name.clone(), e.def.clone());
            let attacker = if blow.by_player { "player" } else { "" };
            let m = self.hook("damage", hooks::map([("attacker", attacker.into()), ("target", name.into()), ("target_def", def.into()), ("amount", (hit as i64).into()), ("crit", blow.crit.into())]));
            hit = hooks::int(&m, "amount", hit as i64).clamp(0, i32::MAX as i64) as i32;
            if hit == 0 || i >= self.loaded.entities.len() {
                return;
            }
        }
        let e = &mut self.loaded.entities[i];
        let before = e.hp;
        e.hp -= hit;
        e.last_hit = hit;
        e.invuln = HIT_INVULN;
        e.flash = super::juice::HIT_FLASH;
        e.hurt = 0.25;
        // The health bar shows again; what the hit took drains away from where the bar stood.
        if e.bar <= 0.0 && e.max_hp > 0 {
            e.bar_lag = before.max(0) as f32 / e.max_hp as f32;
        }
        e.bar = super::juice::BAR_TIME;
        e.body.move_target = None;
        let away = (e.body.pos - from).try_normalize().unwrap_or(vec2(0.0, 1.0));
        e.knock = away * KNOCKBACK * u * knockback.max(0.0);
        let kind = if e.companion.is_some() {
            NumberKind::Hurt
        } else if blow.crit {
            NumberKind::Crit
        } else {
            NumberKind::Hit
        };
        let (head, killed) = (e.body.pos - vec2(0.0, ts * 1.1), e.hp <= 0);
        self.damage_number(head, hit, kind);
        if blow.by_player && blow.crit {
            self.shake_screen(0.12, 0.6);
        }
        if let Some(at) = blow.attacker.filter(|_| !killed) {
            self.notice_attack(i, at);
        }
        if killed {
            if blow.by_npc {
                self.defeat_by(i, false, false);
            } else {
                self.defeat(i);
            }
        } else {
            let (hit, at) = (self.project.config.sfx.hit.clone(), self.loaded.entities[i].body.pos);
            self.sound_from(&hit, at);
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

    /// A character (or companion) hit by a character fighting it: companions as by an enemy,
    /// others as a blow of another character (no experience for the player).
    pub(super) fn npc_hits(&mut self, i: usize, dmg: i32, from: Vec2, knockback: f32, damage_type: Option<crate::srd::DamageType>) {
        let by_npc = self.loaded.entities[i].companion.is_none();
        self.strike_entity(i, dmg, from, knockback, Blow { by_npc, damage_type, ..Default::default() });
    }

    fn defeat_with(&mut self, i: usize, xp: bool) {
        self.defeat_by(i, xp, true);
    }

    /// Defeats character `i`; `xp`: the player gains its experience; `bestiary`: it counts as
    /// the player's.
    fn defeat_by(&mut self, i: usize, xp: bool, bestiary: bool) {
        if self.loaded.entities[i].companion.is_some() {
            // Companions aren't defeated: they fall (and wait to be healed), or go home.
            self.knock_down(i);
            return;
        }
        if xp {
            let n = self.xp_of(&self.loaded.entities[i].def);
            self.give_xp(n as i64);
            if self.loaded.entities[i].hostile {
                self.battle.won += 1;
            }
        }
        // The bestiary counts every character kind the player's side defeated.
        if bestiary && self.loaded.entities[i].kind == crate::entity::EntityKind::Character {
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
        let (defeat, at) = (self.project.config.sfx.defeat.clone(), self.loaded.entities[i].body.pos);
        self.sound_from(&defeat, at);
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
        self.hurt_player_by(dmg, from, None);
    }

    /// [`Self::hurt_player`] with a damage type: the hero's resistances halve it (an immunity:
    /// nothing).
    pub(super) fn hurt_player_by(&mut self, dmg: i32, from: Vec2, damage_type: Option<crate::srd::DamageType>) {
        if !self.combat.enabled() || self.game_over.is_some() {
            return;
        }
        // In a fight: the turns' damage (0 HP = unconscious and death saves).
        if self.fight.is_some() {
            self.combat_damage(super::turns::Who::Player, dmg, damage_type, false, None);
            return;
        }
        let dmg = self.player_defenses().apply(dmg, damage_type);
        if dmg <= 0 {
            return;
        }
        // Armor absorbs damage, but at least 1 always gets through.
        let mut taken = (dmg - self.player_defense()).max(1);
        // Script hook: the damage can change (0 = none).
        if self.hooked("damage") {
            let m = self.hook("damage", hooks::map([("attacker", "".into()), ("target", "player".into()), ("target_def", "".into()), ("amount", (taken as i64).into()), ("crit", false.into())]));
            taken = hooks::int(&m, "amount", taken as i64).clamp(0, i32::MAX as i64) as i32;
            if taken == 0 {
                return;
            }
        }
        let hp = (self.hp() - taken).max(0);
        self.world.player_hp = Some(hp);
        self.combat.invuln = HURT_INVULN;
        let away = (self.player.pos - from).try_normalize().unwrap_or(vec2(0.0, 1.0));
        self.combat.knock = away * KNOCKBACK * self.u();
        self.audio.play(&self.project.config.sfx.hurt);
        self.hurt_feedback();
        let head = self.player.pos - vec2(0.0, self.ts() * 1.1);
        self.damage_number(head, taken, NumberKind::Hurt);
        if hp == 0 {
            self.player_defeated();
        }
    }

    /// The player's HP reached 0: the game-over screen, then the defeat menu.
    pub(super) fn player_defeated(&mut self) {
        self.fight = None;
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
        let amount = self.heal_hook("player", amount);
        if amount <= 0 {
            return;
        }
        if self.fight.is_some() {
            self.combat_heal(super::turns::Who::Player, amount);
            self.ui.toast(format!("+{amount} HP"));
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

    /// Starts the player's attack animation for a weapon with `cooldown` seconds between attacks.
    pub(super) fn start_strike(&mut self, cooldown: f32) {
        let len = cooldown.clamp(0.3, 0.6);
        (self.combat.strike, self.combat.strike_len) = (len, len);
    }

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
            let painted = self.player_appearance().is_some_and(|a| self.figure_animates(&a));
            if let Some(icon) = icon.filter(|_| !painted) {
                let at = center + vec2(a1.cos(), a1.sin()) * (r * 0.5).max(ts * 0.45);
                self.assets.draw_frame_scaled(&icon.sheet, icon.frame, at, a1 + std::f32::consts::FRAC_PI_4, 1.0, WHITE);
            }
        }
        // Health bars of characters hurt in the last few seconds (fading out), the lost part
        // draining in white.
        for e in self.loaded.entities.iter().filter(|e| e.active && e.max_hp > 0 && e.hp > 0 && e.bar > 0.0 && !e.companion.as_ref().is_some_and(|l| l.down.is_some())) {
            let a = (e.bar / 0.5).min(1.0);
            let w = 14.0 * u;
            let (x, y) = (e.body.pos.x - w / 2.0, e.body.pos.y - ts - 6.0 * u);
            let frac = e.hp as f32 / e.max_hp as f32;
            draw_rectangle(x - u, y - u, w + 2.0 * u, 4.0 * u, Color::new(0.0, 0.0, 0.0, 0.7 * a));
            draw_rectangle(x, y, w * e.bar_lag.min(1.0), 2.0 * u, Color::new(1.0, 1.0, 1.0, 0.85 * a));
            draw_rectangle(x, y, w * frac, 2.0 * u, Color::new(0.9, 0.2, 0.2, a));
        }
        // Status effects: small colored squares above the health bar.
        for e in self.loaded.entities.iter().filter(|e| e.active && !e.effects.is_empty()) {
            // Above the head of a full-height (1.5 tiles) character.
            // (Over the head; a character lying down has it lower.)
            let lying = crate::srd::conditions::rules(&e.effects).lying;
            self.draw_npc_effects(e.body.pos.x, e.body.pos.y - if lying { ts * 0.8 } else { ts * 1.5 + 2.0 * u }, &e.effects);
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

    /// Draws projectile `k` (in the depth-sorted world drawing, see `Game::draw`).
    pub(super) fn draw_projectile(&self, k: usize) {
        let Some(p) = self.projectiles.get(k) else { return };
        let angle = p.vel.y.atan2(p.vel.x);
        if !self.assets.draw_frame_rotated(&p.sprite.sheet, p.sprite.frame, p.pos, angle, WHITE) {
            let u = self.u();
            draw_line(p.pos.x, p.pos.y, p.pos.x - p.vel.x * 0.03, p.pos.y - p.vel.y * 0.03, 1.5 * u, WHITE);
        }
    }

    /// Hearts in the top-left corner.
    /// The player status in the top-left corner: a framed panel with a small portrait, the
    /// hero's name, the HP bar with its numbers, the mana bar (with magic) and the ammo of a
    /// ranged weapon.
    pub(super) fn draw_hud(&self) {
        let _font = super::text::role(super::text::Role::Hud);
        if !self.combat.enabled() {
            return;
        }
        const HEART: f32 = 18.0;
        const PAD: f32 = 8.0;
        let (x0, y0) = (10.0, 10.0);
        let hp = self.hp();
        let max_hp = self.player_max_hp().max(1);
        let hp_text = format!("{} / {max_hp}", hp.max(0));
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
        // The HP bar: wide enough for its numbers, the dodge pip and the shield after it.
        let hp_w = (measure_text(&hp_text, None, 16, 1.0).width + 24.0).max(150.0);
        let hearts_w = hp_w + 40.0;
        let bar_w = hp_w.max(120.0);
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
        // "Sneaking" under the panel (companions' rows go below it).
        if self.sneaking() {
            self.draw_sneak_hint(frame.x + 4.0, frame.bottom() + 18.0);
            self.hud_bottom.set(frame.bottom() + 24.0);
        }

        // Frame: the skin's HUD picture, or a dark backing with a gold border, an inner line and
        // corner studs (high contrast: opaque black, a bright yellow border).
        super::skin::hud_frame(frame);
        let dim_gold = super::skin::hud_dim_line();

        // Portrait: the hero as dressed now, on a darker plate.
        draw_rectangle(portrait.x, portrait.y, portrait.w, portrait.h, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_rectangle_lines(portrait.x, portrait.y, portrait.w, portrait.h, 1.5, dim_gold);
        if let Some(a) = self.player_appearance() {
            let (fw, fh) = self.character_frame_size(&a);
            let scale = ((portrait.w - 4.0) / fw).min((portrait.h - 4.0) / fh);
            let (w, hh) = (fw * scale, fh * scale);
            let gear = self.player_gear();
            self.draw_character_in(&a, &gear, Rect::new(portrait.x + (portrait.w - w) / 2.0, portrait.bottom() - 2.0 - hh, w, hh));
        }

        // Name, defense.
        let mut y = y0 + PAD;
        draw_text(&name, tx + 1.0, y + 19.0, 22.0, Color::new(0.0, 0.0, 0.0, 0.7));
        draw_text(&name, tx, y + 18.0, 22.0, super::skin::text(super::skin::Text::Title, Color::new(1.0, 0.86, 0.45, 1.0)));
        if let Some(t) = &def_text {
            let w = measure_text(t, None, 16, 1.0).width;
            draw_text(t, tx + col_w - w, y + 17.0, 16.0, super::skin::dim());
        }
        y += 24.0;

        // HP: a bar with "34 / 44" on it (the SRD's hit points outgrew hearts).
        super::skin::bar(super::skin::Bar::Hp, tx, y + 1.0, hp_w, HEART, hp as f32 / max_hp as f32);
        let tw = measure_text(&hp_text, None, 16, 1.0).width;
        draw_text(&hp_text, tx + (hp_w - tw) / 2.0 + 1.0, y + HEART - 1.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
        draw_text(&hp_text, tx + (hp_w - tw) / 2.0, y + HEART - 2.0, 16.0, WHITE);
        y += HEART + 6.0;

        // Status effects: a badge each, with the seconds left.
        y += self.draw_effect_badges(tx, y - 1.0);

        // Spell slots left (a caster's): a pip per slot by level.
        if magic && self.spellcasting(super::turns::Who::Player).is_some() {
            let (max, left) = (self.slot_max(super::turns::Who::Player), self.slots_left(super::turns::Who::Player));
            let mut x = tx;
            for k in (0..9).filter(|k| max[*k] > 0) {
                draw_text(format!("{}", k + 1), x, y + 9.0, 14.0, Color::new(0.75, 0.82, 1.0, 1.0));
                x += 10.0;
                for n in 0..max[k] {
                    let full = n < left[k];
                    super::skin::bar(super::skin::Bar::Mana, x, y + 1.0, 7.0, 8.0, if full { 1.0 } else { 0.0 });
                    x += 9.0;
                }
                x += 6.0;
            }
            y += 16.0;
        }

        // Level and the experience bar (progress towards the next level).
        if levels {
            let (have, need) = self.xp_progress();
            let (bx, bh) = (tx, 6.0);
            super::skin::bar(super::skin::Bar::Xp, bx, y, bar_w, bh, have as f32 / need.max(1) as f32);
            draw_text(&level_text, bx + bar_w + 6.0, y + 7.0, 15.0, Color::new(1.0, 0.86, 0.45, 1.0));
            y += 14.0;
        }

        // Ammo of an equipped bow / crossbow.
        if let Some((sprite, n)) = ammo {
            if let Some(s) = &sprite {
                self.assets.draw_frame_in(&s.sheet, s.frame, Rect::new(tx - 2.0, y - 2.0, 20.0, 20.0), WHITE);
            }
            let col = if n == 0 { super::skin::text(super::skin::Text::Negative, Color::new(1.0, 0.4, 0.4, 1.0)) } else { super::skin::normal() };
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
pub(super) fn draw_heart_right_half(x: f32, y: f32, s: f32, c: Color) {
    let r = s * 0.28;
    draw_circle(x + s * 0.7, y + s * 0.32, r, c);
    draw_rectangle(x + s * 0.5, y + s * 0.15, s * 0.2, s * 0.3, c);
    draw_triangle(vec2(x + s * 0.5, y + s * 0.42), vec2(x + s * 0.96, y + s * 0.42), vec2(x + s * 0.5, y + s * 0.92), c);
}

pub(crate) fn draw_heart(x: f32, y: f32, s: f32, c: Color) {
    let r = s * 0.28;
    draw_circle(x + s * 0.3, y + s * 0.32, r, c);
    draw_circle(x + s * 0.7, y + s * 0.32, r, c);
    draw_triangle(vec2(x + s * 0.04, y + s * 0.42), vec2(x + s * 0.96, y + s * 0.42), vec2(x + s * 0.5, y + s * 0.92), c);
}
