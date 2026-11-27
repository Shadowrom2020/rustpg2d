//! A character's look, worked out for the character workshop: the colour features of its
//! model and of the pieces it wears, the slots that model can wear a piece in and what fits,
//! what choosing another model does.

use rustpg2d_engine::defs::{Appearance, ColorFeature, Database, NO_PART};

/// The colour features of a look: its model's, then those of the pieces it wears (a feature
/// counts once; the first colours offered for it are its swatches).
pub fn look_features(db: &Database, a: &Appearance) -> Vec<ColorFeature> {
    let mut out: Vec<ColorFeature> = db.model(&a.model).map(|m| m.features.clone()).unwrap_or_default();
    for id in rustpg2d_engine::render::model::parts_of(db, &a.model, a, &Default::default()) {
        for f in db.model(&id).map_or(&[][..], |m| &m.features[..]) {
            match out.iter_mut().find(|o| o.name == f.name) {
                Some(o) if o.colors.is_empty() => o.colors = f.colors.clone(),
                Some(_) => {}
                None => out.push(f.clone()),
            }
        }
    }
    out
}

/// A slot a model can wear a piece in.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceSlot {
    pub slot: String,
    /// The pieces that fit the model: (model id, what it is called).
    pub pieces: Vec<(String, String)>,
    /// The model's own piece of that slot.
    pub own: Option<String>,
}

impl PieceSlot {
    /// "hair_style" -> "Hair style".
    pub fn label(&self) -> String {
        let mut c = self.slot.replace('_', " ");
        if let Some(f) = c.get_mut(0..1) {
            f.make_ascii_uppercase();
        }
        c
    }

    /// What choosing nothing shows: the model's own piece.
    pub fn own_label(&self) -> String {
        match self.own.as_ref().and_then(|o| self.pieces.iter().find(|p| p.0 == *o)) {
            Some((_, l)) => format!("The model's own ({l})"),
            None => "The model's own (none)".to_string(),
        }
    }

    /// What a look's choice for this slot is called.
    pub fn chosen_label(&self, chosen: Option<&str>) -> String {
        match chosen {
            None => self.own_label(),
            Some(NO_PART) => "None".to_string(),
            Some(c) => self.pieces.iter().find(|p| p.0 == c).map_or_else(|| c.to_string(), |p| p.1.clone()),
        }
    }
}

/// The slots model `body` can wear a piece in, by name.
pub fn piece_slots(db: &Database, body: &str) -> Vec<PieceSlot> {
    let mut slots: Vec<PieceSlot> = Vec::new();
    for m in &db.models {
        let Some(p) = m.part.as_ref().filter(|p| p.fits.is_empty() || p.fits.iter().any(|f| f == body)) else { continue };
        let label = if p.style.is_empty() { m.label().to_string() } else { p.label() };
        match slots.iter_mut().find(|s| s.slot == p.slot) {
            Some(s) => s.pieces.push((m.id.clone(), label)),
            None => slots.push(PieceSlot { slot: p.slot.clone(), pieces: vec![(m.id.clone(), label)], own: None }),
        }
    }
    let own: Vec<&String> = db.model(body).map_or(Vec::new(), |b| b.parts.iter().collect());
    for s in &mut slots {
        s.own = s.pieces.iter().find(|(id, _)| own.contains(&id)).map(|(id, _)| id.clone());
    }
    slots.sort_by(|a, b| a.slot.cmp(&b.slot));
    slots
}

/// Another model for a look: its colours and pieces go (the other model has other features
/// and wears other pieces). The same model again changes nothing.
pub fn set_model(a: &mut Appearance, model: &str) {
    if a.model != model {
        a.colors.clear();
        a.parts.clear();
    }
    a.model = model.to_string();
    a.body.clear();
}

/// A piece for a slot: `None` = the model's own, `Some(NO_PART)` = none, else a piece's id.
pub fn set_piece(a: &mut Appearance, slot: &str, piece: Option<&str>) {
    match piece {
        Some(p) => a.parts.insert(slot.to_string(), p.to_string()),
        None => a.parts.remove(slot),
    };
}

