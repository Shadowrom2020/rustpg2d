//! The agent server's `art_*` tools: Generate art for AI agents, with the creator's money
//! guarded (the price must be confirmed per call, the session budget holds).

use super::{api_key, convert, meshy, ArtKind, ArtRequest, Stage, ATTACKS};
use crate::app::Editor;
use crate::mcp::exec::{text, Reply};
use crate::mcp::tools::{ArtApplyParams, ArtJobParams, ArtRedoParams, GenerateArtParams};

impl Editor {
    pub(crate) fn mcp_art_settings(&mut self) -> Result<Reply, String> {
        let s = &self.art_settings;
        let has = !api_key().is_empty() || std::env::var_os("RUSTPG2D_MESHY_FAKE").is_some();
        let spent = *self.art.spent.lock().unwrap();
        text(format!(
            "Meshy key: {}. Model: {} ({} credits a picture). Session budget: {} credits, {} used, {} left. A character costs {} credits (twice that with an attack), anything else {}. 3D: a model {} credits, a rig {}, each animation clip {} (a 3D character = turnaround picture + model + rig + 4-5 clips, about {} credits; a creature = picture + model; an item's model = model).\nCharacter sizes: {}.\nAttacks: {}.\nCharacters are drawn in the style of: {}.",
            if has { "set" } else { "not set (the person enters it in Edit › Settings… › Meshy)" },
            s.model,
            meshy::model_cost(&s.model),
            s.budget,
            spent,
            s.budget.saturating_sub(spent),
            meshy::model_cost(&s.model),
            meshy::model_cost(&s.model),
            super::COST_MODEL,
            super::COST_RIG,
            super::COST_CLIP,
            meshy::model_cost(&s.model) + super::COST_MODEL + super::COST_RIG + 5 * super::COST_CLIP,
            convert::SIZES.iter().map(|z| format!("{} ({})", z.id, z.label)).collect::<Vec<_>>().join(", "),
            ATTACKS.iter().map(|a| format!("{} ({})", a.0, a.1)).collect::<Vec<_>>().join(", "),
            self.art_references().from
        ))
    }

    pub(crate) fn mcp_generate_art(&mut self, p: GenerateArtParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let kind = ArtKind::parse(&p.kind).ok_or_else(|| format!("kind \"{}\" must be character, icon, prop, portrait, image, model, creature or gear", p.kind))?;
        let tiles = p.tiles.unwrap_or([1, 1]);
        let req = ArtRequest {
            kind,
            id: p.id.trim().to_string(),
            description: p.description,
            size: p.size.unwrap_or_else(|| "person".into()),
            attack: p.attack.unwrap_or_else(|| "none".into()),
            tiles: (tiles[0], tiles[1]),
            portrait_for: p.portrait_for.unwrap_or_default(),
            by_agent: true,
            female: p.female.unwrap_or(false),
            extras: p.extras.unwrap_or_default(),
            from: p.from.unwrap_or_default(),
        };
        req.check().map_err(|e| format!("{e:#}"))?;
        // What it is made from must exist before anyone is asked for credits.
        self.art_references_for(&req).map_err(|e| format!("{e:#}"))?;
        let cost = super::ArtState::cost(&req, &self.art_settings.model);
        if p.confirm_credits != Some(cost) {
            let pc = meshy::model_cost(&self.art_settings.model);
            let steps: Vec<String> = req.pictures().iter().map(|n| format!("{n} {}", req.step_cost(n, pc))).collect();
            return Err(format!(
                "this costs {cost} credits of the person's Meshy account ({}; {} left in this session's budget). Ask them, then call again with confirm_credits: {cost}.",
                steps.join(", "),
                self.art.budget_left(&self.art_settings)
            ));
        }
        let id = self.start_art(req).map_err(|e| format!("{e:#}"))?;
        let wait = if kind.is_3d() { "A 3D job takes a few minutes (each step 1-3)" } else { "A picture takes about a minute" };
        text(format!("Started art job {id} ({cost} credits). {wait}: watch it with art_jobs, then art_result {{job: {id}}} and art_apply."))
    }

    pub(crate) fn mcp_art_jobs(&mut self) -> Result<Reply, String> {
        if self.art.jobs.is_empty() {
            return text("No art jobs this session.");
        }
        let lines: Vec<String> = self.art.jobs.iter().map(|j| j.lock().unwrap().line()).collect();
        text(lines.join("\n"))
    }

    pub(crate) fn mcp_art_result(&mut self, p: ArtJobParams) -> Result<Reply, String> {
        let j = self.art.job(p.job).ok_or_else(|| format!("no art job {}", p.job))?.lock().unwrap().clone();
        match &j.stage {
            Stage::Drawing | Stage::Converting => return text(format!("{} (not finished yet)", j.line())),
            Stage::Failed(_) => return text(j.line()),
            Stage::Done => {}
        }
        let mut images = Vec::new();
        let mut note = j.line();
        if let Some(png) = &j.result {
            // Small art is enlarged so details show.
            let im = rustpg2d_engine::imageio::Rgba::decode(png).map_err(|e| e.to_string())?;
            let k = if im.w.max(im.h) <= 256 { 3 } else { 1 };
            images.push(if k > 1 { im.resized(im.w * k, im.h * k).encode().map_err(|e| e.to_string())? } else { png.clone() });
            note += &format!("\nResult: {}x{} px, frames {}x{}", im.w, im.h, j.frame.0, j.frame.1);
            if j.req.kind == ArtKind::Character {
                note += " (rows: 0-3 walk, 4-7 idle, 8-11 attack, 12-15 hurt; facings down, left, right, up).";
            }
        }
        if j.req.kind.is_3d() {
            let files: Vec<String> = j.made.files.iter().map(|(f, b)| format!("{f} ({} KB)", b.len() / 1024)).collect();
            note += &format!("\nMade: {}. art_apply adds it; then model_info shows the model and its clips.", files.join(", "));
        }
        for pic in &j.pictures {
            if let Some(png) = &pic.png {
                images.push(png.clone());
                note += &format!("\nThen the raw {} picture from Meshy.", pic.name);
            }
        }
        Ok(Reply::TextImages(note, images))
    }

    pub(crate) fn mcp_art_apply(&mut self, p: ArtApplyParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let id = match p.id {
            Some(id) => id,
            None => self.art.job(p.job).ok_or_else(|| format!("no art job {}", p.job))?.lock().unwrap().req.id.clone(),
        };
        let msg = self.apply_art(p.job, &id).map_err(|e| format!("{e:#}"))?;
        self.watch_resnapshot();
        text(msg)
    }

    pub(crate) fn mcp_art_redo(&mut self, p: ArtRedoParams) -> Result<Reply, String> {
        let job = self.art.job(p.job).ok_or_else(|| format!("no art job {}", p.job))?.lock().unwrap().clone();
        let steps = super::ArtState::redo_steps(&job, &p.picture);
        let cost = super::ArtState::redo_cost(&job, &p.picture);
        let what = if job.req.kind.is_3d() { format!("making the {} again", steps.join(", ")) } else { format!("drawing the {} picture again", p.picture) };
        if p.confirm_credits != Some(cost) {
            return Err(format!("{what} costs {cost} credits; ask the person, then call with confirm_credits: {cost}"));
        }
        self.redo_art(p.job, &p.picture).map_err(|e| format!("{e:#}"))?;
        text(format!("Job {}: {what} ({cost} credits); art_jobs shows the progress.", p.job))
    }
}
