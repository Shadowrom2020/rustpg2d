//! The level-up window: when the hero or a party member with a class reaches a level that asks
//! for choices (`WorldState::levelups`), it opens between fights and dialogs: the new features
//! with their texts, and a row per choice: the subclass, an ability score improvement (+2 / +1
//! +1, or the feat), a fighting style, expertise, skills. *Done* stores the picks (`HeroClass`).

use macroquad::prelude::*;

use super::input::{self, Act};
use super::skin;
use super::text::{draw_plain, draw_text};
use super::Game;
use crate::classes::{ClassDef, FeatureChoice, HeroClass};
use crate::srd::Ability;

/// The open window: the selected row and every choice row's picked option (index into its options).
#[derive(Clone, Debug, Default)]
pub(super) struct LevelUpWin {
    sel: usize,
    picks: Vec<usize>,
}

/// A row of the window: which feature it belongs to, and the options it cycles through (ids and
/// what they show).
#[derive(Clone, Debug)]
struct ChoiceRow {
    feature: String,
    label: String,
    options: Vec<(String, String)>,
}

const W: f32 = 720.0;

/// The window: as tall as its rows need.
fn levelup_panel(rows: usize) -> Rect {
    let h = (330.0 + rows as f32 * 38.0).min(screen_height() - 40.0);
    Rect::new((screen_width() - W) / 2.0, (screen_height() - h) / 2.0, W, h)
}

impl Game {
    /// The class, its current choices and level of whom a pending level-up is for.
    fn levelup_subject(&self, who: &str) -> Option<(ClassDef, HeroClass, String, crate::srd::Abilities, Vec<String>)> {
        if who == "player" {
            let hc = self.world.class.clone()?;
            let c = self.project.db.class(&hc.class)?.clone();
            let profs = self.world.proficiencies.iter().cloned().collect();
            return Some((c, hc, self.world.player_name.clone(), self.world.abilities, profs));
        }
        let uid = self.member_uid(who)?;
        let (hc, _) = self.member_class(uid)?;
        let c = self.project.db.class(&hc.class)?.clone();
        let name = self.entity_display_name(uid).unwrap_or_default();
        let block = self.member_block(uid)?;
        Some((c, hc, name, block.abilities, block.proficiencies))
    }

    /// The choice rows of a pending level-up.
    fn levelup_rows(&self, who: &str, c: &ClassDef, hc: &HeroClass, level: u32, profs: &[String]) -> Vec<ChoiceRow> {
        let mut rows = Vec::new();
        for (feature, choice) in Self::level_choices(c, level) {
            match choice {
                FeatureChoice::Options { choose, options } => {
                    if feature == "subclass" && !hc.subclass.is_empty() {
                        continue;
                    }
                    let opts: Vec<(String, String)> = options
                        .iter()
                        .map(|o| {
                            let name = c.subclass(o).map(|s| s.name.clone()).or_else(|| c.feature(o).map(|f| f.name.clone())).unwrap_or_else(|| o.clone());
                            (o.clone(), name)
                        })
                        .collect();
                    let label = if feature == "subclass" { c.subclass_title.clone() } else { c.feature(&feature).map_or(feature.clone(), |f| f.name.clone()) };
                    for _ in 0..choose.max(1) {
                        rows.push(ChoiceRow { feature: feature.clone(), label: label.clone(), options: opts.clone() });
                    }
                }
                FeatureChoice::Expertise { count } => {
                    let mut opts: Vec<(String, String)> = crate::srd::abilities::SKILLS.iter().filter(|(id, _, _)| profs.iter().any(|p| p == id)).map(|(id, name, _)| (id.to_string(), name.to_string())).collect();
                    if profs.iter().any(|p| p == "thieves-tools") {
                        opts.push(("thieves-tools".into(), "Thieves' Tools".into()));
                    }
                    for _ in 0..count {
                        rows.push(ChoiceRow { feature: feature.clone(), label: "Expertise".into(), options: opts.clone() });
                    }
                }
                FeatureChoice::Skills { count } => {
                    let opts: Vec<(String, String)> = crate::srd::abilities::SKILLS.iter().filter(|(id, _, _)| !profs.iter().any(|p| p == id)).map(|(id, name, _)| (id.to_string(), name.to_string())).collect();
                    for _ in 0..count {
                        rows.push(ChoiceRow { feature: feature.clone(), label: "Skill".into(), options: opts.clone() });
                    }
                }
                FeatureChoice::AbilityScores => {
                    let plus = |a: &Ability| (format!("ability:{}", a.id()), format!("+1 {}", crate::lang::tr_ui(a.name())));
                    let mut first: Vec<(String, String)> = Ability::ALL.iter().map(plus).collect();
                    first.push(("feat:grappler".into(), crate::lang::tr_ui("Feat: Grappler").into_owned()));
                    rows.push(ChoiceRow { feature: feature.clone(), label: "Ability score".into(), options: first });
                    rows.push(ChoiceRow { feature: feature.clone(), label: "Ability score".into(), options: Ability::ALL.iter().map(plus).collect() });
                }
            }
        }
        // New cantrips and spells from the class's list (known casters, a wizard's spellbook).
        let (cantrips, spells) = Self::spell_gains(c, level);
        let known = self.subject_who(who).map_or(Vec::new(), |w| self.known_of(w));
        for (n, feature, label, cantrip) in [(cantrips, "cantrips", "Cantrip", true), (spells, "spells", "Spell", false)] {
            let opts = self.spell_options(c, level, cantrip, &known);
            if opts.is_empty() {
                continue;
            }
            for _ in 0..n {
                rows.push(ChoiceRow { feature: feature.into(), label: label.into(), options: opts.clone() });
            }
        }
        rows
    }

