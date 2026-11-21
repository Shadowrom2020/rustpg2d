//! Shapes and summons (the SRD's Wild Shape, *Polymorph*, *Find Familiar*, *Conjure Animals*).
//!
//! **Shapes.** The hero in a beast's shape (`WorldState::shape`) plays with the beast's stat block:
//! its hit points (the hero's own come back when the shape ends; a drop to 0 carries the rest of
//! the damage over), AC, speeds, size, attacks and senses, its STR, DEX and CON (Polymorph: all six
//! scores); no spells. Wild Shape is a class action (`ActEffect::Shape`), *Polymorph* a spell part
//! (`SpellPart::Shape`); the hero chooses among the beasts they know (defeated ones and those on
//! this map) in a window. A character in a shape is a new runtime entity of the beast that keeps
//! the old one (`RtEntity::shape`) until it ends. Shapes end with their time, the spell's
//! concentration, 0 hit points, or (the hero's Wild Shape) using it again as a bonus action.
//!
//! **Summons.** `SpellPart::Summon` puts creatures of a character definition next to the caster
//! (`RtEntity::summon`), on its side: the party's are companions the AI plays (the orders menu
//! works), joining a fight at once. They go with the spell's time or concentration, at 0 hit
//! points, and when the map is left (they aren't saved).

use macroquad::prelude::*;

use super::turns::{Fighter, Side, Who};
use super::{Game, RtEntity};
use crate::entity::EntityKind;
use crate::grid::{feet_spot, feet_tile};
use crate::lang::tr_ui;
use crate::path::TilePos;
use crate::srd::StatBlock;
use crate::state::HeroShape;

/// A character in another shape: the one it was, back when the shape ends.
pub(super) struct NpcShape {
    pub orig: RtEntity,
    pub source: String,
    /// Seconds left (0 = until it ends otherwise).
    pub left: f32,
}

impl NpcShape {
    /// The spell that gave it (Dispel Magic).
    pub fn orig_source(&self) -> String {
        self.source.clone()
    }
}

/// A summoned creature: what summoned it (a spell's source) and its time.
#[derive(Clone, Debug)]
pub(super) struct Summoned {
    pub source: String,
    /// Seconds left (0 = until it drops to 0 HP or the map is left).
    pub left: f32,
}

/// A shape waiting for the hero's choice of beast (the window's rows).
#[derive(Clone, Debug)]
pub(super) struct ShapePick {
    /// The class action it uses (Wild Shape: its resource and cost are spent on the choice).
    pub act: Option<String>,
    pub source: String,
    pub seconds: f32,
    /// The hero keeps INT, WIS and CHA (Wild Shape; Polymorph replaces all six).
    pub keep_mind: bool,
    pub forms: Vec<String>,
    /// The list's title outside fights (the class action's or the spell's name).
    pub title: String,
}

/// Seconds a game hour of a shape lasts (as a spell's: 600 rounds of 6 s).
const HOUR: f32 = 3600.0;

impl Game {
    // ------------------------------------------------------------------ the beasts

    /// A character definition's stat block when it is a beast.
    fn beast_block(&self, id: &str) -> Option<&StatBlock> {
        self.project.db.character(id).and_then(|c| c.stats.as_ref()).filter(|b| b.creature_type.to_ascii_lowercase().contains("beast"))
    }

    /// The beasts the hero knows (defeated ones, the bestiary's, and those on this map) of a
    /// challenge rating up to `max_cr`, flying (`fly`) and swimming (`swim`) ones only when
    /// allowed: strongest first.
    pub(super) fn known_beasts(&self, max_cr: f32, fly: bool, swim: bool) -> Vec<String> {
        let mut ids: Vec<String> = self.world.bestiary.keys().cloned().collect();
        ids.extend(self.loaded.entities.iter().filter(|e| e.active && e.kind == EntityKind::Character && e.summon.is_none()).map(|e| e.def.clone()));
        ids.sort();
        ids.dedup();
        self.fitting_beasts(ids, max_cr, fly, swim)
    }

