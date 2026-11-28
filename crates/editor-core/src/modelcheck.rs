//! A 3D model looked over, for the model workshop: what the game needs of it (a size one draw
//! takes, a small palette, a skeleton, a clip for each animation every figure plays), what is
//! missing, and the fixes: an animation given a clip of its files, the clips guessed from
//! their names, a rig template for a model without a skeleton.

use rustpg2d_engine::defs::{ClipMap, ModelDef};
use rustpg2d_engine::refactor::RefKind;

use crate::workspace::Workspace;

/// The animations every figure plays.
pub const BASE: [&str; 4] = ["walk", "idle", "attack", "hurt"];

/// Animations a figure can have besides: work, rest, weapon stances, sneaking.
pub const EXTRAS: [&str; 20] = ["sit", "sleep", "fish", "farm", "craft", "chop", "mine", "sweep", "read", "attack_1h", "attack_2h", "attack_thrust", "attack_bow", "attack_cast", "attack_unarmed", "idle_1h", "idle_2h", "block_shield", "block_weapon", "sneak"];

/// What is known of a model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checked {
    /// Why it isn't loaded (its file is missing or no model); everything else is empty then.
    pub error: Option<String>,
    /// What the check found: whether it is a problem, and the line.
    pub lines: Vec<(bool, String)>,
    /// The clips its files hold (a fitted rig template's for a model without a skeleton):
    /// name and seconds.
    pub clips: Vec<(String, f32)>,
    /// It has a skeleton of its own.
    pub rigged: bool,
    /// It is a piece worn by other models (it has no animations of its own).
    pub piece: bool,
    /// The animations of [`BASE`] that have no clip.
    pub missing: Vec<String>,
}

impl Checked {
    pub fn problems(&self) -> usize {
        self.lines.iter().filter(|l| l.0).count() + self.error.is_some() as usize
    }
}

/// The clip of `clips` an animation most likely plays, by the clips' names.
pub fn guess_for(anim: &str, clips: &[(String, f32)]) -> Option<ClipMap> {
    let names: Vec<String> = clips.iter().map(|c| c.0.clone()).collect();
    rustpg2d_engine::model::guess_clips(&names).into_iter().find(|c| c.anim == anim)
}

impl Workspace {
    /// Model `id` looked over (it must be loaded: the editor has a graphics adapter).
    pub fn model_checked(&self, id: &str) -> Checked {
        let db = &self.project.db;
        let Some(def) = db.model(id) else { return Checked { error: Some(format!("there is no model \"{id}\"")), ..Default::default() } };
        let models = &self.assets.models;
        let (Some(lines), Some(info)) = (models.check(db, id), models.info(id)) else {
            let why = models.errors.iter().find(|e| e.starts_with(&format!("model {id}:"))).cloned().unwrap_or_else(|| format!("{} isn't loaded", def.file));
            return Checked { error: Some(why), ..Default::default() };
        };
        let piece = def.part.is_some();
        let lines: Vec<(bool, String)> = lines.into_iter().map(|l| match l.strip_prefix("! ") {
            Some(p) => (true, p.to_string()),
            None => (false, l),
        }).collect();
        let missing = if piece { Vec::new() } else { BASE.iter().filter(|a| def.clip_of(a).is_none_or(|c| !info.clips.iter().any(|k| k.0 == c.clip))).map(|a| a.to_string()).collect() };
        Checked { error: None, lines, clips: info.clips, rigged: info.rigged, piece, missing }
    }

    /// The models a model without a skeleton can take one from: the rig templates (`rig_…`),
    /// then the figures players can be. Id and name.
    pub fn rig_templates(&self) -> Vec<(String, String)> {
        let db = &self.project.db;
        let usable = |m: &&ModelDef| m.part.is_none() && m.rig.is_none() && self.assets.models.info(&m.id).is_none_or(|i| i.rigged);
        let named = |m: &ModelDef| (m.id.clone(), if m.name.is_empty() { m.id.clone() } else { m.name.clone() });
        let mut out: Vec<(String, String)> = db.models.iter().filter(|m| m.id.starts_with("rig_")).filter(usable).map(named).collect();
        out.extend(db.models.iter().filter(|m| m.playable && !m.id.starts_with("rig_")).filter(usable).map(named));
        out
    }

