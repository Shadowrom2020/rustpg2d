//! Turn-based combat (the SRD's): a fight starts where hostiles notice the party or the hero
//! attacks, everyone near rolls initiative and acts in turns on the tile grid (5 ft a tile):
//! movement up to the speed, one action, one bonus action, one reaction a round. Attacks roll
//! against armor class ([`crate::srd::combat`]), creatures at 0 HP die, the hero and party
//! members fall unconscious and make death saving throws. Exploration (free movement) resumes
//! when no enemy is left. `ai.rs` plays the characters' turns, `ui.rs` draws the bars, overlays
//! and the log and reads the player's input.

mod actions;
mod ai;
mod ai_cast;
mod casting;
mod features;
mod monster_actions;
mod reactions;
pub(in crate::runtime) mod menu;
mod throwing;
mod ui;
pub(in crate::runtime) mod vision;

pub(super) use actions::TurnAct;
pub(super) use casting::CastAt;

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
    /// A readied spell (its id and slot, paid): cast at the first foe that comes within its
    /// range, while its concentration holds.
    pub readied_spell: Option<(String, u32)>,
    /// Who it is grappling.
    pub grappling: Option<Who>,
    /// What its class features keep going (rage, inspiration, temporary hit points…).
    pub fx: features::ClassFx,
    /// The weapons it threw and hasn't picked up again.
    pub thrown: Vec<throwing::Thrown>,
    /// A monster's recharge, uses, legendary points, regeneration (`monster_actions.rs`).
    pub mon: monster_actions::MonsterFx,
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
    /// A spell at a slot level (0 = a cantrip).
    Cast(String, u32),
    /// A class action (its id).
    Feature(String),
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
    /// The free object interaction is used (drawing or stowing a weapon): the next takes the
    /// action.
    pub interacted: bool,
}

/// An attack, ready to roll.
#[derive(Clone, Debug, Default)]
pub(super) struct AttackSpec {
    pub name: String,
    /// The poison item coating the weapon or ammunition (`poisons.rs`; "" = none).
    pub poison: String,
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
    /// Brutal Critical: more weapon dice on a critical melee hit.
    pub brutal: u32,
    /// Great Weapon Fighting: damage dice of 1 or 2 are rolled again.
    pub reroll_low: bool,
    /// The ability it uses (rage's damage: STR); `None` = unknown (a monster's).
    pub ability: Option<Ability>,
    /// A spell attack (no weapon riders).
    pub spell: bool,
    /// One of Flurry of Blows' strikes (Open Hand Technique).
    pub flurry: bool,
    /// A finesse weapon (Sneak Attack).
    pub finesse: bool,
    /// A loading weapon: one attack with it per action.
    pub loading: bool,
    /// Rolled with disadvantage (a heavy weapon in Small hands).
    pub disadvantage: bool,
    /// A melee weapon thrown (a spear, a dagger, a javelin): the AI walks up and strikes instead
    /// when it can reach a foe this turn.
    pub thrown: bool,
    /// More damage of other types on a hit (a monster's: a dragon's bite's fire).
    pub extra: Vec<crate::srd::DamagePart>,
    /// What a hit does besides damage (a monster's: a wolf's bite knocks prone).
    pub riders: Vec<crate::srd::HitRider>,
    /// What makes the hit: a plain weapon, a silvered one, magic (resistances to nonmagical
    /// attacks).
    pub hit: crate::srd::HitKind,
    /// A net: a hit restrains (Large or smaller, escape DC 10), no damage; one attack with it per
    /// action.
    pub net: bool,
    /// A lance: disadvantage against a target within 5 ft.
    pub lance: bool,
}

/// A hit that would drop a foe to 0 HP, held while the player chooses: kill or knock out.
#[derive(Clone, Debug)]
pub(super) struct PendingBlow {
    pub who: Who,
    pub target: Who,
    /// Its damage parts (rolled), before the target's defenses.
    pub parts: Vec<(i32, Option<DamageType>)>,
    pub crit: bool,
    pub hit: crate::srd::HitKind,
}

/// Where a creature can walk this turn: the tiles it may end on (tile -> feet it costs, the tile
/// before it; the map the overlay, clicks and the AI read) and the allies' tiles it only passes.
/// (Until 0.31 a path through an ally's tile was lost: the walk didn't happen.)
#[derive(Clone, Debug, Default)]
pub(super) struct Reach {
    pub tiles: std::collections::HashMap<TilePos, (u32, TilePos)>,
    pub passed: std::collections::HashMap<TilePos, (u32, TilePos)>,
}

impl std::ops::Deref for Reach {
    type Target = std::collections::HashMap<TilePos, (u32, TilePos)>;
    fn deref(&self) -> &Self::Target {
        &self.tiles
    }
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
    /// Items to pick up once the walk next to them ends (a right-click on a pile out of reach).
    pub then_pick_up: Option<TilePos>,
    /// The current fighter's reachable tiles (the overlay, clicks), and what they were for.
    pub reach: Reach,
    /// The choice window over the action bar (Cast, Class, Attack: strike or throw).
    pub menu: Option<menu::FightMenu>,
    /// How the aimed Attack uses a weapon that can be thrown too: Some(true) = throw it,
    /// Some(false) or None = strike.
    pub attack_mode: Option<bool>,
    pub reach_key: Option<(usize, u32, u32, u32, TilePos, u64)>,
    /// The mouse's last screen position (the cursor follows it when it moves).
    pub mouse_last: Vec2,
    /// Once-per-turn features used in the current turn (Sneak Attack…), by whom.
    pub once: Vec<(Who, &'static str)>,
    /// A scroll being read: its spell is aimed (used up when cast).
    pub scroll: Option<String>,
    /// A spell cast from an item being aimed (its charges are spent when cast).
    pub item_cast: Option<crate::runtime::magic_items::ItemCast>,
    /// A finishing blow waiting for the player's Kill / Knock out.
    pub pending_blow: Option<PendingBlow>,
    /// An attack held for the hero's reaction (the Reaction window).
    pub pending_react: Option<reactions::PendingReact>,
    /// The damage being dealt knocks out instead of killing.
    pub knocking: bool,
    /// The hero's ammunition shot in this fight (item, count): half is found again after it.
    pub shot: Vec<(String, u32)>,
    /// The AI shoved or grappled these (target, round): once per target and round.
    pub ai_wrestled: Vec<(Who, u32)>,
}

/// What a fight log keeps.
const LOG_LEN: usize = 40;

impl Combat {
    pub(super) fn current(&self) -> &Fighter {
        &self.order[self.current]
    }

