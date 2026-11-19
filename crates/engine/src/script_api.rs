//! The script API as the editor and the tools see it: every function the engine registers
//! (`script.rs`), its parameters with what each expects, and a one-line description. One table
//! feeds the script editor's completion and signature hints, the agent server's API resource,
//! the renames and the script check (which arguments are ids: [`api_ids`]), the translation
//! templates (which arguments are texts) and the editor's flag / variable collectors. A test
//! keeps it complete against `script.rs` and `Documentation/manual/08-scripting.md`.

use std::sync::LazyLock;

use crate::script_refs::IdKind;

/// The script API's version: the engine's MAJOR.MINOR. Plugins name the version they need
/// (`PluginDef::api`); a plugin needing a newer one isn't loaded.
pub const API_VERSION: &str = "0.31";

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
    /// A condition: "poisoned", "prone", "exhaustion"… (until 0.27 a status effect).
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
    /// A faction id (Database \u{203A} Factions).
    Faction,
    /// A common event id.
    Common,
    /// Any text: a zone, order, mood or slot name.
    Text,
    /// A line players read (dialog, question, title): collected for translation and shown
    /// translated.
    Line,
    Num,
    Bool,
    List,
    /// A class id (Database \u{203A} Classes).
    Class,
}

pub struct ApiFn {
    pub name: &'static str,
    pub params: &'static [(&'static str, Arg)],
    pub doc: &'static str,
    /// The API version that added it ("" = before versions were kept, 0.26 or older).
    pub since: &'static str,
    /// Deprecated: what to use instead. It still works (until 2.0 once 1.0 is out); the script
    /// check warns and completion marks it.
    pub deprecated: Option<&'static str>,
}

/// The table's rows: `"name"(param: Kind, …) "description";`, optionally after
/// `[since "0.27"]` and `[deprecated "what to use instead"]`.
macro_rules! api {
    (@since) => { "" };
    (@since [since $v:literal] $($rest:tt)*) => { $v };
    (@since [$other:ident $v:literal] $($rest:tt)*) => { api!(@since $($rest)*) };
    (@dep) => { None };
    (@dep [deprecated $v:literal] $($rest:tt)*) => { Some($v) };
    (@dep [$other:ident $v:literal] $($rest:tt)*) => { api!(@dep $($rest)*) };
    ($( $([$attr:ident $val:literal])* $name:literal ( $($p:ident : $k:ident),* ) $doc:literal;)*) => {
        &[$(ApiFn {
            name: $name,
            params: &[$((stringify!($p), Arg::$k)),*],
            doc: $doc,
            since: api!(@since $([$attr $val])*),
            deprecated: api!(@dep $([$attr $val])*),
        }),*]
    };
}

/// The API version as a script sees it (`api_version()`).
pub fn api_version() -> &'static str {
    API_VERSION
}

/// The deprecated functions a script calls: (line, 1-based; function; what to use instead).
/// Comments and strings are skipped.
pub fn deprecated_calls(src: &str) -> Vec<(usize, &'static str, &'static str)> {
    let mut out = Vec::new();
    for (k, line) in src.lines().enumerate() {
        let code = strip_strings_and_comments(line);
        for f in API.iter().filter(|f| f.deprecated.is_some()) {
            let mut from = 0;
            while let Some(i) = code[from..].find(f.name) {
                let at = from + i;
                let before = code[..at].chars().next_back();
                let after = code[at + f.name.len()..].trim_start();
                let word_start = before.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
                if word_start && after.starts_with('(') && !code[..at].trim_end().ends_with("fn") {
                    out.push((k + 1, f.name, f.deprecated.unwrap_or_default()));
                    break;
                }
                from = at + f.name.len();
            }
        }
    }
    out
}

