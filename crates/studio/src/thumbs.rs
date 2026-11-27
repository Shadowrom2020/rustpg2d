//! Pictures of assets in the editor's UI: an item's icon, a prop's picture, a character as the
//! game draws it. Nothing is copied or drawn again for the UI: egui shows the engine's own
//! textures (a sprite sheet, the page of the model atlas a pose was rendered into) with the part
//! that is the asset's.

use std::collections::HashMap;

use editor_core::Workspace;
use eframe::egui;
use eframe::egui_wgpu::RenderState;
use gfx::texture::Texture2D;
use rustpg2d_engine::defs::{Appearance, SpriteRef};
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::render::model::{Gear, PoseReq};

/// A picture: part of a texture egui can draw.
#[derive(Clone, Copy)]
pub struct Thumb {
    pub texture: egui::TextureId,
    pub uv: egui::Rect,
    /// Its size in the texture's pixels.
    pub size: egui::Vec2,
}

impl Thumb {
    /// Paints it into `rect`, as big as fits, standing on the bottom's middle.
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        let k = (rect.width() / self.size.x.max(1.0)).min(rect.height() / self.size.y.max(1.0));
        let size = self.size * k;
        let dest = egui::Rect::from_min_size(egui::pos2(rect.center().x - size.x / 2.0, rect.max.y - size.y), size);
        ui.painter().image(self.texture, dest, self.uv, egui::Color32::WHITE);
    }
}

/// A rendered pose for a large preview.
#[derive(Clone, Copy)]
pub struct Pose {
    pub thumb: Thumb,
    /// Where the figure stands, in the pose's pixels.
    pub feet: egui::Pos2,
    /// From its feet to the top of what is drawn.
    pub height: f32,
}

/// A sheet's frames for a frame picker: how many, and in how many columns.
#[derive(Clone, Copy)]
pub struct SheetFrames {
    pub count: u32,
    pub columns: u32,
}

/// The engine's textures as egui knows them. Textures that weren't shown for a while are given
/// back (a page of the model atlas that was thrown away, a reloaded sheet).
#[derive(Default)]
pub struct Thumbs {
    shown: HashMap<usize, (Texture2D, egui::TextureId, u64)>,
    /// Ground materials are pictures in memory: their textures, made when first shown.
    materials: HashMap<String, Texture2D>,
    /// Sheets as egui blends them (colours multiplied by their alpha), by the sheet's texture:
    /// made when a sheet is first shown. The engine's own textures keep colour and alpha apart,
    /// which egui would draw with bright fringes around soft edges.
    sheets: HashMap<usize, Texture2D>,
    frame: u64,
    generation: u64,
}

/// Frames a texture stays known to egui without being shown.
const KEEP_FRAMES: u64 = 600;

impl Thumbs {
    /// Once a frame: forgets what wasn't shown for a while, and everything when the project's
    /// art was loaded again.
    pub fn begin_frame(&mut self, ws: &Workspace, rs: &RenderState) {
        self.frame += 1;
        // (The model atlas renders a few new poses a frame: a long list fills in over some.)
        ws.assets.models.begin_frame();
        let reloaded = ws.assets.generation != self.generation;
        self.generation = ws.assets.generation;
        if reloaded {
            self.materials.clear();
            self.sheets.clear();
        }
        let frame = self.frame;
        let old: Vec<usize> = self.shown.iter().filter(|(_, (_, _, last))| reloaded || frame - last > KEEP_FRAMES).map(|(k, _)| *k).collect();
        if !old.is_empty() {
            let mut renderer = rs.renderer.write();
            for key in old {
                if let Some((_, id, _)) = self.shown.remove(&key) {
                    renderer.free_texture(&id);
                }
            }
        }
    }

    fn id(&mut self, rs: &RenderState, texture: &Texture2D, smooth: bool) -> egui::TextureId {
        let frame = self.frame;
        let entry = self.shown.entry(texture.key()).or_insert_with(|| {
            let filter = if smooth { eframe::wgpu::FilterMode::Linear } else { eframe::wgpu::FilterMode::Nearest };
            (texture.clone(), rs.renderer.write().register_native_texture(&rs.device, texture.view(), filter), frame)
        });
        entry.2 = frame;
        entry.1
    }

