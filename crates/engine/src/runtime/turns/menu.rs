//! The choice window over the action bar: Cast lists the spells (each slot level it can cast,
//! greyed with the reason when it can't), Class the class actions, Attack with a weapon that can
//! also be thrown asks whether to strike or throw, a finishing blow in melee whether to kill or
//! knock out. Keys, mouse and autoplay (`turn menu`, `turn pick K`) pick a row; Cancel closes it
//! (not the finishing blow's: that one waits for an answer).

use macroquad::prelude::*;

use super::{Aim, Who};
use crate::runtime::input::{self, Act};
use crate::runtime::skin;
use crate::runtime::text::{self, draw_plain, measure_plain, Role};
use crate::runtime::Game;

/// What the window chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum MenuKind {
    Cast,
    Class,
    Attack,
    /// A melee hit drops a foe to 0 HP: kill or knock out.
    Finish,
    /// Ready: an attack or a spell, held for a trigger.
    Ready,
    /// A reaction the hero may take now (Shield, Protection…): the fight waits for it.
    Reaction,
}

/// What picking a row does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum MenuPick {
    /// A spell at a slot level (0 = a cantrip).
    Cast(String, u32),
    /// A class action by its id.
    Class(String),
    /// Attack with the weapon: strike in melee or throw it.
    Attack { thrown: bool },
    /// A spell from an item (a wand) at a slot level, spending its charges.
    ItemCast { item: String, spell: String, slot: u32 },
    /// The held finishing blow kills or knocks out.
    Finish { knock_out: bool },
    /// Ready the attack: when a foe comes within reach.
    ReadyAttack,
    /// Ready a spell at a slot level: when a foe comes within its range.
    ReadySpell(String, u32),
    /// The held attack goes on with this reaction (or none).
    React(super::reactions::ReactChoice),
}

/// The rows of the Kill / Knock out window.
pub(super) fn finish_rows() -> Vec<MenuRow> {
    vec![
        MenuRow { label: crate::lang::tr_ui("Kill").into_owned(), detail: String::new(), why: None, pick: MenuPick::Finish { knock_out: false } },
        MenuRow { label: crate::lang::tr_ui("Knock out").into_owned(), detail: crate::lang::tr_ui("unconscious, alive").into_owned(), why: None, pick: MenuPick::Finish { knock_out: true } },
    ]
}

#[derive(Clone, Debug)]
pub(in crate::runtime) struct MenuRow {
    pub label: String,
    /// Shown dimmer after the label (dice, range, uses left).
    pub detail: String,
    /// Why it can't be picked now (greyed).
    pub why: Option<&'static str>,
    pub pick: MenuPick,
}

/// An open choice window.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct FightMenu {
    pub kind: MenuKind,
    pub rows: Vec<MenuRow>,
    pub sel: usize,
}

const ROW_H: f32 = 24.0;
const FONT: f32 = 18.0;

impl MenuKind {
    fn title(self) -> &'static str {
        match self {
            MenuKind::Cast => "Cast a spell",
            MenuKind::Class => "Class action",
            MenuKind::Attack => "Attack",
            MenuKind::Finish => "The foe falls: kill or knock out?",
            MenuKind::Ready => "Ready",
            MenuKind::Reaction => "Reaction",
        }
    }

    pub(super) fn id(self) -> &'static str {
        match self {
            MenuKind::Cast => "cast",
            MenuKind::Class => "class",
            MenuKind::Attack => "attack",
            MenuKind::Finish => "finish",
            MenuKind::Ready => "ready",
            MenuKind::Reaction => "reaction",
        }
    }
}

