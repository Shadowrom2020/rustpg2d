//! Character creator, shown after "New game" when the project enables it: the player types a
//! name, picks a race (when the project has several playable races), male / female, a figure (the
//! race's figures of that sex; without races the project's default figure for that sex and the
//! other playable figures of that sex) and a colour for each colour feature of the figure that
//! offers colours (skin, hair, clothing...: `defs::ColorFeature`).
//!
//! Keyboard: typing edits the name (Backspace deletes), Up/Down select a row, Left/Right
//! change it, Enter starts, Esc goes back to the title screen. Everything can be clicked.

use macroquad::prelude::*;

use super::input::{self, Act, Pad};
use super::{ui::panel, Game};
use crate::defs::Appearance;
use crate::state::Sex;
use super::text::{draw_text, measure_text};

const W: f32 = 760.0;
const H: f32 = 560.0;
const MAX_NAME: usize = 16;
const ROW_H: f32 = 54.0;

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
    /// Chosen colour per feature of the figure: 0 = as drawn, k = the feature's colour k - 1.
    colors: Vec<usize>,
    row: usize,
    time: f32,
}

struct Layout {
    panel: Rect,
    /// Control area of each row (rows before Start / Back).
    rows: Vec<Rect>,
    preview: Rect,
    start: Rect,
    back: Rect,
}