    /// A sprite: its cells of its sheet.
    pub fn sprite(&mut self, ws: &Workspace, rs: &RenderState, sprite: &SpriteRef) -> Option<Thumb> {
        let sheet = ws.assets.sheets.get(&sprite.sheet)?;
        let (w, h) = sprite.cells();
        let src = sheet.region(sprite.frame, w, h);
        let (tw, th) = (sheet.texture.width(), sheet.texture.height());
        if src.w <= 0.0 || src.h <= 0.0 || src.x + src.w > tw + 0.5 || src.y + src.h > th + 0.5 {
            return None;
        }
        let uv = egui::Rect::from_min_max(egui::pos2(src.x / tw, src.y / th), egui::pos2((src.x + src.w) / tw, (src.y + src.h) / th));
        let texture = self
            .sheets
            .entry(sheet.texture.key())
            .or_insert_with(|| {
                let mut bytes = sheet.image.bytes.clone();
                for px in bytes.as_chunks_mut::<4>().0 {
                    let a = px[3] as u32;
                    *px = [(px[0] as u32 * a / 255) as u8, (px[1] as u32 * a / 255) as u8, (px[2] as u32 * a / 255) as u8, px[3]];
                }
                Texture2D::from_rgba8(sheet.image.width, sheet.image.height, &bytes)
            })
            .clone();
        // Pixel art stays crisp; big painted pictures are shown smaller than they are: smoothed.
        let smooth = src.w.max(src.h) > 80.0;
        Some(Thumb { texture: self.id(rs, &texture, smooth), uv, size: egui::vec2(src.w, src.h) })
    }

    /// How many frames a sheet has, for a picker of one.
    pub fn frames(&self, ws: &Workspace, sheet: &str) -> Option<SheetFrames> {
        ws.assets.sheets.get(sheet).map(|s| SheetFrames { count: s.count, columns: s.columns.max(1) })
    }

    /// A figure standing still, facing the viewer, as the game draws it.
    pub fn figure(&mut self, ws: &Workspace, rs: &RenderState, look: &Appearance) -> Option<Thumb> {
        let models = &ws.assets.models;
        let look = models.look_of(look);
        let req = PoseReq { look: &look, gear: &Gear::default(), anim: "idle", time: 0.0, once: false, facing: Facing::Down, turn: 0, bust: false };
        let cell = models.pose(&ws.project.db, &ws.assets.sheets, &req)?;
        // The part of the pose the figure covers: a person and a cow both fill their box.
        let (texture, uv, _) = models.cell_texture(&cell)?;
        let (pw, ph) = (texture.width(), texture.height());
        let d = cell.drawn;
        let min = egui::pos2(uv.x + d.x / pw, uv.y + d.y / ph);
        Some(Thumb { texture: self.id(rs, &texture, true), uv: egui::Rect::from_min_size(min, egui::vec2(d.w / pw, d.h / ph)), size: egui::vec2(d.w, d.h) })
    }

    /// A pose of a look with what it wears and holds, for a large preview: all of the rendered
    /// pose, where its feet are in it and how tall the figure is drawn (pixels of the pose):
    /// held items and swung limbs reach beyond the figure, the feet keep their place.
    pub fn pose(&mut self, ws: &Workspace, rs: &RenderState, req: &PoseReq) -> Option<Pose> {
        let models = &ws.assets.models;
        let cell = models.pose(&ws.project.db, &ws.assets.sheets, req)?;
        let (texture, uv, _) = models.cell_texture(&cell)?;
        let (pw, ph) = (texture.width(), texture.height());
        let thumb = Thumb { texture: self.id(rs, &texture, true), uv: egui::Rect::from_min_size(egui::pos2(uv.x, uv.y), egui::vec2(uv.w, uv.h)), size: egui::vec2(uv.w * pw, uv.h * ph) };
        Some(Pose { thumb, feet: egui::pos2(cell.feet.x, cell.feet.y), height: (cell.feet.y - cell.drawn.y).max(1.0) })
    }