    /// The combatant a pending level-up's key stands for (`player`, a member's `map:uid`).
    fn subject_who(&self, who: &str) -> Option<super::turns::Who> {
        if who == "player" {
            Some(super::turns::Who::Player)
        } else {
            self.member_uid(who).map(super::turns::Who::Npc)
        }
    }

    /// Opens the window for the next pending level-up when nothing else is going on; handles it
    /// while open. Returns true while it is open (it pauses the world).
    pub(super) fn update_levelup(&mut self) -> bool {
        if self.levelup.is_none() {
            let calm = self.fight.is_none() && !self.runner.busy() && !self.ui.dialog_open() && self.game_over.is_none() && !self.on_title() && self.creator.is_none();
            if !calm || self.world.levelups.is_empty() {
                return false;
            }
            // Drop entries nothing can be chosen for any more (a class removed, a member who left
            // the party); a member elsewhere (told to stay on another map) waits for its turn.
            let gone = |g: &Game, who: &str| {
                let member = who != "player" && !g.world.companions.iter().any(|m| format!("{}:{}", m.map, m.uid) == who);
                member || (who == "player" && g.hero_class().is_none())
            };
            let keep: Vec<bool> = self.world.levelups.iter().map(|p| !gone(self, &p.who)).collect();
            let mut keep = keep.into_iter();
            self.world.levelups.retain(|_| keep.next().unwrap_or(true));
            let Some(k) = self.world.levelups.iter().position(|p| self.levelup_subject(&p.who).is_some()) else { return false };
            // (The one that can be chosen now goes first: Done removes the first.)
            let p = self.world.levelups.remove(k);
            self.world.levelups.insert(0, p);
            self.levelup = Some(LevelUpWin::default());
        }
        let Some(p) = self.world.levelups.first().cloned() else {
            self.levelup = None;
            return false;
        };
        let Some((c, hc, _, _, profs)) = self.levelup_subject(&p.who) else {
            self.levelup = None;
            return false;
        };
        let rows = self.levelup_rows(&p.who, &c, &hc, p.level, &profs);
        let n = rows.len() + 1;
        let w = self.levelup.as_mut().unwrap();
        // (A new window: rows of the same choice start on different options.)
        if w.picks.is_empty() {
            w.picks = rows.iter().enumerate().map(|(k, r)| rows[..k].iter().filter(|o| o.feature == r.feature).count() % r.options.len().max(1)).collect();
        }
        w.picks.resize(rows.len(), 0);
        if input::nav_pressed(Act::Down) {
            w.sel = (w.sel + 1) % n;
        }
        if input::nav_pressed(Act::Up) {
            w.sel = (w.sel + n - 1) % n;
        }
        let step = if input::nav_pressed(Act::Right) { 1 } else if input::nav_pressed(Act::Left) { -1 } else { 0 };
        if step != 0 && w.sel < rows.len() {
            let k = w.sel;
            let len = rows[k].options.len().max(1) as i32;
            w.picks[k] = (w.picks[k] as i32 + step).rem_euclid(len) as usize;
        }
        // Mouse: a click on a row's arrows, on Done.
        let m = input::mouse_pos();
        if input::mouse_pressed(MouseButton::Left) {
            let (rects, done) = levelup_layout(rows.len());
            for (k, r) in rects.iter().enumerate() {
                if r.contains(m) {
                    let len = rows[k].options.len().max(1) as i32;
                    let s = if m.x < r.x + r.w / 2.0 { -1 } else { 1 };
                    w.sel = k;
                    w.picks[k] = (w.picks[k] as i32 + s).rem_euclid(len) as usize;
                }
            }
            if done.contains(m) {
                w.sel = rows.len();
                self.levelup_done(&p, &rows);
                return true;
            }
        }
        if input::act_pressed(Act::Confirm) {
            if w.sel < rows.len() {
                w.sel += 1;
            } else {
                self.levelup_done(&p, &rows);
            }
        }
        true
    }

