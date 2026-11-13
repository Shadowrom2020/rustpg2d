//! The script API as the editor and the tools see it: every function the engine registers
//! (`script.rs`), its parameters with what each expects, and a one-line description. One table
//! feeds the script editor's completion and signature hints, the agent server's API resource,
//! the renames and the script check (which arguments are ids: [`api_ids`]), the translation
//! templates (which arguments are texts) and the editor's flag / variable collectors. A test
//! keeps it complete against `script.rs` and `Documentation/manual/08-scripting.md`.

use std::sync::LazyLock;

use crate::script_refs::IdKind;

/// What a function argument expects (drives completion inside string arguments).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arg {
    Item,
    Map,
    /// Spawn point on the map named by the first argument.
    Spawn,
    Sound,
    Quest,
    State,
    Skill,
    Spell,
    /// A sprite sheet id.
    Sheet,
    /// An options map: #{ key: value, ... }.
    Opts,
    /// "self", "player" or an entity name.
    Target,
    Flag,
    Var,
    /// A script function name.
    Func,
    /// "up" / "down" / "left" / "right".
    Dir,
    /// A status effect: "poison", "haste", ...
    Effect,
    /// A character definition id.
    Character,
    /// "rain", "snow", "fog", "none", "default".
    Weather,
    /// Event name for emit().
    Event,
    /// A crafting recipe id.
    Recipe,
    Achievement,
    /// A crafting station (named by recipes).
    Station,
    /// A prop definition id.
    Prop,
    /// A faction id (Database \u{203A} Project \u{203A} Factions).
    Faction,
    /// Any text: a zone, order, mood or slot name.
    Text,
    /// A line players read (dialog, question, title): collected for translation and shown
    /// translated.
    Line,
    Num,
    Bool,
    List,
}

pub struct ApiFn {
    pub name: &'static str,
    pub params: &'static [(&'static str, Arg)],
    pub doc: &'static str,
}

macro_rules! api {
    ($($name:literal ( $($p:ident : $k:ident),* ) $doc:literal;)*) => {
        &[$(ApiFn { name: $name, params: &[$((stringify!($p), Arg::$k)),*], doc: $doc }),*]
    };
}

