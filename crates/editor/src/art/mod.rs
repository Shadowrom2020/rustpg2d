//! *Generate art* with the creator's Meshy account: item icons (in the style of the project's),
//! props, portraits and painted images. Jobs run on worker threads (start a Meshy task, poll it,
//! download, convert with [`convert`]); the Art window and the agent server's `art_*` tools
//! start, watch and apply them. Spending is guarded: the cost is shown (and must be confirmed by
//! agents) before a job starts, and a per-session credit budget stops jobs beyond it.
//!
//! 3D jobs make models (characters are 3D models): a 3D character (a turnaround picture or a figure's views -> a model ->
//! a rig -> animation clips), a 3D creature (a picture -> an unrigged model), an item's 3D model
//! (from its icon).
//!
//! The API key lives in `<config>/meshy.key` (owner-only), never in projects, exports, logs or
//! agent replies. `RUSTPG2D_MESHY_FAKE=<dir>` replaces the API by stored files
//! (`picture.png`, `views.png`, `model.glb`, `rig.glb`, `clips.glb`) for tests; 3D
//! steps leave the request they would have sent there (`sent_<step>.json`).

mod agent;
pub mod convert;
pub mod meshy;
pub mod ui;

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use rustpg2d_engine::imageio::Rgba;

/// What to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtKind {
    /// An item icon (32 x 32).
    Icon,
    /// A prop (w x h tiles).
    Prop,
    /// A dialog portrait (128 x 128 at 2x).
    Portrait,
    /// A painted picture (Show image).
    Image,
    /// A rigged, animated 3D character (a person: Meshy rigs humanoids).
    Model3d,
    /// An unrigged 3D model of an animal or creature (rig templates animate it).
    Creature3d,
    /// The 3D model of an existing item (`id`), from its icon.
    Gear3d,
}

impl ArtKind {
    pub const ALL: [ArtKind; 7] = [ArtKind::Icon, ArtKind::Prop, ArtKind::Portrait, ArtKind::Image, ArtKind::Model3d, ArtKind::Creature3d, ArtKind::Gear3d];

    /// A 3D job (models, not pictures).
    pub fn is_3d(self) -> bool {
        matches!(self, ArtKind::Model3d | ArtKind::Creature3d | ArtKind::Gear3d)
    }

    pub fn name(self) -> &'static str {
        match self {
            ArtKind::Icon => "icon",
            ArtKind::Prop => "prop",
            ArtKind::Portrait => "portrait",
            ArtKind::Image => "image",
            ArtKind::Model3d => "model",
            ArtKind::Creature3d => "creature",
            ArtKind::Gear3d => "gear",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ArtKind::Icon => "Item icon",
            ArtKind::Prop => "Prop",
            ArtKind::Portrait => "Portrait",
            ArtKind::Image => "Painted image",
            ArtKind::Model3d => "3D character",
            ArtKind::Creature3d => "3D creature",
            ArtKind::Gear3d => "Item 3D model",
        }
    }

    pub fn parse(s: &str) -> Option<ArtKind> {
        let s = s.trim().to_lowercase();
        ArtKind::ALL.into_iter().find(|k| k.name() == s || k.name().to_string() + "s" == s)
    }
}

/// Attacks a 3D character can play (none = no attack clip).
pub const ATTACKS: [(&str, &str); 7] = [
    ("none", "No attack"),
    ("slash", "Sword slash"),
    ("chop", "Overhead blow (axe, hammer)"),
    ("thrust", "Spear thrust"),
    ("bow", "Bow shot"),
    ("club", "Club smash"),
    ("cast", "Spell cast"),
];

/// A request for art.
#[derive(Clone, Debug)]
pub struct ArtRequest {
    pub kind: ArtKind,
    /// Id of the asset it becomes.
    pub id: String,
    pub description: String,
    /// Attack (`ATTACKS`), 3D characters.
    pub attack: String,
    /// Props: size in tiles.
    pub tiles: (u32, u32),
    /// Portraits: the character whose dialog portrait it becomes ("" = none).
    pub portrait_for: String,
    /// Started by an AI agent (shown as such).
    pub by_agent: bool,
    /// 3D characters: a woman (her sitting clip).
    pub female: bool,
    /// 3D characters: model this figure (its front, side and back frames) instead of drawing a
    /// turnaround picture ("" = draw one).
    pub from: String,
    /// 3D characters: extra animations from Meshy's library (see [`EXTRAS`]).
    pub extras: Vec<String>,
}