    /// Of `ids`, the beasts within the limits: strongest first, then by name.
    fn fitting_beasts(&self, ids: Vec<String>, max_cr: f32, fly: bool, swim: bool) -> Vec<String> {
        let mut out: Vec<(f32, String, String)> = ids
            .into_iter()
            .filter_map(|id| {
                let b = self.beast_block(&id)?;
                let cr = crate::srd::encounter::parse_cr(&b.cr)?;
                (cr <= max_cr + 0.001 && (fly || b.speeds.fly == 0) && (swim || b.speeds.swim == 0)).then(|| (cr, self.project.db.character(&id).map_or(id.clone(), |c| c.name.clone()), id))
            })
            .collect();
        out.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        out.into_iter().map(|x| x.2).collect()
    }

    /// The project's beasts up to `max_cr` (a foe polymorphed becomes the weakest).
    fn all_beasts(&self, max_cr: f32) -> Vec<String> {
        let ids = self.project.db.characters.iter().map(|c| c.id.clone()).collect();
        self.fitting_beasts(ids, max_cr, true, true)
    }

    /// A creature's challenge rating, or the hero's (and a party member's) level.
    fn level_or_cr(&self, who: Who) -> f32 {
        match who {
            Who::Player => self.world.player_level() as f32,
            Who::Npc(uid) => match self.member_class(uid) {
                Some((_, level)) => level as f32,
                None => self.who_block(who).and_then(|b| crate::srd::encounter::parse_cr(&b.cr)).unwrap_or(0.0),
            },
        }
    }

    // ------------------------------------------------------------------ the hero

    /// The hero's shape's stat block (with the hero's ability scores of now: the beast's STR, DEX
    /// and CON were taken over).
    pub(super) fn hero_form_block(&self) -> Option<StatBlock> {
        let s = self.world.shape.as_ref()?;
        let mut b = self.project.db.character(&s.form).and_then(|c| c.stats.clone())?;
        b.abilities = self.world.abilities;
        Some(b)
    }

    /// The hero takes beast `form`'s shape.
    pub(super) fn shape_hero(&mut self, form: &str, source: &str, seconds: f32, keep_mind: bool) {
        let Some(b) = self.project.db.character(form).and_then(|c| c.stats.clone()) else { return };
        if self.world.shape.is_some() {
            self.revert_hero();
        }
        let own = self.world.abilities;
        let mut scores = b.abilities;
        if keep_mind {
            for k in 3..6 {
                scores.0[k] = own.0[k];
            }
        }
        let hero_hp = self.hp();
        self.world.abilities = scores;
        self.world.shape = Some(HeroShape { form: form.to_string(), source: source.to_string(), max_hp: b.hp.max(1), hero_hp, abilities: own, seconds });
        self.world.player_hp = Some(b.hp.max(1));
        // (No spells in a beast's shape; concentration on one goes on.)
        self.cancel_aim();
        let name = self.who_name(Who::Player);
        let beast = self.project.db.character(form).map_or(form.to_string(), |c| crate::lang::tr(&c.name));
        self.shape_note(format!("{name} {} {beast}", tr_ui("takes the shape of")));
        self.shape_puff(self.player.pos);
    }

    /// The hero's own shape is back (its hit points and scores). Returns its hit points.
    pub(super) fn revert_hero(&mut self) -> i32 {
        let Some(s) = self.world.shape.take() else { return self.hp() };
        self.world.abilities = s.abilities;
        let hp = s.hero_hp.clamp(0, self.player_max_hp());
        self.world.player_hp = Some(hp);
        let name = self.who_name(Who::Player);
        self.shape_note(format!("{name} {}", tr_ui("is back in their own shape")));
        self.shape_puff(self.player.pos);
        hp
    }

    /// Damage `n` on the hero in a shape that drops it to 0: the shape ends and the rest is what
    /// the hero's own hit points take. Returns (the HP before, the damage left).
    pub(super) fn shape_takes(&mut self, before: i32, n: i32) -> (i32, i32) {
        if self.world.shape.is_none() || before - n > 0 {
            return (before, n);
        }
        let over = n - before;
        let source = self.world.shape.as_ref().map(|s| s.source.clone()).unwrap_or_default();
        let hp = self.revert_hero();
        self.spell_over(&source);
        (hp, over)
    }

