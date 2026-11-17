//! The UI skin form's live preview: the game's own skin drawing (`runtime::skin::draw_preview`:
//! a menu, the HUD, a toast and a dialog box) rendered into a texture with the skin being edited
//! and the project's fonts, so it shows exactly what the game will.

use egui::{pos2, vec2, Color32, Rect, Sense, Ui};
use egui_macroquad::egui;
use macroquad::prelude::{pop_camera_state, push_camera_state, render_target, set_camera, Camera2D, FilterMode, RenderTarget};
use rustpg2d_engine::fonts::{FontDef, FontRoles};
use rustpg2d_engine::render::Assets;
use rustpg2d_engine::runtime::skin;
use rustpg2d_engine::skin::SkinDef;

/// The preview's size in game pixels.
const W: f32 = 660.0;
const H: f32 = 300.0;

#[derive(Default)]
pub struct SkinPreview {
    rt: Option<RenderTarget>,
    /// The skin and the art generation installed last (installed again when either changes).
    installed: Option<(SkinDef, u64)>,
    /// The fonts installed last.
    fonts: Option<(FontRoles, Vec<FontDef>)>,
}

impl SkinPreview {
    pub fn ui(&mut self, ui: &mut Ui, def: &SkinDef, assets: &Assets, fonts: &FontRoles, font_defs: &[FontDef], root: &std::path::Path) {
        let scale = (ui.available_width() / W).clamp(0.5, 1.0);
        let (rect, _) = ui.allocate_exact_size(vec2(W * scale, H * scale), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::from_gray(24));
        if crate::ui::headless() {
            return;
        }
        let used: Vec<FontDef> = font_defs.iter().filter(|d| fonts.ids().any(|id| *id == d.id)).cloned().collect();
        if self.fonts.as_ref() != Some(&(fonts.clone(), used.clone())) {
            let files = used.iter().filter_map(|d| std::fs::read(root.join(&d.file)).ok().map(|b| (d.clone(), b))).collect();
            rustpg2d_engine::runtime::text::install(files, fonts);
            self.fonts = Some((fonts.clone(), used));
        }
        if self.installed.as_ref() != Some(&(def.clone(), assets.generation)) {
            skin::install(Some(def), &|n| assets.sheets.get(&n.sheet).filter(|s| n.frame < s.count.max(1)).map(|s| (s.texture.clone(), s.src_rect(n.frame))));
            self.installed = Some((def.clone(), assets.generation));
        }
        let rt = self
            .rt
            .get_or_insert_with(|| {
                let rt = render_target(W as u32, H as u32);
                rt.texture.set_filter(FilterMode::Nearest);
                rt
            })
            .clone();
        push_camera_state();
        let mut cam = Camera2D::from_display_rect(macroquad::prelude::Rect::new(0.0, 0.0, W, H));
        cam.zoom.y = -cam.zoom.y;
        cam.render_target = Some(rt.clone());
        set_camera(&cam);
        skin::draw_preview(W, H);
        pop_camera_state();
        let raw = unsafe { macroquad::window::get_internal_gl().quad_context.texture_raw_id(rt.texture.raw_miniquad_id()) };
        #[allow(irrefutable_let_patterns)]
        let macroquad::miniquad::RawId::OpenGl(gl) = raw
        else {
            return;
        };
        ui.painter().image(egui::TextureId::User(gl as u64), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}
