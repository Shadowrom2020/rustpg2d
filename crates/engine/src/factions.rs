//! Factions (*Database › Project › Factions*, `ProjectConfig::factions`): who is hostile to whom.
//!
//! Each faction has an id and a name; an attitude table says how two factions treat each other
//! (hostile, neutral — the default — or friendly; a faction is friendly to itself). Characters
//! belong to their own *Faction* or else their race's. Characters of hostile factions fight each
//! other. The player has a standing with every faction (`WorldState::standing`), starting from the
//! attitude between the player's faction (their race's, else the player character's) and that
//! faction; a faction whose standing is at [`HOSTILE_AT`] or below is hostile to the player.
//! *Hostile* on a character makes it attack the player unless the player is a friend of its faction
//! (standing [`FRIENDLY_AT`] or more), so projects without factions work as before.

use serde::{Deserialize, Serialize};

/// How two factions treat each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attitude {
    Hostile,
    #[default]
    Neutral,
    Friendly,
}

impl Attitude {
    pub const ALL: [Attitude; 3] = [Attitude::Hostile, Attitude::Neutral, Attitude::Friendly];

    pub fn label(self) -> &'static str {
        match self {
            Attitude::Hostile => "hostile",
            Attitude::Neutral => "neutral",
            Attitude::Friendly => "friendly",
        }
    }

    /// The player's standing this attitude starts from.
    pub fn standing(self) -> i32 {
        match self {
            Attitude::Hostile => -50,
            Attitude::Neutral => 0,
            Attitude::Friendly => 50,
        }
    }
}

/// Standing at or below which a faction is hostile to the player.
pub const HOSTILE_AT: i32 = -20;
/// Standing from which a faction's *Hostile* characters leave the player alone.
pub const FRIENDLY_AT: i32 = 20;
/// The player's standing with their own faction.
pub const OWN_STANDING: i32 = 100;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Faction {
    pub id: String,
    pub name: String,
}

/// Whether a character attacks the player: `marked` = its *Hostile* box, `standing` = the
/// player's standing with its faction (`None`: no faction of the project).
pub fn attacks_player(marked: bool, standing: Option<i32>) -> bool {
    match standing {
        None => marked,
        Some(s) => s <= HOSTILE_AT || (marked && s < FRIENDLY_AT),
    }
}

/// The attitude of two factions to each other (the table is symmetric).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub a: String,
    pub b: String,
    pub attitude: Attitude,
}

/// The project's factions and their attitudes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Factions {
    #[serde(default)]
    pub list: Vec<Faction>,
    #[serde(default)]
    pub relations: Vec<Relation>,
}

impl Factions {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Faction> {
        self.list.iter().find(|f| f.id == id)
    }

    /// How `a` and `b` treat each other ("" = no faction: neutral to everyone).
    pub fn attitude(&self, a: &str, b: &str) -> Attitude {
        if a.is_empty() || b.is_empty() {
            return Attitude::Neutral;
        }
        if a == b {
            return Attitude::Friendly;
        }
        self.relations.iter().find(|r| (r.a == a && r.b == b) || (r.a == b && r.b == a)).map_or(Attitude::Neutral, |r| r.attitude)
    }

    /// Sets the attitude of `a` and `b` (neutral removes the entry).
    pub fn set_attitude(&mut self, a: &str, b: &str, attitude: Attitude) {
        self.relations.retain(|r| !((r.a == a && r.b == b) || (r.a == b && r.b == a)));
        if attitude != Attitude::Neutral && a != b {
            self.relations.push(Relation { a: a.to_string(), b: b.to_string(), attitude });
        }
    }

    /// The player's standing with `faction` when the save doesn't hold one: their own faction
    /// [`OWN_STANDING`], else what the attitude to the player's faction starts from.
    pub fn base_standing(&self, player_faction: &str, faction: &str) -> i32 {
        if !player_faction.is_empty() && player_faction == faction {
            OWN_STANDING
        } else {
            self.attitude(player_faction, faction).standing()
        }
    }