    /// A spell's shape dropped to 0 HP: its caster's concentration on it ends too.
    pub(super) fn spell_over(&mut self, source: &str) {
        let Some((_, by)) = source.split_once('@') else { return };
        let who = if by == "player" { Who::Player } else { match by.parse() { Ok(uid) => Who::Npc(uid), Err(_) => return } };
        if self.concentration_of(who).as_deref() == Some(source) {
            self.end_concentration(who);
        }
    }

    // ------------------------------------------------------------------ characters

    /// Character `i` takes beast `form`'s shape (a new entity of the beast, keeping the old one).
    pub(super) fn shape_npc(&mut self, i: usize, form: &str, source: &str, seconds: f32) {
        if self.project.db.character(form).is_none() {
            return;
        }
        let (uid, pos, facing) = {
            let e = &self.loaded.entities[i];
            (e.uid, e.body.pos, e.body.facing)
        };
        let mut new = self.new_rt_entity(EntityKind::Character, form, uid, "", pos, facing, true, 0);
        let old = &mut self.loaded.entities[i];
        // (It stays who it is: its side, its party link, its conditions.)
        new.hostile = old.hostile;
        new.faction = old.faction;
        new.faction_placed = old.faction_placed.clone();
        new.faction_set = old.faction_set.clone();
        new.companion = old.companion.take();
        new.effects = std::mem::take(&mut old.effects);
        new.home = old.home;
        new.name = old.name.clone();
        new.spawned_by = old.spawned_by;
        new.summon = old.summon.clone();
        let name = self.display_name(i);
        let orig = std::mem::replace(&mut self.loaded.entities[i], new);
        self.loaded.entities[i].shape = Some(Box::new(NpcShape { orig, source: source.to_string(), left: seconds }));
        self.occupy(i);
        let beast = self.project.db.character(form).map_or(form.to_string(), |c| crate::lang::tr(&c.name));
        self.shape_note(format!("{name} {} {beast}", tr_ui("takes the shape of")));
        self.shape_puff(pos);
    }

    /// Character `i` is itself again, `over` damage carried over (at 0 HP it falls or dies).
    pub(super) fn revert_npc(&mut self, i: usize, over: i32) {
        let Some(s) = self.loaded.entities[i].shape.take() else { return };
        let mut orig = s.orig;
        let cur = &mut self.loaded.entities[i];
        orig.body.pos = cur.body.pos;
        orig.body.facing = cur.body.facing;
        orig.hostile = cur.hostile;
        orig.faction = cur.faction;
        orig.faction_set = cur.faction_set.clone();
        orig.companion = cur.companion.take();
        orig.effects = std::mem::take(&mut cur.effects);
        orig.hp = (orig.hp - over.max(0)).max(0);
        let pos = orig.body.pos;
        self.loaded.entities[i] = orig;
        self.occupy(i);
        let name = self.display_name(i);
        self.shape_note(format!("{name} {}", tr_ui("is back in its own shape")));
        self.shape_puff(pos);
    }

    // ------------------------------------------------------------------ choosing

