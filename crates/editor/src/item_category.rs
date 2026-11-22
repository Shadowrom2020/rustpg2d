//! The item categories' filter picker for the editor's lists (the Database's item list, the asset
//! library's browse view and *Add from library* dialog); the categories themselves are the
//! engine's (`rustpg2d_engine::item_category`).

use egui_macroquad::egui;
pub use rustpg2d_engine::item_category::*;

/// The filter picker: *All items*, the groups and their categories, with how many items each
/// has (`cats`: the categories of the items listed). Returns true when it changed.
pub fn picker(ui: &mut egui::Ui, id: &str, filter: &mut ItemFilter, cats: &[ItemCat]) -> bool {
    let count = |f: ItemFilter| cats.iter().filter(|c| f.keeps(**c)).count();
    let before = *filter;
    egui::ComboBox::from_id_salt(id).width(190.0).selected_text(format!("{} ({})", filter.label(), count(*filter))).show_ui(ui, |ui| {
        ui.selectable_value(filter, ItemFilter::All, format!("All items ({})", cats.len()));
        let mut last_group = None;
        for c in ItemCat::ALL {
            let n = count(ItemFilter::Cat(c));
            if c.group() != last_group {
                last_group = c.group();
                if let Some(g) = c.group() {
                    ui.separator();
                    ui.selectable_value(filter, ItemFilter::Group(g), egui::RichText::new(format!("{} ({})", g.label(), count(ItemFilter::Group(g)))).strong());
                } else {
                    ui.separator();
                }
            }
            if n == 0 {
                continue;
            }
            let indent = if c.group().is_some() { "    " } else { "" };
            ui.selectable_value(filter, ItemFilter::Cat(c), format!("{indent}{} ({n})", c.label()));
        }
    })
    .response
    .on_hover_text("The SRD's categories of equipment and magic items, worked out from what each item is; Other = the project's own things");
    *filter != before
}
