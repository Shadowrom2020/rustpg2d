//! Pause menu (Esc): resume, save to a named slot, load / delete saves, save and quit,
//! quit to the title screen; options (graphics, sound, text); and the title screen (main menu).


use macroquad::prelude::*;

use super::input::{self, Act, Pad};
use super::{ui::panel, Arrival, Fade, Game, FADE_TIME};
use crate::settings::{Settings, TEXT_SPEEDS, ZOOMS};
use super::text::{draw_text, measure_text};

const W: f32 = 520.0;
const ROW_H: f32 = 40.0;
const MAX_ROWS: usize = 12;
const MAX_NAME: usize = 32;

#[derive(Clone, Debug, PartialEq)]
enum Page {
    /// Title screen (main menu at game start).
    Title,
    Main,
    Save,
    Load,
    /// Typing the name of a new save.
    Name,
    ConfirmOverwrite(String),
    ConfirmDelete(String),
    /// Quit to the title screen without saving?
    ConfirmQuit,
    /// The window's close button was clicked: save before exiting? Holds the page to go back
    /// to on Cancel (None = back to the game).
    ConfirmClose(Option<Box<Page>>),
    /// Player options (graphics, sound, text).
    Options,
    /// The keys of every action (Options › Controls).
    Controls,
    /// The player was defeated: continue from the last save, start over or quit.
    Defeated,
    /// The game's achievements, unlocked ones with their date (title and pause menus).
    Achievements,
    /// Forget every unlocked achievement?
    ConfirmResetAchievements,
}

/// An entry of the Options page.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Opt {
    Fullscreen,
    Zoom,
    ShowFps,
    Master,
    Music,
    Effects,
    /// Environmental sounds (rain).
    Ambience,
    TextSpeed,
    /// Screen shake and the hurt flash.
    ScreenEffects,
    /// The game's language (its own and its translations).
    Language,
    /// How gamepad buttons are shown (Auto / Xbox / PlayStation / Nintendo).
    ButtonStyle,
    /// Opens the Controls page.
    Controls,
    Reset,
    Back,
}

const OPTS: [Opt; 14] = [Opt::Fullscreen, Opt::Zoom, Opt::ShowFps, Opt::Master, Opt::Music, Opt::Effects, Opt::Ambience, Opt::TextSpeed, Opt::ScreenEffects, Opt::Language, Opt::ButtonStyle, Opt::Controls, Opt::Reset, Opt::Back];

struct SaveInfo {
    slot: String,
    map: String,
    /// Seconds since the epoch when saved (file time for old saves).
    saved: u64,
    play_time: f64,
    /// The picture saved with it (`<slot>.png`; none for older saves).
    thumb: Option<Texture2D>,
}

/// Rows of the Save and Load pages: taller, with the thumbnail at the left.
const SAVE_ROW_H: f32 = 60.0;
const SAVE_ROWS: usize = 7;

pub(super) struct Menu {
    open: bool,
    page: Page,
    sel: usize,
    scroll: usize,
    saves: Vec<SaveInfo>,
    name: String,
    message: Option<String>,
    pub quit: bool,
    /// Showing the title screen (instead of the pause menu).
    title: bool,
    /// Controls page: waiting for a key for this action.
    capture: Option<Act>,
}

impl Default for Menu {
    fn default() -> Self {
        Menu { open: false, page: Page::Main, sel: 0, scroll: 0, saves: Vec::new(), name: String::new(), message: None, quit: false, title: false, capture: None }
    }
}

/// What a row of the current page does.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    NewGame,
    /// Load the newest save.
    Continue,
    Exit,
    Resume,
    SaveMenu,
    LoadMenu,
    SaveQuit,
    Quit,
    NewSave,
    Slot(String),
    Yes,
    No,
    /// Save, then exit the program.
    SaveExit,
    /// Exit the program without saving.
    ExitNoSave,
    OptionsMenu,
    Opt(Opt),
    /// Defeat menu: load the newest save.
    LastSave,
    /// Defeat menu: a new game as the same hero.
    StartOver,
    /// Defeat menu: back to the title screen.
    ToTitle,
    /// Controls page: an action's keys (Enter adds one, Del clears them).
    Bind(Act),
    /// Controls page: every action back to its default keys.
    ResetKeys,
    /// Controls page: back to the Options page.
    BackToOptions,
    /// Opens the Achievements page.
    AchievementsMenu,
    /// An achievement on the Achievements page (nothing to do on Enter).
    Achievement(String),
    ResetAchievements,
    /// Back to the previous page.
    Back,
}

pub(super) fn now_secs() -> u64 {
    crate::sys::unix_now()
}

/// Slot names become file names: letters, digits, space, '-' and '_'.
fn sanitize(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).take(MAX_NAME).collect();
    s.trim().to_string()
}

pub(super) fn display_name(slot: &str) -> String {
    match slot {
        "quick" => "Quicksave".to_string(),
        super::AUTOSAVE => "Autosave".to_string(),
        _ => slot.to_string(),
    }
}

fn ago(secs: u64) -> String {
    let d = now_secs().saturating_sub(secs);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        _ => format!("{} days ago", d / 86_400),
    }
}