impl Game {
    /// The rows of the Cast window for `who`: every spell and slot level it can cast now, then
    /// the spells it knows or prepared but can't cast now, greyed with the reason.
    pub(in crate::runtime) fn cast_rows(&self, who: Who) -> Vec<MenuRow> {
        let options = self.cast_options(who);
        let mut rows: Vec<MenuRow> = options
            .iter()
            .map(|(id, slot)| {
                let name = self.project.db.spell(id).map_or(id.clone(), |d| crate::lang::tr(&d.name));
                let slot_text = if *slot == 0 { crate::lang::tr_ui("cantrip").into_owned() } else { format!("{} {slot}", crate::lang::tr_ui("slot")) };
                MenuRow { label: name, detail: format!("{slot_text}  {}", self.spell_detail(id)), why: None, pick: MenuPick::Cast(id.clone(), *slot) }
            })
            .collect();
        let mut others: Vec<String> = self.known_of(who);
        for s in self.prepared_of(who) {
            if !others.contains(&s) {
                others.push(s);
            }
        }
        for id in others.into_iter().filter(|s| !options.iter().any(|o| o.0 == *s)) {
            let Some(d) = self.project.db.spell(&id) else { continue };
            if d.story_only {
                continue;
            }
            rows.push(MenuRow {
                label: crate::lang::tr(&d.name),
                detail: if d.level == 0 { crate::lang::tr_ui("cantrip").into_owned() } else { format!("{} {}", crate::lang::tr_ui("level"), d.level) },
                why: Some(self.cast_refusal(who, &id, "Not prepared")),
                pick: MenuPick::Cast(id.clone(), d.level),
            });
        }
        rows.extend(self.item_cast_rows(who, None));
        rows
    }

    /// The Cast window's rows of spells from items (`only`: of that item): "Magic Missile
    /// (Wand of Magic Missiles)  slot 2  2 charges".
    pub(in crate::runtime) fn item_cast_rows(&self, who: Who, only: Option<&str>) -> Vec<MenuRow> {
        self.item_casts(who)
            .into_iter()
            .filter(|(c, _)| only.is_none_or(|o| o == c.item))
            .map(|(c, why)| {
                let spell = self.project.db.spell(&c.spell.spell).map_or(c.spell.spell.clone(), |d| crate::lang::tr(&d.name));
                let item = self.project.db.item(&c.item).map_or(c.item.clone(), |d| crate::lang::tr(&d.name));
                let charges = match c.charges {
                    0 => String::new(),
                    1 => format!("  1 {}", crate::lang::tr_ui("charge")),
                    n => format!("  {n} {}", crate::lang::tr_ui("charges")),
                };
                let slot = if c.slot == 0 { crate::lang::tr_ui("cantrip").into_owned() } else { format!("{} {}", crate::lang::tr_ui("slot"), c.slot) };
                MenuRow { label: format!("{spell} ({item})"), detail: format!("{slot}{charges}"), why, pick: MenuPick::ItemCast { item: c.item.clone(), spell: c.spell.spell.clone(), slot: c.slot } }
            })
            .collect()
    }

    /// A spell's range and what it rolls, short ("120 ft  2d10").
    fn spell_detail(&self, id: &str) -> String {
        let Some(d) = self.project.db.spell(id) else { return String::new() };
        let mut s = match d.range {
            crate::skills::SpellRange::Feet(f) => format!("{f} ft"),
            r => crate::lang::tr_ui(&r.label()).into_owned(),
        };
        if let Some(a) = d.area {
            s += &format!("  {}", a.label());
        }
        s
    }

    /// The rows of the Class window: every class action, greyed with the reason it can't be
    /// used now.
    pub(super) fn class_rows(&self, who: Who) -> Vec<MenuRow> {
        self.class_acts(who)
            .into_iter()
            .map(|a| {
                let mut detail = crate::lang::tr_ui(a.cost.label()).into_owned();
                if let Some((left, max)) = (!a.resource.is_empty()).then(|| self.uses(who, &a.resource)).flatten() {
                    detail += &format!("  {left}/{max}");
                }
                MenuRow { label: crate::lang::tr(&a.name), detail, why: self.class_act_ready(who, &a).err(), pick: MenuPick::Class(a.id.clone()) }
            })
            .collect()
    }

