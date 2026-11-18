//! Character creator, shown after "New game" when the project enables it. Page 1, *who*: the
//! player types a name, picks a race (the playable races and subraces), male / female, a model
//! (the race's figures of that sex; without races the project's default figure for that sex and
//! the other playable models of that sex) and a colour for each colour feature of the model that
//! offers colours (`defs::ColorFeature`; the race's own colours first). Page 2, *abilities* (the
//! SRD 5.1 rules): the ability scores by standard array, point buy or 4d6 rolls, the choices the
//! race offers (ability increases, skills, a tool, a language, an ancestry, a cantrip) and a
//! background with its languages. A project without methods, choices and backgrounds skips it.
//!
//! Keyboard: typing edits the name (Backspace deletes), Up/Down select a row, Left/Right
//! change it, Enter goes on (page 1) / starts (page 2), Esc goes back. Everything can be clicked.

use std::collections::BTreeMap;

use macroquad::prelude::*;

use super::skin;

use super::input::{self, Act, Pad};
use super::text::{draw_text, measure_text};
use super::{ui::panel, Game};
use crate::defs::Appearance;
use crate::races::{ChoiceKind, RaceDef};
use crate::srd::abilities::{self, Ability, AbilityMethod};
use crate::srd::Abilities;
use crate::state::Sex;

const W: f32 = 760.0;
const H: f32 = 560.0;
const MAX_NAME: usize = 16;
const ROW_H: f32 = 54.0;
/// Page 2's rows.
const ROW2_H: f32 = 44.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Name,
    /// Playable races (only when there is more than one).
    Race,
    Sex,
    /// Playable figures of the chosen sex (only when there is more than one).
    Body,
    /// A colour feature of the chosen figure (index into its features) that offers colours.
    Feature(usize),
    /// Page 2: how the scores are set.
    Method,
    /// Page 2: an ability's base score (index into `Ability::ALL`).
    Ability(usize),
    /// Page 2: roll the six scores again (the *Roll* method).
    Reroll,
    /// Page 2: a pick of a racial choice (choice index in the merged race, slot).
    Choice(usize, usize),
    /// Page 2: the background.
    Background,
    /// Page 2: a language the background lets the hero choose (slot).
    BgLanguage(usize),
    Start,
    Back,
}

pub(super) struct Creator {
    name: String,
    /// Chosen race (index into `creator_races`).
    race: usize,
    sex: Sex,
    /// Chosen figure per sex (index into `creator_bodies`).
    body: [usize; 2],
    /// Chosen colour per feature of the figure: 0 = as drawn (or the race's colour), k = the
    /// feature's colour k - 1.
    colors: Vec<usize>,
    row: usize,
    time: f32,
    /// 0 = who, 1 = abilities.
    page: usize,
    /// Index into the project's ability methods.
    method: usize,
    /// Base ability scores (before the race's bonuses).
    base: Abilities,
    /// Racial choice index -> the picked option per slot (indices into its options).
    picks: BTreeMap<usize, Vec<usize>>,
    /// Index into the backgrounds (= their number: none).
    background: usize,
    /// Picked languages of the background (indices into `bg_language_options`).
    bg_langs: Vec<usize>,
}

struct Layout {
    panel: Rect,
    /// Control area of each choice row (rows before Start / Back), in row order.
    rows: Vec<Rect>,
    preview: Rect,
    start: Rect,
    back: Rect,
}

fn page1_layout(n_rows: usize) -> Layout {
    let (sw, sh) = (screen_width(), screen_height());
    let p = Rect::new(((sw - W) / 2.0).round(), ((sh - H) / 2.0).round(), W, H);
    let fx = p.x + 170.0;
    // Rows close up a little when there are many (a race row on top of all the looks).
    let row_h = ROW_H.min((H - 76.0 - 84.0) / n_rows.max(1) as f32).floor();
    Layout {
        panel: p,
        rows: (0..n_rows).map(|k| Rect::new(fx, p.y + 76.0 + k as f32 * row_h, 280.0, 38.0)).collect(),
        preview: Rect::new(p.x + p.w - 270.0, p.y + 76.0, 240.0, 300.0),
        start: Rect::new(p.x + 30.0, p.y + H - 62.0, 220.0, 42.0),
        back: Rect::new(p.x + 266.0, p.y + H - 62.0, 120.0, 42.0),
    }
}

/// Page 2: the ability rows in the left column (labels left of them), the race's choices and
/// the background in the right column (labels inside).
fn page2_layout(rows: &[Row]) -> Layout {
    let mut l = page1_layout(0);
    let p = l.panel;
    let (mut left, mut right) = (0.0, 0.0);
    l.rows = rows
        .iter()
        .map(|r| {
            if matches!(r, Row::Method | Row::Ability(_) | Row::Reroll) {
                let y = p.y + 76.0 + left * ROW2_H;
                left += 1.0;
                Rect::new(p.x + 100.0, y, 272.0, 36.0)
            } else {
                let y = p.y + 76.0 + right * ROW2_H;
                right += 1.0;
                Rect::new(p.x + 378.0, y, 360.0, 36.0)
            }
        })
        .collect();
    l
}

/// The "<" and ">" buttons of a choice row.
fn arrows(r: Rect) -> (Rect, Rect) {
    (Rect::new(r.x, r.y, 34.0, r.h), Rect::new(r.x + r.w - 34.0, r.y, 34.0, r.h))
}

