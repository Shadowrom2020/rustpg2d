//! Turn-based combat (the SRD's): a fight starts where hostiles notice the party or the hero
//! attacks, everyone near rolls initiative and acts in turns on the tile grid (5 ft a tile):
//! movement up to the speed, one action, one bonus action, one reaction a round. Attacks roll
//! against armor class ([`crate::srd::combat`]), creatures at 0 HP die, the hero and party
//! members fall unconscious and make death saving throws. Exploration (free movement) resumes
//! when no enemy is left. `ai.rs` plays the characters' turns, `ui.rs` draws the bars, overlays
//! and the log and reads the player's input.

mod actions;
mod ai;
mod ui;

pub(super) use actions::TurnAct;

use macroquad::prelude::*;

use super::juice::NumberKind;
use super::Game;
use crate::entity::{EntityKind, Facing};
use crate::grid::{feet_spot, feet_tile, TilePos};
use crate::srd::combat::{self as rules, AttackSituation, Cover, DeathOutcome, DeathSaves};
use crate::srd::conditions::{self, Condition, ConditionSpec};
use crate::srd::{Ability, DamageType, Mode};

/// How far (tiles) from where it starts a fight draws everyone in.
pub(super) const JOIN_TILES: f32 = 12.0;
/// A fleeing enemy this far (tiles) from every foe has left the fight.
pub(super) const FLED_TILES: f32 = 15.0;

/// Who a combatant is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Who {
    Player,
    Npc(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Party,
    Enemy,
}

/// One combatant in the turn order.
#[derive(Clone, Debug)]
pub(super) struct Fighter {
    pub who: Who,
    /// Its name ("player", a character's name or definition id), kept for when it is gone.
    pub key: String,
    pub side: Side,
    pub init: i32,
    pub dex: i32,
    /// Skips its first turn (round 1).
    pub surprised: bool,
    /// Its turns are played by the AI (enemies, allies, party members left to it).
    pub ai: bool,
    /// The hero and party members at 0 HP.
    pub saves: DeathSaves,
    pub down: bool,
    /// Out of the fight (dead, fled).
    pub out: bool,
    /// Took the Dodge action (until its next turn).
    pub dodging: bool,
    /// Can't be seen (Hide, until it attacks).
    pub hidden: bool,
    /// Its reaction is spent until its next turn.
    pub reacted: bool,
    /// Readied an attack (Ready): the next foe entering its reach.
    pub readied: bool,
    /// Who it is grappling.
    pub grappling: Option<Who>,
}

/// What the player-controlled turn's cursor does on Confirm.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Aim {
    /// Walk to a reachable tile (an enemy in reach is attacked).
    Move,
    Attack,
    /// Two-weapon fighting: the off-hand's attack (a bonus action).
    OffHand,
    Help,
    Grapple,
    Shove,
    Cast(String),
}

/// The current turn's economy.
#[derive(Clone, Debug, Default)]
pub(super) struct Turn {
    /// Feet moved and the speed (doubled by Dash).
    pub moved: u32,
    pub speed: u32,
    pub action: bool,
    pub bonus: bool,
    pub disengaged: bool,
    /// Attacks left of the Attack action (Multiattack).
    pub attacks_left: u32,
    /// It took the Attack action (the off-hand's bonus attack may follow).
    pub attacked: bool,
    /// The AI walked already this turn.
    pub ai_walked: bool,
}

/// An attack, ready to roll.
#[derive(Clone, Debug)]
pub(super) struct AttackSpec {
    pub name: String,
    pub bonus: i32,
    /// Damage dice with the modifier ("1d8+3").
    pub dice: String,
    pub damage_type: Option<DamageType>,
    pub ranged: bool,
    /// Melee reach / ranged normal and long range, in tiles.
    pub reach: i32,
    pub range: i32,
    pub long: i32,
    /// The lowest natural d20 that is critical (20; 19 for a keen blade).
    pub crit_on: u32,
    /// A condition a hit gives (a weapon's, a touch's).
    pub effect: Option<ConditionSpec>,
    pub projectile: Option<crate::defs::SpriteRef>,
}

pub(super) struct Combat {
    pub round: u32,
    pub order: Vec<Fighter>,
    /// Index into `order`.
    pub current: usize,
    pub turn: Turn,
    /// An ally helped against this target (the next attack against it has advantage).
    pub helped: Vec<Who>,
    pub cursor: TilePos,
    pub aim: Aim,
    /// A walk on screen: the tiles left, and the seconds to the next one.
    pub walking: Vec<TilePos>,
    /// Seconds before the AI's next step (its pace).
    pub ai_t: f32,
    /// Readable lines of what happened (the newest last).
    pub log: Vec<String>,
    /// The AI acts without pausing (autoplay `turn fast`).
    pub fast: bool,
    /// The fight is decided: ends after the current animation.
    pub over: bool,
    /// Shove pushes a tile away instead of knocking prone.
    pub shove_push: bool,
    /// The action bar's selected entry (keys, pad).
    pub selected: usize,
    /// An attack to make once the walk there ends (a click on a foe out of reach).
    pub then_attack: Option<Who>,
    /// The current fighter's reachable tiles (the overlay, clicks), and what they were for.
    pub reach: std::collections::HashMap<TilePos, (u32, TilePos)>,
    pub reach_key: Option<(usize, u32, u32, u32, TilePos)>,
    /// The mouse's last screen position (the cursor follows it when it moves).
    pub mouse_last: Vec2,
}

/// What a fight log keeps.
const LOG_LEN: usize = 40;

impl Combat {
    pub(super) fn current(&self) -> &Fighter {
        &self.order[self.current]
    }

    fn fighter(&self, who: Who) -> Option<&Fighter> {
        self.order.iter().find(|f| f.who == who)
    }

    fn fighter_mut(&mut self, who: Who) -> Option<&mut Fighter> {
        self.order.iter_mut().find(|f| f.who == who)
    }

    pub(super) fn log(&mut self, line: String) {
        self.log.push(line);
        let extra = self.log.len().saturating_sub(LOG_LEN);
        self.log.drain(..extra);
    }
}

impl Game {
    // ------------------------------------------------------------------------- who is who

    pub(super) fn who_index(&self, who: Who) -> Option<usize> {
        match who {
            Who::Player => None,
            Who::Npc(uid) => self.loaded.entities.iter().position(|e| e.uid == uid),
        }
    }

    pub(super) fn who_pos(&self, who: Who) -> Vec2 {
        match who {
            Who::Player => self.player.pos,
            Who::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map_or(Vec2::ZERO, |e| e.body.pos),
        }
    }

    pub(super) fn who_tile(&self, who: Who) -> TilePos {
        feet_tile(self.who_pos(who), self.ts())
    }

    pub(super) fn who_name(&self, who: Who) -> String {
        match who {
            Who::Player => self.world.player_name.clone(),
            Who::Npc(uid) => self.entity_display_name(uid).unwrap_or_else(|| "?".into()),
        }
    }

    fn who_hp(&self, who: Who) -> (i32, i32) {
        match who {
            Who::Player => (self.hp(), self.player_max_hp()),
            Who::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map_or((0, 0), |e| (e.hp, e.max_hp)),
        }
    }