    /// Stores the window's picks for the pending level-up and closes it (the next one opens).
    fn levelup_done(&mut self, p: &crate::state::PendingLevel, rows: &[ChoiceRow]) {
        let picks = self.levelup.as_ref().map(|w| w.picks.clone()).unwrap_or_default();
        let mut chosen: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (row, k) in rows.iter().zip(picks) {
            if let Some((id, _)) = row.options.get(k) {
                chosen.entry(row.feature.clone()).or_default().push(id.clone());
            }
        }
        // A feat takes the whole improvement.
        for v in chosen.values_mut() {
            if v.iter().any(|x| x.starts_with("feat:")) {
                v.retain(|x| x.starts_with("feat:"));
            }
        }
        self.apply_level_picks(&p.who, chosen);
        self.world.levelups.remove(0);
        self.levelup = None;
    }

    /// Applies level-up picks: the subclass, the features' choices, ability increases (the hero's
    /// scores; a party member's are added on its stat block from the picks).
    pub(super) fn apply_level_picks(&mut self, who: &str, mut chosen: std::collections::BTreeMap<String, Vec<String>>) {
        // Cantrips and spells go to what the caster knows.
        let mut spells: Vec<String> = chosen.remove("cantrips").unwrap_or_default();
        spells.extend(chosen.remove("spells").unwrap_or_default());
        spells.sort();
        spells.dedup();
        if let Some(w) = self.subject_who(who) {
            if !spells.is_empty() {
                self.add_known(w, &spells);
            }
        }
        let apply = |hc: &mut HeroClass, abilities: Option<&mut crate::srd::Abilities>| {
            let mut abilities = abilities;
            for (feature, v) in chosen.iter() {
                if feature == "subclass" {
                    hc.subclass = v.first().cloned().unwrap_or_default();
                    continue;
                }
                if let Some(a) = abilities.as_deref_mut() {
                    for x in v {
                        Self::apply_ability_pick(a, x);
                    }
                }
                hc.picks.insert(feature.clone(), v.clone());
            }
        };
        if who == "player" {
            let mut abilities = self.world.abilities;
            if let Some(hc) = self.world.class.as_mut() {
                apply(hc, Some(&mut abilities));
            }
            self.world.abilities = abilities;
            self.refresh_training();
            return;
        }
        let Some(uid) = self.member_uid(who) else { return };
        let Some((hc, level)) = self.member_class(uid) else { return };
        let Some(link) = self.loaded.entities.iter().find(|e| e.uid == uid).and_then(|e| e.companion.as_ref()).map(|l| (l.map.clone(), l.uid)) else { return };
        if let Some(comp) = self.world.companions.iter_mut().find(|c| c.map == link.0 && c.uid == link.1) {
            let mut hc = comp.class.clone().unwrap_or(hc);
            apply(&mut hc, None);
            comp.class = Some(hc);
            if comp.level == 0 {
                comp.level = level;
            }
        }
        self.refresh_member(uid);
    }