/// The engine's script API (see [`crate::script`]). Overloads are listed separately.
pub const API: &[ApiFn] = api! {
    "flag"(name: Flag) "Whether a flag is set";
    "set_flag"(name: Flag, value: Bool) "Sets or clears a flag (immediately)";
    "get_var"(name: Var) "Value of a variable (0 if unset)";
    "set_var"(name: Var, value: Num) "Sets a variable (immediately)";
    "queue_set_var"(name: Var, value: Num) "Sets a variable after the actions queued before it (e.g. after a dialog) (queued)";
    "add_var"(name: Var, amount: Num) "Adds to a variable (immediately)";
    "item_count"(item: Item) "How many of an item the player has";
    "has_item"(item: Item) "Whether the player has the item";
    "give_item"(item: Item, count: Num) "Gives items to the player; returns how many didn't fit";
    "take_item"(item: Item, count: Num) "Takes items from the player; returns how many were taken";
    "current_map"() "Name of the current map";
    "player_hp"() "The player's hit points";
    "player_name"() "The player's name";
    "player_sex"() "\"male\" or \"female\"";
    "player_race"() "The player's race id (Database \u{203A} Races), \"\" without one";
    "carry"(target: Target, prop: Prop) "A character (or \"player\") carries a prop in both hands, e.g. carry(\"player\", \"crate\"); while carrying it holds nothing else and the player can't attack (queued)";
    "stop_carrying"(target: Target) "Puts down what a character carries (queued)";
    "player_carrying"() "The prop the player carries (\"\" = none)";
    "player_faction"() "The player's faction id (their race's, else the player character's; \"\" without one)";
    "standing"(faction: Faction) "The player's standing with a faction (-100..100; at -20 or below its characters attack the player)";
    "change_standing"(faction: Faction, amount: Num) "Raises (or with a negative amount lowers) the player's standing with a faction; its characters turn hostile or calm down at once (queued)";
    "is_female"() "Whether the player character is female";
    "is_male"() "Whether the player character is male";
    "hour"() "Time of day in hours (0..24, e.g. 13.5 = half past one in the afternoon)";
    "day"() "The in-game day (0 on the first day; sleeping and midnight move it on), e.g. for things that happen once a day";
    "is_night"() "Whether it is night (dark outdoors)";
    "set_hour"(hour: Num) "Sets the time of day, e.g. set_hour(22) (queued)";
    "skill"(skill: Skill) "The player's level in a skill (0 = none)";
    "skill_points"() "Unspent skill points";
    "set_skill"(skill: Skill, level: Num) "Sets a skill's level without spending points (queued)";
    "give_skill_points"(amount: Num) "Gives skill points (negative takes them) (queued)";
    "knows_spell"(spell: Spell) "Whether the player has learned the spell";
    "learn_spell"(spell: Spell) "Teaches a spell (queued)";
    "mana"() "The player's current mana";
    "add_mana"(amount: Num) "Adds mana (negative drains) (queued)";
    "xp"() "All the experience points the player has gained (experience and levels)";
    "level"() "The player's level (1 = the first)";
    "apply_effect"(target: Target, effect: Effect, strength: Num, seconds: Num) "Starts a status effect on \"player\" or a character: \"poison\", \"burn\" (HP per second), \"regen\" (HP per second), \"slow\", \"haste\", \"strength\", \"weakness\" (percent), \"shield\" (defense), \"stun\" (queued)";
    "cure"(target: Target, effect: Effect) "Ends a status effect, e.g. cure(\"player\", \"poison\") (queued)";
    "cure"(target: Target) "Ends all status effects (queued)";
    "defeated_count"(character: Character) "How many characters of this definition the player has defeated (the bestiary's count), e.g. defeated_count(\"slime\")";
    "shake"(seconds: Num) "Shakes the screen like a hit for a while: earthquakes, explosions (queued)";
    "shake"(seconds: Num, strength: Num) "Shakes the screen; strength 1 = like a hit, 2 = twice as hard (queued)";
    "set_weather"(kind: Weather) "Weather of the current map: \"rain\", \"snow\", \"fog\", \"none\" or \"default\" (the map's own); kept in the save game (queued)";
    "set_weather"(kind: Weather, intensity: Num) "Weather of the current map with a strength from 0 to 1, e.g. set_weather(\"rain\", 0.8) (queued)";
    "set_map_weather"(map: Map, kind: Weather, intensity: Num) "Weather of another map, e.g. set_map_weather(\"forest\", \"fog\", 0.5) (queued)";
    "weather"() "The current map's weather: \"none\", \"rain\", \"snow\" or \"fog\"";
    "weather_intensity"() "The current weather's strength (0 to 1)";
    "hp_of"(target: Target) "A character's HP right now: \"player\", \"self\" (also SELF), or a name on this map; -1 if there is no such character";
    "has_effect"(effect: Effect) "Whether the player has a status effect, e.g. has_effect(\"poison\")";
    "give_xp"(amount: Num) "Gives experience; enough of it raises the level (negative takes it away, the level never drops) (queued)";
    "damage_around"(damage: Num, radius: Num) "Damages every character within radius tiles of the player (queued)";
    "player_light"(radius: Num, seconds: Num) "Lights the player's surroundings for a while (queued)";
    "player_light"(radius: Num, seconds: Num, options: Opts) "Like player_light(); options #{ color: [r, g, b], flicker: true } (queued)";
    "light_at"(x: Num, y: Num, radius: Num, seconds: Num) "A light at tile x, y for a while, fading out at the end (queued)";
    "light_at"(x: Num, y: Num, radius: Num, seconds: Num, options: Opts) "Like light_at(); options #{ color: [r, g, b], flicker: true } (queued)";
    "heal_area"(x: Num, y: Num, radius: Num, amount: Num) "Heals the player and friendly characters within radius tiles of tile x, y (queued)";
    "equip"(target: Target, item: Item) "Puts an item into its slot on a character (clothing, armor, weapon, left hand); what it wore there is carried instead. The player takes it from the inventory (queued)";
    "unequip"(target: Target, slot: Text) "Takes off what a character wears in \"clothing\", \"head\", \"body\", \"weapon\", \"offhand\" or \"tool\" (into its carried items) (queued)";
    "give_item_to"(target: Target, item: Item, count: Num) "Adds items to what a character carries (dropped when it is defeated) (queued)";
    "take_item_from"(target: Target, item: Item, count: Num) "Takes items from what a character carries (queued)";
    "teach_spell"(target: Target, spell: Spell) "Teaches a character a spell; a companion casts Heal and Bolt spells from its own mana (queued)";
    "npc_knows_spell"(target: Target, spell: Spell) "Whether a character on the map has learned a spell (teach_spell, or a scroll given to a companion)";
    "add_stock"(target: Target, item: Item, count: Num) "Adds items to a merchant's stock, e.g. what a villager produced (queued)";
    "set_stock"(target: Target, item: Item, count: Num) "Sets how many of an item a merchant has; 0 removes it from the stock (queued)";
    "stock_of"(target: Target, item: Item) "How many of an item a merchant on the map has (999 = unlimited; 0 if none)";
    "show_image"(sheet: Sheet) "Shows a picture over the game (a sprite sheet drawn whole, scaled down to fit the window); the event waits until the player closes it (queued)";
    "show_image"(sheet: Sheet, options: Opts) "Like show_image(); options #{ wait: false (stays under the following dialogs until the event ends or hide_image()), scroll: true (a tall picture pans down and back up), speed: pixels per second, scale: 2 (pixel art bigger) } (queued)";
    "hide_image"() "Takes away a picture shown with show_image() (queued)";
    "hide_ui"() "Hides the HUD (status frame, clock, quick bar, buttons), e.g. for a cut-scene; dialogs still show. The player shows it again with H (queued)";
    "show_ui"() "Shows the HUD again after hide_ui() or H (queued)";
    "player_x"() "The player's x position in tiles (fractional; the feet)";
    "player_y"() "The player's y position in tiles (fractional; the feet)";
    "player_max_hp"() "The player's maximum hit points";
    "player_facing"() "Where the player looks: \"up\", \"down\", \"left\" or \"right\"";
    "aim"(function: Func) "Magic targeting: the player's next click calls function(x, y) with the target in tiles; in a spell's script the spell is paid then (queued)";
    "fly"(sheet: Sheet, x: Num, y: Num, function: Func) "An animated sprite flies from the player to tile x, y, then calls function(x, y) (queued)";
    "fly"(sheet: Sheet, x: Num, y: Num, function: Func, options: Opts) "Like fly(); options #{ speed: tiles/s, frames: [first, last] or tag: \"animation\", fps, scale, light: radius, color: [r, g, b] } (queued)";
    "effect"(sheet: Sheet, x: Num, y: Num) "Plays a sprite sheet's frames once, centered on tile x, y (explosions) (queued)";
    "effect"(sheet: Sheet, x: Num, y: Num, options: Opts) "Like effect(); options #{ frames: [first, last] or tag: \"animation\", fps, scale, light: radius, color: [r, g, b], shake: seconds } (queued)";
    "damage_area"(x: Num, y: Num, radius: Num, damage: Num) "Damages every character within radius tiles of tile x, y (spell damage skills apply) (queued)";
    "text"(text: Line) "Fills in {player}, {sir|madam} and {variable} placeholders";
    "random"(n: Num) "Random integer from 0 to n-1";
    "say"(text: Line) "Shows a dialog (queued)";
    "say"(speaker: Line, text: Line) "Shows a dialog with a speaker name (queued)";
    "say"(entity: Num, text: Line) "Shows a dialog with an entity's name as speaker, e.g. say(SELF, \"...\") (queued)";
    "say"(speaker: Line, text: Line, options: Opts) "A dialog line with options: #{ mood: \"worried\" } shows the speaker's portrait for that mood for this line, #{ emote: \"joy\" } an emotion over the speaker's head (queued)";
    "choice"(text: Line, options: List, var: Var) "Asks a question; the chosen index goes into the variable (queued)";
    "teleport"(map: Map, spawn: Spawn) "Moves the player to a spawn point on a map (queued)";
    "set_active"(target: Target, active: Bool) "Shows / hides an entity (queued)";
    "set_state"(target: Target, state: State) "Switches a prop to one of its states (queued)";
    "set_frame"(target: Target, frame: Num) "Old: sets a prop's frame offset (queued)";
    "move_by"(target: Target, dx: Num, dy: Num) "Moves a character by tiles (queued)";
    "face"(target: Target, direction: Dir) "Turns a character (queued)";
    "wait"(seconds: Num) "Waits before the next queued action";
    "emit"(event: Event) "Fires a custom event (queued)";
    "then_call"(function: Func) "Calls another script function after the queued actions";
    "cutscene"(function: Func) "Plays a script function as a cut-scene: HUD hidden, black bars, everyone else waits until its actions are done (queued)";
    "walk_to"(target: Target, x: Num, y: Num) "Walks a character (or the player) onto tile x, y (queued, blocks until it arrives)";
    "approach"(target: Target, other: Target) "Walks a character up to another one, a tile before it; then both face each other (queued, blocks)";
    "place"(target: Target, x: Num, y: Num) "Puts a character (or the player) on tile x, y at once (queued)";
    "dim"(amount: Num, seconds: Num) "Darkens the scene to amount (0 = normal, 1 = deep night) over seconds, so lights show by day; a cut-scene's end brightens it again (queued)";
    "disable_spawners"(map: Map) "Switches off every enemy spawner of a map (\"\" = this one); their characters here vanish. Saved (queued)";
    "disable_spawners"(map: Map, zone: Text) "Switches off one spawner zone of a map by name (queued)";
    "enable_spawners"(map: Map) "Switches a map's spawners on again (queued)";
    "defeat_enemies"() "Defeats every hostile character on the current map at once: loot drops, their Defeated triggers run (queued)";
    "autosave"() "Saves to the Autosave slot once the running event (dialogs, cut-scene) has finished (queued)";
    "enable_spawners"(map: Map, zone: Text) "Switches one spawner zone switched off by name on again (queued)";
    "heal"(amount: Num) "Heals the player (queued)";
    "order"(target: Target, order: Text) "Tells a companion (or \"all\") what to do: \"attack\", \"defend\" (only what attacks you), \"passive\" (no fighting), \"stay\" (wait here), \"follow\" (queued)";
    "order_of"(name: Text) "How a companion fights: \"attack\", \"defend\" or \"passive\" (\"\" = no companion of that name)";
    "emote"(target: Target, emotion: Text) "Shows an emotion over a character's head (\"player\" too) for 2.5 s: a tag of an Emotions sprite sheet (\"surprise\", \"joy\", \"sleepy\", ...) (queued)";
    "emote"(target: Target, emotion: Text, options: Opts) "Like emote(); options #{ seconds: 4 (below 0: until clear_emote), wait: true (the script's next actions wait until it is over) } (queued)";
    "clear_emote"(target: Target) "Takes a character's emotion away (queued)";
    "bark"(target: Target, text: Line) "A short line in a speech bubble over a character's head (\"player\" too) that doesn't stop play; it stays by its length (queued)";
    "bark"(target: Target, text: Line, seconds: Num) "Like bark(), for `seconds` (queued)";
    "mood"(target: Target, mood: Text) "A character's portrait shows that mood (its definition's mood portrait) until changed or the event ends; \"\" = its usual one (queued)";
    "open_companion"(target: Target) "Opens a companion's window (its gear and bag next to the player's inventory) once the event is over (queued)";
    "is_staying"(name: Text) "Whether a companion was told to wait where it is";
    "heal_npc"(target: Target, amount: Num) "Heals a character by name (a companion, \"player\"); a negative amount hurts it (queued)";
    "play_sound"(sound: Sound) "Plays a sound (queued)";
    "play_music"(sound: Sound) "Plays music (queued)";
    "set_locked"(target: Target, locked: Bool) "Locks / unlocks a prop (queued)";
    "open_shop"(target: Target) "Opens a merchant's trade window (queued)";
    "patrol"(target: Target) "Starts a character's own patrol route (queued)";
    "patrol"(target: Target, points: List) "Patrols [[x, y], ...] tile waypoints (queued)";
    "stop_patrol"(target: Target) "Stops a character's patrol (queued)";
    "sleep"(hours: Num) "The player sleeps that many hours: the night passes, HP and mana are restored (queued, blocks)";
    "is_sitting"() "Whether the player sits on a chair or bench";
    "is_sleeping"() "Whether the player lies in a bed or the night is passing";
    "lockpick"(difficulty: Num, var: Var) "Plays the lockpicking game; 1 (picked) or 0 goes into the variable (queued, blocks)";
    "unlock_achievement"(id: Achievement) "Unlocks an achievement for the player (queued; kept across save games)";
    "achievement_unlocked"(id: Achievement) "Whether the player has unlocked an achievement";
    "open_crafting"(station: Station) "Opens the crafting window with the station's recipes (queued)";
    "open_crafting"(station: Station, title: Line) "Opens the crafting window with a title, e.g. \"Alchemy table\" (queued)";
    "can_craft"(recipe: Recipe) "Whether the player has the ingredients (and skill) for a recipe";
    "follow"(target: Target) "The character joins the player as a companion (queued, saved)";
    "stop_following"(target: Target) "The companion leaves: stays on its own map, else goes back home (queued)";
    "is_companion"(name: Text) "Whether the character with that name follows the player";
    "companion_count"() "How many companions follow the player";
    "stop_schedule"(target: Target) "The character stops following its daily schedule and stays where it is (queued, saved)";
    "follow_schedule"(target: Target) "The character follows its daily schedule again (queued)";
    "start_quest"(quest: Quest) "Starts a quest (queued)";
    "advance_quest"(quest: Quest) "Moves a quest to its next stage (queued)";
    "set_quest_stage"(quest: Quest, stage: Num) "Sets a quest's stage (queued)";
    "complete_quest"(quest: Quest) "Completes a quest (queued)";
    "fail_quest"(quest: Quest) "Fails a quest (queued)";
    "quest_stage"(quest: Quest) "Current stage of a quest (-1 = not started)";
    "quest_active"(quest: Quest) "Whether a quest is active";
    "quest_done"(quest: Quest) "Whether a quest is completed";
};