    fn change_model(&mut self, id: &str, label: &str, change: impl FnOnce(&mut ModelDef, &rustpg2d_engine::defs::Database)) -> Result<bool, String> {
        let old = self.project.db.model(id).ok_or_else(|| format!("there is no model \"{id}\""))?.clone();
        let mut new = old.clone();
        change(&mut new, &self.project.db);
        if new == old {
            return Ok(false);
        }
        // (Joints of the same rig template moved are fitted again from what is kept: no files read.)
        let template = |d: &ModelDef| d.rig.as_ref().map(|r| r.template.clone());
        let files = (new.file != old.file, template(&new) != template(&old), new.clip_files != old.clip_files, new.shared_clips != old.shared_clips) != (false, false, false, false);
        let value = serde_json::to_value(&new).map_err(|e| e.to_string())?;
        self.set_def_value(RefKind::Model, id, value)?;
        let kept = self.assets.models.set_def(&new);
        let files = files || (new.rig != old.rig && !kept);
        self.begin(label);
        // (Another skeleton or other files: read again. Anything else the loaded model takes as it is.)
        if files && self.gpu != crate::Gpu::No {
            self.reload_assets();
        }
        self.stale = true;
        Ok(true)
    }

    /// Animation `anim` of model `id` plays `clip` (a clip of its files), or none any more
    /// (`None`: one of the four every figure plays is then missing, another is gone). One step.
    pub fn model_play(&mut self, id: &str, anim: &str, clip: Option<&str>) -> Result<(), String> {
        let anim = anim.trim().to_lowercase().replace([' ', '-'], "_");
        if anim.is_empty() {
            return Err("which animation?".into());
        }
        let name = self.project.db.model(id).map_or(id.to_string(), |m| if m.name.is_empty() { id.to_string() } else { m.name.clone() });
        let label = match clip {
            Some(c) => format!("{name}: {anim} plays {c}"),
            None => format!("{name}: {anim} plays nothing"),
        };
        self.change_model(id, &label, |d, _| match clip {
            None => d.clips.retain(|c| c.anim != anim),
            Some(clip) => match d.clips.iter_mut().find(|c| c.anim == anim) {
                Some(c) => c.clip = clip.to_string(),
                None => d.clips.push(ClipMap { anim: anim.clone(), clip: clip.to_string(), fps: if anim.starts_with("idle") { 8.0 } else { 12.0 }, hit: None }),
            },
        })?;
        Ok(())
    }

    /// How many frames a second animation `anim` of model `id` is drawn with.
    pub fn model_fps(&mut self, id: &str, anim: &str, fps: f32) -> Result<(), String> {
        let label = format!("Change how fast {id} plays {anim}");
        self.change_model(id, &label, |d, _| {
            if let Some(c) = d.clips.iter_mut().find(|c| c.anim == anim) {
                c.fps = fps.clamp(1.0, 60.0);
            }
        })?;
        Ok(())
    }

    /// Gives every animation of [`BASE`] and [`EXTRAS`] that has no clip the clip its name
    /// suggests (a clip called "Walking" for walk). One step; returns the animations mapped.
    pub fn model_guess(&mut self, id: &str) -> Result<Vec<String>, String> {
        let checked = self.model_checked(id);
        if let Some(e) = checked.error {
            return Err(e);
        }
        let mut mapped = Vec::new();
        let clips = checked.clips;
        let name = self.project.db.model(id).map_or(id.to_string(), |m| if m.name.is_empty() { id.to_string() } else { m.name.clone() });
        self.change_model(id, &format!("{name}: animations from its clips' names"), |d, _| {
            for anim in BASE.iter().chain(EXTRAS.iter()) {
                if d.clip_of(anim).is_some_and(|c| clips.iter().any(|k| k.0 == c.clip)) {
                    continue;
                }
                if let Some(g) = guess_for(anim, &clips) {
                    d.clips.retain(|c| c.anim != *anim);
                    mapped.push(format!("{anim}={}", g.clip));
                    d.clips.push(g);
                }
            }
        })?;
        Ok(mapped)
    }