    /// Opens the window with the beasts of `pick` (in a fight the choice window, else the action
    /// bar's list). Err when the hero knows none.
    pub(super) fn ask_shape(&mut self, pick: ShapePick) -> Result<(), &'static str> {
        if pick.forms.is_empty() {
            return Err("You know no beast of that kind");
        }
        let rows: Vec<(String, String, String)> = pick
            .forms
            .iter()
            .map(|id| {
                let name = self.project.db.character(id).map_or(id.clone(), |c| crate::lang::tr(&c.name));
                let detail = self.beast_block(id).map_or(String::new(), |b| format!("CR {}, {} HP, AC {}", b.cr, b.hp, b.ac));
                (name, detail, id.clone())
            })
            .collect();
        self.shape_pick = Some(pick);
        if self.fight.is_some() {
            use super::turns::menu::{MenuKind, MenuPick, MenuRow};
            let rows = rows.into_iter().map(|(label, detail, id)| MenuRow { label, detail, why: None, pick: MenuPick::Shape(id) }).collect();
            self.open_fight_menu(MenuKind::Shape, rows);
        } else {
            let rows = rows.into_iter().map(|(label, detail, id)| super::actionbar::BarRow { label, detail, why: None, pick: super::actionbar::BarPick::Shape(id) }).collect();
            let title = self.shape_pick.as_ref().map_or(String::new(), |p| p.title.clone());
            self.open_bar_list(crate::menus::BarEntry::Custom { label: title, function: String::new(), detail: String::new() }, rows);
        }
        Ok(())
    }

    /// The beast chosen in the window: the shape starts (Wild Shape's use and action are spent).
    pub(super) fn pick_shape(&mut self, form: &str) {
        let Some(p) = self.shape_pick.take() else { return };
        if !p.forms.iter().any(|f| f == form) {
            return;
        }
        if let Some(act) = &p.act {
            self.pay_class_act(Who::Player, act);
        }
        self.shape_hero(form, &p.source, p.seconds, p.keep_mind);
    }

    /// Wild Shape (`ActEffect::Shape`) of the hero: the window with the beasts its level allows,
    /// or, already in a shape, back to its own.
    pub(super) fn wild_shape(&mut self, act: &crate::classes::ClassAct) -> Result<(), &'static str> {
        let crate::classes::ActEffect::Shape { max_cr, fly_from, swim_from, hours_per_level } = &act.effect else { return Err("No such class action") };
        if self.world.shape.is_some() {
            self.revert_hero();
            return Ok(());
        }
        let level = self.who_traits(Who::Player).map_or(1, |t| t.level);
        let cr = max_cr.iter().filter(|(l, _)| *l <= level).filter_map(|(_, c)| crate::srd::encounter::parse_cr(c)).fold(-1.0f32, f32::max);
        if cr < 0.0 {
            return Err("Not at this level");
        }
        let forms = self.known_beasts(cr, level >= *fly_from, level >= *swim_from);
        let hours = ((hours_per_level * level as f32).floor()).max(1.0);
        self.ask_shape(ShapePick { act: Some(act.id.clone()), source: act.id.clone(), seconds: hours * HOUR, keep_mind: true, forms, title: act.name.clone() })
    }

    /// A spell's *Shape* part on `target` by `caster` (Polymorph): the hero casting it on
    /// themselves chooses; a foe becomes the weakest beast it may, an ally the strongest.
    pub(super) fn spell_shape(&mut self, caster: Who, target: Who, max_cr: &str, source: &str, seconds: f32) {
        let cr = crate::srd::encounter::parse_cr(max_cr).unwrap_or_else(|| self.level_or_cr(target));
        let foe = self.fight.as_ref().is_some_and(|c| c.fighter(caster).map(|f| f.side) != c.fighter(target).map(|f| f.side));
        let mut forms = self.all_beasts(cr);
        if target == Who::Player && caster == Who::Player {
            let known = self.known_beasts(cr, true, true);
            let title = self.project.db.spell(source.split('@').next().unwrap_or("")).map_or(String::new(), |d| d.name.clone());
            if let Err(why) = self.ask_shape(ShapePick { act: None, source: source.to_string(), seconds, keep_mind: false, forms: known, title }) {
                self.shape_note(tr_ui(why).into_owned());
            }
            return;
        }
        if foe {
            forms.reverse();
        }
        let Some(form) = forms.first().cloned() else { return };
        match target {
            Who::Player => self.shape_hero(&form, source, seconds, false),
            Who::Npc(_) => {
                if let Some(i) = self.who_index(target) {
                    self.shape_npc(i, &form, source, seconds);
                }
            }
        }
    }

    // ------------------------------------------------------------------ summons

    /// A free tile at or around `t` (rings outward, up to 3 tiles).
    fn free_tile_near(&self, t: TilePos, taken: &[TilePos]) -> Option<TilePos> {
        let ts = self.ts();
        let busy = |p: TilePos| {
            taken.contains(&p) || feet_tile(self.player.pos, ts) == p || self.loaded.entities.iter().any(|e| e.active && e.solid && e.kind == EntityKind::Character && feet_tile(e.body.pos, ts) == p) || self.who_at(p).is_some()
        };
        for r in 0..=3i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let p = (t.0 + dx, t.1 + dy);
                    if !self.tile_blocks_npc(p.0, p.1) && !busy(p) {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    /// `count` creatures of `creature` appear around tile `at` (next to the caster, or where the
    /// spell was aimed), on `caster`'s side, for `seconds` (0 =
    /// until they drop to 0 HP or the map is left).
    pub(super) fn summon(&mut self, caster: Who, at: TilePos, creature: &str, count: u32, source: &str, seconds: f32) {
        if self.project.db.character(creature).is_none() {
            eprintln!("[game] Summon: no character '{creature}'");
            return;
        }
        let ts = self.ts();
        let party = match caster {
            Who::Player => true,
            Who::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).is_some_and(|e| e.companion.is_some()),
        };
        let (hostile, faction) = match self.who_index(caster) {
            Some(i) if !party => (self.loaded.entities[i].hostile, self.loaded.entities[i].faction),
            _ => (false, None),
        };
        let mut taken = Vec::new();
        for _ in 0..count.max(1) {
            let Some(t) = self.free_tile_near(at, &taken) else { break };
            taken.push(t);
            let uid = self.next_spawn_uid;
            self.next_spawn_uid += 1;
            let pos = feet_spot(t, ts);
            let mut e = self.new_rt_entity(EntityKind::Character, creature, uid, "", pos, crate::entity::Facing::Down, true, 0);
            e.summon = Some(Summoned { source: source.to_string(), left: seconds });
            e.hostile = hostile;
            if !party {
                e.faction = faction;
            }
            if party {
                // (A companion the AI plays.)
                let k = self.next_companion_slot();
                super::companions::make_summoned(&mut e, k);
            }
            let uid = e.uid;
            self.loaded.entities.push(e);
            let i = self.loaded.entities.len() - 1;
            self.occupy(i);
            self.shape_puff(pos);
            let name = self.display_name(i);
            self.shape_note(format!("{name} {}", tr_ui("appears")));
            self.join_summoned(Who::Npc(uid), if party { Side::Party } else { Side::Enemy });
        }
    }

    /// A summoned creature joins the fight going on (its initiative rolled).
    fn join_summoned(&mut self, who: Who, side: Side) {
        if self.fight.is_none() {
            return;
        }
        let dex = self.who_mod(who, crate::srd::Ability::Dex);
        let init = crate::srd::dice::die(20) as i32 + dex;
        let key = self.who_key(who);
        let f = Fighter { who, key, side, init, dex, surprised: false, ai: true, saves: Default::default(), down: false, out: false, dodging: false, hidden: false, reacted: false, readied: false, readied_spell: None, grappling: None, fx: Default::default(), thrown: Vec::new(), mon: Default::default() };
        if let Some(c) = &mut self.fight {
            c.order.push(f);
        }
    }

    /// Summoned creature `i` is gone (its time, the spell's end, 0 HP).
    pub(super) fn unsummon(&mut self, i: usize) {
        let e = &self.loaded.entities[i];
        if e.summon.is_none() {
            return;
        }
        let (uid, pos) = (e.uid, e.body.pos);
        let name = self.display_name(i);
        if let Some(c) = &mut self.fight {
            if let Some(f) = c.order.iter_mut().find(|f| f.who == Who::Npc(uid)) {
                f.out = true;
                f.down = false;
            }
        }
        self.loaded.entities.remove(i);
        self.shape_note(format!("{name} {}", tr_ui("vanishes")));
        self.shape_puff(pos);
    }

    // ------------------------------------------------------------------ time and endings

    /// What a spell's source (`polymorph@player`) keeps going ends with it (concentration).
    pub(super) fn end_shapes_of(&mut self, source: &str) {
        if self.world.shape.as_ref().is_some_and(|s| s.source == source) {
            self.revert_hero();
        }
        while let Some(i) = self.loaded.entities.iter().position(|e| e.shape.as_ref().is_some_and(|s| s.source == source)) {
            self.revert_npc(i, 0);
        }
        while let Some(i) = self.loaded.entities.iter().position(|e| e.summon.as_ref().is_some_and(|s| s.source == source)) {
            self.unsummon(i);
        }
    }

    /// Time passes for shapes and summons: `dt` seconds outside fights; in a fight a round on
    /// `who`'s turn (`only`).
    pub(super) fn tick_shapes(&mut self, dt: f32, only: Option<Who>) {
        let mine = |w: Who| only.is_none_or(|o| o == w);
        if mine(Who::Player) {
            let over = self.world.shape.as_mut().is_some_and(|s| {
                if s.seconds <= 0.0 {
                    return false;
                }
                s.seconds -= dt;
                s.seconds <= 0.0
            });
            if over {
                self.revert_hero();
            }
        }
        let mut i = 0;
        while i < self.loaded.entities.len() {
            let w = Who::Npc(self.loaded.entities[i].uid);
            if !mine(w) {
                i += 1;
                continue;
            }
            let e = &mut self.loaded.entities[i];
            let shape_over = e.shape.as_mut().is_some_and(|s| s.left > 0.0 && {
                s.left -= dt;
                s.left <= 0.0
            });
            if shape_over {
                self.revert_npc(i, 0);
            }
            let e = &mut self.loaded.entities[i];
            let gone = e.summon.as_mut().is_some_and(|s| s.left > 0.0 && {
                s.left -= dt;
                s.left <= 0.0
            });
            if gone {
                self.unsummon(i);
                continue;
            }
            i += 1;
        }
    }

    // ------------------------------------------------------------------ bits

    fn shape_note(&mut self, line: String) {
        if self.fight.is_some() {
            self.fight_log(line);
        } else {
            self.ui.toast(line);
        }
    }

    fn shape_puff(&mut self, pos: Vec2) {
        let ts = self.ts();
        self.effects.push((pos - vec2(0.0, ts * 0.4), 0.0));
    }

    /// `shape=` for `status`: the hero's (`wolf:11/11:3600s`, `-`) and characters' shapes
    /// (`Orc Warrior>frog:1/1`); `summons=` the summoned creatures (`owl:1/1:-`).
    pub(super) fn shapes_status(&self) -> String {
        let mut out = Vec::new();
        if let Some(s) = &self.world.shape {
            out.push(format!("player>{}:{}/{}:{:.0}s", s.form, self.hp(), s.max_hp, s.seconds));
        }
        for e in &self.loaded.entities {
            if let Some(s) = &e.shape {
                out.push(format!("{}>{}:{}/{}", s.orig.def, e.def, e.hp, e.max_hp));
            }
        }
        let summons: Vec<String> = self.loaded.entities.iter().filter_map(|e| e.summon.as_ref().map(|s| format!("{}:{}/{}:{}", e.def, e.hp, e.max_hp, if s.left > 0.0 { format!("{:.0}s", s.left) } else { "-".into() }))).collect();
        format!("shape={} summons={}", if out.is_empty() { "-".to_string() } else { out.join(",") }, if summons.is_empty() { "-".to_string() } else { summons.join(",") })
    }
}