impl Arg {
    /// Ids of the project (or fixed words): the editor offers them as soon as the argument starts.
    pub fn is_id(self) -> bool {
        !matches!(self, Arg::Text | Arg::Line | Arg::Num | Arg::Bool | Arg::List | Arg::Opts)
    }

    /// The kind of id this argument names (`None` for texts, numbers, targets, …): what renames
    /// rewrite and the script check verifies.
    pub fn id_kind(self) -> Option<IdKind> {
        Some(match self {
            Arg::Item => IdKind::Item,
            Arg::Map => IdKind::Map,
            Arg::Spawn => IdKind::Spawn,
            Arg::Sound => IdKind::Sound,
            Arg::Quest => IdKind::Quest,
            Arg::State => IdKind::State,
            Arg::Skill => IdKind::Skill,
            Arg::Spell => IdKind::Spell,
            Arg::Sheet => IdKind::Sheet,
            Arg::Character => IdKind::Character,
            Arg::Recipe => IdKind::Recipe,
            Arg::Achievement => IdKind::Achievement,
            Arg::Prop => IdKind::Prop,
            _ => return None,
        })
    }
}

/// The API functions' id arguments: (function, argument index, kind), from the table.
pub fn api_ids() -> &'static [(&'static str, usize, IdKind)] {
    static IDS: LazyLock<Vec<(&'static str, usize, IdKind)>> = LazyLock::new(|| {
        let mut out: Vec<(&str, usize, IdKind)> = API.iter().flat_map(|f| f.params.iter().enumerate().filter_map(move |(i, (_, a))| a.id_kind().map(|k| (f.name, i, k)))).collect();
        out.dedup();
        out
    });
    &IDS
}