    pub(super) fn who_conditions(&self, who: Who) -> &[crate::srd::ActiveCondition] {
        match who {
            Who::Player => &self.world.effects,
            Who::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map_or(&[], |e| &e.effects),
        }
    }

    fn who_conditions_mut(&mut self, who: Who) -> Option<&mut Vec<crate::srd::ActiveCondition>> {
        match who {
            Who::Player => Some(&mut self.world.effects),
            Who::Npc(uid) => self.loaded.entities.iter_mut().find(|e| e.uid == uid).map(|e| &mut e.effects),
        }
    }

    /// A character's stat block, or one made from its old real-time values (AC 10 + defense,
    /// HP); a block without attacks gets one with its weapon or its damage.
    pub(super) fn who_block(&self, who: Who) -> Option<crate::srd::StatBlock> {
        let i = self.who_index(who)?;
        let e = &self.loaded.entities[i];
        let mut block = self.stat_block(e).cloned().unwrap_or_else(|| crate::srd::StatBlock { ac: 10 + e.defense, hp: e.max_hp, speed_ft: 30, ..Default::default() });
        if block.attacks.is_empty() {
            let w = e.weapon.clone().unwrap_or_else(|| crate::defs::Weapon::melee(e.damage.max(1), 1.0));
            let ranged = w.ranged.as_ref().map(|r| (r.range.round() as u32 * 5, if r.long_range > 0.0 { r.long_range } else { r.range * 2.0 }.round() as u32 * 5));
            // The attack bonus a stat block of its strength would have: proficiency + STR or DEX.
            let ability = if ranged.is_some() { crate::srd::Ability::Dex } else { crate::srd::Ability::Str };
            let bonus = crate::srd::abilities::proficiency_bonus(block.proficiency_level()) + block.abilities.modifier(ability);
            block.attacks.push(crate::srd::StatAttack {
                name: String::new(),
                bonus,
                dice: w.damage_dice(),
                damage_type: w.damage_type,
                reach_ft: if ranged.is_some() { 0 } else { (w.reach.round().max(1.0) as u32) * 5 },
                range_ft: ranged.map_or(0, |r| r.0),
                long_ft: ranged.map_or(0, |r| r.1),
            });
        }
        if block.speed_ft == 0 {
            block.speed_ft = 30;
        }
        Some(block)
    }

    /// Armor class: the hero's, a stat block's.
    pub(super) fn who_ac(&self, who: Who) -> i32 {
        match who {
            Who::Player => self.armor_class(),
            _ => self.who_block(who).map_or(10, |b| b.ac),
        }
    }

    /// Speed in feet, after conditions (0 when it can't move).
    fn who_speed(&self, who: Who) -> u32 {
        let base = match who {
            Who::Player => self.player_race().map_or(30, |r| r.walk_feet()),
            // (Flyers fly over the grid: their best speed until 0.32's movement modes.)
            _ => self.who_block(who).map_or(30, |b| b.speed_ft.max(b.speeds.fly)),
        };
        let r = conditions::rules(self.who_conditions(who));
        (base as f32 * r.speed).round() as u32
    }

    fn who_dex(&self, who: Who) -> i32 {
        match who {
            Who::Player => self.world.abilities.modifier(Ability::Dex),
            _ => self.who_block(who).map_or(0, |b| b.abilities.modifier(Ability::Dex)),
        }
    }

    /// The fighters' tiles (the living ones), but `except`.
    fn fighter_tiles(&self, except: Who) -> Vec<(Who, Side, TilePos)> {
        let Some(c) = &self.fight else { return Vec::new() };
        c.order.iter().filter(|f| f.who != except && !f.out).map(|f| (f.who, f.side, self.who_tile(f.who))).collect()
    }

    // --------------------------------------------------------------------- start and end