    pub(in crate::runtime) fn fighter(&self, who: Who) -> Option<&Fighter> {
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

    /// How many tiles across `who` takes (1; Large 2, Huge 3, Gargantuan 4).
    pub(super) fn space_n(&self, who: Who) -> i32 {
        rules::space_tiles(self.who_size(who))
    }

    /// The top-left tile of `who`'s space.
    pub(super) fn space_anchor(&self, who: Who) -> TilePos {
        rules::space_anchor(self.who_tile(who), self.space_n(who))
    }

    /// Every tile of `who`'s space.
    pub(super) fn who_space(&self, who: Who) -> Vec<TilePos> {
        rules::footprint(self.space_anchor(who), self.space_n(who))
    }

    /// The distance in tiles between two fighters' spaces (1 = next to each other).
    pub(super) fn who_dist(&self, a: Who, b: Who) -> i32 {
        rules::space_dist(self.space_anchor(a), self.space_n(a), self.space_anchor(b), self.space_n(b))
    }

    /// The distance in tiles from `who`'s space to tile `t`.
    pub(super) fn dist_to(&self, who: Who, t: TilePos) -> i32 {
        rules::space_dist(self.space_anchor(who), self.space_n(who), t, 1)
    }

    /// Where the feet of an `n`-tile creature stand when its space's top-left tile is `anchor`.
    pub(super) fn space_feet(&self, anchor: TilePos, n: i32) -> Vec2 {
        let feet = feet_spot(rules::space_feet_tile(anchor, n), self.ts());
        // (An even width: the feet on the line between the two middle columns.)
        if n % 2 == 0 {
            feet - vec2(self.ts() * 0.5, 0.0)
        } else {
            feet
        }
    }

    pub(super) fn who_name(&self, who: Who) -> String {
        match who {
            Who::Player => self.world.player_name.clone(),
            Who::Npc(uid) => self.entity_display_name(uid).unwrap_or_else(|| "?".into()),
        }
    }

    pub(super) fn who_hp(&self, who: Who) -> (i32, i32) {
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

    pub(super) fn who_conditions_mut(&mut self, who: Who) -> Option<&mut Vec<crate::srd::ActiveCondition>> {
        match who {
            Who::Player => Some(&mut self.world.effects),
            Who::Npc(uid) => self.loaded.entities.iter_mut().find(|e| e.uid == uid).map(|e| &mut e.effects),
        }
    }

    /// A character's stat block, or one made from its old real-time values (AC 10 + defense,
    /// HP); a block without attacks gets one with its weapon or its damage.
    pub(super) fn who_block(&self, who: Who) -> Option<crate::srd::StatBlock> {
        // The hero has none, but in a beast's shape (Wild Shape, Polymorph).
        if who == Who::Player {
            return self.hero_form_block();
        }
        // A party member with a class: its class's block.
        if let Who::Npc(uid) = who {
            if let Some(b) = self.member_block(uid) {
                return Some(b);
            }
        }
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
                reach_ft: if ranged.is_some() { 0 } else { w.reach_tiles().max(1) as u32 * 5 },
                range_ft: ranged.map_or(0, |r| r.0),
                long_ft: ranged.map_or(0, |r| r.1),
                ..Default::default()
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
            // (The hero's counts the spells' buffs already.)
            Who::Player => self.armor_class(),
            // (Spells: Shield of Faith, Mage Armor, Haste.)
            _ => self.buff_ac(who, self.who_block(who).map_or(10, |b| b.ac)),
        }
    }

    /// Speed in feet, after conditions (0 when it can't move).
    pub(super) fn who_speed(&self, who: Who) -> u32 {
        let base = match who {
            // (The class: Fast Movement, Unarmored Movement without armor and shield.)
            Who::Player => self.hero_speed_ft(),
            // (Fliers fly at their best speed, over water, pits and low obstacles.)
            _ => self.who_block(who).map_or(30, |b| b.speed_ft.max(b.speeds.fly)),
        };
        let r = conditions::rules(self.who_conditions(who));
        // Dragging a grappled creature: half speed, unless it is two sizes smaller or more.
        let dragging = self.fight.as_ref().and_then(|c| c.fighter(who)).and_then(|f| f.grappling).is_some_and(|g| rules::dragging_halves(self.who_size(who), self.who_size(g)));
        let drag = if dragging { 0.5 } else { 1.0 };
        (self.buff_speed(who, base) as f32 * r.speed * drag).round() as u32
    }

    fn who_dex(&self, who: Who) -> i32 {
        match who {
            Who::Player => self.world.abilities.modifier(Ability::Dex),
            _ => self.who_block(who).map_or(0, |b| b.abilities.modifier(Ability::Dex)),
        }
    }

    /// The fighters' tiles (the living ones), but `except`.
    pub(super) fn fighter_tiles(&self, except: Who) -> Vec<(Who, Side, TilePos)> {
        let Some(c) = &self.fight else { return Vec::new() };
        // (Every tile of a big creature's space.)
        c.order.iter().filter(|f| f.who != except && !f.out).flat_map(|f| self.who_space(f.who).into_iter().map(move |t| (f.who, f.side, t))).collect()
    }

    // --------------------------------------------------------------------- start and end

    /// Starts a fight: everyone hostile to the party within reach of it, the party and its allies.
    /// `surprise`: the hero struck first, so enemies that hadn't noticed them lose round 1.
    /// `aware`: only enemies that know of the party take part (those that notice it, and with
    /// `surprise` those the hero sees); the others join later when they notice it. Without it
    /// (*Start fight*) every hostile character near joins.
    pub(super) fn start_combat(&mut self, surprise: bool, aware: bool) -> bool {
        if self.fight.is_some() {
            return self.join_fight(aware);
        }
        if !self.combat_on() || self.game_over.is_some() || self.on_title() {
            return false;
        }
        let ts = self.ts();
        let here = self.player.pos;
        let near = |p: Vec2| p.distance(here) <= JOIN_TILES * ts;
        let mut order: Vec<Fighter> = Vec::new();
        let fighter = |who, side, ai| Fighter { who, key: String::new(), side, init: 0, dex: 0, surprised: false, ai, saves: DeathSaves::default(), down: false, out: false, dodging: false, hidden: false, reacted: false, readied: false, readied_spell: None, grappling: None, fx: Default::default(), thrown: Vec::new(), mon: Default::default() };
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
                if aware && !self.takes_part(i, surprise) {
                    continue;
                }
                // (Who is surprised is decided once the fight is set up: Stealth rolls.)
                order.push(fighter(Who::Npc(e.uid), Side::Enemy, true));
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
            // (Feral Instinct: advantage on initiative.)
            let adv = self.who_traits(f.who).is_some_and(|t| t.initiative_advantage);
            let roll = if adv { crate::srd::dice::die(20).max(crate::srd::dice::die(20)) as i32 } else { crate::srd::dice::die(20) as i32 };
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
                // (A big creature: its feet at the bottom middle of its square.)
                let n = rules::space_tiles(self.who_block(f.who).map_or(crate::srd::Size::Medium, |b| b.size));
                let feet = feet_tile(self.loaded.entities[i].body.pos, ts);
                let pos = if n > 1 { self.space_feet(rules::space_anchor(feet, n), n) } else { feet_spot(feet, ts) };
                let e = &mut self.loaded.entities[i];
                e.body.move_target = None;
                e.body.moving = false;
                e.body.pos = pos;
                self.occupy(i);
            }
        }
        let cursor = feet_tile(self.player.pos, ts);
        let mut c = Combat { round: 1, order, current: 0, turn: Turn::default(), helped: Vec::new(), cursor, aim: Aim::Move, walking: Vec::new(), ai_t: 0.4, log: Vec::new(), fast: self.fight_fast, over: false, shove_push: false, selected: 0, then_attack: None, then_pick_up: None, reach: Default::default(), menu: None, attack_mode: None, reach_key: None, mouse_last: Vec2::ZERO, once: Vec::new(), scroll: None, item_cast: None, pending_blow: None, pending_react: None, knocking: false, shot: Vec::new(), ai_wrestled: Vec::new() };
        c.log(crate::lang::tr_ui("Fight! Initiative:").into_owned());
        let names: Vec<String> = first.iter().map(|(w, init)| format!("{} {init}", self.who_name(*w))).collect();
        c.log(names.join(", "));
        self.fight = Some(Box::new(c));
        self.ui.toast(crate::lang::tr_ui("Fight!").into_owned());
        // Surprise (the SRD's): the hero's side struck first while sneaking, or monsters
        // started it out of the hero's sight.
        self.roll_surprise(surprise, aware && !surprise);
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
    fn join_fight(&mut self, aware: bool) -> bool {
        let ts = self.ts();
        let here = self.player.pos;
        let new: Vec<usize> = (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                e.active && e.kind == EntityKind::Character && e.hostile && e.companion.is_none() && e.max_hp > 0 && e.hp > 0 && e.body.pos.distance(here) <= JOIN_TILES * ts && !self.fighting(e.uid) && (!aware || self.takes_part(i, false))
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
            let f = Fighter { who, key, side: Side::Enemy, init, dex, surprised: false, ai: true, saves: DeathSaves::default(), down: false, out: false, dodging: false, hidden: false, reacted: false, readied: false, readied_spell: None, grappling: None, fx: Default::default(), thrown: Vec::new(), mon: Default::default() };
            if let Some(c) = &mut self.fight {
                c.order.push(f);
                c.log(format!("{name} {} ({init})", crate::lang::tr_ui("joins the fight")));
            }
            self.fire(crate::events::GameEvent::CombatStart, super::Source::Entity(uid));
        }
        true
    }

    /// Whether hostile character `i` takes part in a fight starting now: it knows of the party,
    /// or (`hero_struck`) the hero, striking first, sees it.
    fn takes_part(&self, i: usize, hero_struck: bool) -> bool {
        if self.aware_of_party(i) {
            return true;
        }
        let e = &self.loaded.entities[i];
        let eye = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        hero_struck && e.body.pos.distance(self.player.pos) <= super::combat::ATTACK_SIGHT * self.ts() && self.line_clear(eye, e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0))
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
        // The survivors take back what they threw (the hero's lies there).
        self.return_thrown();
        // Rages end with the fight (a frenzy's exhaustion).
        let whos: Vec<Who> = self.fight.as_ref().map_or(Vec::new(), |c| c.order.iter().filter(|f| !f.out).map(|f| f.who).collect());
        for w in whos {
            self.end_rage(w);
        }
        let Some(c) = self.fight.take() else { return };
        for f in c.order.iter().filter(|f| f.side == Side::Party && f.down && !f.out) {
            self.revive(f.who, 1);
        }
        for f in &c.order {
            // (A knocked-out foe stays unconscious where it lies.)
            if self.who_index(f.who).is_some_and(|i| self.loaded.entities[i].knocked_out) {
                continue;
            }
            if let Some(list) = self.who_conditions_mut(f.who) {
                conditions::remove(list, Condition::Unconscious);
            }
        }
        // Fliers hovering over water, a pit or a low obstacle land on walkable ground (they walk
        // outside fights).
        for f in c.order.iter().filter(|f| !f.out) {
            let Some(i) = self.who_index(f.who) else { continue };
            if self.who_flies(f.who) && self.terrain_closed(self.who_tile(f.who)) {
                let (body, pos) = (self.loaded.entities[i].body.clone(), self.loaded.entities[i].body.pos);
                self.loaded.entities[i].body.pos = self.free_spot(&body, pos, super::Mover::Npc(self.loaded.entities[i].uid));
                self.occupy(i);
            }
        }
        // Half the ammunition shot is found again (the SRD: a minute's search after the fight).
        for (item, n) in &c.shot {
            let back = crate::srd::combat::recovered_ammo(*n);
            if back > 0 {
                self.world.give_item(item, back);
                let name = self.project.db.item(item).map_or(item.clone(), |d| crate::lang::tr(&d.name));
                self.ui.toast(crate::lang::tr_ui("Recovered {} {}").replacen("{}", &back.to_string(), 1).replacen("{}", &name, 1));
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

    /// Fighters of the enemy side that are no longer hostile to the player (*Set faction* after
    /// "Spare him", a script) or are gone (defeated by *Defeat all enemies* / `defeat_enemies()`, an
    /// event, removed) leave the fight; the fight ends when no enemy is left.
    fn drop_peaceful(&mut self) {
        let Some(c) = &self.fight else { return };
        // (Gone: defeated outside the fight's rules, by `defeat_enemies` or an event, or removed.)
        let gone = |g: &Self, who: Who| g.who_index(who).is_none_or(|i| !g.loaded.entities[i].active);
        let left: Vec<(Who, bool)> = c
            .order
            .iter()
            .filter(|f| f.side == Side::Enemy && !f.out)
            .filter_map(|f| {
                if gone(self, f.who) {
                    Some((f.who, true))
                } else {
                    self.who_index(f.who).filter(|&i| !self.loaded.entities[i].hostile).map(|_| (f.who, false))
                }
            })
            .collect();
        if left.is_empty() {
            return;
        }
        let current = c.current().who;
        for (who, dead) in &left {
            let name = self.who_name(*who);
            if let Some(c) = &mut self.fight {
                if let Some(f) = c.fighter_mut(*who) {
                    (f.out, f.down) = (true, false);
                }
                c.log(format!("{name} {}", crate::lang::tr_ui(if *dead { "dies" } else { "stops fighting" })));
            }
        }
        self.check_over();
        // (Its own turn: over.)
        if left.iter().any(|(w, _)| *w == current) && self.fight.as_ref().is_some_and(|c| !c.over) {
            self.next_turn();
        }
    }

    /// Whether the fight is decided: no enemy can fight on, or the whole party is down.
    fn check_over(&mut self) {
        let Some(c) = &self.fight else { return };
        // (A troll at 0 hit points that will regenerate is still in the fight.)
        let enemies = c.order.iter().any(|f| f.side == Side::Enemy && !f.out && (!f.down || self.regenerating_down(f.who)));
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
            // (One whose regeneration fire stopped dies where it lies.)
            let stopped: Vec<Who> = c.order.iter().filter(|f| f.side == Side::Enemy && !f.out && f.down && f.mon.regen_down).map(|f| f.who).collect();
            for w in stopped {
                self.dies(w);
            }
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
        (c.menu, c.attack_mode) = (None, None);
        c.helped.retain(|w| *w != f.who);
        let mut held = None;
        if let Some(me) = c.fighter_mut(f.who) {
            me.dodging = false;
            me.reacted = false;
            me.readied = false;
            held = me.readied_spell.take();
        }
        // A readied spell that didn't go off is lost (with its slot), and its concentration.
        if let Some((id, _)) = held {
            if self.concentration_of(f.who).as_deref() == Some(&format!("ready:{id}")) {
                self.end_concentration(f.who);
            }
        }
        if f.out {
            self.next_turn();
            return;
        }
        self.class_turn_start(f.who);
        // (A monster's recharge, legendary points, regeneration.)
        self.monster_turn_start(f.who);
        // (Concentration's time: a round of the caster's.)
        if let Some(t) = match f.who {
            Who::Player => self.world.concentration.as_mut().map(|c| &mut c.1),
            Who::Npc(uid) => self.skills.npc_concentration.get_mut(&uid).map(|c| &mut c.1),
        } {
            *t -= conditions::ROUND;
            if *t <= 0.0 {
                self.end_concentration(f.who);
            }
        }
        // (A shape's and a summon's time: a round of theirs; the lasting areas: the caster's round,
        // and those it starts in act on it.)
        self.tick_shapes(conditions::ROUND, Some(f.who));
        self.areas_new_turn(f.who);
        self.tick_areas(conditions::ROUND, Some(f.who));
        self.areas_turn(f.who);
        let Some(c) = &self.fight else { return };
        if f.surprised && c.round == 1 {
            let line = format!("{} {}", self.who_name(f.who), crate::lang::tr_ui("is surprised"));
            self.fight_log(line);
            self.next_turn();
            return;
        }
        // Conditions run a round: damage and healing per round fall now.
        let ticks = self.who_conditions_mut(f.who).map(|l| conditions::tick(l, conditions::ROUND)).unwrap_or_default();
        if f.who == Who::Player {
            self.tick_coating(conditions::ROUND);
        }
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
            let held = self.fight.as_ref().is_some_and(|c| c.order.iter().any(|g| g.grappling == Some(me.who) && !g.out && !g.down && conditions::can_act(self.who_conditions(g.who)) && self.who_dist(g.who, me.who) <= 1));
            if !held {
                if let Some(l) = self.who_conditions_mut(me.who) {
                    conditions::remove(l, Condition::Grappled);
                }
            }
        }
        let speed = self.who_speed(me.who);
        // (Extra Attack: the hero's class; Multiattack, a party member's Extra Attack: the block.)
        let multi = self.multiattack_of(me.who);
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
        // A finishing blow still waiting for Kill / Knock out lands as a kill.
        if self.fight.as_ref().is_some_and(|c| c.pending_blow.is_some()) {
            if let Some(c) = &mut self.fight {
                c.menu = None;
            }
            self.finish_blow(false);
        }
        // The turn is over: a legendary monster may take a legendary action.
        if let Some(ended) = self.fight.as_ref().filter(|c| !c.over).map(|c| c.current().who) {
            self.legendary_step(ended);
        }
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
            // Hostile characters near that noticed the party since join now (those without a
            // real-time chase too: a deserter standing guard).
            self.join_fight(true);
        }
        self.begin_turn();
    }