    /// Ids renamed (`Database › Project` renames a faction).
    pub fn rename(&mut self, old: &str, new: &str) {
        for f in &mut self.list {
            if f.id == old {
                f.id = new.to_string();
            }
        }
        for r in &mut self.relations {
            for s in [&mut r.a, &mut r.b] {
                if s == old {
                    *s = new.to_string();
                }
            }
        }
    }
}

/// Factions by index with their attitudes, for the runtime's many lookups (built once a map is
/// entered).
#[derive(Clone, Debug, Default)]
pub struct Table {
    ids: Vec<String>,
    hostile: Vec<bool>,
}

impl Table {
    pub fn new(f: &Factions) -> Table {
        let ids: Vec<String> = f.list.iter().map(|f| f.id.clone()).collect();
        let n = ids.len();
        let mut hostile = vec![false; n * n];
        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                hostile[i * n + j] = f.attitude(a, b) == Attitude::Hostile;
            }
        }
        Table { ids, hostile }
    }

    /// Index of a faction (None: none or unknown).
    pub fn index(&self, id: &str) -> Option<u16> {
        (!id.is_empty()).then(|| self.ids.iter().position(|i| i == id).map(|i| i as u16)).flatten()
    }

    pub fn hostile(&self, a: Option<u16>, b: Option<u16>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => self.hostile[a as usize * self.ids.len() + b as usize],
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Factions {
        let mut f = Factions { list: ["village", "orcs", "elves"].iter().map(|id| Faction { id: id.to_string(), name: id.to_string() }).collect(), relations: vec![] };
        f.set_attitude("village", "orcs", Attitude::Hostile);
        f.set_attitude("elves", "orcs", Attitude::Hostile);
        f.set_attitude("village", "elves", Attitude::Friendly);
        f
    }

    #[test]
    fn attitudes_are_symmetric_with_neutral_default() {
        let f = sample();
        assert_eq!(f.attitude("orcs", "village"), Attitude::Hostile);
        assert_eq!(f.attitude("village", "village"), Attitude::Friendly);
        assert_eq!(f.attitude("village", "unknown"), Attitude::Neutral);
        assert_eq!(f.attitude("", "orcs"), Attitude::Neutral);
        let t = Table::new(&f);
        assert!(t.hostile(t.index("elves"), t.index("orcs")));
        assert!(!t.hostile(t.index("elves"), t.index("village")));
        assert!(!t.hostile(t.index(""), t.index("orcs")));
    }

    #[test]
    fn standing_starts_from_the_players_faction() {
        let f = sample();
        assert_eq!(f.base_standing("village", "orcs"), -50);
        assert!(f.base_standing("village", "orcs") <= HOSTILE_AT);
        assert_eq!(f.base_standing("orcs", "orcs"), OWN_STANDING);
        assert_eq!(f.base_standing("", "orcs"), 0);
        let mut f = f;
        f.set_attitude("village", "orcs", Attitude::Neutral);
        assert!(f.relations.iter().all(|r| !(r.a == "village" && r.b == "orcs")));
        f.rename("orcs", "orc_clans");
        assert_eq!(f.attitude("elves", "orc_clans"), Attitude::Hostile);
    }

    #[test]
    fn hostile_characters_spare_friends_of_their_faction() {
        // Without a faction the Hostile box alone decides (projects without factions).
        assert!(attacks_player(true, None) && !attacks_player(false, None));
        // An orc (Hostile) leaves an orc player (own faction) and a friend alone, not a neutral one.
        assert!(!attacks_player(true, Some(OWN_STANDING)) && !attacks_player(true, Some(Attitude::Friendly.standing())));
        assert!(attacks_player(true, Some(Attitude::Neutral.standing())));
        // Anyone attacks at a hostile standing; a raised one calms them down.
        assert!(attacks_player(false, Some(Attitude::Hostile.standing())));
        assert!(!attacks_player(false, Some(Attitude::Hostile.standing() + 40)));
    }
}