fn play_time(secs: f64) -> String {
    let s = secs as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

impl Game {
    /// Lists the save files (newest first).
    fn scan_saves(&mut self) {
        let dir = self.project.saves_dir();
        let mut out = Vec::new();
        for p in crate::sys::files_in(&dir).into_iter().flatten() {
            if p.extension().is_none_or(|x| x != "ron") {
                continue;
            }
            let Some(slot) = p.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
            let file_time = crate::sys::modified_secs(&p);
            let world = match crate::sys::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| crate::state::WorldState::from_ron(&s).map_err(|e| format!("{e:#}"))) {
                Ok(w) => Some(w),
                Err(e) => {
                    eprintln!("[game] save {} is unreadable: {e}", p.display());
                    None
                }
            };
            let (map, saved, play) = match &world {
                Some(w) => (w.current_map.clone(), if w.saved_at > 0 { w.saved_at } else { file_time }, w.play_time),
                None => ("(unreadable)".into(), file_time, 0.0),
            };
            // A damaged picture just isn't shown (Texture2D::from_file_with_format would panic).
            let thumb = crate::sys::read(&p.with_extension("png"))
                .ok()
                .and_then(|b| Image::from_file_with_format(&b, Some(ImageFormat::Png)).ok())
                .map(|img| Texture2D::from_image(&img));
            if let Some(t) = &thumb {
                t.set_filter(FilterMode::Linear);
            }
            out.push(SaveInfo { slot, map, saved, play_time: play, thumb });
        }
        out.sort_by_key(|s| std::cmp::Reverse(s.saved));
        self.menu.saves = out;
    }

    fn rows(&self) -> Vec<(Row, String, String)> {
        let save_rows = |m: &Menu| -> Vec<(Row, String, String)> {
            m.saves
                .iter()
                .map(|s| {
                    let detail = format!("{} · {} played · {}", s.map, play_time(s.play_time), ago(s.saved));
                    (Row::Slot(s.slot.clone()), display_name(&s.slot), detail)
                })
                .collect()
        };
        // The Achievements row of the title and pause menus (games that have achievements).
        let (got, all) = (self.ach.progress.unlocked.keys().filter(|id| self.project.db.achievement(id).is_some()).count(), self.project.db.achievements.len());
        let achievements_row: Option<(Row, String, String)> = (all > 0).then(|| (Row::AchievementsMenu, "Achievements".into(), format!("{got} of {all} unlocked")));
        let latest_save = |m: &Menu| m.saves.first().map(|s| format!("{} · {} · {}", display_name(&s.slot), s.map, ago(s.saved)));
        match &self.menu.page {
            Page::Defeated => vec![
                (Row::LastSave, "Continue from last save".into(), latest_save(&self.menu).unwrap_or("no saved game yet".into())),
                (Row::StartOver, "Start over".into(), "A new game from the beginning, as the same hero".into()),
                (Row::ToTitle, "Quit to main menu".into(), String::new()),
            ],
            Page::Title => {
                let latest = self.menu.saves.first().map_or("no saved game yet".to_string(), |s| {
                    format!("{} · {} · {}", display_name(&s.slot), s.map, ago(s.saved))
                });
                let mut v = vec![
                    (Row::NewGame, "New game".into(), String::new()),
                    (Row::Continue, "Continue".into(), latest),
                    (Row::LoadMenu, "Load game".into(), String::new()),
                    (Row::OptionsMenu, "Options".into(), String::new()),
                ];
                v.extend(achievements_row.clone());
                // (A web page isn't closed by the game: the tab is.)
                if !cfg!(target_arch = "wasm32") {
                    v.push((Row::Exit, "Exit".into(), String::new()));
                }
                v
            }
            Page::Main => {
                let mut v = vec![
                    (Row::Resume, "Resume".into(), String::new()),
                    (Row::SaveMenu, "Save game".into(), String::new()),
                    (Row::LoadMenu, "Load game".into(), String::new()),
                    (Row::OptionsMenu, "Options".into(), "Graphics, sound and text".into()),
                ];
                v.extend(achievements_row);
                v.push((Row::SaveQuit, "Save and quit".into(), format!("Saves to \"{}\" and returns to the main menu", display_name(&self.save_quit_slot()))));
                v.push((Row::Quit, "Quit".into(), "Back to the main menu without saving".into()));
                v
            }
            Page::Achievements => {
                let mut v: Vec<(Row, String, String)> = self
                    .project
                    .db
                    .achievements
                    .iter()
                    .map(|d| {
                        let when = self.ach.progress.unlocked.get(&d.id);
                        let (name, text) = match when {
                            None if d.hidden => ("???".to_string(), "A hidden achievement".to_string()),
                            _ => (d.name.clone(), d.description.clone()),
                        };
                        let detail = match when {
                            Some(t) => format!("{text} · unlocked {}", ago(*t)),
                            None => text,
                        };
                        (Row::Achievement(d.id.clone()), name, detail)
                    })
                    .collect();
                v.push((Row::ResetAchievements, "Reset achievements".into(), "Forget every unlocked achievement".into()));
                v.push((Row::Back, "Back".into(), String::new()));
                v
            }
            Page::ConfirmResetAchievements => vec![
                (Row::Yes, "Reset".into(), "Every achievement is locked again".into()),
                (Row::No, "Cancel".into(), String::new()),
            ],
            Page::Save => {
                let mut v = vec![(Row::NewSave, "+ New save...".into(), String::new())];
                // The quick save (F5) and the autosave have their own keys / moments.
                v.extend(save_rows(&self.menu).into_iter().filter(|r| r.0 != Row::Slot("quick".into()) && r.0 != Row::Slot(super::AUTOSAVE.into())));
                v
            }
            Page::Load => save_rows(&self.menu),
            Page::Name => vec![],
            Page::ConfirmQuit => vec![
                (Row::Yes, "Quit".into(), "Progress since the last save is lost".into()),
                (Row::No, "Cancel".into(), String::new()),
            ],
            Page::ConfirmClose(_) => vec![
                (
                    Row::SaveExit,
                    "Save and exit".into(),
                    if self.runner.busy() {
                        "Not possible during an event: Cancel, finish it, then close again".into()
                    } else {
                        format!("Saves to \"{}\", then closes the game", display_name(&self.save_quit_slot()))
                    },
                ),
                (Row::ExitNoSave, "Exit without saving".into(), "Progress since the last save is lost".into()),
                (Row::No, "Cancel".into(), "Back to the game".into()),
            ],
            Page::Options => OPTS.iter().map(|o| (Row::Opt(*o), Self::opt_label(*o).into(), self.opt_value(*o))).collect(),
            Page::Controls => {
                let mut v: Vec<(Row, String, String)> = Act::ALL
                    .into_iter()
                    .filter(|a| a.rebindable())
                    .map(|a| {
                        let keys: Vec<String> = input::keys_of(a).into_iter().map(input::key_label).collect();
                        (Row::Bind(a), a.label().into(), if keys.is_empty() { "-".into() } else { keys.join(", ") })
                    })
                    .collect();
                v.push((Row::ResetKeys, "Reset to defaults".into(), String::new()));
                v.push((Row::BackToOptions, "Back".into(), String::new()));
                v
            }
            Page::ConfirmOverwrite(_) | Page::ConfirmDelete(_) => {
                vec![(Row::Yes, "Yes".into(), String::new()), (Row::No, "No".into(), String::new())]
            }
        }
    }

    fn title(&self) -> String {
        match &self.menu.page {
            Page::Title => String::new(),
            Page::Main => "Paused".into(),
            Page::Save => "Save game".into(),
            Page::Load => "Load game".into(),
            Page::Name => "Name your save".into(),
            Page::ConfirmOverwrite(s) => format!("Overwrite \"{}\"?", display_name(s)),
            Page::ConfirmDelete(s) => format!("Delete \"{}\"?", display_name(s)),
            Page::ConfirmQuit => "Quit without saving?".into(),
            Page::ConfirmClose(_) => "Exit the game?".into(),
            Page::Options => "Options".into(),
            Page::Controls => "Controls".into(),
            Page::Defeated => "You were defeated".into(),
            Page::Achievements => {
                let got = self.ach.progress.unlocked.keys().filter(|id| self.project.db.achievement(id).is_some()).count();
                format!("Achievements {got}/{}", self.project.db.achievements.len())
            }
            Page::ConfirmResetAchievements => "Reset achievements?".into(),
        }
    }

    /// The Save and Load pages list saves with their thumbnails.
    fn save_page(&self) -> bool {
        matches!(self.menu.page, Page::Save | Page::Load | Page::Achievements)
    }

    /// Selects `row` on the current page (back from a sub-page onto the row that opened it).
    fn select(&mut self, row: &Row) {
        let rows = self.rows();
        if let Some(k) = rows.iter().position(|r| &r.0 == row) {
            self.menu.sel = k;
            let max = self.max_rows();
            super::ui::clamp_list(&mut self.menu.sel, &mut self.menu.scroll, rows.len(), max);
        }
    }

    fn row_h(&self) -> f32 {
        if self.save_page() {
            SAVE_ROW_H
        } else {
            ROW_H
        }
    }

    /// How many rows the page shows at once (more scroll).
    fn max_rows(&self) -> usize {
        if self.save_page() {
            SAVE_ROWS
        } else {
            MAX_ROWS
        }
    }

    /// Panel rect and the screen rects of the visible rows.
    fn menu_layout(&self, n: usize) -> (Rect, Vec<Rect>) {
        let (sw, sh) = (screen_width(), screen_height());
        let (row_h, max_rows) = (self.row_h(), self.max_rows());
        let rows = n.clamp(1, max_rows) as f32;
        // The title page has no heading (the game name is drawn above the panel).
        let head = if self.menu.page == Page::Title { 20.0 } else { 64.0 };
        let h = head + 6.0 + rows * row_h + 50.0 + if self.menu.page == Page::Name { 60.0 } else { 0.0 };
        let p = Rect::new(((sw - W) / 2.0).round(), ((sh - h) / 2.0).round() + if self.menu.title { 40.0 } else { 0.0 }, W, h);
        let top = p.y + head + if self.menu.page == Page::Name { 60.0 } else { 0.0 };
        let rects = (0..n.min(max_rows)).map(|k| Rect::new(p.x + 16.0, top + k as f32 * row_h, W - 32.0, row_h - 4.0)).collect();
        (p, rects)
    }

    fn go(&mut self, page: Page) {
        if matches!(page, Page::Main | Page::Save | Page::Load | Page::Title | Page::Defeated) {
            self.scan_saves();
        }
        self.menu.page = page;
        self.menu.sel = 0;
        self.menu.scroll = 0;
    }

    fn back(&mut self) {
        match self.menu.page.clone() {
            // Esc / a click outside don't leave these.
            Page::Title | Page::Defeated => {}
            Page::Main => self.menu.open = false,
            Page::Load if self.menu.title => {
                self.go(Page::Title);
                self.select(&Row::LoadMenu);
            }
            Page::Save | Page::Load => {
                let row = if self.menu.page == Page::Save { Row::SaveMenu } else { Row::LoadMenu };
                self.go(Page::Main);
                self.select(&row); // back on the row we came from
            }
            Page::Achievements => {
                self.go(if self.menu.title { Page::Title } else { Page::Main });
                self.select(&Row::AchievementsMenu);
            }
            Page::ConfirmResetAchievements => {
                self.go(Page::Achievements);
                self.select(&Row::ResetAchievements);
            }
            Page::Name | Page::ConfirmOverwrite(_) => self.go(Page::Save),
            Page::ConfirmDelete(_) => self.go(Page::Load),
            Page::ConfirmQuit => {
                self.go(Page::Main);
                self.select(&Row::Quit);
            }
            Page::Options => {
                self.go(if self.menu.title { Page::Title } else { Page::Main });
                self.select(&Row::OptionsMenu);
            }
            Page::Controls => {
                self.go(Page::Options);
                self.menu.sel = OPTS.iter().position(|o| *o == Opt::Controls).unwrap();
            }
            Page::ConfirmClose(back) => match back {
                Some(p) => self.go(*p),
                None => self.menu.open = false,
            },
        }
    }

    /// Slot "Save and quit" writes to: the one this game was last saved to / loaded from,
    /// otherwise a new "Save N".
    fn save_quit_slot(&self) -> String {
        if let Some(s) = &self.slot {
            return s.clone();
        }
        (1..).map(|n| format!("Save {n}")).find(|name| !self.menu.saves.iter().any(|s| &s.slot == name)).unwrap()
    }

    /// Leaves the running game for the title screen, with a fresh game behind it.
    pub(super) fn quit_to_title(&mut self) {
        self.reset_world();
        let (map, spawn) = (self.project.config.start_map.clone(), self.project.config.start_spawn.clone());
        self.finish_transition(map, Arrival::Spawn(spawn));
        // The fresh game behind the title screen isn't worth an autosave.
        self.autosave_pending = false;
        self.fade = Fade::In { t: FADE_TIME };
        self.open_title();
    }

    fn do_save(&mut self, slot: &str) {
        match self.save_game(slot) {
            Ok(()) => {
                self.menu.open = false;
                self.ui.toast(format!("Saved \"{}\"", display_name(slot)));
            }
            Err(e) => self.menu.message = Some(format!("Save failed: {e}")),
        }
    }

    fn activate_row(&mut self, row: Row) {
        self.menu.message = None;
        match (self.menu.page.clone(), row) {
            (_, Row::NewGame) => {
                if self.project.config.creation.enabled {
                    self.open_creator();
                } else {
                    self.new_game();
                    self.close_title();
                }
            }
            (_, Row::Continue) => match self.menu.saves.first().map(|s| s.slot.clone()) {
                Some(slot) => match self.load_game(&slot) {
                    Ok(()) => self.close_title(),
                    Err(e) => self.menu.message = Some(format!("Load failed: {e}")),
                },
                None => self.menu.message = Some("There is no saved game yet.".into()),
            },
            (_, Row::Exit) => self.menu.quit = true,
            (_, Row::Resume) => self.menu.open = false,
            (_, Row::SaveMenu) => {
                if self.runner.busy() {
                    self.menu.message = Some("You can't save in the middle of an event.".into());
                } else {
                    self.go(Page::Save);
                }
            }
            (_, Row::LoadMenu) => self.go(Page::Load),
            (_, Row::OptionsMenu) => self.go(Page::Options),
            (_, Row::AchievementsMenu) => self.go(Page::Achievements),
            (_, Row::Achievement(_)) => {}
            (_, Row::ResetAchievements) => self.go(Page::ConfirmResetAchievements),
            (Page::ConfirmResetAchievements, Row::Yes) => {
                self.reset_achievements();
                self.back();
            }
            (_, Row::Back) => self.back(),
            (_, Row::Opt(Opt::Back)) => self.back(),
            (_, Row::Opt(Opt::Controls)) => self.go(Page::Controls),
            (_, Row::Bind(a)) => self.menu.capture = Some(a),
            (_, Row::ResetKeys) => {
                self.set_keys(None, None);
                self.set_pads(None, None);
            }
            (_, Row::BackToOptions) => self.back(),
            (_, Row::Opt(o)) => self.change_opt(o, 1, true),
            (_, Row::SaveQuit) => {
                let slot = self.save_quit_slot();
                match self.save_game(&slot) {
                    // The title screen comes once the thumbnail shows this game's world.
                    Ok(()) => self.after_thumb = Some(super::thumbs::AfterThumb::ToTitle(slot)),
                    Err(_) if self.runner.busy() => self.menu.message = Some("You can't save in the middle of an event.".into()),
                    Err(e) => self.menu.message = Some(format!("Save failed: {e}")),
                }
            }
            (_, Row::Quit) => self.go(Page::ConfirmQuit),
            (_, Row::SaveExit) => {
                let slot = self.save_quit_slot();
                match self.save_game(&slot) {
                    Ok(()) => self.menu.quit = true,
                    Err(_) if self.runner.busy() => {
                        self.menu.message = Some("You can't save in the middle of an event. Cancel, finish it, then exit.".into())
                    }
                    Err(e) => self.menu.message = Some(format!("Save failed: {e}")),
                }
            }
            (_, Row::ExitNoSave) => self.menu.quit = true,
            (_, Row::LastSave) => match self.menu.saves.first().map(|s| s.slot.clone()) {
                Some(slot) => match self.load_game(&slot) {
                    Ok(()) => self.menu = Menu::default(),
                    Err(e) => self.menu.message = Some(format!("Load failed: {e}")),
                },
                None => self.menu.message = Some("There is no saved game yet.".into()),
            },
            (_, Row::StartOver) => {
                self.menu = Menu::default();
                self.start_over();
            }
            (_, Row::ToTitle) => self.quit_to_title(),
            (Page::ConfirmQuit, Row::Yes) => self.quit_to_title(),
            (_, Row::NewSave) => {
                let n = (1..).find(|n| !self.menu.saves.iter().any(|s| s.slot == format!("Save {n}"))).unwrap();
                self.menu.name = format!("Save {n}");
                self.menu.page = Page::Name;
                while get_char_pressed().is_some() {} // drop keys typed before
            }
            (Page::Save, Row::Slot(s)) => self.go(Page::ConfirmOverwrite(s)),
            (Page::Load, Row::Slot(s)) => match self.load_game(&s) {
                Ok(()) => self.close_title(),
                Err(e) => self.menu.message = Some(format!("Load failed: {e}")),
            },
            (Page::ConfirmOverwrite(s), Row::Yes) => self.do_save(&s),
            (Page::ConfirmDelete(s), Row::Yes) => {
                let _ = crate::sys::remove_file(&self.project.saves_dir().join(format!("{s}.ron")));
                let _ = crate::sys::remove_file(&self.project.saves_dir().join(format!("{s}.png")));
                self.go(Page::Load);
            }
            (_, Row::No) => self.back(),
            _ => {}
        }
    }

    fn opt_label(o: Opt) -> &'static str {
        match o {
            Opt::Fullscreen => "Fullscreen",
            Opt::Zoom => "Zoom",
            Opt::ShowFps => "Show FPS",
            Opt::Master => "Master volume",
            Opt::Music => "Music volume",
            Opt::Effects => "Effects volume",
            Opt::Ambience => "Ambience volume",
            Opt::TextSpeed => "Text speed",
            Opt::ScreenEffects => "Shake & flash",
            Opt::Language => "Language",
            Opt::ButtonStyle => "Button style",
            Opt::Controls => "Controls...",
            Opt::Reset => "Reset to defaults",
            Opt::Back => "Back",
        }
    }

    fn opt_value(&self, o: Opt) -> String {
        let s = &self.settings;
        let on = |b: bool| if b { "On" } else { "Off" }.to_string();
        let pct = |v: f32| format!("{:.0}%", v * 100.0);
        match o {
            Opt::Fullscreen => on(s.fullscreen),
            Opt::Zoom => pct(s.zoom),
            Opt::ShowFps => on(s.show_fps),
            Opt::ScreenEffects => on(s.screen_effects),
            Opt::Master => pct(s.master_volume),
            Opt::Music => pct(s.music_volume),
            Opt::Effects => pct(s.effects_volume),
            Opt::Ambience => pct(s.ambience_volume),
            Opt::TextSpeed => TEXT_SPEEDS.iter().find(|t| t.0 == s.text_speed).map_or("Normal", |t| t.1).to_string(),
            Opt::Language => self.languages.iter().find(|(c, _)| *c == self.lang_code).map_or_else(|| self.lang_code.clone(), |(_, n)| n.clone()),
            Opt::ButtonStyle => super::glyphs::PadStyle::from_id(&s.pad_style).label().to_string(),
            Opt::Controls | Opt::Reset | Opt::Back => String::new(),
        }
    }

    /// Changes an option by one step (`wrap`: past the end starts over), applies and saves it.
    fn change_opt(&mut self, o: Opt, dir: i32, wrap: bool) {
        let step = |list: &[f32], cur: f32| -> f32 {
            let i = list.iter().position(|v| (*v - cur).abs() < 1e-3).unwrap_or(0) as i32;
            let n = list.len() as i32;
            let j = if wrap { (i + dir).rem_euclid(n) } else { (i + dir).clamp(0, n - 1) };
            list[j as usize]
        };
        const VOLUMES: [f32; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let s = &mut self.settings;
        match o {
            Opt::Fullscreen => s.fullscreen = !s.fullscreen,
            Opt::ShowFps => s.show_fps = !s.show_fps,
            Opt::ScreenEffects => s.screen_effects = !s.screen_effects,
            Opt::Zoom => s.zoom = step(&ZOOMS, s.zoom),
            Opt::Master => s.master_volume = step(&VOLUMES, s.master_volume),
            Opt::Music => s.music_volume = step(&VOLUMES, s.music_volume),
            Opt::Effects => s.effects_volume = step(&VOLUMES, s.effects_volume),
            Opt::Ambience => s.ambience_volume = step(&VOLUMES, s.ambience_volume),
            Opt::TextSpeed => {
                let speeds: Vec<f32> = TEXT_SPEEDS.iter().map(|t| t.0).collect();
                s.text_speed = step(&speeds, s.text_speed);
            }
            Opt::Language => {
                let n = self.languages.len().max(1) as i32;
                let i = self.languages.iter().position(|(c, _)| *c == self.lang_code).unwrap_or(0) as i32;
                let j = if wrap { (i + dir).rem_euclid(n) } else { (i + dir).clamp(0, n - 1) };
                s.language = self.languages.get(j as usize).map(|(c, _)| c.clone());
            }
            Opt::ButtonStyle => {
                use super::glyphs::PadStyle;
                let n = PadStyle::ALL.len() as i32;
                let i = PadStyle::ALL.iter().position(|p| *p == PadStyle::from_id(&s.pad_style)).unwrap_or(0) as i32;
                let j = if wrap { (i + dir).rem_euclid(n) } else { (i + dir).clamp(0, n - 1) };
                s.pad_style = PadStyle::ALL[j as usize].id().to_string();
            }
            // The keys, the buttons and the language have their own ways back.
            Opt::Reset => *s = Settings { keys: std::mem::take(&mut s.keys), pad: std::mem::take(&mut s.pad), language: s.language.take(), ..Settings::default() },
            Opt::Controls | Opt::Back => return,
        }
        self.apply_settings();
        if o == Opt::Effects || o == Opt::Master {
            self.audio.play(&self.project.config.sfx.pickup); // hear the new volume
        }
        if let Err(e) = self.settings.save(&self.project.config.name) {
            self.menu.message = Some(format!("Could not save the options: {e}"));
        }
    }

    /// Changes the keys of `act`: `Some(key)` adds a key (an action keeps at most three, the
    /// oldest drops out), `None` clears them; `act` None resets every action. Applies and saves.
    fn set_keys(&mut self, act: Option<Act>, key: Option<KeyCode>) {
        let mut table: std::collections::HashMap<Act, Vec<KeyCode>> = Act::ALL.into_iter().map(|a| (a, input::keys_of(a))).collect();
        match (act, key) {
            (None, _) => table.clear(),
            (Some(a), None) => {
                table.insert(a, Vec::new());
            }
            (Some(a), Some(k)) => {
                let keys = table.entry(a).or_default();
                if !keys.contains(&k) {
                    keys.push(k);
                    if keys.len() > 3 {
                        keys.remove(0);
                    }
                }
            }
        }
        self.settings.keys = input::binding_names(&table);
        input::set_bindings(&self.settings.keys);
        if let Err(e) = self.settings.save(&self.project.config.name) {
            self.menu.message = Some(format!("Could not save the options: {e}"));
        }
    }

    /// Sets the gamepad button of `act` (`None` clears it); `act` None resets every action.
    /// Applies and saves.
    fn set_pads(&mut self, act: Option<Act>, pad: Option<Pad>) {
        let mut table: std::collections::HashMap<Act, Vec<Pad>> = Act::ALL.into_iter().map(|a| (a, input::pads_of(a))).collect();
        match act {
            None => table.clear(),
            Some(a) => {
                table.insert(a, pad.into_iter().collect());
            }
        }
        self.settings.pad = input::pad_binding_names(&table);
        input::set_pad_bindings(&self.settings.pad);
        if let Err(e) = self.settings.save(&self.project.config.name) {
            self.menu.message = Some(format!("Could not save the options: {e}"));
        }
    }

    /// Controls page: what the selected row's keys and button clash with.
    fn conflict_text(&self, a: Act) -> Option<String> {
        let clashes: Vec<String> = input::keys_of(a)
            .into_iter()
            .flat_map(|k| input::conflicts(a, k).into_iter().map(move |b| format!("{} is also {}", input::key_label(k), b.label())))
            .chain(input::pads_of(a).into_iter().filter(|_| a.pad_rebindable()).flat_map(|p| input::pad_conflicts(a, p).into_iter().map(move |b| format!("{} is also {}", super::glyphs::pad_label(p), b.label()))))
            .collect();
        (!clashes.is_empty()).then(|| clashes.join(" · "))
    }

    /// Applies the options: volumes, fullscreen, keys (zoom, FPS and text speed are read where used).
    pub(super) fn apply_settings(&mut self) {
        input::set_bindings(&self.settings.keys);
        input::set_pad_bindings(&self.settings.pad);
        super::glyphs::set_style(super::glyphs::PadStyle::from_id(&self.settings.pad_style));
        if let Some(code) = self.settings.language.clone() {
            if code != self.lang_code {
                self.set_language(&code);
            }
        }
        let s = self.settings.clone();
        self.audio.set_volumes(s.effects(), s.music());
        self.audio.set_ambience_volume(s.ambience());
        if self.fullscreen != Some(s.fullscreen) {
            // At startup only switch if the player wants fullscreen (the window starts windowed).
            if self.fullscreen.is_some() || s.fullscreen {
                set_fullscreen(s.fullscreen);
            }
            self.fullscreen = Some(s.fullscreen);
        }
    }

    /// World pixels to screen pixels: the project's pixel scale times the player's zoom.
    pub(super) fn view_scale(&self) -> f32 {
        (self.project.config.scale.max(1.0) * self.settings.zoom.clamp(0.25, 4.0)).max(0.5)
    }

    /// FPS counter (top right, under the clock) if the player turned it on.
    pub(super) fn draw_fps(&self) {
        if self.settings.show_fps {
            let t = format!("{} fps", get_fps());
            let w = measure_text(&t, None, 16, 1.0).width;
            draw_text(&t, screen_width() - w - 14.0, 58.0, 16.0, Color::new(1.0, 1.0, 1.0, 0.7));
        }
    }

    /// The window's close button was clicked: asks whether to save first while a game is
    /// running; on the title / game over screen the game just exits.
    pub fn request_close(&mut self) {
        if (self.menu.open && self.menu.title) || self.game_over.is_some() {
            self.menu.quit = true;
            return;
        }
        if self.menu.open && matches!(self.menu.page, Page::ConfirmClose(_)) {
            return; // already asking
        }
        let back = self.menu.open.then(|| Box::new(self.menu.page.clone()));
        self.menu.open = true;
        self.menu.message = None;
        self.scan_saves();
        self.go(Page::ConfirmClose(back));
    }

    /// Shows the title screen (main menu). The selection starts on Continue if there are saves.
    pub(super) fn open_title(&mut self) {
        self.menu = Menu { open: true, title: true, ..Default::default() };
        self.go(Page::Title);
        if !self.menu.saves.is_empty() {
            self.menu.sel = 1;
        }
    }

    pub(super) fn close_title(&mut self) {
        self.menu.open = false;
        self.menu.title = false;
        // The start map's "map enter" from startup must not fire after loading another game;
        // a new game queues its own when the map is (re)entered.
        self.pending_events.clear();
    }

    pub(super) fn on_title(&self) -> bool {
        self.menu.title
    }

    /// Opens the defeat menu (after the game-over fade). The selection starts on *Continue
    /// from last save* if there is one.
    pub(super) fn open_defeat_menu(&mut self) {
        if self.defeat_menu_open() {
            return;
        }
        self.menu = Menu { open: true, ..Default::default() };
        self.go(Page::Defeated);
        if self.menu.saves.is_empty() {
            self.menu.sel = 1;
        }
    }

    pub(super) fn defeat_menu_open(&self) -> bool {
        self.menu.open && self.menu.page == Page::Defeated
    }

    /// For `Game::status()`: the open menu page ("-" = closed).
    pub(super) fn menu_status(&self) -> String {
        if self.menu.open { format!("{:?}", self.menu.page).to_lowercase() } else { "-".into() }
    }

    /// Handles Esc and the open menu. Returns true while the game is paused by it.
    pub(super) fn update_menu(&mut self, can_open: bool) -> bool {
        if !self.menu.open {
            if can_open && input::act_pressed(Act::Pause) {
                self.menu = Menu { open: true, ..Default::default() };
                self.go(Page::Main);
                return true;
            }
            return false;
        }
        let rows = self.rows();
        let (panel_rect, rects) = self.menu_layout(rows.len());
        let m = input::mouse_pos();

        if self.menu.page == Page::Name {
            while let Some(c) = input::next_char() {
                if !c.is_control() && self.menu.name.chars().count() < MAX_NAME {
                    self.menu.name.push(c);
                }
            }
            if input::pressed(KeyCode::Backspace) {
                self.menu.name.pop();
            }
            // A text field: Enter / Escape (or the gamepad's A / B), not E or Space, which type.
            if input::act_pressed(Act::Cancel) {
                self.back();
            } else if input::pressed(KeyCode::Enter) || input::pad_pressed(Pad::South) || (input::mouse_pressed(MouseButton::Left) && self.name_ok_rect().contains(m)) {
                let slot = sanitize(&self.menu.name);
                if slot.is_empty() {
                    self.menu.message = Some("Please enter a name.".into());
                } else if self.menu.saves.iter().any(|s| s.slot == slot) {
                    self.go(Page::ConfirmOverwrite(slot));
                } else {
                    self.do_save(&slot);
                }
            }
            return true;
        }

        // Controls page waiting for a key: the next key pressed is added (Esc keeps the keys).
        if let Some(a) = self.menu.capture {
            if let Some(k) = input::last_key_pressed() {
                self.menu.capture = None;
                if k == KeyCode::Escape {
                    return true;
                }
                if input::key_name(k).is_none() {
                    self.menu.message = Some("That key can't be used.".into());
                    return true;
                }
                self.set_keys(Some(a), Some(k));
            } else if let Some(b) = input::last_pad_pressed() {
                // B and Start keep the buttons; the d-pad moves.
                self.menu.capture = None;
                if matches!(b, Pad::East | Pad::Start) {
                    return true;
                }
                if !a.pad_rebindable() || matches!(b, Pad::Up | Pad::Down | Pad::Left | Pad::Right) {
                    self.menu.message = Some("That button can't be used for it.".into());
                    return true;
                }
                self.set_pads(Some(a), Some(b));
            }
            return true;
        }
        let n = rows.len();
        if input::act_pressed(Act::Cancel) {
            self.back();
            return true;
        }
        // Controls page: Del / Backspace clears the selected action's keys, the gamepad's X its button.
        if self.menu.page == Page::Controls {
            if let Some((Row::Bind(a), _, _)) = rows.get(self.menu.sel) {
                if input::any_pressed(&[KeyCode::Delete, KeyCode::Backspace]) {
                    self.set_keys(Some(*a), None);
                    return true;
                }
                if input::pad_pressed(Pad::West) && a.pad_rebindable() {
                    self.set_pads(Some(*a), None);
                    return true;
                }
            }
        }
        if n > 0 {
            if input::act_pressed(Act::Up) {
                self.menu.sel = (self.menu.sel + n - 1) % n;
            }
            if input::act_pressed(Act::Down) {
                self.menu.sel = (self.menu.sel + 1) % n;
            }
            // Keep the selection visible.
            let max = self.max_rows();
            super::ui::clamp_list(&mut self.menu.sel, &mut self.menu.scroll, n, max);
            let wheel = input::wheel();
            let max_rows = self.max_rows();
            if wheel != 0.0 && panel_rect.contains(m) && n > max_rows {
                self.menu.scroll = if wheel < 0.0 { (self.menu.scroll + 1).min(n - max_rows) } else { self.menu.scroll.saturating_sub(1) };
            }
            let hovered = rects.iter().position(|r| r.contains(m)).map(|k| k + self.menu.scroll).filter(|k| *k < n);
            if let Some(k) = hovered {
                if mouse_delta_position() != Vec2::ZERO {
                    self.menu.sel = k;
                }
            }
            if self.menu.page == Page::Options {
                let opt = OPTS[self.menu.sel.min(n - 1)];
                if input::act_pressed(Act::Left) {
                    self.change_opt(opt, -1, false);
                } else if input::act_pressed(Act::Right) {
                    self.change_opt(opt, 1, false);
                }
            }
            if input::act_pressed(Act::Confirm) {
                self.activate_row(rows[self.menu.sel.min(n - 1)].0.clone());
            } else if input::mouse_pressed(MouseButton::Left) {
                match hovered {
                    // Options: "<" goes to the previous value, ">" to the next one, a click
                    // elsewhere on the row cycles forward.
                    Some(k) if self.menu.page == Page::Options && !matches!(OPTS[k], Opt::Reset | Opt::Back | Opt::Controls) => {
                        self.menu.sel = k;
                        let (prev, next) = opt_arrows(rects[k - self.menu.scroll]);
                        if prev.contains(m) {
                            self.change_opt(OPTS[k], -1, false);
                        } else if next.contains(m) {
                            self.change_opt(OPTS[k], 1, false);
                        } else {
                            self.change_opt(OPTS[k], 1, true);
                        }
                    }
                    Some(k) => {
                        self.menu.sel = k;
                        self.activate_row(rows[k].0.clone());
                    }
                    None if !panel_rect.contains(m) => self.back(),
                    None => {}
                }
            }
            // Delete a save from the load list.
            if self.menu.page == Page::Load && (input::pressed(KeyCode::Delete) || input::pad_pressed(Pad::West)) {
                if let Row::Slot(s) = &rows[self.menu.sel.min(n - 1)].0 {
                    self.go(Page::ConfirmDelete(s.clone()));
                }
            }
        } else if input::mouse_pressed(MouseButton::Left) && !panel_rect.contains(m) {
            self.back();
        }
        true
    }

    /// Screen position of an Options row's "<" (`prev`), ">" (`next`) or the row itself
    /// (automated play-throughs); `None` unless the Options page is open.
    pub fn option_click_pos(&self, row: usize, part: &str) -> Option<Vec2> {
        if !self.menu.open || self.menu.page != Page::Options || row >= OPTS.len() {
            return None;
        }
        let (_, rects) = self.menu_layout(OPTS.len());
        let r = *rects.get(row.checked_sub(self.menu.scroll)?)?;
        let (prev, next) = opt_arrows(r);
        Some(match part {
            "prev" => prev.center(),
            "next" => next.center(),
            _ => vec2(r.x + 40.0, r.y + r.h / 2.0),
        })
    }

    fn name_ok_rect(&self) -> Rect {
        let (p, _) = self.menu_layout(0);
        Rect::new(p.x + W - 116.0, p.y + 70.0, 100.0, 36.0)
    }

    pub(super) fn draw_menu(&self) {
        if !self.menu.open {
            return;
        }
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
        let rows = self.rows();
        let (p, rects) = self.menu_layout(rows.len());
        if self.menu.title {
            // Big game title above the menu.
            let name = &crate::lang::tr(&self.project.config.name);
            let size = 64u16;
            let tw = measure_text(name, None, size, 1.0).width;
            let y = (p.y - 60.0).max(80.0);
            draw_text(name, (sw - tw) / 2.0 + 3.0, y + 3.0, size as f32, Color::new(0.0, 0.0, 0.0, 0.7));
            draw_text(name, (sw - tw) / 2.0, y, size as f32, GOLD);
        }
        panel(p.x, p.y, p.w, p.h);
        let title = self.title();
        let tw = measure_text(&title, None, 34, 1.0).width;
        draw_text(&title, p.x + (p.w - tw) / 2.0, p.y + 42.0, 34.0, GOLD);
        let m = input::mouse_pos();

        if self.menu.page == Page::Name {
            let field = Rect::new(p.x + 16.0, p.y + 70.0, W - 146.0, 36.0);
            draw_rectangle(field.x, field.y, field.w, field.h, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_rectangle_lines(field.x, field.y, field.w, field.h, 2.0, GOLD);
            let caret = if (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
            draw_text(format!("{}{caret}", self.menu.name), field.x + 10.0, field.y + 26.0, 26.0, WHITE);
            let ok = self.name_ok_rect();
            draw_rectangle(ok.x, ok.y, ok.w, ok.h, Color::new(0.3, 0.25, 0.1, if ok.contains(m) { 1.0 } else { 0.8 }));
            draw_rectangle_lines(ok.x, ok.y, ok.w, ok.h, 1.5, GOLD);
            draw_text("Save", ok.x + 28.0, ok.y + 25.0, 24.0, WHITE);
        }

        if rows.is_empty() && self.menu.page != Page::Name {
            draw_text("No saved games yet.", p.x + 24.0, p.y + 92.0, 22.0, GRAY);
        }
        for (k, r) in rects.iter().enumerate() {
            let idx = k + self.menu.scroll;
            let Some((_, label, detail)) = rows.get(idx) else { break };
            let selected = idx == self.menu.sel;
            if selected {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, 0.12));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, GOLD);
            }
            let disabled = self.menu.saves.is_empty() && ((self.menu.page == Page::Title && idx == 1) || (self.menu.page == Page::Defeated && idx == 0));
            let c = if disabled { GRAY } else if selected { GOLD } else { WHITE };
            if self.menu.page == Page::Options {
                draw_text(label, r.x + 14.0, r.y + 26.0, 24.0, c);
                if !detail.is_empty() {
                    let (prev, next) = opt_arrows(*r);
                    let base = if selected { WHITE } else { LIGHTGRAY };
                    for (a, txt) in [(prev, "<"), (next, ">")] {
                        let hot = a.contains(m);
                        if hot {
                            draw_rectangle(a.x + 2.0, a.y + 4.0, a.w - 4.0, a.h - 8.0, Color::new(1.0, 0.85, 0.3, 0.25));
                        }
                        let aw = measure_text(txt, None, 24, 1.0).width;
                        draw_text(txt, a.x + (a.w - aw) / 2.0, r.y + 26.0, 24.0, if hot { GOLD } else { base });
                    }
                    // The value centered between the arrows.
                    let vw = measure_text(detail, None, 22, 1.0).width;
                    draw_text(detail, (prev.right() + next.x - vw) / 2.0, r.y + 25.0, 22.0, base);
                }
            } else if self.menu.page == Page::Controls && !detail.is_empty() {
                // Action left, keys right (orange where they clash with another action).
                draw_text(label, r.x + 14.0, r.y + 26.0, 22.0, c);
                let Row::Bind(a) = rows[idx].0 else { continue };
                let (text, col) = if self.menu.capture == Some(a) {
                    ("Press a key or a button...".to_string(), GOLD)
                } else if self.conflict_text(a).is_some() {
                    (detail.clone(), Color::new(1.0, 0.65, 0.25, 1.0))
                } else {
                    (detail.clone(), if selected { WHITE } else { LIGHTGRAY })
                };
                let tw = measure_text(&text, None, 20, 1.0).width;
                draw_text(&text, r.x + r.w - 14.0 - tw, r.y + 25.0, 20.0, col);
                // Its gamepad button(s) in a column of their own.
                let mut bx = r.x + r.w * 0.5;
                for b in input::pads_of(a) {
                    bx += super::glyphs::draw_pad(b, bx, r.y + 19.0, 20.0) + 4.0;
                }
            } else if let Row::Achievement(id) = &rows[idx].0 {
                // An achievement: its icon (dim while locked), name and description / date.
                let unlocked = self.ach.progress.has(id);
                let d = self.project.db.achievement(id);
                let t = Rect::new(r.x + 6.0, r.y + 5.0, r.h - 10.0, r.h - 10.0);
                draw_rectangle(t.x, t.y, t.w, t.h, Color::new(0.0, 0.0, 0.0, 0.45));
                match d.filter(|d| !d.icon.sheet.is_empty() && (unlocked || !d.hidden)) {
                    Some(d) => {
                        self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, t, if unlocked { WHITE } else { Color::new(0.35, 0.35, 0.4, 0.8) });
                    }
                    None => {
                        let q = measure_text("?", None, 26, 1.0);
                        draw_text("?", t.x + (t.w - q.width) / 2.0, t.y + (t.h + q.height) / 2.0, 26.0, DARKGRAY);
                    }
                }
                draw_rectangle_lines(t.x, t.y, t.w, t.h, 1.0, Color::new(0.85, 0.72, 0.38, if unlocked { 1.0 } else { 0.35 }));
                let tx = t.right() + 12.0;
                let room = r.right() - tx - 8.0;
                draw_text(fit(label, 22, room), tx, r.y + 24.0, 22.0, if unlocked { c } else { GRAY });
                draw_text(fit(detail, 16, room), tx, r.y + 44.0, 16.0, if unlocked { LIGHTGRAY } else { GRAY });
            } else if let (true, Row::Slot(slot)) = (self.save_page(), &rows[idx].0) {
                // A save: its thumbnail (or a placeholder), then name and details.
                let th = r.h - 10.0;
                let t = Rect::new(r.x + 6.0, r.y + 5.0, th * 16.0 / 9.0, th);
                match self.menu.saves.iter().find(|s| &s.slot == slot).and_then(|s| s.thumb.as_ref()) {
                    Some(tex) => draw_texture_ex(tex, t.x, t.y, WHITE, DrawTextureParams { dest_size: Some(vec2(t.w, t.h)), ..Default::default() }),
                    None => {
                        draw_rectangle(t.x, t.y, t.w, t.h, Color::new(0.0, 0.0, 0.0, 0.45));
                        let q = measure_text("?", None, 26, 1.0);
                        draw_text("?", t.x + (t.w - q.width) / 2.0, t.y + (t.h + q.height) / 2.0, 26.0, DARKGRAY);
                    }
                }
                draw_rectangle_lines(t.x, t.y, t.w, t.h, 1.0, Color::new(0.85, 0.72, 0.38, if selected { 1.0 } else { 0.5 }));
                let tx = t.right() + 12.0;
                draw_text(label, tx, r.y + 24.0, 22.0, c);
                draw_text(detail, tx, r.y + 44.0, 16.0, LIGHTGRAY);
            } else if detail.is_empty() {
                // Centered in the row (the Save page's rows are taller).
                draw_text(label, r.x + 14.0, r.y + r.h / 2.0 + 8.0, 26.0, c);
            } else {
                draw_text(label, r.x + 14.0, r.y + 17.0, 22.0, c);
                draw_text(detail, r.x + 14.0, r.y + 33.0, 16.0, LIGHTGRAY);
            }
        }
        if rows.len() > self.max_rows() {
            draw_text(format!("{}-{} of {}", self.menu.scroll + 1, (self.menu.scroll + self.max_rows()).min(rows.len()), rows.len()), p.x + p.w - 110.0, p.y + 42.0, 16.0, GRAY);
        }
        let hint = match self.menu.page {
            Page::Title => "{key:up}/{key:down} + {key:confirm} or click",
            Page::Main => "{key:up}/{key:down} + {key:confirm} or click · {key:cancel}: resume",
            Page::Save => "Pick a save to overwrite, or make a new one · {key:cancel}: back",
            Page::Load => "{key:confirm} / click: load · Del: delete · {key:cancel}: back",
            Page::Name => "Type a name · Enter: save · Esc: back",
            Page::Options => "{key:left}/{key:right} or click < >: change · saved right away · {key:cancel}: back",
            Page::Controls if self.menu.capture.is_some() => "Press a new key or button · Esc / {pad:b}: keep them",
            Page::Controls if input::device() == input::Device::Pad => "{pad:a}: set a button · {pad:x}: clear the button · saved right away · {pad:b}: back",
            Page::Controls => "Enter: add a key or a button · Del: clear the keys · saved right away · Esc: back",
            Page::Defeated => "{key:up}/{key:down}: choose · {key:confirm} / click: confirm",
            Page::Achievements => "{key:up}/{key:down} or wheel: browse · {key:cancel}: back",
            _ => "{key:confirm} / click · {key:cancel}: back",
        };
        // Controls page: what the selected action's keys clash with.
        let clash = match (&self.menu.page, rows.get(self.menu.sel)) {
            (Page::Controls, Some((Row::Bind(a), _, _))) if self.menu.message.is_none() => self.conflict_text(*a),
            _ => None,
        };
        if let Some(msg) = self.menu.message.as_ref().or(clash.as_ref()) {
            draw_text(msg, p.x + 16.0, p.y + p.h - 34.0, 18.0, Color::new(1.0, 0.5, 0.5, 1.0));
        }
        super::glyphs::draw_hint(hint, p.x + 16.0, p.y + p.h - 12.0, 16.0, GRAY);
    }
}

