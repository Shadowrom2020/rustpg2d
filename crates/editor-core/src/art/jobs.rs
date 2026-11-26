//! Generate art, the part without a window: the references a request takes from the project,
//! starting, redoing and applying jobs.

use rustpg2d_engine::defs::{CharacterDef, ItemDef, SheetDef, SpriteRef};
use rustpg2d_engine::imageio::Rgba;

use super::{meshy, ArtKind, ArtRequest, Job, Locked, References, Stage};
use crate::workspace::Workspace;

impl Workspace {
    /// The pictures art requests refer to: the project's item icons (the style of new icons).
    pub fn art_references(&self) -> References {
        let db = &self.project.db;
        let icons = db.sheet("items").and_then(|d| {
            let im = Rgba::decode(&std::fs::read(self.project.root.join(&d.image)).ok()?).ok()?;
            let n = (im.w / 32).min(8);
            let mut strip = Rgba::new(n * 32 * 4, 32 * 4);
            for y in 0..strip.h {
                for x in 0..strip.w {
                    let c = im.get((x / 4) as i32, (y / 4) as i32);
                    let a = c[3] as u32;
                    let mix = |i: usize| ((c[i] as u32 * a + 235 * (255 - a)) / 255) as u8;
                    strip.set(x as i32, y as i32, [mix(0), mix(1), mix(2), 255]);
                }
            }
            Some(meshy::data_uri(&strip.encode().ok()?))
        });
        References { icons, ..Default::default() }
    }

    /// The references of one request: 3D characters from a figure get its front, side and back
    /// frames, an item's 3D model its icon.
    pub fn art_references_for(&self, req: &ArtRequest) -> anyhow::Result<References> {
        let mut refs = self.art_references();
        let db = &self.project.db;
        let sheet_image = |id: &str| -> anyhow::Result<(Rgba, u32, u32)> {
            let d = db.sheet(id).ok_or_else(|| anyhow::anyhow!("no sprite sheet \"{id}\""))?;
            let im = Rgba::decode(&std::fs::read(self.project.root.join(&d.image))?)?;
            Ok((im, d.frame_w.max(1), d.frame_h.max(1)))
        };
        match req.kind {
            ArtKind::Model3d | ArtKind::Creature3d if !req.from.is_empty() => {
                let (im, fw, fh) = sheet_image(&req.from)?;
                if im.h / fh < 8 || im.w / fw < 1 {
                    anyhow::bail!("\"{}\" isn't a figure with standing frames (rows 4-7)", req.from);
                }
                // Standing facing down, left and up (the character layout's idle rows).
                refs.views = [4, 5, 7].iter().map(|row| framed(&im, 0, row * fh, fw, fh, 640)).collect::<anyhow::Result<_>>()?;
            }
            ArtKind::Clips3d => {
                let m = db.model(&req.id).ok_or_else(|| anyhow::anyhow!("no 3D model \"{}\" to add clips to", req.id))?;
                if m.rig.is_some() || !self.assets.models.info(&m.id).is_some_and(|i| i.rigged) {
                    anyhow::bail!("{} has no skeleton of its own (Meshy rigs people): give it a rig template", req.id);
                }
                refs.model = Some(meshy::data_uri_of(&std::fs::read(self.project.root.join(&m.file))?, "application/octet-stream"));
            }
            ArtKind::Prop3d => {
                let prop = db.prop(&req.id).ok_or_else(|| anyhow::anyhow!("no prop \"{}\" to make a 3D model of: make the prop first (its hitbox is the model's footprint)", req.id))?;
                if let Ok((im, fw, fh)) = sheet_image(&prop.sprite.sheet) {
                    let cols = (im.w / fw).max(1);
                    let (x, y) = ((prop.sprite.frame % cols) * fw, (prop.sprite.frame / cols) * fh);
                    refs.icon = framed(&im, x, y, fw * prop.sprite.w.max(1), fh * prop.sprite.h.max(1), 512).ok();
                }
                if refs.icon.is_none() && req.description.trim().len() < 8 {
                    anyhow::bail!("prop \"{}\" has no picture: describe it in a few words", req.id);
                }
            }
            ArtKind::Gear3d => {
                let item = db.item(&req.id).ok_or_else(|| anyhow::anyhow!("no item \"{}\" to make a 3D model of", req.id))?;
                let (im, fw, fh) = sheet_image(&item.icon.sheet).map_err(|_| anyhow::anyhow!("item \"{}\" has no icon to model it from", req.id))?;
                let cols = (im.w / fw).max(1);
                let (x, y) = ((item.icon.frame % cols) * fw, (item.icon.frame / cols) * fh);
                refs.icon = Some(framed(&im, x, y, fw * item.icon.w.max(1), fh * item.icon.h.max(1), 512)?);
            }
            _ => {}
        }
        Ok(refs)
    }

    /// Saves the Meshy settings (with the agent server's).
    pub fn save_art_settings(&self) {
        let s = &self.art_settings;
        crate::files::save_settings(&crate::files::EditorSettings { meshy_model: s.model.clone(), meshy_budget: s.budget, meshy_ask: s.ask, ..crate::files::load_settings() });
    }

