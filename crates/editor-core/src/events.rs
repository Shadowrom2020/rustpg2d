//! Events in words: one line for a trigger's event, a condition and an action, as the event
//! sheet shows its rows, and the actions' groups as the manual has them (the palette a new
//! action is picked from). They read the values as the files hold them (JSON): a variant's
//! name alone, or an object with the name as its only key.

use rustpg2d_engine::events::{Action, Cond, TriggerOn, TriggerOwner};
use rustpg2d_engine::refactor::RefKind;
use serde_json::Value;

/// A definition's name for people, if there is one (`None`: the id is shown).
pub type Names<'a> = &'a dyn Fn(RefKind, &str) -> Option<String>;

/// The actions by what they are for, in the manual's order (`Documentation/manual/07-events.md`).
pub const ACTION_GROUPS: &[(&str, &str)] = &[
    ("Talking", "Dialog Choice Mood Emote ClearEmote Bark Carry InputNumber InputText"),
    (
        "Game state",
        "SetFlag SetVar GiveItem TakeItem GiveMoney TakeMoney Heal HealCharacter GiveXp Quest UnlockAchievement ChangeStanding SetFaction If Parallel Wait Emit RunScript CallCommon StartTimer StopTimer Autosave Sleep Fall Nourish AddMana",
    ),
    (
        "Entities and the world",
        "SetActive SetState SetLocked SetFrame Teleport Move Face Place Equip Unequip GiveItemTo TakeItemFrom Patrol Schedule Companion CompanionOrder OpenCompanion ChangeShape RushTo OpenShop ShopStock OpenCrafting Lockpick SetTime SetWeather SetSpawners DefeatEnemies PlayerLight LightAt",
    ),
    (
        "Cut-scenes and effects",
        "Cutscene MoveRoute WalkTo Approach Camera ScrollText PlaySound PlayMusic PlayMusicEffect ShowImage HideImage ShowPicture MovePicture ErasePicture TintScreen FlashScreen FadeOut FadeIn Dim Shake Effect Fly Aim SetUiVisible",
    ),
    (
        "Rules (SRD 5.1)",
        "Check GroupCheck Contest Roll SetSkill ApplyEffect CureEffect ExposeDisease CureDisease Poison ShortRest LongRest StartCombat SetClass DamageProp DisarmTrap DamageAround DamageArea HealArea",
    ),
    ("Magic", "LearnSpell TeachSpell CastSpell"),
];

/// The group action `variant` is offered in ("" = in none: a new one the table doesn't know).
pub fn action_group(variant: &str) -> &'static str {
    ACTION_GROUPS.iter().find(|(_, names)| names.split(' ').any(|n| n == variant)).map_or("", |(g, _)| g)
}

/// What a new action of each kind starts as, in the engine's order: (what the game calls it,
/// its variant, the value). A kind with two names is there twice (a check, a saving throw).
pub fn action_templates() -> &'static [(String, String, Value)] {
    static ALL: std::sync::OnceLock<Vec<(String, String, Value)>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| Action::templates().iter().filter_map(|a| serde_json::to_value(a).ok().map(|v| (a.label().to_string(), variant(&v).0.to_string(), v))).collect())
}

/// What a new condition of each kind starts as: (its name, the condition as a list holds it).
pub fn condition_templates() -> &'static [(String, Value)] {
    static ALL: std::sync::OnceLock<Vec<(String, Value)>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| Cond::templates().into_iter().filter_map(|c| serde_json::to_value(rustpg2d_engine::events::Condition::new(c.clone())).ok().map(|v| (c.label().to_string(), v))).collect())
}

/// Whose triggers the list at `site` of a form holds (`None`: a placed thing's, of any kind).
pub fn owner_of(site: &str) -> Option<TriggerOwner> {
    match site.split('.').next().unwrap_or("") {
        "MapSettings" | "Map" => Some(TriggerOwner::Map),
        "Zone" => Some(TriggerOwner::Zone),
        "PropDef" => Some(TriggerOwner::Prop),
        "CharacterDef" => Some(TriggerOwner::Character),
        "ItemDef" => Some(TriggerOwner::ItemUse),
        _ => None,
    }
}

