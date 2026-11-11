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
            "Meshy key: {}. Model: {} ({} credits a picture). Session budget: {} credits, {} used, {} left. A character costs {} credits (twice that with an attack), anything else {}.\nCharacter sizes: {}.\nAttacks: {}.\nCharacters are drawn in the style of: {}.",
            if has { "set" } else { "not set (the person enters it in Edit › Settings… › Meshy)" },
            s.model,
            meshy::model_cost(&s.model),
            s.budget,
            spent,
            s.budget.saturating_sub(spent),
            meshy::model_cost(&s.model),
            meshy::model_cost(&s.model),
            convert::SIZES.iter().map(|z| format!("{} ({})", z.id, z.label)).collect::<Vec<_>>().join(", "),
            ATTACKS.iter().map(|a| format!("{} ({})", a.0, a.1)).collect::<Vec<_>>().join(", "),
            self.art_references().from
        ))
    }

    pub(crate) fn mcp_generate_art(&mut self, p: GenerateArtParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let kind = ArtKind::parse(&p.kind).ok_or_else(|| format!("kind \"{}\" must be character, icon, prop, portrait or image", p.kind))?;
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
        };
        req.check().map_err(|e| format!("{e:#}"))?;
        let cost = super::ArtState::cost(&req, &self.art_settings.model);
        if p.confirm_credits != Some(cost) {
            return Err(format!(
                "this costs {cost} credits of the person's Meshy account ({} pictures at {} credits; {} left in this session's budget). Ask them, then call again with confirm_credits: {cost}.",
                req.pictures().len(),
                meshy::model_cost(&self.art_settings.model),
                self.art.budget_left(&self.art_settings)
            ));
        }
        let id = self.start_art(req).map_err(|e| format!("{e:#}"))?;
        text(format!("Started art job {id} ({cost} credits). A picture takes about a minute: watch it with art_jobs, then art_result {{job: {id}}} and art_apply."))
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
        let cost = meshy::model_cost(&self.art_settings.model);
        if p.confirm_credits != Some(cost) {
            return Err(format!("drawing the {} picture again costs {cost} credits; ask the person, then call with confirm_credits: {cost}", p.picture));
        }
        self.redo_art(p.job, &p.picture).map_err(|e| format!("{e:#}"))?;
        text(format!("Drawing the {} picture of job {} again ({cost} credits); art_jobs shows the progress.", p.picture, p.job))
    }
}