    /// Starts a job for a request (the cost confirmed by the caller).
    pub fn start_art(&mut self, mut req: ArtRequest) -> anyhow::Result<u32> {
        if req.kind == ArtKind::Clips3d {
            // The women's clips (picking crops) for a woman's model.
            req.female = self.project.db.model(&req.id).is_some_and(|m| m.female);
        }
        let refs = self.art_references_for(&req)?;
        let settings = self.art_settings.clone();
        let id = self.art.start(req, &settings, refs)?;
        self.set_status(format!("Drawing art (job {id}) with Meshy"));
        Ok(id)
    }

    pub fn redo_art(&mut self, job: u32, picture: &str) -> anyhow::Result<()> {
        let req = self.art.job(job).ok_or_else(|| anyhow::anyhow!("no job {job}"))?.locked().req.clone();
        let refs = self.art_references_for(&req)?;
        let settings = self.art_settings.clone();
        self.art.redo(job, picture, &settings, refs)
    }

    /// Adds a finished job's art to the project as `id`: the image and its sprite sheet (people's
    /// figures with estimated hand marks), and for characters a character, for icons an item, for
    /// props a prop (unless one exists). Returns what was added.
    pub fn apply_art(&mut self, job: u32, id: &str) -> anyhow::Result<String> {
        if self.library_mode {
            anyhow::bail!("add art to a project, not the library");
        }
        let j = self.art.job(job).ok_or_else(|| anyhow::anyhow!("no job {job}"))?.locked().clone();
        let png = j.result.clone().filter(|_| j.stage == Stage::Done).ok_or_else(|| anyhow::anyhow!("job {job} isn't finished"))?;
        let id = id.trim();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            anyhow::bail!("the id must be letters, digits and _");
        }
        if j.req.kind.is_3d() {
            return self.apply_art_3d(&j, id);
        }
        let db = &self.project.db;
        if db.sheet(id).is_some() {
            anyhow::bail!("the project already has a sprite sheet \"{id}\"; pick another id");
        }
        let mut rel = format!("sprites/{id}.png");
        let mut n = 2;
        while self.project.root.join(&rel).exists() {
            rel = format!("sprites/{id}_{n}.png");
            n += 1;
        }
        std::fs::create_dir_all(self.project.root.join("sprites"))?;
        std::fs::write(self.project.root.join(&rel), &png)?;
        let (fw, fh) = j.frame;
        let sheet = match j.req.kind {
            ArtKind::Portrait => SheetDef { scale: 2, ..SheetDef::new(id, &rel, fw, fh) },
            ArtKind::Icon | ArtKind::Image => SheetDef::new(id, &rel, fw, fh),
            ArtKind::Model3d | ArtKind::Creature3d | ArtKind::Gear3d | ArtKind::Clips3d | ArtKind::Prop3d => unreachable!("3D jobs are added by apply_art_3d"),
        };
        let mut added = vec![format!("sprite sheet {id} ({rel})")];
        let db = &mut self.project.db;
        db.upsert(sheet);
        let name = id.replace('_', " ");
        let name = name[..1].to_uppercase() + &name[1..];
        match j.req.kind {
            ArtKind::Portrait if !j.req.portrait_for.is_empty() => match db.characters.iter_mut().find(|c| c.id == j.req.portrait_for) {
                Some(c) => {
                    c.portrait = SpriteRef::new(id, 0);
                    added.push(format!("the portrait of {}", j.req.portrait_for));
                }
                None => added.push(format!("(no character \"{}\" for the portrait)", j.req.portrait_for)),
            },
            ArtKind::Icon if db.item(id).is_none() => {
                db.upsert(ItemDef { name: name.clone(), icon: SpriteRef::new(id, 0), ..ItemDef::fresh(id) });
                added.push(format!("item {id}"));
            }
            _ => {}
        }
        db.reindex();
        self.defs_dirty = true;
        self.save_defs();
        self.reload_assets();
        if let Some(job) = self.art.job(job) {
            job.locked().applied = Some(id.to_string());
        }
        let msg = format!("Added {}", added.join(", "));
        self.set_status(msg.clone());
        Ok(msg)
    }

    /// Adds a finished 3D job: a 3D character's model (the rigged file with its clips file) and a
    /// character using it, a creature's model, an item's 3D model. Returns what was added.
    fn apply_art_3d(&mut self, j: &Job, id: &str) -> anyhow::Result<String> {
        let file = |name: &str| j.made.files.iter().find(|(f, _)| *f == name).map(|(_, b)| b.clone()).ok_or_else(|| anyhow::anyhow!("job {} has no {name}", j.id));
        let write = |ed: &Workspace, stem: &str, bytes: &[u8]| -> anyhow::Result<String> {
            std::fs::create_dir_all(ed.project.root.join("models"))?;
            let mut rel = format!("models/{stem}.glb");
            let mut n = 2;
            while ed.project.root.join(&rel).exists() {
                rel = format!("models/{stem}_{n}.glb");
                n += 1;
            }
            std::fs::write(ed.project.root.join(&rel), bytes)?;
            Ok(rel)
        };
        let mut added = Vec::new();
        match j.req.kind {
            ArtKind::Gear3d => {
                let rel = write(self, id, &file("model.glb")?)?;
                added.push(self.use_item_model(&j.req.id, &rel));
            }
            ArtKind::Prop3d => {
                let (glb, size) = rustpg2d_engine::model::stand_on_ground(&file("model.glb")?)?;
                let rel = write(self, &format!("{}_3d", j.req.id), &glb)?;
                added.push(self.use_prop_model(&j.req.id, &rel, size)?);
            }
            ArtKind::Clips3d => {
                let model = j.req.id.clone();
                if self.project.db.model(&model).is_none() {
                    anyhow::bail!("the 3D model \"{model}\" is gone");
                }
                let rel = write(self, &format!("{model}_clips"), &file("clips.glb")?)?;
                let bytes = self.compact_file(&rel, false)?;
                let names = rustpg2d_engine::model::clip_names(&bytes)?;
                added.push(self.add_shared_clips(&model, &rel, &names, &j.req.extras));
            }
            kind => {
                if self.project.db.model(id).is_some() {
                    anyhow::bail!("the project already has a 3D model \"{id}\"; pick another id");
                }
                let rigged = kind == ArtKind::Model3d;
                let rel = write(self, id, &file(if rigged { "rig.glb" } else { "model.glb" })?)?;
                let before = self.project.db.models.len();
                added.push(self.use_model_file("", false, &rel));
                let Some(model) = self.project.db.models.get(before).map(|m| m.id.clone()) else { anyhow::bail!("{}", added.join("; ")) };
                if rigged {
                    let clips = write(self, &format!("{id}_clips"), &file("clips.glb")?)?;
                    added.push(self.use_model_file(&model, true, &clips));
                }
                let db = &mut self.project.db;
                if let Some(m) = db.models.iter_mut().find(|m| m.id == model) {
                    m.female = j.req.female;
                    let name = id.replace('_', " ");
                    m.name = name[..1].to_uppercase() + &name[1..];
                }
                if rigged && db.character(id).is_none() {
                    let name = id.replace('_', " ");
                    let name = name[..1].to_uppercase() + &name[1..];
                    let mut c = CharacterDef { description: j.req.description.trim().to_string(), ..crate::defs::default_character(db, id, &name) };
                    c.appearance = rustpg2d_engine::defs::Appearance { model: model.clone(), ..Default::default() };
                    db.upsert(c);
                    added.push(format!("character {id}"));
                }
                db.reindex();
                self.defs_dirty = true;
                self.save_defs();
                self.reload_assets();
            }
        }
        if let Some(job) = self.art.job(j.id) {
            job.locked().applied = Some(id.to_string());
        }
        let msg = added.join("; ");
        self.set_status(msg.clone());
        Ok(msg)
    }

    /// Tells about jobs that finished or failed (the status line), once each.
    pub fn art_tick(&mut self) {
        let mut news = Vec::new();
        for j in &self.art.jobs {
            let mut j = j.locked();
            let end = matches!(j.stage, Stage::Done | Stage::Failed(_));
            if end && !j.told {
                j.told = true;
                news.push(match &j.stage {
                    Stage::Failed(e) => format!("Art job {} ({}) failed: {e}", j.id, j.req.id),
                    _ => format!("Art job {} ({}) is ready: {} credits spent", j.id, j.req.id, j.credits()),
                });
            }
        }
        for n in news {
            self.set_status(n);
        }
    }
}