    /// A rig template for model `id` ("" = none), and how far it is turned first to face the
    /// front (`None`: as it is). One step; the model is read again.
    pub fn model_rig(&mut self, id: &str, template: &str, turn: Option<f32>) -> Result<(), String> {
        let template = if template == "-" { "" } else { template };
        if !template.is_empty() {
            if template == id {
                return Err("a model can't be its own rig template".into());
            }
            self.project.db.model(template).ok_or_else(|| format!("there is no model \"{template}\" to take a skeleton from"))?;
            if self.assets.models.info(template).is_some_and(|i| !i.rigged) {
                return Err(format!("{template} has no skeleton of its own"));
            }
            if self.assets.models.info(id).is_some_and(|i| i.rigged) {
                return Err(format!("{id} has its own skeleton: rig templates are for models without one"));
            }
        }
        let name = self.project.db.model(id).map_or(id.to_string(), |m| if m.name.is_empty() { id.to_string() } else { m.name.clone() });
        let label = if template.is_empty() { format!("{name}: no rig template") } else { format!("{name}: the skeleton of {template}") };
        self.change_model(id, &label, |d, db| {
            let same = d.rig.as_ref().map_or("", |r| r.template.as_str()) == template;
            if !same {
                crate::models::choose_template(d, db, template);
            }
            if let (Some(rig), Some(turn)) = (&mut d.rig, turn) {
                rig.turn = turn.rem_euclid(360.0);
            }
        })?;
        Ok(())
    }

    /// The joints of model `id`'s rig template, by name, in the skeleton's order (empty without
    /// a template, or while the template isn't loaded).
    pub fn rig_joints(&self, id: &str) -> Vec<String> {
        let Some(rig) = self.project.db.model(id).and_then(|m| m.rig.as_ref()) else { return Vec::new() };
        self.assets.models.info(&rig.template).map(|i| i.bones).unwrap_or_default()
    }

    /// Joint `joint` of model `id`'s rig template moved by `offset` (x right, y up, z front; in
    /// parts of the model's height) from where the fit puts it; `None` or no move = where the
    /// fit puts it again. One step; the model is fitted again at once.
    pub fn model_joint(&mut self, id: &str, joint: &str, offset: Option<[f32; 3]>) -> Result<(), String> {
        let def = self.project.db.model(id).ok_or_else(|| format!("there is no model \"{id}\""))?;
        let Some(rig) = &def.rig else { return Err(format!("{id} has no rig template: its joints are its own")) };
        let joints = self.rig_joints(id);
        // (One moved before that the template doesn't have any more can still be put back.)
        let moved = rig.joints.iter().any(|j| j.joint == joint);
        // (While the template isn't loaded its joints aren't known: the name is taken as it is.)
        if !moved && !joints.is_empty() && !joints.iter().any(|j| j == joint) {
            return Err(format!("the rig template {} has no joint \"{joint}\"", rig.template));
        }
        let offset = offset.filter(|o| o.iter().any(|v| *v != 0.0));
        let name = def.label().to_string();
        let label = match offset {
            Some(_) => format!("{name}: the joint {joint} moved"),
            None => format!("{name}: the joint {joint} where the fit puts it"),
        };
        self.change_model(id, &label, |d, _| {
            let Some(rig) = &mut d.rig else { return };
            match (rig.joints.iter_mut().find(|j| j.joint == joint), offset) {
                (Some(j), Some(o)) => j.offset = o,
                (None, Some(o)) => rig.joints.push(rustpg2d_engine::defs::JointNudge { joint: joint.to_string(), offset: o }),
                (_, None) => rig.joints.retain(|j| j.joint != joint),
            }
        })?;
        Ok(())
    }

