//! Dice expressions (`1d20+5`, `2d6+STR`, `8d6-1`, `d20 + PROF`) and the seedable random source
//! every SRD roll uses. The generator is one per thread: the game, its scripts and the tests each
//! roll from their own, and `seed` makes a run repeatable (autoplay `seed N`, `RUSTPG2D_SEED`).

use std::cell::Cell;

use super::abilities::Ability;

thread_local! {
    static STATE: Cell<u64> = Cell::new(initial_seed());
}

/// The seed a thread starts with: `RUSTPG2D_SEED`, else a fixed one in automated play-throughs
/// (so their rolls repeat), else the clock.
fn initial_seed() -> u64 {
    if let Some(n) = std::env::var("RUSTPG2D_SEED").ok().and_then(|s| s.trim().parse::<u64>().ok()) {
        return mix(n);
    }
    if std::env::var_os("RUSTPG2D_AUTOPLAY").is_some() {
        return mix(1);
    }
    // Seconds alone repeat for games started in the same second: mix in an address (ASLR).
    let here = Box::new(0u8);
    mix(crate::sys::unix_now() ^ ((&*here as *const u8 as u64) << 16))
}

fn mix(n: u64) -> u64 {
    // splitmix64 of the seed, never 0 (xorshift's fixed point).
    let mut z = n.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) | 1
}

/// Makes the rolls of this thread repeat from here on.
pub fn seed(n: u64) {
    STATE.with(|s| s.set(mix(n)));
}

fn next_u64() -> u64 {
    STATE.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x
    })
}

/// One die: 1..=sides (1 for sides < 1).
pub fn die(sides: u32) -> u32 {
    if sides <= 1 {
        return 1;
    }
    (next_u64() % sides as u64) as u32 + 1
}

/// A number in 0..n (0 for n < 1), for scripts' `random`-like needs that should follow the seed.
pub fn below(n: u32) -> u32 {
    if n == 0 {
        0
    } else {
        (next_u64() % n as u64) as u32
    }
}

/// Advantage (roll two d20s, keep the higher), disadvantage (the lower) or a plain roll.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    #[default]
    Normal,
    Advantage,
    Disadvantage,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Normal, Mode::Advantage, Mode::Disadvantage];
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "normal",
            Mode::Advantage => "advantage",
            Mode::Disadvantage => "disadvantage",
        }
    }
    /// "advantage" / "adv" / "disadvantage" / "dis"; anything else is a plain roll.
    pub fn parse(s: &str) -> Mode {
        match s.trim().to_ascii_lowercase().as_str() {
            "advantage" | "adv" | "a" => Mode::Advantage,
            "disadvantage" | "dis" | "d" => Mode::Disadvantage,
            _ => Mode::Normal,
        }
    }
    /// Advantage and disadvantage together cancel out (SRD: "Advantage and Disadvantage").
    pub fn combine(self, other: Mode) -> Mode {
        match (self, other) {
            (Mode::Normal, m) | (m, Mode::Normal) => m,
            (a, b) if a == b => a,
            _ => Mode::Normal,
        }
    }
}

/// A d20 roll: both dice with advantage / disadvantage, the one kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct D20 {
    pub kept: u32,
    /// The other die (advantage / disadvantage), else `None`.
    pub other: Option<u32>,
    pub mode: Mode,
}

impl D20 {
    pub fn roll(mode: Mode) -> D20 {
        let a = die(20);
        match mode {
            Mode::Normal => D20 { kept: a, other: None, mode },
            Mode::Advantage | Mode::Disadvantage => {
                let b = die(20);
                let (keep, drop) = if (mode == Mode::Advantage) == (a >= b) { (a, b) } else { (b, a) };
                D20 { kept: keep, other: Some(drop), mode }
            }
        }
    }
    /// "14" or "14/6" (kept first).
    pub fn text(&self) -> String {
        match self.other {
            Some(o) => format!("{}/{}", self.kept, o),
            None => self.kept.to_string(),
        }
    }
}

