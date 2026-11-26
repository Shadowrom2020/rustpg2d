//! Small things about definitions that tools and agents share.

use rustpg2d_engine::defs::{Appearance, CharacterDef, Database, SpriteRef};
use rustpg2d_engine::entity::EntityKind;

use crate::workspace::Workspace;

/// A character definition as a sprite reference: `@char:<id>`, painted from its figure.
pub const CHAR_PREFIX: &str = "@char:";

/// A fresh character: the first playable model of the project (else none: the mannequin), with
/// walking speed and hitbox of a person.
pub fn default_character(db: &Database, id: &str, name: &str) -> CharacterDef {
    let model = db.models.iter().find(|m| m.playable).map(|m| m.id.clone()).unwrap_or_default();
    let u = rustpg2d_engine::project::TILE_SIZE as f32 / 16.0;
    CharacterDef {
        name: name.into(),
        speed: 50.0 * u,
        hitbox: [10.0 * u, 6.0 * u],
        appearance: Appearance { model, ..Default::default() },
        ..CharacterDef::fresh(id)
    }
}

/// The thumbnail sprite of a character definition: a `@char:` marker that [`paint_sprite`]
/// paints from its layers.
pub fn character_sprite(d: &rustpg2d_engine::defs::CharacterDef) -> SpriteRef {
    SpriteRef::new(&format!("{CHAR_PREFIX}{}", d.id), 0)
}

/// Id and name of every definition of `kind`, in the order of its file (the name is the id for
/// kinds without names).
pub fn list(db: &Database, kind: rustpg2d_engine::refactor::RefKind) -> Vec<(String, String)> {
    use rustpg2d_engine::defs::Def;
    rustpg2d_engine::with_def_kind!(kind, |T| T::list(db)
        .iter()
        .map(|d| {
            let name = serde_json::to_value(d).ok().and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string)).filter(|n| !n.is_empty());
            (d.id().to_string(), name.unwrap_or_else(|| d.id().to_string()))
        })
        .collect(), else => Vec::new())
}

/// A definition as the text of its file entry (RON).
pub fn ron_of(db: &Database, kind: rustpg2d_engine::refactor::RefKind, id: &str) -> Option<String> {
    use rustpg2d_engine::defs::Def;
    rustpg2d_engine::with_def_kind!(kind, |T| T::list(db).iter().find(|d| d.id() == id).and_then(|d| rustpg2d_engine::project::ron_text(d).ok()), else => None)
}

impl Workspace {
    /// Sprite of an entity definition (props / items; characters a `@char:` marker that
    /// `widgets::paint_sprite` paints from their figure).
    pub fn def_sprite(&self, kind: EntityKind, def: &str) -> Option<rustpg2d_engine::defs::SpriteRef> {
        let db = &self.project.db;
        match kind {
            EntityKind::Prop => db.prop(def).map(|d| d.sprite.clone()),
            EntityKind::Item => db.item(def).map(|d| d.icon.clone()),
            EntityKind::Character => db.character(def).map(character_sprite),
        }
    }

    /// Width of an entity definition in whole tiles (for snapping): a prop's hitbox, else its
    /// sprite (a prop's picture of its model can be narrower than its footprint).
    pub fn def_tiles_wide(&self, kind: EntityKind, def: &str) -> u32 {
        let ts = self.ts();
        if let Some(p) = (kind == EntityKind::Prop).then(|| self.project.db.prop(def)).flatten() {
            return (p.hitbox[0] / ts).round().max(1.0) as u32;
        }
        self.def_sprite(kind, def).map_or(1, |s| (self.assets.sprite_size(&s, ts).x / ts).round().max(1.0) as u32)
    }

    /// Writes `lang/<code>.ron` with every content text of the project (saving everything first,
    /// so the maps' texts are current): existing translations are kept, new texts are added
    /// empty. Adds the language to the project's translations. Returns a summary.
    pub fn export_translation(&mut self, code: &str) -> Result<String, String> {
        let code = code.trim().to_lowercase();
        if code.len() < 2 || !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(format!("\"{code}\" is not a language code (like de, fr or pt-br)"));
        }
        if code == self.project.config.language {
            return Err(format!("{code} is the game's own language"));
        }
        if !self.project.config.languages.contains(&code) {
            self.project.config.languages.push(code.clone());
            self.defs_dirty = true;
        }
        self.save_all();
        let existing = rustpg2d_engine::lang::load_file(&self.project.root, &code);
        let (file, unused) = rustpg2d_engine::lang::template(&self.project, &code, existing);
        let path = self.project.root.join("lang").join(format!("{code}.ron"));
        rustpg2d_engine::project::write_ron(&path, &file).map_err(|e| format!("{e:#}"))?;
        self.watch_note(&path);
        let open = file.texts.values().filter(|v| v.is_empty()).count();
        let mut msg = format!("lang/{code}.ron: {} texts, {open} not translated yet", file.texts.len());
        if !unused.is_empty() {
            msg += &format!(", {} no longer used (kept, at the end of the list in your editor: delete them by hand)", unused.len());
        }
        Ok(msg)
    }

}