    /// A ground material: the first frame of its texture.
    fn material(&mut self, ws: &Workspace, rs: &RenderState, id: &str) -> Option<Thumb> {
        let image = ws.assets.materials.get(id)?;
        let frames = ws.assets.material_frames.get(id).copied().unwrap_or(1).max(1) as f32;
        let texture = self.materials.entry(id.to_string()).or_insert_with(|| Texture2D::from_image(image)).clone();
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0 / frames, 1.0));
        Some(Thumb { texture: self.id(rs, &texture, false), uv, size: egui::vec2(image.width as f32 / frames, image.height as f32) })
    }

    /// The picture of a definition; `None` for what has none (a sound, a quest).
    pub fn of(&mut self, ws: &Workspace, rs: &RenderState, kind: RefKind, id: &str) -> Option<Thumb> {
        let db = &ws.project.db;
        match kind {
            RefKind::Item => self.sprite(ws, rs, &db.item(id)?.icon),
            RefKind::Spell => self.sprite(ws, rs, &db.spell(id)?.icon),
            RefKind::Achievement => self.sprite(ws, rs, &db.achievements.iter().find(|a| a.id == id)?.icon),
            RefKind::Prop => {
                let p = db.prop(id)?;
                self.sprite(ws, rs, &p.sprite).or_else(|| p.states.first().and_then(|s| self.sprite(ws, rs, &s.sprite)))
            }
            RefKind::Sheet => self.sprite(ws, rs, &SpriteRef::new(id, 0)),
            RefKind::Character => {
                let c = db.character(id)?;
                self.figure(ws, rs, &c.appearance).or_else(|| self.sprite(ws, rs, &c.portrait))
            }
            RefKind::Model => self.figure(ws, rs, &Appearance { model: id.to_string(), ..Default::default() }),
            RefKind::Race => {
                let r = db.race(id)?;
                let body = r.bodies_male.first().or(r.bodies_female.first())?;
                self.figure(ws, rs, &Appearance { model: body.clone(), ..Default::default() })
            }
            RefKind::Material => self.material(ws, rs, id),
            _ => None,
        }
    }
}

/// The pictures a form shows, of the project it edits.
pub struct Pics<'a> {
    pub thumbs: &'a mut Thumbs,
    pub ws: &'a Workspace,
    pub rs: &'a RenderState,
}

impl crate::form::Pictures for Pics<'_> {
    fn sprite(&mut self, sprite: &SpriteRef) -> Option<Thumb> {
        self.thumbs.sprite(self.ws, self.rs, sprite)
    }

    fn of(&mut self, kind: RefKind, id: &str) -> Option<Thumb> {
        self.thumbs.of(self.ws, self.rs, kind, id)
    }

    fn frames(&self, sheet: &str) -> Option<SheetFrames> {
        self.thumbs.frames(self.ws, sheet)
    }
}

/// A box of `side` points with the picture `thumb` gives in it; the picture is only asked for
/// while the box can be seen (a long list asks for what is on screen).
pub fn boxed(ui: &mut egui::Ui, side: f32, thumb: impl FnOnce() -> Option<Thumb>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        if let Some(t) = thumb() {
            t.paint(ui, rect);
        }
    }
    response
}

/// Whether definitions of `kind` have pictures ([`Thumbs::of`]).
pub fn has_pictures(kind: RefKind) -> bool {
    matches!(kind, RefKind::Item | RefKind::Spell | RefKind::Achievement | RefKind::Prop | RefKind::Sheet | RefKind::Character | RefKind::Model | RefKind::Race | RefKind::Material)
}

/// A card to click: a picture with a name under it.
pub fn card(ui: &mut egui::Ui, name: &str, id: &str, selected: bool, mark: Option<egui::Color32>, thumb: impl FnOnce() -> Option<Thumb>) -> egui::Response {
    let size = egui::vec2(104.0, 112.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let visuals = ui.style().interact(&response);
    let fill = if selected {
        ui.visuals().selection.bg_fill.gamma_multiply(0.45)
    } else if response.hovered() {
        visuals.bg_fill
    } else {
        ui.visuals().faint_bg_color
    };
    ui.painter().rect_filled(rect, 4.0, fill);
    if selected {
        ui.painter().rect_stroke(rect, 4.0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
    }
    let picture = egui::Rect::from_min_size(rect.min + egui::vec2(12.0, 6.0), egui::vec2(80.0, 80.0));
    if let Some(t) = thumb() {
        t.paint(ui, picture);
    }
    // (Something is wrong with it, or nothing uses it: a dot in its corner.)
    if let Some(color) = mark {
        ui.painter().circle_filled(rect.right_top() + egui::vec2(-10.0, 10.0), 4.0, color);
    }
    let font = egui::TextStyle::Small.resolve(ui.style());
    let galley = ui.painter().layout(name.to_string(), font, ui.visuals().text_color(), rect.width() - 8.0);
    // (One line: a long name is cut at the card's edge.)
    let pos = egui::pos2(rect.center().x - galley.size().x.min(rect.width() - 8.0) / 2.0, rect.max.y - 22.0);
    ui.painter().with_clip_rect(egui::Rect::from_min_max(egui::pos2(rect.min.x + 4.0, rect.max.y - 24.0), egui::pos2(rect.max.x - 4.0, rect.max.y - 8.0))).galley(pos, galley, ui.visuals().text_color());
    response.on_hover_text(if name == id { name.to_string() } else { format!("{name}  ({id})") })
}