/// A line of Rhai with string contents blanked and a `//` comment cut off.
fn strip_strings_and_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_str = false;
    let mut prev = ' ';
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            if c == '"' && prev != '\\' {
                in_str = false;
                out.push('"');
            } else {
                out.push(' ');
            }
        } else if c == '"' {
            in_str = true;
            out.push('"');
        } else if c == '/' && chars.peek() == Some(&'/') {
            break;
        } else {
            out.push(c);
        }
        prev = c;
    }
    out
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
    "set_home"(map: Map, owned: Bool) "Makes a map the player's home (true) or no longer (false): furniture items can be placed there. Saved (immediately)";
    "is_home"(map: Map) "Whether a map is one of the player's homes";
    "player_hp"() "The player's hit points";
    "player_name"() "The player's name";
    "player_sex"() "\"male\" or \"female\"";
    "player_race"() "The player's race id (Database \u{203A} Races; a subrace's, e.g. \"hill-dwarf\"), \"\" without one";
    [since "0.28"] "knows_language"(language: Text) "Whether the hero speaks a language (\"dwarvish\", \"elvish\"…): their race's, the creator's picks and their background's";
    [since "0.28"] "has_resistance"(damage: Text) "Whether the hero resists a damage type (\"fire\", \"poison\"…): their race's, a dragonborn's ancestry";
    [since "0.28"] "background"() "The hero's background id (Database \u{203A} Backgrounds), \"\" without one";
    [since "0.28"] "race_choice"(choice: Text) "What the hero picked for a racial choice in the creator, as an array (e.g. race_choice(\"high-elf-cantrip\") = [\"fire-bolt\"]); empty when nothing was picked";
    "carry"(target: Target, prop: Prop) "A character (or \"player\") carries a prop in both hands, e.g. carry(\"player\", \"crate\"); while carrying it holds nothing else and the player can't attack (queued)";
    "stop_carrying"(target: Target) "Puts down what a character carries (queued)";
    "player_carrying"() "The prop the player carries (\"\" = none)";
    "player_faction"() "The player's faction id (their race's, else the player character's; \"\" without one)";
    "standing"(faction: Faction) "The player's standing with a faction (-100..100; at -20 or below its characters attack the player)";
    "change_standing"(faction: Faction, amount: Num) "Raises (or with a negative amount lowers) the player's standing with a faction; its characters turn hostile or calm down at once (queued)";
    [since "0.27"] "plugin_param"(plugin: Text, name: Text) "A plugin's parameter as the project set it in the Plugins window (else its default): a number, on / off or a text; () when the plugin or parameter isn't there";
    "set_faction"(target: Target, faction: Faction) "A character on this map (\"self\", a name) or \"player\" joins a faction; \"\" = back to its own (the placed character's, its definition's, its race's). Saved; who attacks whom changes at once (queued)";
    "faction_of"(target: Target) "The faction id a character belongs to now: \"player\", \"self\" (also SELF) or a name on this map; \"\" = none";
    "is_female"() "Whether the player character is female";
    "is_male"() "Whether the player character is male";
    "hour"() "Time of day in hours (0..24, e.g. 13.5 = half past one in the afternoon)";
    "day"() "The in-game day (0 on the first day; sleeping and midnight move it on), e.g. for things that happen once a day";
    "is_night"() "Whether it is night (dark outdoors)";
    "is_sneaking"() "Whether the player is sneaking (the sneak key: Left Ctrl, V or the left stick)";
    "set_hour"(hour: Num) "Sets the time of day, e.g. set_hour(22) (queued)";
    [deprecated "is_proficient(skill)"] "skill"(skill: Skill) "1 when the hero is proficient in a skill or tool, else 0 (until 0.27 a skill tree's level). Deprecated: use is_proficient";
    [deprecated "set_proficient(skill, on)"] "set_skill"(skill: Skill, level: Num) "Makes the hero proficient in a skill or tool (level above 0) or not (0). Deprecated: use set_proficient (queued)";
    [since "0.28"] "roll"(dice: Text) "Rolls dice such as \"2d6+3\", \"1d20+DEX\" or \"8d6\" (STR to CHA = the hero's ability modifier, PROF = the proficiency bonus) and returns the total; the roll goes into the roll log (immediately)";
    [since "0.28"] "check"(what: Skill, dc: Num) "An ability check of the hero against a DC: a skill (\"persuasion\"), an ability (\"str\") or ability:tool (\"dex:thieves-tools\"); true on a success. Rolled at once, shown on screen and in the roll log; Check passed conditions see it (immediately)";
    [since "0.28"] "check"(what: Skill, dc: Num, mode: Text) "Like check(); mode \"advantage\" or \"disadvantage\" rolls two d20s and keeps the higher or lower (immediately)";
    [since "0.28"] "save"(ability: Text, dc: Num) "A saving throw of the hero (\"dex\", \"wis\", ...) against a DC; true on a success (immediately)";
    [since "0.28"] "save"(ability: Text, dc: Num, mode: Text) "Like save(); mode \"advantage\" or \"disadvantage\" (immediately)";
    [since "0.28"] "modifier"(what: Skill) "The hero's bonus: an ability's modifier (\"str\"), a skill's or ability:tool's check bonus with proficiency, or a saving throw's (\"save:dex\")";
    [since "0.28"] "ability"(ability: Text) "The hero's ability score: \"str\", \"dex\", \"con\", \"int\", \"wis\" or \"cha\" (1 to 30)";
    [since "0.28"] "set_ability"(ability: Text, score: Num) "Sets one of the hero's ability scores (1 to 30) (immediately)";
    [since "0.28"] "proficiency"() "The hero's proficiency bonus by level (+2 at levels 1-4 up to +6 at 17-20)";
    [since "0.28"] "is_proficient"(what: Skill) "Whether the hero is proficient in a skill (\"stealth\"), a tool (\"thieves-tools\") or a saving throw (\"save:dex\")";
    [since "0.28"] "set_proficient"(what: Skill, on: Bool) "Makes the hero proficient in a skill, tool or saving throw, or takes it away (immediately)";
    [since "0.28"] "armor_class"() "The hero's armor class: 10 + the DEX modifier + the worn armor's defense";
    [since "0.28"] "hit_dice"() "The hit dice the hero has left (one per level; spent and regained by resting)";
    [since "0.28"] "npc_stat"(target: Target, field: Text) "A value of a character's SRD stat block: \"str\" to \"cha\", \"ac\", \"hp\", \"speed\" (feet), \"darkvision\" (feet) or \"xp\"; target \"self\" (also SELF) or a name on this map; -1 without a stat block";
    "knows_spell"(spell: Spell) "Whether the hero knows the spell (a cantrip or a known spell, the spellbook's) or has it prepared";
    "learn_spell"(spell: Spell) "Teaches the hero a spell: only when their class's spell list has it (the SRD's rule; a wizard adds it to the spellbook) (queued)";
    [deprecated "slots_left(level)"] "mana"() "Deprecated: spells use slots since 0.30; always 0";
    [deprecated "restore_slots()"] "add_mana"(amount: Num) "Deprecated: spells use slots since 0.30; does nothing (queued)";
    [since "0.31"] "money"() "The hero's money in copper pieces: every coin carried by its worth (gp 100, sp 10, cp 1, ep 50, pp 1000)";
    [since "0.31"] "give_money"(cp: Num) "Gives the hero money in the fewest gold, silver and copper pieces, e.g. give_money(250) = 2 gp 5 sp";
    [since "0.31"] "take_money"(cp: Num) "Takes money from the hero's coins, the change given back; false (nothing taken) when they have less";
    [since "0.31"] "format_money"(cp: Num) "A sum in copper pieces as it reads in the player's language: format_money(150) = \"1 gp 5 sp\"";
    [since "0.30"] "spell_slots"(level: Num) "How many spell slots of a level (1-9) the hero has at their class level";
    [since "0.30"] "slots_left"(level: Num) "Spell slots of a level (1-9) the hero has left until a rest";
    [since "0.30"] "restore_slots"() "Gives the hero all their spell slots back (a potion, a shrine)";
    [since "0.30"] "prepare_spell"(spell: Spell) "Prepares a spell for the hero (a preparing caster: cleric, druid, paladin, wizard), as far as their count allows; false when it can't";
    [since "0.30"] "is_prepared"(spell: Spell) "Whether the hero has a spell prepared";
    [since "0.30"] "concentrating"() "The id of the spell the hero concentrates on, \"\" when none";
    [since "0.30"] "cast_spell"(spell: Spell) "The hero casts a spell outside a fight on themselves, with a slot (queued)";
    [since "0.30"] "cast_spell"(spell: Spell, target: Target, free: Bool) "The hero casts a spell on a target (\"player\" or a character on this map), free = without a slot (queued)";
    "xp"() "All the experience points the player has gained (experience and levels)";
    "level"() "The player's level (1 = the first)";
    [since "0.28"] "apply_condition"(target: Target, condition: Effect, seconds: Num) "A condition on \"player\" or a character: \"poisoned\", \"prone\", \"stunned\", \"exhaustion\" (one level)… for seconds (0 = until removed or a rest) (queued)";
    [since "0.28"] "apply_condition"(target: Target, condition: Effect, seconds: Num, options: Opts) "Like apply_condition(); options #{ save: \"con\", dc: 12 (a saving throw keeps it off), damage: \"1d4\", type: \"poison\" (damage every round = 6 s; type \"heal\" = healing), level: 2 (exhaustion levels) }; condition \"\" = only the damage or healing (queued)";
    [since "0.28"] "remove_condition"(target: Target, condition: Effect) "Ends a condition (all levels of exhaustion); \"\" or \"all\" = every condition and ongoing damage (queued)";
    [since "0.28"] "has_condition"(condition: Effect) "Whether the hero has a condition (\"any\" = any)";
    [since "0.28"] "has_condition"(target: Target, condition: Effect) "Whether \"player\", \"self\" (also SELF) or a character on this map has a condition";
    [since "0.28"] "srd_attribution"() "The SRD 5.1's attribution (CC BY 4.0) when the project uses SRD content (Project › Uses SRD 5.1 content), else \"\": for credits a script shows";
    [since "0.28"] "exhaustion"() "The hero's level of exhaustion (0-6)";
    [since "0.28"] "set_exhaustion"(level: Num) "Sets the hero's exhaustion (0 = none, 6 = death) (immediately)";
    [since "0.28"] "short_rest"() "Opens the rest window: the hero spends hit dice (d10 + CON each), an hour passes (queued)";
    [since "0.28"] "long_rest"() "A long rest of 8 hours: full HP, half the hit dice back, one level of exhaustion less, timed conditions over (queued)";
    [since "0.28"] "long_rest"(hours: Num) "Like long_rest(), for some hours (queued)";
    [since "0.29"] "start_combat"() "Starts a turn-based fight with the hostile characters within 12 tiles of the player; their allies and the party join (queued)";
    [since "0.29"] "in_combat"() "Whether a turn-based fight is going on";
    [since "0.29"] "combat_round"() "The round of the fight going on (1 = the first; 0 = no fight)";
    [since "0.30"] "class"() "The hero's class id (Database \u{203A} Classes), \"\" without one";
    [since "0.30"] "subclass"() "The hero's subclass id (\"champion\", \"thief\"), \"\" before they chose one";
    [since "0.30"] "class_level"() "The hero's level in their class (0 without a class)";
    [since "0.30"] "resource"(resource: Text) "Uses left of one of the hero's class resources, e.g. resource(\"rage\"), resource(\"ki\"), resource(\"lay-on-hands\") (its hit points); 0 without it. A short or long rest gives them back";
    [since "0.30"] "has_feature"(feature: Text) "Whether the hero has a class feature at their level, e.g. has_feature(\"second-wind\"); chosen options count (\"fighter-fighting-style-defense\")";
    [since "0.30"] "level_up_pending"() "Whether level-up choices (the hero's or a party member's) wait for the level-up window";
    [since "0.30"] "set_class"(target: Target, class: Class) "Gives the hero (\"player\") or a party member a class: its saving throws and proficiencies; the level's choices open the level-up window (queued)";
    [since "0.30"] "set_class"(target: Target, class: Class, level: Num) "Like set_class(target, class); a party member also gets this level (queued)";
    [deprecated "apply_condition(target, condition, seconds, options)"] "apply_effect"(target: Target, effect: Effect, strength: Num, seconds: Num) "Deprecated: use apply_condition. The status effects of 0.27 as conditions: \"poison\" = poisoned + poison damage per round, \"burn\" = fire damage per round, \"regen\" = healing per round (strength x 6 per round), \"stun\" = stunned, \"slow\" = restrained; \"haste\", \"strength\", \"weakness\", \"shield\" do nothing (queued)";
    [deprecated "remove_condition(target, condition)"] "cure"(target: Target, effect: Effect) "Deprecated: use remove_condition. Ends a condition, e.g. cure(\"player\", \"poison\") (queued)";
    [deprecated "remove_condition(target, \"all\")"] "cure"(target: Target) "Deprecated: use remove_condition. Ends every condition (queued)";
    "defeated_count"(character: Character) "How many characters of this definition the player has defeated (the bestiary's count), e.g. defeated_count(\"slime\")";
    "shake"(seconds: Num) "Shakes the screen like a hit for a while: earthquakes, explosions (queued)";
    "shake"(seconds: Num, strength: Num) "Shakes the screen; strength 1 = like a hit, 2 = twice as hard (queued)";
    "set_weather"(kind: Weather) "Weather of the current map: \"rain\", \"snow\", \"fog\", \"none\" or \"default\" (the map's own); kept in the save game (queued)";
    "set_weather"(kind: Weather, intensity: Num) "Weather of the current map with a strength from 0 to 1, e.g. set_weather(\"rain\", 0.8) (queued)";
    "set_map_weather"(map: Map, kind: Weather, intensity: Num) "Weather of another map, e.g. set_map_weather(\"forest\", \"fog\", 0.5) (queued)";
    "weather"() "The current map's weather: \"none\", \"rain\", \"snow\" or \"fog\"";
    "weather_intensity"() "The current weather's strength (0 to 1)";
    "hp_of"(target: Target) "A character's HP right now: \"player\", \"self\" (also SELF), or a name on this map; -1 if there is no such character";
    [deprecated "has_condition(condition)"] "has_effect"(effect: Effect) "Deprecated: use has_condition. Whether the hero has a condition, e.g. has_effect(\"poison\") (= poisoned)";
    "give_xp"(amount: Num) "Gives experience; enough of it raises the level (negative takes it away, the level never drops) (queued)";
    "damage_around"(damage: Num, radius: Num) "Damages every character within radius tiles of the player (queued)";
    [since "0.28"] "damage_around"(damage: Num, radius: Num, damage_type: Text) "Like damage_around(), of a damage type (\"fire\", \"cold\"…; resistances halve it) (queued)";
    "player_light"(radius: Num, seconds: Num) "Lights the player's surroundings for a while (queued)";
    "player_light"(radius: Num, seconds: Num, options: Opts) "Like player_light(); options #{ color: [r, g, b], flicker: true } (queued)";
    "light_at"(x: Num, y: Num, radius: Num, seconds: Num) "A light at tile x, y for a while, fading out at the end (queued)";
    "light_at"(x: Num, y: Num, radius: Num, seconds: Num, options: Opts) "Like light_at(); options #{ color: [r, g, b], flicker: true } (queued)";
    "heal_area"(x: Num, y: Num, radius: Num, amount: Num) "Heals the player and friendly characters within radius tiles of tile x, y (queued)";
    "equip"(target: Target, item: Item) "Puts an item into its slot on a character (clothing, armor, weapon, left hand); what it wore there is carried instead. The player takes it from the inventory (queued)";
    "unequip"(target: Target, slot: Text) "Takes off what a character wears in \"clothing\", \"head\", \"body\", \"weapon\", \"offhand\" or \"tool\" (into its carried items) (queued)";
    "give_item_to"(target: Target, item: Item, count: Num) "Adds items to what a character carries (dropped when it is defeated) (queued)";
    "take_item_from"(target: Target, item: Item, count: Num) "Takes items from what a character carries (queued)";
    "teach_spell"(target: Target, spell: Spell) "Teaches a party member a spell, when its class's list has it; it casts it in fights when the player plays its turns (queued)";
    "npc_knows_spell"(target: Target, spell: Spell) "Whether a character on the map has learned a spell (teach_spell, or a scroll given to a companion)";
    "add_stock"(target: Target, item: Item, count: Num) "Adds items to a merchant's stock, e.g. what a villager produced (queued)";
    "set_stock"(target: Target, item: Item, count: Num) "Sets how many of an item a merchant has; 0 removes it from the stock (queued)";
    "stock_of"(target: Target, item: Item) "How many of an item a merchant on the map has (999 = unlimited; 0 if none)";
    "show_image"(sheet: Sheet) "Shows a picture over the game (a sprite sheet drawn whole, scaled down to fit the window); the event waits until the player closes it (queued)";
    "move_route"(target: Target, steps: List) "A character (or \"player\") walks a list of steps; the event waits until it is done: \"move up 2\", \"toward 1\", \"away 1\" (the player), \"turn left\", \"turn random\", \"face player\", \"wait 0.5\", \"jump 1,0\", \"speed 2\" (times its usual speed), \"anim wave 1.5\" (a clip for seconds), \"sound blip\", \"through on\", \"script name\". A step whose tile is taken is left out (queued)";
    "move_route"(target: Target, steps: List, options: Opts) "Like move_route(); options #{ repeat: true (again and again; the event doesn't wait), skip: false (wait until a taken tile is free), wait: false } (queued)";
    "get_text"(name: Text) "A text variable's value (\"\" when unset); {name} in texts shows it";
    "set_text"(name: Text, value: Text) "Sets a text variable (shown by {name} placeholders; saved) (immediately)";
    "input_number"(text: Line, var: Var, min: Num, max: Num) "Asks the player for a whole number between min and max (a window with the text) and puts it into a variable; the event waits (queued)";
    "input_text"(text: Line, var: Text, max_length: Num) "Asks the player for a text (a name, a password; prefilled with the text variable's value) and puts it into a text variable; the event waits (queued)";
    "call_common"(id: Common) "Runs a common event's actions (Database \u{203A} Common events), as an action of this event would (queued)";
    "timer_start"(name: Text, seconds: Num) "Starts (or restarts) a hidden timer; when it runs out nothing runs (check it with timer_left()). Timers pause while a menu or a dialog is open, and are saved (immediately)";
    "timer_start"(name: Text, seconds: Num, options: Opts) "Like timer_start(); options #{ shown: true (at the top of the screen), run: \"function\" (runs when it runs out), event: \"name\" (fires that custom event) } (immediately)";
    "timer_left"(name: Text) "Seconds left on a timer (0 when it isn't running)";
    "timer_stop"(name: Text) "Stops a timer: nothing runs (immediately)";
    "tint_screen"(color: Text, seconds: Num) "Colours the whole screen over seconds: a colour name, \"#rrggbb\" (a light tint), \"#rrggbbaa\" (aa = how strongly) or [r, g, b, a]; stays until changed, saved with the game (queued)";
    "clear_tint"(seconds: Num) "Takes the screen tint away over seconds (queued)";
    "flash_screen"(color: Text, seconds: Num) "A flash of a colour fading out over seconds (lightning, magic); left out when the player turned off screen effects (queued)";
    "fade_out"(seconds: Num) "Fades the screen to black over seconds and waits; it stays black across map changes until fade_in() (dialogs show over it) (queued)";
    "fade_out"(seconds: Num, color: Text) "Fades to a colour instead of black (queued)";
    "fade_in"(seconds: Num) "Fades the screen back in over seconds and waits (queued)";
    "camera_to"(x: Num, y: Num) "Pans the camera to tile x, y over a second and waits; it stays there while the event or cut-scene runs, then glides back to the player (queued)";
    "camera_to"(x: Num, y: Num, options: Opts) "Like camera_to(); options #{ seconds: 2, easing: \"in_out\" (linear, in, out, in_out), zoom: 2 (twice as close; 1 = usual), wait: false } (queued)";
    "camera_follow"(target: Target) "Pans the camera to a character (\"self\", a name) and follows it as it moves, while the event runs (queued)";
    "camera_follow"(target: Target, options: Opts) "Like camera_follow(); options #{ seconds, easing, zoom, wait } (queued)";
    "camera_reset"() "Pans the camera back to the player (0.8 s) and waits (queued)";
    "camera_reset"(seconds: Num) "Pans the camera back to the player over seconds (0 = at once) (queued)";
    "scroll_text"(text: Line) "Rolls text up the screen over black (credits, a story intro; lines starting with \"# \" are headings, \"\\n\" starts a line) and waits until it has gone; holding confirm speeds it up, cancel skips it. Works from a title screen entry too (queued)";
    "scroll_text"(text: Line, options: Opts) "Like scroll_text(); options #{ speed: 40 (screen pixels per second), background: \"#102030\" (a colour name, \"#rrggbb\", \"#rrggbbaa\" or [r, g, b, a]; a see-through one shows the map) } (queued)";
    "picture_show"(layer: Num, sheet: Sheet, x: Num, y: Num) "Shows a picture on a layer (1 to 100, drawn in number order above the map and below the HUD; replaces what the layer showed): a sprite sheet's frame, x, y screen pixels from the top left corner. Saved with the game (queued)";
    "picture_show"(layer: Num, sheet: Sheet, x: Num, y: Num, options: Opts) "Like picture_show(); options #{ frame: 0, anchor: \"center\" (top_left, top, top_right, left, center, right, bottom_left, bottom, bottom_right), on_map: true (x, y = a map tile, the picture's middle on it), scale: 2, opacity: 0.5, rotation: 90 (degrees), tint: \"gold\" (a colour name, \"#rrggbb\" or [r, g, b]), blend: \"add\" } (queued)";
    "picture_move"(layer: Num, options: Opts) "Moves a picture: options #{ x, y, scale, opacity, rotation, tint (the targets; left out = unchanged), seconds: 1, easing: \"in_out\" (linear, in, out, in_out), wait: true (the event waits until it is there) } (queued)";
    "picture_erase"(layer: Num) "Takes the picture of a layer away (0 = every picture) (queued)";
    "hud_text"(id: Text, x: Num, y: Num, text: Line) "Puts a line on the HUD (replaces the element with this id): x, y = pixels from the top left corner; placeholders such as {coins} (a variable), {item:coin} (how many the player has) or {player} are filled in every frame. Saved with the game (immediately)";
    "hud_text"(id: Text, x: Num, y: Num, text: Line, options: Opts) "Like hud_text(); options #{ anchor: \"top_right\" (top_left, top, top_right, left, center, right, bottom_left, bottom, bottom_right: x, y count inwards from there), size: 24, color: \"gold\" (a colour name or \"#rrggbb\") } (immediately)";
    "hud_image"(id: Text, x: Num, y: Num, sheet: Sheet, frame: Num) "Puts a frame of a sprite sheet on the HUD (drawn at 2x); saved with the game (immediately)";
    "hud_image"(id: Text, x: Num, y: Num, sheet: Sheet, frame: Num, options: Opts) "Like hud_image(); options #{ anchor: \"top_right\", scale: 3 } (immediately)";
    "hud_bar"(id: Text, x: Num, y: Num, width: Num, value: Text, max: Text, color: Text) "Puts a bar on the HUD: value and max are each a variable name, a number, \"hp:NAME\" (a character's HP on this map; \"hp:player\" the player's) or \"max_hp:NAME\"; it follows them every frame. Saved with the game (immediately)";
    "hud_bar"(id: Text, x: Num, y: Num, width: Num, value: Text, max: Text, color: Text, options: Opts) "Like hud_bar(); options #{ anchor: \"top\", label: \"Grumbold\" (a name over the bar, translated) } (immediately)";
    [since "0.27"] "hud_minimap"(id: Text, x: Num, y: Num, size: Num) "Puts a small map on the HUD: a square of size pixels around the player, like the map screen (24 tiles across). Saved with the game (immediately)";
    [since "0.27"] "hud_minimap"(id: Text, x: Num, y: Num, size: Num, options: Opts) "Like hud_minimap(); options #{ anchor: \"top_right\", tiles: 32 } (how many tiles across) (immediately)";
    "hud_remove"(id: Text) "Takes a HUD element away (immediately)";
    "hud_clear"() "Takes every HUD element the scripts set away (immediately)";
    "window_open"(id: Text, title: Line, rows: List) "Opens a window over the game (it pauses the game; Esc closes it). rows: an array of lines (strings, placeholders filled in), #{ image: \"sheet\", frame: 0 } pictures and #{ button: \"Label\", run: \"function\" } buttons that run a script function (the window stays open: open it again with the same id to refresh it, or call window_close()). Not saved (after the function)";
    "window_close"() "Closes the window a script opened (after the function)";
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
    "damage_area"(x: Num, y: Num, radius: Num, damage: Num) "Damages every character within radius tiles of tile x, y (queued)";
    [since "0.28"] "damage_area"(x: Num, y: Num, radius: Num, damage: Num, damage_type: Text) "Like damage_area(), of a damage type (\"fire\"…; resistances halve it) (queued)";
    "text"(text: Line) "Fills in {player}, {sir|madam} and {variable} placeholders";
    "random"(n: Num) "Random integer from 0 to n-1";
    "say"(text: Line) "Shows a dialog (queued)";
    "say"(speaker: Line, text: Line) "Shows a dialog with a speaker name (queued)";
    "say"(entity: Num, text: Line) "Shows a dialog with an entity's name as speaker, e.g. say(SELF, \"...\") (queued)";
    "say"(speaker: Line, text: Line, options: Opts) "A dialog line with options: #{ mood: \"worried\" } shows the speaker's portrait for that mood for this line, #{ emote: \"joy\" } an emotion over the speaker's head (queued)";
    "say_at"(at: Text, text: Line) "Shows a dialog at \"top\", \"middle\" or \"bottom\" of the screen (queued)";
    "say_at"(at: Text, speaker: Line, text: Line) "Shows a dialog with a speaker name at \"top\", \"middle\" or \"bottom\" of the screen (queued)";
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
    "used_item"() "The item the player used on the trigger's owner (its 'Use item' trigger; \"\" for other events), e.g. for gifts";
    "self_name"() "The name of the placed character or prop whose trigger runs the script (\"\" if it has none)";
    "is_consumable"(item: Item) "Whether an item is used up when used (the game already took one when a 'Use item' trigger reacted to it)";
    "rush_to"(target: Target, to: Target) "A fighting character (Chase behaviour) runs to where another character (or \"player\") is, following it for 15 s, and fights whoever it sees on the way: a chief calling his men (queued)";
    "bark"(target: Target, text: Line) "A short line in a speech bubble over a character's head (\"player\" too) that doesn't stop play; it stays by its length (queued)";
    "bark"(target: Target, text: Line, seconds: Num) "Like bark(), for `seconds` (queued)";
    "mood"(target: Target, mood: Text) "A character's portrait shows that mood (its definition's mood portrait) until changed or the event ends; \"\" = its usual one (queued)";
    "open_companion"(target: Target) "Opens a companion's window (its gear and bag next to the player's inventory) once the event is over (queued)";
    "is_staying"(name: Text) "Whether a companion was told to wait where it is";
    [deprecated "heal_character(target, amount)"] "heal_npc"(target: Target, amount: Num) "Heals a character by name (a companion, \"player\"); a negative amount hurts it. Deprecated: use heal_character (queued)";
    [since "0.27"] "heal_character"(target: Target, amount: Num) "Heals a character by name (a companion, any character with HP; \"player\" = the player); a negative amount hurts it (queued)";
    [since "0.27"] "api_version"() "The script API version of this engine, e.g. \"0.27\" (plugins and scripts can check what they may use)";
    "play_sound"(sound: Sound) "Plays a sound (queued)";
    "play_sound"(sound: Sound, options: Opts) "Plays a sound with options #{ volume: 0.5 (times its own), pitch: 1.5 (2 = an octave higher and faster, 0.5 = lower), pan: -1 (left) .. 1 (right) } (queued)";
    "play_music"(sound: Sound) "Plays music (queued)";
    "play_music"(sound: Sound, seconds: Num) "Crossfades to the music over seconds: the old track fades out while the new one fades in (queued)";
    "play_music_effect"(sound: Sound) "Plays a music effect (a fanfare) once: the music ducks under it and comes back after (queued)";
    "play_music_effect"(sound: Sound, options: Opts) "Like play_music_effect(); options #{ wait: true } (the event waits until it is over) (queued)";
    "set_locked"(target: Target, locked: Bool) "Locks / unlocks a prop (queued); an unlocked door stays closed";
    "is_locked"(target: Target) "Whether a prop on this map is locked (false without a lock)";
    "open_shop"(target: Target) "Opens a merchant's trade window (queued)";
    "patrol"(target: Target) "Starts a character's own patrol route (queued)";
    "patrol"(target: Target, points: List) "Patrols [[x, y], ...] tile waypoints (queued)";
    "stop_patrol"(target: Target) "Stops a character's patrol (queued)";
    "sleep"(hours: Num) "The player sleeps that many hours: the night passes, HP and mana are restored (queued, blocks)";
    "is_sitting"() "Whether the player sits on a chair or bench";
    "is_sleeping"() "Whether the player lies in a bed or the night is passing";
    "lockpick"(dc: Num, var: Var) "Picks a lock in a custom puzzle: a DEX (Thieves' Tools) check against the DC; 1 (picked) or 0 goes into the variable (queued)";
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
            Arg::Common => IdKind::CommonEvent,
            Arg::Class => IdKind::Class,
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

    /// Functions added since versions were kept say so in the manual, deprecated ones say what
    /// replaces them.
    #[test]
    fn versions_and_deprecations_are_documented() {
        let doc = include_str!("../../../Documentation/manual/08-scripting.md");
        let row = |name: &str| doc.lines().find(|l| l.starts_with(&format!("| `{name}("))).unwrap_or_else(|| panic!("no manual row for {name}")).to_string();
        for f in API.iter().filter(|f| !f.since.is_empty()) {
            assert!(row(f.name).contains(&format!("since {}", f.since)), "08-scripting.md: {} should say *(since {})*", f.name, f.since);
        }
        for f in API.iter().filter(|f| f.deprecated.is_some()) {
            assert!(row(f.name).contains("Deprecated"), "08-scripting.md: {} should say it is deprecated", f.name);
            let new = f.deprecated.unwrap().split('(').next().unwrap();
            assert!(API.iter().any(|g| g.name == new && g.deprecated.is_none()), "{} is replaced by {new}, which the table lacks", f.name);
        }
        assert!(API.iter().filter(|f| !f.since.is_empty()).all(|f| crate::plugins::compare_versions(f.since, API_VERSION) != std::cmp::Ordering::Greater));
    }

    #[test]
    fn deprecated_calls_are_found() {
        let src = "fn a() {\n    heal_npc(\"tilda\", 2); // heal_npc(\n    say(\"heal_npc(x)\");\n    my_heal_npc(1);\n}\nfn heal_npc_twice() { heal_character(\"x\", 1) }";
        assert_eq!(deprecated_calls(src), vec![(2, "heal_npc", "heal_character(target, amount)")]);
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
            ("is_consumable", 0, IdKind::Item),
            ("teleport", 0, IdKind::Map),
            ("set_map_weather", 0, IdKind::Map),
            ("teleport", 1, IdKind::Spawn),
            ("disable_spawners", 0, IdKind::Map),
            ("enable_spawners", 0, IdKind::Map),
            ("play_sound", 0, IdKind::Sound),
            ("play_music", 0, IdKind::Sound),
            ("play_music_effect", 0, IdKind::Sound),
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
            ("check", 0, IdKind::Skill),
            ("modifier", 0, IdKind::Skill),
            ("is_proficient", 0, IdKind::Skill),
            ("set_proficient", 0, IdKind::Skill),
            ("knows_spell", 0, IdKind::Spell),
            ("learn_spell", 0, IdKind::Spell),
            ("fly", 0, IdKind::Sheet),
            ("equip", 1, IdKind::Item),
            ("give_item_to", 1, IdKind::Item),
            ("take_item_from", 1, IdKind::Item),
            ("effect", 0, IdKind::Sheet),
            ("show_image", 0, IdKind::Sheet),
            ("hud_image", 3, IdKind::Sheet),
            ("picture_show", 1, IdKind::Sheet),
            ("add_stock", 1, IdKind::Item),
            ("teach_spell", 1, IdKind::Spell),
            ("npc_knows_spell", 1, IdKind::Spell),
            ("prepare_spell", 0, IdKind::Spell),
            ("is_prepared", 0, IdKind::Spell),
            ("cast_spell", 0, IdKind::Spell),
            ("set_stock", 1, IdKind::Item),
            ("stock_of", 1, IdKind::Item),
            ("defeated_count", 0, IdKind::Character),
            ("can_craft", 0, IdKind::Recipe),
            ("unlock_achievement", 0, IdKind::Achievement),
            ("achievement_unlocked", 0, IdKind::Achievement),
            ("carry", 1, IdKind::Prop),
            ("set_home", 0, IdKind::Map),
            ("is_home", 0, IdKind::Map),
            ("call_common", 0, IdKind::CommonEvent),
            ("set_class", 1, IdKind::Class),
        ];
        let derived: std::collections::BTreeSet<(&str, usize, u8)> = api_ids().iter().map(|(f, i, k)| (*f, *i, *k as u8)).collect();
        let wanted: std::collections::BTreeSet<(&str, usize, u8)> = expected.iter().map(|(f, i, k)| (*f, *i, *k as u8)).collect();
        assert_eq!(derived, wanted);
        assert!(is_text_arg("say", 0) && is_text_arg("say", 1) && is_text_arg("choice", 0) && is_text_arg("open_crafting", 1));
        assert!(!is_text_arg("choice", 2) && !is_text_arg("give_item", 0) && !is_text_arg("order", 1) && !is_text_arg("disable_spawners", 1));
        assert!(arg_kinds("lockpick", 1).any(|a| a == Arg::Var));
    }
}
