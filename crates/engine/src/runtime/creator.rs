//! Character creator, shown after "New game" when the project enables it. Its pages follow the
//! order of making an SRD 5.1 hero; a page the project has nothing for is skipped:
//!
//! 1. *Who*: a name, the race (the playable races and subraces), male / female, a model (the
//!    race's figures of that sex; without races the project's default figure for that sex and
//!    the other playable models of that sex), a colour for each colour feature of the model that
//!    offers colours, and the race's choices that colour it (a dragonborn's ancestry).
//! 2. *Class*: the class, the skills it lets the hero choose and level 1's other choices (a
//!    fighting style, a cleric's domain, a warlock's patron), the same rows the level-up window
//!    shows.
//! 3. *Ability scores*: by standard array (placed by the class's `ability_order`, swappable),
//!    point buy (starting from that array) or 4d6 rolls (placed the same way), and the race's
//!    other choices (ability increases, skills, a tool, a language, a cantrip).
//! 4. *Background*: the background, its languages and level 1's expertise (which needs every
//!    skill the hero has).
//! 5. *Spells*: the cantrips and spells a class picks at level 1 (a wizard's spellbook, a
//!    sorcerer's known spells), as a checklist.
//! 6. *Starting equipment*: the starter store (`CharacterCreation::store`, a character's shop):
//!    the cart starts with the class's equipment (`ClassDef::equipment`, as far as the store has
//!    it), and the budget is the class's starting gold (`ClassDef::starting_gold`) or the
//!    equipment's price, whichever is more, so the kit always fits and any part of it can be
//!    swapped; what is left stays in the purse.
//!
//! Keyboard: typing edits the name (page 1; Backspace deletes), Up/Down select a row,
//! Left/Right change it (a store item: one less / more), Enter goes on (a spell: picks it, an
//! item: buys one), Esc goes back. Everything can be clicked; lists scroll with the wheel.

use std::collections::BTreeMap;

use gfx::prelude::*;

use super::skin;

use super::input::{self, Act, Pad};
use super::levelup::ChoiceRow;
use super::text::{draw_plain, draw_text, fit_plain, fit_size, measure_plain};
use super::{ui::panel, Game};
use crate::classes::{ClassDef, HeroClass};
use crate::defs::Appearance;
use crate::item_category::StoreTab;
use crate::races::{ChoiceKind, RaceDef};
use crate::srd::abilities::{self, Ability, AbilityMethod};
use crate::srd::Abilities;
use crate::state::Sex;

const W: f32 = 860.0;
const H: f32 = 600.0;
const MAX_NAME: usize = 16;
const ROW_H: f32 = 54.0;
/// The other pages' rows.
const ROW2_H: f32 = 44.0;
/// A list row (spells, the store).
const LIST_H: f32 = 30.0;

/// Options of a choice: (id, name).
type Options = Vec<(String, String)>;

/// A page of the creator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Who,
    Class,
    Abilities,
    Background,
    Spells,
    Store,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Who => "Create your character",
            Page::Class => "Class",
            Page::Abilities => "Ability scores",
            Page::Background => "Background",
            Page::Spells => "Spells",
            Page::Store => "Starting equipment",
        }
    }

    /// The page's name in `creator=` (status).
    fn id(self) -> &'static str {
        match self {
            Page::Who => "who",
            Page::Class => "class",
            Page::Abilities => "abilities",
            Page::Background => "background",
            Page::Spells => "spells",
            Page::Store => "store",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Name,
    /// Playable races (only when there is more than one).
    Race,
    Sex,
    /// Playable figures of the chosen sex (only when there is more than one).
    Body,
    /// A colour feature of the chosen figure and its pieces (index into `creator_all_features`)
    /// that offers colours.
    Feature(usize),
    /// A piece the player may choose (index into `CHOSEN_SLOTS`: hair, beard).
    Part(usize),
    /// How the scores are set.
    Method,
    /// An ability's base score (index into `Ability::ALL`).
    Ability(usize),
    /// Roll the six scores again (the *Roll* method).
    Reroll,
    /// A pick of a racial choice (choice index in the merged race, slot).
    Choice(usize, usize),
    /// The background.
    Background,
    /// A language the background lets the hero choose (slot).
    BgLanguage(usize),
    /// The class.
    Class,
    /// A skill the class lets the hero choose (slot).
    ClassSkill(usize),
    /// A level 1 choice of the class (index into `creator_level_rows`).
    Level(usize),
    /// A cantrip or spell of the checklist (index into `creator_spell_list`).
    Spell(usize),
    /// An item of the starter store (index into `creator_store`).
    Item(usize),
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
    /// Chosen colour per feature (by name) of the figure: 0 = as drawn (or the race's colour),
    /// k = the feature's colour k - 1.
    colors: BTreeMap<String, usize>,
    /// Chosen piece per slot: an index into `creator_part_options` (none = the figure's own).
    parts: BTreeMap<String, usize>,
    row: usize,
    time: f32,
    page: Page,
    /// Index into the project's ability methods.
    method: usize,
    /// Base ability scores (before the race's bonuses).
    base: Abilities,
    /// The class the base scores were placed for (the standard array follows the class).
    placed_for: Option<usize>,
    /// Racial choice index -> the picked option per slot (indices into its options).
    picks: BTreeMap<usize, Vec<usize>>,
    /// Index into the backgrounds (= their number: none).
    background: usize,
    /// Picked languages of the background (indices into `bg_language_options`).
    bg_langs: Vec<usize>,
    /// Index into the classes.
    class: usize,
    /// Picked skills of the class (indices into its options).
    class_skills: Vec<usize>,
    /// Level 1's choices: the picked option of each of `creator_level_rows` (None = its default).
    level: Vec<Option<usize>>,
    /// The cantrips and spells picked (ids).
    spells: Vec<String>,
    /// The store's cart: item id -> how many.
    cart: BTreeMap<String, u32>,
    /// The class whose equipment the cart was filled with (filled again for another class).
    kit_for: Option<usize>,
    /// The store's filter tab.
    store_tab: StoreTab,
    /// The first list row shown (spells, the store).
    scroll: usize,
    /// A line under the page (a pick that isn't possible) and how long it stays.
    message: Option<(String, f32)>,
}

struct Layout {
    panel: Rect,
    /// Control area of each choice row (rows before Start / Back), in row order; a list row
    /// scrolled out of view has an empty rect.
    rows: Vec<Rect>,
    preview: Rect,
    start: Rect,
    back: Rect,
}

/// The "<" and ">" buttons of a choice row.
fn arrows(r: Rect) -> (Rect, Rect) {
    (Rect::new(r.x, r.y, 34.0, r.h), Rect::new(r.x + r.w - 34.0, r.y, 34.0, r.h))
}

/// A button with a runtime text (translated; a smaller font when it is wider than the button).
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
    let t = crate::lang::tr_ui(label);
    let size = fit_size(&t, 24.0, r.w - 12.0, 12.0);
    let t = fit_plain(&t, size, r.w - 8.0);
    let tw = measure_plain(&t, size).width;
    draw_plain(&t, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + size / 3.0, size, skin::normal());
}

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

/// Draws `text` (in the player's language) wrapped into `w` from `y` on (at most `max_lines`,
/// 0 = nothing; the last line cut with "..." when there is more); returns the y below it.
fn wrapped(text: &str, x: f32, mut y: f32, w: f32, size: f32, color: Color, max_lines: usize) -> f32 {
    if max_lines == 0 {
        return y;
    }
    let lines = super::text::wrap(text, size, w);
    for (k, line) in lines.iter().take(max_lines).enumerate() {
        let line = if k + 1 == max_lines && lines.len() > max_lines { fit_plain(&format!("{line} ..."), size, w) } else { line.clone() };
        draw_plain(&line, x, y, size, color);
        y += size + 2.0;
    }
    y
}

/// The abilities in a class's order (best first; the rest in the usual order).
fn ability_order(cl: Option<&ClassDef>) -> Vec<Ability> {
    let mut order: Vec<Ability> = cl.map_or(Vec::new(), |c| c.ability_order.iter().filter_map(|a| Ability::parse(a)).collect());
    for a in Ability::ALL {
        if !order.contains(&a) {
            order.push(a);
        }
    }
    order
}