    /// Whether the current turn is the player's to play (the hero, a party member without AI).
    pub(in crate::runtime) fn players_turn(&self) -> bool {
        self.fight.as_ref().is_some_and(|c| !c.current().ai && c.walking.is_empty() && !c.over)
    }

    /// Once a frame while fighting: walks on screen, the AI's turns, the player's input.
    pub(super) fn update_turns(&mut self, dt: f32, key_consumed: bool) {
        if self.fight.is_none() {
            return;
        }
        // A reaction asked: the fight waits for the hero's answer (its window takes the input).
        if self.fight.as_ref().is_some_and(|c| c.pending_react.is_some()) {
            if self.fight.as_ref().and_then(|c| c.menu.as_ref()).is_some_and(|m| m.kind == menu::MenuKind::Reaction) {
                if !key_consumed {
                    self.fight_menu_input();
                }
            } else {
                self.resume_reaction(reactions::ReactChoice::Pass);
            }
            return;
        }
        if self.update_walk(dt) {
            return;
        }
        // Foes that made peace (spared, a new faction, charmed out of it) or are gone (defeated by
        // a script or event, removed) leave the fight.
        self.drop_peaceful();
        // A click on a foe out of reach: the attack once the walk there is over.
        if let Some(t) = self.fight.as_mut().and_then(|c| c.then_attack.take()) {
            if let Err(why) = self.act_on(&Aim::Attack, t) {
                self.ui.toast(crate::lang::tr_ui(why).into_owned());
            }
        }
        // A right-click on items out of reach: picked up once next to them.
        if let Some(t) = self.fight.as_mut().and_then(|c| c.then_pick_up.take()) {
            let me = self.fight.as_ref().unwrap().current().who;
            let here = self.who_tile(me);
            if here.0.abs_diff(t.0) <= 1 && here.1.abs_diff(t.1) <= 1 {
                self.pick_up_at(me, t);
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
    /// A tile no one walks onto, whoever stands where: off the map, blocked ground, a solid prop.
    /// Whether a creature can't step onto `t`: a walker [`Game::terrain_closed`], a flier only
    /// what stops shots too (walls, solid ground, props that aren't low).
    pub(super) fn closed_for(&self, t: TilePos, fly: bool) -> bool {
        if fly {
            self.blocks_shot(t.0, t.1)
        } else {
            self.terrain_closed(t)
        }
    }

    /// Whether `who` flies (a fly speed in its stat block): it crosses water, pits and low
    /// obstacles in fights.
    pub(super) fn who_flies(&self, who: Who) -> bool {
        self.who_block(who).is_some_and(|b| b.speeds.fly > 0)
    }

    pub(super) fn terrain_closed(&self, t: TilePos) -> bool {
        !self.loaded.map.in_bounds(t.0, t.1) || self.tile_blocks_npc(t.0, t.1) || self.loaded.occupancy.has(t.0, t.1, super::occupancy::OCC_PROP)
    }

    /// Walking distances (steps) to `goals` over the terrain (creatures don't block), out to
    /// `limit` steps: the AI's way round trees and walls to a foe.
    pub(super) fn walk_distances(&self, goals: &[TilePos], limit: i32, fly: bool) -> std::collections::HashMap<TilePos, i32> {
        distance_field(goals, limit, |t| self.closed_for(t, fly))
    }

    /// Where a creature can walk this turn: tile -> (feet it costs, the tile before it); the
    /// allies' tiles it passes on the way are kept apart (`Reach::passed`).
    pub(super) fn reachable(&self, who: Who) -> Reach {
        let mut out = std::collections::HashMap::new();
        let Some(c) = &self.fight else { return Reach::default() };
        let Some(me) = c.fighter(who) else { return Reach::default() };
        let left = c.turn.speed.saturating_sub(c.turn.moved);
        let start = self.who_tile(who);
        let n = self.space_n(who);
        let my_size = self.who_size(who);
        let fighters = self.fighter_tiles(who);
        let fly = self.who_flies(who);
        // Frightened: no step closer to the source of its fear while it sees it.
        let fear = self.fear_in_sight(who).map(|s| {
            let (at, sn) = (self.space_anchor(s), self.space_n(s));
            (at, sn, rules::space_dist(rules::space_anchor(start, n), n, at, sn))
        });
        // Who stands on a tile: an ally's space can be passed (as difficult terrain), a foe's only
        // when one of the two is two sizes larger or more; nobody ends on another's space.
        let passable = |w: Who, s: Side| s == me.side || (my_size as i32 - self.who_size(w) as i32).abs() >= 2;
        out.insert(start, (0, start));
        let mut open = std::collections::BinaryHeap::new();
        open.push(std::cmp::Reverse((0u32, start)));
        while let Some(std::cmp::Reverse((cost, t))) = open.pop() {
            if out.get(&t).is_some_and(|(c, _)| *c < cost) {
                continue;
            }
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let f = (t.0 + d.0, t.1 + d.1);
                let Some((space, squeezed)) = self.space_for(f, n, fly) else { continue };
                let mut through = false;
                if fighters.iter().any(|(w, s, p)| space.contains(p) && {
                    through = true;
                    !passable(*w, *s)
                }) {
                    continue;
                }
                if fear.is_some_and(|(at, sn, d0)| rules::space_dist(rules::space_anchor(f, n), n, at, sn) < d0) {
                    continue;
                }
                // A diagonal step, unless both corners are walls (no squeezing through).
                if n == 1 && d.0 != 0 && d.1 != 0 && !diagonal_ok(self.closed_for((t.0 + d.0, t.1), fly), self.closed_for((t.0, t.1 + d.1), fly)) {
                    continue;
                }
                // (Difficult ground under any of its tiles, squeezing, or another's space: double.)
                let rough = squeezed || through || space.iter().any(|p| self.difficult_at(*p) || self.area_difficult(*p, who));
                let nc = cost + rules::step_cost(rough);
                if nc > left {
                    continue;
                }
                if out.get(&f).is_none_or(|(c, _)| nc < *c) {
                    out.insert(f, (nc, t));
                    open.push(std::cmp::Reverse((nc, f)));
                }
            }
        }
        // Spaces with someone on them can be passed, not ended on (kept for the paths through them).
        let (tiles, passed) = out.into_iter().partition(|(f, _)| *f == start || self.space_for(*f, n, fly).is_none_or(|(space, _)| !fighters.iter().any(|(_, _, p)| space.contains(p))));
        Reach { tiles, passed }
    }

    /// The tiles an `n`-tile creature takes with its feet on `feet`, if the terrain lets it stand
    /// there: its whole square, or (squeezing, `true`) the square one size smaller.
    pub(super) fn space_at(&self, feet: TilePos, n: i32) -> Option<(Vec<TilePos>, bool)> {
        self.space_for(feet, n, false)
    }

    /// [`Game::space_at`] for a flier too (`fly`: over water, pits and low obstacles).
    pub(super) fn space_for(&self, feet: TilePos, n: i32, fly: bool) -> Option<(Vec<TilePos>, bool)> {
        let open = |t: &TilePos| !self.closed_for(*t, fly);
        let full = rules::footprint(rules::space_anchor(feet, n), n);
        if full.iter().all(open) {
            return Some((full, false));
        }
        if n > 1 {
            let small = rules::footprint(rules::space_anchor(feet, n - 1), n - 1);
            if small.iter().all(open) {
                return Some((small, true));
            }
        }
        None
    }

    /// Whether `who` stands squeezed into a space smaller than it (a Large creature in a
    /// corridor): disadvantage on its attacks and DEX saves, advantage against it.
    pub(super) fn squeezed(&self, who: Who) -> bool {
        let n = self.space_n(who);
        n > 1 && self.space_at(self.who_tile(who), n).is_some_and(|s| s.1)
    }

    /// A path from the reach map (excludes the start), also through allies' tiles.
    pub(super) fn path_to(reach: &Reach, start: TilePos, goal: TilePos) -> Vec<TilePos> {
        let mut path = Vec::new();
        let mut t = goal;
        while t != start {
            // (A reach map of another start tile leads nowhere: no path.)
            if path.len() > reach.tiles.len() + reach.passed.len() {
                return Vec::new();
            }
            path.push(t);
            match reach.tiles.get(&t).or_else(|| reach.passed.get(&t)) {
                Some((_, prev)) => t = *prev,
                None => return Vec::new(),
            }
        }
        path.reverse();
        path
    }

    /// Difficult terrain: the painted ground's material says so, or a rough decoration
    /// (undergrowth, rubble) stands there.
    pub(super) fn difficult_at(&self, t: TilePos) -> bool {
        if self.loaded.occupancy.has(t.0, t.1, super::occupancy::OCC_ROUGH) {
            return true;
        }
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
        let cost = rules::step_cost(self.difficult_at(next) || self.area_difficult(next, who));
        let disengaged = self.fight.as_ref().is_some_and(|c| c.turn.disengaged);
        // Leaving a foe's reach without Disengage: its opportunity attack (a reaction).
        if !disengaged {
            // (Not the creature it drags along: it comes with it.)
            let dragged = self.fight.as_ref().and_then(|c| c.fighter(who)).and_then(|f| f.grappling);
            let foes: Vec<Who> = self.fight.as_ref().map_or(Vec::new(), |c| c.order.iter().filter(|f| f.side != side && !f.out && !f.down && !f.reacted && Some(f.who) != dragged).map(|f| f.who).collect());
            for foe in foes {
                let (fa, fnn) = (self.space_anchor(foe), self.space_n(foe));
                // (Its melee reach: a glaive's two tiles; from its whole space to the mover's.)
                let r = self.melee_spec(foe).map_or(1, |s| s.reach.max(1));
                let mn = self.space_n(who);
                let near = |t: TilePos| rules::space_dist(rules::space_anchor(t, mn), mn, fa, fnn) <= r;
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
                c.then_pick_up = None;
            }
            return true;
        }
        let ts = self.ts();
        // (A big creature's feet: the bottom middle of its square.)
        let n = self.space_n(who);
        let to = if n > 1 { self.space_feet(rules::space_anchor(next, n), n) } else { feet_spot(next, ts) };
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
        // A grappler drags the creature it holds into the tile it left.
        self.drag_grappled(who, from, next);
        // Its own thrown weapon lying there: picked up (free); characters the player plays pick up
        // everything lying there.
        self.pick_up_thrown(who, next);
        let played = self.fight.as_ref().and_then(|c| c.fighter(who)).is_some_and(|f| !f.ai && f.side == Side::Party);
        if played {
            self.pick_up_at(who, next);
        }
        // Entering a readied foe's reach: its attack.
        let readied: Vec<Who> = self.fight.as_ref().map_or(Vec::new(), |c| c.order.iter().filter(|f| f.side != side && f.readied && !f.reacted && !f.out && !f.down).map(|f| f.who).collect());
        for foe in readied {
            let r = self.melee_spec(foe).map_or(1, |s| s.reach.max(1));
            let mn = self.space_n(who);
            if rules::space_dist(rules::space_anchor(next, mn), mn, self.space_anchor(foe), self.space_n(foe)) <= r {
                if let Some(spec) = self.melee_spec(foe) {
                    if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(foe)) {
                        f.reacted = true;
                        f.readied = false;
                    }
                    self.make_attack(foe, who, &spec);
                }
            }
        }
        // Coming within range of a foe's readied spell: it is cast at it.
        self.readied_spells(who, side, next);
        // Into a lasting area: Spirit Guardians, Spike Growth, a Moonbeam act.
        self.areas_step(who, next);
        true
    }

    /// `mover` ended a step on `at`: a foe's readied spell whose range now reaches it goes off at
    /// it (the reaction; its slot was paid when it was readied).
    fn readied_spells(&mut self, mover: Who, side: Side, at: TilePos) {
        let ready: Vec<(Who, String, u32)> = self.fight.as_ref().map_or(Vec::new(), |c| {
            c.order.iter().filter(|f| f.side != side && !f.reacted && !f.out && !f.down).filter_map(|f| f.readied_spell.clone().map(|(s, l)| (f.who, s, l))).collect()
        });
        for (caster, id, slot) in ready {
            let Some(d) = self.project.db.spell(&id).cloned() else { continue };
            let mn = self.space_n(mover);
            let dist = rules::space_dist(self.space_anchor(caster), self.space_n(caster), rules::space_anchor(at, mn), mn);
            if dist > d.range.tiles().max(1) || self.cover(caster, mover) == Cover::Total {
                continue;
            }
            // (Its concentration broke: the spell is lost.)
            let held = self.concentration_of(caster).as_deref() == Some(&format!("ready:{id}"));
            if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(caster)) {
                f.readied_spell = None;
                if held {
                    f.reacted = true;
                }
            }
            if !held {
                continue;
            }
            self.end_concentration(caster);
            let Ok((targets, point)) = self.spell_targets(caster, &d, slot, casting::CastAt::On(mover)) else { continue };
            let name = self.who_name(caster);
            self.fight_log(format!("{name}: {} ({})", crate::lang::tr(&d.name), crate::lang::tr_ui("readied")));
            self.resolve_spell(caster, &d, slot, &targets, Some(point));
        }
    }