    /// Autoplay `levelup FEATURE VALUE[,VALUE] …` (picks for the pending level-up, then it is
    /// done: `levelup subclass champion fighter-fighting-style fighter-fighting-style-archery`);
    /// `levelup` alone takes the first options.
    pub fn levelup_command(&mut self, line: &str) -> String {
        let Some(p) = self.world.levelups.first().cloned() else { return "no level-up".into() };
        let mut chosen: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        let words: Vec<&str> = line.split_whitespace().collect();
        for pair in words.chunks(2) {
            if let [f, v] = pair {
                chosen.insert(f.to_string(), v.split(',').map(str::to_string).collect());
            }
        }
        let Some((c, hc, _, _, profs)) = self.levelup_subject(&p.who) else { return "no class".into() };
        let rows = self.levelup_rows(&p.who, &c, &hc, p.level, &profs);
        // Only this level's choices.
        let mut ignored: Vec<String> = chosen.keys().filter(|f| !rows.iter().any(|r| r.feature == **f)).cloned().collect();
        chosen.retain(|f, _| rows.iter().any(|r| r.feature == *f));
        // Only values the feature's rows offer (no Thief for a barbarian).
        for (f, values) in chosen.iter_mut() {
            let offered = |v: &String| rows.iter().any(|r| r.feature == *f && r.options.iter().any(|o| o.0 == *v));
            ignored.extend(values.iter().filter(|v| !offered(v)).cloned());
            values.retain(offered);
        }
        chosen.retain(|_, v| !v.is_empty());
        if chosen.is_empty() {
            for (k, r) in rows.iter().enumerate() {
                // (Distinct options for rows of the same feature.)
                let used: Vec<String> = chosen.get(&r.feature).cloned().unwrap_or_default();
                if let Some((id, _)) = r.options.iter().skip(k % r.options.len().max(1)).chain(r.options.iter()).find(|o| !used.contains(&o.0)) {
                    chosen.entry(r.feature.clone()).or_default().push(id.clone());
                }
            }
        }
        self.apply_level_picks(&p.who, chosen.clone());
        self.world.levelups.remove(0);
        self.levelup = None;
        format!("{}:{} {:?}{}", p.who, p.level, chosen, if ignored.is_empty() { String::new() } else { format!(" (not offered: {})", ignored.join(", ")) })
    }

    /// `levelup=`: `closed`, or the pending ones (`player:4 3:2`) with the open window's rows.
    pub(super) fn levelup_status(&self) -> String {
        // (Party members by name: `player:4 finn:3`.)
        let name = |who: &str| self.world.companions.iter().find(|m| format!("{}:{}", m.map, m.uid) == who).map_or(who.to_string(), |m| m.name.clone());
        let pending: Vec<String> = self.world.levelups.iter().map(|p| format!("{}:{}", name(&p.who), p.level)).collect();
        if pending.is_empty() {
            return "-".into();
        }
        format!("{}{}", pending.join(" "), if self.levelup.is_some() { "(open)" } else { "" })
    }

