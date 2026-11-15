//! The material form's preview: a patch of the material on a neighbour material, drawn by the
//! game's ground shader (edges, raggedness, rim, priority and flow as on a map), following the
//! texture while it is being drawn in the pixel editor.

use std::collections::HashMap;

use egui::{pos2, vec2, Color32, Rect, Sense, Ui};
use egui_macroquad::egui;
use macroquad::prelude::{
    clear_background, pop_camera_state, push_camera_state, render_target, set_camera, Camera2D, Color, FilterMode, Image, RenderTarget, WHITE,
};
use rustpg2d_engine::defs::Database;
use rustpg2d_engine::ground::MaterialDef;
use rustpg2d_engine::imageio::Rgba;
use rustpg2d_engine::map::Map;
use rustpg2d_engine::render::ground::GroundRenderer;
use rustpg2d_engine::render::Assets;

/// Size of the sample in tiles.
const W: u32 = 6;
const H: u32 = 3;
/// Screen pixels per world pixel.
const ZOOM: f32 = 2.0;

#[derive(Default)]
pub struct MaterialPreview {
    renderer: GroundRenderer,
    rt: Option<RenderTarget>,
    map: Option<Map>,
    /// The texture images the sample is drawn with, and what they were made from.
    images: HashMap<String, Image>,
    key: Option<(String, String, String, u64, u64)>,
    generation: u64,
    /// The material around the patch (empty: chosen automatically).
    pub neighbour: String,
    /// The patch drawn the other way round: the neighbour inside the material.
    pub inverted: bool,
}

impl MaterialPreview {
    /// The neighbour shown with `def`: the chosen one, else grass (dirt for grass itself), else the
    /// first other material.
    fn neighbour_of(&self, def: &MaterialDef, db: &Database) -> Option<MaterialDef> {
        let pick = |id: &str| db.materials.iter().find(|m| m.id == id && m.id != def.id).cloned();
        pick(&self.neighbour).or_else(|| pick("grass")).or_else(|| pick("dirt")).or_else(|| db.materials.iter().find(|m| m.id != def.id).cloned())
    }

    /// The preview with its controls. `drawing` = the texture being drawn in the pixel editor (its
    /// path, picture and change counter), used instead of the saved file.
    pub fn ui(&mut self, ui: &mut Ui, def: &MaterialDef, db: &Database, assets: &Assets, drawing: Option<(&str, &Rgba, u64)>) {
        let neighbour = self.neighbour_of(def, db);
        ui.horizontal(|ui| {
            ui.label("Preview next to");
            let shown = neighbour.as_ref().map(|n| if n.name.is_empty() { n.id.clone() } else { n.name.clone() }).unwrap_or_else(|| "nothing".into());
            egui::ComboBox::from_id_salt("material_preview_neighbour").selected_text(shown).height(320.0).show_ui(ui, |ui| {
                for m in db.materials.iter().filter(|m| m.id != def.id) {
                    let label = if m.name.is_empty() { m.id.clone() } else { m.name.clone() };
                    if ui.selectable_label(neighbour.as_ref().is_some_and(|n| n.id == m.id), label).clicked() {
                        self.neighbour = m.id.clone();
                    }
                }
            });
            ui.checkbox(&mut self.inverted, "Swap").on_hover_text("The other material inside this one: both sides of the edge");
        });
        let size = vec2((W * 32) as f32 * ZOOM, (H * 32) as f32 * ZOOM);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::from_gray(24));
        if crate::ui::headless() {
            return;
        }
        let Some(neighbour) = neighbour else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "(another material is needed next to it)",
                egui::FontId::proportional(13.0),
                Color32::GRAY,
            );
            return;
        };
        self.draw(ui, rect, def, &neighbour, assets, drawing);
    }

    fn draw(&mut self, ui: &Ui, rect: Rect, def: &MaterialDef, neighbour: &MaterialDef, assets: &Assets, drawing: Option<(&str, &Rgba, u64)>) {
        // Keep the texture images current: the drawing in progress, else the loaded file.
        let drawn = drawing.filter(|(rel, ..)| !def.texture.is_empty() && *rel == def.texture);
        let key = (def.id.clone(), def.texture.clone(), neighbour.id.clone(), assets.generation, drawn.map_or(u64::MAX, |d| d.2));
        if self.key.as_ref() != Some(&key) {
            self.images.clear();
            if let Some(img) = assets.materials.get(&neighbour.id) {
                self.images.insert(neighbour.id.clone(), img.clone());
            }
            let own = match drawn {
                Some((_, px, _)) => Some(Image { bytes: px.px.clone(), width: px.w as u16, height: px.h as u16 }),
                None => assets.materials.get(&def.id).cloned(),
            };
            if let Some(img) = own {
                self.images.insert(def.id.clone(), img);
            }
            self.key = Some(key);
            self.generation += 1;
        }
        // The sample: a round patch and a square one (curves and corners).
        let (inside, outside) = if self.inverted { (1, 2) } else { (2, 1) };
        let map = self.map.get_or_insert_with(|| Map::new("material_preview", W, H));
        map.materials = vec![neighbour.id.clone(), def.id.clone()];
        if map.ground.get(0, 0) != outside {
            map.ground.fill(outside);
            map.ground.paint_circle((52.0, 48.0), 30.0, inside);
            map.ground.paint_rect((104.0, 20.0), (168.0, 76.0), inside);
        }
        let db = Database { materials: vec![neighbour.clone(), def.clone()], ..Default::default() };
        let (pw, ph) = ((W * 32) as f32, (H * 32) as f32);
        let rt = self
            .rt
            .get_or_insert_with(|| {
                let rt = render_target(pw as u32, ph as u32);
                rt.texture.set_filter(FilterMode::Nearest);
                rt
            })
            .clone();
        push_camera_state();
        let mut cam = Camera2D::from_display_rect(macroquad::prelude::Rect::new(0.0, 0.0, pw, ph));
        cam.zoom.y = -cam.zoom.y;
        cam.render_target = Some(rt.clone());
        set_camera(&cam);
        clear_background(Color::new(0.1, 0.1, 0.12, 1.0));
        self.renderer.draw_with(map, &db, &self.images, self.generation, macroquad::prelude::Rect::new(0.0, 0.0, pw, ph), WHITE);
        pop_camera_state();
        // The render target's OpenGL texture: egui-miniquad draws `TextureId::User` ids as raw GL textures.
        let raw = unsafe { macroquad::window::get_internal_gl().quad_context.texture_raw_id(rt.texture.raw_miniquad_id()) };
        #[allow(irrefutable_let_patterns)]
        let macroquad::miniquad::RawId::OpenGl(gl) = raw
        else {
            return;
        };
        ui.painter().image(egui::TextureId::User(gl as u64), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        // Flowing and animated materials move: keep drawing.
        let moves = |m: &MaterialDef| m.scroll != [0.0, 0.0] || (m.frames > 1 && m.fps > 0.0);
        if moves(def) || moves(neighbour) {
            ui.ctx().request_repaint();
        }
    }
}