    /// The creature `who` grapples follows it: into the tile `who` left (`from`) when it isn't
    /// next to `who`'s new tile any more.
    fn drag_grappled(&mut self, who: Who, from: TilePos, to: TilePos) {
        let Some(g) = self.fight.as_ref().and_then(|c| c.fighter(who)).and_then(|f| f.grappling) else { return };
        let n = self.space_n(who);
        if rules::space_dist(self.space_anchor(g), self.space_n(g), rules::space_anchor(to, n), n) <= 1 {
            return;
        }
        let p = feet_spot(from, self.ts());
        match self.who_index(g) {
            None => self.player.pos = p,
            Some(i) => {
                self.loaded.entities[i].body.pos = p;
                self.occupy(i);
            }
        }
    }

    /// Whether character `uid` takes part in the fight (its turns move it).
    pub(super) fn fighting(&self, uid: u32) -> bool {
        self.fight.as_ref().is_some_and(|c| c.order.iter().any(|f| f.who == Who::Npc(uid) && !f.out))
    }

    // --------------------------------------------------------------------------- attacks

    /// The attacks a combatant can make: the hero's weapon, a stat block's attacks.
    pub(super) fn attack_specs(&self, who: Who) -> Vec<AttackSpec> {
        match who {
            // (In a beast's shape: the beast's attacks.)
            Who::Player if self.world.shape.is_none() => {
                let w = self.attack_weapon();
                let mut v = vec![self.hero_spec(&w, false)];
                // A thrown weapon: a ranged attack too (it stays in hand: a simplification).
                if w.has(crate::defs::WeaponProperty::Thrown) && w.thrown_ft.is_some() && w.ranged.is_none() {
                    v.push(self.weapon_spec(&w, false, true));
                }
                v
            }
            _ => {
                let Some(b) = self.who_block(who) else { return Vec::new() };
                let effect = self.who_index(who).and_then(|i| self.touch_effect(i));
                let projectile = self.who_index(who).and_then(|i| self.loaded.entities[i].weapon.as_ref().and_then(|w| w.ranged.as_ref().map(|r| r.projectile.clone())));
                // (A party member's Improved Critical.)
                let crit_on = self.who_traits(who).map_or(20, |t| t.crit_from);
                // (A party member's weapon: STR in melee, DEX at range; two-handed or not.)
                let classed = matches!(who, Who::Npc(uid) if self.member_class(uid).is_some());
                let finesse = self.who_index(who).is_some_and(|i| self.loaded.entities[i].weapon.as_ref().is_some_and(|w| w.has(crate::defs::WeaponProperty::Finesse)));
                // (A thrown weapon flies as the icon of the weapon in its hand.)
                let weapon_icon = self.who_index(who).and_then(|i| self.loaded.entities[i].equipment.item(crate::defs::EquipSlot::Weapon)).and_then(|id| self.project.db.item(id)).map(|d| d.icon.clone());
                let gone = self.thrown_attacks(who);
                // (Magic Weapons, a magic or silvered weapon in its hand: through resistances to
                // nonmagical attacks.)
                let held = self.who_index(who).and_then(|i| self.loaded.entities[i].equipment.item(crate::defs::EquipSlot::Weapon)).and_then(|id| self.project.db.item(id));
                let hit = if b.features.iter().any(|f| f.name.eq_ignore_ascii_case("Magic Weapons")) || held.is_some_and(|d| d.magic.is_some()) {
                    crate::srd::HitKind::Magical
                } else if held.is_some_and(|d| d.silvered) {
                    crate::srd::HitKind::Silvered
                } else {
                    crate::srd::HitKind::Plain
                };
                // (Both hands free: a versatile weapon's two-handed damage, for a character whose
                // equipment shows it: a weapon in hand, nothing in the left.)
                let two_hands = self.who_index(who).is_some_and(|i| {
                    let eq = &self.loaded.entities[i].equipment;
                    eq.item(crate::defs::EquipSlot::Weapon).is_some() && eq.item(crate::defs::EquipSlot::Offhand).is_none()
                });
                let mut specs: Vec<AttackSpec> = b.attacks
                    .iter()
                    .filter(|a| !gone.contains(&a.name))
                    .flat_map(|a| {
                        let mut v = Vec::new();
                        if a.reach_ft > 0 {
                            let dice = if two_hands && !a.versatile.is_empty() { a.versatile.clone() } else { a.dice.clone() };
                            v.push(AttackSpec { name: a.name.clone(), bonus: a.bonus, dice, damage_type: a.damage_type, ranged: false, reach: (a.reach_ft / 5).max(1) as i32, range: 0, long: 0, crit_on, effect: effect.clone(), projectile: None, ability: classed.then_some(Ability::Str), finesse, extra: a.extra.clone(), riders: a.on_hit.clone(), hit, ..Default::default() });
                        }
                        if a.range_ft > 0 {
                            // (Melee or ranged: a thrown weapon.)
                            let thrown = a.reach_ft > 0;
                            let projectile = if thrown { weapon_icon.clone().or_else(|| projectile.clone()) } else { projectile.clone() };
                            v.push(AttackSpec { name: a.name.clone(), bonus: a.bonus, dice: a.dice.clone(), damage_type: a.damage_type, ranged: true, reach: 0, range: (a.range_ft / 5).max(1) as i32, long: (a.long_ft.max(a.range_ft) / 5).max(1) as i32, crit_on, effect: None, projectile, ability: classed.then_some(if thrown { Ability::Str } else { Ability::Dex }), finesse, thrown, extra: a.extra.clone(), riders: a.on_hit.clone(), hit, ..Default::default() });
                        }
                        v
                    })
                    .collect();
                // Its weapons thrown and nothing else left: an unarmed strike (1 + STR).
                if specs.is_empty() && !gone.is_empty() {
                    let m = b.abilities.modifier(Ability::Str);
                    let bonus = crate::srd::abilities::proficiency_bonus(b.proficiency_level()) + m;
                    specs.push(AttackSpec { name: crate::lang::tr_ui("Unarmed strike").into_owned(), bonus, dice: (1 + m).max(1).to_string(), damage_type: Some(DamageType::Bludgeoning), ranged: false, reach: 1, crit_on, ability: Some(Ability::Str), ..Default::default() });
                }
                specs
            }
        }
    }

    /// The hero's attack with weapon `w` (`offhand`: no ability modifier on the damage).
    pub(super) fn hero_spec(&self, w: &crate::defs::Weapon, offhand: bool) -> AttackSpec {
        self.weapon_spec(w, offhand, false)
    }