/// Extra animations a 3D character can get (3 credits each): working at an anvil, bending to
/// pick crops, chopping wood, sleeping in a bed. Fishing, sweeping and reading have no clip in
/// Meshy's library (they play idle with the tool in hand).
pub const EXTRAS: [&str; 4] = ["craft", "farm", "chop", "sleep"];

/// Credits of the 3D steps (Meshy's prices; what a task really consumed is shown when it ends).
pub const COST_MODEL: u32 = 30;
pub const COST_RIG: u32 = 5;
pub const COST_CLIP: u32 = 3;

/// The animation-library clips a 3D character gets: walking, idle, its attack, a hit reaction,
/// sitting on a chair.
pub fn clip_actions(attack: &str, female: bool, extras: &[String]) -> Vec<u32> {
    let attack = match attack {
        "slash" => Some(219),
        // Heavy Hammer Swing: an overhead blow (the Charged Axe Chop crouches for seconds first).
        "chop" => Some(128),
        "thrust" => Some(240),
        // Archery Shot (Draw and Shoot from Back lasts eight seconds).
        "bow" => Some(224),
        "club" => Some(4),
        "cast" => Some(129),
        _ => None,
    };
    let extra = extras.iter().filter_map(|e| match e.as_str() {
        "craft" => Some(128),
        "farm" => Some(if female { 277 } else { 276 }),
        "chop" => Some(128),
        "sleep" => Some(267),
        _ => None,
    });
    let mut ids: Vec<u32> = [Some(566), Some(11), Some(178), attack, Some(if female { 32 } else { 33 })].into_iter().flatten().chain(extra).collect();
    let mut seen = std::collections::HashSet::new();
    ids.retain(|i| seen.insert(*i));
    ids
}

impl ArtRequest {
    /// The pictures it needs.
    pub fn pictures(&self) -> Vec<&'static str> {
        match self.kind {
            ArtKind::Model3d if !self.from.is_empty() => vec!["model", "rig", "clips"],
            ArtKind::Model3d => vec!["views", "model", "rig", "clips"],
            ArtKind::Creature3d if !self.from.is_empty() => vec!["model"],
            ArtKind::Creature3d => vec!["views", "model"],
            ArtKind::Gear3d => vec!["model"],
            _ => vec!["picture"],
        }
    }

    /// Credits of one step.
    pub fn step_cost(&self, name: &str, picture_cost: u32) -> u32 {
        match name {
            "model" => COST_MODEL,
            "rig" => COST_RIG,
            "clips" => COST_CLIP * clip_actions(&self.attack, self.female, &self.extras).len() as u32,
            _ => picture_cost,
        }
    }

    pub fn check(&self) -> Result<()> {
        if self.description.trim().len() < 8 && self.kind != ArtKind::Gear3d {
            return Err(anyhow!("describe it in a few words (what it looks like, what it wears or holds)"));
        }
        if self.id.trim().is_empty() || !self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(anyhow!("the id must be letters, digits and _"));
        }
        if self.kind == ArtKind::Model3d && !ATTACKS.iter().any(|a| a.0 == self.attack) {
            return Err(anyhow!("unknown attack \"{}\" for a 3D character: {}", self.attack, ATTACKS.map(|a| a.0).join(", ")));
        }
        if let Some(e) = self.extras.iter().find(|e| !EXTRAS.contains(&e.as_str())) {
            return Err(anyhow!("unknown extra animation \"{e}\": {}", EXTRAS.join(", ")));
        }
        if self.kind == ArtKind::Prop && !(1..=4).contains(&self.tiles.0) | !(1..=4).contains(&self.tiles.1) {
            return Err(anyhow!("a prop is 1 to 4 tiles wide and high"));
        }
        Ok(())
    }

    /// The Meshy request of picture `name` (references: the project's icons for icons).
    fn request(&self, refs: &References) -> meshy::Request {
        let desc = self.description.trim();
        match self.kind {
            ArtKind::Icon => match &refs.icons {
                Some(strip) => meshy::Request::FromImages {
                    prompt: format!("Pixel art inventory icon in exactly the style of the reference icons (same pixel size, dark outline, simple shading, 32x32 pixel look). One single icon, centred, filling most of the square image. Plain flat white background, no text, no shadow: {desc}"),
                    refs: vec![strip.clone()],
                    aspect: "1:1".into(),
                },
                None => meshy::Request::FromText { prompt: format!("Pixel art inventory icon, 32x32 pixel look, dark outline, simple shading, one single object, centred, filling most of the square, plain flat background, no text: {desc}"), aspect: "1:1".into(), background: false },
            },
            ArtKind::Prop => meshy::Request::FromText {
                prompt: format!("Pixel art game prop for a top-down RPG, seen from slightly above, detailed 16-bit style with a dark outline and soft shading, one single object, centred, plain flat background, no text, no ground shadow: {desc}"),
                aspect: if self.tiles.0 > self.tiles.1 { "4:3" } else if self.tiles.1 > self.tiles.0 { "3:4" } else { "1:1" }.into(),
                background: false,
            },
            ArtKind::Model3d => meshy::Request::FromText {
                prompt: format!("Character turnaround sheet for a 3D model: the same character three times side by side, full body, standing straight in an A-pose with the arms slightly away from the body, both hands empty and open, no weapons, no sheaths or scabbards, nothing hanging from the belt. Left: front view. Middle: side view facing left. Right: back view. Stylized fantasy RPG character, clean shapes, soft shading, plain white background, no text, no shadows. The character: {desc}"),
                aspect: "16:9".into(),
                background: false,
            },
            ArtKind::Creature3d => meshy::Request::FromText {
                prompt: format!("A single creature for a 3D model, full body, standing on all its feet, seen from the front and a little from the side, stylized fantasy RPG creature, clean shapes, soft shading, plain white background, no text, no shadows: {desc}"),
                aspect: "1:1".into(),
                background: false,
            },
            ArtKind::Gear3d => meshy::Request::FromText { prompt: desc.to_string(), aspect: "1:1".into(), background: false },
            ArtKind::Portrait => meshy::Request::FromText { prompt: format!("Painted fantasy RPG character portrait, head and shoulders, facing the viewer, soft painterly shading, centred, plain flat background, no text: {desc}"), aspect: "1:1".into(), background: false },
            ArtKind::Image => meshy::Request::FromText { prompt: format!("Painted fantasy RPG illustration, detailed, rich colours, no text: {desc}"), aspect: "3:4".into(), background: true },
        }
    }
}

