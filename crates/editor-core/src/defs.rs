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

/// What a kind of definition is, in a sentence (the *New…* gallery, hover texts).
pub fn about(kind: rustpg2d_engine::refactor::RefKind) -> &'static str {
    use rustpg2d_engine::refactor::RefKind::*;
    match kind {
        Sheet => "A picture file cut into frames: icons, portraits, effects, a prop's picture.",
        Item => "Something carried: a weapon, armour, a potion, a key, a tool, money.",
        Prop => "Something that stands on a map: furniture, a chest, a door, a tree, a wall, a building.",
        Character => "Someone or something that lives: a villager, a merchant, a monster, the hero.",
        Sound => "A sound effect or a piece of music.",
        Quest => "A task for the player, with its steps and rewards.",
        Structure => "A piece of map kept to place again: a house, a room, a wall with its gate.",
        Spell => "A spell: how it is cast, whom it reaches and what it does.",
        Recipe => "How something is made: ingredients, tools, where, and what comes out.",
        Achievement => "A deed the game remembers for the player.",
        Race => "A people: looks, size, speed, traits (humans, elves, goblins).",
        Model => "A 3D figure or a piece of one: a body, hair, an outfit, a creature.",
        Material => "Ground to paint maps with: grass, a path, water, a floor.",
        Scatter => "A set of props the scatter brush spreads: a forest, a flower meadow.",
        Font => "A font for the game's texts.",
        Skin => "The look of the game's windows, buttons and bars.",
        CommonEvent => "A list of actions with a name, called from anywhere or running by itself.",
        Background => "What the hero was before the adventure: proficiencies and gear.",
        Class => "A class with its levels and features (fighter, wizard).",
        Disease => "An illness: how it is caught, what it does, how it ends.",
        Cutscene => "A scene on a timeline: who walks where, says what, when.",
        Animation => "An effect drawn over the map: a slash, an explosion, sparks.",
        VisualScript => "A script function made of nodes instead of typed code.",
        Skill => "A proficiency: a skill or a tool of the rules.",
        Map => "A place of the game: its ground, what stands on it, who lives there.",
    }
}

/// An id made from a name: `Potion of Healing` -> `potion_of_healing` (letters, digits and `_`;
/// `thing` when nothing is left).
pub fn id_from_name(name: &str) -> String {
    let mut id = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if !id.is_empty() && !id.ends_with('_') {
            id.push('_');
        }
    }
    let id = id.trim_end_matches('_');
    if id.is_empty() {
        "thing".to_string()
    } else {
        id.to_string()
    }
}

/// The category of every definition of `kind`, told from what it is: `(id, rank, label)`, the
/// rank for the order categories are listed in. Empty for kinds without categories. (Lists use
/// it to group and filter: items by the SRD's categories, props by what they do, characters by
/// creature type, spells by level; the creator's own tags come on top.)
pub fn categories(db: &Database, kind: rustpg2d_engine::refactor::RefKind) -> Vec<(String, u32, String)> {
    use rustpg2d_engine::item_category::{self, ItemCat};
    use rustpg2d_engine::refactor::RefKind;
    let row = |id: &str, rank: usize, label: &str| (id.to_string(), rank as u32, label.to_string());
    match kind {
        RefKind::Item => {
            let ammo = item_category::ammo_of(db);
            db.items
                .iter()
                .map(|d| {
                    let c = item_category::category(d, &ammo);
                    row(&d.id, ItemCat::ALL.iter().position(|x| *x == c).unwrap_or(99), c.label())
                })
                .collect()
        }
        RefKind::Prop => db
            .props
            .iter()
            .map(|d| {
                let (rank, label) = if d.building.is_some() || d.block > 0.0 || !d.pieces.is_empty() {
                    (0, "Walls and buildings")
                } else if d.door {
                    (1, "Doors and gates")
                } else if d.container.is_some() {
                    (2, "Containers")
                } else if d.use_spot.is_some() {
                    (3, "Furniture to use")
                } else if d.light.is_some() {
                    (4, "Lights")
                } else if d.gather.is_some() || d.plot {
                    (5, "Gathering and farming")
                } else if d.trap.is_some() {
                    (6, "Traps")
                } else if d.lock.is_some() || !d.triggers.is_empty() || d.states.len() > 1 {
                    (7, "Things that react")
                } else if d.carry.is_some() {
                    (8, "Things to carry")
                } else {
                    (9, "Scenery")
                };
                row(&d.id, rank, label)
            })
            .collect(),
        RefKind::Character => db
            .characters
            .iter()
            .map(|d| match d.stats.as_ref().map(|s| s.creature_type.split(" (").next().unwrap_or("").trim().to_lowercase()).filter(|t| !t.is_empty()) {
                // "humanoid (any race)" -> Humanoids
                Some(t) => {
                    let label = format!("{}{}s", t[..1].to_uppercase(), &t[1..]);
                    (d.id.clone(), if t == "humanoid" { 1 } else { 2 + t.bytes().next().unwrap_or(b'a') as u32 }, label)
                }
                None => row(&d.id, 0, "Without a stat block"),
            })
            .collect(),
        RefKind::Spell => db.spells.iter().map(|d| row(&d.id, d.level as usize, &if d.level == 0 { "Cantrips".to_string() } else { format!("Level {}", d.level) })).collect(),
        RefKind::Model => db
            .models
            .iter()
            .map(|d| {
                if d.part.is_some() {
                    row(&d.id, 2, "Pieces (hair, outfits, headwear)")
                } else if d.id.starts_with("rig_") {
                    row(&d.id, 3, "Rig templates")
                } else if d.playable {
                    row(&d.id, 0, "Offered in the character creator")
                } else {
                    row(&d.id, 1, "Figures, creatures and things")
                }
            })
            .collect(),
        RefKind::Structure => {
            let mut names: Vec<&str> = db.structures.iter().map(|d| d.category.as_str()).collect();
            names.sort_unstable();
            names.dedup();
            db.structures.iter().map(|d| row(&d.id, names.iter().position(|n| *n == d.category).unwrap_or(0), if d.category.is_empty() { "Other" } else { &d.category })).collect()
        }
        RefKind::Sheet => db.sheets.iter().map(|d| row(&d.id, d.kind as usize, &format!("{:?}", d.kind))).collect(),
        _ => Vec::new(),
    }
}