    /// The hero's attack with weapon `w` (`offhand`: no ability modifier on the damage; `thrown`:
    /// a thrown weapon's ranged attack) by the SRD: finesse (STR or DEX, the better), versatile
    /// (its larger die with the off-hand free), reach, heavy (disadvantage for the Small),
    /// loading, the proficiency bonus only with proficiency.
    pub(super) fn weapon_spec(&self, w: &crate::defs::Weapon, offhand: bool, thrown: bool) -> AttackSpec {
        use crate::classes::FightingStyle as F;
        use crate::defs::WeaponProperty as P;
        let t = self.hero_traits().unwrap_or_default();
        let ranged = w.ranged.is_some() || thrown;
        let unarmed = self.current_weapon().is_none() && !offhand;
        let (str_m, dex_m) = (self.world.abilities.modifier(Ability::Str), self.world.abilities.modifier(Ability::Dex));
        let ability = if w.has(P::Finesse) {
            if dex_m > str_m { Ability::Dex } else { Ability::Str }
        } else if w.ranged.is_some() {
            Ability::Dex
        } else {
            Ability::Str
        };
        let mut m = self.world.abilities.modifier(ability);
        // Shillelagh: a melee weapon's new die, the spellcasting ability, magical hits.
        let shillelagh = if ranged || unarmed { None } else { conditions::buffs(&self.world.effects).find_map(|b| if let conditions::Buff::WeaponDie { dice } = b { Some(dice.clone()) } else { None }) };
        if shillelagh.is_some() {
            m = m.max(self.spell_mod(Who::Player));
        }
        let free_hand = self.world.equipment.item(crate::defs::EquipSlot::Offhand).is_none();
        let mut base_dice = if !offhand && !thrown && w.has(P::Versatile) && !w.versatile.is_empty() && free_hand { w.versatile.clone() } else { w.damage_dice() };
        if let Some(d) = &shillelagh {
            base_dice = d.clone();
        }
        // Martial Arts: an unarmed strike with DEX if better, the class's martial arts die.
        if unarmed && t.martial_arts {
            m = m.max(dex_m);
            let die = self.hero_class().map_or(4, |c| c.number(self.world.player_level(), "martial_arts.dice_value")).max(4);
            base_dice = format!("1d{die}");
        }
        // The proficiency bonus with proficiency in the weapon (bare hands always).
        let slot = if offhand { crate::defs::EquipSlot::Offhand } else { crate::defs::EquipSlot::Weapon };
        let srd = self.world.equipment.item(slot).and_then(|id| self.project.db.item(id)).map_or(String::new(), |d| d.srd.clone());
        let proficient = unarmed || crate::srd::equipment::weapon_proficient(|p| self.world.is_proficient(p), w, &srd);
        let prof = if proficient { crate::srd::abilities::proficiency_bonus(self.world.player_level()) } else { 0 };
        // The off-hand adds a positive modifier only with Two-Weapon Fighting.
        let dmg_mod = if offhand && m > 0 && !t.styles.contains(&F::TwoWeaponFighting) { 0 } else { m };
        // Dueling: +2 with a one-handed melee weapon and no weapon in the other hand.
        let dueling = !ranged && !w.has(P::TwoHanded) && !unarmed && t.styles.contains(&F::Dueling) && self.offhand_weapon(Who::Player).is_none();
        // Magic: the weapon's +N (its ammunition's when shooting), the items' bonuses.
        let mut magic = if unarmed { 0 } else { self.world.equipment.item(slot).map_or(0, |id| self.item_bonus(Who::Player, id)) };
        // What makes the hit: a magic or silvered weapon, or the ammunition shot.
        let mut made_of: Vec<String> = if unarmed { Vec::new() } else { self.world.equipment.item(slot).map(str::to_string).into_iter().collect() };
        if let Some(r) = w.ranged.as_ref().filter(|r| !r.ammo.is_empty() && !thrown) {
            if let Some((ammo, ..)) = self.next_ammo(&r.ammo) {
                magic += self.item_bonus(Who::Player, &ammo);
                made_of.push(ammo);
            }
        }
        let made = |f: &dyn Fn(&crate::defs::ItemDef) -> bool| made_of.iter().any(|id| self.project.db.item(id).is_some_and(f));
        let hit = if shillelagh.is_some() || made(&|d| d.magic.is_some()) {
            crate::srd::HitKind::Magical
        } else if made(&|d| d.silvered) {
            crate::srd::HitKind::Silvered
        } else {
            crate::srd::HitKind::Plain
        };
        let dmg_mod = dmg_mod + if dueling { 2 } else { 0 } + magic + t.bonus(crate::classes::BonusTo::Damage);
        // (A net does no damage: no modifier on its 0.)
        let dice = if dmg_mod == 0 || srd == "net" { base_dice } else { format!("{base_dice}{dmg_mod:+}") };
        let (range, long) = match (&w.ranged, thrown) {
            (_, true) => w.thrown_ft.map_or((4, 12), |(n, l)| ((n / 5).max(1) as i32, (l / 5).max(1) as i32)),
            (Some(r), _) => (r.range.round() as i32, if r.long_range > 0.0 { r.long_range } else { r.range * 2.0 }.round() as i32),
            (None, _) => (0, 0),
        };
        let small = self.player_race().is_some_and(|r| r.size_category.eq_ignore_ascii_case("small"));
        let item_icon = self.world.equipment.item(slot).and_then(|id| self.project.db.item(id)).map(|d| d.icon.clone());
        AttackSpec {
            name: String::new(),
            // (Archery: +2 to ranged attacks.)
            // (Failing sight from a disease: its penalty.)
            bonus: m + prof + magic + t.bonus(crate::classes::BonusTo::Attack) + if w.ranged.is_some() && t.styles.contains(&F::Archery) { 2 } else { 0 } - self.disease_sight_penalty(),
            dice,
            damage_type: w.damage_type,
            ranged,
            reach: if ranged { 0 } else { w.reach_tiles() },
            range,
            long,
            // (A keen weapon on 19; Improved / Superior Critical.)
            crit_on: (if w.crit >= 0.1 { 19 } else { 20 }).min(t.crit_from),
            effect: w.effect.clone(),
            projectile: if thrown { item_icon } else { w.ranged.as_ref().map(|r| r.projectile.clone()) },
            thrown,
            brutal: if ranged { 0 } else { t.brutal_dice },
            reroll_low: !ranged && w.has(P::TwoHanded) && t.styles.contains(&F::GreatWeaponFighting),
            ability: Some(ability),
            spell: false,
            flurry: false,
            finesse: w.has(P::Finesse),
            loading: w.has(P::Loading),
            disadvantage: small && w.has(P::Heavy),
            hit,
            poison: self.coated_attack(&made_of),
            net: srd == "net",
            lance: srd == "lance",
            ..Default::default()
        }
    }

    /// A combatant's melee attack (opportunity attacks, readied ones).
    pub(in crate::runtime) fn melee_spec(&self, who: Who) -> Option<AttackSpec> {
        self.attack_specs(who).into_iter().find(|s| !s.ranged)
    }

    /// The attack `spec` can make on `target` from where `who` stands, or why not.
    pub(super) fn can_attack(&self, who: Who, target: Who, spec: &AttackSpec) -> Result<(), &'static str> {
        // Charmed: never against the charmer.
        if self.charmed_by(who) == Some(target) {
            return Err("Charmed");
        }
        let dist = self.who_dist(who, target);
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
        // (A big attacker: from the tile of its space that sees best.)
        self.who_space(who).into_iter().map(|a| self.cover_from(a, who, target)).min().unwrap_or_default()
    }