/// The events a new trigger of `owner` can wait for, the likeliest first (`None`: every event
/// a placed thing may get), each as a trigger to start with.
pub fn trigger_templates(owner: Option<TriggerOwner>) -> Vec<Value> {
    let events = match owner {
        Some(o) => o.templates(),
        None => TriggerOn::templates().into_iter().filter(|t| [TriggerOwner::Character, TriggerOwner::Prop, TriggerOwner::Pickup].iter().any(|o| o.allows(t))).collect(),
    };
    events.into_iter().filter_map(|on| serde_json::to_value(rustpg2d_engine::events::Trigger::new(on)).ok()).collect()
}

/// A value's variant and what it holds: `"Name"` or `{"Name": data}`.
pub fn variant(v: &Value) -> (&str, Option<&Value>) {
    match v {
        Value::String(s) => (s.as_str(), None),
        Value::Object(m) if m.len() == 1 => m.iter().next().map_or(("", None), |(k, d)| (k.as_str(), Some(d))),
        _ => ("", None),
    }
}

fn labels<T: serde::Serialize>(all: Vec<T>, label: impl Fn(&T) -> &'static str) -> std::collections::HashMap<String, &'static str> {
    // (The first of a kind names it: a check and a saving throw are one kind of action.)
    let mut out = std::collections::HashMap::new();
    for t in &all {
        if let Ok(v) = serde_json::to_value(t) {
            out.entry(variant(&v).0.to_string()).or_insert(label(t));
        }
    }
    out
}

/// What the game calls action `variant` ("Set flag", "Bark (speech bubble)").
pub fn action_label(variant: &str) -> String {
    static LABELS: std::sync::OnceLock<std::collections::HashMap<String, &'static str>> = std::sync::OnceLock::new();
    LABELS.get_or_init(|| labels(Action::templates(), |a| a.label())).get(variant).map_or_else(|| crate::schema::variant_label(variant), |l| l.to_string())
}

fn cond_label(variant: &str) -> String {
    static LABELS: std::sync::OnceLock<std::collections::HashMap<String, &'static str>> = std::sync::OnceLock::new();
    LABELS.get_or_init(|| labels(Cond::templates(), |c| c.label())).get(variant).map_or_else(|| crate::schema::variant_label(variant), |l| l.to_string())
}

/// The first line of `text`, at most `max` characters of it, in quotes.
fn quote(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() > max {
        format!("\"{}…\"", line.chars().take(max).collect::<String>().trim_end())
    } else {
        format!("\"{line}\"")
    }
}

/// An hour of the day as the clock shows it (7.5 = 07:30).
fn clock(hour: f64) -> String {
    let minutes = (hour * 60.0).round() as i64;
    format!("{:02}:{:02}", minutes.div_euclid(60).rem_euclid(24), minutes.rem_euclid(60))
}

/// Reads the fields of a variant's data.
struct Fields<'a> {
    data: Option<&'a Value>,
    names: Names<'a>,
}

impl Fields<'_> {
    fn text(&self, key: &str) -> &str {
        self.data.and_then(|d| d.get(key)).and_then(Value::as_str).unwrap_or("")
    }

    /// A number as the file means it: most are `f32`, whose widening shows its error
    /// (0.4 as 0.4000000059604645); the shortest number that is the same `f32` is said.
    fn num(&self, key: &str) -> f64 {
        let v = self.data.and_then(|d| d.get(key)).and_then(Value::as_f64).unwrap_or(0.0);
        (v as f32).to_string().parse().unwrap_or(v)
    }

    fn yes(&self, key: &str) -> bool {
        self.data.and_then(|d| d.get(key)).and_then(Value::as_bool).unwrap_or(false)
    }

    fn len(&self, key: &str) -> usize {
        self.data.and_then(|d| d.get(key)).and_then(Value::as_array).map_or(0, Vec::len)
    }

    /// A definition by its name (its id when it has none; "?" while nothing is picked).
    fn def(&self, kind: RefKind, key: &str) -> String {
        let id = self.text(key);
        if id.is_empty() {
            "?".to_string()
        } else {
            (self.names)(kind, id).filter(|n| !n.is_empty()).unwrap_or_else(|| id.to_string())
        }
    }

    /// Who an action is about: the trigger's owner, the hero, or a placed thing by its name.
    fn who(&self, key: &str) -> String {
        match self.text(key) {
            "" | "self" => "it".to_string(),
            "player" => "the hero".to_string(),
            name => name.to_string(),
        }
    }

    fn count_of(&self, kind: RefKind, key: &str) -> String {
        let n = self.num("count").max(1.0) as i64;
        if n == 1 {
            self.def(kind, key)
        } else {
            format!("{n} x {}", self.def(kind, key))
        }
    }

    /// The plain values, for kinds without words of their own: texts, numbers, and the names
    /// of what is switched on.
    fn plain(&self) -> String {
        let Some(Value::Object(m)) = self.data else {
            return match self.data {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            };
        };
        let parts: Vec<String> = m
            .iter()
            .filter_map(|(k, v)| match v {
                Value::String(s) if !s.is_empty() => Some(if s.chars().count() > 40 { quote(s, 40) } else { s.clone() }),
                Value::Number(n) => Some(format!("{} {n}", k.replace('_', " "))),
                Value::Bool(true) => Some(k.replace('_', " ")),
                Value::Array(a) if !a.is_empty() => Some(format!("{} {}", a.len(), k.replace('_', " "))),
                _ => None,
            })
            .take(5)
            .collect();
        parts.join(", ")
    }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// What a trigger waits for, after "When": "the player uses or talks to it".