fn button(r: Rect, label: &str, selected: bool, active: bool, hover: bool) {
    if !skin::button_picture(r, active || hover || selected) {
        let fill = if active {
            skin::button_fill(true, Color::new(0.45, 0.35, 0.12, 1.0))
        } else if hover {
            skin::button_fill(true, Color::new(0.3, 0.26, 0.14, 1.0))
        } else {
            skin::button_fill(false, Color::new(0.12, 0.12, 0.2, 0.9))
        };
        draw_rectangle(r.x, r.y, r.w, r.h, fill);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 } else { 1.5 }, if selected { skin::highlight() } else { skin::line(0.8) });
    } else if selected {
        skin::cursor_picture(r);
    }
    let tw = measure_text(label, None, 24, 1.0).width;
    draw_text(label, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + 8.0, 24.0, skin::normal());
}

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

/// Draws `text` wrapped into `w` from `y` on (at most `max_lines`, 0 = nothing); returns the y
/// below it.
fn wrapped(text: &str, x: f32, mut y: f32, w: f32, size: f32, color: Color, max_lines: usize) -> f32 {
    if max_lines == 0 {
        return y;
    }
    let mut line = String::new();
    let mut lines = 0;
    for word in text.split_whitespace() {
        let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if measure_text(&next, None, size as u16, 1.0).width > w && !line.is_empty() {
            if lines + 1 >= max_lines {
                draw_text(format!("{line}..."), x, y, size, color);
                return y + size + 2.0;
            }
            draw_text(&line, x, y, size, color);
            y += size + 2.0;
            lines += 1;
            line = word.to_string();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        draw_text(&line, x, y, size, color);
        y += size + 2.0;
    }
    y
}

impl Game {
    /// Name used when the creator is skipped (play-tests, old saves).
    pub(super) fn default_player_name(&self) -> String {
        let c = &self.project.config;
        let name = if c.creation.default_name.trim().is_empty() {
            self.project.db.character(&c.player).map(|d| d.name.clone()).unwrap_or_default()
        } else {
            c.creation.default_name.clone()
        };
        if name.trim().is_empty() { "Hero".into() } else { name }
    }

    /// Races the player may choose (playable ones and subraces; none = the project has no race
    /// choice).
    fn creator_races(&self) -> Vec<&RaceDef> {
        let races = &self.project.db.races;
        let pos = |id: &str| races.iter().position(|r| r.id == id).unwrap_or(usize::MAX);
        let mut v: Vec<(usize, usize, &RaceDef)> = races.iter().enumerate().filter(|(_, r)| r.playable).map(|(k, r)| (if r.parent.is_empty() { k } else { pos(&r.parent).min(k) }, k, r)).collect();
        // A subrace stands where its race stands (the SRD's order: hill dwarf where the dwarf is).
        v.sort_by_key(|(at, k, _)| (*at, *k));
        v.into_iter().map(|(_, _, r)| r).collect()
    }

    /// The chosen race with its parent race folded in (None without playable races).
    fn creator_race(&self, c: &Creator) -> Option<RaceDef> {
        let races = self.creator_races();
        (!races.is_empty()).then(|| races[c.race % races.len()]).and_then(|r| self.project.db.race_full(&r.id))
    }

    /// Whether `id` is a look the creator can show: a loaded 3D model.
    fn is_look(&self, id: &str) -> bool {
        self.assets.models.has(id)
    }

    /// Models a player of `sex` may choose: the race's (that exist), else the project's default
    /// for that sex first, then the playable models of that sex.
    fn creator_bodies(&self, race: Option<&RaceDef>, sex: Sex) -> Vec<String> {
        if let Some(r) = race {
            let v: Vec<String> = r.bodies(sex).iter().filter(|b| self.is_look(b)).cloned().collect();
            if !v.is_empty() {
                return v;
            }
        }
        let cc = &self.project.config.creation;
        let mut v: Vec<String> = Vec::new();
        let default = cc.body(sex);
        if !default.is_empty() {
            v.push(default.to_string());
        }
        let female = sex == Sex::Female;
        let models = self.project.db.models.iter().filter(|m| m.playable && m.female == female).map(|m| &m.id);
        for id in models {
            if !v.contains(id) {
                v.push(id.clone());
            }
        }
        v.retain(|b| self.is_look(b));
        v
    }

    /// The chosen figure (`""` when none can be chosen: the player character's own).
    fn creator_body(&self, c: &Creator) -> String {
        let bodies = self.creator_bodies(self.creator_race(c).as_ref(), c.sex);
        if bodies.is_empty() {
            self.project.db.character(&self.project.config.player).map(|d| self.look(&d.appearance).model).unwrap_or_default()
        } else {
            bodies[c.body[c.sex as usize] % bodies.len()].clone()
        }
    }

    /// The chosen figure's colour features that offer colours: (feature index, name, colours,
    /// label).
    fn creator_features(&self, c: &Creator) -> Vec<(usize, String, Vec<[u8; 3]>, String)> {
        let body = self.creator_body(c);
        let features = match self.project.db.model(&body) {
            Some(m) => &m.features,
            _ => return Vec::new(),
        };
        features.iter().enumerate().filter(|(_, f)| !f.colors.is_empty()).map(|(k, f)| (k, f.name.clone(), f.colors.clone(), f.label())).collect()
    }

    /// The project's ability methods.
    fn creator_methods(&self) -> Vec<AbilityMethod> {
        self.project.config.creation.ability_methods.clone()
    }

    /// Base scores a method starts with.
    fn method_start(m: AbilityMethod) -> Abilities {
        match m {
            AbilityMethod::Standard => Abilities::STANDARD,
            AbilityMethod::PointBuy => Abilities([8; 6]),
            AbilityMethod::Roll => Abilities(std::array::from_fn(|_| abilities::roll_score())),
        }
    }

    fn creator_method(&self, c: &Creator) -> Option<AbilityMethod> {
        let m = self.creator_methods();
        (!m.is_empty()).then(|| m[c.method % m.len()])
    }

    /// The languages a background may add: every SRD language the race doesn't speak and the
    /// race's language choices didn't pick.
    fn bg_language_options(c: &Creator, race: Option<&RaceDef>) -> Vec<String> {
        let mut known: Vec<String> = race.map_or(Vec::new(), |r| r.languages.clone());
        if let Some(r) = race {
            for (ci, ch) in r.choices.iter().enumerate().filter(|(_, ch)| ch.kind == ChoiceKind::Language) {
                known.extend(Self::picked(c, ci, ch.choose as usize, ch.options.len()).iter().filter_map(|k| ch.options.get(*k).cloned()));
            }
        }
        crate::backgrounds::LANGUAGES.iter().map(|(id, _)| id.to_string()).filter(|id| !known.contains(id)).collect()
    }

    /// The chosen background (None = none or the project has none).
    fn creator_background(&self, c: &Creator) -> Option<&crate::backgrounds::BackgroundDef> {
        self.project.db.backgrounds.get(c.background)
    }

    /// A choice's picked option indices, defaulting to the first ones (distinct).
    fn picked(c: &Creator, ci: usize, choose: usize, n: usize) -> Vec<usize> {
        let mut v = c.picks.get(&ci).cloned().unwrap_or_default();
        v.truncate(choose);
        while v.len() < choose.min(n) {
            let next = (0..n).find(|k| !v.contains(k)).unwrap_or(0);
            v.push(next);
        }
        v
    }

    fn bg_picked(c: &Creator, choose: usize, n: usize) -> Vec<usize> {
        let mut v = c.bg_langs.clone();
        v.truncate(choose);
        while v.len() < choose.min(n) {
            let next = (0..n).find(|k| !v.contains(k)).unwrap_or(0);
            v.push(next);
        }
        v
    }

    /// The racial choices as the hero gets them: choice id -> picked option ids (with the
    /// background's languages under `races::BACKGROUND_LANGUAGES`).
    fn creator_choices(&self, c: &Creator) -> BTreeMap<String, Vec<String>> {
        let mut out = BTreeMap::new();
        let race = self.creator_race(c);
        if let Some(r) = &race {
            for (ci, ch) in r.choices.iter().enumerate() {
                let picks = Self::picked(c, ci, ch.choose as usize, ch.options.len());
                let entry: &mut Vec<String> = out.entry(ch.id.clone()).or_default();
                entry.extend(picks.iter().filter_map(|k| ch.options.get(*k).cloned()));
            }
        }
        if let Some(b) = self.creator_background(c).filter(|b| b.language_choices > 0) {
            let opts = Self::bg_language_options(c, race.as_ref());
            let picks = Self::bg_picked(c, b.language_choices as usize, opts.len());
            out.insert(crate::races::BACKGROUND_LANGUAGES.to_string(), picks.iter().filter_map(|k| opts.get(*k).cloned()).collect());
        }
        out
    }

    /// The hero the creator's page 2 makes (for the totals it shows).
    fn creator_hero(&self, c: &Creator) -> crate::races::Hero {
        let race = self.creator_race(c);
        let base = self.creator_method(c).map(|_| c.base);
        crate::races::build_hero(race.as_ref(), &self.project.config.rules, base, &self.creator_choices(c), self.creator_background(c))
    }

    /// Whether the creator has a second page: ability methods, racial choices or backgrounds.
    fn creator_has_page2(&self, c: &Creator) -> bool {
        !self.creator_methods().is_empty() || self.creator_race(c).is_some_and(|r| !r.choices.is_empty()) || !self.project.db.backgrounds.is_empty()
    }

    /// The label of a row (feature rows: the feature's name).
    fn creator_label(&self, c: &Creator, row: Row) -> String {
        match row {
            Row::Name => "Name".into(),
            Row::Race => "Race".into(),
            Row::Sex => "Sex".into(),
            Row::Body => "Body".into(),
            Row::Feature(k) => self.creator_features(c).into_iter().find(|f| f.0 == k).map_or(String::new(), |f| crate::lang::tr(&f.3)),
            Row::Method => "Scores".into(),
            Row::Ability(i) => Ability::ALL[i].short().into(),
            Row::Reroll => String::new(),
            Row::Choice(..) | Row::Background | Row::BgLanguage(_) => String::new(),
            Row::Start | Row::Back => String::new(),
        }
    }

    /// The rows of the current page (Race and Body only with a choice; a row per colour feature
    /// that offers colours; on page 2 what the project and the race offer).
    fn creator_rows(&self, c: &Creator) -> Vec<Row> {
        let mut rows = Vec::new();
        if c.page == 0 {
            rows.push(Row::Name);
            if self.creator_races().len() > 1 {
                rows.push(Row::Race);
            }
            rows.push(Row::Sex);
            if self.creator_bodies(self.creator_race(c).as_ref(), c.sex).len() > 1 {
                rows.push(Row::Body);
            }
            rows.extend(self.creator_features(c).into_iter().map(|f| Row::Feature(f.0)));
        } else {
            if let Some(m) = self.creator_method(c) {
                rows.push(Row::Method);
                rows.extend((0..6).map(Row::Ability));
                if m == AbilityMethod::Roll {
                    rows.push(Row::Reroll);
                }
            }
            if let Some(r) = self.creator_race(c) {
                for (ci, ch) in r.choices.iter().enumerate() {
                    rows.extend((0..(ch.choose as usize).min(ch.options.len())).map(|slot| Row::Choice(ci, slot)));
                }
            }
            if !self.project.db.backgrounds.is_empty() {
                rows.push(Row::Background);
                if let Some(b) = self.creator_background(c) {
                    rows.extend((0..b.language_choices as usize).map(Row::BgLanguage));
                }
            }
        }
        rows.extend([Row::Start, Row::Back]);
        rows
    }

    /// The player definition's look with the creator's choices (and the race's colours where a
    /// feature is left as drawn).
    fn chosen_appearance(&self, c: &Creator) -> Appearance {
        let mut a = self.project.db.character(&self.project.config.player).map(|d| d.appearance.clone()).unwrap_or_default();
        let body = self.creator_body(c);
        let current = self.look(&a).model;
        if body != current {
            // Another look: the player character's colours were chosen for its own.
            a.colors.clear();
        }
        a.model = body;
        a.body.clear();
        let race = self.creator_race(c);
        for (k, name, colors, _) in self.creator_features(c) {
            match c.colors.get(k).copied().unwrap_or(0) {
                0 => {
                    if let Some(col) = race.as_ref().and_then(|r| r.colors.get(&name)) {
                        a.colors.insert(name, *col);
                    }
                }
                n => {
                    a.colors.insert(name, colors[(n - 1) % colors.len()]);
                }
            }
        }
        a
    }

    /// Opens the creator (from the title screen's "New game").
    pub(super) fn open_creator(&mut self) {
        while input::next_char().is_some() {} // drop keys typed before
        let sex = self.project.config.creation.sex.fixed().unwrap_or(Sex::Male);
        // The player character's own race comes first when it is playable.
        let own = self.project.db.character(&self.project.config.player).map(|d| d.race.clone()).unwrap_or_default();
        let race = self.creator_races().iter().position(|r| r.id == own).unwrap_or(0);
        let base = self.creator_methods().first().map_or(self.project.config.rules.abilities, |m| Self::method_start(*m));
        self.creator = Some(Creator { name: self.default_player_name(), race, sex, body: [0, 0], colors: Vec::new(), row: 0, time: 0.0, page: 0, method: 0, base, picks: BTreeMap::new(), background: 0, bg_langs: Vec::new() });
    }

    /// Steps the choice of a row.
    fn change_row(&mut self, row: Row, step: i32) {
        let Some(c) = self.creator.as_ref() else { return };
        let race = self.creator_race(c);
        let n = match row {
            Row::Race => self.creator_races().len(),
            Row::Body => self.creator_bodies(race.as_ref(), c.sex).len(),
            // "As drawn" and the feature's colours.
            Row::Feature(k) => self.creator_features(c).into_iter().find(|f| f.0 == k).map_or(0, |f| f.2.len() + 1),
            Row::Method => self.creator_methods().len(),
            Row::Background => self.project.db.backgrounds.len() + 1,
            _ => 0,
        }
        .max(1) as i32;
        let method = self.creator_method(c);
        let methods = self.creator_methods();
        let choice = match row {
            Row::Choice(ci, _) => race.as_ref().and_then(|r| r.choices.get(ci).cloned()),
            _ => None,
        };
        let bg_opts = Self::bg_language_options(c, race.as_ref()).len();
        let bg_choose = self.creator_background(c).map_or(0, |b| b.language_choices as usize);
        let fixed = self.project.config.creation.sex.fixed().is_some();
        let c = self.creator.as_mut().unwrap();
        let cycle = |v: &mut usize| *v = (*v as i32 + step).rem_euclid(n) as usize;
        match row {
            Row::Race => {
                cycle(&mut c.race);
                // Another race has other figures, colours and choices.
                c.body = [0, 0];
                c.colors.clear();
                c.picks.clear();
                c.bg_langs.clear();
            }
            Row::Sex if !fixed => {
                c.sex = if c.sex == Sex::Male { Sex::Female } else { Sex::Male };
                c.colors.clear();
            }
            Row::Body => {
                cycle(&mut c.body[c.sex as usize]);
                c.colors.clear();
            }
            Row::Feature(k) => {
                if c.colors.len() <= k {
                    c.colors.resize(k + 1, 0);
                }
                cycle(&mut c.colors[k]);
            }
            Row::Method => {
                cycle(&mut c.method);
                c.base = Self::method_start(methods[c.method % methods.len()]);
            }
            Row::Ability(i) => match method {
                Some(AbilityMethod::PointBuy) => {
                    let mut b = c.base;
                    b.0[i] = (b.0[i] + step).clamp(8, 15);
                    if abilities::point_buy_spent(&b).is_some_and(|s| s <= abilities::POINT_BUY_BUDGET) {
                        c.base = b;
                    }
                }
                Some(_) => abilities::swap_assigned(&mut c.base, i, step),
                None => {}
            },
            Row::Reroll => c.base = Self::method_start(AbilityMethod::Roll),
            Row::Choice(ci, slot) => {
                let Some(ch) = choice else { return };
                let n = ch.options.len();
                let mut picks = Self::picked(c, ci, ch.choose as usize, n);
                if n > picks.len() {
                    // The next option no other slot of the choice holds.
                    let mut k = picks[slot];
                    loop {
                        k = (k as i32 + step).rem_euclid(n as i32) as usize;
                        if !picks.iter().enumerate().any(|(s, p)| s != slot && *p == k) {
                            break;
                        }
                    }
                    picks[slot] = k;
                }
                c.picks.insert(ci, picks);
                if ch.kind == ChoiceKind::Language {
                    c.bg_langs.clear();
                }
            }
            Row::Background => {
                cycle(&mut c.background);
                c.bg_langs.clear();
            }
            Row::BgLanguage(slot) => {
                let mut picks = Self::bg_picked(c, bg_choose, bg_opts);
                if bg_opts > picks.len() && slot < picks.len() {
                    let mut k = picks[slot];
                    loop {
                        k = (k as i32 + step).rem_euclid(bg_opts as i32) as usize;
                        if !picks.iter().enumerate().any(|(s, p)| s != slot && *p == k) {
                            break;
                        }
                    }
                    picks[slot] = k;
                }
                c.bg_langs = picks;
            }
            _ => {}
        }
    }

    /// Start (page 1: on to page 2 when there is one).
    fn creator_forward(&mut self) {
        let Some(c) = &self.creator else { return };
        if c.page == 0 && self.creator_has_page2(c) {
            let c = self.creator.as_mut().unwrap();
            c.page = 1;
            c.row = 0;
        } else {
            self.start_created();
        }
    }

    /// Back (page 2: to page 1; page 1: to the title screen).
    fn creator_backward(&mut self) {
        match &mut self.creator {
            Some(c) if c.page > 0 => {
                c.page = 0;
                c.row = 0;
            }
            _ => self.creator = None,
        }
    }

    fn start_created(&mut self) {
        let Some(c) = self.creator.take() else { return };
        let appearance = self.chosen_appearance(&c);
        let race = self.creator_race(&c).map(|r| r.id.clone()).unwrap_or_default();
        let base = self.creator_method(&c).map(|_| c.base);
        let choices = self.creator_choices(&c);
        let background = self.creator_background(&c).map(|b| b.id.clone()).unwrap_or_default();
        let name = c.name.trim().to_string();
        self.new_game();
        self.world.player_name = if name.is_empty() { self.default_player_name() } else { name };
        self.world.player_sex = c.sex;
        self.world.player_appearance = Some(appearance);
        self.begin_as_hero(super::races::HeroChoices { race, base, choices, background });
        self.close_title();
    }

    /// The control rectangles of the current page's choice rows (rows before Start / Back).
    fn creator_layout(&self, c: &Creator, rows: &[Row]) -> Layout {
        let choice_rows: Vec<Row> = rows.iter().copied().filter(|r| !matches!(r, Row::Start | Row::Back)).collect();
        if c.page == 0 {
            page1_layout(choice_rows.len())
        } else {
            page2_layout(&choice_rows)
        }
    }

    /// Handles the open creator. Returns false when it isn't open.
    pub(super) fn update_creator(&mut self, dt: f32) -> bool {
        let Some(c) = &mut self.creator else { return false };
        c.time += dt;
        while let Some(ch) = input::next_char() {
            if c.page == 0 && !ch.is_control() && c.name.chars().count() < MAX_NAME {
                c.name.push(ch);
            }
        }
        if c.page == 0 && input::pressed(KeyCode::Backspace) {
            c.name.pop();
        }
        // The name field takes typed text: arrows and Enter / Escape (or the gamepad) navigate.
        if input::act_pressed(Act::Cancel) {
            self.creator_backward();
            return true;
        }
        let Some(c) = &self.creator else { return true };
        let rows = self.creator_rows(c);
        let n = rows.len();
        // A fixed sex can't be changed: its row is skipped.
        let fixed = self.project.config.creation.sex.fixed().is_some();
        let step = |r: usize, d: usize| {
            let k = (r + d) % n;
            if fixed && rows[k] == Row::Sex { (k + d) % n } else { k }
        };
        let mut row = c.row.min(n - 1);
        if input::nav_pressed(Act::Down) {
            row = step(row, 1);
        }
        if input::nav_pressed(Act::Up) {
            row = step(row, n - 1);
        }
        let (left, right) = (input::nav_pressed(Act::Left), input::nav_pressed(Act::Right));
        if let Some(c) = &mut self.creator {
            c.row = row;
        }
        match rows[row] {
            Row::Start | Row::Back if left || right => {
                let other = rows.iter().position(|r| *r == if rows[row] == Row::Start { Row::Back } else { Row::Start }).unwrap_or(row);
                self.creator.as_mut().unwrap().row = other;
            }
            r if left => self.change_row(r, -1),
            r if right => self.change_row(r, 1),
            _ => {}
        }
        if input::pressed(KeyCode::Enter) || input::pad_pressed(Pad::South) {
            match rows.get(row) {
                Some(Row::Back) => self.creator_backward(),
                Some(Row::Reroll) => self.change_row(Row::Reroll, 1),
                _ => self.creator_forward(),
            }
            return true;
        }
        // Mouse.
        if input::mouse_pressed(MouseButton::Left) {
            let Some(c) = &self.creator else { return true };
            let choice_rows: Vec<Row> = rows.iter().copied().filter(|r| !matches!(r, Row::Start | Row::Back)).collect();
            let l = self.creator_layout(c, &rows);
            let m = input::mouse_pos();
            for (k, r) in l.rows.iter().enumerate() {
                if !r.contains(m) {
                    continue;
                }
                let row = choice_rows[k];
                self.creator.as_mut().unwrap().row = rows.iter().position(|x| *x == row).unwrap_or(0);
                match row {
                    Row::Name => {}
                    Row::Sex if !fixed => {
                        let female = m.x > r.x + r.w / 2.0;
                        let c = self.creator.as_mut().unwrap();
                        let sex = if female { Sex::Female } else { Sex::Male };
                        if c.sex != sex {
                            c.sex = sex;
                            c.colors.clear();
                        }
                    }
                    Row::Reroll => self.change_row(Row::Reroll, 1),
                    other => {
                        let (prev, _) = arrows(*r);
                        self.change_row(other, if prev.contains(m) { -1 } else { 1 });
                    }
                }
            }
            if l.start.contains(m) {
                self.creator_forward();
            } else if l.back.contains(m) {
                self.creator_backward();
            }
        }
        true
    }

    /// Creator state for automated play-throughs: `name|race|sex|look rows …|pN …` (page 2: the
    /// method, the base and total scores, the picks and the background).
    pub(super) fn creator_status(&self) -> Option<String> {
        let c = self.creator.as_ref()?;
        let a = self.chosen_appearance(c);
        let rows: Vec<String> = self.creator_rows(c).iter().map(|r| self.creator_label(c, *r)).filter(|l| !l.is_empty()).collect();
        let colors: Vec<String> = a.colors.iter().map(|(k, v)| format!("{k}:{:02x}{:02x}{:02x}", v[0], v[1], v[2])).collect();
        let look = format!("{} colors{{{}}} rows {}", a.body, colors.join(" "), rows.join("/"));
        let race = self.creator_race(c).map_or("-".to_string(), |r| r.id.clone());
        let hero = self.creator_hero(c);
        let picks: Vec<String> = self.creator_choices(c).iter().map(|(k, v)| format!("{k}={}", v.join("+"))).collect();
        Some(format!(
            "{}|{}|{:?}|{}|p{} {} base {} total {} picks[{}] bg:{}",
            c.name,
            race,
            c.sex,
            look,
            c.page + 1,
            self.creator_method(c).map_or("-", |m| m.label()),
            c.base.text(),
            hero.abilities.text(),
            picks.join(";"),
            self.creator_background(c).map_or("-", |b| b.id.as_str())
        ))
    }

    pub(super) fn draw_creator(&self) {
        let Some(c) = &self.creator else { return };
        let rows = self.creator_rows(c);
        let choice_rows: Vec<Row> = rows.iter().copied().filter(|r| !matches!(r, Row::Start | Row::Back)).collect();
        let l = self.creator_layout(c, &rows);
        let m = input::mouse_pos();
        let p = l.panel;
        let sel = rows.get(c.row).copied();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.6));
        panel(p.x, p.y, p.w, p.h);
        let title = if c.page == 0 { "Create your character" } else { "Abilities and background" };
        let tw = measure_text(title, None, 34, 1.0).width;
        draw_text(title, p.x + (p.w - tw) / 2.0, p.y + 46.0, 34.0, skin::highlight());
        if c.page == 0 {
            self.draw_creator_page1(c, &choice_rows, &l, sel, m);
        } else {
            self.draw_creator_page2(c, &choice_rows, &l, sel, m);
        }
        let more = c.page == 0 && self.creator_has_page2(c);
        button(l.start, if more { "Next" } else { "Start adventure" }, sel == Some(Row::Start), false, l.start.contains(m));
        button(l.back, "Back", sel == Some(Row::Back), false, l.back.contains(m));
        let hint = if c.page == 0 {
            "Type a name  -  {key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  Enter: next  -  {key:cancel}: back"
        } else {
            "{key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  Enter: start  -  {key:cancel}: back"
        };
        super::glyphs::draw_hint_in(hint, p.x + 30.0, p.y + H - 6.0, 16.0, p.w - 60.0, skin::disabled());
    }

    /// A row's "<" value ">" control.
    fn draw_choice_control(r: Rect, active: bool, m: Vec2, text: &str, swatch: Option<[u8; 3]>) {
        let (prev, next) = arrows(r);
        for (b, t) in [(prev, "<"), (next, ">")] {
            button(b, t, false, false, b.contains(m));
        }
        let mid = Rect::new(prev.right() + 6.0, r.y, next.x - prev.right() - 12.0, r.h);
        if active {
            draw_rectangle_lines(mid.x, mid.y, mid.w, mid.h, 2.0, skin::highlight());
        }
        let mut x = mid.x + 10.0;
        if let Some(col) = swatch {
            draw_rectangle(x, mid.y + 7.0, 40.0, mid.h - 14.0, rgb(col));
            draw_rectangle_lines(x, mid.y + 7.0, 40.0, mid.h - 14.0, 1.5, skin::line(0.8));
            x += 52.0;
        }
        // A smaller size before anything is cut off.
        let avail = mid.x + mid.w - x - 6.0;
        let size = 22.0;
        let mut t = text.to_string();
        while t.chars().count() > 3 && measure_text(&t, None, size as u16, 1.0).width > avail {
            t.pop();
        }
        draw_text(&t, x, mid.y + mid.h / 2.0 + size / 2.0 - 3.0, size, skin::normal());
    }

    fn draw_creator_page1(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let cc = &self.project.config.creation;
        let race = self.creator_race(c);
        let features = self.creator_features(c);
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            draw_text(self.creator_label(c, *row), p.x + 30.0, r.y + 27.0, 24.0, if active { skin::highlight() } else { skin::dim() });
            match row {
                Row::Name => {
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, if active { 2.5 } else { 1.5 }, if active { skin::highlight() } else { skin::line(0.8) });
                    let caret = if active && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
                    draw_text(format!("{}{caret}", c.name), r.x + 10.0, r.y + 27.0, 26.0, skin::normal());
                }
                Row::Sex if cc.sex.fixed().is_some() => {
                    draw_text(c.sex.label(), r.x + 6.0, r.y + 26.0, 24.0, skin::normal());
                }
                Row::Sex => {
                    for (k, s) in Sex::ALL.iter().enumerate() {
                        let b = Rect::new(r.x + k as f32 * (r.w / 2.0 + 4.0), r.y, r.w / 2.0 - 4.0, r.h);
                        button(b, s.label(), active && c.sex == *s, c.sex == *s, b.contains(m));
                    }
                }
                _ => {
                    let (text, swatch) = match row {
                        Row::Race => (race.as_ref().map_or(String::new(), |r| r.name.clone()), None),
                        Row::Body => {
                            let b = self.creator_body(c);
                            let label = match (self.project.db.model(&b), self.project.db.sheet(&b)) {
                                (Some(m), _) => m.label().to_string(),
                                (None, Some(s)) => s.label(),
                                _ => b.clone(),
                            };
                            (label, None)
                        }
                        Row::Feature(f) => {
                            let (name, colors) = features.iter().find(|x| x.0 == *f).map(|x| (x.1.clone(), x.2.clone())).unwrap_or_default();
                            match c.colors.get(*f).copied().unwrap_or(0) {
                                0 => match race.as_ref().and_then(|r| r.colors.get(&name)) {
                                    Some(col) => ("The race's".to_string(), Some(*col)),
                                    None => ("As drawn".to_string(), self.project.db.model(&self.creator_body(c)).and_then(|m| m.features.get(*f)).and_then(|x| x.default)),
                                },
                                n => (format!("{}/{}", (n - 1) % colors.len().max(1) + 1, colors.len()), colors.get((n - 1) % colors.len().max(1)).copied()),
                            }
                        }
                        _ => (String::new(), None),
                    };
                    Self::draw_choice_control(r, active, m, &text, swatch);
                }
            }
        }

        // Preview: walks in place, turning every 1.5 s; the four directions below.
        let pr = l.preview;
        draw_rectangle(pr.x, pr.y, pr.w, pr.h, Color::new(0.0, 0.0, 0.0, 0.35));
        let a = self.chosen_appearance(c);
        {
            let order = [crate::entity::Facing::Down, crate::entity::Facing::Left, crate::entity::Facing::Up, crate::entity::Facing::Right];
            let facing = order[(c.time / 1.5) as usize % 4];
            let (fw, fh) = self.character_frame_size(&a);
            let big = (190.0 / fh).floor().max(1.0);
            let dest = Rect::new(pr.x + (pr.w - fw * big) / 2.0, pr.y + 6.0, fw * big, fh * big);
            // The model walks with its clip; standing in the four directions below.
            let gear = super::paperdoll::Gear::default();
            if let Some(cell) = self.model_cell(&a, &gear, (facing, super::paperdoll::Anim::Walking((c.time * 100.0) as u32))) {
                self.assets.models.draw_in(&cell, dest, WHITE);
            }
            for (k, f) in order.iter().enumerate() {
                let r = Rect::new(pr.x + 8.0 + k as f32 * 58.0, pr.y + pr.h - 88.0, fw * 1.6, fh * 1.6);
                if let Some(cell) = self.model_cell(&a, &gear, (*f, super::paperdoll::Anim::Idle(0))) {
                    self.assets.models.draw_in(&cell, r, WHITE);
                }
            }
        }

        // The race under the preview: its increases, speed, size and senses, its traits.
        if let Some(r) = race.as_ref().filter(|_| self.creator_races().len() > 1) {
            let mut y = pr.y + pr.h + 24.0;
            let bonus = r.bonus_text();
            if !bonus.is_empty() {
                y = wrapped(&bonus, pr.x, y, pr.w, 17.0, skin::highlight(), 2) + 2.0;
            }
            let size = if r.size_category.is_empty() { "Medium".to_string() } else { r.size_category.clone() };
            let mut facts = format!("{} - {} ft", crate::lang::tr_ui(&size), r.walk_feet());
            if r.darkvision_ft > 0 {
                facts += &format!(" - {}", crate::lang::tr_ui(&format!("darkvision {} ft", r.darkvision_ft)));
            }
            draw_text(&facts, pr.x, y, 18.0, skin::dim());
            y += 20.0;
            let traits: Vec<&str> = r.traits.iter().filter(|t| !t.name.is_empty() && t.id != "darkvision").map(|t| t.name.as_str()).collect();
            let text = if traits.is_empty() { r.description.clone() } else { traits.join(", ") };
            wrapped(&text, pr.x, y, pr.w, 17.0, skin::dim(), 4);
        }
    }

    fn draw_creator_page2(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let race = self.creator_race(c);
        let hero = self.creator_hero(c);
        let method = self.creator_method(c);
        let bg = self.creator_background(c);
        let bg_opts = Self::bg_language_options(c, race.as_ref());
        let choices = self.creator_choices(c);
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            let label = self.creator_label(c, *row);
            if !label.is_empty() {
                draw_text(&label, p.x + 30.0, r.y + 26.0, 22.0, if active { skin::highlight() } else { skin::dim() });
            }
            match row {
                Row::Ability(i) => {
                    let a = Ability::ALL[*i];
                    Self::draw_choice_control(Rect::new(r.x, r.y, 128.0, r.h), active, m, &c.base.score(a).to_string(), None);
                    // The racial bonus, the total and its modifier in columns of their own.
                    let total = hero.abilities.score(a);
                    let extra = total - c.base.score(a);
                    if extra != 0 {
                        draw_text(abilities::signed(extra as i64), r.x + 136.0, r.y + 25.0, 18.0, skin::highlight());
                    }
                    draw_text(total.to_string(), r.x + 180.0, r.y + 25.0, 22.0, skin::normal());
                    draw_text(format!("({})", abilities::signed(abilities::modifier(total) as i64)), r.x + 216.0, r.y + 24.0, 16.0, skin::dim());
                }
                Row::Reroll => button(r, "Roll again", active, false, r.contains(m)),
                Row::Method => {
                    let mut text = method.map_or(String::new(), |m| crate::lang::tr_ui(m.label()).into_owned());
                    if method == Some(AbilityMethod::PointBuy) {
                        let left = abilities::POINT_BUY_BUDGET - abilities::point_buy_spent(&c.base).unwrap_or(0);
                        text = format!("{text} ({left})");
                    }
                    Self::draw_choice_control(r, active, m, &text, None);
                }
                Row::Choice(ci, slot) => {
                    let Some(ch) = race.as_ref().and_then(|r| r.choices.get(*ci)) else { continue };
                    let picked = choices.get(&ch.id).and_then(|v| v.get(*slot)).cloned().unwrap_or_default();
                    // A short name of what is picked (the trait's name shows below while selected).
                    let name = crate::lang::tr_ui(ch.kind.label()).into_owned();

                    let value = match ch.kind {
                        ChoiceKind::Language => crate::backgrounds::language_label(&picked),
                        ChoiceKind::Skill | ChoiceKind::Tool | ChoiceKind::Ability => crate::lang::tr_ui(&ch.option_name(&picked)).into_owned(),
                        // "Draconic Ancestry (Red)" -> "Red".
                        _ => {
                            let n = ch.option_name(&picked);
                            match (n.find('('), n.rfind(')')) {
                                (Some(a), Some(b)) if b > a => n[a + 1..b].to_string(),
                                _ => n,
                            }
                        }
                    };
                    Self::draw_choice_control(r, active, m, &format!("{name}: {value}"), None);
                }
                Row::Background => {
                    let name = bg.map_or_else(|| crate::lang::tr_ui("None").into_owned(), |b| b.name.clone());
                    Self::draw_choice_control(r, active, m, &format!("{}: {name}", crate::lang::tr_ui("Background")), None);
                }
                Row::BgLanguage(slot) => {
                    let picks = Self::bg_picked(c, bg.map_or(0, |b| b.language_choices as usize), bg_opts.len());
                    let lang = picks.get(*slot).and_then(|k| bg_opts.get(*k)).map_or(String::new(), |id| crate::backgrounds::language_label(id));
                    Self::draw_choice_control(r, active, m, &format!("{}: {lang}", crate::lang::tr_ui("Language")), None);
                }
                _ => {}
            }
        }
        // Under the right column: the background's skills and feature, the hero's languages.
        let right: Vec<Rect> = choice_rows.iter().zip(&l.rows).filter(|(r, _)| !matches!(r, Row::Method | Row::Ability(_) | Row::Reroll)).map(|(_, r)| *r).collect();
        let x = p.x + 378.0;
        let mut y = right.last().map_or(p.y + 96.0, |r| r.bottom() + 26.0);
        let w = 356.0;
        // Lines that still fit above the Start / Back buttons.
        let room = |y: f32, size: f32| (((l.back.y - 6.0) - y) / (size + 2.0)).max(0.0) as usize;
        // The selected racial choice: its trait's name and what it gives.
        if let Some(Row::Choice(ci, _)) = sel {
            if let Some(ch) = race.as_ref().and_then(|r| r.choices.get(ci)) {
                y = wrapped(&ch.label, x, y, w, 17.0, skin::highlight(), room(y, 17.0).min(1));
                if let Some(t) = race.as_ref().and_then(|r| r.traits.iter().find(|t| t.id == ch.id)) {
                    y = wrapped(&t.description, x, y, w, 15.0, skin::dim(), room(y, 15.0).min(3)) + 6.0;
                }
            }
        }
        if let Some(b) = bg {
            let profs: Vec<String> = b.proficiencies.iter().map(|id| crate::lang::tr_ui(&abilities::proficiency_name(&abilities::proficiency_id(id))).into_owned()).collect();
            if !profs.is_empty() {
                y = wrapped(&profs.join(", "), x, y, w, 17.0, skin::normal(), room(y, 17.0).min(2));
            }
            if !b.feature.is_empty() {
                y = wrapped(&format!("{}: {}", b.feature, b.feature_text), x, y + 2.0, w, 16.0, skin::dim(), room(y + 2.0, 16.0).min(3));
            }
        }
        let langs: Vec<String> = hero.languages.iter().map(|id| crate::backgrounds::language_label(id)).collect();
        if !langs.is_empty() {
            wrapped(&format!("{} {}", crate::lang::tr_ui("Languages:"), langs.join(", ")), x, y + 4.0, w, 16.0, skin::dim(), room(y + 4.0, 16.0).min(2));
        }
    }
}