/// Reference pictures (data URIs) the requests use.
#[derive(Clone, Debug, Default)]
pub struct References {
    /// A strip of the project's item icons, enlarged (icon style), if it has some.
    pub icons: Option<String>,
    /// 3D characters from a figure: its front, side and back frames, enlarged (PNG data URIs).
    pub views: Vec<String>,
    /// Item 3D models: the item's icon, enlarged (PNG data URI).
    pub icon: Option<String>,
}

/// A job's progress.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    Drawing,
    Converting,
    Done,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Picture {
    pub name: &'static str,
    pub task: Option<String>,
    pub status: String,
    pub progress: u32,
    pub png: Option<Vec<u8>>,
    pub credits: u32,
}

/// What a 3D step made and how it went (kept for the next step and for adding it).
#[derive(Clone, Debug, Default)]
pub struct Made3d {
    /// Meshy's task id of each step ("model", "rig"): the next step names it.
    pub tasks: Vec<(&'static str, String)>,
    /// Downloaded files: "model.glb", "rig.glb", "clips.glb".
    pub files: Vec<(&'static str, Vec<u8>)>,
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: u32,
    pub req: ArtRequest,
    pub model: String,
    /// Estimated cost in credits.
    pub cost: u32,
    pub pictures: Vec<Picture>,
    pub stage: Stage,
    /// The converted art (PNG).
    pub result: Option<Vec<u8>>,
    /// Frame size of the result.
    pub frame: (u32, u32),
    /// The asset it became.
    pub applied: Option<String>,
    /// Counted in the window's preview (texture rebuilt when it changes).
    pub version: u32,
    /// The status line told that it ended.
    pub told: bool,
    /// 3D jobs: the tasks and files made.
    pub made: Made3d,
}

impl Job {
    pub fn credits(&self) -> u32 {
        self.pictures.iter().map(|p| p.credits).sum()
    }

    /// One line for scripts and agents.
    pub fn line(&self) -> String {
        let pics: Vec<String> = self.pictures.iter().map(|p| format!("{} {}{}", p.name, p.status.to_lowercase(), if p.status == "IN_PROGRESS" { format!(" {}%", p.progress) } else { String::new() })).collect();
        let stage = match &self.stage {
            Stage::Drawing => "drawing".to_string(),
            Stage::Converting => "converting".to_string(),
            Stage::Done => "done".to_string(),
            Stage::Failed(e) => format!("failed: {e}"),
        };
        format!(
            "job {}: {} \"{}\"{} - {stage} [{}], {} credits{}",
            self.id,
            self.req.kind.name(),
            self.req.id,
            if self.req.by_agent { " (agent)" } else { "" },
            pics.join(", "),
            self.credits(),
            self.applied.as_ref().map_or(String::new(), |a| format!(", added as {a}"))
        )
    }
}

/// Meshy settings (in `EditorSettings`) and the session's spending.
#[derive(Clone, Debug)]
pub struct ArtSettings {
    pub model: String,
    /// Credits a session may spend.
    pub budget: u32,
    /// The Art window asks before each spend.
    pub ask: bool,
}

/// The art state of the editor.
#[derive(Default)]
pub struct ArtState {
    pub jobs: Vec<Arc<Mutex<Job>>>,
    next_id: u32,
    /// Credits spent (and reserved by running jobs) this session.
    pub spent: Arc<Mutex<u32>>,
    pub window: ui::ArtWindow,
    pub settings_open: bool,
    /// The settings window's key field (never shown again once saved).
    pub key_field: String,
    pub balance: Option<std::result::Result<u64, String>>,
}

/// `<config>/meshy.key`.
fn key_path() -> Option<std::path::PathBuf> {
    crate::files::config_dir().map(|d| d.join("meshy.key"))
}

/// The saved API key ("" = none). `MESHY_API_KEY` overrides it.
pub fn api_key() -> String {
    if let Ok(k) = std::env::var("MESHY_API_KEY") {
        return k.trim().to_string();
    }
    key_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|k| k.trim().to_string()).unwrap_or_default()
}

/// Saves (or with "" removes) the API key, readable by the user only.
pub fn save_key(key: &str) -> Result<()> {
    let p = key_path().ok_or_else(|| anyhow!("no config folder"))?;
    if key.trim().is_empty() {
        let _ = std::fs::remove_file(&p);
        return Ok(());
    }
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, key.trim())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn fake_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("RUSTPG2D_MESHY_FAKE").map(std::path::PathBuf::from)
}