/// A region of a picture, cut to its opaque part, enlarged (nearest) to about `size` px on its
/// longer side and centred on white with a margin: a PNG data URI for Meshy. Half-transparent
/// pixels (a figure's ground shadow) count as background, or they would be modelled.
fn framed(im: &Rgba, x0: u32, y0: u32, w: u32, h: u32, size: u32) -> anyhow::Result<String> {
    let (mut l, mut t, mut r, mut b) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if im.get((x0 + x) as i32, (y0 + y) as i32)[3] >= 128 {
                (l, t, r, b) = (l.min(x), t.min(y), r.max(x + 1), b.max(y + 1));
            }
        }
    }
    if r <= l || b <= t {
        anyhow::bail!("the picture is empty");
    }
    let k = (size * 8 / 10 / (r - l).max(b - t)).max(1);
    let mut out = Rgba::new(size, size);
    let (ox, oy) = ((size - (r - l) * k) / 2, (size - (b - t) * k) / 2);
    for y in 0..size {
        for x in 0..size {
            let (sx, sy) = (x as i64 - ox as i64, y as i64 - oy as i64);
            let c = if sx >= 0 && sy >= 0 && (sx as u32) < (r - l) * k && (sy as u32) < (b - t) * k { im.get((x0 + l) as i32 + sx as i32 / k as i32, (y0 + t) as i32 + sy as i32 / k as i32) } else { [0; 4] };
            out.set(x as i32, y as i32, if c[3] >= 128 { [c[0], c[1], c[2], 255] } else { [255; 4] });
        }
    }
    Ok(meshy::data_uri(&out.encode()?))
}
