//! *Generate art* with the creator's Meshy account: characters (a walk grid drawn in the style of
//! the project's own sheets, then an attack grid of the same character), item icons, props,
//! portraits and painted images. Jobs run on worker threads (start a Meshy task, poll it,
//! download, convert with [`convert`]); the Art window and the agent server's `art_*` tools
//! start, watch and apply them. Spending is guarded: the cost is shown (and must be confirmed by
//! agents) before a job starts, and a per-session credit budget stops jobs beyond it.
//!
//! The API key lives in `<config>/meshy.key` (owner-only), never in projects, exports, logs or
//! agent replies. `RUSTPG2D_MESHY_FAKE=<dir>` replaces the API by stored pictures
//! (`walk.png`, `attack.png`, `picture.png`) for tests.

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
    /// A character sheet (walk, idle, hurt; with `attack` != "none" also attack frames).
    Character,
    /// An item icon (32 x 32).
    Icon,
    /// A prop (w x h tiles).
    Prop,
    /// A dialog portrait (128 x 128 at 2x).
    Portrait,
    /// A painted picture (Show image).
    Image,
}

impl ArtKind {
    pub const ALL: [ArtKind; 5] = [ArtKind::Character, ArtKind::Icon, ArtKind::Prop, ArtKind::Portrait, ArtKind::Image];

    pub fn name(self) -> &'static str {
        match self {
            ArtKind::Character => "character",
            ArtKind::Icon => "icon",
            ArtKind::Prop => "prop",
            ArtKind::Portrait => "portrait",
            ArtKind::Image => "image",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ArtKind::Character => "Character",
            ArtKind::Icon => "Item icon",
            ArtKind::Prop => "Prop",
            ArtKind::Portrait => "Portrait",
            ArtKind::Image => "Painted image",
        }
    }

    pub fn parse(s: &str) -> Option<ArtKind> {
        let s = s.trim().to_lowercase();
        ArtKind::ALL.into_iter().find(|k| k.name() == s || k.name().to_string() + "s" == s)
    }
}

/// Attacks a character sheet can show (none = no attack picture).
pub const ATTACKS: [(&str, &str); 8] = [
    ("none", "No attack"),
    ("slash", "Sword slash"),
    ("chop", "Overhead blow (axe, hammer)"),
    ("thrust", "Spear thrust"),
    ("bow", "Bow shot"),
    ("club", "Club smash"),
    ("cast", "Spell cast"),
    ("bite", "Pounce / bite (animals)"),
];

/// The attack motion. People are drawn with empty hands (the game puts the weapon in the hand), so
/// their attacks are arm motions; animals keep their pounce.
fn attack_prompt(attack: &str) -> &'static str {
    match attack {
        "chop" => "The 4 columns are an overhead blow with EMPTY fists (no weapon): 1 both fists raised high behind the head, 2 both arms swinging down in front in the facing direction, 3 both fists low in front after the blow, 4 back to standing.",
        "thrust" => "The 4 columns are a spear-thrust motion with EMPTY fists (no spear): 1 both fists pulled back at the hip, 2 both arms thrusting forward in the facing direction, 3 both arms fully extended forward, 4 back to standing.",
        "bow" => "The 4 columns are an archery motion with EMPTY hands (no bow, no arrow): 1 raising the left fist forward, 2 the left arm fully extended forward in the facing direction and the right fist pulled back to the cheek, 3 the right hand opening and moving back, 4 lowering both arms.",
        "club" => "The 4 columns are a club smash with EMPTY fists (no club): 1 both fists raised high over the head, 2 both arms smashing down in front in the facing direction, 3 both fists down near the feet in front, 4 lifting the arms back up.",
        "cast" => "The 4 columns are casting a spell with EMPTY hands (no staff, no effects): 1 the right hand raised high, 2 the right arm thrust forward in the facing direction, palm open, 3 the arm fully extended, 4 back to standing.",
        "bite" => "The 4 columns are a lunging attack: 1 crouched low gathering to spring, 2 leaping forward in the facing direction with jaws or claws out, 3 striking at the end of the leap, 4 landing and standing again.",
        _ => "The 4 columns are a sword-slash motion with an EMPTY right fist (no sword): 1 the right fist raised to the side ready to strike, 2 the right arm swinging across in the facing direction, 3 follow-through with the right fist low on the other side, 4 back to standing.",
    }
}