/// A definition as the text of its file entry (RON).
pub fn ron_of(db: &Database, kind: rustpg2d_engine::refactor::RefKind, id: &str) -> Option<String> {
    use rustpg2d_engine::defs::Def;
    rustpg2d_engine::with_def_kind!(kind, |T| T::list(db).iter().find(|d| d.id() == id).and_then(|d| rustpg2d_engine::project::ron_text(d).ok()), else => None)
}

impl Workspace {
    /// Makes a new definition from its name: the id is made from the name (and a number when
    /// it is taken), the definition starts blank or, with `from`, as a copy of that one, and
    /// gets the name. One step of the history. Returns the id.
    pub fn make_def(&mut self, kind: rustpg2d_engine::refactor::RefKind, name: &str, from: Option<&str>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("it needs a name".into());
        }
        let id = rustpg2d_engine::refactor::unique_id(&id_from_name(name), |c| self.project.db.has(kind, c));
        self.begin(&format!("New {}: {name}", kind.label().to_lowercase()));
        match from {
            Some(from) => self.duplicate_def(kind, from, &id)?,
            None => self.new_def(kind, &id)?,
        }
        // (A quest has a title, most others a name, a few only their id.)
        if let Some(mut value) = self.def_value(kind, &id) {
            let field = ["name", "title"].into_iter().find(|f| value.get(*f).is_some_and(|v| v.is_string()));
            if let Some(field) = field {
                value[field] = serde_json::Value::String(name.to_string());
                self.set_def_value(kind, &id, value)?;
            }
        }
        Ok(id)
    }

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

#[cfg(test)]
mod tests {
    use rustpg2d_engine::refactor::RefKind;

    #[test]
    fn definitions_are_put_into_categories_by_what_they_are() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let db = rustpg2d_engine::project::Project::load(&dir).unwrap().db;
        let of = |kind: RefKind, id: &str| super::categories(&db, kind).into_iter().find(|(i, ..)| i == id).map(|(_, _, label)| label);
        assert_eq!(of(RefKind::Item, "iron_sword").as_deref(), Some("Martial melee weapons"));
        assert_eq!(of(RefKind::Prop, "chest").as_deref(), Some("Containers"));
        assert_eq!(of(RefKind::Spell, "fire-bolt").as_deref(), Some("Cantrips"));
        assert_eq!(super::categories(&db, RefKind::Item).len(), db.items.len(), "every item has one");
        assert!(super::categories(&db, RefKind::Quest).is_empty(), "quests have none");
        assert_eq!(super::id_from_name("  Potion of Healing! "), "potion_of_healing");
        assert_eq!(super::id_from_name("+1 Sword"), "1_sword");
        assert_eq!(super::id_from_name("???"), "thing");
        // Ranks give the categories' order: the same label never has two ranks.
        for kind in RefKind::ALL {
            let mut seen = std::collections::HashMap::new();
            for (_, rank, label) in super::categories(&db, kind) {
                assert_eq!(*seen.entry(label.clone()).or_insert(rank), rank, "{kind:?}: {label}");
            }
        }
    }
}
