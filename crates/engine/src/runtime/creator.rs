//! Character creator, shown after "New game" when the project enables it: the player types a
//! name, picks male / female, a body (the project's default body for that sex and the other
//! playable bodies of that sex, e.g. a goblin), skin color, hair style and hair color, and
//! (men) a beard.
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
    Sex,
    /// Playable bodies of the chosen sex (only when there is more than one).
    Body,
    Skin,
    Hair,
    HairColor,
    /// Men only.
    Beard,
    Start,
    Back,
}

impl Row {
    fn label(self) -> &'static str {
        match self {
            Row::Name => "Name",
            Row::Sex => "Sex",
            Row::Body => "Body",
            Row::Skin => "Skin",
            Row::Hair => "Hair",
            Row::HairColor => "Hair color",
            Row::Beard => "Beard",
            Row::Start | Row::Back => "",
        }
    }
}

pub(super) struct Creator {
    name: String,
    sex: Sex,
    /// Chosen body per sex (index into `creator_bodies`).
    body: [usize; 2],
    skin: usize,
    hair: usize,
    hair_color: usize,
    beard: usize,
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
    Layout {
        panel: p,
        rows: (0..n_rows).map(|k| Rect::new(fx, p.y + 76.0 + k as f32 * ROW_H, 280.0, 38.0)).collect(),
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

/// "hair_short" -> "Short", "beard_full" -> "Full".
fn pretty(id: &str) -> String {
    let s = id.trim_start_matches("hair_").trim_start_matches("beard_").replace('_', " ");
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
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

    /// Bodies a player of `sex` may choose: the project's default body for that sex first, then
    /// the other playable bodies of that sex (sheets that exist).
    fn creator_bodies(&self, sex: Sex) -> Vec<String> {
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

    /// Choice rows the project offers (a list without entries has no row; Body only with a
    /// choice).
    fn creator_rows(&self, c: &Creator) -> Vec<Row> {
        let cc = &self.project.config.creation;
        let mut rows = vec![Row::Name, Row::Sex];
        let beards = if c.sex == Sex::Male { cc.beards.len() } else { 0 };
        let bodies = self.creator_bodies(c.sex).len();
        // A complete figure (a pre-made hero) has its skin and hair painted in.
        let complete = self.project.db.complete_body(&self.chosen_appearance(c).body);
        let looks = |n: usize| if complete { 0 } else { n };
        let choices = [(Row::Body, if bodies > 1 { bodies } else { 0 }), (Row::Skin, looks(cc.skin_colors.len())), (Row::Hair, looks(cc.hair_styles.len())), (Row::HairColor, looks(cc.hair_colors.len())), (Row::Beard, looks(beards))];
        rows.extend(choices.into_iter().filter(|(_, n)| *n > 0).map(|(r, _)| r));
        rows.extend([Row::Start, Row::Back]);
        rows
    }

    /// The player definition's look with the creator's choices.
    fn chosen_appearance(&self, c: &Creator) -> Appearance {
        let cc = &self.project.config.creation;
        let mut a = self.project.db.character(&self.project.config.player).map(|d| d.appearance.clone()).unwrap_or_default();
        let bodies = self.creator_bodies(c.sex);
        if !bodies.is_empty() {
            a.body = bodies[c.body[c.sex as usize] % bodies.len()].clone();
        }
        let pick = |v: &Vec<[u8; 3]>, k: usize, d: [u8; 3]| if v.is_empty() { d } else { v[k % v.len()] };
        a.skin = pick(&cc.skin_colors, c.skin, a.skin);
        a.hair_color = pick(&cc.hair_colors, c.hair_color, a.hair_color);
        if !cc.hair_styles.is_empty() {
            a.hair = cc.hair_styles[c.hair % cc.hair_styles.len()].clone();
        }
        if c.sex == Sex::Female {
            a.beard.clear();
        } else if !cc.beards.is_empty() {
            a.beard = cc.beards[c.beard % cc.beards.len()].clone();
        }
        a
    }

    /// Opens the creator (from the title screen's "New game").
    pub(super) fn open_creator(&mut self) {
        while input::next_char().is_some() {} // drop keys typed before
        let sex = self.project.config.creation.sex.fixed().unwrap_or(Sex::Male);
        self.creator = Some(Creator { name: self.default_player_name(), sex, body: [0, 0], skin: 0, hair: 0, hair_color: 0, beard: 0, row: 0, time: 0.0 });
    }

    /// Steps the choice of a row.
    fn change_row(&mut self, row: Row, step: i32) {
        if self.creator.is_none() {
            return;
        }
        let cc = &self.project.config.creation;
        let sex = self.creator.as_ref().map_or(Sex::Male, |c| c.sex);
        let n = match row {
            Row::Body => self.creator_bodies(sex).len(),
            Row::Skin => cc.skin_colors.len(),
            Row::Hair => cc.hair_styles.len(),
            Row::HairColor => cc.hair_colors.len(),
            Row::Beard => cc.beards.len(),
            _ => 0,
        }
        .max(1) as i32;
        let fixed = cc.sex.fixed().is_some();
        let c = self.creator.as_mut().unwrap();
        let cycle = |v: &mut usize| *v = (*v as i32 + step).rem_euclid(n) as usize;
        match row {
            Row::Sex if !fixed => c.sex = if c.sex == Sex::Male { Sex::Female } else { Sex::Male },
            Row::Body => cycle(&mut c.body[c.sex as usize]),
            Row::Skin => cycle(&mut c.skin),
            Row::Hair => cycle(&mut c.hair),
            Row::HairColor => cycle(&mut c.hair_color),
            Row::Beard => cycle(&mut c.beard),
            _ => {}
        }
    }

    fn start_created(&mut self) {
        let Some(c) = self.creator.take() else { return };
        let appearance = self.chosen_appearance(&c);
        let name = c.name.trim().to_string();
        self.new_game();
        self.world.player_name = if name.is_empty() { self.default_player_name() } else { name };
        self.world.player_sex = c.sex;
        self.world.player_appearance = Some(appearance);
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
                        self.creator.as_mut().unwrap().sex = if female { Sex::Female } else { Sex::Male };
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
        let or = |s: &str| if s.is_empty() { "-".to_string() } else { s.to_string() };
        let rows: Vec<&str> = self.creator_rows(c).iter().map(|r| r.label()).filter(|l| !l.is_empty()).collect();
        let look = format!("{} skin {:?} hair {} {:?} beard {} rows {}", a.body, a.skin, or(&a.hair), a.hair_color, or(&a.beard), rows.join("/"));
        Some(format!("{}|{:?}|{}", c.name, c.sex, look))
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

        for (k, row) in choice_rows.iter().enumerate() {
            let r = l.rows[k];
            let active = sel == Some(*row);
            draw_text(row.label(), p.x + 30.0, r.y + 27.0, 24.0, if active { GOLD } else { LIGHTGRAY });
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
                    let style = |list: &Vec<String>, k: usize| {
                        let h = list.get(k % list.len().max(1)).cloned().unwrap_or_default();
                        (if h.is_empty() { "None".to_string() } else { pretty(&h) }, None)
                    };
                    let bodies = self.creator_bodies(c.sex);
                    let (text, swatch) = match row {
                        Row::Body => {
                            let b = bodies.get(c.body[c.sex as usize] % bodies.len().max(1)).cloned().unwrap_or_default();
                            (self.project.db.sheet(&b).map_or(b.clone(), |s| s.label()), None)
                        }
                        Row::Skin => (format!("{}/{}", c.skin % cc.skin_colors.len().max(1) + 1, cc.skin_colors.len()), cc.skin_colors.get(c.skin % cc.skin_colors.len().max(1)).copied()),
                        Row::Hair => style(&cc.hair_styles, c.hair),
                        Row::Beard => style(&cc.beards, c.beard),
                        _ => (format!("{}/{}", c.hair_color % cc.hair_colors.len().max(1) + 1, cc.hair_colors.len()), cc.hair_colors.get(c.hair_color % cc.hair_colors.len().max(1)).copied()),
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
        let eq = self.project.db.character(&self.project.config.player).map(|d| d.equipment.clone()).unwrap_or_default();
        {
            let order = [crate::entity::Facing::Down, crate::entity::Facing::Left, crate::entity::Facing::Up, crate::entity::Facing::Right];
            let facing = order[(c.time / 1.5) as usize % 4];
            let col = (c.time * 8.0) as u32 % super::paperdoll::LAYOUT_FRAMES;
            let (fw, fh) = self.character_frame_size(&a);
            let big = (190.0 / fh).floor().max(1.0);
            let dest = Rect::new(pr.x + (pr.w - fw * big) / 2.0, pr.y + 6.0, fw * big, fh * big);
            for layer in self.character_layers(&a, &eq, facing, col, false) {
                self.assets.draw_frame_colored_in(&layer.sheet, layer.frame, dest, layer.skin, layer.hair);
            }
            for (k, f) in order.iter().enumerate() {
                let r = Rect::new(pr.x + 8.0 + k as f32 * 58.0, pr.y + pr.h - 88.0, fw * 1.6, fh * 1.6);
                for layer in self.character_layers(&a, &eq, *f, 0, false) {
                    self.assets.draw_frame_colored_in(&layer.sheet, layer.frame, r, layer.skin, layer.hair);
                }
            }
        }

        button(l.start, "Start adventure", sel == Some(Row::Start), false, l.start.contains(m));
        button(l.back, "Back", sel == Some(Row::Back), false, l.back.contains(m));
        super::glyphs::draw_hint("Type a name  -  {key:up}/{key:down}: select  -  {key:left}/{key:right}: change  -  Enter: start  -  {key:cancel}: back", p.x + 30.0, p.y + H - 6.0, 16.0, GRAY);
    }
}