/// People hold nothing in their pictures: the game draws weapons, tools and shields at their hands.
const EMPTY_HANDS: &str = "IMPORTANT: the character holds nothing at all: no weapon, no sword, no spear, no bow, no staff, no tool, no shield, nothing in the hands and nothing on the back. Both hands are empty and closed into loose fists, as if gripping a handle. ";

const GRID: &str = "Layout exactly like the second reference image: a grid of 4 rows and 4 columns, every cell the same size, one full-body figure per cell, nothing overlapping or crossing into another cell. Row 1: facing the viewer (front). Row 2: facing left (side view). Row 3: facing right (side view). Row 4: facing away (back view). ";

/// A request for art.
#[derive(Clone, Debug)]
pub struct ArtRequest {
    pub kind: ArtKind,
    /// Id of the asset it becomes.
    pub id: String,
    pub description: String,
    /// Size class (`convert::SIZES`), characters.
    pub size: String,
    /// Attack (`ATTACKS`), characters.
    pub attack: String,
    /// Props: size in tiles.
    pub tiles: (u32, u32),
    /// Portraits: the character whose dialog portrait it becomes ("" = none).
    pub portrait_for: String,
    /// Started by an AI agent (shown as such).
    pub by_agent: bool,
}

impl ArtRequest {
    /// The pictures it needs.
    pub fn pictures(&self) -> Vec<&'static str> {
        match self.kind {
            ArtKind::Character if self.attack != "none" => vec!["walk", "attack"],
            ArtKind::Character => vec!["walk"],
            _ => vec!["picture"],
        }
    }

    pub fn check(&self) -> Result<()> {
        if self.description.trim().len() < 8 {
            return Err(anyhow!("describe it in a few words (what it looks like, what it wears or holds)"));
        }
        if self.id.trim().is_empty() || !self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(anyhow!("the id must be letters, digits and _"));
        }
        if self.kind == ArtKind::Character {
            convert::size_class(&self.size).ok_or_else(|| anyhow!("unknown size \"{}\": {}", self.size, convert::SIZES.map(|s| s.id).join(", ")))?;
            if !ATTACKS.iter().any(|a| a.0 == self.attack) {
                return Err(anyhow!("unknown attack \"{}\": {}", self.attack, ATTACKS.map(|a| a.0).join(", ")));
            }
        }
        if self.kind == ArtKind::Prop && !(1..=4).contains(&self.tiles.0) | !(1..=4).contains(&self.tiles.1) {
            return Err(anyhow!("a prop is 1 to 4 tiles wide and high"));
        }
        Ok(())
    }

    /// An animal or creature size class: no hands, no held items.
    pub fn animal(&self) -> bool {
        matches!(self.size.as_str(), "animal" | "beast" | "flying")
    }

    /// The Meshy request of picture `name` (references: the style sheets' walk grids and the
    /// attack layout; the walk picture once it exists).
    fn request(&self, name: &str, refs: &References, walk_png: Option<&[u8]>) -> meshy::Request {
        let desc = self.description.trim();
        match (self.kind, name) {
            (ArtKind::Character, "walk") => {
                let what = if self.animal() {
                    "The character is an animal (or creature) seen from slightly above like the figures of the second reference, drawn small and centred in each cell. The 4 columns are its walking (or hopping / flying / floating) cycle in 4 frames. "
                } else {
                    "The 4 columns are a walk cycle with clear steps: standing, left foot well forward, standing, right foot well forward. "
                };
                let empty = if self.animal() { "" } else { EMPTY_HANDS };
                let prompt = format!("Pixel art in exactly the art style of the references (same shading, dark brown outline, proportions, face style, same figure size and pixel size as in the second reference). {GRID}Exactly the same character, colours and size in every frame, feet at the same height in every cell. Plain flat background, no text, no labels, no ground shadows. {what}{empty}The character: {desc}");
                meshy::Request::FromImages { prompt, refs: refs.walk.clone(), aspect: "3:4".into() }
            }
            (ArtKind::Character, _) => {
                let prompt = format!("Pixel art sprite sheet. The character is exactly the character of the first reference image (same face, clothes, colours, proportions, size and pixel style). {} Feet at the same height in every cell. Plain flat background, no text, no labels, no ground shadows, no motion lines or effects. {}{} The character: {desc}", GRID.trim_end(), if self.animal() { "" } else { EMPTY_HANDS }, attack_prompt(&self.attack));
                let mut r = vec![walk_png.map(meshy::data_uri).unwrap_or_default()];
                r.push(refs.attack.clone());
                meshy::Request::FromImages { prompt, refs: r, aspect: "3:4".into() }
            }
            (ArtKind::Icon, _) => match &refs.icons {
                Some(strip) => meshy::Request::FromImages {
                    prompt: format!("Pixel art inventory icon in exactly the style of the reference icons (same pixel size, dark outline, simple shading, 32x32 pixel look). One single icon, centred, filling most of the square image. Plain flat white background, no text, no shadow: {desc}"),
                    refs: vec![strip.clone()],
                    aspect: "1:1".into(),
                },
                None => meshy::Request::FromText { prompt: format!("Pixel art inventory icon, 32x32 pixel look, dark outline, simple shading, one single object, centred, filling most of the square, plain flat background, no text: {desc}"), aspect: "1:1".into(), background: false },
            },
            (ArtKind::Prop, _) => meshy::Request::FromText {
                prompt: format!("Pixel art game prop for a top-down RPG, seen from slightly above, detailed 16-bit style with a dark outline and soft shading, one single object, centred, plain flat background, no text, no ground shadow: {desc}"),
                aspect: if self.tiles.0 > self.tiles.1 { "4:3" } else if self.tiles.1 > self.tiles.0 { "3:4" } else { "1:1" }.into(),
                background: false,
            },
            (ArtKind::Portrait, _) => meshy::Request::FromText { prompt: format!("Painted fantasy RPG character portrait, head and shoulders, facing the viewer, soft painterly shading, centred, plain flat background, no text: {desc}"), aspect: "1:1".into(), background: false },
            (ArtKind::Image, _) => meshy::Request::FromText { prompt: format!("Painted fantasy RPG illustration, detailed, rich colours, no text: {desc}"), aspect: "3:4".into(), background: true },
        }
    }
}