pub fn trigger_summary(on: &Value, names: Names) -> String {
    let (name, data) = variant(on);
    let f = Fields { data, names };
    match name {
        "Interact" => "the player uses it or talks to it".into(),
        "Enter" => "the player steps in".into(),
        "Leave" => "the player steps out".into(),
        "UseItem" if f.text("item").is_empty() => "any item is used on it".into(),
        "UseItem" => format!("{} is used on it", f.def(RefKind::Item, "item")),
        "MapEnter" => "the map is entered".into(),
        "Custom" => format!("the event {} is raised", if f.text("name").is_empty() { "?" } else { f.text("name") }),
        "Defeat" => "it is defeated".into(),
        "Unlock" => "it is unlocked".into(),
        "Lock" => "it is locked again".into(),
        "HpBelow" => format!("its hit points fall below {} %", f.num("percent")),
        "Sleep" => "the player wakes up".into(),
        "Hour" => format!("the clock reaches {}", clock(f.num("hour"))),
        "CombatStart" => "a fight starts".into(),
        "CombatEnd" => "the fight is over".into(),
        "Search" => "the hero searches here".into(),
        "TrapSprung" => "its trap springs".into(),
        "TrapDisarmed" => "its trap is disarmed".into(),
        other => {
            static LABELS: std::sync::OnceLock<std::collections::HashMap<String, &'static str>> = std::sync::OnceLock::new();
            LABELS.get_or_init(|| labels(TriggerOn::templates(), |t| t.label())).get(other).map_or_else(|| crate::schema::variant_label(other), |l| l.to_string())
        }
    }
}

fn compare(op: &str) -> &'static str {
    match op {
        "Eq" => "is",
        "Ne" => "is not",
        "Lt" => "is less than",
        "Le" => "is at most",
        "Gt" => "is more than",
        "Ge" => "is at least",
        _ => "is",
    }
}