/// `text` cut to `width` pixels at font size `size` ("..." at the end when cut).
fn fit(text: &str, size: u16, width: f32) -> String {
    if measure_text(text, None, size, 1.0).width <= width {
        return text.to_string();
    }
    let mut cut: String = text.to_string();
    while !cut.is_empty() && measure_text(format!("{cut}...").as_str(), None, size, 1.0).width > width {
        cut.pop();
    }
    format!("{}...", cut.trim_end())
}

/// Click areas of the "<" (previous value) and ">" (next value) arrows of an Options row.
fn opt_arrows(r: Rect) -> (Rect, Rect) {
    const AW: f32 = 30.0; // arrow width
    const VALUE_W: f32 = 100.0; // room for the value between the arrows
    let next = Rect::new(r.x + r.w - AW - 6.0, r.y, AW, r.h);
    let prev = Rect::new(next.x - VALUE_W - AW, r.y, AW, r.h);
    (prev, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_names_and_labels() {
        assert_eq!(sanitize("  My/../save: 1 "), "Mysave 1");
        assert_eq!(sanitize(&"x".repeat(50)).len(), MAX_NAME);
        assert_eq!(display_name("quick"), "Quicksave");
        assert_eq!(play_time(3725.0), "1:02:05");
        assert_eq!(play_time(65.0), "1:05");
        assert_eq!(ago(now_secs()), "just now");
        assert_eq!(ago(now_secs() - 7200), "2 h ago");
    }
}