/// Draws one picture: starts the task, waits for it, downloads it.
fn draw(job: &Arc<Mutex<Job>>, k: usize, key: &str, model: &str, req: meshy::Request, i2i: bool) -> Result<()> {
    let set = |f: &dyn Fn(&mut Picture)| {
        let mut j = job.lock().unwrap();
        f(&mut j.pictures[k]);
        j.version += 1;
    };
    if let Some(dir) = fake_dir() {
        let name = job.lock().unwrap().pictures[k].name;
        std::thread::sleep(std::time::Duration::from_millis(300));
        let png = std::fs::read(dir.join(format!("{name}.png")))?;
        let credits = meshy::model_cost(model);
        set(&|p| {
            p.status = "SUCCEEDED".into();
            p.progress = 100;
            p.png = Some(png.clone());
            p.credits = credits;
        });
        return Ok(());
    }
    let task = meshy::start(key, model, &req)?;
    set(&|p| {
        p.task = Some(task.clone());
        p.status = "PENDING".into();
    });
    loop {
        std::thread::sleep(std::time::Duration::from_secs(4));
        let t = meshy::task(key, i2i, &task)?;
        set(&|p| {
            p.status = t.status.clone();
            p.progress = t.progress;
            p.credits = t.credits;
        });
        match t.status.as_str() {
            "SUCCEEDED" => {
                let png = meshy::download(t.url.as_deref().ok_or_else(|| anyhow!("the finished picture has no address"))?)?;
                set(&|p| p.png = Some(png.clone()));
                return Ok(());
            }
            "FAILED" | "CANCELED" => return Err(anyhow!("Meshy could not draw it{}", if t.error.is_empty() { String::new() } else { format!(": {}", t.error) })),
            _ => {}
        }
    }
}

/// A grey square: the preview of 3D jobs without a picture.
fn placeholder_png() -> Vec<u8> {
    let mut im = Rgba::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            im.set(x, y, [60, 60, 70, 255]);
        }
    }
    im.encode().unwrap_or_default()
}