    /// The rows of the Attack window when `who` can also throw its weapon: strike or throw.
    /// Empty when there is nothing to choose (the attack is aimed at once).
    pub(super) fn attack_rows(&self, who: Who) -> Vec<MenuRow> {
        let specs = self.attack_specs(who);
        let Some(throw) = specs.iter().find(|s| s.thrown) else { return Vec::new() };
        let what = self.thrown_item_name(who, throw);
        let mut rows = Vec::new();
        if let Some(s) = specs.iter().find(|s| !s.ranged) {
            rows.push(MenuRow { label: format!("{}: {what}", crate::lang::tr_ui("Strike")), detail: format!("{}  {} ft", s.dice, s.reach.max(1) * 5), why: None, pick: MenuPick::Attack { thrown: false } });
        }
        rows.push(MenuRow { label: format!("{}: {what}", crate::lang::tr_ui("Throw")), detail: format!("{}  {}/{} ft", throw.dice, throw.range * 5, throw.long * 5), why: None, pick: MenuPick::Attack { thrown: true } });
        rows
    }

    /// The rows of the Ready window: the attack (when a foe comes within reach), and each spell
    /// with a creature as its target it can cast now (when a foe comes within its range).
    pub(super) fn ready_rows(&self, who: Who) -> Vec<MenuRow> {
        let mut rows = Vec::new();
        if let Some(s) = self.melee_spec(who) {
            let ft = s.reach.max(1) * 5;
            rows.push(MenuRow { label: crate::lang::tr_ui("Attack").into_owned(), detail: crate::lang::tr_ui("when a foe comes within {} ft").replacen("{}", &ft.to_string(), 1), why: None, pick: MenuPick::ReadyAttack });
        }
        for (id, slot) in self.cast_options(who) {
            let Some(d) = self.project.db.spell(&id) else { continue };
            let aimed = d.area.is_none() && d.range != crate::skills::SpellRange::Caster && d.parts.iter().any(|p| matches!(p, crate::skills::SpellPart::Attack { .. } | crate::skills::SpellPart::Save { .. } | crate::skills::SpellPart::Damage { .. } | crate::skills::SpellPart::Condition { .. }));
            if !aimed || d.casting != crate::skills::SpellCast::Action {
                continue;
            }
            let label = if slot == 0 { crate::lang::tr(&d.name) } else { format!("{} ({} {slot})", crate::lang::tr(&d.name), crate::lang::tr_ui("slot")) };
            let ft = d.range.tiles().max(1) * 5;
            rows.push(MenuRow { label, detail: crate::lang::tr_ui("when a foe comes within {} ft").replacen("{}", &ft.to_string(), 1), why: None, pick: MenuPick::ReadySpell(id.clone(), slot) });
        }
        rows
    }

    /// Ready › Attack: the attack waits for a foe to come within reach (the reaction).
    pub(super) fn ready_attack(&mut self) {
        let Some(c) = &mut self.fight else { return };
        let who = c.current().who;
        c.turn.action = true;
        if let Some(f) = c.fighter_mut(who) {
            f.readied = true;
        }
        let name = self.who_name(who);
        self.fight_log(format!("{name}: {}", crate::lang::tr_ui("readies an attack")));
    }

