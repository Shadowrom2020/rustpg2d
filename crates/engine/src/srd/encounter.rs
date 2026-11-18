//! Encounters sized for the party: the SRD's experience by challenge rating, and R(ust)PG2D's own
//! budget for an encounter's difficulty (the SRD 5.1 has no encounter tables): per party member
//! at the hero's level L, the XP of one monster of challenge L/8 (easy), L/4 (medium), L/2 (hard)
//! or L (deadly). Encounter zones (`ZoneKind::Encounter`) pick their group within it.

use serde::{Deserialize, Serialize};

/// How an encounter zone's group is made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    /// The listed monsters, as many as listed.
    #[default]
    Fixed,
    Easy,
    Medium,
    Hard,
    Deadly,
}

impl Difficulty {
    pub const ALL: [Difficulty; 5] = [Difficulty::Fixed, Difficulty::Easy, Difficulty::Medium, Difficulty::Hard, Difficulty::Deadly];

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Fixed => "Fixed group",
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
            Difficulty::Deadly => "Deadly",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Difficulty::Fixed => "fixed",
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
            Difficulty::Deadly => "deadly",
        }
    }

    /// The challenge rating whose XP each party member is worth, times the hero's level.
    fn share(self) -> f32 {
        match self {
            Difficulty::Fixed => 0.0,
            Difficulty::Easy => 0.125,
            Difficulty::Medium => 0.25,
            Difficulty::Hard => 0.5,
            Difficulty::Deadly => 1.0,
        }
    }
}

/// One line of an encounter: a character and how many (with a difficulty: at most how many).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterEntry {
    pub character: String,
    #[serde(default = "one")]
    pub count: u32,
}

fn one() -> u32 {
    1
}

/// The SRD's experience points by challenge rating.
const XP_BY_CR: [(f32, u32); 34] = [
    (0.0, 10),
    (0.125, 25),
    (0.25, 50),
    (0.5, 100),
    (1.0, 200),
    (2.0, 450),
    (3.0, 700),
    (4.0, 1100),
    (5.0, 1800),
    (6.0, 2300),
    (7.0, 2900),
    (8.0, 3900),
    (9.0, 5000),
    (10.0, 5900),
    (11.0, 7200),
    (12.0, 8400),
    (13.0, 10000),
    (14.0, 11500),
    (15.0, 13000),
    (16.0, 15000),
    (17.0, 18000),
    (18.0, 20000),
    (19.0, 22000),
    (20.0, 25000),
    (21.0, 33000),
    (22.0, 41000),
    (23.0, 50000),
    (24.0, 62000),
    (25.0, 75000),
    (26.0, 90000),
    (27.0, 105000),
    (28.0, 120000),
    (29.0, 135000),
    (30.0, 155000),
];

/// The XP of a challenge rating ("1/4", "2"; between two ratings the lower one's).
pub fn xp_for_cr(cr: f32) -> u32 {
    XP_BY_CR.iter().rev().find(|(c, _)| *c <= cr + 1e-6).map_or(10, |(_, xp)| *xp)
}

/// A challenge rating as the SRD writes it ("1/8", "2") as a number.
pub fn parse_cr(s: &str) -> Option<f32> {
    match s.trim().split_once('/') {
        Some((a, b)) => Some(a.trim().parse::<f32>().ok()? / b.trim().parse::<f32>().ok()?),
        None => s.trim().parse().ok(),
    }
}

/// The XP budget of an encounter of `difficulty` for a party of `members` at the hero's `level`.
pub fn budget(difficulty: Difficulty, level: u32, members: u32) -> u32 {
    if difficulty == Difficulty::Fixed {
        return 0;
    }
    let cr = (difficulty.share() * level.max(1) as f32).max(0.125);
    xp_for_cr(cr) * members.max(1)
}

/// The most monsters an encounter picks.
pub const MAX_GROUP: usize = 8;

/// Picks a group from `pool` (character, its XP, at most how many) within `budget`: the most
/// expensive that fits first, then any that still fit (by `seed`, the same each time), at least
/// one (the cheapest when nothing fits), at most [`MAX_GROUP`].
pub fn pick(pool: &[(String, u32, u32)], budget: u32, seed: u64) -> Vec<String> {
    let mut left: Vec<(String, u32, u32)> = pool.iter().filter(|p| p.2 > 0).cloned().collect();
    let mut rng = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (rng >> 33) as usize
    };
    let mut out = Vec::new();
    let mut spent = 0;
    while out.len() < MAX_GROUP {
        let fits: Vec<usize> = (0..left.len()).filter(|&k| left[k].2 > 0 && spent + left[k].1 <= budget).collect();
        let k = match (fits.is_empty(), out.is_empty()) {
            (true, true) => match (0..left.len()).min_by_key(|&k| left[k].1) {
                Some(k) => k,
                None => break,
            },
            (true, false) => break,
            (false, true) => *fits.iter().max_by_key(|&&k| left[k].1).unwrap(),
            (false, false) => fits[next() % fits.len()],
        };
        spent += left[k].1;
        left[k].2 -= 1;
        out.push(left[k].0.clone());
    }
    out
}

/// A seed from a map name and a zone (the same group every visit).
pub fn seed_of(map: &str, zone: u32) -> u64 {
    map.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)) ^ zone as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_grow_with_level_and_party() {
        // A lone level-1 hero: one monster of challenge 1/8, 1/4, 1/2, 1.
        let b = |d, l, m| budget(d, l, m);
        assert_eq!([b(Difficulty::Easy, 1, 1), b(Difficulty::Medium, 1, 1), b(Difficulty::Hard, 1, 1), b(Difficulty::Deadly, 1, 1)], [25, 50, 100, 200]);
        assert_eq!(b(Difficulty::Medium, 1, 2), 100, "a companion doubles it");
        assert_eq!(b(Difficulty::Hard, 4, 1), 450, "level 4: challenge 2");
        assert_eq!(b(Difficulty::Medium, 3, 1), 100, "challenge 3/4 rounds down to the table's 1/2");
        assert_eq!(b(Difficulty::Fixed, 5, 3), 0);
        assert_eq!((xp_for_cr(0.0), xp_for_cr(30.0), parse_cr("1/8"), parse_cr("2")), (10, 155000, Some(0.125), Some(2.0)));
    }

    #[test]
    fn a_group_stays_within_its_budget() {
        let pool = vec![("wolf".to_string(), 50, 3), ("giant-rat".to_string(), 25, 4), ("ogre".to_string(), 450, 1)];
        let g = pick(&pool, 100, 7);
        let xp: u32 = g.iter().map(|c| pool.iter().find(|p| p.0 == *c).unwrap().1).sum();
        assert!(xp <= 100 && !g.is_empty(), "{g:?}");
        assert_eq!(g[0], "wolf", "the most expensive that fits comes first");
        assert_eq!(pick(&pool, 100, 7), g, "the same seed, the same group");
        // Nothing fits: the cheapest one alone; counts cap the picks.
        assert_eq!(pick(&pool, 10, 1), vec!["giant-rat".to_string()]);
        assert_eq!(pick(&[("rat".to_string(), 25, 2)], 1000, 3).len(), 2);
        assert!(pick(&[("rat".to_string(), 1, 99)], 1000, 3).len() <= MAX_GROUP);
    }
}