/// The three views of a turnaround picture (left, middle, right thirds), each on white, as PNG
/// data URIs.
pub fn split_views(png: &[u8]) -> Result<Vec<String>> {
    let im = Rgba::decode(png)?;
    let w = im.w / 3;
    (0..3)
        .map(|k| {
            let mut v = Rgba::new(w, im.h);
            for y in 0..im.h {
                for x in 0..w {
                    let c = im.get((k * w + x) as i32, y as i32);
                    let a = c[3] as u32;
                    let mix = |i: usize| ((c[i] as u32 * a + 255 * (255 - a)) / 255) as u8;
                    v.set(x as i32, y as i32, [mix(0), mix(1), mix(2), 255]);
                }
            }
            Ok(meshy::data_uri(&v.encode()?))
        })
        .collect()
}

/// The file a 3D step makes.
fn file3d(step: &str) -> &'static str {
    match step {
        "model" => "model.glb",
        "rig" => "rig.glb",
        "clips" => "clips.glb",
        _ => "",
    }
}

/// Runs one 3D step of a job: starts its Meshy task (from the previous step's), waits for it and
/// downloads its file.
fn step3d(job: &Arc<Mutex<Job>>, k: usize, key: &str, req: &ArtRequest, refs: &References) -> Result<()> {
    let name = job.lock().unwrap().pictures[k].name;
    let set = |f: &dyn Fn(&mut Picture)| {
        let mut j = job.lock().unwrap();
        f(&mut j.pictures[k]);
        j.version += 1;
    };
    let file = file3d(name);
    let task_of = |n: &str| job.lock().unwrap().made.tasks.iter().find(|(t, _)| *t == n).map(|(_, id)| id.clone());
    let (path, body) = match (name, req.kind) {
        ("model", ArtKind::Model3d) => {
            let views = if req.from.is_empty() {
                let png = job.lock().unwrap().pictures.iter().find(|p| p.name == "views").and_then(|p| p.png.clone()).ok_or_else(|| anyhow!("the turnaround picture is missing"))?;
                split_views(&png)?
            } else {
                refs.views.clone()
            };
            ("multi-image-to-3d", serde_json::json!({ "image_urls": views, "should_texture": true, "should_remesh": true, "target_polycount": 6000, "pose_mode": "a-pose" }))
        }
        ("model", ArtKind::Creature3d) if !req.from.is_empty() => ("multi-image-to-3d", serde_json::json!({ "image_urls": refs.views, "should_texture": true, "should_remesh": true, "target_polycount": 5000 })),
        ("model", ArtKind::Creature3d) => {
            let png = job.lock().unwrap().pictures.iter().find(|p| p.name == "views").and_then(|p| p.png.clone()).ok_or_else(|| anyhow!("the picture is missing"))?;
            ("image-to-3d", serde_json::json!({ "image_url": meshy::data_uri(&png), "should_texture": true, "should_remesh": true, "target_polycount": 6000 }))
        }
        ("model", _) => {
            let icon = refs.icon.clone().ok_or_else(|| anyhow!("the item has no icon to model it from"))?;
            ("image-to-3d", serde_json::json!({ "image_url": icon, "should_texture": true, "should_remesh": true, "target_polycount": 1500, "texture_prompt": req.description.trim() }))
        }
        ("rig", _) => ("rigging", serde_json::json!({ "input_task_id": task_of("model").ok_or_else(|| anyhow!("no model to rig"))?, "height_meters": 1.7 })),
        _ => ("animations", serde_json::json!({ "rig_task_id": task_of("rig").ok_or_else(|| anyhow!("no rig to animate"))?, "action_ids": clip_actions(&req.attack, req.female, &req.extras) })),
    };
    if let Some(dir) = fake_dir() {
        // Keeps what would have been sent (tests look at the views and the icon).
        std::fs::write(dir.join(format!("sent_{name}.json")), serde_json::to_vec_pretty(&serde_json::json!({ "path": path, "body": body }))?)?;
        std::thread::sleep(std::time::Duration::from_millis(300));
        let bytes = std::fs::read(dir.join(file))?;
        let credits = req.step_cost(name, 0);
        let mut j = job.lock().unwrap();
        j.made.tasks.push((name, format!("fake-{name}")));
        j.made.files.push((file, bytes));
        let p = &mut j.pictures[k];
        (p.status, p.progress, p.credits) = ("SUCCEEDED".into(), 100, credits);
        j.version += 1;
        return Ok(());
    }
    let task = meshy::start3d(key, path, &body)?;
    set(&|p| {
        p.task = Some(task.clone());
        p.status = "PENDING".into();
    });
    loop {
        std::thread::sleep(std::time::Duration::from_secs(6));
        let t = meshy::task3d(key, path, &task)?;
        set(&|p| {
            p.status = t.status.clone();
            p.progress = t.progress;
            p.credits = t.credits;
        });
        match t.status.as_str() {
            "SUCCEEDED" => {
                let url = t.files.iter().find(|(n, _)| *n == file).map(|(_, u)| u.clone()).ok_or_else(|| anyhow!("the finished task has no {file}"))?;
                let bytes = meshy::download(&url)?;
                let mut j = job.lock().unwrap();
                j.made.tasks.push((name, task.clone()));
                j.made.files.push((file, bytes));
                return Ok(());
            }
            "FAILED" | "CANCELED" => return Err(anyhow!("Meshy could not make the {name}{}", if t.error.is_empty() { String::new() } else { format!(": {}", t.error) })),
            _ => {}
        }
    }
}

