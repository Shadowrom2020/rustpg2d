//! Shared look of the editor: the Phosphor icon font, icon helpers, ribbon groups and tool
//! buttons with uniform tooltips (name, shortcut, how to use).

use egui_macroquad::egui::{self, Color32, Response, RichText, Ui};
pub use egui_phosphor::regular as ph;

/// Installs the icon font next to egui's default fonts (once, at startup).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

/// "<icon> text" (shortcuts in it as on this platform, see [`keys`]).
pub fn t(icon: &str, text: &str) -> String {
    format!("{icon} {}", keys(text))
}

/// Shortcut texts for this platform: the editor's texts say "Ctrl+", which is Cmd on macOS.
pub fn keys(s: &str) -> std::borrow::Cow<'_, str> {
    if cfg!(target_os = "macos") && s.contains("Ctrl") {
        std::borrow::Cow::Owned(s.replace("Ctrl+", "Cmd+").replace("Ctrl-", "Cmd-").replace("Ctrl ", "Cmd "))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// The command modifier of the editor's own shortcuts is held: Ctrl, on macOS Cmd (or Ctrl).
pub fn command_down() -> bool {
    use macroquad::input::{is_key_down, KeyCode};
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) || (cfg!(target_os = "macos") && (is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper)))
}

/// Tooltip of a tool or action: name with shortcut, then how to use it.
pub fn tip(r: Response, name: &str, key: &str, help: &str) -> Response {
    r.on_hover_ui(|ui| {
        ui.set_max_width(320.0);
        let title = if key.is_empty() { name.to_string() } else { format!("{name}   {}", keys(key)) };
        ui.label(RichText::new(title).strong());
        if !help.is_empty() {
            ui.label(keys(help));
        }
    })
}

/// An icon-only tool button, highlighted when selected (ribbons and tool rows).
pub fn tool_button(ui: &mut Ui, selected: bool, icon: &str, name: &str, key: &str, help: &str) -> Response {
    let r = ui.add_sized([30.0, 26.0], egui::SelectableLabel::new(selected, RichText::new(icon).size(17.0)));
    tip(r, name, key, help)
}

/// An icon-only toggle (view options).
pub fn toggle(ui: &mut Ui, on: &mut bool, icon: &str, name: &str, key: &str, help: &str) -> Response {
    let r = ui.add_sized([30.0, 26.0], egui::SelectableLabel::new(*on, RichText::new(icon).size(17.0)));
    if r.clicked() {
        *on = !*on;
    }
    tip(r, name, key, help)
}

/// An icon-only action button with a tooltip.
pub fn icon_button(ui: &mut Ui, enabled: bool, icon: &str, name: &str, key: &str, help: &str) -> Response {
    let r = ui.add_enabled(enabled, egui::Button::new(RichText::new(icon).size(17.0)).min_size(egui::vec2(30.0, 26.0)));
    tip(r, name, key, help)
}

/// A ribbon group: its widgets in a row with a small caption underneath, then a separator.
pub fn group(ui: &mut Ui, caption: &str, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.horizontal(|ui| add(ui));
        ui.label(RichText::new(caption).small().color(Color32::from_gray(140)));
    });
    ui.separator();
}

/// A tab label with icon (mode switcher, Database tabs, library kinds).
pub fn tab(ui: &mut Ui, selected: bool, icon: &str, text: &str) -> Response {
    ui.selectable_label(selected, t(icon, text))
}

/// Icon of an asset kind (Database tabs, library filters and cards).
pub fn kind_icon(k: rustpg2d_engine::refactor::RefKind) -> &'static str {
    use rustpg2d_engine::refactor::RefKind;
    match k {
        RefKind::Tileset => ph::GRID_FOUR,
        RefKind::Sheet => ph::IMAGE,
        RefKind::Sound => ph::SPEAKER_HIGH,
        RefKind::Item => ph::SWORD,
        RefKind::Prop => ph::TREASURE_CHEST,
        RefKind::Character => ph::USERS,
        RefKind::Structure => ph::HOUSE,
        RefKind::Skill => ph::STAR,
        RefKind::Spell => ph::MAGIC_WAND,
        RefKind::Recipe => ph::HAMMER,
        RefKind::Quest => ph::SCROLL,
        RefKind::Achievement => ph::TROPHY,
        RefKind::Race => ph::USERS_THREE,
        RefKind::Model => ph::CUBE,
        RefKind::Map => ph::MAP_TRIFOLD,
    }
}