    /// The cover `target` would have against `who` standing on tile `a`.
    pub(super) fn cover_from(&self, a: TilePos, who: Who, target: Who) -> Cover {
        let ts = self.ts();
        // (The target's whole space: a big creature's square.)
        let (t, n) = (self.space_anchor(target), self.space_n(target));
        if rules::space_dist(a, 1, t, n) <= 1 {
            return Cover::None;
        }
        let space = rules::footprint(t, n);
        let from = vec2((a.0 as f32 + 0.5) * ts, (a.1 as f32 + 0.5) * ts);
        let creatures: Vec<TilePos> = self.fighter_tiles(who).into_iter().filter(|(w, _, _)| *w != target && *w != who).map(|(_, _, p)| p).collect();
        let inset = 0.15 * ts;
        let size = n as f32 * ts;
        let corners = [(inset, inset), (size - inset, inset), (inset, size - inset), (size - inset, size - inset)];
        // Walls and tall props block lines; creatures in the way give half cover at most (the SRD).
        let (mut walls, mut creature) = (0, false);
        for (dx, dy) in corners {
            let to = vec2(t.0 as f32 * ts + dx, t.1 as f32 * ts + dy);
            let (mut wall, mut body) = (false, false);
            for p in crate::grid::line_tiles(from, to, ts).filter(|p| *p != a && !space.contains(p)) {
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

    /// Who charmed `who` (still in the fight), if anyone.
    pub(super) fn charmed_by(&self, who: Who) -> Option<Who> {
        self.who_of_tag(conditions::origin_of(self.who_conditions(who), Condition::Charmed))
    }

    /// The source of `who`'s fear while it can see it (the frightened condition's rules count
    /// only then); None = not frightened, the source unknown or out of sight.
    pub(super) fn fear_in_sight(&self, who: Who) -> Option<Who> {
        let s = self.who_of_tag(conditions::origin_of(self.who_conditions(who), Condition::Frightened))?;
        let gone = self.fight.as_ref().and_then(|c| c.fighter(s)).is_none_or(|f| f.out);
        (!gone && self.cover(who, s) != Cover::Total).then_some(s)
    }

    /// `who`'s condition rules in a fight: frightened counts only while its source is in sight
    /// (an unknown source: always).
    fn fight_rules(&self, who: Who) -> conditions::Rules {
        let list = self.who_conditions(who);
        let fear_by = conditions::origin_of(list, Condition::Frightened);
        if conditions::has(list, Condition::Frightened) && !fear_by.is_empty() && self.fear_in_sight(who).is_none() {
            let calm: Vec<_> = list.iter().filter(|a| a.condition != Some(Condition::Frightened)).cloned().collect();
            return conditions::rules(&calm);
        }
        conditions::rules(list)
    }

    /// The situation of an attack (for advantage and disadvantage).
    fn situation(&self, who: Who, target: Who, spec: &AttackSpec) -> AttackSituation {
        let dist = self.who_dist(who, target);
        let me = self.fight_rules(who);
        let them = conditions::rules(self.who_conditions(target));
        let c = self.fight.as_ref();
        let my_side = c.and_then(|c| c.fighter(who)).map(|f| f.side);
        let enemy_adjacent = c.is_some_and(|c| {
            c.order.iter().any(|f| Some(f.side) != my_side && !f.out && !f.down && self.who_dist(f.who, who) <= 1 && conditions::can_act(self.who_conditions(f.who)))
        });
        AttackSituation {
            ranged: spec.ranged,
            adjacent: dist <= 1,
            enemy_adjacent,
            long_range: spec.ranged && dist > spec.range,
            attacker_disadvantage: me.attack_disadvantage || spec.disadvantage || (!spec.spell && self.who_armor_untrained(who)) || (who == Who::Player && self.world.training.heavy_load) || (spec.lance && dist <= 1) || self.squeezed(who),
            // (Unseen: invisible, hidden, or the target can't see it: darkness, fog.)
            // (Truesight, blindsight, See Invisibility: the invisible are seen; can_see knows.)
            attacker_unseen: c.and_then(|c| c.fighter(who)).is_some_and(|f| f.hidden && !self.perceives(target, who)) || !self.can_see(target, who),
            target_prone: conditions::has(self.who_conditions(target), Condition::Prone),
            target_helpless: conditions::has(self.who_conditions(target), Condition::Restrained) || them.str_dex_saves_fail || them.blinded || self.squeezed(target),
            target_unseen: !self.can_see(who, target),
            target_dodging: c.and_then(|c| c.fighter(target)).is_some_and(|f| f.dodging) && conditions::can_act(self.who_conditions(target)),
            // The Grappler feat: advantage against the creature it grapples (counted as a helping hand).
            // (And Reckless Attack: the attacker's on its STR melee attacks, the target's on all.)
            // (Pack Tactics: an ally of the attacker's next to the target.)
            helped: c.is_some_and(|c| c.helped.contains(&target)) || (c.and_then(|c| c.fighter(who)).is_some_and(|f| f.grappling == Some(target)) && self.has_grappler(who)) || self.reckless_edge(who, target, spec) || self.pack_tactics(who, target),
        }
    }

    /// The chance (0-1) `spec` hits `target` (the UI).
    pub(super) fn hit_chance(&self, who: Who, target: Who, spec: &AttackSpec) -> f32 {
        let spec = &self.buffed(who, spec);
        let mode = rules::odds(&self.situation(who, target, spec)).mode();
        let ac = self.who_ac(target) + self.cover(who, target).ac_bonus();
        rules::AttackRoll::chance(spec.bonus, ac, mode)
    }

    /// Rolls an attack of `who` on `target` and applies its damage and condition (the
    /// reactions to it in between, `reactions.rs`: an asked one holds the attack).
    pub(super) fn make_attack(&mut self, who: Who, target: Who, spec: &AttackSpec) -> bool {
        self.attack_with(who, target, spec, reactions::AttackOpts::default())
    }

    /// [`Self::make_attack`] with the reactions decided so far.
    fn attack_with(&mut self, who: Who, target: Who, spec: &AttackSpec, opts: reactions::AttackOpts) -> bool {
        // Protection: an ally of the target's makes the attack roll with disadvantage.
        let protected = match opts.protected {
            Some(p) => p,
            None => match self.protection_before(who, target, spec) {
                reactions::ReactNow::Wait => return false,
                reactions::ReactNow::Use(_) => true,
                reactions::ReactNow::None => false,
            },
        };
        let mut spec = self.buffed(who, spec);
        // (Bless / Bane: a die on the attack roll.)
        spec.bonus += self.buff_roll(who, true);
        let spec = &spec;
        let sit = self.situation(who, target, spec);
        let mut odds = rules::odds(&sit);
        odds.disadvantage |= protected;
        // (True Strike: advantage, spent by this attack.)
        odds.advantage |= self.take_next_attack(who, target);
        let mode = odds.mode();
        let cover = self.cover(who, target);
        let ac = self.who_ac(target) + cover.ac_bonus();
        let auto_crit = sit.adjacent && conditions::rules(self.who_conditions(target)).hits_crit;
        let mut roll = rules::attack(spec.bonus, ac, mode, spec.crit_on, auto_crit);
        // Autoplay `crit always|never`: the hits of the player's side.
        let party = self.fight.as_ref().and_then(|c| c.fighter(who)).is_some_and(|f| f.side == Side::Party);
        if let (true, Some(always)) = (party && roll.hit, self.juice.crit_mode) {
            roll.crit = always;
        }
        // Bardic Inspiration; attacking keeps a rage going.
        self.inspire_attack(who, &mut roll);
        if let Some(fx) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)).map(|f| &mut f.fx) {
            fx.rage_kept = true;
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
        let logged = line.clone();
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
        // A thrown weapon leaves the hand and lands next to the target (hit or miss).
        if spec.thrown {
            self.after_throw(who, target, spec);
        }
        if !roll.hit {
            let head = to - vec2(0.0, self.ts() * 1.1);
            self.miss_number(head);
            return false;
        }
        // The target's reaction to the hit (Shield, Parry, Uncanny Dodge…).
        let react = match self.hit_reaction(who, target, spec, &roll, &logged) {
            reactions::ReactNow::Wait => return true,
            reactions::ReactNow::Use(c) => c,
            reactions::ReactNow::None => reactions::ReactChoice::Pass,
        };
        self.land_attack(who, target, spec, roll, logged, react)
    }

    /// A hit's damage and what comes with it, after the target's reaction `react`: a Shield or
    /// Parry that raises the AC above the roll turns it into a miss.
    fn land_attack(&mut self, who: Who, target: Who, spec: &AttackSpec, roll: rules::AttackRoll, logged: String, react: reactions::ReactChoice) -> bool {
        let mode = roll.mode;
        let to = self.who_pos(target);
        if self.ac_reaction(target, react, &roll) {
            if let Some(l) = self.fight.as_mut().and_then(|c| c.log.iter_mut().rev().find(|l| **l == logged)) {
                l.push_str(&format!(" > {}", crate::lang::tr_ui("miss")));
            }
            let head = to - vec2(0.0, self.ts() * 1.1);
            self.miss_number(head);
            return false;
        }
        let dice = if roll.crit { rules::crit_dice(&spec.dice) } else { spec.dice.clone() };
        // (A net does no damage: it restrains.)
        let mut amount = if spec.net { 0 } else { roll_damage(&dice, spec.reroll_low) };
        // Brutal Critical: one more weapon die per step on a critical melee hit.
        if roll.crit && spec.brutal > 0 {
            if let Some((n, sides)) = first_dice(&spec.dice) {
                let _ = n;
                amount += roll_damage(&format!("{}d{sides}", spec.brutal), spec.reroll_low);
            }
        }
        // The attacker's riders (Sneak Attack…), the target's reaction (Uncanny Dodge…).
        amount += self.rider_damage(who, target, spec, mode, roll.crit);
        let amount = self.react_to_hit(who, target, spec, amount, react);
        // More damage of other types (a dragon's bite's fire), doubled on a critical hit.
        let mut parts = vec![(amount, spec.damage_type)];
        for x in &spec.extra {
            let dice = if roll.crit { rules::crit_dice(&x.dice) } else { x.dice.clone() };
            parts.push((roll_damage(&dice, false), x.damage_type));
        }
        // The buffs' damage on weapon hits (Divine Favor, Hunter's Mark, a smite).
        if !spec.spell {
            for (dice, t) in self.hit_buffs(who, target) {
                let dice = if roll.crit { rules::crit_dice(&dice) } else { dice };
                parts.push((roll_damage(&dice, false), t.or(spec.damage_type)));
            }
        }
        let total: i32 = parts.iter().map(|p| p.0).sum();
        // The damage onto the attack's line ("hit, 7"; before resistances).
        if let Some(l) = self.fight.as_mut().and_then(|c| c.log.iter_mut().rev().find(|l| **l == logged)) {
            l.push_str(&format!(", {total}"));
        }
        // A finishing blow of the player's in melee: Kill or Knock out? (held until chosen)
        if self.asks_knock_out(who, target, spec, &parts) {
            // (The blow lands: a coating is spent on it.)
            if who == Who::Player && !spec.poison.is_empty() {
                self.world.coating = None;
            }
            if let Some(c) = &mut self.fight {
                c.pending_blow = Some(PendingBlow { who, target, parts, crit: roll.crit, hit: spec.hit });
            }
            self.open_fight_menu(menu::MenuKind::Finish, menu::finish_rows());
            return true;
        }
        self.combat_damage_parts(target, &parts, roll.crit, Some(who), spec.hit);
        self.on_hit_saves(who, target, spec);
        self.hit_riders(who, target, spec);
        // A coated weapon or arrow: the poison's dose.
        if !spec.poison.is_empty() {
            self.deliver_coating(who, target, &spec.poison);
        }
        // A net restrains a creature of Large size or smaller (escape DC 10).
        if spec.net && self.who_size(target) <= crate::srd::Size::Large {
            let tag = self.who_tag(who);
            if let Some(l) = self.who_conditions_mut(target) {
                conditions::apply(l, &ConditionSpec::new(Condition::Restrained, 0.0));
                conditions::set_origin(l, Condition::Restrained, &tag, 10);
            }
            let tname = self.who_name(target);
            self.fight_log(format!("{tname}: {}", crate::lang::tr_ui(Condition::Restrained.name())));
        }
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
        // Hurt: the hero's Hellish Rebuke at the attacker.
        if total > 0 {
            self.hurt_reaction(who, target);
        }
        true
    }

    // ----------------------------------------------------------------------- damage, 0 HP

    /// Damage in a fight: through the target's resistances; the hero and party members fall
    /// at 0 HP (death saves), others die.
    pub(super) fn combat_damage(&mut self, target: Who, amount: i32, t: Option<DamageType>, crit: bool, by: Option<Who>) {
        self.combat_damage_parts(target, &[(amount, t)], crit, by, crate::srd::HitKind::Plain);
    }

    /// [`Self::combat_damage`] of a magic source (a spell): through resistances to nonmagical
    /// attacks.
    pub(super) fn combat_damage_magic(&mut self, target: Who, amount: i32, t: Option<DamageType>, by: Option<Who>) {
        self.combat_damage_parts(target, &[(amount, t)], false, by, crate::srd::HitKind::Magical);
    }

    /// The target's defenses in a fight: its own (the hero's race and items, a stat block's)
    /// and its buffs' resistances.
    fn fight_defenses(&self, target: Who) -> crate::srd::Defenses {
        let mut defenses = match target {
            Who::Player => self.player_defenses(),
            _ => self.who_index(target).and_then(|i| self.stat_block(&self.loaded.entities[i]).map(|s| s.defenses())).unwrap_or_default(),
        };
        defenses.resistances.extend(self.buff_resistances(target));
        defenses
    }

    /// Whether the player chooses what a hit does that drops `target` to 0 HP: a melee hit by the
    /// fighter whose turn the player plays on a fighter of the other side.
    fn asks_knock_out(&self, who: Who, target: Who, spec: &AttackSpec, parts: &[(i32, Option<DamageType>)]) -> bool {
        let Some(c) = &self.fight else { return false };
        let (Some(me), Some(them)) = (c.fighter(who), c.fighter(target)) else { return false };
        if spec.ranged || spec.spell || c.current().who != who || c.current().ai || me.side != Side::Party || them.side == Side::Party || them.down || them.out {
            return false;
        }
        let Some(i) = self.who_index(target) else { return false };
        // (A beast's shape at 0 HP: the creature is back, not down.)
        if self.loaded.entities[i].shape.is_some() || self.loaded.entities[i].summon.is_some() {
            return false;
        }
        let hp = self.loaded.entities[i].hp;
        let defenses = self.fight_defenses(target);
        let n: i32 = parts.iter().map(|(a, t)| defenses.apply_hit(*a, *t, spec.hit)).sum();
        hp > 0 && n >= hp
    }

    /// The player's answer to Kill / Knock out: the held hit lands.
    pub(super) fn finish_blow(&mut self, knock_out: bool) {
        let Some(b) = self.fight.as_mut().and_then(|c| c.pending_blow.take()) else { return };
        if let Some(c) = &mut self.fight {
            c.knocking = knock_out;
        }
        self.combat_damage_parts(b.target, &b.parts, b.crit, Some(b.who), b.hit);
        if let Some(c) = &mut self.fight {
            c.knocking = false;
        }
    }

    /// Damage of several parts (a dragon's bite: piercing and fire) in a fight: each part
    /// through the target's defenses (`hit`: a magic weapon gets through resistance to
    /// nonmagical attacks), one hit of the sum.
    pub(super) fn combat_damage_parts(&mut self, target: Who, parts: &[(i32, Option<DamageType>)], crit: bool, by: Option<Who>, hit: crate::srd::HitKind) {
        // (Fire on a troll: no regeneration at the start of its next turn.)
        self.regeneration_hit(target, &parts.iter().filter(|p| p.0 > 0).map(|p| p.1).collect::<Vec<_>>());
        if self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| f.mon.regen_down) {
            return;
        }
        let defenses = self.fight_defenses(target);
        // (Rage's resistance counts per part: bludgeoning, piercing and slashing only.)
        let mut n: i32 = parts.iter().map(|(a, t)| self.rage_halves(target, defenses.apply_hit(*a, *t, hit), *t)).sum();
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
        // Temporary hit points (rage's resistance was counted per part).
        if !self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| f.down) {
            n = self.class_absorb(target, n, None);
        }
        let party = self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| f.side == Side::Party);
        let down = self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| f.down);
        let head = self.who_pos(target) - vec2(0.0, self.ts() * 1.1);
        if down {
            // Damage while down: a failed death save (a crit, two); as much as the maximum HP
            // kills outright.
            if n > 0 && rules::massive_damage(0, n, self.who_max_hp(target)) {
                self.dies_instantly(target);
                return;
            }
            let out = self.fight.as_mut().and_then(|c| c.fighter_mut(target)).map(|f| f.saves.hit(crit));
            if out == Some(DeathOutcome::Dead) {
                self.dies(target);
            }
            return;
        }
        if n <= 0 {
            return;
        }
        // (Asleep from a poison: the hit wakes it.)
        self.wake_poisoned(target);
        let kind = if party { NumberKind::Hurt } else if crit { NumberKind::Crit } else { NumberKind::Hit };
        self.damage_number(head, n, kind);
        self.audio.play(&self.project.config.sfx.hit);
        match target {
            Who::Player => {
                // (A beast's shape takes it: at 0 HP the hero is back, the rest carries over.)
                let (before, n) = self.shape_takes(self.hp(), n);
                let mut hp = (before - n).max(0);
                if hp == 0 && self.relentless(target) {
                    hp = 1;
                }
                self.world.player_hp = Some(hp);
                self.hurt_feedback();
                if hp == 0 && rules::massive_damage(before, n, self.player_max_hp()) {
                    self.dies_instantly(target);
                } else if hp == 0 {
                    self.falls(target);
                } else {
                    // (Hurt and sick: a disease's fit, Cackle Fever's laughter.)
                    self.disease_fit();
                }
            }
            Who::Npc(_) => {
                let Some(i) = self.who_index(target) else { return };
                let e = &mut self.loaded.entities[i];
                let before = e.hp;
                e.hp = (e.hp - n).max(0);
                if e.hp == 0 && party && self.relentless(target) {
                    if let Some(i) = self.who_index(target) {
                        self.loaded.entities[i].hp = 1;
                    }
                }
                let Some(i) = self.who_index(target) else { return };
                let e = &mut self.loaded.entities[i];
                e.flash = super::juice::HIT_FLASH;
                e.hurt = 0.25;
                if e.bar <= 0.0 && e.max_hp > 0 {
                    e.bar_lag = before.max(0) as f32 / e.max_hp as f32;
                }
                e.bar = super::juice::BAR_TIME;
                // (A character in a beast's shape: itself again, the rest carried over.)
                if e.hp == 0 && e.shape.is_some() {
                    let source = e.shape.as_ref().map(|s| s.source.clone()).unwrap_or_default();
                    self.revert_npc(i, n - before);
                    self.spell_over(&source);
                }
                let e = &self.loaded.entities[i];
                if e.hp == 0 {
                    let max = e.max_hp;
                    let knocking = self.fight.as_ref().is_some_and(|c| c.knocking);
                    if party && rules::massive_damage(before, n, max) {
                        self.dies_instantly(target);
                    } else if party {
                        self.falls(target);
                    } else if knocking {
                        self.knocked_out(target);
                    } else if !self.regeneration_falls(target) {
                        self.dies(target);
                    }
                } else {
                    self.hp_dropped(i, before);
                }
            }
        }
        self.after_damage(target, by);
        // (Concentration: a CON save, the spell ends on a failure.)
        if n > 0 {
            self.concentration_check(target, n);
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
                    // (A summoned creature vanishes.)
                    if self.loaded.entities[i].summon.is_some() {
                        self.unsummon(i);
                        self.check_over();
                        return;
                    }
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

    /// Massive damage: dead at once, no death saves ("dies instantly").
    fn dies_instantly(&mut self, who: Who) {
        let name = self.who_name(who);
        self.fight_log(format!("{name}: {}", crate::lang::tr_ui("massive damage")));
        self.dies(who);
    }

    /// A foe the player chose to knock out: out of the fight, unconscious and stable; it counts
    /// as defeated (experience, loot, *Defeat* triggers) but lies where it fell until the hero
    /// leaves the map (saved as defeated), and `knocked_out` remembers it.
    fn knocked_out(&mut self, who: Who) {
        let name = self.who_name(who);
        if let Some(c) = &mut self.fight {
            if let Some(f) = c.fighter_mut(who) {
                f.out = true;
                f.down = false;
            }
            c.log(format!("{name} {}", crate::lang::tr_ui("is knocked out")));
        }
        if let Some(i) = self.who_index(who) {
            self.knock_out_entity(i);
        }
        self.check_over();
    }

    /// The hit points `who` has at most.
    pub(super) fn who_max_hp(&self, who: Who) -> i32 {
        match who {
            Who::Player => self.player_max_hp(),
            _ => self.who_index(who).map_or(0, |i| self.loaded.entities[i].max_hp),
        }
    }

    /// The current fighter is down: a death saving throw (into the roll log).
    fn death_save(&mut self, who: Who) {
        // (Beacon of Hope: advantage.)
        let hope = conditions::buffs(self.who_conditions(who)).any(|b| matches!(b, conditions::Buff::Hope));
        let d = if hope { crate::srd::dice::die(20).max(crate::srd::dice::die(20)) } else { crate::srd::dice::die(20) };
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
                // (~ surprised, ^attack / ^SPELL readied, >NAME grappling.)
                let mut more = String::new();
                if f.surprised && c.round == 1 {
                    more.push('~');
                }
                if f.readied {
                    more.push_str("^attack");
                }
                if let Some((s, _)) = &f.readied_spell {
                    more.push_str(&format!("^{s}"));
                }
                if let Some(g) = f.grappling {
                    more.push_str(&format!(">{}", self.who_key(g)));
                }
                let n = self.space_n(f.who);
                if n > 1 {
                    more.push_str(&format!("[{n}x{n}]"));
                }
                if self.squeezed(f.who) {
                    more.push_str("#squeezed");
                }
                // (A casting monster's slots left: `$1:2,2:1`.)
                if let Some(left) = self.monster_slots_left(f.who) {
                    more.push_str(&format!("${left}"));
                }
                format!("{side}{}{mark}{more}:{hp}/{max}", self.who_key(f.who))
            })
            .collect();
        let aim = match &c.aim {
            Aim::Move => String::new(),
            Aim::Attack => ":aim=attack".into(),
            Aim::OffHand => ":aim=offhand".into(),
            Aim::Help => ":aim=help".into(),
            Aim::Grapple => ":aim=grapple".into(),
            Aim::Shove => ":aim=shove".into(),
            Aim::Cast(s, slot) => format!(":aim=cast:{s}@{slot}"),
            Aim::Feature(s) => format!(":aim=class:{s}"),
        } + c.menu.as_ref().map_or(String::new(), |m| format!(":menu={}:{}/{}", m.kind.id(), m.sel, m.rows.len())).as_str();
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
    /// `shove NAME prone|push`, `cast SPELL [NAME | X Y] [slot N]`, `aim SPELL [slot N] [X Y]`, `class ACT [NAME]`, `use ITEM`, `end`, `log [N]`, `ai NAME on|off`,
    /// `fast [on|off]`, `start`). Returns what happened (or why not).
    pub fn turn_command(&mut self, line: &str) -> String {
        let a: Vec<&str> = line.split_whitespace().collect();
        let arg = |k: usize| a.get(k).copied().unwrap_or("");
        match arg(0) {
            "start" => return if self.start_combat(false, false) { "fight".into() } else { "no fight".into() },
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
        // The hero's attacks as a fight would roll them (also outside fights): name, bonus, dice,
        // melee reach or ranged range/long in tiles, finesse, disadvantage.
        if arg(0) == "specs" {
            return self
                .attack_specs(Who::Player)
                .iter()
                .map(|s| {
                    let how = if s.ranged { format!("ranged{}/{}", s.range, s.long) } else { format!("melee{}", s.reach) };
                    format!("{}:{:+}:{}:{how}{}{}", s.name, s.bonus, s.dice, if s.finesse { ":finesse" } else { "" }, if s.disadvantage { ":dis" } else { "" })
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
        if self.fight.is_none() {
            return "no fight".into();
        }
        // The fight log's last lines (newest last).
        if arg(0) == "log" {
            let c = self.fight.as_ref().unwrap();
            let n: usize = arg(1).parse().unwrap_or(8);
            return c.log[c.log.len().saturating_sub(n)..].join(" | ");
        }
        // (The Reaction window is answered on any fighter's turn.)
        if arg(0) == "pick" && self.fight_menu_open() && !self.players_turn() {
            return match arg(1).parse::<usize>() {
                Ok(k) => self.pick_fight_menu_row(k).map_or_else(|e| e.to_string(), |_| "ok".into()),
                Err(_) => "pick K".into(),
            };
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
            // `attack NAME` strikes (walking up first); `attack NAME throw` throws the weapon
            // (Attack › Throw), `attack NAME strike` as Attack › Strike.
            "attack" => match target(self, 1) {
                Ok(t) => {
                    let tile = self.who_tile(t);
                    let c = self.fight.as_mut().unwrap();
                    c.menu = None;
                    c.cursor = tile;
                    match arg(2) {
                        "throw" | "strike" => {
                            c.aim = Aim::Attack;
                            c.attack_mode = Some(arg(2) == "throw");
                            done(self.act_on(&Aim::Attack, t))
                        }
                        _ => {
                            c.aim = Aim::Move;
                            self.confirm_cursor();
                            "ok".into()
                        }
                    }
                }
                Err(e) => e,
            },
            // The choice window: `menu` lists its rows, `pick K` picks row K.
            "menu" => self.fight_menu_status(),
            "pick" => match arg(1).parse::<usize>() {
                Ok(k) => done(self.pick_fight_menu_row(k)),
                Err(_) => "pick K".into(),
            },
            "action" | "end" => {
                let id = if arg(0) == "end" { "end" } else { arg(1) };
                match TurnAct::parse(id) {
                    Some(act) => done(self.take_act(act)),
                    None => format!("no action {id}"),
                }
            }
            // `release`: the current fighter lets go of the creature it grapples.
            "release" => {
                let who = self.fight.as_ref().unwrap().current().who;
                if self.fight.as_ref().unwrap().current().grappling.is_none() {
                    return "not grappling".into();
                }
                self.release_grapple(who);
                "ok".into()
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
            "aim" => {
                // `aim SPELL [slot N] [X Y]`: the spell aimed, the cursor on the tile (screenshots).
                let me = self.fight.as_ref().unwrap().current().who;
                let level = self.project.db.spell(arg(1)).map_or(1, |d| d.level);
                let mut words: Vec<&str> = a[2..].to_vec();
                let mut slot = None;
                if let Some(k) = words.iter().position(|w| *w == "slot") {
                    slot = words.get(k + 1).and_then(|n| n.parse::<u32>().ok());
                    words.drain(k..(k + 2).min(words.len()));
                }
                let slot = slot.unwrap_or_else(|| if level == 0 { 0 } else { self.lowest_slot(me, level).unwrap_or(level) });
                self.aim_spell(arg(1), slot);
                if let ([x, y], Some(c)) = (words.as_slice(), self.fight.as_mut()) {
                    c.cursor = (x.parse().unwrap_or(c.cursor.0), y.parse().unwrap_or(c.cursor.1));
                }
                "ok".into()
            }
            "cast" => {
                // `cast SPELL [NAME | X Y] [slot N]`: at a fighter, a tile, or the cursor's aim.
                let me = self.fight.as_ref().unwrap().current().who;
                let words: Vec<&str> = a[2..].to_vec();
                let (rest, slot) = match words.iter().position(|w| *w == "slot") {
                    Some(k) => (words[..k].to_vec(), words.get(k + 1).and_then(|n| n.parse::<u32>().ok())),
                    None => (words.clone(), None),
                };
                let id = arg(1);
                let level = self.project.db.spell(id).map_or(1, |d| d.level);
                let slot = slot.unwrap_or_else(|| if level == 0 { 0 } else { self.lowest_slot(me, level).unwrap_or(level) });
                let at = match rest.as_slice() {
                    [] => {
                        self.aim_spell(id, slot);
                        let cur = self.fight.as_ref().unwrap().cursor;
                        if self.project.db.spell(id).is_some_and(|d| d.area.is_some() || d.range == crate::skills::SpellRange::Caster) { CastAt::Tile(cur) } else { CastAt::On(self.who_at(cur).unwrap_or(me)) }
                    }
                    [x, y] if x.parse::<i32>().is_ok() => CastAt::Tile((x.parse().unwrap_or(0), y.parse().unwrap_or(0))),
                    [name] => match self.who_by_key(name) {
                        Some(w) => CastAt::On(w),
                        None => return format!("no fighter {name}"),
                    },
                    _ => return "cast SPELL [NAME | X Y] [slot N]".into(),
                };
                // (A wand's spell picked in the Cast window: from the wand.)
                let r = self.cast_item_spell(id, at).unwrap_or_else(|| self.cast_spell_in_fight(id, slot, at, false));
                if let Some(c) = &mut self.fight {
                    c.aim = Aim::Move;
                }
                done(r)
            }
            "class" => {
                // A class action: on the named fighter, else the nearest fitting one, else itself.
                let me = self.fight.as_ref().unwrap().current().who;
                let t = if arg(2).is_empty() { self.class_act_default_target(arg(1)).unwrap_or(me) } else { match target(self, 2) { Ok(t) => t, Err(e) => return e } };
                done(self.use_class_act(arg(1), t))
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
    /// Autoplay `waitturn`: a fight is on and it is the hero's turn, nothing moving on screen
    /// and no finishing blow waiting.
    pub fn heros_turn(&self) -> bool {
        self.fight.as_ref().is_some_and(|c| c.current().who == Who::Player && !c.current().ai && c.walking.is_empty() && !c.over && c.pending_blow.is_none() && c.pending_react.is_none() && !self.runner.busy())
    }

    /// Who as an [`ActiveCondition::by`](crate::srd::ActiveCondition) tag: `player` or the uid.
    pub(super) fn who_tag(&self, who: Who) -> String {
        match who {
            Who::Player => "player".into(),
            Who::Npc(uid) => uid.to_string(),
        }
    }

    /// The fighter a [`Self::who_tag`] names (None = unknown, not on the map).
    pub(super) fn who_of_tag(&self, tag: &str) -> Option<Who> {
        match tag {
            "" => None,
            "player" => Some(Who::Player),
            t => t.parse::<u32>().ok().map(Who::Npc).filter(|w| self.who_index(*w).is_some()),
        }
    }

    pub(super) fn who_key(&self, who: Who) -> String {
        match who {
            Who::Player => "player".into(),
            Who::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map(|e| if e.name.is_empty() { e.def.clone() } else { e.name.clone() }).or_else(|| self.fight.as_ref()?.fighter(who).map(|f| f.key.clone())).unwrap_or_else(|| "?".into()),
        }
    }

    /// A combatant by the name `status` gives it ("player", a name, a definition id).
    pub(super) fn who_by_key(&self, key: &str) -> Option<Who> {
        let c = self.fight.as_ref()?;
        // (Several of that name: the one nearest to whose turn it is.)
        let here = self.who_tile(c.current().who);
        c.order.iter().filter(|f| !f.out).map(|f| f.who).filter(|w| self.who_key(*w).eq_ignore_ascii_case(key)).min_by_key(|w| {
            let t = self.who_tile(*w);
            t.0.abs_diff(here.0).max(t.1.abs_diff(here.1))
        })
    }
}

/// The first dice of an expression ("1d8+3" -> (1, 8)).
fn first_dice(expr: &str) -> Option<(u32, u32)> {
    let t = expr.trim();
    let end = t.find(['+', '-']).unwrap_or(t.len());
    let (n, sides) = t[..end].split_once('d')?;
    Some((if n.is_empty() { 1 } else { n.parse().ok()? }, sides.parse().ok()?))
}

/// Rolls damage dice ("2d6+3"), at least 1; `reroll_low`: a die showing 1 or 2 is rolled again
/// once (Great Weapon Fighting).
pub(super) fn roll_damage(expr: &str, reroll_low: bool) -> i32 {
    if reroll_low {
        if let Some((n, sides)) = first_dice(expr) {
            let rest: i32 = expr.trim().find(['+', '-']).map_or(0, |k| expr.trim()[k..].replace('+', "").trim().parse().unwrap_or(0));
            let mut total = 0;
            for _ in 0..n {
                let mut d = crate::srd::dice::die(sides);
                if d <= 2 {
                    d = crate::srd::dice::die(sides);
                }
                total += d as i32;
            }
            return (total + rest).max(1);
        }
    }
    crate::srd::Expr::parse(expr).map(|e| e.roll(&crate::srd::dice::NoMods).total.max(1)).unwrap_or(1) as i32
}

/// A diagonal step is allowed unless both corner tiles beside it are walls (creatures never
/// block a corner: the SRD has no corner rule; two walls touching at a corner can't be squeezed
/// through).
pub(super) fn diagonal_ok(corner_a_wall: bool, corner_b_wall: bool) -> bool {
    !(corner_a_wall && corner_b_wall)
}

/// Steps from each tile to the nearest of `goals` (8 directions, diagonals by [`diagonal_ok`]),
/// over tiles `closed` doesn't close, out to `limit` steps. The goals themselves may be closed
/// (a foe's tile).
pub(super) fn distance_field(goals: &[TilePos], limit: i32, closed: impl Fn(TilePos) -> bool) -> std::collections::HashMap<TilePos, i32> {
    let mut dist: std::collections::HashMap<TilePos, i32> = goals.iter().map(|g| (*g, 0)).collect();
    let mut queue: std::collections::VecDeque<TilePos> = goals.iter().copied().collect();
    while let Some(t) = queue.pop_front() {
        let d = dist[&t];
        if d >= limit {
            continue;
        }
        for s in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let n = (t.0 + s.0, t.1 + s.1);
            if dist.contains_key(&n) || closed(n) {
                continue;
            }
            if s.0 != 0 && s.1 != 0 && !diagonal_ok(closed((t.0 + s.0, t.1)), closed((t.0, t.1 + s.1))) {
                continue;
            }
            dist.insert(n, d + 1);
            queue.push_back(n);
        }
    }
    dist
}

#[cfg(test)]
mod reach_tests {
    use super::*;

    #[test]
    fn a_path_leads_through_an_allys_tile() {
        // Start (0,0); an ally on (1,0) is passed on the way to (2,0), which may be ended on.
        let tiles = [((0, 0), (0, (0, 0))), ((2, 0), (10, (1, 0)))].into_iter().collect();
        let passed = [((1, 0), (5, (0, 0)))].into_iter().collect();
        let reach = Reach { tiles, passed };
        assert!(!reach.contains_key(&(1, 0)), "an ally's tile isn't a place to stop");
        assert_eq!(Game::path_to(&reach, (0, 0), (2, 0)), vec![(1, 0), (2, 0)]);
    }

    #[test]
    fn diagonals_are_blocked_only_between_two_walls() {
        assert!(diagonal_ok(false, false) && diagonal_ok(true, false) && diagonal_ok(false, true));
        assert!(!diagonal_ok(true, true));
    }

    #[test]
    fn the_distance_field_walks_round_a_wall() {
        // A wall from (2,-3) to (2,3) between the goal (0,0) and (4,0): the way round it is longer
        // than the straight line.
        let wall = |t: TilePos| t.0 == 2 && (-3..=3).contains(&t.1);
        let d = distance_field(&[(0, 0)], 40, wall);
        assert_eq!(d.get(&(1, 0)), Some(&1));
        assert!(!d.contains_key(&(2, 0)), "a wall tile has no distance");
        assert_eq!(d.get(&(4, 0)), Some(&8), "round the end of the wall: 4 up, 4 down");
        assert_eq!(distance_field(&[(0, 0)], 3, wall).get(&(4, 0)), None, "beyond the limit");
    }
}