/// Converts a job's pictures into its result.
fn convert_job(job: &Arc<Mutex<Job>>) -> Result<()> {
    let (req, pics) = {
        let j = job.lock().unwrap();
        (j.req.clone(), j.pictures.clone())
    };
    let png = |name: &str| pics.iter().find(|p| p.name == name).and_then(|p| p.png.clone()).ok_or_else(|| anyhow!("the {name} picture is missing"));
    let decode = |name: &str| -> Result<Rgba> { Rgba::decode(&png(name)?) };
    let (img, frame) = match req.kind {
        ArtKind::Icon => (convert::fit(&decode("picture")?, 32, 32, 1, false)?, (32, 32)),
        ArtKind::Prop => {
            let (w, h) = (req.tiles.0 * 64, req.tiles.1 * 64);
            (convert::fit(&decode("picture")?, w, h, 2, true)?, (w, h))
        }
        ArtKind::Portrait => {
            let p = decode("picture")?;
            let sq = convert::fit(&p, 128, 128, 0, false)?;
            (sq, (128, 128))
        }
        ArtKind::Image => {
            let p = convert::shrink(&decode("picture")?, 1024);
            let f = (p.w, p.h);
            (p, f)
        }
        // 3D jobs: the picture they were made from (the files are kept in `made`).
        ArtKind::Model3d | ArtKind::Creature3d | ArtKind::Gear3d => {
            let p = decode("views").or_else(|_| Rgba::decode(&placeholder_png()))?;
            let p = convert::shrink(&p, 512);
            let f = (p.w, p.h);
            (p, f)
        }
    };
    let bytes = img.encode()?;
    let mut j = job.lock().unwrap();
    j.result = Some(bytes);
    j.frame = frame;
    j.stage = Stage::Done;
    j.version += 1;
    Ok(())
}

impl ArtState {
    /// The estimated cost of a request.
    pub fn cost(req: &ArtRequest, model: &str) -> u32 {
        req.pictures().iter().map(|n| req.step_cost(n, meshy::model_cost(model))).sum()
    }

    /// Books `cost` credits of the session's budget (refused beyond it).
    fn reserve(&self, cost: u32, budget: u32) -> Result<()> {
        let mut spent = self.spent.lock().unwrap();
        if *spent + cost > budget {
            return Err(anyhow!("this costs {cost} credits, but only {} of this session's budget of {budget} are left (Edit › Settings › Meshy)", budget.saturating_sub(*spent)));
        }
        *spent += cost;
        Ok(())
    }

    /// Credits the budget still allows.
    pub fn budget_left(&self, settings: &ArtSettings) -> u32 {
        settings.budget.saturating_sub(*self.spent.lock().unwrap())
    }

    /// Starts a job (after the caller confirmed its cost); refuses without a key or past the
    /// budget.
    pub fn start(&mut self, req: ArtRequest, settings: &ArtSettings, refs: References) -> Result<u32> {
        req.check()?;
        let key = api_key();
        if key.is_empty() && fake_dir().is_none() {
            return Err(anyhow!("no Meshy API key: enter one in Edit › Settings › Meshy"));
        }
        let cost = Self::cost(&req, &settings.model);
        self.reserve(cost, settings.budget)?;
        self.next_id += 1;
        let pictures = req.pictures().into_iter().map(|name| Picture { name, task: None, status: "WAITING".into(), progress: 0, png: None, credits: 0 }).collect();
        let job = Arc::new(Mutex::new(Job { id: self.next_id, req, model: settings.model.clone(), cost, pictures, stage: Stage::Drawing, result: None, frame: (0, 0), applied: None, version: 0, told: false, made: Made3d::default() }));
        self.jobs.push(job.clone());
        let spent = self.spent.clone();
        std::thread::spawn(move || run(job, key, refs, None, spent));
        Ok(self.next_id)
    }