    /// Every joint of model `id`'s rig template where the fit puts it. One step.
    pub fn model_joints_reset(&mut self, id: &str) -> Result<(), String> {
        let name = self.project.db.model(id).map_or(id.to_string(), |m| m.label().to_string());
        self.change_model(id, &format!("{name}: every joint where the fit puts it"), |d, _| {
            if let Some(rig) = &mut d.rig {
                rig.joints.clear();
            }
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    /// The example with a few of its models loaded (models are read without a graphics adapter;
    /// only drawing them needs one).
    fn scratch(tag: &str, ids: &[&str]) -> (Workspace, std::path::PathBuf) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-modelcheck-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        load(&mut ws, ids);
        (ws, dir)
    }

    fn load(ws: &mut Workspace, ids: &[&str]) {
        let mut few = rustpg2d_engine::defs::Database { models: ws.project.db.models.iter().filter(|m| ids.contains(&m.id.as_str())).cloned().collect(), ..Default::default() };
        few.reindex();
        let project = &ws.project;
        ws.assets.models = rustpg2d_engine::render::model::Models::load(&few, &|rel| std::fs::read(project.path(rel)));
    }

    #[test]
    fn a_figure_is_checked_and_an_animation_without_a_clip_gets_one() {
        let (mut ws, dir) = scratch("clips", &["hero"]);
        // The hero is whole: a skeleton, clips, the four animations every figure plays.
        let c = ws.model_checked("hero");
        assert!(c.error.is_none() && c.rigged && !c.piece && c.missing.is_empty(), "{c:?}");
        assert!(c.problems() == 0 && c.clips.len() >= 4, "{c:?}");
        assert!(c.lines.iter().any(|l| l.1.contains("triangles")) && c.lines.iter().any(|l| l.1.contains("a figure")));
        let walk = ws.project.db.model("hero").unwrap().clip_of("walk").unwrap().clone();
        // Walking plays nothing any more: that is a problem, said as one, and one step.
        ws.model_play("hero", "walk", None).unwrap();
        ws.history_frame(None);
        let c = ws.model_checked("hero");
        assert_eq!(c.missing, ["walk"]);
        assert!(c.lines.iter().any(|l| l.0 && l.1.contains("no clip for the animation walk")), "{c:?}");
        assert_eq!(ws.timeline.done.len(), 1);
        // Guessed from the clips' names, it walks again with the clip it had.
        let mapped = ws.model_guess("hero").unwrap();
        ws.history_frame(None);
        assert!(mapped.iter().any(|m| *m == format!("walk={}", walk.clip)), "{mapped:?}");
        assert!(ws.model_checked("hero").missing.is_empty());
        // Picked by hand: any clip of its files, an animation of its own too; taken away again.
        let other = c.clips.iter().find(|k| k.0 != walk.clip).unwrap().0.clone();
        ws.model_play("hero", "walk", Some(&other)).unwrap();
        ws.model_play("hero", "Wave hello", Some(&other)).unwrap();
        ws.history_frame(None);
        let d = ws.project.db.model("hero").unwrap();
        assert_eq!((d.clip_of("walk").unwrap().clip.as_str(), d.clip_of("wave_hello").map(|c| c.clip.as_str())), (other.as_str(), Some(other.as_str())));
        ws.model_play("hero", "wave_hello", None).unwrap();
        ws.model_fps("hero", "walk", 20.0).unwrap();
        ws.history_frame(None);
        let d = ws.project.db.model("hero").unwrap();
        assert!(d.clip_of("wave_hello").is_none() && d.clip_of("walk").unwrap().fps == 20.0);
        // A model that isn't there, and one that isn't loaded, say so.
        assert!(ws.model_checked("nobody").error.is_some());
        assert!(ws.model_checked("heroine").error.is_some_and(|e| e.contains("isn't loaded")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_model_without_a_skeleton_takes_a_rig_templates() {
        // A wolf of the example stands on the skeleton of the four-legged template.
        let (mut ws, dir) = scratch("rig", &[]);
        let fitted = ws.project.db.models.iter().find(|m| m.rig.as_ref().is_some_and(|r| r.template == "rig_quadruped")).expect("the example has a four-legged creature").clone();
        load(&mut ws, &[&fitted.id, "rig_quadruped", "rig_bird"]);
        let c = ws.model_checked(&fitted.id);
        assert!(c.error.is_none() && !c.rigged && c.missing.is_empty() && !c.clips.is_empty(), "{c:?}");
        assert!(ws.rig_templates().iter().any(|t| t.0 == "rig_quadruped") && !ws.rig_templates().iter().any(|t| t.0 == fitted.id));
        // Without the template it has no skeleton and no animations: all four are missing.
        ws.model_rig(&fitted.id, "", None).unwrap();
        ws.history_frame(None);
        load(&mut ws, &[&fitted.id, "rig_quadruped", "rig_bird"]);
        assert!(ws.project.db.model(&fitted.id).unwrap().rig.is_none());
        let c = ws.model_checked(&fitted.id);
        assert!(!c.rigged && c.missing.len() == 4, "{c:?}");
        // Given the template again, turned a quarter: its clips are the template's, as before.
        ws.model_rig(&fitted.id, "rig_quadruped", Some(90.0)).unwrap();
        ws.history_frame(None);
        load(&mut ws, &[&fitted.id, "rig_quadruped", "rig_bird"]);
        let d = ws.project.db.model(&fitted.id).unwrap();
        assert_eq!(d.rig.as_ref().map(|r| (r.template.as_str(), r.turn)), Some(("rig_quadruped", 90.0)));
        assert!(ws.model_checked(&fitted.id).missing.is_empty());
        // What can't be: itself, a model that is none, a template for a model with a skeleton.
        assert!(ws.model_rig(&fitted.id, &fitted.id, None).is_err() && ws.model_rig(&fitted.id, "nobody", None).is_err());
        assert!(ws.model_rig("rig_bird", "rig_quadruped", None).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_joint_of_a_rig_template_is_moved_by_hand_and_put_back() {
        // The dog stands on the four-legged template's skeleton, its joints moved to fit it.
        let (mut ws, dir) = scratch("joints", &["dog", "rig_quadruped"]);
        let rig = |ws: &Workspace| ws.project.db.model("dog").unwrap().rig.clone().unwrap();
        let first = rig(&ws);
        assert_eq!(first.template, "rig_quadruped");
        let joints = ws.rig_joints("dog");
        assert!(joints.len() > 4 && joints.iter().any(|j| j == "Head"), "{joints:?}");
        let offset = |ws: &Workspace, joint: &str| rig(ws).joints.iter().find(|j| j.joint == joint).map(|j| j.offset);
        let was = offset(&ws, "Head").unwrap_or_default();
        let at = |ws: &Workspace| {
            let view = ws.assets.models.rig_view("dog").unwrap();
            (view.joints.iter().find(|j| j.0 == "Head").unwrap().1, view.hi.y - view.lo.y)
        };
        let (before, height) = at(&ws);
        // The head a tenth of the height higher: one step, and the model fitted again at once.
        let up = [was[0], was[1] + 0.1, was[2]];
        ws.model_joint("dog", "Head", Some(up)).unwrap();
        ws.history_frame(None);
        assert_eq!(offset(&ws, "Head"), Some(up));
        assert_eq!(rig(&ws).joints.len(), first.joints.len());
        assert_eq!(ws.timeline.done.len(), 1);
        assert!(ws.timeline.done[0].label.contains("the joint Head moved"), "{}", ws.timeline.done[0].label);
        let (after, _) = at(&ws);
        assert!((after.y - before.y - 0.1 * height).abs() < 0.02 * height, "{before} {after} {height}");
        // Set, not added to: the offset is the one given. Undone, it is the one before.
        ws.model_joint("dog", "Head", Some([0.0, 0.05, 0.0])).unwrap();
        ws.history_frame(None);
        assert_eq!(offset(&ws, "Head"), Some([0.0, 0.05, 0.0]));
        ws.undo();
        assert_eq!(offset(&ws, "Head"), Some(up));
        // No move (or none given) is where the fit puts it: the joint leaves the list.
        ws.model_joint("dog", "Head", Some([0.0; 3])).unwrap();
        ws.history_frame(None);
        assert_eq!((offset(&ws, "Head"), rig(&ws).joints.len()), (None, first.joints.len() - 1));
        ws.model_joint("dog", "Tail", None).unwrap();
        ws.history_frame(None);
        assert_eq!((offset(&ws, "Tail"), rig(&ws).joints.len()), (None, first.joints.len() - 2));
        // A joint the template doesn't have, and a model without a template, are refused in words.
        assert_eq!(ws.model_joint("dog", "Antenna", Some([0.0, 0.1, 0.0])).unwrap_err(), "the rig template rig_quadruped has no joint \"Antenna\"");
        assert_eq!(ws.model_joint("rig_quadruped", "Head", Some([0.0, 0.1, 0.0])).unwrap_err(), "rig_quadruped has no rig template: its joints are its own");
        // All put back in one step.
        let steps = ws.timeline.done.len();
        ws.model_joints_reset("dog").unwrap();
        ws.history_frame(None);
        assert!(rig(&ws).joints.is_empty());
        assert_eq!(ws.timeline.done.len(), steps + 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