#[cfg(test)]
mod tests {
    use crate::state::{HeroShape, WorldState};

    #[test]
    fn a_shape_is_saved_and_old_saves_have_none() {
        let w = WorldState { shape: Some(HeroShape { form: "wolf".into(), source: "wild-shape".into(), max_hp: 11, hero_hp: 30, seconds: 3600.0, ..Default::default() }), ..Default::default() };
        let text = w.to_ron().unwrap();
        let back = WorldState::from_ron(&text).unwrap();
        assert_eq!(back.shape, w.shape);
        let plain = WorldState::default().to_ron().unwrap();
        assert!(!plain.contains("shape") && WorldState::from_ron(&plain).unwrap().shape.is_none());
    }

    #[test]
    fn the_library_has_wild_shape_and_the_shape_and_summon_spells() {
        let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../editor/assets/starter_library/defs");
        let spells = std::fs::read_to_string(lib.join("spells.ron")).unwrap();
        let spells: Vec<crate::skills::SpellDef> = ron::from_str(&spells).unwrap();
        let parts = |id: &str| spells.iter().find(|s| s.id == id).map(|s| s.parts.clone()).unwrap_or_default();
        assert!(parts("polymorph").iter().any(|p| matches!(p, crate::skills::SpellPart::Shape { .. })));
        assert!(parts("conjure-animals").iter().any(|p| matches!(p, crate::skills::SpellPart::Summon { creature, count: 2 } if creature == "dire-wolf")));
        let classes = std::fs::read_to_string(lib.join("classes.ron")).unwrap();
        assert!(classes.contains("effect: Shape("), "the druid's Wild Shape");
    }
}