    pub(super) fn draw_levelup(&self) {
        let (Some(w), Some(p)) = (&self.levelup, self.world.levelups.first()) else { return };
        let Some((c, hc, name, _, profs)) = self.levelup_subject(&p.who) else { return };
        let _font = super::text::role(super::text::Role::Dialog);
        let rows = self.levelup_rows(&p.who, &c, &hc, p.level, &profs);
        let (rects, done) = levelup_layout(rows.len());
        let panel = levelup_panel(rows.len());
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.5));
        skin::panel(panel.x, panel.y, panel.w, panel.h);
        let title = format!("{name}: {}, {} {}", crate::lang::tr(&c.name), crate::lang::tr_ui("level"), p.level);
        draw_plain(&title, panel.x + 24.0, panel.y + 40.0, 26.0, skin::highlight());
        // The level's features (the class's and the subclass's).
        let mut feats: Vec<String> = c.levels.get(p.level as usize - 1).map(|l| l.features.clone()).unwrap_or_default();
        if let Some(sc) = c.subclass(&hc.subclass) {
            feats.extend(sc.levels.iter().filter(|(l, _)| *l == p.level).flat_map(|(_, f)| f.clone()));
        }
        let names: Vec<String> = feats.iter().filter_map(|f| c.feature(f)).map(|f| crate::lang::tr(&f.name)).collect();
        let mut y = panel.y + 70.0;
        for line in super::text::wrap(&names.join(", "), 18.0, W - 48.0).into_iter().take(2) {
            draw_plain(&line, panel.x + 24.0, y, 18.0, skin::normal());
            y += 22.0;
        }
        // The selected row's feature text.
        let sel_feature = rows.get(w.sel).map(|r| r.feature.clone()).or_else(|| feats.first().cloned()).unwrap_or_default();
        let text = rows
            .get(w.sel)
            .and_then(|r| r.options.get(w.picks.get(w.sel).copied().unwrap_or(0)))
            .and_then(|(id, _)| c.feature(id).map(|f| f.text.clone()).or_else(|| c.subclass(id).map(|s| s.description.clone())).or_else(|| self.project.db.spell(id).map(|d| d.description.clone())))
            .or_else(|| c.feature(&sel_feature).map(|f| f.text.clone()))
            .unwrap_or_default();
        for line in super::text::wrap(&crate::lang::tr(&text), 16.0, W - 48.0).into_iter().take(5) {
            draw_plain(&line, panel.x + 24.0, y, 16.0, skin::dim());
            y += 19.0;
        }
        for (k, (row, r)) in rows.iter().zip(&rects).enumerate() {
            let active = k == w.sel;
            draw_plain(&tr_any(&row.label), panel.x + 24.0, r.y + 24.0, 20.0, if active { skin::highlight() } else { skin::dim() });
            draw_rectangle(r.x, r.y, r.w, r.h, skin::button_fill(active, Color::new(0.12, 0.12, 0.18, 0.9)));
            if active {
                skin::cursor(*r, Color::new(1.0, 0.86, 0.45, 1.0));
            }
            let value = row.options.get(w.picks.get(k).copied().unwrap_or(0)).map_or(String::new(), |o| tr_any(&o.1));
            draw_plain(&format!("<  {value}  >"), r.x + 12.0, r.y + 24.0, 20.0, skin::normal());
        }
        let active = w.sel >= rows.len();
        draw_rectangle(done.x, done.y, done.w, done.h, skin::button_fill(active, Color::new(0.12, 0.12, 0.18, 0.9)));
        if active {
            skin::cursor(done, Color::new(1.0, 0.86, 0.45, 1.0));
        }
        draw_text("Done", done.x + 20.0, done.y + 26.0, 22.0, skin::normal());
        super::glyphs::draw_hint_in("{key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  {key:confirm}: next / done", panel.x + 24.0, panel.bottom() - 10.0, 16.0, W - 48.0, skin::disabled());
    }
}

/// The rows' value boxes and the Done button (screen pixels).
fn levelup_layout(n: usize) -> (Vec<Rect>, Rect) {
    let panel = levelup_panel(n);
    let top = panel.y + 220.0;
    let rows = (0..n).map(|k| Rect::new(panel.x + 260.0, top + k as f32 * 38.0, 420.0, 34.0)).collect();
    let done = Rect::new(panel.x + 260.0, (top + n as f32 * 38.0 + 10.0).min(panel.bottom() - 60.0), 160.0, 38.0);
    (rows, done)
}

/// A text of the window in the player's language: content (feature, subclass names) or a
/// runtime text ("Expertise", skill names).
fn tr_any(s: &str) -> String {
    let t = crate::lang::tr(s);
    if t != s {
        t
    } else {
        crate::lang::tr_ui(s).into_owned()
    }
}