/// One of the offered colours for every colour feature that offers some, picked by `seed`.
pub fn random_colors(a: &mut Appearance, db: &Database, seed: u64) {
    let mut s = seed | 1;
    for f in look_features(db, a).iter().filter(|f| !f.colors.is_empty()) {
        // (xorshift: any spread will do.)
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        a.colors.insert(f.name.clone(), f.colors[(s % f.colors.len() as u64) as usize]);
    }
}

/// The models a character can be shown as: figures, not the pieces figures wear nor the rig
/// templates; (id, name).
pub fn figures(db: &Database) -> Vec<(String, String)> {
    db.models.iter().filter(|m| m.part.is_none() && !m.id.starts_with("rig_")).map(|m| (m.id.clone(), m.label().to_string())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::project::Project;

    fn example() -> Database {
        Project::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project")).unwrap().db
    }

    #[test]
    fn a_looks_slots_and_features_come_from_its_model_and_pieces() {
        let db = example();
        // A character of the example with a figure that wears pieces.
        let c = db.characters.iter().find(|c| !piece_slots(&db, &c.appearance.model).is_empty()).expect("the example has a figure that wears pieces");
        let slots = piece_slots(&db, &c.appearance.model);
        assert!(slots.windows(2).all(|w| w[0].slot < w[1].slot), "by name");
        for s in &slots {
            assert!(!s.pieces.is_empty());
            // Every piece fits this body and is of this slot.
            for (id, label) in &s.pieces {
                let part = db.model(id).and_then(|m| m.part.as_ref()).unwrap();
                assert!(part.slot == s.slot && (part.fits.is_empty() || part.fits.contains(&c.appearance.model)) && !label.is_empty());
            }
            assert_eq!(s.chosen_label(Some(NO_PART)), "None");
            assert!(s.chosen_label(None).starts_with("The model's own"));
            assert_eq!(s.chosen_label(Some(&s.pieces[0].0)), s.pieces[0].1);
        }
        // A piece chosen brings its colour features; each feature is there once.
        let mut a = c.appearance.clone();
        let base = look_features(&db, &a);
        for s in &slots {
            for (id, _) in &s.pieces {
                set_piece(&mut a, &s.slot, Some(id));
                let with = look_features(&db, &a);
                let mut names: Vec<&str> = with.iter().map(|f| f.name.as_str()).collect();
                let n = names.len();
                names.sort();
                names.dedup();
                assert_eq!(names.len(), n, "{id}: a feature twice");
                assert!(with.len() >= db.model(&a.model).map_or(0, |m| m.features.len()));
            }
            set_piece(&mut a, &s.slot, None);
        }
        assert_eq!(look_features(&db, &a), base);
        // Figures are offered as models, pieces and rig templates are not.
        let figs = figures(&db);
        assert!(figs.iter().any(|(id, _)| *id == c.appearance.model));
        assert!(figs.iter().all(|(id, _)| db.model(id).is_some_and(|m| m.part.is_none())));
    }

    #[test]
    fn another_model_drops_the_colours_and_pieces_of_the_old_one() {
        let db = example();
        let figs = figures(&db);
        let mut a = Appearance { model: figs[0].0.clone(), ..Default::default() };
        a.colors.insert("hair".into(), [1, 2, 3]);
        a.parts.insert("hair".into(), NO_PART.into());
        let same = a.clone();
        set_model(&mut a, &figs[0].0);
        assert_eq!(a, same, "the same model again keeps them");
        set_model(&mut a, &figs[1].0);
        assert!(a.colors.is_empty() && a.parts.is_empty() && a.model == figs[1].0);
        // Random colours are offered ones, and the same for the same seed.
        let c = db.characters.iter().find(|c| look_features(&db, &c.appearance).iter().any(|f| f.colors.len() > 1)).expect("a figure with colours to choose");
        let (mut x, mut y) = (c.appearance.clone(), c.appearance.clone());
        random_colors(&mut x, &db, 7);
        random_colors(&mut y, &db, 7);
        assert_eq!(x, y);
        for f in look_features(&db, &x).iter().filter(|f| !f.colors.is_empty()) {
            assert!(f.colors.contains(&x.colors[&f.name]));
        }
    }
}