    /// The steps a redo of `name` makes again: that picture; in 3D jobs also every later step
    /// (a new model needs a new rig and new clips).
    pub fn redo_steps(job: &Job, name: &str) -> Vec<&'static str> {
        let names: Vec<&'static str> = job.pictures.iter().map(|p| p.name).collect();
        match names.iter().position(|n| *n == name) {
            Some(k) if job.req.kind.is_3d() => names[k..].to_vec(),
            Some(k) => vec![names[k]],
            None => Vec::new(),
        }
    }

    /// The credits of redoing `name`.
    pub fn redo_cost(job: &Job, name: &str) -> u32 {
        Self::redo_steps(job, name).iter().map(|n| job.req.step_cost(n, meshy::model_cost(&job.model))).sum()
    }

    /// Draws picture `name` of a job again (in 3D jobs with the steps after it) and converts again.
    pub fn redo(&mut self, id: u32, name: &str, settings: &ArtSettings, refs: References) -> Result<()> {
        let job = self.job(id).ok_or_else(|| anyhow!("no job {id}"))?;
        let (steps, cost) = {
            let j = job.lock().unwrap();
            if !j.pictures.iter().any(|p| p.name == name) {
                return Err(anyhow!("job {id} has no {name} picture (it has {})", j.pictures.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")));
            }
            if j.stage == Stage::Drawing || j.stage == Stage::Converting {
                return Err(anyhow!("job {id} is still drawing"));
            }
            (Self::redo_steps(&j, name), Self::redo_cost(&j, name))
        };
        self.reserve(cost, settings.budget)?;
        {
            let mut j = job.lock().unwrap();
            j.stage = Stage::Drawing;
            j.told = false;
            j.applied = None;
            j.cost += cost;
            for p in j.pictures.iter_mut().filter(|p| steps.contains(&p.name)) {
                p.status = "WAITING".into();
                p.progress = 0;
                p.png = None;
                p.credits = 0;
            }
            // Later steps are made from the new ones.
            j.made.tasks.retain(|(n, _)| !steps.contains(n));
            let files: Vec<&str> = steps.iter().map(|n| file3d(n)).collect();
            j.made.files.retain(|(f, _)| !files.contains(f));
            j.version += 1;
        }
        let key = api_key();
        let spent = self.spent.clone();
        std::thread::spawn(move || run(job, key, refs, Some(steps), spent));
        Ok(())
    }

    /// `art=` for `status`: jobs drawing / done / failed, credits used this session.
    pub fn status(&self) -> String {
        let (mut run, mut done, mut failed) = (0, 0, 0);
        for j in &self.jobs {
            match j.lock().unwrap().stage {
                Stage::Drawing | Stage::Converting => run += 1,
                Stage::Done => done += 1,
                Stage::Failed(_) => failed += 1,
            }
        }
        format!("{run}/{done}/{failed}:{}cr", *self.spent.lock().unwrap())
    }

    pub fn job(&self, id: u32) -> Option<Arc<Mutex<Job>>> {
        self.jobs.iter().find(|j| j.lock().unwrap().id == id).cloned()
    }
}

/// A job's worker: draws its pictures in order (only `only` when redoing), converts.
fn run(job: Arc<Mutex<Job>>, key: String, refs: References, only: Option<Vec<&'static str>>, spent: Arc<Mutex<u32>>) {
    let (req, model, names) = {
        let j = job.lock().unwrap();
        (j.req.clone(), j.model.clone(), j.pictures.iter().map(|p| p.name).collect::<Vec<_>>())
    };
    for (k, name) in names.iter().enumerate() {
        if only.as_ref().is_some_and(|o| !o.contains(name)) {
            continue;
        }
        let result = if matches!(*name, "model" | "rig" | "clips") {
            step3d(&job, k, &key, &req, &refs)
        } else {
            let r = req.request(&refs);
            let i2i = matches!(r, meshy::Request::FromImages { .. });
            draw(&job, k, &key, &model, r, i2i)
        };
        if let Err(e) = result {
            let mut j = job.lock().unwrap();
            // A failed step costs nothing: give its reservation back.
            let back = req.step_cost(name, meshy::model_cost(&model)).saturating_sub(j.pictures[k].credits);
            let mut s = spent.lock().unwrap();
            *s = s.saturating_sub(back);
            j.pictures[k].status = "FAILED".into();
            j.stage = Stage::Failed(format!("{e:#}"));
            j.version += 1;
            return;
        }
    }
    job.lock().unwrap().stage = Stage::Converting;
    if let Err(e) = convert_job(&job) {
        let mut j = job.lock().unwrap();
        j.stage = Stage::Failed(format!("converting: {e:#}"));
        j.version += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(kind: ArtKind, attack: &str) -> ArtRequest {
        ArtRequest { kind, id: "orc_shaman".into(), description: "an orc shaman with a staff".into(), attack: attack.into(), tiles: (1, 1), portrait_for: String::new(), by_agent: true, female: false, from: String::new(), extras: Vec::new() }
    }

    #[test]
    fn costs_follow_the_pictures_and_the_budget_holds() {
        assert_eq!(ArtState::cost(&req(ArtKind::Portrait, "none"), "nano-banana-2-lite"), 3);
        assert_eq!(ArtState::cost(&req(ArtKind::Icon, "none"), "nano-banana-2"), 6);
        let s = ArtState::default();
        s.reserve(6, 8).unwrap();
        assert!(s.reserve(3, 8).is_err(), "6 + 3 is past a budget of 8");
        s.reserve(2, 8).unwrap();
        assert_eq!(*s.spent.lock().unwrap(), 8);
    }

    #[test]
    fn requests_are_checked() {
        assert!(req(ArtKind::Model3d, "cast").check().is_ok());
        assert!(req(ArtKind::Model3d, "juggle").check().is_err());
        assert!(ArtRequest { id: "bad id".into(), ..req(ArtKind::Icon, "none") }.check().is_err());
        assert!(ArtRequest { description: "orc".into(), ..req(ArtKind::Icon, "none") }.check().is_err());
        assert_eq!(ArtKind::parse("Icons"), Some(ArtKind::Icon));
        assert!(req(ArtKind::Model3d, "bite").check().is_err(), "3D characters are people: no pounce");
        assert!(ArtRequest { description: String::new(), ..req(ArtKind::Gear3d, "none") }.check().is_ok(), "an item's model needs no description");
    }

    #[test]
    fn three_d_jobs_have_their_steps_and_costs() {
        let r = req(ArtKind::Model3d, "slash");
        assert_eq!(r.pictures(), vec!["views", "model", "rig", "clips"]);
        // A turnaround picture, the model, the rig, five clips (walk, idle, hurt, slash, sit).
        assert_eq!(ArtState::cost(&r, "nano-banana-2-lite"), 3 + COST_MODEL + COST_RIG + 5 * COST_CLIP);
        assert_eq!(clip_actions("none", true, &[]), vec![566, 11, 178, 32]);
        assert_eq!(clip_actions("chop", false, &["craft".into(), "farm".into()]), vec![566, 11, 178, 128, 33, 276]);
        assert!(ArtRequest { extras: vec!["juggle".into()], ..r.clone() }.check().is_err());
        let from = ArtRequest { from: "hero".into(), ..r.clone() };
        assert_eq!(from.pictures(), vec!["model", "rig", "clips"]);
        assert_eq!(req(ArtKind::Creature3d, "none").pictures(), vec!["views", "model"]);
        assert_eq!(ArtRequest { from: "wolf".into(), ..req(ArtKind::Creature3d, "none") }.pictures(), vec!["model"]);
        assert_eq!(ArtState::cost(&req(ArtKind::Gear3d, "none"), "nano-banana-2-lite"), COST_MODEL);
        let job = Job { id: 1, req: r.clone(), model: "nano-banana-2-lite".into(), cost: 0, pictures: r.pictures().into_iter().map(|name| Picture { name, task: None, status: String::new(), progress: 0, png: None, credits: 0 }).collect(), stage: Stage::Done, result: None, frame: (0, 0), applied: None, version: 0, told: false, made: Made3d::default() };
        assert_eq!(ArtState::redo_steps(&job, "rig"), vec!["rig", "clips"], "a new rig needs new clips");
        assert_eq!(ArtState::redo_cost(&job, "rig"), COST_RIG + 5 * COST_CLIP);
    }

    #[test]
    fn a_turnaround_is_cut_into_three_views() {
        let mut im = Rgba::new(30, 10);
        for x in 0..30 {
            im.set(x, 5, [(x / 10 * 100) as u8, 0, 0, 255]);
        }
        let views = split_views(&im.encode().unwrap()).unwrap();
        assert_eq!(views.len(), 3);
        assert!(views.iter().all(|v| v.starts_with("data:image/png;base64,")));
    }
}