    /// Ready › a spell: its slot is spent now and it is held with concentration until a foe
    /// comes within its range (or the next turn: then it is lost).
    pub(super) fn ready_spell(&mut self, id: &str, slot: u32) -> Result<(), &'static str> {
        let who = self.fight.as_ref().ok_or("No fight")?.current().who;
        if !self.cast_options(who).iter().any(|(s, l)| s == id && *l == slot) {
            return Err("You can't cast that spell");
        }
        if slot >= 1 {
            self.spend_slot(who, slot);
        }
        // (Holding it takes concentration: one it had ends.)
        self.begin_concentration(who, &format!("ready:{id}"), 60.0);
        if let Some(c) = &mut self.fight {
            c.turn.action = true;
            if let Some(f) = c.fighter_mut(who) {
                f.readied_spell = Some((id.to_string(), slot));
                f.hidden = false;
            }
        }
        let name = self.who_name(who);
        let spell = self.project.db.spell(id).map_or(id.to_string(), |d| crate::lang::tr(&d.name));
        self.fight_log(format!("{name}: {} {spell}", crate::lang::tr_ui("readies")));
        Ok(())
    }

    /// Whether a fight's choice window is open (Esc closes it instead of pausing).
    pub(in crate::runtime) fn fight_menu_open(&self) -> bool {
        self.fight.as_ref().is_some_and(|c| c.menu.is_some())
    }

    /// Opens the choice window (the first row that can be picked is selected). False when it
    /// has no rows.
    pub(in crate::runtime) fn open_fight_menu(&mut self, kind: MenuKind, rows: Vec<MenuRow>) -> bool {
        if rows.is_empty() {
            return false;
        }
        let sel = rows.iter().position(|r| r.why.is_none()).unwrap_or(0);
        if let Some(c) = &mut self.fight {
            c.menu = Some(FightMenu { kind, rows, sel });
        }
        true
    }

    /// Picks row `k` of the open window: aims the spell or class action, or the attack with the
    /// chosen way (strike / throw). A greyed row says why and keeps the window open.
    pub(super) fn pick_fight_menu_row(&mut self, k: usize) -> Result<(), &'static str> {
        let row = self.fight.as_ref().and_then(|c| c.menu.as_ref()).and_then(|m| m.rows.get(k)).cloned().ok_or("No such row")?;
        if let Some(why) = row.why {
            if let Some(m) = self.fight.as_mut().and_then(|c| c.menu.as_mut()) {
                m.sel = k;
            }
            return Err(why);
        }
        if let Some(c) = &mut self.fight {
            c.menu = None;
        }
        match row.pick {
            MenuPick::Cast(id, slot) => self.aim_spell(&id, slot),
            MenuPick::Class(id) => self.set_aim(Aim::Feature(id)),
            MenuPick::Attack { thrown } => {
                self.set_aim(Aim::Attack);
                if let Some(c) = &mut self.fight {
                    c.attack_mode = Some(thrown);
                }
            }
            MenuPick::ItemCast { item, spell, slot } => {
                let who = self.fight.as_ref().map_or(Who::Player, |c| c.current().who);
                let cast = self.item_casts(who).into_iter().map(|c| c.0).find(|c| c.item == item && c.spell.spell == spell && c.slot == slot).ok_or("No charges left")?;
                if let Some(c) = &mut self.fight {
                    c.item_cast = Some(cast);
                }
                self.aim_spell(&spell, slot);
            }
            MenuPick::Finish { knock_out } => self.finish_blow(knock_out),
            MenuPick::ReadyAttack => self.ready_attack(),
            MenuPick::ReadySpell(id, slot) => self.ready_spell(&id, slot)?,
            MenuPick::React(r) => self.resume_reaction(r),
        }
        Ok(())
    }

    /// The window's input (it takes all of the turn's input while open). True while open.
    pub(super) fn fight_menu_input(&mut self) -> bool {
        let Some(menu) = self.fight.as_ref().and_then(|c| c.menu.clone()) else { return false };
        let n = menu.rows.len();
        let mut sel = menu.sel;
        if input::act_pressed(Act::Up) {
            sel = (sel + n - 1) % n;
        }
        if input::act_pressed(Act::Down) {
            sel = (sel + 1) % n;
        }
        let (_, rects) = self.fight_menu_layout(&menu);
        let mouse = input::mouse_pos();
        let hovered = rects.iter().position(|r| r.contains(mouse));
        let moved = self.fight.as_ref().is_some_and(|c| c.mouse_last != mouse);
        if let (Some(h), true) = (hovered, input::mouse_pressed(MouseButton::Left) || moved) {
            sel = h;
        }
        if let Some(c) = &mut self.fight {
            c.mouse_last = mouse;
            if let Some(m) = &mut c.menu {
                m.sel = sel;
            }
        }
        let pick = input::act_pressed(Act::Confirm) || input::act_pressed(Act::Attack) || (input::mouse_pressed(MouseButton::Left) && hovered.is_some());
        if pick {
            if let Err(why) = self.pick_fight_menu_row(sel) {
                self.ui.toast(crate::lang::tr_ui(why).into_owned());
            }
        } else if menu.kind != MenuKind::Finish && (input::act_pressed(Act::Cancel) || input::mouse_pressed(MouseButton::Right) || (input::mouse_pressed(MouseButton::Left) && hovered.is_none())) {
            if let Some(c) = &mut self.fight {
                c.menu = None;
            }
            // (Closing the Reaction window = no reaction.)
            if menu.kind == MenuKind::Reaction {
                self.resume_reaction(super::reactions::ReactChoice::Pass);
            }
        }
        true
    }

    /// The window's panel and its rows' rects (above the action bar, centred).
    fn fight_menu_layout(&self, menu: &FightMenu) -> (Rect, Vec<Rect>) {
        let _font = text::role(Role::Hud);
        let width = menu.rows.iter().map(|r| measure_plain(&format!("{}    {}", r.label, row_tail(r)), FONT).width).fold(measure_plain(&crate::lang::tr_ui(menu.kind.title()), FONT).width, f32::max) + 40.0;
        let w = width.min(screen_width() - 32.0);
        let h = 34.0 + menu.rows.len() as f32 * ROW_H + 10.0;
        let bottom = self.bar_layout().first().map_or(screen_height() - 40.0, |r| r.y) - 34.0;
        let panel = Rect::new((screen_width() - w) / 2.0, (bottom - h).max(60.0), w, h);
        let rows = (0..menu.rows.len()).map(|k| Rect::new(panel.x + 8.0, panel.y + 34.0 + k as f32 * ROW_H, panel.w - 16.0, ROW_H)).collect();
        (panel, rows)
    }

    /// Draws the open window (in the HUD).
    pub(super) fn draw_fight_menu(&self) {
        let Some(menu) = self.fight.as_ref().and_then(|c| c.menu.as_ref()) else { return };
        let _font = text::role(Role::Hud);
        let (panel, rects) = self.fight_menu_layout(menu);
        skin::panel(panel.x, panel.y, panel.w, panel.h);
        draw_plain(&crate::lang::tr_ui(menu.kind.title()), panel.x + 14.0, panel.y + 24.0, FONT, skin::highlight());
        for (k, (row, r)) in menu.rows.iter().zip(&rects).enumerate() {
            if k == menu.sel {
                skin::cursor(*r, Color::new(1.0, 0.86, 0.45, 1.0));
            }
            let color = if row.why.is_some() { skin::disabled() } else { skin::normal() };
            draw_plain(&row.label, r.x + 8.0, r.y + 17.0, FONT, color);
            let tail = row_tail(row);
            let tw = measure_plain(&tail, FONT - 2.0).width;
            draw_plain(&tail, r.right() - tw - 8.0, r.y + 17.0, FONT - 2.0, if row.why.is_some() { skin::disabled() } else { skin::dim() });
        }
    }

    /// `turn menu`: the open window's rows (`>` = selected, `!why` = greyed).
    pub(super) fn fight_menu_status(&self) -> String {
        let Some(menu) = self.fight.as_ref().and_then(|c| c.menu.as_ref()) else { return "no menu".into() };
        let rows: Vec<String> = menu
            .rows
            .iter()
            .enumerate()
            .map(|(k, r)| format!("{}{k}:{}{}", if k == menu.sel { ">" } else { "" }, r.label, r.why.map_or(String::new(), |w| format!("!{w}"))))
            .collect();
        format!("{}[{}]", menu.kind.id(), rows.join(" | "))
    }
}

/// A row's right-hand text: its detail, or why it can't be picked.
fn row_tail(r: &MenuRow) -> String {
    match r.why {
        Some(w) => crate::lang::tr_ui(w).into_owned(),
        None => r.detail.clone(),
    }
}