/// Reference pictures (data URIs) the requests use.
#[derive(Clone, Debug, Default)]
pub struct References {
    /// Walk grids of two complete-figure sheets (style; the second sets the layout).
    pub walk: Vec<String>,
    /// An attack grid (layout of attack pictures).
    pub attack: String,
    /// A strip of the project's item icons, enlarged (icon style), if it has some.
    pub icons: Option<String>,
    /// Which sheets they come from (for the window).
    pub from: String,
}

/// Rows `first..first + 4` of a 16-row character sheet, enlarged 4x on a flat background.
pub fn grid_ref(sheet_png: &[u8], first_row: u32) -> Result<String> {
    let im = Rgba::decode(sheet_png)?;
    let (fw, fh) = (im.w / 4, im.h / 16);
    let mut out = Rgba::new(im.w * 4, fh * 4 * 4);
    for y in 0..out.h {
        for x in 0..out.w {
            let c = im.get((x / 4) as i32, (first_row * fh + y / 4) as i32);
            let a = c[3] as u32;
            let bg = [140u32, 140, 150];
            let mix = |i: usize| ((c[i] as u32 * a + bg[i] * (255 - a)) / 255) as u8;
            out.set(x as i32, y as i32, [mix(0), mix(1), mix(2), 255]);
        }
    }
    let _ = fw;
    Ok(meshy::data_uri(&out.encode()?))
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

/// Converts a job's pictures into its result.
fn convert_job(job: &Arc<Mutex<Job>>) -> Result<()> {
    let (req, pics) = {
        let j = job.lock().unwrap();
        (j.req.clone(), j.pictures.clone())
    };
    let png = |name: &str| pics.iter().find(|p| p.name == name).and_then(|p| p.png.clone()).ok_or_else(|| anyhow!("the {name} picture is missing"));
    let decode = |name: &str| -> Result<Rgba> { Rgba::decode(&png(name)?) };
    let (img, frame) = match req.kind {
        ArtKind::Character => {
            let size = convert::size_class(&req.size).ok_or_else(|| anyhow!("unknown size"))?;
            let attack = if req.attack != "none" { Some(decode("attack")?) } else { None };
            (convert::character_sheet(&decode("walk")?, attack.as_ref(), &size, false)?, size.frame)
        }
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
        req.pictures().len() as u32 * meshy::model_cost(model)
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
        let job = Arc::new(Mutex::new(Job { id: self.next_id, req, model: settings.model.clone(), cost, pictures, stage: Stage::Drawing, result: None, frame: (0, 0), applied: None, version: 0, told: false }));
        self.jobs.push(job.clone());
        let spent = self.spent.clone();
        std::thread::spawn(move || run(job, key, refs, None, spent));
        Ok(self.next_id)
    }

    /// Draws picture `name` of a job again (and converts again).
    pub fn redo(&mut self, id: u32, name: &str, settings: &ArtSettings, refs: References) -> Result<()> {
        let job = self.job(id).ok_or_else(|| anyhow!("no job {id}"))?;
        let cost = meshy::model_cost(&settings.model);
        {
            let j = job.lock().unwrap();
            if !j.pictures.iter().any(|p| p.name == name) {
                return Err(anyhow!("job {id} has no {name} picture (it has {})", j.pictures.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")));
            }
            if j.stage == Stage::Drawing || j.stage == Stage::Converting {
                return Err(anyhow!("job {id} is still drawing"));
            }
        }
        self.reserve(cost, settings.budget)?;
        {
            let mut j = job.lock().unwrap();
            j.stage = Stage::Drawing;
            j.told = false;
            j.cost += cost;
            if let Some(p) = j.pictures.iter_mut().find(|p| p.name == name) {
                p.status = "WAITING".into();
                p.progress = 0;
                p.png = None;
                p.credits = 0;
            }
            j.version += 1;
        }
        let key = api_key();
        let spent = self.spent.clone();
        let only = name.to_string();
        std::thread::spawn(move || run(job, key, refs, Some(only), spent));
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
fn run(job: Arc<Mutex<Job>>, key: String, refs: References, only: Option<String>, spent: Arc<Mutex<u32>>) {
    let (req, model, names) = {
        let j = job.lock().unwrap();
        (j.req.clone(), j.model.clone(), j.pictures.iter().map(|p| p.name).collect::<Vec<_>>())
    };
    for (k, name) in names.iter().enumerate() {
        if only.as_deref().is_some_and(|o| o != *name) {
            continue;
        }
        let walk = job.lock().unwrap().pictures.iter().find(|p| p.name == "walk").and_then(|p| p.png.clone());
        let r = req.request(name, &refs, walk.as_deref());
        let i2i = matches!(r, meshy::Request::FromImages { .. });
        if let Err(e) = draw(&job, k, &key, &model, r, i2i) {
            let mut j = job.lock().unwrap();
            // A failed picture costs nothing: give its reservation back.
            let back = meshy::model_cost(&model).saturating_sub(j.pictures[k].credits);
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
        ArtRequest { kind, id: "orc_shaman".into(), description: "an orc shaman with a staff".into(), size: "big".into(), attack: attack.into(), tiles: (1, 1), portrait_for: String::new(), by_agent: true }
    }

    #[test]
    fn costs_follow_the_pictures_and_the_budget_holds() {
        assert_eq!(ArtState::cost(&req(ArtKind::Character, "cast"), "nano-banana-2-lite"), 6);
        assert_eq!(ArtState::cost(&req(ArtKind::Character, "none"), "nano-banana-2-lite"), 3);
        assert_eq!(ArtState::cost(&req(ArtKind::Icon, "none"), "nano-banana-2"), 6);
        let s = ArtState::default();
        s.reserve(6, 8).unwrap();
        assert!(s.reserve(3, 8).is_err(), "6 + 3 is past a budget of 8");
        s.reserve(2, 8).unwrap();
        assert_eq!(*s.spent.lock().unwrap(), 8);
    }

    #[test]
    fn requests_are_checked() {
        assert!(req(ArtKind::Character, "cast").check().is_ok());
        assert!(req(ArtKind::Character, "juggle").check().is_err());
        assert!(ArtRequest { id: "bad id".into(), ..req(ArtKind::Icon, "none") }.check().is_err());
        assert!(ArtRequest { description: "orc".into(), ..req(ArtKind::Icon, "none") }.check().is_err());
        assert_eq!(ArtKind::parse("Icons"), Some(ArtKind::Icon));
    }
}