/// What argument `index` of `function` expects in any of its overloads (`None`: not an API
/// function, or no such argument).
pub fn arg_kinds(function: &str, index: usize) -> impl Iterator<Item = Arg> + '_ {
    API.iter().filter(move |f| f.name == function).filter_map(move |f| f.params.get(index).map(|(_, a)| *a))
}

/// Does argument `index` of `function` carry a line players read (dialogs, questions, titles)?
pub fn is_text_arg(function: &str, index: usize) -> bool {
    arg_kinds(function, index).any(|a| a == Arg::Line)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every function the engine registers for scripts is in the completion table.
    #[test]
    fn api_table_is_complete() {
        let src = include_str!("script.rs");
        let registered: std::collections::BTreeSet<&str> = src.split("register_fn(\"").skip(1).filter_map(|s| s.split('"').next()).collect();
        assert!(registered.len() > 60, "{registered:?}");
        let missing: Vec<&&str> = registered.iter().filter(|n| !API.iter().any(|f| f.name == **n)).collect();
        assert!(missing.is_empty(), "not in the completion table: {missing:?}");
        // ... and the table has nothing the engine doesn't know.
        let unknown: Vec<&str> = API.iter().map(|f| f.name).filter(|n| !registered.contains(n)).collect();
        assert!(unknown.is_empty(), "not registered in the engine: {unknown:?}");
        // ... and Documentation/manual/08-scripting.md describes every one.
        let doc = include_str!("../../../Documentation/manual/08-scripting.md");
        let undocumented: Vec<&&str> = registered.iter().filter(|n| !doc.contains(&format!("`{n}("))).collect();
        assert!(undocumented.is_empty(), "missing in Documentation/manual/08-scripting.md: {undocumented:?}");
    }

    /// The id arguments derived from the table are the ones renames and the script check knew
    /// (the list this replaced).
    #[test]
    fn id_arguments_from_the_table() {
        let expected: &[(&str, usize, IdKind)] = &[
            ("give_item", 0, IdKind::Item),
            ("take_item", 0, IdKind::Item),
            ("has_item", 0, IdKind::Item),
            ("item_count", 0, IdKind::Item),
            ("teleport", 0, IdKind::Map),
            ("set_map_weather", 0, IdKind::Map),
            ("teleport", 1, IdKind::Spawn),
            ("disable_spawners", 0, IdKind::Map),
            ("enable_spawners", 0, IdKind::Map),
            ("play_sound", 0, IdKind::Sound),
            ("play_music", 0, IdKind::Sound),
            ("start_quest", 0, IdKind::Quest),
            ("advance_quest", 0, IdKind::Quest),
            ("set_quest_stage", 0, IdKind::Quest),
            ("complete_quest", 0, IdKind::Quest),
            ("fail_quest", 0, IdKind::Quest),
            ("quest_stage", 0, IdKind::Quest),
            ("quest_active", 0, IdKind::Quest),
            ("quest_done", 0, IdKind::Quest),
            ("set_state", 1, IdKind::State),
            ("skill", 0, IdKind::Skill),
            ("set_skill", 0, IdKind::Skill),
            ("knows_spell", 0, IdKind::Spell),
            ("learn_spell", 0, IdKind::Spell),
            ("fly", 0, IdKind::Sheet),
            ("equip", 1, IdKind::Item),
            ("give_item_to", 1, IdKind::Item),
            ("take_item_from", 1, IdKind::Item),
            ("effect", 0, IdKind::Sheet),
            ("show_image", 0, IdKind::Sheet),
            ("add_stock", 1, IdKind::Item),
            ("teach_spell", 1, IdKind::Spell),
            ("npc_knows_spell", 1, IdKind::Spell),
            ("set_stock", 1, IdKind::Item),
            ("stock_of", 1, IdKind::Item),
            ("defeated_count", 0, IdKind::Character),
            ("can_craft", 0, IdKind::Recipe),
            ("unlock_achievement", 0, IdKind::Achievement),
            ("achievement_unlocked", 0, IdKind::Achievement),
            ("carry", 1, IdKind::Prop),
        ];
        let derived: std::collections::BTreeSet<(&str, usize, u8)> = api_ids().iter().map(|(f, i, k)| (*f, *i, *k as u8)).collect();
        let wanted: std::collections::BTreeSet<(&str, usize, u8)> = expected.iter().map(|(f, i, k)| (*f, *i, *k as u8)).collect();
        assert_eq!(derived, wanted);
        assert!(is_text_arg("say", 0) && is_text_arg("say", 1) && is_text_arg("choice", 0) && is_text_arg("open_crafting", 1));
        assert!(!is_text_arg("choice", 2) && !is_text_arg("give_item", 0) && !is_text_arg("order", 1) && !is_text_arg("disable_spawners", 1));
        assert!(arg_kinds("lockpick", 1).any(|a| a == Arg::Var));
    }
}