fn layout(n_rows: usize) -> Layout {
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

/// The "<" and ">" buttons of a choice row.
fn arrows(r: Rect) -> (Rect, Rect) {
    (Rect::new(r.x, r.y, 38.0, r.h), Rect::new(r.x + r.w - 38.0, r.y, 38.0, r.h))
}

fn button(r: Rect, label: &str, selected: bool, active: bool, hover: bool) {
    let fill = if active {
        Color::new(0.45, 0.35, 0.12, 1.0)
    } else if hover {
        Color::new(0.3, 0.26, 0.14, 1.0)
    } else {
        Color::new(0.12, 0.12, 0.2, 0.9)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, fill);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 } else { 1.5 }, if selected { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.8) });
    let tw = measure_text(label, None, 24, 1.0).width;
    draw_text(label, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + 8.0, 24.0, WHITE);
}

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
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

    /// Races the player may choose (playable ones; none = the project has no race choice).
    fn creator_races(&self) -> Vec<&crate::races::RaceDef> {
        self.project.db.races.iter().filter(|r| r.playable).collect()
    }

    /// The chosen race (None without playable races).
    fn creator_race(&self, c: &Creator) -> Option<&crate::races::RaceDef> {
        let races = self.creator_races();
        (!races.is_empty()).then(|| races[c.race % races.len()])
    }

    /// Bodies a player of `sex` may choose: the race's (that exist), else the project's default
    /// body for that sex first, then the other playable bodies of that sex.
    fn creator_bodies(&self, race: Option<&crate::races::RaceDef>, sex: Sex) -> Vec<String> {
        if let Some(r) = race {
            let v: Vec<String> = r.bodies(sex).iter().filter(|b| self.assets.sheets.contains_key(*b)).cloned().collect();
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
        for s in self.project.db.playable_bodies(sex == Sex::Female) {
            if !v.contains(&s.id) {
                v.push(s.id.clone());
            }
        }
        v.retain(|b| self.assets.sheets.contains_key(b));
        v
    }

    /// The chosen figure (`""` when none can be chosen: the player character's own).
    fn creator_body(&self, c: &Creator) -> String {
        let bodies = self.creator_bodies(self.creator_race(c), c.sex);
        if bodies.is_empty() {
            self.project.db.character(&self.project.config.player).map(|d| d.appearance.body.clone()).unwrap_or_default()
        } else {
            bodies[c.body[c.sex as usize] % bodies.len()].clone()
        }
    }

    /// The chosen figure's colour features that offer colours: (feature index, name, colours,
    /// label).
    fn creator_features(&self, c: &Creator) -> Vec<(usize, String, Vec<[u8; 3]>, String)> {
        let body = self.creator_body(c);
        let Some(sheet) = self.project.db.sheet(&body) else { return Vec::new() };
        sheet.features.iter().enumerate().filter(|(_, f)| !f.colors.is_empty()).map(|(k, f)| (k, f.name.clone(), f.colors.clone(), f.label())).collect()
    }

    /// The label of a row (feature rows: the feature's name).
    fn creator_label(&self, c: &Creator, row: Row) -> String {
        match row {
            Row::Name => "Name".into(),
            Row::Race => "Race".into(),
            Row::Sex => "Sex".into(),
            Row::Body => "Body".into(),
            Row::Feature(k) => self.creator_features(c).into_iter().find(|f| f.0 == k).map_or(String::new(), |f| crate::lang::tr(&f.3)),
            Row::Start | Row::Back => String::new(),
        }
    }

    /// Choice rows the project offers (Race and Body only with a choice; a row per colour feature
    /// that offers colours).
    fn creator_rows(&self, c: &Creator) -> Vec<Row> {
        let mut rows = vec![Row::Name];
        if self.creator_races().len() > 1 {
            rows.push(Row::Race);
        }
        rows.push(Row::Sex);
        if self.creator_bodies(self.creator_race(c), c.sex).len() > 1 {
            rows.push(Row::Body);
        }
        rows.extend(self.creator_features(c).into_iter().map(|f| Row::Feature(f.0)));
        rows.extend([Row::Start, Row::Back]);
        rows
    }

    /// The player definition's look with the creator's choices.
    fn chosen_appearance(&self, c: &Creator) -> Appearance {
        let mut a = self.project.db.character(&self.project.config.player).map(|d| d.appearance.clone()).unwrap_or_default();
        let body = self.creator_body(c);
        if body != a.body {
            // Another figure: the player character's colours were chosen for its own.
            a.colors.clear();
            a.body = body;
        }
        for (k, name, colors, _) in self.creator_features(c) {
            match c.colors.get(k).copied().unwrap_or(0) {
                0 => {}
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
        self.creator = Some(Creator { name: self.default_player_name(), race, sex, body: [0, 0], colors: Vec::new(), row: 0, time: 0.0 });
    }

    /// Steps the choice of a row.
    fn change_row(&mut self, row: Row, step: i32) {
        if self.creator.is_none() {
            return;
        }
        let c = self.creator.as_ref().unwrap();
        let n = match row {
            Row::Race => self.creator_races().len(),
            Row::Body => self.creator_bodies(self.creator_race(c), c.sex).len(),
            // "As drawn" and the feature's colours.
            Row::Feature(k) => self.creator_features(c).into_iter().find(|f| f.0 == k).map_or(0, |f| f.2.len() + 1),
            _ => 0,
        }
        .max(1) as i32;
        let fixed = self.project.config.creation.sex.fixed().is_some();
        let c = self.creator.as_mut().unwrap();
        let cycle = |v: &mut usize| *v = (*v as i32 + step).rem_euclid(n) as usize;
        match row {
            Row::Race => {
                cycle(&mut c.race);
                // Another race has other figures and colours.
                c.body = [0, 0];
                c.colors.clear();
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
            _ => {}
        }
    }

    fn start_created(&mut self) {
        let Some(c) = self.creator.take() else { return };
        let appearance = self.chosen_appearance(&c);
        let race = self.creator_race(&c).map(|r| r.id.clone());
        let name = c.name.trim().to_string();
        self.new_game();
        self.world.player_name = if name.is_empty() { self.default_player_name() } else { name };
        self.world.player_sex = c.sex;
        self.world.player_appearance = Some(appearance);
        match race {
            Some(r) => self.begin_as_race(&r),
            None => self.refresh_player_size(),
        }
        self.close_title();
    }

    /// Handles the open creator. Returns false when it isn't open.
    pub(super) fn update_creator(&mut self, dt: f32) -> bool {
        let Some(c) = &mut self.creator else { return false };
        c.time += dt;
        while let Some(ch) = input::next_char() {
            if !ch.is_control() && c.name.chars().count() < MAX_NAME {
                c.name.push(ch);
            }
        }
        if input::pressed(KeyCode::Backspace) {
            c.name.pop();
        }
        // The name field takes typed text: arrows and Enter / Escape (or the gamepad) navigate.
        if input::act_pressed(Act::Cancel) {
            self.creator = None;
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
            let row = self.creator.as_ref().map_or(0, |c| c.row);
            if rows.get(row) == Some(&Row::Back) {
                self.creator = None;
            } else {
                self.start_created();
            }
            return true;
        }
        // Mouse.
        if input::mouse_pressed(MouseButton::Left) {
            let choice_rows: Vec<Row> = rows.iter().copied().filter(|r| !matches!(r, Row::Start | Row::Back)).collect();
            let l = layout(choice_rows.len());
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
                    other => {
                        let (prev, _) = arrows(*r);
                        self.change_row(other, if prev.contains(m) { -1 } else { 1 });
                    }
                }
            }
            if l.start.contains(m) {
                self.start_created();
            } else if l.back.contains(m) {
                self.creator = None;
            }
        }
        true
    }

    /// Creator state for automated play-throughs.
    pub(super) fn creator_status(&self) -> Option<String> {
        let c = self.creator.as_ref()?;
        let a = self.chosen_appearance(c);
        let rows: Vec<String> = self.creator_rows(c).iter().map(|r| self.creator_label(c, *r)).filter(|l| !l.is_empty()).collect();
        let colors: Vec<String> = a.colors.iter().map(|(k, v)| format!("{k}:{:02x}{:02x}{:02x}", v[0], v[1], v[2])).collect();
        let look = format!("{} colors{{{}}} rows {}", a.body, colors.join(" "), rows.join("/"));
        let race = self.creator_race(c).map_or("-".to_string(), |r| r.id.clone());
        Some(format!("{}|{}|{:?}|{}", c.name, race, c.sex, look))
    }

    pub(super) fn draw_creator(&self) {
        let Some(c) = &self.creator else { return };
        let rows = self.creator_rows(c);
        let choice_rows: Vec<Row> = rows.iter().copied().filter(|r| !matches!(r, Row::Start | Row::Back)).collect();
        let l = layout(choice_rows.len());
        let m = input::mouse_pos();
        let p = l.panel;
        let sel = rows.get(c.row).copied();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.6));
        panel(p.x, p.y, p.w, p.h);
        let title = "Create your character";
        let tw = measure_text(title, None, 34, 1.0).width;
        draw_text(title, p.x + (p.w - tw) / 2.0, p.y + 46.0, 34.0, GOLD);
        let cc = &self.project.config.creation;
        let race = self.creator_race(c);
        let features = self.creator_features(c);

        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            draw_text(self.creator_label(c, *row), p.x + 30.0, r.y + 27.0, 24.0, if active { GOLD } else { LIGHTGRAY });
            match row {
                Row::Name => {
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, if active { 2.5 } else { 1.5 }, if active { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.8) });
                    let caret = if active && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
                    draw_text(format!("{}{caret}", c.name), r.x + 10.0, r.y + 27.0, 26.0, WHITE);
                }
                Row::Sex if cc.sex.fixed().is_some() => {
                    draw_text(c.sex.label(), r.x + 6.0, r.y + 26.0, 24.0, WHITE);
                }
                Row::Sex => {
                    for (k, s) in Sex::ALL.iter().enumerate() {
                        let b = Rect::new(r.x + k as f32 * (r.w / 2.0 + 4.0), r.y, r.w / 2.0 - 4.0, r.h);
                        button(b, s.label(), active && c.sex == *s, c.sex == *s, b.contains(m));
                    }
                }
                _ => {
                    let (prev, next) = arrows(r);
                    for (b, t) in [(prev, "<"), (next, ">")] {
                        button(b, t, false, false, b.contains(m));
                    }
                    let mid = Rect::new(prev.right() + 6.0, r.y, next.x - prev.right() - 12.0, r.h);
                    if active {
                        draw_rectangle_lines(mid.x, mid.y, mid.w, mid.h, 2.0, GOLD);
                    }
                    let (text, swatch) = match row {
                        Row::Race => (race.map_or(String::new(), |r| r.name.clone()), None),
                        Row::Body => {
                            let b = self.creator_body(c);
                            (self.project.db.sheet(&b).map_or(b.clone(), |s| s.label()), None)
                        }
                        Row::Feature(f) => {
                            let colors = features.iter().find(|x| x.0 == *f).map(|x| x.2.clone()).unwrap_or_default();
                            match c.colors.get(*f).copied().unwrap_or(0) {
                                0 => ("As drawn".to_string(), None),
                                n => (format!("{}/{}", (n - 1) % colors.len().max(1) + 1, colors.len()), colors.get((n - 1) % colors.len().max(1)).copied()),
                            }
                        }
                        _ => (String::new(), None),
                    };
                    let mut x = mid.x + 10.0;
                    if let Some(col) = swatch {
                        draw_rectangle(x, mid.y + 7.0, 40.0, mid.h - 14.0, rgb(col));
                        draw_rectangle_lines(x, mid.y + 7.0, 40.0, mid.h - 14.0, 1.5, Color::new(0.85, 0.8, 0.6, 0.8));
                        x += 52.0;
                    }
                    draw_text(&text, x, mid.y + 26.0, 22.0, WHITE);
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
            let col = (c.time * 8.0) as u32 % super::paperdoll::LAYOUT_FRAMES;
            let (fw, fh) = self.character_frame_size(&a);
            let big = (190.0 / fh).floor().max(1.0);
            let dest = Rect::new(pr.x + (pr.w - fw * big) / 2.0, pr.y + 6.0, fw * big, fh * big);
            let layer = self.character_figure(&a, facing, col);
            self.assets.draw_frame_colored_in(&layer.sheet, self.layer_frame(&layer), dest, &layer.colors);
            for (k, f) in order.iter().enumerate() {
                let r = Rect::new(pr.x + 8.0 + k as f32 * 58.0, pr.y + pr.h - 88.0, fw * 1.6, fh * 1.6);
                let layer = self.character_figure(&a, *f, 0);
                self.assets.draw_frame_colored_in(&layer.sheet, self.layer_frame(&layer), r, &layer.colors);
            }
        }

        // The race's description under the preview.
        if let Some(r) = race.filter(|r| !r.description.is_empty() && self.creator_races().len() > 1) {
            let text = r.description.clone();
            let mut y = pr.y + pr.h + 26.0;
            let mut line = String::new();
            for w in text.split_whitespace() {
                let next = if line.is_empty() { w.to_string() } else { format!("{line} {w}") };
                if measure_text(&next, None, 18, 1.0).width > pr.w && !line.is_empty() {
                    draw_text(&line, pr.x, y, 18.0, LIGHTGRAY);
                    y += 20.0;
                    line = w.to_string();
                } else {
                    line = next;
                }
            }
            if !line.is_empty() {
                draw_text(&line, pr.x, y, 18.0, LIGHTGRAY);
            }
        }

        button(l.start, "Start adventure", sel == Some(Row::Start), false, l.start.contains(m));
        button(l.back, "Back", sel == Some(Row::Back), false, l.back.contains(m));
        super::glyphs::draw_hint("Type a name  -  {key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  Enter: start  -  {key:cancel}: back", p.x + 30.0, p.y + H - 6.0, 16.0, GRAY);
    }
}