/// A condition in words, after "If": "the flag shrine_lit is set".
pub fn condition_summary(c: &Value, names: Names) -> String {
    let negate = c.get("negate").and_then(Value::as_bool).unwrap_or(false);
    let (name, data) = variant(c.get("cond").unwrap_or(c));
    let f = Fields { data, names };
    // (Said the other way round where that reads better than "not: …".)
    let said = |yes: String, no: String| if negate { no } else { yes };
    let s = match name {
        "Flag" => return said(format!("the flag {} is set", f.text("name")), format!("the flag {} is not set", f.text("name"))),
        "Var" => format!("{} {} {}", f.text("name"), compare(f.text("op")), f.num("value")),
        "HasItem" => return said(format!("the hero has {}", f.count_of(RefKind::Item, "item")), format!("the hero doesn't have {}", f.count_of(RefKind::Item, "item"))),
        "Script" => format!("{}() says yes", f.text("function")),
        "PlayerHp" => format!("the hero's hit points {} {}", compare(f.text("op")), f.num("value")),
        "PropState" => return said(format!("{} is in the state {}", f.who("target"), f.text("state")), format!("{} is not in the state {}", f.who("target"), f.text("state"))),
        "Quest" => {
            let quest = f.def(RefKind::Quest, "quest");
            match f.text("check") {
                "NotStarted" => format!("the quest {quest} has not started"),
                "Active" => format!("the quest {quest} is under way"),
                "Completed" => format!("the quest {quest} is done"),
                "Failed" => format!("the quest {quest} has failed"),
                "AtStage" => format!("the quest {quest} is at stage {}", f.num("stage")),
                "StageAtLeast" => format!("the quest {quest} has reached stage {}", f.num("stage")),
                other => format!("the quest {quest}: {other}"),
            }
        }
        "TimeBetween" => format!("it is between {} and {}", clock(f.num("from")), clock(f.num("to"))),
        "Level" => format!("the hero's level {} {}", compare(f.text("op")), f.num("value")),
        "KnowsSpell" => format!("the hero knows {}", f.def(RefKind::Spell, "spell")),
        "Defeated" => format!("{} defeated {} {}", f.def(RefKind::Character, "character"), compare(f.text("op")), f.num("count")),
        "Timer" => format!("the timer {} {} {} s", f.text("name"), compare(f.text("op")), f.num("seconds")),
        "TextVar" => format!("the text {} is {}", f.text("name"), quote(f.text("value"), 30)),
        "Money" => format!("the hero's coins {} {}", compare(f.text("op")), rustpg2d_engine::srd::equipment::format_cp(f.num("cp") as u32, ["gp", "sp", "cp"])),
        "KnockedOut" => format!("{} was knocked out", f.text("name")),
        "HasDisease" if f.text("disease").is_empty() => "the hero is ill".into(),
        other => {
            let plain = f.plain();
            if plain.is_empty() {
                cond_label(other)
            } else {
                format!("{}: {plain}", cond_label(other))
            }
        }
    };
    if negate {
        format!("not: {s}")
    } else {
        s
    }
}

/// An action in one line: "elder: "Welcome, traveller."", "Give 2 x Healing Potion".
pub fn action_summary(a: &Value, names: Names) -> String {
    // (A line that starts with whom it is about starts with a capital like the others.)
    let line = action_line(a, names);
    match line.strip_prefix("it ").map(|rest| format!("It {rest}")) {
        Some(l) => l,
        None => line.strip_prefix("the hero").map_or(line.clone(), |rest| format!("The hero{rest}")),
    }
}