/// Six scores placed by a class's order: the highest on its first ability.
fn place(mut scores: Vec<i32>, cl: Option<&ClassDef>) -> Abilities {
    scores.sort_unstable_by(|a, b| b.cmp(a));
    let mut out = [8; 6];
    for (a, s) in ability_order(cl).iter().zip(scores) {
        out[Ability::ALL.iter().position(|x| x == a).unwrap_or(0)] = s;
    }
    Abilities(out)
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
        let features = self.creator_all_features(c);
        // (Features a race's choice colours, a dragonborn's ancestry, get no row of their own.)
        let chosen: Vec<String> = self.creator_race(c).map_or(Vec::new(), |r| r.choices.iter().flat_map(|ch| ch.colors.values().flat_map(|m| m.keys().cloned())).collect());
        features.iter().enumerate().filter(|(_, f)| !f.colors.is_empty() && !chosen.contains(&f.name)).map(|(k, f)| (k, f.name.clone(), f.colors.clone(), f.label())).collect()
    }

    /// The colour features of the chosen figure and of the pieces it wears: a feature counts once
    /// (the body's, else the first piece's that has it), with the first colours offered for it.
    fn creator_all_features(&self, c: &Creator) -> Vec<crate::defs::ColorFeature> {
        let db = &self.project.db;
        let body = self.creator_body(c);
        let mut out: Vec<crate::defs::ColorFeature> = db.model(&body).map_or(Vec::new(), |m| m.features.clone());
        let look = Appearance { model: body.clone(), parts: self.creator_parts(c), ..Default::default() };
        for id in crate::render::model::parts_of(db, &body, &look, &Default::default()) {
            for f in db.model(&id).map_or(&[][..], |m| &m.features[..]) {
                match out.iter_mut().find(|o| o.name == f.name) {
                    Some(o) if o.colors.is_empty() => o.colors = f.colors.clone(),
                    Some(_) => {}
                    None => out.push(f.clone()),
                }
            }
        }
        out
    }

    /// What the player may wear in `slot` on figure `body`: no piece, then the offered pieces
    /// that fit it (model ids); and which of them the figure has of its own.
    fn creator_part_options(&self, body: &str, slot: &str) -> (Vec<String>, usize) {
        let db = &self.project.db;
        let mut ids = vec![crate::defs::NO_PART.to_string()];
        ids.extend(db.models.iter().filter(|m| m.part.as_ref().is_some_and(|p| p.slot == slot && p.offer && (p.fits.is_empty() || p.fits.iter().any(|f| f == body)))).map(|m| m.id.clone()));
        let own = db.model(body).and_then(|b| b.parts.iter().find(|id| db.model(id).and_then(|m| m.part.as_ref()).is_some_and(|p| p.slot == slot)).cloned());
        let own = match own {
            Some(o) => ids.iter().position(|i| *i == o).unwrap_or_else(|| {
                ids.push(o);
                ids.len() - 1
            }),
            None => 0,
        };
        (ids, own)
    }

    /// The slots the creator has a row for: those with a piece to choose for the chosen figure.
    fn creator_part_rows(&self, c: &Creator) -> Vec<usize> {
        let body = self.creator_body(c);
        (0..crate::defs::CHOSEN_SLOTS.len()).filter(|k| self.creator_part_options(&body, crate::defs::CHOSEN_SLOTS[*k]).0.len() > 1).collect()
    }

    /// The pieces the player chose that differ from the figure's own (a look's `parts`).
    fn creator_parts(&self, c: &Creator) -> BTreeMap<String, String> {
        let body = self.creator_body(c);
        let mut out = BTreeMap::new();
        for (slot, k) in &c.parts {
            let (ids, own) = self.creator_part_options(&body, slot);
            if k % ids.len() != own {
                out.insert(slot.clone(), ids[k % ids.len()].clone());
            }
        }
        out
    }

    /// The name of a piece in the creator's row ("Ponytail"; "None" for no piece).
    fn creator_part_label(&self, id: &str) -> String {
        match self.project.db.model(id).and_then(|m| m.part.as_ref()) {
            Some(p) => crate::lang::tr(&p.label()),
            None => crate::lang::tr_ui("None").into_owned(),
        }
    }

    /// The race's choices that colour the model (picked on page 1): their indices.
    fn color_choices(&self, c: &Creator) -> Vec<usize> {
        self.creator_race(c).map_or(Vec::new(), |r| r.choices.iter().enumerate().filter(|(_, ch)| !ch.colors.is_empty() && !ch.options.is_empty()).map(|(ci, _)| ci).collect())
    }

    /// The colours a colour choice's current pick gives: feature -> colour.
    fn choice_colors(c: &Creator, ci: usize, ch: &crate::races::RaceChoice) -> std::collections::BTreeMap<String, [u8; 3]> {
        let pick = Self::picked(c, ci, 1, ch.options.len()).first().copied().unwrap_or(0);
        ch.options.get(pick).and_then(|o| ch.colors.get(o)).cloned().unwrap_or_default()
    }

    /// The project's ability methods.
    fn creator_methods(&self) -> Vec<AbilityMethod> {
        self.project.config.creation.ability_methods.clone()
    }

    /// Base scores a method starts with for class `cl`: the standard array placed by the
    /// class's order (also point buy's start: the array costs the whole budget), or six 4d6
    /// rolls placed the same way.
    fn method_start(m: AbilityMethod, cl: Option<&ClassDef>) -> Abilities {
        match m {
            AbilityMethod::Standard | AbilityMethod::PointBuy => place(Abilities::STANDARD.0.to_vec(), cl),
            AbilityMethod::Roll => place((0..6).map(|_| abilities::roll_score()).collect(), cl),
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

    /// The hero the creator makes so far (for the totals and proficiencies it shows).
    fn creator_hero(&self, c: &Creator) -> crate::races::Hero {
        let race = self.creator_race(c);
        let base = self.creator_method(c).map(|_| c.base);
        crate::races::build_hero(race.as_ref(), &self.project.config.rules, base, &self.creator_choices(c), self.creator_background(c))
    }

    /// The chosen class (None = the project has none).
    fn creator_class(&self, c: &Creator) -> Option<&ClassDef> {
        self.project.db.classes.get(c.class)
    }

    /// The class's skills the creator picked (distinct, as many as the class lets choose).
    fn class_skills_picked(c: &Creator, choose: usize, n: usize) -> Vec<usize> {
        let mut picks = c.class_skills.clone();
        picks.truncate(choose.min(n));
        let mut next = 0;
        while picks.len() < choose.min(n) {
            while picks.contains(&next) {
                next += 1;
            }
            picks.push(next);
        }
        picks
    }

    /// The class skills picked (ids).
    fn creator_class_skills(&self, c: &Creator) -> Vec<String> {
        self.creator_class(c).map(|cl| Self::class_skills_picked(c, cl.skill_choices as usize, cl.skill_options.len()).iter().filter_map(|k| cl.skill_options.get(*k).cloned()).collect()).unwrap_or_default()
    }

    /// Every proficiency the hero has so far: race, background and rules (the hero), the class's
    /// and the class skills picked.
    fn creator_profs(&self, c: &Creator) -> Vec<String> {
        let mut v: Vec<String> = self.creator_hero(c).proficiencies.into_iter().collect();
        if let Some(cl) = self.creator_class(c) {
            v.extend(cl.proficiencies.iter().cloned());
            v.extend(cl.saves.iter().map(|s| format!("save:{s}")));
        }
        v.extend(self.creator_class_skills(c));
        v.sort();
        v.dedup();
        v
    }

    /// Level 1's choice rows of the class except its spells (a fighting style, a domain,
    /// expertise), as the level-up window shows them.
    fn creator_level_rows(&self, c: &Creator) -> Vec<ChoiceRow> {
        let Some(cl) = self.creator_class(c) else { return Vec::new() };
        let hc = HeroClass { class: cl.id.clone(), ..Default::default() };
        self.level_rows(cl, &hc, 1, &self.creator_profs(c), &[]).into_iter().filter(|r| r.feature != "cantrips" && r.feature != "spells").collect()
    }

    /// Whether level row `r` belongs on the background page (it needs every skill: expertise).
    fn on_background_page(r: &ChoiceRow) -> bool {
        r.label == "Expertise"
    }

    /// The picked option of level row `k` (distinct from the rows of the same feature before it
    /// by default).
    fn level_pick(c: &Creator, rows: &[ChoiceRow], k: usize) -> usize {
        let n = rows[k].options.len().max(1);
        match c.level.get(k).copied().flatten() {
            Some(p) => p % n,
            None => rows[..k].iter().filter(|o| o.feature == rows[k].feature).count() % n,
        }
    }

    /// Level 1's spell picks: how many cantrips and spells, and the options (id, name) of each.
    fn creator_spell_choices(&self, c: &Creator) -> (usize, usize, Options, Options) {
        let Some(cl) = self.creator_class(c) else { return (0, 0, Vec::new(), Vec::new()) };
        let (cantrips, spells) = Self::spell_gains(cl, 1);
        let (co, so) = (self.spell_options(cl, 1, true, &[]), self.spell_options(cl, 1, false, &[]));
        ((cantrips as usize).min(co.len()), (spells as usize).min(so.len()), co, so)
    }

    /// The checklist: the cantrip options, then the spell options (id, is a cantrip).
    fn creator_spell_list(&self, c: &Creator) -> Vec<(String, bool)> {
        let (nc, ns, co, so) = self.creator_spell_choices(c);
        let mut v: Vec<(String, bool)> = Vec::new();
        if nc > 0 {
            v.extend(co.into_iter().map(|(id, _)| (id, true)));
        }
        if ns > 0 {
            v.extend(so.into_iter().map(|(id, _)| (id, false)));
        }
        v
    }

    /// How many cantrips and spells are picked.
    fn creator_spell_counts(&self, c: &Creator) -> (usize, usize) {
        let list = self.creator_spell_list(c);
        let picked = |cantrip: bool| list.iter().filter(|(id, k)| *k == cantrip && c.spells.contains(id)).count();
        (picked(true), picked(false))
    }

    /// The starter store's stock: item id and price in copper pieces (the shop's price, else
    /// the item's cost); empty when the project has no store or the class no starting gold.
    /// The starter store's stock under the chosen filter tab.
    fn creator_store(&self, c: &Creator) -> Vec<(String, u32)> {
        let ammo = crate::item_category::ammo_of(&self.project.db);
        let mut v = self.creator_store_all(c);
        v.retain(|(id, _)| c.store_tab.keeps(crate::item_category::category_of(&self.project.db, id, &ammo)));
        v
    }

    /// The filter tabs the starter store offers.
    fn creator_store_tabs(&self, c: &Creator) -> Vec<StoreTab> {
        let ammo = crate::item_category::ammo_of(&self.project.db);
        let cats: Vec<_> = self.creator_store_all(c).iter().map(|(id, _)| crate::item_category::category_of(&self.project.db, id, &ammo)).collect();
        StoreTab::offered(&cats)
    }

    /// The starter store's filter tabs on screen.
    fn creator_store_tab_rects(&self, c: &Creator) -> Vec<(StoreTab, Rect)> {
        let p = Self::creator_panel();
        super::shop_ui::tab_rects(&self.creator_store_tabs(c), Rect::new(p.x + 30.0, p.y + 110.0, 470.0, 26.0))
    }

    /// Shows the store's items under tab `t` (from the top of the list).
    fn set_creator_store_tab(&mut self, t: StoreTab) {
        if let Some(c) = &mut self.creator {
            if c.store_tab != t {
                c.store_tab = t;
                c.scroll = 0;
                c.row = 0;
            }
        }
    }

    /// Screen position of the starter store's filter tab `id` (for automated play-throughs).
    pub fn creator_tab_pos(&self, id: &str) -> Option<Vec2> {
        let c = self.creator.as_ref().filter(|c| c.page == Page::Store)?;
        self.creator_store_tab_rects(c).into_iter().find(|(t, _)| t.id() == id).map(|(_, r)| r.center())
    }

    /// The starter store's whole stock.
    fn creator_store_all(&self, c: &Creator) -> Vec<(String, u32)> {
        if self.creator_class(c).is_none_or(|cl| cl.starting_gold_cp() == 0) {
            return Vec::new();
        }
        let Some(shop) = self.project.db.character(&self.project.config.creation.store).and_then(|d| d.shop.as_ref()) else { return Vec::new() };
        shop.items.iter().filter_map(|e| self.project.db.item(&e.item).map(|d| (e.item.clone(), if e.price > 0 { e.price } else { d.cost_cp }))).collect()
    }

    /// The class's starting equipment the store sells (item id, how many) and its price (cp).
    fn creator_kit(&self, c: &Creator) -> (BTreeMap<String, u32>, u32) {
        let prices: BTreeMap<String, u32> = self.creator_store_all(c).into_iter().collect();
        let mut kit = BTreeMap::new();
        let mut value = 0;
        for st in self.creator_class(c).map_or(&[][..], |cl| &cl.equipment[..]) {
            if let Some(price) = prices.get(&st.item) {
                *kit.entry(st.item.clone()).or_insert(0) += st.count.max(1);
                value += price * st.count.max(1);
            }
        }
        (kit, value)
    }

    /// The budget (cp: the starting gold or the class's equipment's price, whichever is more)
    /// and what the cart costs.
    fn creator_purse(&self, c: &Creator) -> (u32, u32) {
        let gold = self.creator_class(c).map_or(0, |cl| cl.starting_gold_cp()).max(self.creator_kit(c).1);
        let prices: BTreeMap<String, u32> = self.creator_store_all(c).into_iter().collect();
        let spent = c.cart.iter().map(|(id, n)| prices.get(id).copied().unwrap_or(0) * n).sum();
        (gold, spent)
    }

    /// The pages the project has: who always; the class with classes; the scores with ability
    /// methods or racial choices; the background with backgrounds (or expertise at level 1);
    /// spells when the class picks some at level 1; the store with one and starting gold.
    fn creator_pages(&self, c: &Creator) -> Vec<Page> {
        let mut v = vec![Page::Who];
        if !self.project.db.classes.is_empty() {
            v.push(Page::Class);
        }
        if !self.creator_methods().is_empty() || self.creator_race(c).is_some_and(|r| r.choices.iter().any(|ch| ch.colors.is_empty())) {
            v.push(Page::Abilities);
        }
        if !self.project.db.backgrounds.is_empty() || self.creator_level_rows(c).iter().any(Self::on_background_page) {
            v.push(Page::Background);
        }
        if !self.creator_spell_list(c).is_empty() {
            v.push(Page::Spells);
        }
        if !self.creator_store_all(c).is_empty() {
            v.push(Page::Store);
        }
        v
    }

    /// The label of a row in the player's language (feature rows: the feature's name).
    fn creator_label(&self, c: &Creator, row: Row) -> String {
        let ui = |s: &str| crate::lang::tr_ui(s).into_owned();
        match row {
            Row::Name => ui("Name"),
            Row::Race => ui("Race"),
            Row::Sex => ui("Sex"),
            Row::Body => ui("Body"),
            Row::Feature(k) => self.creator_features(c).into_iter().find(|f| f.0 == k).map_or(String::new(), |f| crate::lang::tr(&f.3)),
            Row::Part(k) => ui(if crate::defs::CHOSEN_SLOTS[k] == "hair" { "Hair style" } else { "Beard" }),
            Row::Method => ui("Scores"),
            Row::Ability(i) => Ability::ALL[i].short().into(),
            // (Page 1: a choice that colours the model, "Ancestry".)
            Row::Choice(ci, _) if c.page == Page::Who => self.creator_race(c).and_then(|r| r.choices.get(ci).map(|ch| ui(ch.kind.label()))).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// The rows of the current page (Race and Body only with a choice; a row per colour feature
    /// that offers colours; what the project, the race and the class offer).
    fn creator_rows(&self, c: &Creator) -> Vec<Row> {
        let mut rows = Vec::new();
        let level = self.creator_level_rows(c);
        match c.page {
            Page::Who => {
                rows.push(Row::Name);
                if self.creator_races().len() > 1 {
                    rows.push(Row::Race);
                }
                rows.push(Row::Sex);
                if self.creator_bodies(self.creator_race(c).as_ref(), c.sex).len() > 1 {
                    rows.push(Row::Body);
                }
                rows.extend(self.color_choices(c).into_iter().map(|ci| Row::Choice(ci, 0)));
                rows.extend(self.creator_part_rows(c).into_iter().map(Row::Part));
                rows.extend(self.creator_features(c).into_iter().map(|f| Row::Feature(f.0)));
            }
            Page::Class => {
                if let Some(cl) = self.creator_class(c) {
                    rows.push(Row::Class);
                    rows.extend((0..(cl.skill_choices as usize).min(cl.skill_options.len())).map(Row::ClassSkill));
                    rows.extend(level.iter().enumerate().filter(|(_, r)| !Self::on_background_page(r) && !r.options.is_empty()).map(|(k, _)| Row::Level(k)));
                }
            }
            Page::Abilities => {
                if let Some(m) = self.creator_method(c) {
                    rows.push(Row::Method);
                    rows.extend((0..6).map(Row::Ability));
                    if m == AbilityMethod::Roll {
                        rows.push(Row::Reroll);
                    }
                }
                if let Some(r) = self.creator_race(c) {
                    for (ci, ch) in r.choices.iter().enumerate().filter(|(_, ch)| ch.colors.is_empty()) {
                        rows.extend((0..(ch.choose as usize).min(ch.options.len())).map(|slot| Row::Choice(ci, slot)));
                    }
                }
            }
            Page::Background => {
                if !self.project.db.backgrounds.is_empty() {
                    rows.push(Row::Background);
                    if let Some(b) = self.creator_background(c) {
                        rows.extend((0..b.language_choices as usize).map(Row::BgLanguage));
                    }
                }
                rows.extend(level.iter().enumerate().filter(|(_, r)| Self::on_background_page(r) && !r.options.is_empty()).map(|(k, _)| Row::Level(k)));
            }
            Page::Spells => rows.extend((0..self.creator_spell_list(c).len()).map(Row::Spell)),
            Page::Store => rows.extend((0..self.creator_store(c).len()).map(Row::Item)),
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
        a.parts = self.creator_parts(c);
        let race = self.creator_race(c);
        for (_, name, colors, _) in self.creator_features(c) {
            match c.colors.get(&name).copied().unwrap_or(0) {
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
        // A choice that colours the model: its pick's colours (a dragonborn's ancestry).
        if let Some(r) = &race {
            for ci in self.color_choices(c) {
                if let Some(ch) = r.choices.get(ci) {
                    a.colors.extend(Self::choice_colors(c, ci, ch));
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
        let class = self.project.db.classes.iter().position(|c| c.id == "fighter").unwrap_or(0);
        let cl = self.project.db.classes.get(class);
        let base = self.creator_methods().first().map_or(self.project.config.rules.abilities, |m| Self::method_start(*m, cl));
        self.creator = Some(Creator {
            name: self.default_player_name(),
            race,
            sex,
            body: [0, 0],
            colors: BTreeMap::new(),
            parts: BTreeMap::new(),
            row: 0,
            time: 0.0,
            page: Page::Who,
            method: 0,
            base,
            placed_for: Some(class),
            picks: BTreeMap::new(),
            background: 0,
            bg_langs: Vec::new(),
            class,
            class_skills: Vec::new(),
            level: Vec::new(),
            spells: Vec::new(),
            cart: BTreeMap::new(),
            kit_for: None,
            store_tab: StoreTab::All,
            scroll: 0,
            message: None,
        });
    }

    /// Shows a line under the page for a few seconds (a runtime text, translated).
    fn creator_say(&mut self, text: String) {
        if let Some(c) = &mut self.creator {
            c.message = Some((text, 3.0));
        }
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
            Row::Part(k) => self.creator_part_options(&self.creator_body(c), crate::defs::CHOSEN_SLOTS[k]).0.len(),
            Row::Method => self.creator_methods().len(),
            Row::Background => self.project.db.backgrounds.len() + 1,
            Row::Class => self.project.db.classes.len(),
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
        let (cl_choose, cl_n) = self.creator_class(c).map_or((0, 0), |cl| (cl.skill_choices as usize, cl.skill_options.len()));
        let level = self.creator_level_rows(c);
        let level_picks: Vec<usize> = (0..level.len()).map(|k| Self::level_pick(c, &level, k)).collect();
        let fixed = self.project.config.creation.sex.fixed().is_some();
        let classes = self.project.db.classes.clone();
        let feature = match row {
            Row::Feature(k) => self.creator_features(c).into_iter().find(|f| f.0 == k).map(|f| f.1),
            _ => None,
        };
        let own_part = match row {
            Row::Part(k) => self.creator_part_options(&self.creator_body(c), crate::defs::CHOSEN_SLOTS[k]).1,
            _ => 0,
        };
        let c = self.creator.as_mut().unwrap();
        let cycle = |v: &mut usize| *v = (*v as i32 + step).rem_euclid(n) as usize;
        // The next option no other slot of a choice holds.
        let next_free = |picks: &[usize], slot: usize, n: usize| {
            let mut k = picks[slot];
            loop {
                k = (k as i32 + step).rem_euclid(n as i32) as usize;
                if !picks.iter().enumerate().any(|(s, p)| s != slot && *p == k) {
                    return k;
                }
            }
        };
        match row {
            Row::Race => {
                cycle(&mut c.race);
                // Another race has other figures, colours and choices.
                c.body = [0, 0];
                c.colors.clear();
                c.parts.clear();
                c.picks.clear();
                c.bg_langs.clear();
                c.level.clear();
            }
            Row::Sex if !fixed => {
                c.sex = if c.sex == Sex::Male { Sex::Female } else { Sex::Male };
                c.colors.clear();
                c.parts.clear();
            }
            Row::Body => {
                cycle(&mut c.body[c.sex as usize]);
                c.colors.clear();
                c.parts.clear();
            }
            Row::Feature(_) => {
                if let Some(name) = feature {
                    cycle(c.colors.entry(name).or_insert(0));
                }
            }
            Row::Part(k) => cycle(c.parts.entry(crate::defs::CHOSEN_SLOTS[k].to_string()).or_insert(own_part)),
            Row::Method => {
                cycle(&mut c.method);
                c.base = Self::method_start(methods[c.method % methods.len()], classes.get(c.class));
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
            Row::Reroll => c.base = Self::method_start(AbilityMethod::Roll, classes.get(c.class)),
            Row::Choice(ci, slot) => {
                let Some(ch) = choice else { return };
                let n = ch.options.len();
                let mut picks = Self::picked(c, ci, ch.choose as usize, n);
                if n > picks.len() {
                    picks[slot] = next_free(&picks, slot, n);
                }
                c.picks.insert(ci, picks);
                if ch.kind == ChoiceKind::Language {
                    c.bg_langs.clear();
                }
                // (Skills picked here change what expertise offers.)
                c.level.clear();
            }
            Row::Background => {
                cycle(&mut c.background);
                c.bg_langs.clear();
                c.level.retain(|_| false);
            }
            Row::Class => {
                cycle(&mut c.class);
                // Another class: other skills, level 1 choices, spells, gold and its scores.
                c.class_skills.clear();
                c.level.clear();
                c.spells.clear();
                c.cart.clear();
                c.kit_for = None;
            }
            Row::ClassSkill(slot) => {
                let mut picks = Self::class_skills_picked(c, cl_choose, cl_n);
                if cl_n > picks.len() && slot < picks.len() {
                    picks[slot] = next_free(&picks, slot, cl_n);
                }
                c.class_skills = picks;
                c.level.clear();
            }
            Row::BgLanguage(slot) => {
                let mut picks = Self::bg_picked(c, bg_choose, bg_opts);
                if bg_opts > picks.len() && slot < picks.len() {
                    picks[slot] = next_free(&picks, slot, bg_opts);
                }
                c.bg_langs = picks;
            }
            Row::Level(k) => {
                let Some(r) = level.get(k) else { return };
                let n = r.options.len();
                if n == 0 {
                    return;
                }
                // (Distinct from the other rows of the same feature: two expertise skills.)
                let same: Vec<usize> = level.iter().enumerate().filter(|(_, o)| o.feature == r.feature).map(|(j, _)| j).collect();
                let picks: Vec<usize> = same.iter().map(|j| level_picks[*j]).collect();
                let slot = same.iter().position(|j| *j == k).unwrap_or(0);
                let pick = if n > picks.len() { next_free(&picks, slot, n) } else { (level_picks[k] as i32 + step).rem_euclid(n as i32) as usize };
                if c.level.len() < level.len() {
                    c.level.resize(level.len(), None);
                }
                c.level[k] = Some(pick);
            }
            _ => {}
        }
    }

    /// Picks or drops spell `k` of the checklist.
    fn toggle_spell(&mut self, k: usize) {
        let Some(c) = &self.creator else { return };
        let list = self.creator_spell_list(c);
        let Some((id, cantrip)) = list.get(k).cloned() else { return };
        let (nc, ns, _, _) = self.creator_spell_choices(c);
        let (pc, ps) = self.creator_spell_counts(c);
        let c = self.creator.as_mut().unwrap();
        if let Some(at) = c.spells.iter().position(|s| *s == id) {
            c.spells.remove(at);
            return;
        }
        let (have, most) = if cantrip { (pc, nc) } else { (ps, ns) };
        if have >= most {
            let what = if cantrip { "You know {} cantrips: drop one first." } else { "You know {} spells: drop one first." };
            let t = crate::lang::tr_ui(what).replacen("{}", &most.to_string(), 1);
            self.creator_say(t);
            return;
        }
        c.spells.push(id);
    }

    /// Buys one more of store item `k` (`more`) or puts one back.
    fn change_cart(&mut self, k: usize, more: bool) {
        let Some(c) = &self.creator else { return };
        let Some((id, price)) = self.creator_store(c).get(k).cloned() else { return };
        let (gold, spent) = self.creator_purse(c);
        let c = self.creator.as_mut().unwrap();
        if more {
            if spent + price > gold {
                self.creator_say(crate::lang::tr_ui("Not enough money.").into_owned());
                return;
            }
            *c.cart.entry(id).or_insert(0) += 1;
        } else if let Some(n) = c.cart.get_mut(&id) {
            *n -= 1;
            if *n == 0 {
                c.cart.remove(&id);
            }
        }
    }

    /// Why the page can't be left forward yet (the spells not all picked).
    fn creator_page_missing(&self, c: &Creator) -> Option<String> {
        if c.page != Page::Spells {
            return None;
        }
        let (nc, ns, _, _) = self.creator_spell_choices(c);
        let (pc, ps) = self.creator_spell_counts(c);
        if pc < nc {
            return Some(crate::lang::tr_ui("Choose {} more cantrips.").replacen("{}", &(nc - pc).to_string(), 1));
        }
        if ps < ns {
            return Some(crate::lang::tr_ui("Choose {} more spells.").replacen("{}", &(ns - ps).to_string(), 1));
        }
        None
    }

    /// Next (the last page: start the adventure).
    fn creator_forward(&mut self) {
        let Some(c) = &self.creator else { return };
        if let Some(why) = self.creator_page_missing(c) {
            self.creator_say(why);
            return;
        }
        let pages = self.creator_pages(c);
        let at = pages.iter().position(|p| *p == c.page).unwrap_or(0);
        let Some(next) = pages.get(at + 1).copied() else {
            self.start_created();
            return;
        };
        let class = c.class;
        let cl = self.project.db.classes.get(class).cloned();
        let method = self.creator_method(c);
        let kit = (next == Page::Store && c.kit_for != Some(class)).then(|| self.creator_kit(c).0);
        let c = self.creator.as_mut().unwrap();
        // The store's cart starts with the class's equipment (again for a class picked since);
        // the player swaps what they like.
        if let Some(kit) = kit {
            c.cart = kit;
            c.kit_for = Some(class);
        }
        // The scores follow the class: placed again for a class picked since.
        if next == Page::Abilities && c.placed_for != Some(class) {
            if let Some(m) = method {
                c.base = Self::method_start(m, cl.as_ref());
            }
            c.placed_for = Some(class);
        }
        c.page = next;
        c.row = 0;
        c.scroll = 0;
        c.message = None;
    }

    /// Back (the first page: to the title screen).
    fn creator_backward(&mut self) {
        let Some(c) = &self.creator else { return };
        let pages = self.creator_pages(c);
        let at = pages.iter().position(|p| *p == c.page).unwrap_or(0);
        match at.checked_sub(1).and_then(|k| pages.get(k)).copied() {
            Some(prev) => {
                let c = self.creator.as_mut().unwrap();
                c.page = prev;
                c.row = 0;
                c.scroll = 0;
                c.message = None;
            }
            None => self.creator = None,
        }
    }

    /// Level 1's picks made in the creator (feature -> option ids, `cantrips` and `spells`).
    fn creator_level_picks(&self, c: &Creator) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let rows = self.creator_level_rows(c);
        for (k, r) in rows.iter().enumerate() {
            if let Some((id, _)) = r.options.get(Self::level_pick(c, &rows, k)) {
                out.entry(r.feature.clone()).or_default().push(id.clone());
            }
        }
        for (id, cantrip) in self.creator_spell_list(c) {
            if c.spells.contains(&id) {
                out.entry(if cantrip { "cantrips" } else { "spells" }.to_string()).or_default().push(id);
            }
        }
        out
    }

    fn start_created(&mut self) {
        let Some(c) = self.creator.take() else { return };
        let appearance = self.chosen_appearance(&c);
        let race = self.creator_race(&c).map(|r| r.id.clone()).unwrap_or_default();
        let base = self.creator_method(&c).map(|_| c.base);
        let choices = self.creator_choices(&c);
        let background = self.creator_background(&c).map(|b| b.id.clone()).unwrap_or_default();
        let class = self.creator_class(&c).map(|cl| cl.id.clone()).unwrap_or_default();
        let class_skills = self.creator_class_skills(&c);
        let level_picks = self.creator_level_picks(&c);
        // The store: what was bought and the gold left, instead of the class's equipment.
        let store = (!self.creator_store_all(&c).is_empty()).then(|| {
            let (gold, spent) = self.creator_purse(&c);
            (c.cart.iter().map(|(id, n)| (id.clone(), *n)).collect(), gold.saturating_sub(spent))
        });
        let name = c.name.trim().to_string();
        self.new_game();
        self.world.player_name = if name.is_empty() { self.default_player_name() } else { name };
        self.world.player_sex = c.sex;
        self.world.player_appearance = Some(appearance);
        self.begin_as_hero(super::races::HeroChoices { race, base, choices, background, class, class_skills, level_picks, store });
        self.close_title();
    }

    /// The panel.
    fn creator_panel() -> Rect {
        let (sw, sh) = (screen_width(), screen_height());
        Rect::new(((sw - W) / 2.0).round().max(0.0), ((sh - H) / 2.0).round().max(0.0), W, H)
    }

    /// How many list rows (spells, the store) show at once.
    fn creator_list_shown(page: Page) -> usize {
        let n = ((H - 110.0 - 90.0) / LIST_H).floor() as usize;
        // (The store's filter tabs take a row.)
        if page == Page::Store { n - 1 } else { n }
    }

    /// The control rectangles of the current page's choice rows (rows before Start / Back).
    fn creator_layout(&self, c: &Creator, rows: &[Row]) -> Layout {
        let p = Self::creator_panel();
        let choice_rows: Vec<Row> = rows.iter().copied().filter(|r| !matches!(r, Row::Start | Row::Back)).collect();
        let n = choice_rows.len();
        let top = p.y + 80.0;
        let rects = match c.page {
            Page::Who => {
                // Rows close up a little when there are many (a race row on top of all the looks).
                let row_h = ROW_H.min((H - 80.0 - 90.0) / n.max(1) as f32).floor();
                (0..n).map(|k| Rect::new(p.x + 170.0, top + k as f32 * row_h, 300.0, 38.0)).collect()
            }
            // The ability rows in the left column (labels left of them), the race's choices in the
            // right one (labels inside).
            Page::Abilities => {
                let (mut left, mut right) = (0.0, 0.0);
                choice_rows
                    .iter()
                    .map(|r| {
                        if matches!(r, Row::Method | Row::Ability(_) | Row::Reroll) {
                            let y = top + left * ROW2_H;
                            left += 1.0;
                            Rect::new(p.x + 120.0, y, 290.0, 36.0)
                        } else {
                            let y = top + right * ROW2_H;
                            right += 1.0;
                            Rect::new(p.x + 430.0, y, 400.0, 36.0)
                        }
                    })
                    .collect()
            }
            // The rows on the right, what they are on the left.
            Page::Class | Page::Background => {
                let row_h = ROW2_H.min((H - 80.0 - 200.0) / n.max(1) as f32).floor();
                (0..n).map(|k| Rect::new(p.x + 430.0, top + k as f32 * row_h, 400.0, 36.0)).collect()
            }
            // A scrolling list on the left (under a line of counts).
            Page::Spells | Page::Store => {
                let shown = Self::creator_list_shown(c.page);
                let (w, first) = if c.page == Page::Spells { (400.0, top + 30.0) } else { (470.0, top + 30.0 + LIST_H) };
                (0..n)
                    .map(|k| match k.checked_sub(c.scroll).filter(|v| *v < shown) {
                        Some(v) => Rect::new(p.x + 30.0, first + v as f32 * LIST_H, w, LIST_H - 2.0),
                        None => Rect::default(),
                    })
                    .collect()
            }
        };
        Layout {
            panel: p,
            rows: rects,
            preview: Rect::new(p.x + p.w - 300.0, top, 260.0, 300.0),
            start: Rect::new(p.x + 30.0, p.y + H - 70.0, 260.0, 42.0),
            back: Rect::new(p.x + 302.0, p.y + H - 70.0, 140.0, 42.0),
        }
    }

    /// Keeps the selected list row in view.
    fn creator_keep_in_view(&mut self, rows: &[Row]) {
        let Some(page) = self.creator.as_ref().map(|c| c.page) else { return };
        let shown = Self::creator_list_shown(page);
        let Some(c) = &mut self.creator else { return };
        let n = rows.iter().filter(|r| matches!(r, Row::Spell(_) | Row::Item(_))).count();
        if let Some(Row::Spell(k) | Row::Item(k)) = rows.get(c.row) {
            if *k < c.scroll {
                c.scroll = *k;
            } else if *k >= c.scroll + shown {
                c.scroll = k + 1 - shown;
            }
        }
        c.scroll = c.scroll.min(n.saturating_sub(shown));
    }

    /// Handles the open creator. Returns false when it isn't open.
    pub(super) fn update_creator(&mut self, dt: f32) -> bool {
        let Some(c) = &mut self.creator else { return false };
        c.time += dt;
        if let Some((_, left)) = &mut c.message {
            *left -= dt;
            if *left <= 0.0 {
                c.message = None;
            }
        }
        while let Some(ch) = input::next_char() {
            if c.page == Page::Who && !ch.is_control() && c.name.chars().count() < MAX_NAME {
                c.name.push(ch);
            }
        }
        if c.page == Page::Who && input::pressed(KeyCode::Backspace) {
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
        // The store's filter: Tab / Shift+Tab (LB / RB) step through its tabs, 1-6 pick one.
        if c.page == Page::Store {
            let tabs = self.creator_store_tabs(c);
            if !tabs.is_empty() {
                let at = tabs.iter().position(|t| *t == c.store_tab).unwrap_or(0);
                let pick = if input::act_pressed(Act::NextTab) {
                    Some((at + 1) % tabs.len())
                } else if input::act_pressed(Act::PrevTab) {
                    Some((at + tabs.len() - 1) % tabs.len())
                } else {
                    Act::QUICK.iter().position(|a| input::act_pressed(*a)).filter(|k| *k < tabs.len())
                };
                if let Some(k) = pick {
                    self.set_creator_store_tab(tabs[k]);
                    return true;
                }
            }
        }
        if let Some(c) = &mut self.creator {
            c.row = row;
            // The wheel scrolls a list, the selected row along when it would leave the view (on
            // Start / Back the list just scrolls).
            let wheel = input::wheel();
            let lists = rows.iter().filter(|r| matches!(r, Row::Spell(_) | Row::Item(_))).count();
            let shown = Self::creator_list_shown(c.page);
            if wheel != 0.0 && lists > 0 {
                match rows.get(c.row) {
                    Some(Row::Spell(k) | Row::Item(k)) => {
                        let mut sel = *k;
                        super::ui::wheel_list(&mut sel, &mut c.scroll, lists, shown, wheel);
                        c.row = rows.iter().position(|r| matches!(r, Row::Spell(x) | Row::Item(x) if *x == sel)).unwrap_or(c.row);
                    }
                    _ => {
                        let mut none = c.scroll;
                        super::ui::wheel_list(&mut none, &mut c.scroll, lists, shown, wheel);
                    }
                }
            }
            self.creator_keep_in_view(&rows);
        }
        match rows[row] {
            Row::Start | Row::Back if left || right => {
                let other = rows.iter().position(|r| *r == if rows[row] == Row::Start { Row::Back } else { Row::Start }).unwrap_or(row);
                self.creator.as_mut().unwrap().row = other;
            }
            Row::Spell(k) if left || right => {
                let on = self.creator.as_ref().is_some_and(|c| self.creator_spell_list(c).get(k).is_some_and(|s| c.spells.contains(&s.0)));
                if on != right {
                    self.toggle_spell(k);
                }
            }
            Row::Item(k) if left || right => self.change_cart(k, right),
            r if left => self.change_row(r, -1),
            r if right => self.change_row(r, 1),
            _ => {}
        }
        if input::pressed(KeyCode::Enter) || input::pad_pressed(Pad::South) {
            match rows.get(row) {
                Some(Row::Back) => self.creator_backward(),
                Some(Row::Reroll) => self.change_row(Row::Reroll, 1),
                Some(Row::Spell(k)) => self.toggle_spell(*k),
                Some(Row::Item(k)) => self.change_cart(*k, true),
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
            let tab_hit = if c.page == Page::Store { self.creator_store_tab_rects(c).into_iter().find(|(_, r)| r.contains(m)).map(|(t, _)| t) } else { None };
            for (k, r) in l.rows.iter().enumerate() {
                if r.w <= 0.0 || !r.contains(m) {
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
                            c.parts.clear();
                        }
                    }
                    Row::Reroll => self.change_row(Row::Reroll, 1),
                    Row::Spell(k) => self.toggle_spell(k),
                    Row::Item(k) => {
                        let (minus, plus) = Self::cart_buttons(*r);
                        if plus.contains(m) {
                            self.change_cart(k, true);
                        } else if minus.contains(m) {
                            self.change_cart(k, false);
                        }
                    }
                    other => {
                        let (prev, _) = arrows(*r);
                        self.change_row(other, if prev.contains(m) { -1 } else { 1 });
                    }
                }
            }
            if let Some(t) = tab_hit {
                self.set_creator_store_tab(t);
                return true;
            }
            if l.start.contains(m) {
                self.creator_forward();
            } else if l.back.contains(m) {
                self.creator_backward();
            }
        }
        true
    }

    /// The "-" and "+" buttons of a store row.
    fn cart_buttons(r: Rect) -> (Rect, Rect) {
        (Rect::new(r.right() - 62.0, r.y + 2.0, 28.0, r.h - 4.0), Rect::new(r.right() - 30.0, r.y + 2.0, 28.0, r.h - 4.0))
    }

    /// Creator state for automated play-throughs: `name|race|sex|look rows …|pN PAGE METHOD
    /// base … total … picks[…] bg:… class:CLASS(skills)[level picks] spells[…] gold:LEFT/GOLD
    /// cart[item:n …]`.
    pub(super) fn creator_status(&self) -> Option<String> {
        let c = self.creator.as_ref()?;
        let a = self.chosen_appearance(c);
        let rows: Vec<String> = self.creator_rows(c).iter().map(|r| self.creator_label(c, *r)).filter(|l| !l.is_empty()).collect();
        let colors: Vec<String> = a.colors.iter().map(|(k, v)| format!("{k}:{:02x}{:02x}{:02x}", v[0], v[1], v[2])).collect();
        let parts: Vec<String> = a.parts.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        let look = format!("{} colors{{{}}} parts{{{}}} rows {}", a.model, colors.join(" "), parts.join(" "), rows.join("/"));
        let race = self.creator_race(c).map_or("-".to_string(), |r| r.id.clone());
        let hero = self.creator_hero(c);
        let picks: Vec<String> = self.creator_choices(c).iter().map(|(k, v)| format!("{k}={}", v.join("+"))).collect();
        let level: Vec<String> = self.creator_level_picks(c).iter().filter(|(k, _)| *k != "cantrips" && *k != "spells").map(|(k, v)| format!("{k}={}", v.join("+"))).collect();
        let class = self.creator_class(c).map_or("-".to_string(), |cl| format!("{}({})[{}]", cl.id, self.creator_class_skills(c).join("+"), level.join(";")));
        let (gold, spent) = self.creator_purse(c);
        let cart: Vec<String> = c.cart.iter().map(|(k, n)| format!("{k}:{n}")).collect();
        let pages = self.creator_pages(c);
        let at = pages.iter().position(|p| *p == c.page).unwrap_or(0);
        Some(format!(
            "{}|{}|{:?}|{}|p{}/{} {} {} base {} total {} picks[{}] bg:{} class:{} spells[{}] gold:{}/{} cart[{}] tab:{}/{}{}",
            c.name,
            race,
            c.sex,
            look,
            at + 1,
            pages.len(),
            c.page.id(),
            self.creator_method(c).map_or("-", |m| m.label()),
            c.base.text(),
            hero.abilities.text(),
            picks.join(";"),
            self.creator_background(c).map_or("-", |b| b.id.as_str()),
            class,
            c.spells.join(","),
            gold.saturating_sub(spent),
            gold,
            cart.join(" "),
            c.store_tab.id(),
            self.creator_store(c).len(),
            c.message.as_ref().map_or(String::new(), |(m, _)| format!(" msg:{m}"))
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
        let title = crate::lang::tr_ui(c.page.title());
        let size = fit_size(&title, 34.0, p.w - 260.0, 16.0);
        let tw = measure_plain(&title, size).width;
        draw_plain(&title, p.x + (p.w - tw) / 2.0, p.y + 50.0, size, skin::highlight());
        // Which step of how many.
        let pages = self.creator_pages(c);
        let at = pages.iter().position(|x| *x == c.page).unwrap_or(0);
        if pages.len() > 1 {
            let step = crate::lang::tr_ui("Step {} of {}").replacen("{}", &(at + 1).to_string(), 1).replacen("{}", &pages.len().to_string(), 1);
            let sw = measure_plain(&step, 16.0).width;
            draw_plain(&step, p.right() - skin::inset().max(20.0) - sw - 8.0, p.y + 46.0, 16.0, skin::dim());
        }
        match c.page {
            Page::Who => self.draw_creator_who(c, &choice_rows, &l, sel, m),
            Page::Class => self.draw_creator_class(c, &choice_rows, &l, sel, m),
            Page::Abilities => self.draw_creator_abilities(c, &choice_rows, &l, sel, m),
            Page::Background => self.draw_creator_background(c, &choice_rows, &l, sel, m),
            Page::Spells => self.draw_creator_spells(c, &choice_rows, &l, sel, m),
            Page::Store => self.draw_creator_store(c, &choice_rows, &l, sel, m),
        }
        let last = at + 1 >= pages.len();
        button(l.start, if last { "Start adventure" } else { "Next" }, sel == Some(Row::Start), false, l.start.contains(m));
        button(l.back, "Back", sel == Some(Row::Back), false, l.back.contains(m));
        if let Some((msg, _)) = &c.message {
            draw_plain(&fit_plain(msg, 18.0, p.right() - l.back.right() - 40.0), l.back.right() + 20.0, l.back.y + 27.0, 18.0, skin::text(skin::Text::Negative, Color::new(1.0, 0.5, 0.5, 1.0)));
        }
        let hint = match c.page {
            Page::Who => "Type a name  -  {key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  Enter: next  -  {key:cancel}: back",
            Page::Spells => "{key:up}/{key:down}: select  -  Enter / click: pick or drop  -  {key:cancel}: back",
            Page::Store => "{key:up}/{key:down}: select  -  {key:right} / Enter / +: buy one  -  {key:left} / -: put back  -  {key:next_tab} / 1-6: filter  -  {key:cancel}: back",
            _ if last => "{key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  Enter: start  -  {key:cancel}: back",
            _ => "{key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  Enter: next  -  {key:cancel}: back",
        };
        let x = p.x + skin::inset().max(20.0) + 10.0;
        super::glyphs::draw_hint_in(hint, x, skin::hint_baseline(p.bottom()), 16.0, p.right() - x - skin::inset().max(20.0), skin::disabled());
    }

    /// A row's "<" value ">" control (the value in the player's language).
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
        let size = fit_size(text, 22.0, avail, 14.0);
        let t = fit_plain(text, size, avail);
        draw_plain(&t, x, mid.y + mid.h / 2.0 + size / 3.0, size, skin::normal());
    }

    /// A row's label left of its control (in the player's language).
    fn draw_row_label(label: &str, x: f32, r: Rect, active: bool) {
        if label.is_empty() {
            return;
        }
        let room = r.x - x - 10.0;
        let size = fit_size(label, 22.0, room, 14.0);
        draw_plain(&fit_plain(label, size, room), x, r.y + 26.0, size, if active { skin::highlight() } else { skin::dim() });
    }

    fn draw_creator_who(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let cc = &self.project.config.creation;
        let race = self.creator_race(c);
        let features = self.creator_features(c);
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            Self::draw_row_label(&self.creator_label(c, *row), p.x + 30.0, r, active);
            match row {
                Row::Name => {
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, if active { 2.5 } else { 1.5 }, if active { skin::highlight() } else { skin::line(0.8) });
                    let caret = if active && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
                    draw_plain(&format!("{}{caret}", c.name), r.x + 10.0, r.y + 27.0, 26.0, skin::normal());
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
                            let label = self.project.db.model(&b).map_or_else(|| b.clone(), |m| m.label().to_string());
                            (label, None)
                        }
                        Row::Feature(f) => {
                            let (name, colors) = features.iter().find(|x| x.0 == *f).map(|x| (x.1.clone(), x.2.clone())).unwrap_or_default();
                            match c.colors.get(&name).copied().unwrap_or(0) {
                                0 => match race.as_ref().and_then(|r| r.colors.get(&name)) {
                                    Some(col) => (crate::lang::tr_ui("The race's").into_owned(), Some(*col)),
                                    None => (crate::lang::tr_ui("As drawn").into_owned(), self.creator_all_features(c).get(*f).and_then(|x| x.default)),
                                },
                                n => (format!("{}/{}", (n - 1) % colors.len().max(1) + 1, colors.len()), colors.get((n - 1) % colors.len().max(1)).copied()),
                            }
                        }
                        Row::Part(k) => {
                            let slot = crate::defs::CHOSEN_SLOTS[*k];
                            let (ids, own) = self.creator_part_options(&self.creator_body(c), slot);
                            (self.creator_part_label(&ids[c.parts.get(slot).copied().unwrap_or(own) % ids.len()]), None)
                        }
                        // A choice that colours the model: the pick's name ("Red") and its first colour.
                        Row::Choice(ci, _) => match race.as_ref().and_then(|r| r.choices.get(*ci)) {
                            Some(ch) => {
                                let pick = Self::picked(c, *ci, 1, ch.options.len()).first().copied().unwrap_or(0);
                                let id = ch.options.get(pick).cloned().unwrap_or_default();
                                let n = ch.option_name(&id);
                                let short = match (n.find('('), n.rfind(')')) {
                                    (Some(a), Some(b)) if b > a => n[a + 1..b].to_string(),
                                    _ => n,
                                };
                                let colors = Self::choice_colors(c, *ci, ch);
                                let swatch = colors.get("skin").or_else(|| colors.values().next()).copied();
                                (crate::lang::tr_ui(&short).into_owned(), swatch)
                            }
                            None => (String::new(), None),
                        },
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
                let r = Rect::new(pr.x + 8.0 + k as f32 * 62.0, pr.y + pr.h - 88.0, fw * 1.6, fh * 1.6);
                if let Some(cell) = self.model_cell(&a, &gear, (*f, super::paperdoll::Anim::Idle(0))) {
                    self.assets.models.draw_in(&cell, r, WHITE);
                }
            }
        }

        // The race under the preview: its increases, speed, size and senses, its traits.
        if let Some(r) = race.as_ref().filter(|_| self.creator_races().len() > 1) {
            let room = |y: f32, size: f32| (((l.start.y - 8.0) - y) / (size + 2.0)).max(0.0) as usize;
            let mut y = pr.y + pr.h + 22.0;
            let bonus = r.bonus_text();
            if !bonus.is_empty() {
                y = wrapped(&crate::lang::tr_ui(&bonus), pr.x, y, pr.w, 17.0, skin::highlight(), room(y, 17.0).min(2)) + 2.0;
            }
            let size = if r.size_category.is_empty() { "Medium".to_string() } else { r.size_category.clone() };
            let mut facts = format!("{} - {} ft", crate::lang::tr_ui(&size), r.walk_feet());
            if r.darkvision_ft > 0 {
                facts += &format!(" - {}", crate::lang::tr_ui(&format!("darkvision {} ft", r.darkvision_ft)));
            }
            y = wrapped(&facts, pr.x, y, pr.w, 18.0, skin::dim(), room(y, 18.0).min(1));
            let traits: Vec<&str> = r.traits.iter().filter(|t| !t.name.is_empty() && t.id != "darkvision").map(|t| t.name.as_str()).collect();
            let text = if traits.is_empty() { r.description.clone() } else { traits.join(", ") };
            wrapped(&text, pr.x, y, pr.w, 17.0, skin::dim(), room(y, 17.0).min(4));
        }
    }

    /// The class page: the class, its skills and level 1's choices on the right; what the class
    /// is on the left (hit die, saving throws, proficiencies, starting gold, level 1's features).
    fn draw_creator_class(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let Some(cl) = self.creator_class(c) else { return };
        let picks = Self::class_skills_picked(c, cl.skill_choices as usize, cl.skill_options.len());
        let level = self.creator_level_rows(c);
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            match row {
                Row::Class => Self::draw_choice_control(r, active, m, &format!("{}: {}", crate::lang::tr_ui("Class"), crate::lang::tr(&cl.name)), None),
                Row::ClassSkill(slot) => {
                    let skill = picks.get(*slot).and_then(|k| cl.skill_options.get(*k)).map_or(String::new(), |id| crate::lang::tr_ui(&abilities::proficiency_name(&abilities::proficiency_id(id))).into_owned());
                    Self::draw_choice_control(r, active, m, &format!("{}: {skill}", crate::lang::tr_ui("Skill")), None);
                }
                Row::Level(k) => self.draw_level_row(c, &level, *k, r, active, m),
                _ => {}
            }
        }
        let room = |y: f32, size: f32| (((l.start.y - 8.0) - y) / (size + 2.0)).max(0.0) as usize;
        // Under the rows: the selected level choice's option and what it does.
        if let Some(Row::Level(k)) = sel {
            let below = l.rows.iter().map(|r| r.bottom()).fold(p.y + 80.0, f32::max) + 24.0;
            self.draw_level_option(cl, &level, c, k, p.x + 430.0, below, 400.0, &room);
        }
        let (x, w) = (p.x + 30.0, 380.0);
        let mut y = p.y + 100.0;
        let saves: Vec<String> = cl.saves.iter().filter_map(|a| Ability::parse(a)).map(|a| a.short().to_string()).collect();
        y = wrapped(&format!("{} d{}   {} {}", crate::lang::tr_ui("Hit die"), cl.hit_die, crate::lang::tr_ui("Saves"), saves.join(", ")), x, y, w, 18.0, skin::highlight(), 1) + 4.0;
        let order: Vec<&str> = ability_order(Some(cl)).iter().take(3).map(|a| a.short()).collect();
        y = wrapped(&format!("{} {}", crate::lang::tr_ui("Best abilities:"), order.join(", ")), x, y, w, 16.0, skin::dim(), 1) + 2.0;
        if !self.creator_store_all(c).is_empty() {
            y = wrapped(&format!("{} {}", crate::lang::tr_ui("Starting gold:"), Self::format_cp(cl.starting_gold_cp())), x, y, w, 16.0, skin::dim(), 1) + 2.0;
        }
        let profs: Vec<String> = cl.proficiencies.iter().map(|id| crate::lang::tr_ui(&abilities::proficiency_name(id)).into_owned()).collect();
        if !profs.is_empty() {
            y = wrapped(&profs.join(", "), x, y + 2.0, w, 16.0, skin::dim(), room(y, 16.0).min(3)) + 8.0;
        }
        for id in cl.levels.first().map(|l| l.features.clone()).unwrap_or_default() {
            let Some(f) = cl.feature(&id) else { continue };
            if room(y, 17.0) < 2 {
                break;
            }
            y = wrapped(&crate::lang::tr(&f.name), x, y, w, 17.0, skin::normal(), 1);
            y = wrapped(&crate::lang::tr(&f.text), x, y, w, 15.0, skin::dim(), room(y, 15.0).min(3)) + 6.0;
        }
    }

    /// A level choice row: "Label: Option".
    fn draw_level_row(&self, c: &Creator, level: &[ChoiceRow], k: usize, r: Rect, active: bool, m: Vec2) {
        let Some(row) = level.get(k) else { return };
        let pick = Self::level_pick(c, level, k);
        let label = super::levelup::tr_any(&row.label);
        let value = row.options.get(pick).map_or(String::new(), |o| super::levelup::tr_any(&o.1));
        // ("Fighting Style: Archery" under the label "Fighting Style": just "Archery".)
        let value = value.strip_prefix(&format!("{label}: ")).map_or(value.clone(), str::to_string);
        Self::draw_choice_control(r, active, m, &format!("{label}: {value}"), None);
    }

    /// The picked option of level row `k` and its text (a fighting style, a domain).
    #[allow(clippy::too_many_arguments)]
    fn draw_level_option(&self, cl: &ClassDef, level: &[ChoiceRow], c: &Creator, k: usize, x: f32, y: f32, w: f32, room: &dyn Fn(f32, f32) -> usize) {
        let Some(row) = level.get(k) else { return };
        let Some((id, name)) = row.options.get(Self::level_pick(c, level, k)) else { return };
        let text = cl.feature(id).map(|f| f.text.clone()).or_else(|| cl.subclass(id).map(|s| s.description.clone())).unwrap_or_default();
        let y = wrapped(&super::levelup::tr_any(name), x, y, w, 17.0, skin::highlight(), room(y, 17.0).min(1));
        if !text.is_empty() {
            wrapped(&crate::lang::tr(&text), x, y, w, 15.0, skin::dim(), room(y, 15.0).min(8));
        }
    }

    fn draw_creator_abilities(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let race = self.creator_race(c);
        let hero = self.creator_hero(c);
        let method = self.creator_method(c);
        let choices = self.creator_choices(c);
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            Self::draw_row_label(&self.creator_label(c, *row), p.x + 30.0, r, active);
            match row {
                Row::Ability(i) => {
                    let a = Ability::ALL[*i];
                    Self::draw_choice_control(Rect::new(r.x, r.y, 128.0, r.h), active, m, &c.base.score(a).to_string(), None);
                    // The racial bonus, the total and its modifier in columns of their own.
                    let total = hero.abilities.score(a);
                    let extra = total - c.base.score(a);
                    if extra != 0 {
                        draw_plain(&abilities::signed(extra as i64), r.x + 136.0, r.y + 25.0, 18.0, skin::highlight());
                    }
                    draw_plain(&total.to_string(), r.x + 180.0, r.y + 25.0, 22.0, skin::normal());
                    draw_plain(&format!("({})", abilities::signed(abilities::modifier(total) as i64)), r.x + 216.0, r.y + 24.0, 16.0, skin::dim());
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
                _ => {}
            }
        }
        let room = |y: f32, size: f32| (((l.start.y - 8.0) - y) / (size + 2.0)).max(0.0) as usize;
        // Under the left column: where the class wants its best scores.
        let left_bottom = choice_rows.iter().zip(&l.rows).filter(|(r, _)| matches!(r, Row::Method | Row::Ability(_) | Row::Reroll)).map(|(_, r)| r.bottom()).fold(p.y + 80.0, f32::max);
        if let Some(cl) = self.creator_class(c) {
            let order: Vec<&str> = ability_order(Some(cl)).iter().take(3).map(|a| a.short()).collect();
            let line = crate::lang::tr_ui("Placed for the {}: {} first").replacen("{}", &crate::lang::tr(&cl.name), 1).replacen("{}", &order.join(", "), 1);
            wrapped(&line, p.x + 30.0, left_bottom + 24.0, 370.0, 16.0, skin::dim(), room(left_bottom + 24.0, 16.0).min(2));
        }
        // Under the right column: the selected racial choice's trait.
        let right_bottom = choice_rows.iter().zip(&l.rows).filter(|(r, _)| matches!(r, Row::Choice(..))).map(|(_, r)| r.bottom()).fold(p.y + 80.0, f32::max);
        let (x, w) = (p.x + 430.0, 400.0);
        let mut y = right_bottom + 24.0;
        if let Some(Row::Choice(ci, _)) = sel {
            if let Some(ch) = race.as_ref().and_then(|r| r.choices.get(ci)) {
                y = wrapped(&ch.label, x, y, w, 17.0, skin::highlight(), room(y, 17.0).min(1));
                if let Some(t) = race.as_ref().and_then(|r| r.traits.iter().find(|t| t.id == ch.id)) {
                    y = wrapped(&t.description, x, y, w, 15.0, skin::dim(), room(y, 15.0).min(5)) + 6.0;
                }
            }
        }
        let langs: Vec<String> = hero.languages.iter().map(|id| crate::backgrounds::language_label(id)).collect();
        if !langs.is_empty() {
            wrapped(&format!("{} {}", crate::lang::tr_ui("Languages:"), langs.join(", ")), x, y + 4.0, w, 16.0, skin::dim(), room(y + 4.0, 16.0).min(2));
        }
    }

    /// The background page: the background, its languages and expertise on the right; what it
    /// gives on the left.
    fn draw_creator_background(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let race = self.creator_race(c);
        let hero = self.creator_hero(c);
        let bg = self.creator_background(c);
        let bg_opts = Self::bg_language_options(c, race.as_ref());
        let level = self.creator_level_rows(c);
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            match row {
                Row::Background => {
                    let name = bg.map_or_else(|| crate::lang::tr_ui("None").into_owned(), |b| b.name.clone());
                    Self::draw_choice_control(r, active, m, &format!("{}: {name}", crate::lang::tr_ui("Background")), None);
                }
                Row::BgLanguage(slot) => {
                    let picks = Self::bg_picked(c, bg.map_or(0, |b| b.language_choices as usize), bg_opts.len());
                    let lang = picks.get(*slot).and_then(|k| bg_opts.get(*k)).map_or(String::new(), |id| crate::backgrounds::language_label(id));
                    Self::draw_choice_control(r, active, m, &format!("{}: {lang}", crate::lang::tr_ui("Language")), None);
                }
                Row::Level(k) => self.draw_level_row(c, &level, *k, r, active, m),
                _ => {}
            }
        }
        let room = |y: f32, size: f32| (((l.start.y - 8.0) - y) / (size + 2.0)).max(0.0) as usize;
        let (x, w) = (p.x + 30.0, 380.0);
        let mut y = p.y + 100.0;
        if let Some(b) = bg {
            if !b.description.is_empty() {
                y = wrapped(&b.description, x, y, w, 16.0, skin::dim(), room(y, 16.0).min(4)) + 6.0;
            }
            let profs: Vec<String> = b.proficiencies.iter().map(|id| crate::lang::tr_ui(&abilities::proficiency_name(&abilities::proficiency_id(id))).into_owned()).collect();
            if !profs.is_empty() {
                y = wrapped(&profs.join(", "), x, y, w, 17.0, skin::normal(), room(y, 17.0).min(2)) + 4.0;
            }
            if !b.feature.is_empty() {
                y = wrapped(&format!("{}: {}", b.feature, b.feature_text), x, y, w, 16.0, skin::dim(), room(y, 16.0).min(5)) + 4.0;
            }
            let gear: Vec<String> = b.equipment.iter().filter(|s| !s.item.is_empty()).map(|s| if s.count > 1 { format!("{} x{}", self.item_name(&s.item), s.count) } else { self.item_name(&s.item) }).collect();
            if !gear.is_empty() {
                y = wrapped(&format!("{} {}", crate::lang::tr_ui("Equipment:"), gear.join(", ")), x, y, w, 16.0, skin::dim(), room(y, 16.0).min(3)) + 4.0;
            }
        }
        let langs: Vec<String> = hero.languages.iter().map(|id| crate::backgrounds::language_label(id)).collect();
        if !langs.is_empty() {
            wrapped(&format!("{} {}", crate::lang::tr_ui("Languages:"), langs.join(", ")), x, y + 2.0, w, 16.0, skin::dim(), room(y + 2.0, 16.0).min(2));
        }
        // Under the rows: the selected expertise and the hero's skills it can double.
        if let (Some(Row::Level(k)), Some(cl)) = (sel, self.creator_class(c)) {
            let below = l.rows.iter().map(|r| r.bottom()).fold(p.y + 80.0, f32::max) + 24.0;
            self.draw_level_option(cl, &level, c, k, p.x + 430.0, below, 400.0, &room);
        }
    }

    /// The spells page: the checklist on the left, the selected spell on the right.
    fn draw_creator_spells(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let list = self.creator_spell_list(c);
        let (nc, ns, _, _) = self.creator_spell_choices(c);
        let (pc, ps) = self.creator_spell_counts(c);
        let mut counts = Vec::new();
        if nc > 0 {
            counts.push(crate::lang::tr_ui("Cantrips: {} of {}").replacen("{}", &pc.to_string(), 1).replacen("{}", &nc.to_string(), 1));
        }
        if ns > 0 {
            let what = if self.creator_class(c).is_some_and(|cl| cl.id == "wizard") { "Spellbook: {} of {}" } else { "Spells: {} of {}" };
            counts.push(crate::lang::tr_ui(what).replacen("{}", &ps.to_string(), 1).replacen("{}", &ns.to_string(), 1));
        }
        draw_plain(&fit_plain(&counts.join("   "), 18.0, 400.0), p.x + 30.0, p.y + 100.0, 18.0, skin::highlight());
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let Row::Spell(i) = row else { continue };
            if r.w <= 0.0 {
                continue;
            }
            let Some((id, cantrip)) = list.get(*i) else { continue };
            let active = sel == Some(*row);
            let on = c.spells.contains(id);
            if active {
                skin::cursor(r, Color::new(1.0, 0.86, 0.45, 1.0));
            } else if r.contains(m) {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, 0.08));
            }
            // The check box, the name and its level.
            let b = Rect::new(r.x + 6.0, r.y + 6.0, r.h - 12.0, r.h - 12.0);
            draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.5, skin::line(0.9));
            if on {
                draw_rectangle(b.x + 3.0, b.y + 3.0, b.w - 6.0, b.h - 6.0, skin::highlight());
            }
            let d = self.project.db.spell(id);
            let name = d.map_or(id.clone(), |d| d.name.clone());
            let tail = if *cantrip { crate::lang::tr_ui("cantrip").into_owned() } else { crate::lang::tr_ui("level {}").replacen("{}", &d.map_or(1, |d| d.level).to_string(), 1) };
            let tw = measure_plain(&tail, 15.0).width;
            draw_plain(&fit_plain(&name, 19.0, r.w - b.w - tw - 30.0), b.right() + 10.0, r.y + 20.0, 19.0, if on { skin::highlight() } else { skin::normal() });
            draw_plain(&tail, r.right() - tw - 8.0, r.y + 19.0, 15.0, skin::dim());
        }
        // A list longer than the page: which rows show.
        let shown = Self::creator_list_shown(c.page);
        if list.len() > shown {
            let t = crate::lang::tr_ui("{}-{} of {}").replacen("{}", &(c.scroll + 1).to_string(), 1).replacen("{}", &(c.scroll + shown).min(list.len()).to_string(), 1).replacen("{}", &list.len().to_string(), 1);
            let tw = measure_plain(&t, 15.0).width;
            draw_plain(&t, p.x + 430.0 - tw, p.y + 100.0, 15.0, skin::disabled());
        }
        // The selected spell.
        let room = |y: f32, size: f32| (((l.start.y - 8.0) - y) / (size + 2.0)).max(0.0) as usize;
        let Some(Row::Spell(i)) = sel else { return };
        let Some(d) = list.get(i).and_then(|(id, _)| self.project.db.spell(id)) else { return };
        let (x, w) = (p.x + 460.0, 370.0);
        let mut y = p.y + 110.0;
        y = wrapped(&d.name, x, y, w, 22.0, skin::highlight(), 1) + 2.0;
        let kind = if d.level == 0 { crate::lang::tr_ui("Cantrip").into_owned() } else { crate::lang::tr_ui("Level {}").replacen("{}", &d.level.to_string(), 1) };
        let school = if d.school.is_empty() { String::new() } else { format!(" - {}", crate::lang::tr_ui(&d.school)) };
        y = wrapped(&format!("{kind}{school}"), x, y, w, 16.0, skin::normal(), 1) + 2.0;
        let mut facts = vec![crate::lang::tr_ui(&d.casting.label()).into_owned(), match d.range {
            crate::skills::SpellRange::Feet(f) => format!("{f} ft"),
            r => crate::lang::tr_ui(&r.label()).into_owned(),
        }];
        if let Some(a) = d.area {
            facts.push(a.label());
        }
        if d.concentration {
            facts.push(crate::lang::tr_ui("concentration").into_owned());
        }
        y = wrapped(&facts.join(" - "), x, y, w, 16.0, skin::dim(), room(y, 16.0).min(2)) + 6.0;
        wrapped(&d.description, x, y, w, 15.0, skin::dim(), room(y, 15.0).min(14));
    }

    /// The store page: the stock on the left (price, how many bought, - / +), the purse, the
    /// cart and the selected item on the right.
    fn draw_creator_store(&self, c: &Creator, choice_rows: &[Row], l: &Layout, sel: Option<Row>, m: Vec2) {
        let p = l.panel;
        let stock = self.creator_store(c);
        let (gold, spent) = self.creator_purse(c);
        let profs = self.creator_profs(c);
        let has = |p: &str| profs.iter().any(|x| x == p);
        let proficient = |d: &crate::defs::ItemDef| {
            let weapon = d.weapon.as_ref().is_none_or(|w| crate::srd::equipment::weapon_proficient(has, w, &d.srd));
            let armor = d.armor.as_ref().and_then(|a| a.category).is_none_or(|cat| crate::srd::equipment::armor_proficient(has, cat));
            weapon && armor
        };
        let line = crate::lang::tr_ui("Gold left: {} of {}").replacen("{}", &Self::format_cp(gold.saturating_sub(spent)), 1).replacen("{}", &Self::format_cp(gold), 1);
        draw_plain(&fit_plain(&line, 18.0, 470.0), p.x + 30.0, p.y + 100.0, 18.0, skin::highlight());
        super::shop_ui::draw_tabs(&self.creator_store_tab_rects(c), c.store_tab, m);
        if stock.is_empty() {
            draw_text("Nothing here.", p.x + 42.0, p.y + 170.0, 20.0, skin::disabled());
        }
        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let Row::Item(i) = row else { continue };
            if r.w <= 0.0 {
                continue;
            }
            let Some((id, price)) = stock.get(*i) else { continue };
            let Some(d) = self.project.db.item(id) else { continue };
            let active = sel == Some(*row);
            if active {
                skin::cursor(r, Color::new(1.0, 0.86, 0.45, 1.0));
            } else if r.contains(m) {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, 0.08));
            }
            self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(r.x + 4.0, r.y + 2.0, r.h - 4.0, r.h - 4.0), WHITE);
            let (minus, plus) = Self::cart_buttons(r);
            let n = c.cart.get(id).copied().unwrap_or(0);
            let price_t = Self::format_cp(*price);
            let pw = measure_plain(&price_t, 16.0).width;
            let count_t = if n > 0 { format!("x{n}") } else { String::new() };
            let room = minus.x - r.x - 44.0 - pw - 52.0;
            let col = if !proficient(d) { skin::dim() } else if n > 0 { skin::highlight() } else { skin::normal() };
            draw_plain(&fit_plain(&d.name, 19.0, room), r.x + 36.0, r.y + 20.0, 19.0, col);
            draw_plain(&count_t, minus.x - 48.0 - pw - 8.0, r.y + 20.0, 17.0, skin::highlight());
            draw_plain(&price_t, minus.x - pw - 8.0, r.y + 20.0, 16.0, if spent + price > gold { skin::disabled() } else { skin::dim() });
            button(minus, "-", false, false, minus.contains(m));
            button(plus, "+", false, false, plus.contains(m));
        }
        let shown = Self::creator_list_shown(c.page);
        if stock.len() > shown {
            let t = crate::lang::tr_ui("{}-{} of {}").replacen("{}", &(c.scroll + 1).to_string(), 1).replacen("{}", &(c.scroll + shown).min(stock.len()).to_string(), 1).replacen("{}", &stock.len().to_string(), 1);
            let tw = measure_plain(&t, 15.0).width;
            draw_plain(&t, p.x + 500.0 - tw, p.y + 100.0, 15.0, skin::disabled());
        }
        // The right column: the selected item, then the cart.
        let room = |y: f32, size: f32| (((l.start.y - 8.0) - y) / (size + 2.0)).max(0.0) as usize;
        let (x, w) = (p.x + 530.0, 300.0);
        let mut y = p.y + 110.0;
        if let Some(d) = match sel {
            Some(Row::Item(i)) => stock.get(i).and_then(|(id, _)| self.project.db.item(id)),
            _ => None,
        } {
            y = wrapped(&d.name, x, y, w, 21.0, skin::highlight(), 1) + 2.0;
            if let Some(stats) = super::ui::item_stats_line(d) {
                y = wrapped(&stats, x, y, w, 16.0, skin::normal(), room(y, 16.0).min(2));
            }
            if d.weight > 0.0 {
                y = wrapped(&crate::lang::tr_ui("{} lb").replacen("{}", &format!("{}", d.weight), 1), x, y, w, 15.0, skin::dim(), 1);
            }
            if !proficient(d) {
                y = wrapped(&crate::lang::tr_ui("Your class isn't trained with it."), x, y + 2.0, w, 16.0, skin::text(skin::Text::Negative, Color::new(1.0, 0.55, 0.45, 1.0)), room(y + 2.0, 16.0).min(2));
            }
            y = wrapped(&d.description, x, y + 4.0, w, 15.0, skin::dim(), room(y + 4.0, 15.0).min(5)) + 10.0;
        }
        if !c.cart.is_empty() && room(y, 17.0) > 1 {
            y = wrapped(&crate::lang::tr_ui("Bought:"), x, y, w, 17.0, skin::normal(), 1);
            let bought: Vec<String> = c.cart.iter().map(|(id, n)| if *n > 1 { format!("{} x{n}", self.item_name(id)) } else { self.item_name(id) }).collect();
            wrapped(&bought.join(", "), x, y, w, 15.0, skin::dim(), room(y, 15.0));
        }
    }
}