    /// Starts a fight: everyone hostile to the party within reach of it, the party and its allies.
    /// `surprise`: the hero struck first, so enemies that hadn't noticed them lose round 1.
    pub(super) fn start_combat(&mut self, surprise: bool) -> bool {
        if self.fight.is_some() {
            return self.join_fight();
        }
        if !self.combat_on() || self.game_over.is_some() || self.on_title() {
            return false;
        }
        let ts = self.ts();
        let here = self.player.pos;
        let near = |p: Vec2| p.distance(here) <= JOIN_TILES * ts;
        let mut order: Vec<Fighter> = Vec::new();
        let fighter = |who, side, ai| Fighter { who, key: String::new(), side, init: 0, dex: 0, surprised: false, ai, saves: DeathSaves::default(), down: false, out: false, dodging: false, hidden: false, reacted: false, readied: false, grappling: None };
        order.push(fighter(Who::Player, Side::Party, false));
        let mut enemies = Vec::new();
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if !e.active || e.kind != EntityKind::Character || e.max_hp <= 0 || e.hp <= 0 || !near(e.body.pos) {
                continue;
            }
            if e.companion.is_some() {
                let ai = e.companion.as_ref().is_some_and(|l| l.ai);
                order.push(fighter(Who::Npc(e.uid), Side::Party, ai));
            } else if e.hostile {
                let mut f = fighter(Who::Npc(e.uid), Side::Enemy, true);
                f.surprised = surprise && self.unaware(i);
                order.push(f);
                enemies.push(e.faction);
            }
        }
        if enemies.is_empty() {
            return false;
        }
        // Characters of factions hostile to the enemies fight beside the party (the guards).
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if !e.active || e.kind != EntityKind::Character || e.max_hp <= 0 || e.hp <= 0 || e.hostile || e.companion.is_some() || !near(e.body.pos) {
                continue;
            }
            if e.faction.is_some() && enemies.iter().any(|f| self.factions.hostile(e.faction, *f)) {
                order.push(fighter(Who::Npc(e.uid), Side::Party, true));
            }
        }
        // Initiative: d20 + DEX, the higher DEX first on a tie (into the roll log).
        for f in &mut order {
            f.dex = self.who_dex(f.who);
            f.key = self.who_key(f.who);
        }
        for f in &mut order {
            let roll = crate::srd::dice::die(20) as i32;
            f.init = roll + f.dex;
        }
        order.sort_by(|a, b| b.init.cmp(&a.init).then(b.dex.cmp(&a.dex)));
        let first = order.iter().map(|f| (f.who, f.init)).collect::<Vec<_>>();
        // Everyone onto the grid: tile centres.
        self.player.move_target = None;
        self.player.moving = false;
        let p = feet_spot(feet_tile(self.player.pos, ts), ts);
        self.player.pos = p;
        for f in &order {
            if let Some(i) = self.who_index(f.who) {
                let e = &mut self.loaded.entities[i];
                e.body.move_target = None;
                e.body.moving = false;
                e.body.pos = feet_spot(feet_tile(e.body.pos, ts), ts);
                self.occupy(i);
            }
        }
        let cursor = feet_tile(self.player.pos, ts);
        let mut c = Combat { round: 1, order, current: 0, turn: Turn::default(), helped: Vec::new(), cursor, aim: Aim::Move, walking: Vec::new(), ai_t: 0.4, log: Vec::new(), fast: self.fight_fast, over: false, shove_push: false, selected: 0, then_attack: None, reach: Default::default(), reach_key: None, mouse_last: Vec2::ZERO };
        c.log(crate::lang::tr_ui("Fight! Initiative:").into_owned());
        let names: Vec<String> = first.iter().map(|(w, init)| format!("{} {init}", self.who_name(*w))).collect();
        c.log(names.join(", "));
        self.fight = Some(Box::new(c));
        self.ui.toast(crate::lang::tr_ui("Fight!").into_owned());
        self.fire(crate::events::GameEvent::CombatStart, super::Source::Map);
        // (And the fighters' own triggers: a boss's words when its fight starts.)
        let uids: Vec<u32> = self.fight.as_ref().map_or(Vec::new(), |c| c.order.iter().filter_map(|f| if let Who::Npc(u) = f.who { Some(u) } else { None }).collect());
        for uid in uids {
            self.fire(crate::events::GameEvent::CombatStart, super::Source::Entity(uid));
        }
        self.begin_turn();
        true
    }

    /// Hostile characters near the player that aren't fighting yet join the fight going on (they
    /// came running, noticed it late): their initiative puts them at the end of the round.
    fn join_fight(&mut self) -> bool {
        let ts = self.ts();
        let here = self.player.pos;
        let new: Vec<usize> = (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                e.active && e.kind == EntityKind::Character && e.hostile && e.companion.is_none() && e.max_hp > 0 && e.hp > 0 && e.body.pos.distance(here) <= JOIN_TILES * ts && !self.fighting(e.uid)
            })
            .collect();
        if new.is_empty() {
            return false;
        }
        for i in new {
            let uid = self.loaded.entities[i].uid;
            let who = Who::Npc(uid);
            let dex = self.who_dex(who);
            let init = crate::srd::dice::die(20) as i32 + dex;
            let key = self.who_key(who);
            let name = self.who_name(who);
            let e = &mut self.loaded.entities[i];
            e.body.move_target = None;
            e.body.moving = false;
            e.body.pos = feet_spot(feet_tile(e.body.pos, ts), ts);
            self.occupy(i);
            let f = Fighter { who, key, side: Side::Enemy, init, dex, surprised: false, ai: true, saves: DeathSaves::default(), down: false, out: false, dodging: false, hidden: false, reacted: false, readied: false, grappling: None };
            if let Some(c) = &mut self.fight {
                c.order.push(f);
                c.log(format!("{name} {} ({init})", crate::lang::tr_ui("joins the fight")));
            }
            self.fire(crate::events::GameEvent::CombatStart, super::Source::Entity(uid));
        }
        true
    }

    /// The project has combat (the player character has HP).
    pub(super) fn combat_on(&self) -> bool {
        self.fight.is_some() || self.combat_enabled()
    }

    fn combat_enabled(&self) -> bool {
        self.player_max_hp() > 0
    }

    /// Ends the fight: party members still down get up on 1 HP, exploration resumes.
    pub(super) fn end_combat(&mut self) {
        let Some(c) = self.fight.take() else { return };
        for f in c.order.iter().filter(|f| f.side == Side::Party && f.down && !f.out) {
            self.revive(f.who, 1);
        }
        for f in &c.order {
            if let Some(list) = self.who_conditions_mut(f.who) {
                conditions::remove(list, Condition::Unconscious);
            }
        }
        self.fire(crate::events::GameEvent::CombatEnd, super::Source::Map);
        for f in c.order.iter().filter(|f| !f.out) {
            if let Who::Npc(uid) = f.who {
                self.fire(crate::events::GameEvent::CombatEnd, super::Source::Entity(uid));
            }
        }
        self.autosave_pending = true;
    }

    /// Whether the fight is decided: no enemy can fight on, or the whole party is down.
    fn check_over(&mut self) {
        let Some(c) = &self.fight else { return };
        let enemies = c.order.iter().any(|f| f.side == Side::Enemy && !f.out && !f.down);
        // (The party lost when nobody stands and those down are stable: their death saves are over.)
        let party = c.order.iter().any(|f| f.side == Side::Party && !f.out && (!f.down || !f.saves.stable));
        if !party {
            if let Some(c) = &mut self.fight {
                c.over = true;
            }
            self.fight = None;
            self.world.player_hp = Some(0);
            self.player_defeated();
            return;
        }
        if !enemies {
            if let Some(c) = &mut self.fight {
                c.over = true;
            }
        }
    }

    // ------------------------------------------------------------------------------ turns

    /// The current fighter's turn starts: its economy, conditions per round, death saves.
    fn begin_turn(&mut self) {
        let Some(c) = &mut self.fight else { return };
        let f = c.current().clone();
        c.turn = Turn { attacks_left: 1, ..Default::default() };
        c.aim = Aim::Move;
        c.helped.retain(|w| *w != f.who);
        if let Some(me) = c.fighter_mut(f.who) {
            me.dodging = false;
            me.reacted = false;
            me.readied = false;
        }
        if f.out {
            self.next_turn();
            return;
        }
        if f.surprised && c.round == 1 {
            let line = format!("{} {}", self.who_name(f.who), crate::lang::tr_ui("is surprised"));
            self.fight_log(line);
            self.next_turn();
            return;
        }
        // Conditions run a round: damage and healing per round fall now.
        let ticks = self.who_conditions_mut(f.who).map(|l| conditions::tick(l, conditions::ROUND)).unwrap_or_default();
        for (amount, t) in ticks {
            match t {
                Some(t) => self.combat_damage(f.who, amount, Some(t), false, None),
                None => self.combat_heal(f.who, amount),
            }
        }
        let Some(c) = &self.fight else { return };
        let me = c.fighter(f.who).cloned().unwrap_or(f);
        if me.out {
            self.next_turn();
            return;
        }
        if me.down {
            self.death_save(me.who);
            self.next_turn();
            return;
        }
        // A grapple ends when the grappler isn't next to its target any more.
        let grappled = conditions::has(self.who_conditions(me.who), Condition::Grappled);
        if grappled {
            let held = self.fight.as_ref().is_some_and(|c| c.order.iter().any(|g| g.grappling == Some(me.who) && !g.out && !g.down && self.who_tile(g.who).0.abs_diff(self.who_tile(me.who).0) <= 1 && self.who_tile(g.who).1.abs_diff(self.who_tile(me.who).1) <= 1));
            if !held {
                if let Some(l) = self.who_conditions_mut(me.who) {
                    conditions::remove(l, Condition::Grappled);
                }
            }
        }
        let speed = self.who_speed(me.who);
        let multi = if me.who == Who::Player { 1 } else { self.who_block(me.who).map_or(1, |b| b.multiattack.max(1)) };
        let act = conditions::can_act(self.who_conditions(me.who));
        let c = self.fight.as_mut().unwrap();
        c.turn.speed = speed;
        c.turn.attacks_left = multi;
        c.ai_t = if c.fast { 0.0 } else { 0.35 };
        if !act {
            let line = format!("{} {}", self.who_name(me.who), crate::lang::tr_ui("can't act"));
            self.fight_log(line);
            self.next_turn();
            return;
        }
        // Standing up from prone costs half the speed.
        if conditions::has(self.who_conditions(me.who), Condition::Prone) {
            if let Some(l) = self.who_conditions_mut(me.who) {
                conditions::remove(l, Condition::Prone);
            }
            let c = self.fight.as_mut().unwrap();
            c.turn.moved = c.turn.speed / 2;
        }
        let cursor = feet_tile(self.who_pos(me.who), self.ts());
        self.fight.as_mut().unwrap().cursor = cursor;
    }

    /// Ends the current turn: the next one in initiative order (a new round after the last).
    pub(super) fn next_turn(&mut self) {
        self.check_over();
        let Some(c) = &mut self.fight else { return };
        if c.over {
            self.end_combat();
            return;
        }
        c.walking.clear();
        c.current += 1;
        if c.current >= c.order.len() {
            c.current = 0;
            c.round += 1;
            let r = c.round;
            c.log(format!("{} {r}", crate::lang::tr_ui("Round")));
        }
        self.begin_turn();
    }

    /// Whether the current turn is the player's to play (the hero, a party member without AI).
    pub(super) fn players_turn(&self) -> bool {
        self.fight.as_ref().is_some_and(|c| !c.current().ai && c.walking.is_empty() && !c.over)
    }

    /// Once a frame while fighting: walks on screen, the AI's turns, the player's input.
    pub(super) fn update_turns(&mut self, dt: f32, key_consumed: bool) {
        if self.fight.is_none() {
            return;
        }
        if self.update_walk(dt) {
            return;
        }
        // A click on a foe out of reach: the attack once the walk there is over.
        if let Some(t) = self.fight.as_mut().and_then(|c| c.then_attack.take()) {
            if let Err(why) = self.act_on(&Aim::Attack, t) {
                self.ui.toast(crate::lang::tr_ui(why).into_owned());
            }
        }
        let over = self.fight.as_ref().is_some_and(|c| c.over);
        if over {
            self.end_combat();
            return;
        }
        let ai = self.fight.as_ref().is_some_and(|c| c.current().ai);
        if ai {
            let c = self.fight.as_mut().unwrap();
            c.ai_t -= dt;
            if c.ai_t > 0.0 {
                return;
            }
            c.ai_t = if c.fast { 0.0 } else { 0.3 };
            self.ai_step();
        } else if !key_consumed {
            self.turn_input();
        }
    }

    // ---------------------------------------------------------------------------- moving

    /// Tiles a creature may not enter: solid tiles, props, and the tiles of its foes (allies may
    /// be passed, not stopped on: `stop`).
    fn tile_closed(&self, who: Who, side: Side, t: TilePos, stop: bool, fighters: &[(Who, Side, TilePos)]) -> bool {
        if !self.loaded.map.in_bounds(t.0, t.1) || self.tile_blocks_npc(t.0, t.1) {
            return true;
        }
        if self.loaded.occupancy.has(t.0, t.1, super::occupancy::OCC_PROP) {
            return true;
        }
        let _ = who;
        fighters.iter().any(|(_, s, p)| *p == t && (*s != side || stop))
    }

    /// Where a creature can walk this turn: tile -> (feet it costs, the tile before it).
    pub(super) fn reachable(&self, who: Who) -> std::collections::HashMap<TilePos, (u32, TilePos)> {
        let mut out = std::collections::HashMap::new();
        let Some(c) = &self.fight else { return out };
        let Some(me) = c.fighter(who) else { return out };
        let left = c.turn.speed.saturating_sub(c.turn.moved);
        let start = self.who_tile(who);
        let fighters = self.fighter_tiles(who);
        out.insert(start, (0, start));
        let mut open = std::collections::BinaryHeap::new();
        open.push(std::cmp::Reverse((0u32, start)));
        while let Some(std::cmp::Reverse((cost, t))) = open.pop() {
            if out.get(&t).is_some_and(|(c, _)| *c < cost) {
                continue;
            }
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let n = (t.0 + d.0, t.1 + d.1);
                if self.tile_closed(who, me.side, n, false, &fighters) {
                    continue;
                }
                // Diagonals only past free corners (no clipping walls).
                if d.0 != 0 && d.1 != 0 && (self.tile_closed(who, me.side, (t.0 + d.0, t.1), false, &fighters) || self.tile_closed(who, me.side, (t.0, t.1 + d.1), false, &fighters)) {
                    continue;
                }
                let nc = cost + rules::step_cost(self.difficult_at(n));
                if nc > left {
                    continue;
                }
                if out.get(&n).is_none_or(|(c, _)| nc < *c) {
                    out.insert(n, (nc, t));
                    open.push(std::cmp::Reverse((nc, n)));
                }
            }
        }
        // Tiles with an ally on them can be passed, not ended on.
        out.retain(|t, _| *t == start || !fighters.iter().any(|(_, _, p)| p == t));
        out
    }

    /// A path from the reach map (excludes the start).
    pub(super) fn path_to(reach: &std::collections::HashMap<TilePos, (u32, TilePos)>, start: TilePos, goal: TilePos) -> Vec<TilePos> {
        let mut path = Vec::new();
        let mut t = goal;
        while t != start {
            // (A reach map of another start tile leads nowhere: no path.)
            if path.len() > reach.len() {
                return Vec::new();
            }
            path.push(t);
            match reach.get(&t) {
                Some((_, prev)) => t = *prev,
                None => return Vec::new(),
            }
        }
        path.reverse();
        path
    }

    /// Difficult terrain: the painted ground's material says so.
    pub(super) fn difficult_at(&self, t: TilePos) -> bool {
        let m = &self.loaded.map;
        let per = (crate::project::TILE_SIZE / crate::ground::CELL) as i32;
        let v = m.ground.get(t.0 * per + per / 2, t.1 * per + per / 2);
        m.material_of(v).and_then(|id| self.project.db.material(id)).is_some_and(|d| d.difficult)
    }

    /// Starts walking the current fighter along `path` (paid as it goes).
    pub(super) fn walk(&mut self, path: Vec<TilePos>) {
        if let Some(c) = &mut self.fight {
            c.walking = path;
        }
    }

    /// The walk on screen, a tile at a time (the body's move target walks it there): each step
    /// costs its feet and may draw opportunity attacks. Returns true while walking.
    fn update_walk(&mut self, _dt: f32) -> bool {
        let Some(c) = &self.fight else { return false };
        let who = c.current().who;
        let side = c.current().side;
        let fast = c.fast;
        // Still on the way to the last tile.
        let stepping = match self.who_index(who) {
            None => self.player.move_target.is_some(),
            Some(i) => self.loaded.entities[i].body.move_target.is_some(),
        };
        if stepping {
            return true;
        }
        let Some(c) = &mut self.fight else { return false };
        if c.walking.is_empty() {
            return false;
        }
        let next = c.walking.remove(0);
        let from = self.who_tile(who);
        let cost = rules::step_cost(self.difficult_at(next));
        let disengaged = self.fight.as_ref().is_some_and(|c| c.turn.disengaged);
        // Leaving a foe's reach without Disengage: its opportunity attack (a reaction).
        if !disengaged {
            let foes: Vec<Who> = self.fight.as_ref().map_or(Vec::new(), |c| c.order.iter().filter(|f| f.side != side && !f.out && !f.down && !f.reacted).map(|f| f.who).collect());
            for foe in foes {
                let ft = self.who_tile(foe);
                let near = |t: TilePos| t.0.abs_diff(ft.0) <= 1 && t.1.abs_diff(ft.1) <= 1;
                if near(from) && !near(next) && conditions::can_act(self.who_conditions(foe)) {
                    if let Some(spec) = self.melee_spec(foe) {
                        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(foe)) {
                            f.reacted = true;
                        }
                        let name = self.who_name(foe);
                        self.fight_log(format!("{name}: {}", crate::lang::tr_ui("opportunity attack")));
                        self.make_attack(foe, who, &spec);
                    }
                }
            }
        }
        // (A hit may have put it down.)
        let still = self.fight.as_ref().is_some_and(|c| c.fighter(who).is_some_and(|f| !f.down && !f.out)) && conditions::rules(self.who_conditions(who)).speed > 0.0;
        if !still {
            if let Some(c) = &mut self.fight {
                c.walking.clear();
                c.then_attack = None;
            }
            return true;
        }
        let ts = self.ts();
        let to = feet_spot(next, ts);
        match self.who_index(who) {
            None if fast => self.player.pos = to,
            None => self.player.move_target = Some(to),
            Some(i) => {
                let e = &mut self.loaded.entities[i];
                if fast {
                    e.body.pos = to;
                } else {
                    e.body.move_target = Some(to);
                }
                self.occupy(i);
            }
        }
        if let Some(c) = &mut self.fight {
            c.turn.moved += cost;
        }
        // Entering a readied foe's reach: its attack.
        let readied: Vec<Who> = self.fight.as_ref().map_or(Vec::new(), |c| c.order.iter().filter(|f| f.side != side && f.readied && !f.reacted && !f.out && !f.down).map(|f| f.who).collect());
        for foe in readied {
            let ft = self.who_tile(foe);
            if next.0.abs_diff(ft.0) <= 1 && next.1.abs_diff(ft.1) <= 1 {
                if let Some(spec) = self.melee_spec(foe) {
                    if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(foe)) {
                        f.reacted = true;
                        f.readied = false;
                    }
                    self.make_attack(foe, who, &spec);
                }
            }
        }
        true
    }

    /// Whether character `uid` takes part in the fight (its turns move it).
    pub(super) fn fighting(&self, uid: u32) -> bool {
        self.fight.as_ref().is_some_and(|c| c.order.iter().any(|f| f.who == Who::Npc(uid) && !f.out))
    }

    // --------------------------------------------------------------------------- attacks

    /// The attacks a combatant can make: the hero's weapon, a stat block's attacks.
    pub(super) fn attack_specs(&self, who: Who) -> Vec<AttackSpec> {
        match who {
            Who::Player => {
                let w = self.attack_weapon();
                vec![self.hero_spec(&w, false)]
            }
            _ => {
                let Some(b) = self.who_block(who) else { return Vec::new() };
                let effect = self.who_index(who).and_then(|i| self.touch_effect(i));
                let projectile = self.who_index(who).and_then(|i| self.loaded.entities[i].weapon.as_ref().and_then(|w| w.ranged.as_ref().map(|r| r.projectile.clone())));
                b.attacks
                    .iter()
                    .flat_map(|a| {
                        let mut v = Vec::new();
                        if a.reach_ft > 0 {
                            v.push(AttackSpec { name: a.name.clone(), bonus: a.bonus, dice: a.dice.clone(), damage_type: a.damage_type, ranged: false, reach: (a.reach_ft / 5).max(1) as i32, range: 0, long: 0, crit_on: 20, effect: effect.clone(), projectile: None });
                        }
                        if a.range_ft > 0 {
                            v.push(AttackSpec { name: a.name.clone(), bonus: a.bonus, dice: a.dice.clone(), damage_type: a.damage_type, ranged: true, reach: 0, range: (a.range_ft / 5).max(1) as i32, long: (a.long_ft.max(a.range_ft) / 5).max(1) as i32, crit_on: 20, effect: None, projectile: projectile.clone() });
                        }
                        v
                    })
                    .collect()
            }
        }
    }

    /// The hero's attack with weapon `w` (`offhand`: no ability modifier on the damage).
    fn hero_spec(&self, w: &crate::defs::Weapon, offhand: bool) -> AttackSpec {
        let ranged = w.ranged.is_some();
        let ability = if ranged { Ability::Dex } else { Ability::Str };
        let m = self.world.abilities.modifier(ability);
        let prof = crate::srd::abilities::proficiency_bonus(self.world.player_level());
        let dmg_mod = if offhand && m > 0 { 0 } else { m };
        let dice = if dmg_mod == 0 { w.damage_dice() } else { format!("{}{:+}", w.damage_dice(), dmg_mod) };
        let (range, long) = w.ranged.as_ref().map_or((0, 0), |r| (r.range.round() as i32, if r.long_range > 0.0 { r.long_range } else { r.range * 2.0 }.round() as i32));
        AttackSpec {
            name: String::new(),
            bonus: m + prof,
            dice,
            damage_type: w.damage_type,
            ranged,
            reach: if ranged { 0 } else { w.reach.round().max(1.0) as i32 },
            range,
            long,
            crit_on: if w.crit >= 0.1 { 19 } else { 20 },
            effect: w.effect.clone(),
            projectile: w.ranged.as_ref().map(|r| r.projectile.clone()),
        }
    }

    /// A combatant's melee attack (opportunity attacks, readied ones).
    pub(super) fn melee_spec(&self, who: Who) -> Option<AttackSpec> {
        self.attack_specs(who).into_iter().find(|s| !s.ranged)
    }

    /// The attack `spec` can make on `target` from where `who` stands, or why not.
    pub(super) fn can_attack(&self, who: Who, target: Who, spec: &AttackSpec) -> Result<(), &'static str> {
        let (a, t) = (self.who_tile(who), self.who_tile(target));
        let dist = a.0.abs_diff(t.0).max(a.1.abs_diff(t.1)) as i32;
        if spec.ranged {
            if dist > spec.long.max(spec.range) {
                return Err("Out of range");
            }
            if self.cover(who, target) == Cover::Total {
                return Err("No line of sight");
            }
        } else if dist > spec.reach.max(1) {
            return Err("Out of reach");
        }
        Ok(())
    }

    /// The cover `target` has against `who`: lines from the attacker's tile centre to the
    /// target tile's corners, blocked by walls, tall props and other creatures.
    pub(super) fn cover(&self, who: Who, target: Who) -> Cover {
        self.cover_from(self.who_tile(who), who, target)
    }

    /// The cover `target` would have against `who` standing on tile `a`.
    pub(super) fn cover_from(&self, a: TilePos, who: Who, target: Who) -> Cover {
        let ts = self.ts();
        let t = self.who_tile(target);
        if a.0.abs_diff(t.0) <= 1 && a.1.abs_diff(t.1) <= 1 {
            return Cover::None;
        }
        let from = vec2((a.0 as f32 + 0.5) * ts, (a.1 as f32 + 0.5) * ts);
        let creatures: Vec<TilePos> = self.fighter_tiles(who).into_iter().filter(|(w, _, _)| *w != target).map(|(_, _, p)| p).collect();
        let inset = 0.15 * ts;
        let corners = [(inset, inset), (ts - inset, inset), (inset, ts - inset), (ts - inset, ts - inset)];
        // Walls and tall props block lines; creatures in the way give half cover at most (the SRD).
        let (mut walls, mut creature) = (0, false);
        for (dx, dy) in corners {
            let to = vec2(t.0 as f32 * ts + dx, t.1 as f32 * ts + dy);
            let (mut wall, mut body) = (false, false);
            for p in crate::grid::line_tiles(from, to, ts).filter(|p| *p != a && *p != t) {
                wall |= self.blocks_shot(p.0, p.1) || self.loaded.occupancy.has(p.0, p.1, super::occupancy::OCC_HIGH);
                body |= creatures.contains(&p);
            }
            if wall {
                walls += 1;
            } else if body {
                creature = true;
            }
        }
        let cover = Cover::of_lines(walls, corners.len());
        if creature && cover < Cover::Half {
            Cover::Half
        } else {
            cover
        }
    }

    /// The situation of an attack (for advantage and disadvantage).
    fn situation(&self, who: Who, target: Who, spec: &AttackSpec) -> AttackSituation {
        let (a, t) = (self.who_tile(who), self.who_tile(target));
        let dist = a.0.abs_diff(t.0).max(a.1.abs_diff(t.1)) as i32;
        let me = conditions::rules(self.who_conditions(who));
        let them = conditions::rules(self.who_conditions(target));
        let c = self.fight.as_ref();
        let my_side = c.and_then(|c| c.fighter(who)).map(|f| f.side);
        let enemy_adjacent = c.is_some_and(|c| {
            c.order.iter().any(|f| Some(f.side) != my_side && !f.out && !f.down && {
                let p = self.who_tile(f.who);
                p.0.abs_diff(a.0) <= 1 && p.1.abs_diff(a.1) <= 1 && conditions::can_act(self.who_conditions(f.who))
            })
        });
        AttackSituation {
            ranged: spec.ranged,
            adjacent: dist <= 1,
            enemy_adjacent,
            long_range: spec.ranged && dist > spec.range,
            attacker_disadvantage: me.attack_disadvantage,
            attacker_unseen: me.invisible || c.and_then(|c| c.fighter(who)).is_some_and(|f| f.hidden),
            target_prone: conditions::has(self.who_conditions(target), Condition::Prone),
            target_helpless: conditions::has(self.who_conditions(target), Condition::Restrained) || them.str_dex_saves_fail || them.blinded,
            target_unseen: them.invisible,
            target_dodging: c.and_then(|c| c.fighter(target)).is_some_and(|f| f.dodging) && conditions::can_act(self.who_conditions(target)),
            helped: c.is_some_and(|c| c.helped.contains(&target)),
        }
    }

    /// The chance (0-1) `spec` hits `target` (the UI).
    pub(super) fn hit_chance(&self, who: Who, target: Who, spec: &AttackSpec) -> f32 {
        let mode = rules::odds(&self.situation(who, target, spec)).mode();
        let ac = self.who_ac(target) + self.cover(who, target).ac_bonus();
        rules::AttackRoll::chance(spec.bonus, ac, mode)
    }

    /// Rolls an attack of `who` on `target` and applies its damage and condition.
    pub(super) fn make_attack(&mut self, who: Who, target: Who, spec: &AttackSpec) -> bool {
        let sit = self.situation(who, target, spec);
        let mode = rules::odds(&sit).mode();
        let cover = self.cover(who, target);
        let ac = self.who_ac(target) + cover.ac_bonus();
        let auto_crit = sit.adjacent && conditions::rules(self.who_conditions(target)).hits_crit;
        let mut roll = rules::attack(spec.bonus, ac, mode, spec.crit_on, auto_crit);
        // Autoplay `crit always|never`: the hits of the player's side.
        let party = self.fight.as_ref().and_then(|c| c.fighter(who)).is_some_and(|f| f.side == Side::Party);
        if let (true, Some(always)) = (party && roll.hit, self.juice.crit_mode) {
            roll.crit = always;
        }
        // Attacking gives a hidden attacker away; Help is used up.
        if let Some(c) = &mut self.fight {
            if let Some(f) = c.fighter_mut(who) {
                f.hidden = false;
            }
            c.helped.retain(|w| *w != target);
        }
        let (aname, tname) = (self.who_name(who), self.who_name(target));
        let weapon = if spec.name.is_empty() { String::new() } else { format!(" ({})", spec.name) };
        let result = if roll.crit {
            crate::lang::tr_ui("critical hit").into_owned()
        } else if roll.hit {
            crate::lang::tr_ui("hit").into_owned()
        } else {
            crate::lang::tr_ui("miss").into_owned()
        };
        let cover_text = if cover == Cover::None { String::new() } else { format!(" [{}]", crate::lang::tr_ui(cover.id())) };
        let line = format!("{aname} > {tname}{weapon}: {}{cover_text}: {result}", roll.text());
        let rec = crate::srd::RollRecord { label: format!("{aname}: {}", crate::lang::tr_ui("attack")), id: "attack".into(), dice: format!("d20 ({})", roll.other.map_or(roll.d20.to_string(), |o| format!("{}/{o}", roll.d20))), modifier: spec.bonus as i64, total: roll.total as i64, dc: ac, success: Some(roll.hit), natural: if roll.d20 == 20 || roll.d20 == 1 { roll.d20 as u8 } else { 0 }, mode };
        self.world.log_roll(rec);
        if let Some(c) = &mut self.fight {
            c.log(line);
        }
        // Face the target; a ranged attack flies.
        let (from, to) = (self.who_pos(who), self.who_pos(target));
        let d = to - from;
        if let Some(f) = Facing::from_vec(d.x, d.y) {
            match self.who_index(who) {
                None => self.player.facing = f,
                Some(i) => self.loaded.entities[i].body.facing = f,
            }
        }
        if let Some(p) = &spec.projectile {
            let h = self.ts() * 0.5;
            self.fly_visual(from - vec2(0.0, h), to - vec2(0.0, h), p.clone());
        }
        self.audio.play(&self.project.config.sfx.attack);
        if !roll.hit {
            let head = to - vec2(0.0, self.ts() * 1.1);
            self.miss_number(head);
            return false;
        }
        let dice = if roll.crit { rules::crit_dice(&spec.dice) } else { spec.dice.clone() };
        let amount = crate::srd::Expr::parse(&dice).map(|e| e.roll(&crate::srd::dice::NoMods).total.max(1)).unwrap_or(1) as i32;
        self.combat_damage(target, amount, spec.damage_type, roll.crit, Some(who));
        if let Some(fx) = &spec.effect {
            match target {
                Who::Player => self.apply_player_effect(fx),
                Who::Npc(_) => {
                    if let Some(i) = self.who_index(target) {
                        self.apply_entity_effect(i, fx);
                    }
                }
            }
        }
        true
    }

    // ----------------------------------------------------------------------- damage, 0 HP

    /// Damage in a fight: through the target's resistances; the hero and party members fall
    /// at 0 HP (death saves), others die.
    pub(super) fn combat_damage(&mut self, target: Who, amount: i32, t: Option<DamageType>, crit: bool, by: Option<Who>) {
        let defenses = match target {
            Who::Player => self.player_defenses(),
            _ => self.who_index(target).and_then(|i| self.stat_block(&self.loaded.entities[i]).map(|s| s.defenses())).unwrap_or_default(),
        };
        let mut n = defenses.apply(amount, t);
        // Script hook: the damage can change (0 = none).
        if self.hooked("damage") && n > 0 {
            use crate::hooks;
            let attacker = match by {
                Some(Who::Player) => "player".to_string(),
                Some(w) => self.who_name(w),
                None => String::new(),
            };
            let (name, def) = match target {
                Who::Player => ("player".to_string(), String::new()),
                Who::Npc(_) => self.who_index(target).map_or((String::new(), String::new()), |i| (self.loaded.entities[i].name.clone(), self.loaded.entities[i].def.clone())),
            };
            let m = self.hook("damage", hooks::map([("attacker", attacker.into()), ("target", name.into()), ("target_def", def.into()), ("amount", (n as i64).into()), ("crit", crit.into())]));
            n = hooks::int(&m, "amount", n as i64).clamp(0, i32::MAX as i64) as i32;
        }
        let party = self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| f.side == Side::Party);
        let down = self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| f.down);
        let head = self.who_pos(target) - vec2(0.0, self.ts() * 1.1);
        if down {
            // Damage while down: a failed death save (a crit, two).
            let out = self.fight.as_mut().and_then(|c| c.fighter_mut(target)).map(|f| f.saves.hit(crit));
            if out == Some(DeathOutcome::Dead) {
                self.dies(target);
            }
            return;
        }
        if n <= 0 {
            return;
        }
        let kind = if party { NumberKind::Hurt } else if crit { NumberKind::Crit } else { NumberKind::Hit };
        self.damage_number(head, n, kind);
        self.audio.play(&self.project.config.sfx.hit);
        match target {
            Who::Player => {
                let hp = (self.hp() - n).max(0);
                self.world.player_hp = Some(hp);
                self.hurt_feedback();
                if hp == 0 {
                    self.falls(target);
                }
            }
            Who::Npc(_) => {
                let Some(i) = self.who_index(target) else { return };
                let e = &mut self.loaded.entities[i];
                let before = e.hp;
                e.hp = (e.hp - n).max(0);
                e.flash = super::juice::HIT_FLASH;
                e.hurt = 0.25;
                if e.bar <= 0.0 && e.max_hp > 0 {
                    e.bar_lag = before.max(0) as f32 / e.max_hp as f32;
                }
                e.bar = super::juice::BAR_TIME;
                if e.hp == 0 {
                    if party {
                        self.falls(target);
                    } else {
                        self.dies(target);
                    }
                } else {
                    self.hp_dropped(i, before);
                }
            }
        }
    }

    /// Healing in a fight: a creature at 0 HP gets up.
    pub(super) fn combat_heal(&mut self, who: Who, amount: i32) {
        if amount <= 0 {
            return;
        }
        match who {
            Who::Player => {
                let hp = (self.hp() + amount).min(self.player_max_hp());
                self.world.player_hp = Some(hp);
            }
            Who::Npc(_) => {
                if let Some(i) = self.who_index(who) {
                    let e = &mut self.loaded.entities[i];
                    e.hp = (e.hp + amount).min(e.max_hp);
                }
            }
        }
        let down = self.fight.as_ref().and_then(|c| c.fighter(who)).is_some_and(|f| f.down);
        if down {
            self.revive(who, 0);
        }
    }

    /// The hero or a party member drops to 0 HP: unconscious, death saves from its next turn.
    fn falls(&mut self, who: Who) {
        if let Some(l) = self.who_conditions_mut(who) {
            conditions::apply(l, &ConditionSpec::new(Condition::Unconscious, 0.0));
        }
        let name = self.who_name(who);
        if let Some(c) = &mut self.fight {
            if let Some(f) = c.fighter_mut(who) {
                f.down = true;
                f.saves = DeathSaves::default();
            }
            c.log(format!("{name} {}", crate::lang::tr_ui("falls unconscious")));
        }
        self.check_over();
    }

    /// Back up from 0 HP (healed, a natural 20, the fight's end): at least `hp` HP.
    pub(super) fn revive(&mut self, who: Who, hp: i32) {
        if let Some(l) = self.who_conditions_mut(who) {
            conditions::remove(l, Condition::Unconscious);
        }
        match who {
            Who::Player => {
                let now = self.hp().max(hp);
                self.world.player_hp = Some(now);
            }
            Who::Npc(_) => {
                if let Some(i) = self.who_index(who) {
                    let e = &mut self.loaded.entities[i];
                    e.hp = e.hp.max(hp);
                }
            }
        }
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
            f.down = false;
            f.saves = DeathSaves::default();
        }
    }

    /// Out of the fight for good: characters are defeated (loot, Defeat triggers, XP); the hero
    /// dies (game over), a party member dies.
    fn dies(&mut self, who: Who) {
        let name = self.who_name(who);
        if let Some(c) = &mut self.fight {
            if let Some(f) = c.fighter_mut(who) {
                f.out = true;
                f.down = false;
            }
            c.log(format!("{name} {}", crate::lang::tr_ui("dies")));
        }
        match who {
            Who::Player => {
                self.world.player_hp = Some(0);
                self.fight = None;
                self.player_defeated();
            }
            Who::Npc(_) => {
                if let Some(i) = self.who_index(who) {
                    self.loaded.entities[i].hp = 0;
                    if self.loaded.entities[i].companion.is_some() {
                        // (The project's rule for companions at 0 HP: they lie there, go home or get up.)
                        self.knock_down(i);
                    } else {
                        self.defeat(i);
                    }
                }
                self.check_over();
            }
        }
    }

    /// The current fighter is down: a death saving throw (into the roll log).
    fn death_save(&mut self, who: Who) {
        let d = crate::srd::dice::die(20);
        let name = self.who_name(who);
        let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) else { return };
        if f.saves.stable {
            return;
        }
        let out = f.saves.roll(d);
        let (s, fl) = (f.saves.successes, f.saves.failures);
        self.world.log_roll(crate::srd::RollRecord { label: format!("{name}: {}", crate::lang::tr_ui("death save")), id: "death-save".into(), dice: format!("d20 ({d})"), modifier: 0, total: d as i64, dc: 10, success: Some(d >= 10), natural: if d == 20 || d == 1 { d as u8 } else { 0 }, mode: Mode::Normal });
        if let Some(c) = &mut self.fight {
            c.log(format!("{name}: {} {d} ({s}/{fl})", crate::lang::tr_ui("death save")));
        }
        match out {
            DeathOutcome::Up => self.revive(who, 1),
            DeathOutcome::Dead => self.dies(who),
            DeathOutcome::Stable => {
                if let Some(c) = &mut self.fight {
                    c.log(format!("{name} {}", crate::lang::tr_ui("is stable")));
                }
                self.check_over();
            }
            DeathOutcome::Going => {}
        }
    }

    // -------------------------------------------------------------------------- status

    /// `combat=off`, or `round:R:who:moved/speed:A|B[order …]{name:hp/max …}`: the round, whose
    /// turn, its movement and what of the action (A) and bonus action (B) is left.
    pub(super) fn combat_status(&self) -> String {
        let Some(c) = &self.fight else { return "off".into() };
        let f = c.current();
        let order: Vec<String> = c
            .order
            .iter()
            .map(|f| {
                let (hp, max) = self.who_hp(f.who);
                let mark = if f.out { "x" } else if f.down { "!" } else { "" };
                let side = if f.side == Side::Party { "+" } else { "-" };
                format!("{side}{}{mark}:{hp}/{max}", self.who_key(f.who))
            })
            .collect();
        let aim = match &c.aim {
            Aim::Move => String::new(),
            Aim::Attack => ":aim=attack".into(),
            Aim::OffHand => ":aim=offhand".into(),
            Aim::Help => ":aim=help".into(),
            Aim::Grapple => ":aim=grapple".into(),
            Aim::Shove => ":aim=shove".into(),
            Aim::Cast(s) => format!(":aim=cast:{s}"),
        };
        format!(
            "{}:{}:{}/{}:{}{}{}{aim}@{},{}[{}]",
            c.round,
            self.who_key(f.who),
            c.turn.moved,
            c.turn.speed,
            if c.turn.action { "-" } else { "A" },
            if c.turn.bonus { "-" } else { "B" },
            if f.ai { ":ai" } else { "" },
            c.cursor.0,
            c.cursor.1,
            order.join(" ")
        )
    }

    /// `death_saves=` the downed party members: `name:successes/failures[:stable]`.
    pub(super) fn death_saves_status(&self) -> Vec<String> {
        let Some(c) = &self.fight else { return Vec::new() };
        c.order.iter().filter(|f| f.down).map(|f| format!("{}:{}/{}{}", self.who_key(f.who), f.saves.successes, f.saves.failures, if f.saves.stable { ":stable" } else { "" })).collect()
    }

    /// Autoplay `turn …`: plays the current turn from a script (`move X Y`, `attack NAME`,
    /// `action dash|disengage|dodge|hide|ready|search`, `help|grapple|offhand NAME`,
    /// `shove NAME prone|push`, `cast SPELL [NAME]`, `use ITEM`, `end`, `ai NAME on|off`,
    /// `fast [on|off]`, `start`). Returns what happened (or why not).
    pub fn turn_command(&mut self, line: &str) -> String {
        let a: Vec<&str> = line.split_whitespace().collect();
        let arg = |k: usize| a.get(k).copied().unwrap_or("");
        match arg(0) {
            "start" => return if self.start_combat(false) { "fight".into() } else { "no fight".into() },
            "fast" => {
                let on = arg(1) != "off";
                self.fight_fast = on;
                if let Some(c) = &mut self.fight {
                    c.fast = on;
                }
                return format!("fast={on}");
            }
            "ai" => {
                let Some(i) = self.loaded.entities.iter().position(|e| e.companion.is_some() && (e.name.eq_ignore_ascii_case(arg(1)) || e.def == arg(1))) else { return format!("no companion {}", arg(1)) };
                self.set_companion_ai(i, arg(2) != "off");
                return "ok".into();
            }
            _ => {}
        }
        if self.fight.is_none() {
            return "no fight".into();
        }
        if !self.players_turn() {
            return "not the player's turn".into();
        }
        self.refresh_reach();
        let target = |g: &Game, k: usize| g.who_by_key(arg(k)).ok_or_else(|| format!("no fighter {}", arg(k)));
        let done = |r: Result<(), &'static str>| match r {
            Ok(()) => "ok".to_string(),
            Err(e) => e.to_string(),
        };
        match arg(0) {
            "move" => {
                let t = (arg(1).parse().unwrap_or(-1), arg(2).parse().unwrap_or(-1));
                let c = self.fight.as_ref().unwrap();
                if !c.reach.contains_key(&t) || self.who_at(t).is_some() {
                    return "Too far".into();
                }
                let here = self.who_tile(c.current().who);
                let path = Self::path_to(&c.reach, here, t);
                self.walk(path);
                "ok".into()
            }
            "attack" => match target(self, 1) {
                Ok(t) => {
                    let tile = self.who_tile(t);
                    let c = self.fight.as_mut().unwrap();
                    c.aim = Aim::Move;
                    c.cursor = tile;
                    self.confirm_cursor();
                    "ok".into()
                }
                Err(e) => e,
            },
            "action" | "end" => {
                let id = if arg(0) == "end" { "end" } else { arg(1) };
                match TurnAct::parse(id) {
                    Some(act) => done(self.take_act(act)),
                    None => format!("no action {id}"),
                }
            }
            "help" | "grapple" | "offhand" | "shove" => match target(self, 1) {
                Ok(t) => {
                    let aim = match arg(0) {
                        "help" => Aim::Help,
                        "grapple" => Aim::Grapple,
                        "offhand" => Aim::OffHand,
                        _ => Aim::Shove,
                    };
                    if let Some(c) = &mut self.fight {
                        c.shove_push = arg(2) == "push";
                    }
                    done(self.act_on(&aim, t))
                }
                Err(e) => e,
            },
            "cast" => {
                let me = self.fight.as_ref().unwrap().current().who;
                let t = if arg(2).is_empty() { Ok(me) } else { target(self, 2) };
                match t {
                    Ok(t) => done(self.act_on(&Aim::Cast(arg(1).to_string()), t)),
                    Err(e) => e,
                }
            }
            "use" => {
                if self.world.item_count(arg(1)) == 0 {
                    return format!("no {}", arg(1));
                }
                self.use_item_by_id(arg(1));
                "ok".into()
            }
            other => format!("unknown turn command {other}"),
        }
    }

    /// Who plays fighter `uid`'s turns (a party member's AI toggle).
    pub(super) fn set_fighter_ai(&mut self, uid: u32, on: bool) {
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(Who::Npc(uid))) {
            f.ai = on;
        }
    }

    /// The name scripts and status use: "player", a character's name, else its definition id.
    pub(super) fn who_key(&self, who: Who) -> String {
        match who {
            Who::Player => "player".into(),
            Who::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map(|e| if e.name.is_empty() { e.def.clone() } else { e.name.clone() }).or_else(|| self.fight.as_ref()?.fighter(who).map(|f| f.key.clone())).unwrap_or_else(|| "?".into()),
        }
    }

    /// A combatant by the name `status` gives it ("player", a name, a definition id).
    pub(super) fn who_by_key(&self, key: &str) -> Option<Who> {
        let c = self.fight.as_ref()?;
        c.order.iter().filter(|f| !f.out).map(|f| f.who).find(|w| self.who_key(*w).eq_ignore_ascii_case(key))
    }
}
