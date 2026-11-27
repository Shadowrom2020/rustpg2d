//! The system's file dialogs: a file or a folder picked instead of its place typed. A dialog
//! runs beside the editor (which keeps drawing); whoever asked takes the answer in a later
//! frame, by its key. One dialog at a time.
//!
//! Scripted runs and tests never open a real dialog: `answer_next` says what the next one
//! answers (the script's `pickanswer PATH`), and without that it answers nothing.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use eframe::egui;

/// Who asks (and so who takes the answer).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// The home screen: a project's file, a project's folder, where a new game is kept.
    OpenProject,
    OpenFolder,
    NewProject,
    /// *New…*: the file a new thing starts from.
    NewFromFile,
    /// Publishing: the folder the game is exported into.
    PublishOut,
}

/// What is asked for.
pub enum Want {
    /// A file there is, of one of these kinds: what they are called, their extensions.
    File(&'static str, &'static [&'static str]),
    Folder,
    /// A file to be written: its kind, its extension, the name offered.
    Save(&'static str, &'static str, String),
}

#[derive(Default)]
struct State {
    waiting: Option<(Key, Receiver<Option<PathBuf>>)>,
    done: Option<(Key, PathBuf)>,
    /// Frames the answer has waited to be taken.
    age: u32,
    /// What the next dialog answers without opening (scripts and tests).
    answer: Option<PathBuf>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Whether real dialogs open: not in tests, not while a script drives the editor.
fn real() -> bool {
    !cfg!(test) && std::env::var_os("RUSTPG2D_STUDIO_SCRIPT").is_none()
}

/// The next dialog answers `path` at once, without opening.
pub fn answer_next(path: impl Into<PathBuf>) {
    STATE.with_borrow_mut(|s| s.answer = Some(path.into()));
}

/// Whether a dialog is open (its button waits).
pub fn waiting() -> bool {
    STATE.with_borrow(|s| s.waiting.is_some())
}

/// Opens a dialog titled `title`, starting in `start` (a folder, or a file whose folder it is).
pub fn ask(ctx: &egui::Context, key: Key, title: &str, want: Want, start: Option<PathBuf>) {
    if waiting() {
        return;
    }
    if let Some(path) = STATE.with_borrow_mut(|s| s.answer.take()) {
        STATE.with_borrow_mut(|s| (s.done, s.age) = (Some((key, path)), 0));
        ctx.request_repaint();
        return;
    }
    if !real() {
        println!("[pick] {key:?}: a dialog would open here ({title})");
        return;
    }
    let (tx, rx) = channel();
    let (title, ctx2) = (title.to_string(), ctx.clone());
    let start = start.map(|p| if p.is_dir() { p } else { p.parent().map(PathBuf::from).unwrap_or(p) }).filter(|p| p.is_dir());
    let job = move || {
        let mut d = rfd::FileDialog::new().set_title(title);
        if let Some(dir) = start {
            d = d.set_directory(dir);
        }
        let answer = match want {
            Want::File(name, exts) => d.add_filter(name, exts).pick_file(),
            Want::Folder => d.pick_folder(),
            Want::Save(name, ext, offered) => d.add_filter(name, &[ext]).set_file_name(offered).save_file(),
        };
        let _ = tx.send(answer);
        ctx2.request_repaint();
    };
    // macOS shows dialogs from the main thread only (AppKit): there the editor waits for it.
    if cfg!(target_os = "macos") {
        job();
    } else {
        std::thread::spawn(job);
    }
    STATE.with_borrow_mut(|s| s.waiting = Some((key, rx)));
}

/// Once a frame: takes a closed dialog's answer, and forgets an answer nobody took (whoever
/// asked isn't shown any more).
pub fn frame() {
    STATE.with_borrow_mut(|s| {
        if s.done.is_some() {
            s.age += 1;
            if s.age > 3 {
                s.done = None;
            }
        }
        let Some((key, rx)) = &s.waiting else { return };
        match rx.try_recv() {
            Ok(answer) => {
                (s.done, s.age) = (answer.map(|p| (*key, p)), 0);
                s.waiting = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => s.waiting = None,
        }
    });
}

/// The answer to `key`'s dialog, once: `None` while it is open, and when it was closed
/// without one.
pub fn take(key: Key) -> Option<PathBuf> {
    STATE.with_borrow_mut(|s| if s.done.as_ref().is_some_and(|(k, _)| *k == key) { s.done.take().map(|(_, p)| p) } else { None })
}

/// A *Browse…* button for `key`: opens the dialog when clicked; returns what was picked, in
/// the frame the answer is there.
pub fn browse(ui: &mut egui::Ui, key: Key, label: &str, title: &str, want: impl FnOnce() -> Want, start: Option<PathBuf>) -> Option<PathBuf> {
    let open = waiting();
    if ui.add_enabled(!open, egui::Button::new(label)).on_hover_text(title).on_disabled_hover_text("A file dialog is open").clicked() {
        ask(ui.ctx(), key, title, want(), start);
    }
    take(key)
}