/// One term of an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Term {
    /// `count` dice of `sides`.
    Dice { count: u32, sides: u32 },
    Number(i64),
    /// An ability's modifier (`STR`).
    Mod(Ability),
    /// The proficiency bonus (`PROF`).
    Prof,
}

/// A parsed dice expression: signed terms added up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub terms: Vec<(i64, Term)>,
}

/// The numbers an expression's ability tokens stand for.
pub trait Modifiers {
    fn ability_mod(&self, a: Ability) -> i64;
    fn proficiency(&self) -> i64;
}

/// No modifiers (every token is 0): for expressions without a character.
pub struct NoMods;
impl Modifiers for NoMods {
    fn ability_mod(&self, _: Ability) -> i64 {
        0
    }
    fn proficiency(&self) -> i64 {
        0
    }
}

/// The outcome of rolling an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rolled {
    /// Each die rolled, in order.
    pub dice: Vec<u32>,
    /// Everything that isn't a die: numbers, modifiers, proficiency.
    pub modifier: i64,
    pub total: i64,
}

impl Expr {
    /// Parses `2d6+3`, `d20 - 1`, `1d8+STR`, `8d6`, `PROF+2`. Dice counts up to 100 and sides up
    /// to 1000; case and spaces don't matter.
    pub fn parse(src: &str) -> Result<Expr, String> {
        let s: String = src.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
        if s.is_empty() {
            return Err("empty dice expression".into());
        }
        let mut terms = Vec::new();
        let mut rest = s.as_str();
        let mut sign = 1;
        if let Some(r) = rest.strip_prefix('-') {
            sign = -1;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('+') {
            rest = r;
        }
        loop {
            let end = rest.find(['+', '-']).unwrap_or(rest.len());
            let (tok, after) = rest.split_at(end);
            terms.push((sign, Self::term(tok, src)?));
            if after.is_empty() {
                break;
            }
            sign = if after.starts_with('-') { -1 } else { 1 };
            rest = &after[1..];
        }
        Ok(Expr { terms })
    }

    fn term(tok: &str, src: &str) -> Result<Term, String> {
        if tok.is_empty() {
            return Err(format!("'{src}': a + or - with nothing after it"));
        }
        if tok == "prof" {
            return Ok(Term::Prof);
        }
        if let Some(a) = Ability::parse(tok) {
            return Ok(Term::Mod(a));
        }
        if let Some((c, d)) = tok.split_once('d') {
            let count = if c.is_empty() { 1 } else { c.parse::<u32>().map_err(|_| format!("'{src}': '{c}' is not a number of dice"))? };
            let sides = d.parse::<u32>().map_err(|_| format!("'{src}': '{d}' is not a number of sides"))?;
            if count == 0 || count > 100 {
                return Err(format!("'{src}': roll 1 to 100 dice"));
            }
            if sides == 0 || sides > 1000 {
                return Err(format!("'{src}': dice have 1 to 1000 sides"));
            }
            return Ok(Term::Dice { count, sides });
        }
        tok.parse::<i64>().map(Term::Number).map_err(|_| format!("'{src}': '{tok}' is not dice (2d6), a number, an ability (STR) or PROF"))
    }

    pub fn roll(&self, mods: &dyn Modifiers) -> Rolled {
        let mut out = Rolled { dice: Vec::new(), modifier: 0, total: 0 };
        for (sign, t) in &self.terms {
            match t {
                Term::Dice { count, sides } => {
                    for _ in 0..*count {
                        let d = die(*sides);
                        out.dice.push(d);
                        out.total += sign * d as i64;
                    }
                }
                Term::Number(n) => out.modifier += sign * n,
                Term::Mod(a) => out.modifier += sign * mods.ability_mod(*a),
                Term::Prof => out.modifier += sign * mods.proficiency(),
            }
        }
        out.total += out.modifier;
        out
    }

    /// The lowest and highest total (modifiers counted with `mods`).
    pub fn range(&self, mods: &dyn Modifiers) -> (i64, i64) {
        let (mut lo, mut hi) = (0, 0);
        for (sign, t) in &self.terms {
            let (a, b) = match t {
                Term::Dice { count, sides } => (*count as i64, (*count * *sides) as i64),
                Term::Number(n) => (*n, *n),
                Term::Mod(a) => (mods.ability_mod(*a), mods.ability_mod(*a)),
                Term::Prof => (mods.proficiency(), mods.proficiency()),
            };
            if *sign > 0 {
                lo += a;
                hi += b;
            } else {
                lo -= b;
                hi -= a;
            }
        }
        (lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Hero;
    impl Modifiers for Hero {
        fn ability_mod(&self, a: Ability) -> i64 {
            if a == Ability::Str {
                3
            } else {
                -1
            }
        }
        fn proficiency(&self) -> i64 {
            2
        }
    }

    #[test]
    fn expressions_parse() {
        assert_eq!(Expr::parse("2d6+3").unwrap().terms, vec![(1, Term::Dice { count: 2, sides: 6 }), (1, Term::Number(3))]);
        assert_eq!(Expr::parse(" D20 - 1 ").unwrap().terms, vec![(1, Term::Dice { count: 1, sides: 20 }), (-1, Term::Number(1))]);
        assert_eq!(Expr::parse("1d8+STR+prof").unwrap().terms, vec![(1, Term::Dice { count: 1, sides: 8 }), (1, Term::Mod(Ability::Str)), (1, Term::Prof)]);
        assert_eq!(Expr::parse("-2").unwrap().terms, vec![(-1, Term::Number(2))]);
        for bad in ["", "2d", "d0", "101d6", "2d6+", "2x6", "1d6++1", "fish"] {
            assert!(Expr::parse(bad).is_err(), "{bad} should not parse");
        }
    }

    #[test]
    fn rolls_stay_in_range_and_repeat_with_a_seed() {
        let e = Expr::parse("2d6+STR-1").unwrap();
        assert_eq!(e.range(&Hero), (4, 14));
        seed(7);
        let a: Vec<i64> = (0..50).map(|_| e.roll(&Hero).total).collect();
        seed(7);
        let b: Vec<i64> = (0..50).map(|_| e.roll(&Hero).total).collect();
        assert_eq!(a, b);
        assert!(a.iter().all(|t| (4..=14).contains(t)));
        assert_eq!(Expr::parse("PROF+1").unwrap().roll(&Hero).total, 3);
        assert_eq!(Expr::parse("4").unwrap().roll(&NoMods).total, 4);
    }

    #[test]
    fn d20_statistics() {
        seed(42);
        let n = 20_000;
        let mut faces = [0u32; 21];
        let (mut plain, mut adv, mut dis) = (0u64, 0u64, 0u64);
        for _ in 0..n {
            let d = D20::roll(Mode::Normal).kept;
            faces[d as usize] += 1;
            plain += d as u64;
            adv += D20::roll(Mode::Advantage).kept as u64;
            dis += D20::roll(Mode::Disadvantage).kept as u64;
        }
        let mean = |s: u64| s as f64 / n as f64;
        assert!((mean(plain) - 10.5).abs() < 0.15, "d20 mean {}", mean(plain));
        assert!((mean(adv) - 13.825).abs() < 0.2, "advantage mean {}", mean(adv));
        assert!((mean(dis) - 7.175).abs() < 0.2, "disadvantage mean {}", mean(dis));
        assert_eq!(faces[0], 0);
        assert!(faces[1..].iter().all(|&c| c > n / 30), "every face comes up: {faces:?}");
        let r = D20::roll(Mode::Advantage);
        assert!(r.kept >= r.other.unwrap());
    }

    #[test]
    fn modes_combine() {
        assert_eq!(Mode::Advantage.combine(Mode::Disadvantage), Mode::Normal);
        assert_eq!(Mode::Normal.combine(Mode::Disadvantage), Mode::Disadvantage);
        assert_eq!(Mode::Advantage.combine(Mode::Advantage), Mode::Advantage);
        assert_eq!(Mode::parse("Adv"), Mode::Advantage);
        assert_eq!(Mode::parse(""), Mode::Normal);
    }
}