fn action_line(a: &Value, names: Names) -> String {
    let (name, data) = variant(a);
    let f = Fields { data, names };
    let speaker = |text: String| if f.text("speaker").is_empty() { text } else { format!("{}: {text}", f.text("speaker")) };
    match name {
        "Dialog" => speaker(quote(f.text("text"), 80)),
        "Choice" => {
            let options: Vec<&str> = data.and_then(|d| d.get("options")).and_then(Value::as_array).map(|o| o.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            speaker(format!("{} [{}]", quote(f.text("text"), 50), options.join(" / ")))
        }
        "GiveItem" => format!("Give {}", f.count_of(RefKind::Item, "item")),
        "TakeItem" => format!("Take {}", f.count_of(RefKind::Item, "item")),
        "GiveItemTo" => format!("Give {} to {}", f.count_of(RefKind::Item, "item"), f.who("target")),
        "TakeItemFrom" => format!("Take {} from {}", f.count_of(RefKind::Item, "item"), f.who("target")),
        "SetFlag" => format!("{} the flag {}", if f.yes("value") { "Set" } else { "Clear" }, f.text("name")),
        "SetVar" if f.text("op") == "Set" => format!("Set {} to {}", f.text("name"), f.num("value")),
        "SetVar" => format!("Add {} to {}", f.num("value"), f.text("name")),
        "GiveMoney" => format!("Give {}", rustpg2d_engine::srd::equipment::format_cp(f.num("cp") as u32, ["gp", "sp", "cp"])),
        "TakeMoney" => format!("Take {}", rustpg2d_engine::srd::equipment::format_cp(f.num("cp") as u32, ["gp", "sp", "cp"])),
        "Heal" => format!("Heal the hero by {}", f.num("amount")),
        "HealCharacter" => format!("Heal {} by {}", f.who("target"), f.num("amount")),
        "GiveXp" => format!("Give {} XP", f.num("amount")),
        "Teleport" if f.text("spawn").is_empty() => format!("Go to {}", f.def(RefKind::Map, "map")),
        "Teleport" => format!("Go to {} ({})", f.def(RefKind::Map, "map"), f.text("spawn")),
        "SetActive" => format!("{} {}", f.who("target"), if f.yes("active") { "appears" } else { "disappears" }),
        "SetState" => format!("{} changes to the state {}", f.who("target"), if f.text("state").is_empty() { "?" } else { f.text("state") }),
        "SetLocked" => format!("{} is {}", f.who("target"), if f.yes("locked") { "locked" } else { "unlocked" }),
        "Move" => format!("{} moves by {}, {}", f.who("target"), f.num("dx"), f.num("dy")),
        "Face" => format!("{} faces {}", f.who("target"), f.text("facing").to_lowercase()),
        "WalkTo" => format!("{} walks to {}, {}", f.who("target"), f.num("x"), f.num("y")),
        "Place" => format!("{} is put on {}, {}", f.who("target"), f.num("x"), f.num("y")),
        "Approach" => format!("{} walks up to {}", f.who("target"), f.who("to")),
        "RushTo" => format!("{} rushes to {}", f.who("target"), f.who("to")),
        "Wait" => format!("Wait {} s", f.num("seconds")),
        "Emit" => format!("Raise the event {}", f.text("name")),
        "RunScript" => format!("Run {}()", f.text("function")),
        "Cutscene" => format!("Play the cut-scene {}", f.text("function")),
        "EndCutscene" => "End the cut-scene".into(),
        "CallCommon" => format!("Call {}", f.def(RefKind::CommonEvent, "id")),
        "If" => {
            let conds: Vec<String> = data.and_then(|d| d.get("conditions")).and_then(Value::as_array).map(|c| c.iter().map(|c| condition_summary(c, names)).collect()).unwrap_or_default();
            let then = plural(f.len("then"), "action");
            match (conds.is_empty(), f.len("otherwise")) {
                (true, _) => format!("If (no condition yet): {then}"),
                (false, 0) => format!("If {}: {then}", conds.join(" and ")),
                (false, n) => format!("If {}: {then}, else {}", conds.join(" and "), n),
            }
        }
        "Parallel" => format!("At the same time: {}", plural(f.len("actions"), "action")),
        "Quest" => {
            let quest = f.def(RefKind::Quest, "quest");
            match f.text("op") {
                "Start" => format!("Start the quest {quest}"),
                "Advance" => format!("The quest {quest} goes to its next stage"),
                "SetStage" => format!("The quest {quest} goes to stage {}", f.num("stage")),
                "Complete" => format!("The quest {quest} is done"),
                "Fail" => format!("The quest {quest} fails"),
                other => format!("Quest {quest}: {other}"),
            }
        }
        "UnlockAchievement" => format!("Unlock the achievement {}", f.def(RefKind::Achievement, "id")),
        "ChangeStanding" => format!("Standing with {} {}{}", if f.text("faction").is_empty() { "?" } else { f.text("faction") }, if f.num("amount") >= 0.0 { "+" } else { "" }, f.num("amount")),
        "PlaySound" => format!("Play the sound {}", f.def(RefKind::Sound, "sound")),
        "PlayMusic" if f.text("sound").is_empty() => "Stop the music".into(),
        "PlayMusic" => format!("Play the music {}", f.def(RefKind::Sound, "sound")),
        "Emote" => format!("{} shows {}", f.who("target"), f.text("emotion")),
        "ClearEmote" => format!("{} shows no emotion any more", f.who("target")),
        "Mood" => format!("{}'s mood: {}", f.who("target"), if f.text("mood").is_empty() { "as usual" } else { f.text("mood") }),
        "Bark" => {
            let line = data.and_then(|d| d.get("lines")).and_then(Value::as_array).and_then(|l| l.first()).and_then(Value::as_str).unwrap_or("");
            format!("{} says in passing {}", f.who("target"), quote(line, 60))
        }
        "Companion" => format!("{} {} the party", f.who("target"), if f.yes("join") { "joins" } else { "leaves" }),
        "OpenShop" => format!("Open the shop of {}", f.who("target")),
        "Equip" => format!("{} puts on {}", f.who("target"), f.def(RefKind::Item, "item")),
        "LearnSpell" => format!("The hero learns {}", f.def(RefKind::Spell, "spell")),
        "TeachSpell" => format!("{} learns {}", f.who("target"), f.def(RefKind::Spell, "spell")),
        "CastSpell" => format!("The hero casts {}", f.def(RefKind::Spell, "spell")),
        "SetTime" => format!("Set the time to {}", clock(f.num("hour"))),
        "Sleep" => format!("Sleep {} hours", f.num("hours")),
        "Roll" => format!("Roll {} into {}", f.text("dice"), if f.text("var").is_empty() { "?" } else { f.text("var") }),
        "Check" => format!("{} {}, DC {}: {} if it succeeds, {} if not", f.text("what"), if f.yes("save") { "saving throw" } else { "check" }, f.num("dc"), f.len("then"), f.len("otherwise")),
        "GroupCheck" => format!("Group check {}, DC {}: {} if it succeeds, {} if not", f.text("what"), f.num("dc"), f.len("then"), f.len("otherwise")),
        "StartCombat" | "ShortRest" | "DefeatEnemies" | "Autosave" | "HideImage" => action_label(name),
        other => {
            let plain = f.plain();
            if plain.is_empty() {
                action_label(other)
            } else {
                format!("{}: {plain}", action_label(other))
            }
        }
    }
}

// ------------------------------------------------------------ conversations

/// The variable the *Choice* action at `choice` keeps its answer in ("" when it isn't one or
/// names none).
fn choice_var(actions: &[Value], choice: usize) -> &str {
    actions.get(choice).and_then(|a| a.get("Choice")).and_then(|c| c.get("var")).and_then(Value::as_str).unwrap_or("")
}

/// Whether `action` is an *If* whose only condition is that `var` is `answer`: what a choice's
/// answer leads to.
fn is_branch(action: &Value, var: &str, answer: Option<usize>) -> bool {
    let Some([only]) = action.get("If").and_then(|i| i.get("conditions")).and_then(Value::as_array).map(Vec::as_slice) else { return false };
    let Some(cond) = only.get("cond").and_then(|c| c.get("Var")) else { return false };
    !only.get("negate").and_then(Value::as_bool).unwrap_or(false)
        && cond.get("name").and_then(Value::as_str) == Some(var)
        && cond.get("op").and_then(Value::as_str) == Some("Eq")
        && answer.is_none_or(|a| cond.get("value").and_then(Value::as_u64) == Some(a as u64))
}

/// The actions after the choice at `choice` that belong to it: up to the next choice that
/// keeps its answer in the same variable.
fn after_choice(actions: &[Value], choice: usize) -> std::ops::Range<usize> {
    let var = choice_var(actions, choice);
    let end = (choice + 1..actions.len()).find(|j| choice_var(actions, *j) == var).unwrap_or(actions.len());
    choice + 1..end
}

/// Where in `actions` it says what happens after answer `answer` (from 0) of the choice at
/// `choice`: the *If* behind it that asks for just that answer.
pub fn branch_of(actions: &[Value], choice: usize, answer: usize) -> Option<usize> {
    let var = choice_var(actions, choice);
    if var.is_empty() {
        return None;
    }
    after_choice(actions, choice).find(|j| is_branch(&actions[*j], var, Some(answer)))
}

/// Every action of `actions` that is the branch of an answer of a choice of the list (shown
/// under its answer, not as a row of its own).
pub fn branches(actions: &[Value]) -> Vec<usize> {
    let mut out = Vec::new();
    for choice in (0..actions.len()).filter(|k| !choice_var(actions, *k).is_empty()) {
        let answers = actions[choice]["Choice"].get("options").and_then(Value::as_array).map_or(0, Vec::len);
        out.extend((0..answers).filter_map(|a| branch_of(actions, choice, a)));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// What happens after answer `answer` of a choice that keeps its answer in `var`, to begin
/// with: nothing yet.
pub fn new_branch(var: &str, answer: usize) -> Value {
    serde_json::json!({"If": {"conditions": [{"cond": {"Var": {"name": var, "op": "Eq", "value": answer}}, "negate": false}], "then": [], "otherwise": []}})
}

/// Where a new branch for answer `answer` of the choice at `choice` goes: behind the branches
/// of the answers before it, else right behind the choice.
pub fn branch_place(actions: &[Value], choice: usize, answer: usize) -> usize {
    (0..answer).rev().find_map(|a| branch_of(actions, choice, a)).map_or(choice + 1, |j| j + 1)
}

/// Takes answer `answer` of the choice at `choice` away, with what it leads to; the answers
/// behind it move up, and what they lead to with them.
pub fn remove_answer(actions: &mut Vec<Value>, choice: usize, answer: usize) {
    let var = choice_var(actions, choice).to_string();
    let Some(options) = actions.get_mut(choice).and_then(|a| a.get_mut("Choice")).and_then(|c| c.get_mut("options")).and_then(Value::as_array_mut) else { return };
    if answer >= options.len() {
        return;
    }
    options.remove(answer);
    if var.is_empty() {
        return;
    }
    let range = after_choice(actions, choice);
    let gone = range.clone().find(|j| is_branch(&actions[*j], &var, Some(answer)));
    for j in range {
        if !is_branch(&actions[j], &var, None) {
            continue;
        }
        if let Some(v) = actions[j].pointer_mut("/If/conditions/0/cond/Var/value") {
            if let Some(n) = v.as_u64().filter(|n| *n as usize > answer) {
                *v = Value::from(n - 1);
            }
        }
    }
    if let Some(j) = gone {
        actions.remove(j);
    }
}

/// Who speaks next in a conversation, for a new line: the one before the last speaker (two
/// people take turns), else the last one.
pub fn next_speaker(actions: &[Value]) -> String {
    let speakers: Vec<&str> = actions.iter().filter_map(|a| a.get("Dialog").or_else(|| a.get("Choice"))).filter_map(|d| d.get("speaker").and_then(Value::as_str)).collect();
    match speakers.as_slice() {
        [.., last] => speakers.iter().rev().find(|s| *s != last).unwrap_or(last).to_string(),
        [] => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::events::Condition;
    use serde_json::json;

    fn none(_: RefKind, _: &str) -> Option<String> {
        None
    }

    #[test]
    fn every_action_has_a_group_and_a_line() {
        let mut first = std::collections::HashSet::new();
        for a in Action::templates() {
            let v = serde_json::to_value(&a).unwrap();
            let name = variant(&v).0;
            assert!(!name.is_empty(), "{v}");
            assert!(!action_group(name).is_empty(), "the action {name} is in no group of ACTION_GROUPS");
            assert!(!action_summary(&v, &none).is_empty(), "{name}");
            if first.insert(name.to_string()) {
                assert_eq!(action_label(name), a.label());
            }
        }
        // Nothing twice, nothing the engine doesn't have.
        let known: Vec<String> = Action::templates().iter().map(|a| variant(&serde_json::to_value(a).unwrap()).0.to_string()).collect();
        let mut seen = std::collections::HashSet::new();
        for name in ACTION_GROUPS.iter().flat_map(|(_, names)| names.split(' ')) {
            assert!(seen.insert(name), "{name} is in two groups");
            assert!(known.iter().any(|k| k == name), "{name} is no action");
        }
    }

    #[test]
    fn every_event_and_condition_has_a_line() {
        for t in TriggerOn::templates() {
            assert!(!trigger_summary(&serde_json::to_value(&t).unwrap(), &none).is_empty(), "{t:?}");
        }
        for c in Cond::templates() {
            let v = serde_json::to_value(Condition::new(c.clone())).unwrap();
            assert!(!condition_summary(&v, &none).is_empty(), "{c:?}");
        }
    }

    #[test]
    fn a_choices_answers_lead_to_their_branches() {
        let say = |who: &str, text: &str| json!({"Dialog": {"speaker": who, "text": text}});
        let mut actions = vec![
            say("Elder", "Will you help?"),
            json!({"Choice": {"speaker": "Elder", "text": "Well?", "options": ["Yes", "Later", "No"], "var": "help"}}),
            new_branch("help", 0),
            json!({"SetFlag": {"name": "asked", "value": true}}),
            new_branch("help", 2),
            // (Another question in the same variable: its branches are its own.)
            json!({"Choice": {"speaker": "Elder", "text": "Sure?", "options": ["Yes"], "var": "help"}}),
            new_branch("help", 0),
        ];
        assert_eq!((branch_of(&actions, 1, 0), branch_of(&actions, 1, 1), branch_of(&actions, 1, 2)), (Some(2), None, Some(4)));
        assert_eq!(branch_of(&actions, 5, 0), Some(6));
        assert_eq!(branches(&actions), [2, 4, 6]);
        // A branch for the middle answer goes behind the first one's.
        assert_eq!(branch_place(&actions, 1, 1), 3);
        assert_eq!(branch_place(&actions, 1, 0), 2);
        // The first answer taken away: its branch goes, "No" is answer 1 now and keeps its own.
        remove_answer(&mut actions, 1, 0);
        assert_eq!(actions[1]["Choice"]["options"], json!(["Later", "No"]));
        assert_eq!(actions.len(), 6);
        assert_eq!((branch_of(&actions, 1, 0), branch_of(&actions, 1, 1)), (None, Some(3)));
        assert_eq!(branch_of(&actions, 4, 0), Some(5), "the second question's branch is untouched");
        // An If that asks for more than the answer is no branch.
        let mut two = new_branch("help", 0);
        two["If"]["conditions"].as_array_mut().unwrap().push(json!({"cond": {"Flag": {"name": "x"}}, "negate": false}));
        assert!(!is_branch(&two, "help", Some(0)));
        // Two people take turns.
        assert_eq!(next_speaker(&[say("Elder", "a"), say("{player}", "b")]), "Elder");
        assert_eq!(next_speaker(&[say("Elder", "a"), say("Elder", "b")]), "Elder");
        assert_eq!(next_speaker(&[]), "");
    }

    #[test]
    fn lines_say_what_happens() {
        let names = |kind: RefKind, id: &str| (kind == RefKind::Item && id == "potion").then(|| "Healing Potion".to_string());
        let line = |v: Value| action_summary(&v, &names);
        assert_eq!(line(json!({"Dialog": {"speaker": "Elder", "text": "Welcome, traveller.\nStay a while."}})), "Elder: \"Welcome, traveller.\"");
        assert_eq!(line(json!({"Choice": {"speaker": "", "text": "Help her?", "options": ["Yes", "No"], "var": "help"}})), "\"Help her?\" [Yes / No]");
        assert_eq!(line(json!({"GiveItem": {"item": "potion", "count": 2}})), "Give 2 x Healing Potion");
        assert_eq!(line(json!({"TakeItem": {"item": "gate_key", "count": 1}})), "Take gate_key");
        assert_eq!(line(json!({"SetFlag": {"name": "met_elder", "value": true}})), "Set the flag met_elder");
        assert_eq!(line(json!({"SetVar": {"name": "visits", "op": "Add", "value": 1}})), "Add 1 to visits");
        assert_eq!(line(json!({"SetActive": {"target": "self", "active": false}})), "It disappears");
        assert_eq!(line(json!({"GiveMoney": {"cp": 250}})), "Give 2 gp 5 sp");
        assert_eq!(line(json!("EndCutscene")), "End the cut-scene");
        assert_eq!(line(json!({"SetTime": {"hour": 7.5}})), "Set the time to 07:30");
        let cond = json!({"cond": {"Flag": {"name": "met_elder"}}, "negate": true});
        assert_eq!(condition_summary(&cond, &names), "the flag met_elder is not set");
        assert_eq!(line(json!({"If": {"conditions": [cond], "then": [{"Wait": {"seconds": 1.0}}], "otherwise": []}})), "If the flag met_elder is not set: 1 action");
        // (An f32 widened to f64 is said as written.)
        assert_eq!(line(serde_json::to_value(rustpg2d_engine::events::Action::Wait { seconds: 0.4 }).unwrap()), "Wait 0.4 s");
        assert_eq!(condition_summary(&json!({"cond": {"Var": {"name": "visits", "op": "Ge", "value": 3}}, "negate": false}), &names), "visits is at least 3");
        assert_eq!(trigger_summary(&json!({"UseItem": {"item": "potion"}}), &names), "Healing Potion is used on it");
        assert_eq!(trigger_summary(&json!({"Hour": {"hour": 18.0}}), &names), "the clock reaches 18:00");
        // A kind without words of its own: its name and its plain values.
        assert_eq!(line(json!({"Shake": {"seconds": 0.5, "strength": 3.0}})), "Shake the screen: seconds 0.5, strength 3.0");
    }
}
