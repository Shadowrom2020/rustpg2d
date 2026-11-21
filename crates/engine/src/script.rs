//! Rhai scripting. Script functions read/modify the world state directly and
//! queue world actions (dialogs, teleports, movement...) which the game then
//! runs through the same [`ActionRunner`](crate::events::runner::ActionRunner)
//! as editor-authored triggers.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, Map, Scope, AST};

use crate::entity::Facing;
use crate::events::{Action, FxSprite, VarOp};
use crate::state::WorldState;

/// A merchant on the current map and what it has: (uid, name, [(item, count)]), unlimited = 999.
pub type MerchantStock = (u32, String, Vec<(String, u32)>);

#[derive(Default)]
struct Ctx {
    world: WorldState,
    actions: Vec<Action>,
    player: PlayerInfo,
    /// The entity whose trigger runs the script (`SELF`).
    owner: Option<u32>,
    /// Characters on the current map that can be hurt: (uid, name, HP).
    characters: Vec<(u32, String, i32)>,
    /// Merchants on the current map and what they have: (uid, name, [(item, count)]).
    merchants: Vec<MerchantStock>,
    /// Characters on the current map that learned spells: (uid, name, spell ids).
    casters: Vec<(u32, String, Vec<String>)>,
    /// Tool items' kinds (`can_craft` with tools).
    tool_kinds: crate::crafting::ToolKinds,
    /// The project's crafting recipes (`can_craft`).
    recipes: Vec<crate::crafting::RecipeDef>,
    /// Ids of the player's unlocked achievements (`achievement_unlocked`).
    achievements: std::collections::BTreeSet<String>,
    /// The project's factions (`standing()` where the save holds none).
    factions: crate::factions::Factions,
    /// Characters on the current map and their factions: (uid, name, faction id) (`faction_of`).
    character_factions: Vec<(u32, String, String)>,
    /// The SRD's attribution when the project uses SRD content (`srd_attribution()`), else "".
    srd_attribution: String,
    /// Characters on the current map with conditions: (uid, name, conditions) (`has_condition`).
    npc_conditions: Vec<(u32, String, Vec<crate::srd::ActiveCondition>)>,
    /// Characters on the current map with an SRD stat block: (uid, name, block) (`npc_stat`).
    stat_blocks: Vec<(u32, String, crate::srd::StatBlock)>,
    /// The enabled plugins' parameters (`plugin_param`).
    plugin_params: std::collections::BTreeMap<String, std::collections::BTreeMap<String, crate::plugins::ParamValue>>,
    /// Props with a lock on the current map: (uid, name, locked) (`is_locked`).
    locks: Vec<(u32, String, bool)>,
    /// Props with object statistics on the current map: (uid, name, hit points) (`prop_hp`).
    objects: Vec<(u32, String, i32)>,
    /// The traps on the current map: (name, state) (`trap_state`).
    traps: Vec<(String, String)>,
    /// Consumable item ids (`is_consumable`).
    consumables: std::collections::BTreeSet<String>,
    /// Magic items' charges by item id (`item_charges`).
    item_charges: std::collections::BTreeMap<String, u32>,
    /// The trigger's event: the used item and the owner's name (`used_item()`, `self_name()`).
    used_item: String,
    self_name: String,
}

/// What scripts can read about the player besides the world state (the game updates it every
/// frame with [`ScriptHost::set_player`]).
#[derive(Clone, Debug, Default)]
pub struct PlayerInfo {
    /// Position in tiles (the feet).
    pub x: f32,
    pub y: f32,
    pub max_hp: i32,
    pub facing: Facing,
    /// The current map's weather (`weather()`).
    pub weather: crate::map::Weather,
    /// Using furniture: "sit", "lie", "stand"; "" on their feet.
    pub pose: String,
    /// The night is passing (the Sleep action).
    pub sleeping: bool,
    /// The player's faction (their race's, else the player character's; "" = none).
    pub faction: String,
    /// Sneaking (`is_sneaking()`).
    pub sneaking: bool,
    /// The hero's armor class (`armor_class()`).
    pub armor_class: i32,
    /// The hero's hit dice left (`hit_dice()`).
    pub hit_dice: u32,
    /// The round of the fight going on (`combat_round()`; 0 = none).
    pub combat_round: u32,
    /// Pounds the hero carries (`carried_weight()`).
    pub carried: f32,
    /// The companions on the map who can act, with their stat blocks (`group_check()`).
    pub party: Vec<(String, crate::srd::StatBlock)>,
    /// The hero's class features at their level (`has_feature()`), chosen options included.
    pub features: Vec<String>,
    /// The hero's class resources: id -> uses left (`resource()`).
    pub resources: Vec<(String, i64)>,
    /// The hero's spell slots by level (index 0 = level 1): most, left.
    pub slots: [(u32, u32); 9],
    /// Prepared spells, what may be prepared and how many (`prepare_spell`, `is_prepared`).
    pub prepared: Vec<String>,
    pub preparable: Vec<String>,
    pub prepare_max: usize,
}

pub struct ScriptHost {
    engine: Engine,
    ast: AST,
    ctx: Rc<RefCell<Ctx>>,
    pub errors: Vec<String>,
    /// Script folders compiled after the project's: the enabled plugins' ([`Self::set_extra_dirs`]).
    extra_dirs: Vec<std::path::PathBuf>,
    /// Text printed by scripts since the last `take_printed` (the dev console shows it).
    printed: Rc<RefCell<Vec<String>>>,
}

/// Rhai operations a script call may take (a runaway loop stops with an error).
const MAX_OPERATIONS: u64 = 2_000_000;
/// Rhai operations one script hook call may take: hooks run inside the engine's own work
/// (every hit, every price), so they must stay short.
pub const HOOK_OPERATIONS: u64 = 200_000;

impl ScriptHost {
    pub fn new() -> Self {
        let ctx = Rc::new(RefCell::new(Ctx::default()));
        let mut engine = Engine::new();
        engine.set_max_operations(MAX_OPERATIONS);
        engine.set_max_call_levels(64);
        let printed: Rc<RefCell<Vec<String>>> = Rc::default();
        let sink = printed.clone();
        engine.on_print(move |s| {
            println!("[script] {s}");
            let mut p = sink.borrow_mut();
            // Only the dev console drains it: keep the newest lines.
            if p.len() >= 200 {
                p.remove(0);
            }
            p.push(s.to_string());
        });
        register_api(&mut engine, &ctx);
        ScriptHost { engine, ast: AST::empty(), ctx, errors: Vec::new(), printed, extra_dirs: Vec::new() }
    }

    /// The player's unlocked achievements, for `achievement_unlocked(id)`.
    pub fn set_achievements(&mut self, ids: std::collections::BTreeSet<String>) {
        self.ctx.borrow_mut().achievements = ids;
    }

    /// The project's crafting recipes, for `can_craft(id)`.
    pub fn set_recipes(&mut self, recipes: Vec<crate::crafting::RecipeDef>, tool_kinds: crate::crafting::ToolKinds) {
        let mut c = self.ctx.borrow_mut();
        c.recipes = recipes;
        c.tool_kinds = tool_kinds;
    }

    /// The project's magic items' charges, for `item_charges(id)`.
    pub fn set_item_charges(&mut self, charges: std::collections::BTreeMap<String, u32>) {
        self.ctx.borrow_mut().item_charges = charges;
    }

    /// The project's consumable items, for `is_consumable(id)`.
    pub fn set_consumables(&mut self, ids: std::collections::BTreeSet<String>) {
        self.ctx.borrow_mut().consumables = ids;
    }

    /// The event of the next call: the used item ("" = none) and the owner's name.
    pub fn set_event(&mut self, item: &str, owner_name: &str) {
        let mut c = self.ctx.borrow_mut();
        c.used_item = item.to_string();
        c.self_name = owner_name.to_string();
    }

    pub fn set_factions(&mut self, factions: crate::factions::Factions) {
        self.ctx.borrow_mut().factions = factions;
    }

    pub fn set_player(&mut self, info: PlayerInfo) {
        self.ctx.borrow_mut().player = info;
    }

    /// The current map's characters that can be hurt, as (uid, name, HP), for `hp_of()`.
    /// The props with a lock on the current map: (uid, name, locked).
    pub fn set_locks(&mut self, locks: Vec<(u32, String, bool)>) {
        self.ctx.borrow_mut().locks = locks;
    }

    /// The traps on the current map: (zone or prop name, "armed" | "found" | "disarmed" | "sprung").
    pub fn set_traps(&mut self, traps: Vec<(String, String)>) {
        self.ctx.borrow_mut().traps = traps;
    }

    /// The props with object statistics on the current map: (uid, name, hit points).
    pub fn set_objects(&mut self, objects: Vec<(u32, String, i32)>) {
        self.ctx.borrow_mut().objects = objects;
    }

    pub fn set_characters(&mut self, characters: Vec<(u32, String, i32)>) {
        self.ctx.borrow_mut().characters = characters;
    }

    /// The enabled plugins' parameter values, for `plugin_param()`.
    pub fn set_plugin_params(&mut self, params: std::collections::BTreeMap<String, std::collections::BTreeMap<String, crate::plugins::ParamValue>>) {
        self.ctx.borrow_mut().plugin_params = params;
    }

    /// Script folders compiled after the project's (the enabled plugins'), also on hot reload.
    pub fn set_extra_dirs(&mut self, dirs: Vec<std::path::PathBuf>) {
        self.extra_dirs = dirs;
    }

    /// The current map's characters and their factions, as (uid, name, faction id), for `faction_of()`.
    pub fn set_character_factions(&mut self, list: Vec<(u32, String, String)>) {
        self.ctx.borrow_mut().character_factions = list;
    }

    /// The SRD's attribution for `srd_attribution()` ("" = the project uses no SRD content).
    pub fn set_srd_attribution(&mut self, text: &str) {
        self.ctx.borrow_mut().srd_attribution = text.to_string();
    }

    /// The current map's characters with conditions, for `has_condition(target, …)`.
    pub fn set_npc_conditions(&mut self, list: Vec<(u32, String, Vec<crate::srd::ActiveCondition>)>) {
        self.ctx.borrow_mut().npc_conditions = list;
    }

    /// The current map's characters with an SRD stat block, for `npc_stat()`.
    pub fn set_stat_blocks(&mut self, list: Vec<(u32, String, crate::srd::StatBlock)>) {
        self.ctx.borrow_mut().stat_blocks = list;
    }

    /// The current map's characters that learned spells, for `npc_knows_spell()`.
    pub fn set_casters(&mut self, casters: Vec<(u32, String, Vec<String>)>) {
        self.ctx.borrow_mut().casters = casters;
    }

    /// The current map's merchants and their stock, for `stock_of()`.
    pub fn set_merchants(&mut self, merchants: Vec<MerchantStock>) {
        self.ctx.borrow_mut().merchants = merchants;
    }

    /// Compiles every `*.rhai` file in `dir` into one module (functions are global). Files
    /// that don't compile are reported in `errors` and left out.
    pub fn load_dir(&mut self, dir: &Path) {
        self.errors.clear();
        let (ast, errors) = self.compile_dir(dir);
        self.ast = ast;
        for e in errors {
            self.report(e);
        }
    }

    /// Hot reload (dev play-tests): compiles the scripts again and uses them if every file
    /// compiles; otherwise the previous scripts stay loaded and the errors are returned.
    /// `Ok` carries the number of script functions.
    pub fn reload_dir(&mut self, dir: &Path) -> Result<usize, Vec<String>> {
        let (ast, errors) = self.compile_dir(dir);
        if !errors.is_empty() {
            for e in &errors {
                eprintln!("[script] error: {e}");
            }
            return Err(errors);
        }
        self.ast = ast;
        self.errors.clear();
        Ok(self.function_names().len())
    }

    fn compile_dir(&self, dir: &Path) -> (AST, Vec<String>) {
        let mut ast = AST::empty();
        let mut errors = Vec::new();
        let files = std::iter::once(dir.to_path_buf()).chain(self.extra_dirs.iter().cloned()).flat_map(|d| crate::project::script_files_in(&d));
        for f in files {
            match crate::sys::read_to_string(&f).map_err(|e| e.to_string()).and_then(|src| self.engine.compile(&src).map_err(|e| e.to_string())) {
                Ok(a) => ast = ast.merge(&a),
                Err(e) => errors.push(format!("{}: {e}", f.file_name().map_or_else(|| f.display().to_string(), |n| n.to_string_lossy().into_owned()))),
            }
        }
        (ast, errors)
    }

    /// Runs a piece of Rhai (the dev console's input) with the project's script functions
    /// available. Returns the value as text ("" for none) or the error, and the world actions
    /// it queued.
    pub fn eval(&mut self, world: &mut WorldState, src: &str) -> (Result<String, String>, Vec<Action>) {
        let snippet = match self.engine.compile(src) {
            Ok(a) => a,
            Err(e) => return (Err(e.to_string()), Vec::new()),
        };
        let ast = self.ast.clone_functions_only().merge(&snippet);
        {
            let mut c = self.ctx.borrow_mut();
            c.world = std::mem::take(world);
            c.actions.clear();
            c.owner = None;
        }
        let mut scope = Scope::new();
        scope.push_constant("SELF", -1_i64);
        let res = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);
        *world = std::mem::take(&mut self.ctx.borrow_mut().world);
        let actions = std::mem::take(&mut self.ctx.borrow_mut().actions);
        (res.map(|v| if v.is_unit() { String::new() } else { v.to_string() }).map_err(|e| e.to_string()), actions)
    }

    /// Lines printed by scripts (`print`) since the last call.
    pub fn take_printed(&mut self) -> Vec<String> {
        std::mem::take(&mut *self.printed.borrow_mut())
    }

    pub fn load_source(&mut self, src: &str) -> Result<(), String> {
        let ast = self.engine.compile(src).map_err(|e| e.to_string())?;
        self.ast = self.ast.merge(&ast);
        Ok(())
    }

    /// Names of all script functions (sorted, deduplicated).
    pub fn function_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.ast.iter_functions().map(|f| f.name.to_string()).collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.ast.iter_functions().any(|f| f.name == name)
    }

    fn report(&mut self, msg: String) {
        eprintln!("[script] error: {msg}");
        self.errors.push(msg);
    }

    fn call_inner(&mut self, world: &mut WorldState, function: &str, args: Vec<Dynamic>, owner: Option<u32>) -> Option<Dynamic> {
        {
            let mut c = self.ctx.borrow_mut();
            c.world = std::mem::take(world);
            c.actions.clear();
            c.owner = owner;
        }
        let mut scope = Scope::new();
        scope.push_constant("SELF", owner.map_or(-1, |u| u as i64));
        let res = self.engine.call_fn::<Dynamic>(&mut scope, &self.ast, function, args);
        *world = std::mem::take(&mut self.ctx.borrow_mut().world);
        match res {
            Ok(v) => Some(v),
            Err(e) => {
                self.report(format!("calling {function}(): {e}"));
                None
            }
        }
    }

    /// Runs a script function; returns the world actions it queued.
    pub fn call(&mut self, world: &mut WorldState, function: &str, owner: Option<u32>) -> Vec<Action> {
        self.call_inner(world, function, Vec::new(), owner);
        std::mem::take(&mut self.ctx.borrow_mut().actions)
    }

    /// Runs a script function with arguments (skill effects: `function(level)`).
    pub fn call_args(&mut self, world: &mut WorldState, function: &str, args: Vec<Dynamic>, owner: Option<u32>) -> Vec<Action> {
        self.call_inner(world, function, args, owner);
        std::mem::take(&mut self.ctx.borrow_mut().actions)
    }

    /// Runs a script hook (`hooks.rs`): `function(map)` under [`HOOK_OPERATIONS`]. Returns the
    /// changed map (None: it returned `()`, failed or returned something else, so the map stays
    /// as it was) and the world actions it queued.
    pub fn call_hook(&mut self, world: &mut WorldState, function: &str, map: rhai::Map) -> (Option<rhai::Map>, Vec<Action>) {
        self.engine.set_max_operations(HOOK_OPERATIONS);
        let r = self.call_inner(world, function, vec![Dynamic::from_map(map)], None);
        self.engine.set_max_operations(MAX_OPERATIONS);
        let actions = std::mem::take(&mut self.ctx.borrow_mut().actions);
        let map = match r {
            Some(v) if v.is_map() => v.try_cast::<rhai::Map>(),
            Some(v) if v.is_unit() => None,
            Some(v) => {
                self.report(format!("hook {function}() returned {} instead of a map or ()", v.type_name()));
                None
            }
            None => None,
        };
        (map, actions)
    }

    /// Runs a script function used as a condition (must return `bool`).
    pub fn condition(&mut self, world: &mut WorldState, function: &str, owner: Option<u32>) -> bool {
        let r = self.call_inner(world, function, Vec::new(), owner);
        self.ctx.borrow_mut().actions.clear();
        r.and_then(|v| v.as_bool().ok()).unwrap_or(false)
    }
}

impl Default for ScriptHost {
    fn default() -> Self {
        Self::new()
    }
}

fn register_api(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    macro_rules! with {
        ($ctx:ident, |$c:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let ctx = $ctx.clone();
            move |$($arg: $ty),*| {
                #[allow(unused_mut)]
                let mut $c = ctx.borrow_mut();
                $body
            }
        }};
    }

    // World state (immediate).
    engine.register_fn("flag", with!(ctx, |c, n: &str| c.world.flag(n)));
    engine.register_fn("set_flag", with!(ctx, |c, n: &str, v: bool| c.world.set_flag(n, v)));
    engine.register_fn("get_var", with!(ctx, |c, n: &str| c.world.var(n)));
    engine.register_fn("set_var", with!(ctx, |c, n: &str, v: i64| c.world.set_var(n, v)));
    engine.register_fn("add_var", with!(ctx, |c, n: &str, v: i64| {
        let cur = c.world.var(n);
        c.world.set_var(n, cur + v)
    }));
    engine.register_fn("item_count", with!(ctx, |c, n: &str| c.world.item_count(n) as i64));
    engine.register_fn("has_item", with!(ctx, |c, n: &str| c.world.item_count(n) > 0));
    // Returns how many did not fit into the inventory.
    engine.register_fn("give_item", with!(ctx, |c, n: &str, k: i64| c.world.give_item(n, k.max(0) as u32) as i64));
    engine.register_fn("take_item", with!(ctx, |c, n: &str, k: i64| c.world.take_item(n, k.max(0) as u32) as i64));
    engine.register_fn("current_map", with!(ctx, |c| c.world.current_map.clone()));
    engine.register_fn("set_home", with!(ctx, |c, m: &str, on: bool| {
        if on {
            c.world.homes.insert(m.to_string());
        } else {
            c.world.homes.remove(m);
        }
    }));
    engine.register_fn("is_home", with!(ctx, |c, m: &str| c.world.homes.contains(m)));
    engine.register_fn("player_hp", with!(ctx, |c| c.world.player_hp.unwrap_or(0) as i64));
    // random(n): integer in 0..n
    engine.register_fn("random", |n: i64| if n <= 0 { 0 } else { macroquad::rand::gen_range(0, n) });

    // Queued world actions.
    engine.register_fn("say", with!(ctx, |c, text: &str| c.actions.push(Action::Dialog {
        speaker: String::new(),
        text: text.into()
    })));
    engine.register_fn("say", with!(ctx, |c, speaker: &str, text: &str| c.actions.push(Action::Dialog {
        speaker: speaker.into(),
        text: text.into()
    })));
    // say(SELF, "...") / say(uid, "..."): the entity's name as speaker (resolved by the game).
    engine.register_fn("say", with!(ctx, |c, entity: i64, text: &str| c.actions.push(Action::Dialog {
        speaker: if entity < 0 { String::new() } else { format!("{{uid:{entity}}}") },
        text: text.into()
    })));
    // say_at("top", ...): the box at the top / middle / bottom of the screen.
    engine.register_fn("say_at", with!(ctx, |c, at: &str, text: &str| c.actions.push(Action::Dialog {
        speaker: String::new(),
        text: crate::dialog_codes::set_box_at(text, crate::dialog_codes::BoxAt::parse(at).unwrap_or_default())
    })));
    engine.register_fn("say_at", with!(ctx, |c, at: &str, speaker: &str, text: &str| c.actions.push(Action::Dialog {
        speaker: speaker.into(),
        text: crate::dialog_codes::set_box_at(text, crate::dialog_codes::BoxAt::parse(at).unwrap_or_default())
    })));
    // say(speaker, "...", #{ mood: "worried" }): the line with a mood portrait of the speaker.
    fn say_with(c: &mut Ctx, speaker: String, text: &str, opts: &rhai::Map) {
        // An emotion over the speaker's head while the line is up (the dialog takes it away).
        let emote = opts.get("emote").map(|m| m.to_string()).unwrap_or_default();
        if !emote.is_empty() {
            c.actions.push(Action::Emote { target: speaker.clone(), emotion: emote, seconds: -1.0, wait: false });
        }
        let mood = opts.get("mood").map(|m| m.to_string()).unwrap_or_default();
        if !mood.is_empty() {
            c.actions.push(Action::Mood { target: speaker.clone(), mood });
        }
        c.actions.push(Action::Dialog { speaker: speaker.clone(), text: text.into() });
        if opts.contains_key("emote") {
            c.actions.push(Action::ClearEmote { target: speaker.clone() });
        }
        if opts.contains_key("mood") {
            c.actions.push(Action::Mood { target: speaker, mood: String::new() });
        }
    }
    engine.register_fn("say", with!(ctx, |c, speaker: &str, text: &str, opts: rhai::Map| say_with(&mut c, speaker.into(), text, &opts)));
    engine.register_fn("say", with!(ctx, |c, entity: i64, text: &str, opts: rhai::Map| say_with(&mut c, if entity < 0 { String::new() } else { format!("{{uid:{entity}}}") }, text, &opts)));
    // Move routes (queued).
    fn route_of(steps: &rhai::Array) -> Vec<crate::routes::RouteStep> {
        steps
            .iter()
            .filter_map(|s| {
                let t = s.to_string();
                let step = crate::routes::RouteStep::parse(&t);
                if step.is_none() {
                    eprintln!("[script] move_route: '{t}' is not a step (move up 2, toward 1, away 1, turn left, turn random, face player, wait 0.5, jump 1,0, speed 2, anim wave 1, sound blip, through on, script name)");
                }
                step
            })
            .collect()
    }
    engine.register_fn("move_route", with!(ctx, |c, target: &str, steps: rhai::Array| c.actions.push(Action::MoveRoute { target: target.into(), steps: route_of(&steps), repeat: false, skip_blocked: true, wait: true })));
    engine.register_fn("move_route", with!(ctx, |c, target: &str, steps: rhai::Array, o: rhai::Map| {
        let b = |k: &str, d: bool| o.get(k).and_then(|v| v.as_bool().ok()).unwrap_or(d);
        c.actions.push(Action::MoveRoute { target: target.into(), steps: route_of(&steps), repeat: b("repeat", false), skip_blocked: b("skip", true), wait: b("wait", true) })
    }));
    // Text variables (immediately) and asking the player (queued).
    engine.register_fn("get_text", with!(ctx, |c, name: &str| c.world.text(name).to_string()));
    engine.register_fn("set_text", with!(ctx, |c, name: &str, value: &str| {
        c.world.texts.insert(name.to_string(), value.to_string());
    }));
    engine.register_fn("input_number", with!(ctx, |c, text: &str, var: &str, min: Dynamic, max: Dynamic| c.actions.push(Action::InputNumber {
        text: text.into(),
        var: var.into(),
        digits: 0,
        min: num(&min) as i64,
        max: num(&max) as i64,
    })));
    engine.register_fn("input_text", with!(ctx, |c, text: &str, var: &str, max_len: Dynamic| c.actions.push(Action::InputText { text: text.into(), var: var.into(), max_len: num(&max_len).max(1.0) as u32 })));
    // Timers (immediately).
    engine.register_fn("timer_start", with!(ctx, |c, name: &str, secs: Dynamic| {
        c.world.timers.insert(name.to_string(), crate::timers::Timer::new(num(&secs), false, "", ""));
    }));
    engine.register_fn("timer_start", with!(ctx, |c, name: &str, secs: Dynamic, o: rhai::Map| {
        let s = |k: &str| o.get(k).map(|v| v.to_string()).unwrap_or_default();
        let shown = o.get("shown").and_then(|v| v.as_bool().ok()).unwrap_or(false);
        c.world.timers.insert(name.to_string(), crate::timers::Timer::new(num(&secs), shown, &s("run"), &s("event")));
    }));
    engine.register_fn("timer_left", with!(ctx, |c, name: &str| c.world.timers.get(name).map_or(0.0, |t| t.left as rhai::FLOAT)));
    engine.register_fn("call_common", with!(ctx, |c, id: &str| c.actions.push(Action::CallCommon { id: id.to_string() })));
    engine.register_fn("timer_stop", with!(ctx, |c, name: &str| {
        c.world.timers.remove(name);
    }));
    // Screen tint, flash and fades (queued).
    fn rgba_of(v: &Dynamic, alpha: u8) -> [u8; 4] {
        if let Some(a) = v.clone().try_cast::<rhai::Array>() {
            let c = |k: usize, d: f32| a.get(k).map_or(d, num).clamp(0.0, 255.0) as u8;
            return [c(0, 255.0), c(1, 255.0), c(2, 255.0), c(3, alpha as f32)];
        }
        let s = v.to_string();
        if let Some(c) = crate::skin::SkinColor::parse(&s).filter(|_| s.trim().starts_with('#')) {
            return if s.trim().len() == 9 { c.0 } else { [c.0[0], c.0[1], c.0[2], alpha] };
        }
        crate::dialog_codes::color(&s).map_or([0, 0, 0, alpha], |[r, g, b]| [r, g, b, alpha])
    }
    engine.register_fn("tint_screen", with!(ctx, |c, color: Dynamic, secs: Dynamic| c.actions.push(Action::TintScreen { color: rgba_of(&color, 90), seconds: num(&secs), wait: false })));
    engine.register_fn("clear_tint", with!(ctx, |c, secs: Dynamic| c.actions.push(Action::TintScreen { color: [0, 0, 0, 0], seconds: num(&secs), wait: false })));
    engine.register_fn("flash_screen", with!(ctx, |c, color: Dynamic, secs: Dynamic| c.actions.push(Action::FlashScreen { color: rgba_of(&color, 200), seconds: num(&secs) })));
    engine.register_fn("fade_out", with!(ctx, |c, secs: Dynamic| c.actions.push(Action::FadeOut { seconds: num(&secs), color: [0, 0, 0], wait: true })));
    engine.register_fn("fade_out", with!(ctx, |c, secs: Dynamic, color: Dynamic| {
        let [r, g, b, _] = rgba_of(&color, 255);
        c.actions.push(Action::FadeOut { seconds: num(&secs), color: [r, g, b], wait: true })
    }));
    engine.register_fn("fade_in", with!(ctx, |c, secs: Dynamic| c.actions.push(Action::FadeIn { seconds: num(&secs), wait: true })));
    // The camera and scrolling text (queued).
    fn cam(c: &mut Ctx, aim: crate::camera::CameraAim, o: &rhai::Map, seconds: f32) {
        let f = |k: &str, d: f32| o.get(k).map_or(d, num);
        c.actions.push(Action::Camera {
            aim,
            seconds: f("seconds", seconds),
            easing: o.get("easing").and_then(|e| crate::pictures::Easing::parse(&e.to_string())).unwrap_or(crate::pictures::Easing::InOut),
            zoom: f("zoom", 1.0),
            wait: o.get("wait").and_then(|v| v.as_bool().ok()).unwrap_or(true),
        });
    }
    let tile_aim = |x: &Dynamic, y: &Dynamic| crate::camera::CameraAim::Tile { x: num(x).floor() as i32, y: num(y).floor() as i32 };
    engine.register_fn("camera_to", with!(ctx, |c, x: Dynamic, y: Dynamic| cam(&mut c, tile_aim(&x, &y), &rhai::Map::new(), 1.0)));
    engine.register_fn("camera_to", with!(ctx, |c, x: Dynamic, y: Dynamic, o: rhai::Map| cam(&mut c, tile_aim(&x, &y), &o, 1.0)));
    engine.register_fn("camera_follow", with!(ctx, |c, target: &str| cam(&mut c, crate::camera::CameraAim::Follow { target: target.into() }, &rhai::Map::new(), 1.0)));
    engine.register_fn("camera_follow", with!(ctx, |c, target: &str, o: rhai::Map| cam(&mut c, crate::camera::CameraAim::Follow { target: target.into() }, &o, 1.0)));
    engine.register_fn("camera_reset", with!(ctx, |c| cam(&mut c, crate::camera::CameraAim::Player, &rhai::Map::new(), 0.8)));
    engine.register_fn("camera_reset", with!(ctx, |c, secs: Dynamic| cam(&mut c, crate::camera::CameraAim::Player, &rhai::Map::new(), num(&secs))));
    fn scroll(c: &mut Ctx, text: &str, o: &rhai::Map) {
        c.actions.push(Action::ScrollText {
            text: text.to_string(),
            speed: o.get("speed").map_or(40.0, num),
            background: o.get("background").map_or([0, 0, 0, 255], |v| rgba_of(v, 255)),
        });
    }
    engine.register_fn("scroll_text", with!(ctx, |c, text: &str| scroll(&mut c, text, &rhai::Map::new())));
    engine.register_fn("scroll_text", with!(ctx, |c, text: &str, o: rhai::Map| scroll(&mut c, text, &o)));
    // Picture layers (queued).
    fn pic_tint(v: &Dynamic) -> Option<[u8; 3]> {
        if let Some(a) = v.clone().try_cast::<rhai::Array>() {
            let c = |k: usize| a.get(k).map_or(255.0, num).clamp(0.0, 255.0) as u8;
            return Some([c(0), c(1), c(2)]);
        }
        crate::dialog_codes::color(&v.to_string())
    }
    fn pic_show(c: &mut Ctx, layer: &Dynamic, sheet: &str, x: &Dynamic, y: &Dynamic, o: &rhai::Map) {
        let f = |k: &str, d: f32| o.get(k).map_or(d, num);
        c.actions.push(Action::ShowPicture {
            layer: num(layer).max(0.0) as u32,
            sheet: sheet.to_string(),
            frame: f("frame", 0.0).max(0.0) as u32,
            anchor: o.get("anchor").and_then(|a| crate::script_ui::Anchor::parse(&a.to_string())).unwrap_or_default(),
            on_map: o.get("on_map").and_then(|v| v.as_bool().ok()).unwrap_or(false),
            x: num(x),
            y: num(y),
            scale: f("scale", 1.0),
            opacity: f("opacity", 1.0),
            rotation: f("rotation", 0.0),
            tint: o.get("tint").and_then(pic_tint).unwrap_or([255, 255, 255]),
            blend: if o.get("blend").is_some_and(|b| b.to_string().starts_with("add")) { crate::pictures::Blend::Additive } else { crate::pictures::Blend::Normal },
        });
    }
    engine.register_fn("picture_show", with!(ctx, |c, layer: Dynamic, sheet: &str, x: Dynamic, y: Dynamic| pic_show(&mut c, &layer, sheet, &x, &y, &rhai::Map::new())));
    engine.register_fn("picture_show", with!(ctx, |c, layer: Dynamic, sheet: &str, x: Dynamic, y: Dynamic, o: rhai::Map| pic_show(&mut c, &layer, sheet, &x, &y, &o)));
    engine.register_fn("picture_move", with!(ctx, |c, layer: Dynamic, o: rhai::Map| {
        let f = |k: &str| o.get(k).map(num);
        let to = crate::pictures::PictureTargets { x: f("x"), y: f("y"), scale: f("scale"), opacity: f("opacity"), rotation: f("rotation"), tint: o.get("tint").and_then(pic_tint) };
        c.actions.push(Action::MovePicture {
            layer: num(&layer).max(0.0) as u32,
            to,
            seconds: f("seconds").unwrap_or(1.0),
            easing: o.get("easing").and_then(|e| crate::pictures::Easing::parse(&e.to_string())).unwrap_or_default(),
            wait: o.get("wait").and_then(|v| v.as_bool().ok()).unwrap_or(false),
        });
    }));
    engine.register_fn("picture_erase", with!(ctx, |c, layer: Dynamic| c.actions.push(Action::ErasePicture { layer: num(&layer).max(0.0) as u32 })));
    // Script UI: HUD elements (saved with the game) and windows.
    fn hud_opts(e: &mut crate::script_ui::HudElement, opts: &rhai::Map) {
        if let Some(a) = opts.get("anchor").and_then(|a| crate::script_ui::Anchor::parse(&a.to_string())) {
            e.anchor = a;
        }
        if let Some(v) = opts.get("size").or(opts.get("scale")) {
            e.size = num(v);
        }
        if let Some(v) = opts.get("color").or(opts.get("colour")) {
            e.color = v.to_string();
        }
        if let (Some(l), crate::script_ui::HudKind::Bar { label, .. }) = (opts.get("label"), &mut e.kind) {
            *label = l.to_string();
        }
    }
    fn hud_set(c: &mut Ctx, id: &str, kind: crate::script_ui::HudKind, x: &Dynamic, y: &Dynamic, opts: &rhai::Map) {
        let mut e = crate::script_ui::HudElement::new(kind, num(x), num(y));
        hud_opts(&mut e, opts);
        c.world.hud.insert(id.to_string(), e);
    }
    let text_of = |t: &str| crate::script_ui::HudKind::Text { text: t.to_string() };
    engine.register_fn("hud_text", with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, t: &str| hud_set(&mut c, id, text_of(t), &x, &y, &rhai::Map::new())));
    engine.register_fn("hud_text", with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, t: &str, o: rhai::Map| hud_set(&mut c, id, text_of(t), &x, &y, &o)));
    let image_of = |sheet: &str, frame: &Dynamic| crate::script_ui::HudKind::Image { sheet: sheet.to_string(), frame: num(frame).max(0.0) as u32 };
    engine.register_fn("hud_image", with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, sheet: &str, f: Dynamic| hud_set(&mut c, id, image_of(sheet, &f), &x, &y, &rhai::Map::new())));
    engine.register_fn("hud_image", with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, sheet: &str, f: Dynamic, o: rhai::Map| hud_set(&mut c, id, image_of(sheet, &f), &x, &y, &o)));
    // A minimap: `size` pixels square; options #{ anchor, tiles } (tiles across).
    let minimap_of = |o: &rhai::Map| crate::script_ui::HudKind::Minimap { tiles: o.get("tiles").map_or(0.0, num).max(0.0) };
    engine.register_fn("hud_minimap", with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, size: Dynamic| {
        let o: rhai::Map = [("size".into(), size)].into_iter().collect();
        hud_set(&mut c, id, minimap_of(&o), &x, &y, &o)
    }));
    engine.register_fn("hud_minimap", with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, size: Dynamic, o: rhai::Map| {
        let mut o = o;
        o.insert("size".into(), size);
        hud_set(&mut c, id, minimap_of(&o), &x, &y, &o)
    }));
    let bar_of = |w: &Dynamic, v: &Dynamic, m: &Dynamic| crate::script_ui::HudKind::Bar { value: v.to_string(), max: m.to_string(), w: num(w).max(1.0), label: String::new() };
    engine.register_fn("hud_bar",
        with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, w: Dynamic, v: Dynamic, m: Dynamic, col: &str| {
            let o: rhai::Map = [("color".into(), Dynamic::from(col.to_string()))].into_iter().collect();
            hud_set(&mut c, id, bar_of(&w, &v, &m), &x, &y, &o)
        }),
    );
    engine.register_fn("hud_bar",
        with!(ctx, |c, id: &str, x: Dynamic, y: Dynamic, w: Dynamic, v: Dynamic, m: Dynamic, col: &str, o: rhai::Map| {
            let mut o = o;
            o.entry("color".into()).or_insert_with(|| Dynamic::from(col.to_string()));
            hud_set(&mut c, id, bar_of(&w, &v, &m), &x, &y, &o)
        }),
    );
    engine.register_fn("hud_remove", with!(ctx, |c, id: &str| {
        c.world.hud.remove(id);
    }));
    engine.register_fn("hud_clear", with!(ctx, |c| c.world.hud.clear()));
    engine.register_fn("window_open", with!(ctx, |c, id: &str, title: &str, rows: rhai::Array| {
        use crate::script_ui::{ScriptWindow, WindowRequest, WindowRow};
        let rows = rows
            .into_iter()
            .map(|r| match r.clone().try_cast::<rhai::Map>() {
                Some(m) if m.contains_key("button") => WindowRow::Button { label: m["button"].to_string(), call: m.get("run").or(m.get("call")).map(|f| f.to_string()).unwrap_or_default() },
                Some(m) if m.contains_key("image") => WindowRow::Image { sheet: m["image"].to_string(), frame: m.get("frame").map_or(0.0, num).max(0.0) as u32 },
                Some(m) => WindowRow::Text(m.get("text").map(|t| t.to_string()).unwrap_or_default()),
                None => WindowRow::Text(r.to_string()),
            })
            .collect();
        c.world.window_request = Some(WindowRequest::Open(ScriptWindow { id: id.to_string(), title: title.to_string(), rows }));
    }));
    engine.register_fn("window_close", with!(ctx, |c| c.world.window_request = Some(crate::script_ui::WindowRequest::Close)));
    // emote(target, "joy"[, #{ seconds, wait }]) / clear_emote(target): an emotion over the head.
    engine.register_fn("emote", with!(ctx, |c, t: &str, e: &str| c.actions.push(Action::Emote { target: t.into(), emotion: e.into(), seconds: 0.0, wait: false })));
    engine.register_fn("used_item", with!(ctx, |c| c.used_item.clone()));
    engine.register_fn("self_name", with!(ctx, |c| c.self_name.clone()));
    engine.register_fn("is_consumable", with!(ctx, |c, item: &str| c.consumables.contains(item)));
    // Magic items (immediate).
    engine.register_fn("attuned", with!(ctx, |c, item: &str| c.world.attuned.iter().any(|a| a == item)));
    engine.register_fn("knocked_out", with!(ctx, |c, name: &str| c.world.was_knocked_out(name)));
    engine.register_fn("attune", with!(ctx, |c, item: &str| {
        let w = &mut c.world;
        let ok = w.item_count(item) > 0 && !w.attuned.iter().any(|a| a == item) && w.attuned.len() < crate::srd::magic_items::MAX_ATTUNED;
        if ok {
            w.attuned.push(item.to_string());
        }
        ok
    }));
    engine.register_fn("end_attunement", with!(ctx, |c, item: &str| {
        let before = c.world.attuned.len();
        c.world.attuned.retain(|a| a != item);
        c.world.attuned.len() < before
    }));
    engine.register_fn("item_charges", with!(ctx, |c, item: &str| {
        let max = c.item_charges.get(item).copied().unwrap_or(0);
        match c.world.stack_of(item) {
            Some(s) if max > 0 => max.saturating_sub(s.spent) as i64,
            _ => -1,
        }
    }));
    engine.register_fn("set_item_charges", with!(ctx, |c, item: &str, n: i64| {
        let max = c.item_charges.get(item).copied().unwrap_or(0);
        match c.world.stack_of_mut(item) {
            Some(s) if max > 0 => {
                s.spent = max.saturating_sub((n.max(0) as u32).min(max));
                true
            }
            _ => false,
        }
    }));
    engine.register_fn("rush_to", with!(ctx, |c, t: &str, to: &str| c.actions.push(Action::RushTo { target: t.into(), to: to.into() })));
    engine.register_fn("bark", with!(ctx, |c, t: &str, text: &str| c.actions.push(Action::Bark { target: t.into(), lines: vec![text.into()], seconds: 0.0 })));
    engine.register_fn("bark", with!(ctx, |c, t: &str, text: &str, seconds: f64| c.actions.push(Action::Bark { target: t.into(), lines: vec![text.into()], seconds: seconds as f32 })));
    engine.register_fn("bark", with!(ctx, |c, t: &str, text: &str, seconds: i64| c.actions.push(Action::Bark { target: t.into(), lines: vec![text.into()], seconds: seconds as f32 })));
    engine.register_fn("emote", with!(ctx, |c, t: &str, e: &str, opts: rhai::Map| {
        let seconds = opts.get("seconds").and_then(|v| v.as_float().ok().or_else(|| v.as_int().ok().map(|i| i as f64))).unwrap_or(0.0) as f32;
        let wait = opts.get("wait").and_then(|v| v.as_bool().ok()).unwrap_or(false);
        c.actions.push(Action::Emote { target: t.into(), emotion: e.into(), seconds, wait })
    }));
    engine.register_fn("clear_emote", with!(ctx, |c, t: &str| c.actions.push(Action::ClearEmote { target: t.into() })));
    // mood(name, "worried"): the character's portrait shows that mood until changed ("" = usual).
    engine.register_fn("mood", with!(ctx, |c, t: &str, m: &str| c.actions.push(Action::Mood { target: t.into(), mood: m.into() })));
    engine.register_fn("choice", with!(ctx, |c, text: &str, options: Array, var: &str| {
        let options = options.into_iter().map(|o| o.to_string()).collect();
        c.actions.push(Action::Choice { speaker: String::new(), text: text.into(), options, var: var.into() })
    }));
    engine.register_fn("teleport", with!(ctx, |c, map: &str, spawn: &str| c.actions.push(Action::Teleport {
        map: map.into(),
        spawn: spawn.into()
    })));
    engine.register_fn("set_active", with!(ctx, |c, t: &str, a: bool| c.actions.push(Action::SetActive {
        target: t.into(),
        active: a
    })));
    engine.register_fn("set_frame", with!(ctx, |c, t: &str, f: i64| c.actions.push(Action::SetFrame {
        target: t.into(),
        frame: f.max(0) as u32
    })));
    engine.register_fn("player_name", with!(ctx, |c| c.world.player_name.clone()));
    engine.register_fn("hour", with!(ctx, |c| c.world.hour as f64));
    // The SRD rules: dice, abilities, checks (they roll at once, into the roll log).
    engine.register_fn("roll", with!(ctx, |c, dice: &str| c.world.roll_dice(dice).map_or(0, |r| r.total)));
    // Checks outside fights: passive values, group checks, contests.
    engine.register_fn("search_roll", with!(ctx, |c| c.world.last_search.unwrap_or(-1)));
    engine.register_fn("passive", with!(ctx, |c, what: &str| c.world.passive(what).unwrap_or(-1)));
    engine.register_fn("group_check", with!(ctx, |c, what: &str, dc: i64| {
        let party = c.player.party.clone();
        c.world.group_check(what, dc as i32, &party)
    }));
    fn contest_with(c: &mut Ctx, what: &str, name: &str, against: &str) -> bool {
        let who = match name {
            "" | "self" => c.owner.and_then(|u| c.stat_blocks.iter().find(|s| s.0 == u)),
            n => c.stat_blocks.iter().find(|s| s.1 == n),
        };
        let Some((_, n, block)) = who.cloned() else {
            eprintln!("[script] contest: no character '{name}' with a stat block on this map");
            return false;
        };
        c.world.contest(what, &n, &block, against)
    }
    engine.register_fn("contest", with!(ctx, |c, what: &str, name: &str| contest_with(&mut c, what, name, "")));
    engine.register_fn("contest", with!(ctx, |c, what: &str, name: &str, against: &str| contest_with(&mut c, what, name, against)));
    engine.register_fn("check", with!(ctx, |c, what: &str, dc: i64| c.world.check(what, dc as i32, crate::srd::Mode::Normal, false).is_some_and(|r| r.success == Some(true))));
    engine.register_fn("check", with!(ctx, |c, what: &str, dc: i64, mode: &str| c.world.check(what, dc as i32, crate::srd::Mode::parse(mode), false).is_some_and(|r| r.success == Some(true))));
    engine.register_fn("save", with!(ctx, |c, ability: &str, dc: i64| c.world.check(ability, dc as i32, crate::srd::Mode::Normal, true).is_some_and(|r| r.success == Some(true))));
    engine.register_fn("save", with!(ctx, |c, ability: &str, dc: i64, mode: &str| c.world.check(ability, dc as i32, crate::srd::Mode::parse(mode), true).is_some_and(|r| r.success == Some(true))));
    engine.register_fn("modifier", with!(ctx, |c, what: &str| {
        let w = what.trim();
        match w.strip_prefix("save:") {
            Some(a) => c.world.check_bonus(a, true),
            None => c.world.check_bonus(w, false),
        }
        .unwrap_or_else(|| {
            eprintln!("[script] modifier: '{what}' is no ability, skill or tool");
            0
        })
    }));
    engine.register_fn("ability", with!(ctx, |c, name: &str| crate::srd::Ability::parse(name).map_or(0, |a| c.world.abilities.score(a) as i64)));
    engine.register_fn("set_ability", with!(ctx, |c, name: &str, score: i64| match crate::srd::Ability::parse(name) {
        Some(a) => c.world.base_abilities_mut().set(a, score.clamp(1, 30) as i32),
        None => eprintln!("[script] set_ability: '{name}' is no ability (str, dex, con, int, wis, cha)"),
    }));
    engine.register_fn("proficiency", with!(ctx, |c| crate::srd::abilities::proficiency_bonus(c.world.player_level()) as i64));
    engine.register_fn("is_proficient", with!(ctx, |c, what: &str| c.world.is_proficient(what)));
    engine.register_fn("set_proficient", with!(ctx, |c, what: &str, on: bool| c.world.set_proficient(what, on)));
    // Until 0.27 a skill tree's level; now 1 when the hero is proficient (deprecated).
    engine.register_fn("skill", with!(ctx, |c, id: &str| c.world.is_proficient(id) as i64));
    // Magic.
    engine.register_fn("knows_spell", with!(ctx, |c, id: &str| c.world.spells.contains(id) || c.player.prepared.iter().any(|s| s == id)));
    engine.register_fn("mana", with!(ctx, |_c| 0.0f64));
    // Money in copper pieces (every coin item counts by its worth; change comes back).
    engine.register_fn("carried_weight", with!(ctx, |c| c.player.carried as f64));
    engine.register_fn("money", with!(ctx, |c| c.world.money_cp() as i64));
    engine.register_fn("give_money", with!(ctx, |c, cp: i64| {
        c.world.give_cp(cp.max(0) as u32);
    }));
    engine.register_fn("take_money", with!(ctx, |c, cp: i64| c.world.pay_cp(cp.max(0) as u32)));
    engine.register_fn("eat", with!(ctx, |c, days: Dynamic| c.actions.push(Action::Nourish { food: num(&days).clamp(0.0, 100.0), water: 0.0 })));
    engine.register_fn("drink", with!(ctx, |c, days: Dynamic| c.actions.push(Action::Nourish { food: 0.0, water: num(&days).clamp(0.0, 100.0) })));
    engine.register_fn("fall", with!(ctx, |c, feet: i64| c.actions.push(Action::Fall { feet: feet.clamp(0, 10_000) as u32 })));
    engine.register_fn("format_money", |cp: i64| crate::srd::equipment::format_cp(cp.max(0) as u32, [&crate::lang::tr_ui("gp"), &crate::lang::tr_ui("sp"), &crate::lang::tr_ui("cp")]));
    engine.register_fn("spell_slots", with!(ctx, |c, l: Dynamic| c.player.slots.get((num(&l) as usize).wrapping_sub(1)).map_or(0, |s| s.0 as i64)));
    engine.register_fn("slots_left", with!(ctx, |c, l: Dynamic| c.player.slots.get((num(&l) as usize).wrapping_sub(1)).map_or(0, |s| s.1 as i64)));
    engine.register_fn("restore_slots", with!(ctx, |c| {
        c.world.slots_spent = [0; 9];
        for s in &mut c.player.slots {
            s.1 = s.0;
        }
    }));
    engine.register_fn("is_prepared", with!(ctx, |c, id: &str| c.player.prepared.iter().any(|s| s == id)));
    engine.register_fn("prepare_spell", with!(ctx, |c, id: &str| {
        if c.player.prepared.iter().any(|s| s == id) {
            return true;
        }
        if !c.player.preparable.iter().any(|s| s == id) || c.player.prepared.len() >= c.player.prepare_max {
            return false;
        }
        c.player.prepared.push(id.to_string());
        c.world.prepared = c.player.prepared.clone();
        true
    }));
    engine.register_fn("concentrating", with!(ctx, |c| c.world.concentration.as_ref().map_or(String::new(), |x| x.0.split('@').next().unwrap_or("").to_string())));
    engine.register_fn("cast_spell", with!(ctx, |c, id: &str| c.actions.push(Action::CastSpell { spell: id.into(), target: "player".into(), free: false })));
    engine.register_fn("cast_spell", with!(ctx, |c, id: &str, t: &str, free: bool| c.actions.push(Action::CastSpell { spell: id.into(), target: t.into(), free })));
    // Experience and levels.
    engine.register_fn("xp", with!(ctx, |c| c.world.xp.min(i64::MAX as u64) as i64));
    engine.register_fn("level", with!(ctx, |c| c.world.player_level() as i64));
    engine.register_fn("give_xp", with!(ctx, |c, n: i64| c.actions.push(Action::GiveXp { amount: n })));
    // Characters' HP: "player", "self", a name, or an entity uid (SELF); -1 = no such character.
    engine.register_fn("hp_of", with!(ctx, |c, t: &str| match t {
        "player" => c.world.player_hp.unwrap_or(0) as i64,
        "" | "self" => c.owner.and_then(|u| c.characters.iter().find(|ch| ch.0 == u)).map_or(-1, |ch| ch.2 as i64),
        name => c.characters.iter().find(|ch| ch.1 == name).map_or(-1, |ch| ch.2 as i64),
    }));
    engine.register_fn("hp_of", with!(ctx, |c, uid: i64| c.characters.iter().find(|ch| ch.0 as i64 == uid).map_or(-1, |ch| ch.2 as i64)));
    // Merchants' stock (what villagers produce reaches their stall), and the day number for
    // "once a day" rules.
    engine.register_fn("add_stock", with!(ctx, |c, t: &str, item: &str, n: i64| c.actions.push(Action::ShopStock {
        target: t.into(),
        item: item.into(),
        op: crate::events::VarOp::Add,
        count: n.max(0) as u32,
        price: 0
    })));
    engine.register_fn("set_stock", with!(ctx, |c, t: &str, item: &str, n: i64| c.actions.push(Action::ShopStock {
        target: t.into(),
        item: item.into(),
        op: crate::events::VarOp::Set,
        count: n.max(0) as u32,
        price: 0
    })));
    engine.register_fn("stock_of", with!(ctx, |c, t: &str, item: &str| {
        let m = match t {
            "" | "self" => c.owner.and_then(|u| c.merchants.iter().find(|m| m.0 == u)),
            name => c.merchants.iter().find(|m| m.1 == name),
        };
        m.map_or(0, |m| m.2.iter().filter(|(i, _)| i == item).map(|(_, n)| *n as i64).sum())
    }));
    engine.register_fn("day", with!(ctx, |c| c.world.day as i64));
    // Characters' spells (companions cast Heal and Bolt from their mana).
    engine.register_fn("teach_spell", with!(ctx, |c, t: &str, spell: &str| c.actions.push(Action::TeachSpell { target: t.into(), spell: spell.into() })));
    engine.register_fn("npc_knows_spell", with!(ctx, |c, t: &str, spell: &str| {
        let who = match t {
            "" | "self" => c.owner.and_then(|u| c.casters.iter().find(|m| m.0 == u)),
            name => c.casters.iter().find(|m| m.1 == name),
        };
        who.is_some_and(|m| m.2.iter().any(|s| s == spell))
    }));
    // Bestiary: how many characters of a definition the player has defeated.
    engine.register_fn("defeated_count", with!(ctx, |c, id: &str| c.world.defeated(id) as i64));
    // Screen shake (scenes: earthquakes, a boss landing).
    engine.register_fn("shake", with!(ctx, |c, s: Dynamic| c.actions.push(Action::Shake { seconds: num(&s), strength: 1.0 })));
    engine.register_fn("shake", with!(ctx, |c, s: Dynamic, st: Dynamic| c.actions.push(Action::Shake { seconds: num(&s), strength: num(&st) })));
    // Weather: set_weather("rain" | "snow" | "fog" | "none" | "default", [intensity]) on the
    // current map, set_map_weather(map, kind, intensity) on another one; weather() reads it.
    engine.register_fn("set_weather", with!(ctx, |c, kind: &str| c.actions.push(weather_action("", kind, 0.6))));
    engine.register_fn("set_weather", with!(ctx, |c, kind: &str, i: Dynamic| c.actions.push(weather_action("", kind, num(&i)))));
    engine.register_fn("set_map_weather", with!(ctx, |c, map: &str, kind: &str, i: Dynamic| c.actions.push(weather_action(map, kind, num(&i)))));
    engine.register_fn("weather", with!(ctx, |c| c.player.weather.kind.id().to_string()));
    engine.register_fn("weather_intensity", with!(ctx, |c| c.player.weather.intensity as f64));
    // Conditions (SRD); the status effect functions of 0.27 map onto them.
    engine.register_fn("apply_condition", with!(ctx, |c, t: &str, cond: &str, secs: Dynamic| {
        if let Some(k) = condition(cond) {
            c.actions.push(Action::ApplyEffect { target: t.into(), effect: crate::srd::ConditionSpec::new(k, num(&secs)) });
        }
    }));
    engine.register_fn("apply_condition", with!(ctx, |c, t: &str, cond: &str, secs: Dynamic, o: Map| {
        let k = if cond.trim().is_empty() { Some(crate::srd::Condition::Any) } else { condition(cond) };
        if let Some(k) = k {
            c.actions.push(Action::ApplyEffect { target: t.into(), effect: condition_spec(k, num(&secs), &o) });
        }
    }));
    engine.register_fn("remove_condition", with!(ctx, |c, t: &str, cond: &str| {
        let k = if cond.trim().is_empty() { Some(crate::srd::Condition::Any) } else { condition(cond) };
        if let Some(k) = k {
            c.actions.push(Action::CureEffect { target: t.into(), kind: Some(k) });
        }
    }));
    engine.register_fn("has_condition", with!(ctx, |c, cond: &str| condition(cond).is_some_and(|k| crate::srd::conditions::has(&c.world.effects, k))));
    engine.register_fn("has_condition", with!(ctx, |c, t: &str, cond: &str| {
        let Some(k) = condition(cond) else { return false };
        match t {
            "player" => crate::srd::conditions::has(&c.world.effects, k),
            "" | "self" => c.owner.and_then(|u| c.npc_conditions.iter().find(|n| n.0 == u)).is_some_and(|n| crate::srd::conditions::has(&n.2, k)),
            name => c.npc_conditions.iter().find(|n| n.1 == name).is_some_and(|n| crate::srd::conditions::has(&n.2, k)),
        }
    }));
    engine.register_fn("has_condition", with!(ctx, |c, uid: i64, cond: &str| {
        let Some(k) = condition(cond) else { return false };
        c.npc_conditions.iter().find(|n| n.0 as i64 == uid).is_some_and(|n| crate::srd::conditions::has(&n.2, k))
    }));
    engine.register_fn("srd_attribution", with!(ctx, |c| c.srd_attribution.clone()));
    engine.register_fn("exhaustion", with!(ctx, |c| crate::srd::conditions::exhaustion(&c.world.effects) as i64));
    engine.register_fn("set_exhaustion", with!(ctx, |c, n: i64| crate::srd::conditions::set_exhaustion(&mut c.world.effects, n.clamp(0, 6) as u8)));
    engine.register_fn("short_rest", with!(ctx, |c| c.actions.push(Action::ShortRest)));
    engine.register_fn("start_combat", with!(ctx, |c| c.actions.push(Action::StartCombat)));
    engine.register_fn("in_combat", with!(ctx, |c| c.player.combat_round > 0));
    engine.register_fn("combat_round", with!(ctx, |c| c.player.combat_round as i64));
    engine.register_fn("long_rest", with!(ctx, |c| c.actions.push(Action::LongRest { hours: 8.0 })));
    engine.register_fn("long_rest", with!(ctx, |c, h: Dynamic| c.actions.push(Action::LongRest { hours: num(&h).max(0.0) })));
    engine.register_fn("apply_effect", with!(ctx, |c, t: &str, kind: &str, s: Dynamic, secs: Dynamic| {
        match crate::srd::conditions::from_effect(kind, num(&s), num(&secs)) {
            Some(spec) => c.actions.push(Action::ApplyEffect { target: t.into(), effect: spec }),
            None => eprintln!("[script] unknown effect '{kind}'"),
        }
    }));
    engine.register_fn("cure", with!(ctx, |c, t: &str, kind: &str| {
        if let Some(k) = condition(kind) {
            c.actions.push(Action::CureEffect { target: t.into(), kind: Some(k) });
        }
    }));
    engine.register_fn("cure", with!(ctx, |c, t: &str| c.actions.push(Action::CureEffect { target: t.into(), kind: None })));
    engine.register_fn("has_effect", with!(ctx, |c, kind: &str| condition(kind).is_some_and(|k| crate::srd::conditions::has(&c.world.effects, k))));
    engine.register_fn("set_skill", with!(ctx, |c, id: &str, l: i64| c.actions.push(Action::SetSkill { skill: id.into(), level: l.max(0) as u32 })));
    engine.register_fn("learn_spell", with!(ctx, |c, id: &str| c.actions.push(Action::LearnSpell { spell: id.into() })));
    engine.register_fn("add_mana", with!(ctx, |c, n: i64| c.actions.push(Action::AddMana { amount: n as i32 })));
    // (Numbers may be integers or floats in any mix.)
    engine.register_fn("damage_around", with!(ctx, |c, d: Dynamic, r: Dynamic| c.actions.push(Action::DamageAround { damage: num(&d).round() as i32, radius: num(&r), damage_type: None })));
    engine.register_fn("damage_around", with!(ctx, |c, d: Dynamic, r: Dynamic, t: &str| c.actions.push(Action::DamageAround { damage: num(&d).round() as i32, radius: num(&r), damage_type: damage_type(t) })));
    engine.register_fn("player_light", with!(ctx, |c, r: Dynamic, s: Dynamic| c.actions.push(player_light(&r, &s, &Map::new()))));
    engine.register_fn("player_light", with!(ctx, |c, r: Dynamic, s: Dynamic, o: Map| c.actions.push(player_light(&r, &s, &o))));
    engine.register_fn("light_at", with!(ctx, |c, x: Dynamic, y: Dynamic, r: Dynamic, s: Dynamic| c.actions.push(light_at(&x, &y, &r, &s, &Map::new()))));
    engine.register_fn("light_at", with!(ctx, |c, x: Dynamic, y: Dynamic, r: Dynamic, s: Dynamic, o: Map| c.actions.push(light_at(&x, &y, &r, &s, &o))));
    engine.register_fn("heal_area", with!(ctx, |c, x: Dynamic, y: Dynamic, r: Dynamic, n: Dynamic| c.actions.push(Action::HealArea {
        x: num(&x),
        y: num(&y),
        radius: num(&r),
        amount: num(&n).round() as i32
    })));
    // Equipment and carried items of characters ("player", "self" or an entity name).
    engine.register_fn("equip", with!(ctx, |c, t: &str, item: &str| c.actions.push(Action::Equip { target: t.into(), item: item.into() })));
    engine.register_fn("unequip", with!(ctx, |c, t: &str, slot: &str| match crate::defs::EquipSlot::parse(slot) {
        Some(slot) => c.actions.push(Action::Unequip { target: t.into(), slot }),
        None => eprintln!("[script] unequip: unknown slot '{slot}' (clothing, head, body, weapon, offhand)"),
    }));
    engine.register_fn("give_item_to", with!(ctx, |c, t: &str, item: &str, n: i64| c.actions.push(Action::GiveItemTo {
        target: t.into(),
        item: item.into(),
        count: n.max(0) as u32
    })));
    engine.register_fn("take_item_from", with!(ctx, |c, t: &str, item: &str, n: i64| c.actions.push(Action::TakeItemFrom {
        target: t.into(),
        item: item.into(),
        count: n.max(0) as u32
    })));
    // The player (position in tiles, updated every frame).
    engine.register_fn("player_x", with!(ctx, |c| c.player.x as f64));
    engine.register_fn("player_y", with!(ctx, |c| c.player.y as f64));
    engine.register_fn("player_max_hp", with!(ctx, |c| c.player.max_hp as i64));
    engine.register_fn("player_facing", with!(ctx, |c| match c.player.facing {
        Facing::Up => "up",
        Facing::Down => "down",
        Facing::Left => "left",
        Facing::Right => "right",
    }
    .to_string()));
    // Spell effects: targeting, flying sprites, one-shot animations, area damage. Positions are
    // tile coordinates (numbers, fractions allowed: 10.5 = middle of tile 10).
    engine.register_fn("aim", with!(ctx, |c, f: &str| c.actions.push(Action::Aim { function: f.into() })));
    engine.register_fn("fly", with!(ctx, |c, sheet: &str, x: Dynamic, y: Dynamic, f: &str| c.actions.push(fly(sheet, &x, &y, f, &Map::new()))));
    engine.register_fn("fly", with!(ctx, |c, sheet: &str, x: Dynamic, y: Dynamic, f: &str, o: Map| c.actions.push(fly(sheet, &x, &y, f, &o))));
    engine.register_fn("effect", with!(ctx, |c, sheet: &str, x: Dynamic, y: Dynamic| c.actions.push(Action::Effect {
        sprite: fx_sprite(sheet, &Map::new()),
        x: num(&x),
        y: num(&y)
    })));
    engine.register_fn("effect", with!(ctx, |c, sheet: &str, x: Dynamic, y: Dynamic, o: Map| c.actions.push(Action::Effect {
        sprite: fx_sprite(sheet, &o),
        x: num(&x),
        y: num(&y)
    })));
    engine.register_fn("damage_area", with!(ctx, |c, x: Dynamic, y: Dynamic, r: Dynamic, d: Dynamic| c.actions.push(Action::DamageArea {
        x: num(&x),
        y: num(&y),
        radius: num(&r),
        damage: num(&d).round() as i32,
        damage_type: None
    })));
    engine.register_fn("damage_area", with!(ctx, |c, x: Dynamic, y: Dynamic, r: Dynamic, d: Dynamic, t: &str| c.actions.push(Action::DamageArea {
        x: num(&x),
        y: num(&y),
        radius: num(&r),
        damage: num(&d).round() as i32,
        damage_type: damage_type(t)
    })));
    engine.register_fn("is_night", with!(ctx, |c| crate::render::lighting::is_night(c.world.hour)));
    engine.register_fn("is_sneaking", with!(ctx, |c| c.player.sneaking));
    engine.register_fn("set_hour", with!(ctx, |c, h: f64| c.actions.push(Action::SetTime { hour: h as f32 })));
    engine.register_fn("set_hour", with!(ctx, |c, h: i64| c.actions.push(Action::SetTime { hour: h as f32 })));
    engine.register_fn("player_sex", with!(ctx, |c| c.world.player_sex.key().to_string()));
    engine.register_fn("player_race", with!(ctx, |c| c.world.player_race.clone()));
    engine.register_fn("class", with!(ctx, |c| c.world.class.as_ref().map_or(String::new(), |h| h.class.clone())));
    engine.register_fn("subclass", with!(ctx, |c| c.world.class.as_ref().map_or(String::new(), |h| h.subclass.clone())));
    engine.register_fn("resource", with!(ctx, |c, id: &str| c.player.resources.iter().find(|r| r.0 == id).map_or(0, |r| r.1)));
    engine.register_fn("class_level", with!(ctx, |c| if c.world.class.is_some() { c.world.player_level() as i64 } else { 0 }));
    engine.register_fn("has_feature", with!(ctx, |c, id: &str| c.player.features.iter().any(|f| f == id)));
    engine.register_fn("level_up_pending", with!(ctx, |c| !c.world.levelups.is_empty()));
    engine.register_fn("set_class", with!(ctx, |c, t: &str, class: &str| c.actions.push(Action::SetClass { target: t.into(), class: class.into(), level: 0 })));
    engine.register_fn("set_class", with!(ctx, |c, t: &str, class: &str, l: Dynamic| c.actions.push(Action::SetClass { target: t.into(), class: class.into(), level: num(&l).clamp(0.0, 20.0) as u32 })));
    engine.register_fn("knows_language", with!(ctx, |c, id: &str| c.world.languages.contains(id.trim())));
    engine.register_fn("has_resistance", with!(ctx, |c, kind: &str| c.world.resistances.contains(kind.trim())));
    engine.register_fn("background", with!(ctx, |c| c.world.player_background.clone()));
    engine.register_fn("race_choice", with!(ctx, |c, id: &str| c.world.choices.get(id).map_or(rhai::Array::new(), |v| v.iter().map(|s| Dynamic::from(s.clone())).collect())));
    engine.register_fn("player_faction", with!(ctx, |c| c.player.faction.clone()));
    engine.register_fn("carry", with!(ctx, |c, t: &str, prop: &str| c.actions.push(Action::Carry { target: t.into(), prop: prop.into() })));
    engine.register_fn("stop_carrying", with!(ctx, |c, t: &str| c.actions.push(Action::Carry { target: t.into(), prop: String::new() })));
    engine.register_fn("player_carrying", with!(ctx, |c| c.world.player_carrying.clone()));
    engine.register_fn("standing", with!(ctx, |c, f: &str| c.world.standing.get(f).copied().unwrap_or_else(|| c.factions.base_standing(&c.player.faction, f)) as i64));
    engine.register_fn("change_standing", with!(ctx, |c, f: &str, amount: i64| c.actions.push(Action::ChangeStanding { faction: f.into(), amount: amount as i32 })));
    // Plugins' parameters: () when the plugin or the parameter isn't there.
    engine.register_fn("plugin_param", with!(ctx, |c, plugin: &str, name: &str| c.plugin_params.get(plugin).and_then(|m| m.get(name)).map_or(Dynamic::UNIT, |v| v.to_dynamic())));
    // Faction membership: "player", "self" (also SELF) or a name on this map.
    engine.register_fn("set_faction", with!(ctx, |c, t: &str, f: &str| c.actions.push(Action::SetFaction { target: t.into(), faction: f.into() })));
    engine.register_fn("faction_of", with!(ctx, |c, t: &str| match t {
        "player" => c.player.faction.clone(),
        "" | "self" => c.owner.and_then(|u| c.character_factions.iter().find(|ch| ch.0 == u)).map_or(String::new(), |ch| ch.2.clone()),
        name => c.character_factions.iter().find(|ch| ch.1 == name).map_or(String::new(), |ch| ch.2.clone()),
    }));
    engine.register_fn("faction_of", with!(ctx, |c, uid: i64| c.character_factions.iter().find(|ch| ch.0 as i64 == uid).map_or(String::new(), |ch| ch.2.clone())));
    // SRD: the hero's armor class and hit dice, a character's stat block (-1 without one).
    engine.register_fn("armor_class", with!(ctx, |c| c.player.armor_class as i64));
    engine.register_fn("hit_dice", with!(ctx, |c| c.player.hit_dice as i64));
    engine.register_fn("npc_stat", with!(ctx, |c, t: &str, field: &str| {
        let who = match t {
            "" | "self" => c.owner.and_then(|u| c.stat_blocks.iter().find(|s| s.0 == u)),
            name => c.stat_blocks.iter().find(|s| s.1 == name),
        };
        who.and_then(|s| s.2.field(field)).unwrap_or(-1)
    }));
    engine.register_fn("npc_stat", with!(ctx, |c, uid: i64, field: &str| c.stat_blocks.iter().find(|s| s.0 as i64 == uid).and_then(|s| s.2.field(field)).unwrap_or(-1)));
    engine.register_fn("is_female", with!(ctx, |c| c.world.player_sex == crate::state::Sex::Female));
    engine.register_fn("is_male", with!(ctx, |c| c.world.player_sex == crate::state::Sex::Male));
    // Placeholders like in dialog texts: "{player}", "{sir|madam}", "{var}".
    engine.register_fn("text", with!(ctx, |c, t: &str| c.world.expand_text(t)));
    engine.register_fn("quest_stage", with!(ctx, |c, q: &str| c.world.quest(q).map_or(-1, |p| p.stage as i64)));
    engine.register_fn("quest_active", with!(ctx, |c, q: &str| c.world.check_quest(q, crate::quest::QuestCheck::Active, 0)));
    engine.register_fn("quest_done", with!(ctx, |c, q: &str| c.world.check_quest(q, crate::quest::QuestCheck::Completed, 0)));
    {
        use crate::quest::QuestOp;
        let op = |o: QuestOp| move |q: &str| Action::Quest { quest: q.into(), op: o, stage: 0 };
        let (st, adv, done, fail) = (op(QuestOp::Start), op(QuestOp::Advance), op(QuestOp::Complete), op(QuestOp::Fail));
        engine.register_fn("start_quest", with!(ctx, |c, q: &str| c.actions.push(st(q))));
        engine.register_fn("advance_quest", with!(ctx, |c, q: &str| c.actions.push(adv(q))));
        engine.register_fn("complete_quest", with!(ctx, |c, q: &str| c.actions.push(done(q))));
        engine.register_fn("fail_quest", with!(ctx, |c, q: &str| c.actions.push(fail(q))));
    }
    engine.register_fn("set_quest_stage", with!(ctx, |c, q: &str, n: i64| c.actions.push(Action::Quest {
        quest: q.into(),
        op: crate::quest::QuestOp::SetStage,
        stage: n.max(0) as u32
    })));
    engine.register_fn("patrol", with!(ctx, |c, t: &str| c.actions.push(Action::Patrol { target: t.into(), active: true, points: Vec::new() })));
    engine.register_fn("patrol", with!(ctx, |c, t: &str, pts: Array| {
        // [[x, y], [x, y], ...] in tile coordinates.
        let points = pts
            .into_iter()
            .filter_map(|p| {
                let a = p.try_cast::<Array>()?;
                Some((a.first()?.as_int().ok()? as i32, a.get(1)?.as_int().ok()? as i32))
            })
            .collect();
        c.actions.push(Action::Patrol { target: t.into(), active: true, points })
    }));
    engine.register_fn("stop_patrol", with!(ctx, |c, t: &str| c.actions.push(Action::Patrol { target: t.into(), active: false, points: Vec::new() })));
    // (Typed, so they win over Rhai's own `sleep(seconds)`, which would pause the script.)
    engine.register_fn("sleep", with!(ctx, |c, hours: i64| c.actions.push(Action::Sleep { hours: hours as f32 })));
    engine.register_fn("sleep", with!(ctx, |c, hours: f64| c.actions.push(Action::Sleep { hours: hours as f32 })));
    engine.register_fn("is_sitting", with!(ctx, |c| c.player.pose == "sit"));
    engine.register_fn("is_sleeping", with!(ctx, |c| c.player.sleeping || c.player.pose == "lie"));
    engine.register_fn("lockpick", with!(ctx, |c, difficulty: i64, var: &str| c.actions.push(Action::Lockpick { difficulty: difficulty.max(0) as u32, var: var.into() })));
    engine.register_fn("unlock_achievement", with!(ctx, |c, id: &str| c.actions.push(Action::UnlockAchievement { id: id.into() })));
    engine.register_fn("achievement_unlocked", with!(ctx, |c, id: &str| c.achievements.contains(id)));
    engine.register_fn("can_craft", with!(ctx, |c, id: &str| c.recipes.iter().find(|r| r.id == id).is_some_and(|r| r.missing(&c.world, &c.tool_kinds).is_none())));
    engine.register_fn("open_crafting", with!(ctx, |c, station: &str| c.actions.push(Action::OpenCrafting { station: station.into(), title: String::new() })));
    engine.register_fn("open_crafting", with!(ctx, |c, station: &str, title: &str| c.actions.push(Action::OpenCrafting { station: station.into(), title: title.into() })));
    engine.register_fn("follow", with!(ctx, |c, t: &str| c.actions.push(Action::Companion { target: t.into(), join: true })));
    engine.register_fn("stop_following", with!(ctx, |c, t: &str| c.actions.push(Action::Companion { target: t.into(), join: false })));
    engine.register_fn("is_companion", with!(ctx, |c, name: &str| c.world.companions.iter().any(|m| m.name == name)));
    engine.register_fn("companion_count", with!(ctx, |c| c.world.companions.len() as i64));
    // Orders: order("guard", "defend" | "attack" | "passive" | "stay" | "follow"), "all" = every companion.
    engine.register_fn("order", with!(ctx, |c, t: &str, o: &str| c.actions.push(Action::CompanionOrder { target: t.into(), order: o.into() })));
    engine.register_fn("order_of", with!(ctx, |c, name: &str| c.world.companions.iter().find(|m| m.name == name).map_or(String::new(), |m| m.order.id().to_string())));
    engine.register_fn("open_companion", with!(ctx, |c, t: &str| c.actions.push(Action::OpenCompanion { target: t.into() })));
    engine.register_fn("is_staying", with!(ctx, |c, name: &str| c.world.companions.iter().any(|m| m.name == name && m.post.is_some())));
    engine.register_fn("follow_schedule", with!(ctx, |c, t: &str| c.actions.push(Action::Schedule { target: t.into(), active: true })));
    engine.register_fn("stop_schedule", with!(ctx, |c, t: &str| c.actions.push(Action::Schedule { target: t.into(), active: false })));
    engine.register_fn("open_shop", with!(ctx, |c, t: &str| c.actions.push(Action::OpenShop { target: t.into() })));
    // The HUD (status frame, clock, action bar, buttons), e.g. for cut-scenes.
    engine.register_fn("hide_ui", with!(ctx, |c| c.actions.push(Action::SetUiVisible { visible: false })));
    // Pictures over the game: a sprite sheet drawn whole (the event waits until the player closes
    // it, unless `wait: false`).
    engine.register_fn("show_image", with!(ctx, |c, sheet: &str| c.actions.push(show_image(sheet, &Map::new()))));
    engine.register_fn("show_image", with!(ctx, |c, sheet: &str, o: Map| c.actions.push(show_image(sheet, &o))));
    engine.register_fn("hide_image", with!(ctx, |c| c.actions.push(Action::HideImage)));
    // Cut-scenes and scripted walking (tile coordinates; fractional ones are rounded down).
    engine.register_fn("cutscene", with!(ctx, |c, f: &str| c.actions.push(Action::Cutscene { function: f.into() })));
    engine.register_fn("dim", with!(ctx, |c, amount: Dynamic, seconds: Dynamic| c.actions.push(Action::Dim {
        amount: num(&amount).clamp(0.0, 1.0),
        seconds: num(&seconds).max(0.0),
    })));
    // Enemy spawners of a map ("" = the current one), all or one zone by name.
    engine.register_fn("disable_spawners", with!(ctx, |c, map: &str| c.actions.push(Action::SetSpawners { map: map.into(), zone: String::new(), active: false })));
    engine.register_fn("disable_spawners", with!(ctx, |c, map: &str, zone: &str| c.actions.push(Action::SetSpawners { map: map.into(), zone: zone.into(), active: false })));
    engine.register_fn("enable_spawners", with!(ctx, |c, map: &str| c.actions.push(Action::SetSpawners { map: map.into(), zone: String::new(), active: true })));
    engine.register_fn("defeat_enemies", with!(ctx, |c| c.actions.push(Action::DefeatEnemies)));
    engine.register_fn("autosave", with!(ctx, |c| c.actions.push(Action::Autosave)));
    engine.register_fn("enable_spawners", with!(ctx, |c, map: &str, zone: &str| c.actions.push(Action::SetSpawners { map: map.into(), zone: zone.into(), active: true })));
    engine.register_fn("walk_to", with!(ctx, |c, t: &str, x: Dynamic, y: Dynamic| c.actions.push(Action::WalkTo {
        target: t.into(),
        x: num(&x).floor() as i32,
        y: num(&y).floor() as i32,
    })));
    engine.register_fn("approach", with!(ctx, |c, t: &str, to: &str| c.actions.push(Action::Approach { target: t.into(), to: to.into() })));
    engine.register_fn("place", with!(ctx, |c, t: &str, x: Dynamic, y: Dynamic| c.actions.push(Action::Place {
        target: t.into(),
        x: num(&x).floor() as i32,
        y: num(&y).floor() as i32,
    })));
    engine.register_fn("show_ui", with!(ctx, |c| c.actions.push(Action::SetUiVisible { visible: true })));
    engine.register_fn("set_state", with!(ctx, |c, t: &str, s: &str| c.actions.push(Action::SetState { target: t.into(), state: s.into() })));
    engine.register_fn("move_by", with!(ctx, |c, t: &str, dx: i64, dy: i64| c.actions.push(Action::Move {
        target: t.into(),
        dx: dx as i32,
        dy: dy as i32
    })));
    engine.register_fn("face", with!(ctx, |c, t: &str, dir: &str| {
        let facing = match dir {
            "up" => Facing::Up,
            "left" => Facing::Left,
            "right" => Facing::Right,
            _ => Facing::Down,
        };
        c.actions.push(Action::Face { target: t.into(), facing })
    }));
    engine.register_fn("wait", with!(ctx, |c, s: Dynamic| c.actions.push(Action::Wait { seconds: num(&s) })));
    engine.register_fn("emit", with!(ctx, |c, n: &str| c.actions.push(Action::Emit { name: n.into() })));
    engine.register_fn("heal", with!(ctx, |c, n: i64| c.actions.push(Action::Heal { amount: n as i32 })));
    // A character ("player" works too): companions, friendly characters, enemies (negative hurts).
    // (heal_npc is the old name of heal_character: deprecated in 0.27, still works.)
    engine.register_fn("heal_character", with!(ctx, |c, t: &str, n: i64| c.actions.push(Action::HealCharacter { target: t.into(), amount: n as i32 })));
    engine.register_fn("heal_npc", with!(ctx, |c, t: &str, n: i64| c.actions.push(Action::HealCharacter { target: t.into(), amount: n as i32 })));
    engine.register_fn("api_version", || crate::script_api::API_VERSION.to_string());
    engine.register_fn("set_locked", with!(ctx, |c, t: &str, locked: bool| c.actions.push(Action::SetLocked {
        target: t.into(),
        locked
    })));
    // Whether a prop is locked: "self", a name or an entity uid (SELF); false without a lock.
    engine.register_fn("is_locked", with!(ctx, |c, t: &str| {
        let owner = c.owner;
        c.locks.iter().any(|l| l.2 && if t.is_empty() || t == "self" { Some(l.0) == owner } else { l.1 == t })
    }));
    engine.register_fn("is_locked", with!(ctx, |c, uid: i64| c.locks.iter().any(|l| l.2 && l.0 as i64 == uid)));
    // Objects: a prop's hit points (-1 without object statistics), damage to one (queued).
    engine.register_fn("prop_hp", with!(ctx, |c, t: &str| {
        let owner = c.owner;
        c.objects.iter().find(|o| if t.is_empty() || t == "self" { Some(o.0) == owner } else { o.1 == t }).map_or(-1, |o| o.2 as i64)
    }));
    engine.register_fn("trap_state", with!(ctx, |c, t: &str| c.traps.iter().find(|x| x.0 == t).map_or(String::new(), |x| x.1.clone())));
    engine.register_fn("disarm_trap", with!(ctx, |c, t: &str| c.actions.push(Action::DisarmTrap { target: t.into() })));
    engine.register_fn("poison", with!(ctx, |c, t: &str, item: &str| c.actions.push(Action::Poison { target: t.into(), item: item.into() })));
    engine.register_fn("damage_prop", with!(ctx, |c, t: &str, amount: i64| c.actions.push(Action::DamageProp { target: t.into(), amount: amount as i32, damage_type: None })));
    engine.register_fn("damage_prop", with!(ctx, |c, t: &str, amount: i64, kind: &str| c.actions.push(Action::DamageProp { target: t.into(), amount: amount as i32, damage_type: crate::srd::DamageType::parse(kind) })));
    engine.register_fn("play_sound", with!(ctx, |c, id: &str| c.actions.push(Action::PlaySound { sound: id.into(), volume: 1.0, pitch: 1.0, pan: 0.0 })));
    engine.register_fn("play_sound", with!(ctx, |c, id: &str, o: rhai::Map| {
        let f = |k: &str, d: f32| o.get(k).map_or(d, num);
        c.actions.push(Action::PlaySound { sound: id.into(), volume: f("volume", 1.0), pitch: f("pitch", 1.0), pan: f("pan", 0.0) })
    }));
    engine.register_fn("play_music", with!(ctx, |c, id: &str| c.actions.push(Action::PlayMusic { sound: id.into(), fade: 0.0 })));
    engine.register_fn("play_music", with!(ctx, |c, id: &str, secs: Dynamic| c.actions.push(Action::PlayMusic { sound: id.into(), fade: num(&secs).max(0.0) })));
    engine.register_fn("play_music_effect", with!(ctx, |c, id: &str| c.actions.push(Action::PlayMusicEffect { sound: id.into(), wait: false })));
    engine.register_fn("play_music_effect", with!(ctx, |c, id: &str, o: rhai::Map| c.actions.push(Action::PlayMusicEffect { sound: id.into(), wait: o.get("wait").and_then(|v| v.as_bool().ok()).unwrap_or(false) })));
    // Continue in another function after the queued actions (e.g. after a choice).
    engine.register_fn("then_call", with!(ctx, |c, f: &str| c.actions.push(Action::RunScript {
        function: f.into()
    })));
    // Queued variant of set_var, for ordering relative to dialogs.
    engine.register_fn("queue_set_var", with!(ctx, |c, n: &str, v: i64| c.actions.push(Action::SetVar {
        name: n.into(),
        op: VarOp::Set,
        value: v
    })));
}

/// A script number (integer or float) as `f32` (anything else: 0).
/// A condition by its id ("poisoned", "prone"…, or a status effect name of 0.27; "all" = any);
/// unknown names are logged.
fn condition(name: &str) -> Option<crate::srd::Condition> {
    let k = crate::srd::Condition::parse(name);
    if k.is_none() {
        let names: Vec<&str> = crate::srd::Condition::ALL.iter().map(|k| k.id()).collect();
        eprintln!("[script] unknown condition '{name}' (one of: {})", names.join(", "));
    }
    k
}

/// A damage type by its id ("fire"; "" = untyped); unknown ones are logged and untyped.
fn damage_type(name: &str) -> Option<crate::srd::DamageType> {
    if name.trim().is_empty() {
        return None;
    }
    let t = crate::srd::DamageType::parse(name);
    if t.is_none() {
        eprintln!("[script] unknown damage type '{name}' (acid, bludgeoning, cold, fire, force, lightning, necrotic, piercing, poison, psychic, radiant, slashing, thunder)");
    }
    t
}

/// `apply_condition(target, condition, seconds, #{ save, dc, damage, type, level })`.
fn condition_spec(k: crate::srd::Condition, seconds: f32, o: &Map) -> crate::srd::ConditionSpec {
    let text = |key: &str| o.get(key).and_then(|v| v.clone().into_string().ok()).unwrap_or_default();
    let mut spec = crate::srd::ConditionSpec::new(k, seconds);
    if k == crate::srd::Condition::Any {
        spec.condition = None;
    }
    if let Some(l) = o.get("level") {
        spec.level = num(l).clamp(0.0, 6.0) as u8;
    }
    let save = text("save");
    if !save.is_empty() {
        let dc = o.get("dc").map_or(10.0, num) as i32;
        spec.save = Some(crate::srd::conditions::SaveDc { ability: save, dc });
    }
    let dice = match o.get("damage") {
        Some(v) if v.is_int() || v.is_float() => format!("{}", num(v).round() as i64),
        _ => text("damage"),
    };
    if !dice.is_empty() {
        let kind = text("type");
        spec.ongoing = Some(crate::srd::conditions::Ongoing { dice, damage_type: if kind == "heal" || kind == "healing" { None } else { damage_type(&kind).or(Some(crate::srd::DamageType::Force)) } });
    }
    spec
}

/// `show_image(sheet, #{ wait, scroll, speed, scale })`.
fn show_image(sheet: &str, o: &Map) -> Action {
    Action::ShowImage {
        sheet: sheet.into(),
        wait: o.get("wait").and_then(|v| v.as_bool().ok()).unwrap_or(true),
        scroll: o.get("scroll").and_then(|v| v.as_bool().ok()).unwrap_or(false),
        speed: o.get("speed").map_or(crate::events::image_speed(), num).max(1.0),
        scale: o.get("scale").map_or(1.0, num).max(0.05),
    }
}

fn num(d: &Dynamic) -> f32 {
    d.as_float().map(|f| f as f32).or_else(|_| d.as_int().map(|i| i as f32)).unwrap_or(0.0)
}

/// The sprite of `fly` / `effect` from the sheet and the options map: `frames: [first, last]`
/// or `tag: "name"` (a named animation of the sheet), `fps`, `scale`, `light`, `color: [r, g, b]`
/// (of the light).
fn fx_sprite(sheet: &str, o: &Map) -> FxSprite {
    let opt = |k: &str| o.get(k).map(num);
    let mut s = FxSprite { sheet: sheet.into(), ..Default::default() };
    if let Some(f) = o.get("frames").and_then(|f| f.clone().try_cast::<Array>()) {
        s.first = f.first().map_or(0.0, num).max(0.0) as u32;
        s.last = f.get(1).map_or(s.first as f32, num).max(s.first as f32) as u32;
    } else if let Some(f) = opt("frame") {
        (s.first, s.last) = (f.max(0.0) as u32, f.max(0.0) as u32);
    }
    if let Some(t) = o.get("tag").and_then(|t| t.clone().into_string().ok()) {
        s.tag = t;
    }
    s.fps = opt("fps").unwrap_or(s.fps).max(0.1);
    s.scale = opt("scale").unwrap_or(s.scale).max(0.05);
    s.light = opt("light").unwrap_or(0.0).max(0.0);
    s.shake = opt("shake").unwrap_or(0.0).max(0.0);
    if let Some(c) = o.get("color").and_then(|c| c.clone().try_cast::<Array>()).filter(|a| a.len() >= 3) {
        s.glow = [0, 1, 2].map(|i| num(&c[i]).clamp(0.0, 255.0) as u8);
    }
    s
}

/// *Set weather* from a script: "default" goes back to the map's own weather; an unknown kind
/// is logged and changes nothing (a `SetWeather` without weather = default would be wrong).
fn weather_action(map: &str, kind: &str, intensity: f32) -> Action {
    use crate::map::{Weather, WeatherKind};
    let weather = if kind.trim().eq_ignore_ascii_case("default") {
        None
    } else {
        match WeatherKind::from_id(kind) {
            Some(k) => Some(Weather::new(k, intensity)),
            None => {
                eprintln!("[script] set_weather: unknown weather '{kind}' (none, rain, snow, fog or default)");
                return Action::Wait { seconds: 0.0 };
            }
        }
    };
    Action::SetWeather { map: map.into(), weather }
}

/// Light options: `color: [r, g, b]` (0..255), `flicker: true`.
fn light_opts(o: &Map, default: [u8; 3]) -> ([u8; 3], bool) {
    let color = o
        .get("color")
        .and_then(|c| c.clone().try_cast::<Array>())
        .filter(|a| a.len() >= 3)
        .map_or(default, |a| [0, 1, 2].map(|i| num(&a[i]).clamp(0.0, 255.0) as u8));
    let flicker = o.get("flicker").and_then(|f| f.as_bool().ok()).unwrap_or(false);
    (color, flicker)
}

fn player_light(r: &Dynamic, s: &Dynamic, o: &Map) -> Action {
    let (color, flicker) = light_opts(o, crate::events::spell_light_color());
    Action::PlayerLight { radius: num(r), seconds: num(s), color, flicker }
}

fn light_at(x: &Dynamic, y: &Dynamic, r: &Dynamic, s: &Dynamic, o: &Map) -> Action {
    let (color, flicker) = light_opts(o, [255, 200, 140]);
    Action::LightAt { x: num(x), y: num(y), radius: num(r), seconds: num(s), color, flicker }
}

fn fly(sheet: &str, x: &Dynamic, y: &Dynamic, function: &str, o: &Map) -> Action {
    let speed = o.get("speed").map_or(8.0, num).max(0.5);
    Action::Fly { sprite: fx_sprite(sheet, o), x: num(x), y: num(y), speed, function: function.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_functions() {
        use crate::map::{Weather, WeatherKind};
        let mut host = ScriptHost::new();
        host.load_source(r#"fn w() { set_weather("rain", 0.8); set_weather("default"); set_map_weather("forest", "fog", 2); set_weather("hail"); weather() }"#).unwrap();
        host.set_player(PlayerInfo { weather: Weather::new(WeatherKind::Snow, 0.4), ..Default::default() });
        let mut world = WorldState::default();
        let v = host.call_inner(&mut world, "w", vec![], None).unwrap();
        assert_eq!(v.into_string().unwrap(), "snow");
        let a = std::mem::take(&mut host.ctx.borrow_mut().actions);
        assert_eq!(
            a,
            vec![
                Action::SetWeather { map: String::new(), weather: Some(Weather::new(WeatherKind::Rain, 0.8)) },
                Action::SetWeather { map: String::new(), weather: None },
                Action::SetWeather { map: "forest".into(), weather: Some(Weather::new(WeatherKind::Fog, 1.0)) },
                Action::Wait { seconds: 0.0 },
            ]
        );
    }

    #[test]
    fn effect_tag_option() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn boom() { effect("boom", 2, 3, #{ tag: "explode", scale: 2 }); }"#).unwrap();
        let mut w = WorldState::default();
        let a = host.call(&mut w, "boom", None);
        let Some(Action::Effect { sprite, .. }) = a.first() else { panic!("{a:?}") };
        assert_eq!((sprite.sheet.as_str(), sprite.tag.as_str(), sprite.scale), ("boom", "explode", 2.0));
    }

    #[test]
    fn hot_reload_keeps_working_scripts_on_errors() {
        let dir = std::env::temp_dir().join(format!("rustpg2d_reload_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rhai"), "fn greet() { set_var(\"n\", 1); }").unwrap();
        let mut host = ScriptHost::new();
        host.load_dir(&dir);
        let mut w = WorldState::default();
        host.call(&mut w, "greet", None);
        assert_eq!(w.var("n"), 1);
        // A broken edit: the old function stays.
        std::fs::write(dir.join("a.rhai"), "fn greet() { set_var(\"n\", 2) ").unwrap();
        let errs = host.reload_dir(&dir).unwrap_err();
        assert!(errs[0].starts_with("a.rhai:"), "{errs:?}");
        host.call(&mut w, "greet", None);
        assert_eq!(w.var("n"), 1);
        // Fixed, plus a new file.
        std::fs::write(dir.join("a.rhai"), "fn greet() { set_var(\"n\", 2); }").unwrap();
        std::fs::write(dir.join("b.rhai"), "fn extra() { 5 }").unwrap();
        assert_eq!(host.reload_dir(&dir), Ok(2));
        host.call(&mut w, "greet", None);
        assert_eq!(w.var("n"), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn console_eval_uses_script_functions() {
        let mut host = ScriptHost::new();
        host.load_source("fn double(x) { x * 2 } fn hi() { print(\"hi\"); give_item(\"coin\", 1); }").unwrap();
        let mut w = WorldState::default();
        assert_eq!(host.eval(&mut w, "double(21)").0, Ok("42".into()));
        let (r, actions) = host.eval(&mut w, "set_flag(\"x\", true); hi()");
        assert!(r.is_ok());
        assert_eq!(host.eval(&mut w, "let a = 1;").0, Ok(String::new()));
        assert!(w.flag("x") && w.item_count("coin") == 1 && actions.is_empty());
        assert_eq!(host.take_printed(), vec!["hi".to_string()]);
        assert!(host.eval(&mut w, "nope(").0.is_err());
        assert!(host.eval(&mut w, "missing()").0.is_err());
    }

    #[test]
    fn hooks_change_the_map_or_keep_it() {
        let mut host = ScriptHost::new();
        host.load_source(
            r#"
            fn double(m) { m.amount *= 2; give_item("coin", 1); m }
            fn keep(m) { }
            fn wrong(m) { 5 }
            fn forever(m) { loop { } }
        "#,
        )
        .unwrap();
        let mut w = WorldState::default();
        let m = crate::hooks::map([("amount", Dynamic::from(3_i64))]);
        let (r, actions) = host.call_hook(&mut w, "double", m.clone());
        assert_eq!(crate::hooks::int(&r.unwrap(), "amount", 0), 6);
        assert!(actions.is_empty() && w.item_count("coin") == 1);
        assert!(host.call_hook(&mut w, "keep", m.clone()).0.is_none());
        assert!(host.call_hook(&mut w, "wrong", m.clone()).0.is_none());
        // A runaway hook stops at its operation limit (an error, the map unchanged).
        assert!(host.call_hook(&mut w, "forever", m.clone()).0.is_none());
        assert!(host.errors.iter().any(|e| e.contains("forever")));
        // ... and ordinary calls get their own, bigger limit back.
        host.load_source("fn count() { let n = 0; for i in 0..100000 { n += 1; } n }").unwrap();
        assert_eq!(host.call_inner(&mut w, "count", vec![], None).and_then(|v| v.as_int().ok()), Some(100000));
    }

    #[test]
    fn faction_membership_from_scripts() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn who() { [faction_of("player"), faction_of("self"), faction_of(SELF), faction_of("Rook"), faction_of("nobody")] } fn turn() { set_faction("Rook", "humans"); set_faction("player", "") }"#).unwrap();
        host.set_player(PlayerInfo { faction: "elves".into(), ..Default::default() });
        host.set_character_factions(vec![(7, "Rook".into(), "bandits".into()), (9, String::new(), "wild".into())]);
        let mut w = WorldState::default();
        let list: Vec<String> = host.call_inner(&mut w, "who", vec![], Some(9)).unwrap().into_array().unwrap().into_iter().map(|v| v.into_string().unwrap()).collect();
        assert_eq!(list, ["elves", "wild", "wild", "bandits", ""]);
        let actions = host.call(&mut w, "turn", None);
        assert_eq!(actions, vec![Action::SetFaction { target: "Rook".into(), faction: "humans".into() }, Action::SetFaction { target: "player".into(), faction: String::new() }]);
    }

    #[test]
    fn stat_blocks_from_scripts() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn s() { [armor_class(), hit_dice(), npc_stat("Brom", "ac"), npc_stat(SELF, "str"), npc_stat("self", "xp"), npc_stat("Brom", "luck"), npc_stat("nobody", "hp")] }"#).unwrap();
        host.set_player(PlayerInfo { armor_class: 13, hit_dice: 2, ..Default::default() });
        let guard = crate::srd::npc_template("guard").unwrap().clone();
        let bandit = crate::srd::npc_template("bandit").unwrap().clone();
        host.set_stat_blocks(vec![(7, "Brom".into(), guard), (9, String::new(), bandit)]);
        let mut w = WorldState::default();
        let v = host.call_inner(&mut w, "s", vec![], Some(9)).unwrap_or_else(|| panic!("{:?}", host.errors)).into_typed_array::<i64>().unwrap();
        assert_eq!(v, [13, 2, 16, 11, 25, -1, -1]);
    }

    #[test]
    fn hp_of_characters() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn hp() { [hp_of("self"), hp_of(SELF), hp_of("chief"), hp_of("nobody"), hp_of("player")] }"#).unwrap();
        host.set_characters(vec![(7, "chief".into(), 5), (9, String::new(), 2)]);
        let mut w = WorldState { player_hp: Some(11), ..Default::default() };
        let v = host.call_inner(&mut w, "hp", vec![], Some(9)).unwrap().into_typed_array::<i64>().unwrap();
        assert_eq!(v, vec![2, 2, 5, -1, 11]);
    }

    #[test]
    fn api_mutates_world_and_queues_actions() {
        let mut host = ScriptHost::new();
        host.load_source(
            r#"
            fn talk() {
                if !flag("met") {
                    set_flag("met", true);
                    give_item("coin", 2);
                    say("Bob", "Hello!");
                    choice("Buy?", ["Yes", "No"], "buy");
                    then_call("after");
                } else {
                    say("Again?");
                }
            }
            fn rich() { add_var("n", 2); get_var("n") == 2 && item_count("coin") >= 2 && random(5) < 5 }
            fn fight() { if player_hp() < 3 { heal(2); play_sound("ding"); set_locked("door", false); } }
            "#,
        )
        .unwrap();
        let mut world = WorldState::default();
        let actions = host.call(&mut world, "talk", Some(3));
        assert!(world.flag("met"));
        assert_eq!(world.item_count("coin"), 2);
        assert_eq!(actions.len(), 3);
        assert!(matches!(&actions[1], Action::Choice { options, .. } if options.len() == 2));
        assert!(host.condition(&mut world, "rich", None));
        let actions = host.call(&mut world, "talk", None);
        assert_eq!(actions, vec![Action::Dialog { speaker: String::new(), text: "Again?".into() }]);
        world.player_hp = Some(1);
        let actions = host.call(&mut world, "fight", None);
        assert_eq!(
            actions,
            vec![
                Action::Heal { amount: 2 },
                Action::PlaySound { sound: "ding".into(), volume: 1.0, pitch: 1.0, pan: 0.0 },
                Action::SetLocked { target: "door".into(), locked: false }
            ]
        );
    }

    #[test]
    fn spell_effect_api() {
        let mut host = ScriptHost::new();
        host.load_source(
            r#"
            fn cast() { aim("hit"); }
            fn hit(x, y) {
                fly("fireball", x, y, "boom", #{ speed: 10, frames: [0, 3], light: 2.5 });
                effect("explosion", 4, 5.5);
                damage_area(x, y, 2, 4);
            }
            "#,
        )
        .unwrap();
        let mut w = WorldState::default();
        assert_eq!(host.call(&mut w, "cast", None), vec![Action::Aim { function: "hit".into() }]);
        // Lights, healing and the player's position; ints and floats mix freely.
        host.load_source(
            r#"fn glow() { player_light(5, 30.5); player_light(4, 10, #{ color: [255, 0, 0], flicker: true });
                light_at(player_x(), player_y() - 1, 3, 2.5); heal_area(1.5, 2, 2, 3); player_max_hp() + 0 }"#,
        )
        .unwrap();
        host.load_source("fn pause() { wait(1); wait(0.5); }").unwrap();
        // Cut-scenes: the function to play, scripted walks (tiles; decimals rounded down).
        host.load_source(r#"fn scene() { cutscene("intro"); place("player", 1, 18); walk_to("player", 5.7, 18); approach("guard", "player"); hide_ui(); show_ui(); }"#).unwrap();
        assert_eq!(
            host.call(&mut w, "scene", None),
            vec![
                Action::Cutscene { function: "intro".into() },
                Action::Place { target: "player".into(), x: 1, y: 18 },
                Action::WalkTo { target: "player".into(), x: 5, y: 18 },
                Action::Approach { target: "guard".into(), to: "player".into() },
                Action::SetUiVisible { visible: false },
                Action::SetUiVisible { visible: true },
            ]
        );
        host.load_source(r#"fn calm() { dim(0.8, 1.5); dim(2, -1); disable_spawners("forest"); enable_spawners("", "den"); defeat_enemies(); autosave(); }"#).unwrap();
        assert_eq!(
            host.call(&mut w, "calm", None),
            vec![
                Action::Dim { amount: 0.8, seconds: 1.5 },
                Action::Dim { amount: 1.0, seconds: 0.0 },
                Action::SetSpawners { map: "forest".into(), zone: String::new(), active: false },
                Action::SetSpawners { map: String::new(), zone: "den".into(), active: true },
                Action::DefeatEnemies,
                Action::Autosave,
            ]
        );
        assert_eq!(host.call(&mut w, "pause", None), vec![Action::Wait { seconds: 1.0 }, Action::Wait { seconds: 0.5 }]);
        host.set_player(PlayerInfo { x: 4.5, y: 6.0, max_hp: 12, facing: Facing::Up, ..Default::default() });
        assert_eq!(
            host.call(&mut w, "glow", None),
            vec![
                Action::PlayerLight { radius: 5.0, seconds: 30.5, color: crate::events::spell_light_color(), flicker: false },
                Action::PlayerLight { radius: 4.0, seconds: 10.0, color: [255, 0, 0], flicker: true },
                Action::LightAt { x: 4.5, y: 5.0, radius: 3.0, seconds: 2.5, color: [255, 200, 140], flicker: false },
                Action::HealArea { x: 1.5, y: 2.0, radius: 2.0, amount: 3 },
            ]
        );
        let a = host.call_args(&mut w, "hit", vec![Dynamic::from(3.5_f64), Dynamic::from(7_i64)], None);
        let fs = |sheet: &str, first, last, light| FxSprite { sheet: sheet.into(), first, last, light, ..Default::default() };
        assert_eq!(
            a,
            vec![
                Action::Fly { sprite: fs("fireball", 0, 3, 2.5), x: 3.5, y: 7.0, speed: 10.0, function: "boom".into() },
                Action::Effect { sprite: fs("explosion", 0, 0, 0.0), x: 4.0, y: 5.5 },
                Action::DamageArea { x: 3.5, y: 7.0, radius: 2.0, damage: 4, damage_type: None },
            ]
        );
    }

    /// The examples in Documentation/manual/08-scripting.md compile, and the Rhai primer does what it says.
    #[test]
    fn documentation_examples() {
        let doc = include_str!("../../../Documentation/manual/08-scripting.md");
        let blocks: Vec<&str> = doc.split("```rust").skip(1).filter_map(|b| b.split("```").next()).collect();
        assert!(blocks.len() >= 10, "{}", blocks.len());
        for b in &blocks {
            // Snippets of calls only (no function around them) are compiled inside one.
            let src = if b.contains("fn ") { b.to_string() } else { format!("fn snippet(x, y) {{ {b} }}") };
            let mut host = ScriptHost::new();
            host.load_source(&src).unwrap_or_else(|e| panic!("{e}\n{src}"));
        }
        let mut host = ScriptHost::new();
        host.load_source(blocks.iter().find(|b| b.contains("fn count_to_five")).unwrap()).unwrap();
        let mut w = WorldState::default();
        let mut run = |f: &str, args: Vec<Dynamic>| host.call_inner(&mut w, f, args, None).map(|v| v.to_string());
        assert_eq!(run("count_to_five", vec![]).as_deref(), Some("9"));
        assert_eq!(run("greet", vec![Dynamic::from("Mira")]).as_deref(), Some("Hello, Mira!"));
        assert!(run("lists_and_maps", vec![]).is_some());
        assert!(host.errors.is_empty(), "{:?}", host.errors);
    }

    #[test]
    fn patrol_api() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn walk() { patrol("guard", [[1, 2], [3, 4]]); stop_patrol("guard"); patrol("self"); }"#).unwrap();
        let actions = host.call(&mut WorldState::default(), "walk", None);
        assert_eq!(
            actions,
            vec![
                Action::Patrol { target: "guard".into(), active: true, points: vec![(1, 2), (3, 4)] },
                Action::Patrol { target: "guard".into(), active: false, points: vec![] },
                Action::Patrol { target: "self".into(), active: true, points: vec![] },
            ]
        );
        host.load_source(r#"fn day() { stop_schedule("mira"); follow_schedule("mira"); }"#).unwrap();
        let actions = host.call(&mut WorldState::default(), "day", None);
        assert_eq!(actions, vec![Action::Schedule { target: "mira".into(), active: false }, Action::Schedule { target: "mira".into(), active: true }]);
    }

    #[test]
    fn companion_api() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn party() { if !is_companion("guard") && companion_count() == 0 { follow("guard"); } stop_following("self"); }"#).unwrap();
        let actions = host.call(&mut WorldState::default(), "party", None);
        assert_eq!(actions, vec![Action::Companion { target: "guard".into(), join: true }, Action::Companion { target: "self".into(), join: false }]);
        let mut w = WorldState::default();
        w.companions.push(crate::state::Companion { ai: false, map: "village".into(), uid: 22, name: "guard".into(), hp: None, down: None, order: Default::default(), post: None, mana: None, ..Default::default() });
        assert!(host.call(&mut w, "party", None).len() == 1, "already following: only stop_following");
    }

    #[test]
    fn sleep_api() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn rest() { if !is_sitting() && !is_sleeping() { sleep(8); } sleep(2.5); }"#).unwrap();
        assert_eq!(host.call(&mut WorldState::default(), "rest", None), vec![Action::Sleep { hours: 8.0 }, Action::Sleep { hours: 2.5 }]);
        host.set_player(PlayerInfo { pose: "sit".into(), ..Default::default() });
        assert_eq!(host.call(&mut WorldState::default(), "rest", None), vec![Action::Sleep { hours: 2.5 }]);
    }

    #[test]
    fn event_api_tells_the_item_and_the_owner() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn gift() { if is_consumable(used_item()) { set_var(self_name() + "_" + used_item(), 1); } else { set_var("kept", 1); } }"#).unwrap();
        host.set_consumables(["bread".to_string()].into_iter().collect());
        let mut w = WorldState::default();
        host.set_event("bread", "tomas");
        host.call(&mut w, "gift", Some(7));
        assert_eq!(w.var("tomas_bread"), 1);
        host.set_event("old_boot", "tomas");
        host.call(&mut w, "gift", Some(7));
        assert_eq!(w.var("kept"), 1);
    }

    #[test]
    fn lockpick_api() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn puzzle() { lockpick(2, "vault"); then_call("after"); }"#).unwrap();
        let actions = host.call(&mut WorldState::default(), "puzzle", None);
        assert_eq!(actions[0], Action::Lockpick { difficulty: 2, var: "vault".into() });
    }

    #[test]
    fn achievement_api() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn hoard() { if !achievement_unlocked("hoard") { unlock_achievement("hoard"); } }"#).unwrap();
        assert_eq!(host.call(&mut WorldState::default(), "hoard", None), vec![Action::UnlockAchievement { id: "hoard".into() }]);
        host.set_achievements(["hoard".to_string()].into());
        assert!(host.call(&mut WorldState::default(), "hoard", None).is_empty());
    }

    #[test]
    fn crafting_api() {
        let mut host = ScriptHost::new();
        host.set_recipes(vec![crate::crafting::RecipeDef::new("brew", "potion", 1, vec![crate::inventory::ItemStack::new("herb", 2)], "alchemy")], Default::default());
        host.load_source(r#"fn table() { if can_craft("brew") { open_crafting("alchemy", "Alchemy table"); } else { open_crafting("alchemy"); } }"#).unwrap();
        let mut w = WorldState::default();
        assert_eq!(host.call(&mut w, "table", None), vec![Action::OpenCrafting { station: "alchemy".into(), title: String::new() }]);
        w.give_item("herb", 2);
        assert_eq!(host.call(&mut w, "table", None), vec![Action::OpenCrafting { station: "alchemy".into(), title: "Alchemy table".into() }]);
    }

    #[test]
    fn say_with_an_entity_as_speaker() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn talk() { say(SELF, "Hello"); }"#).unwrap();
        let actions = host.call(&mut WorldState::default(), "talk", Some(7));
        assert_eq!(actions, vec![Action::Dialog { speaker: "{uid:7}".into(), text: "Hello".into() }]);
        // Without an owner SELF is -1: no speaker.
        let actions = host.call(&mut WorldState::default(), "talk", None);
        assert_eq!(actions, vec![Action::Dialog { speaker: String::new(), text: "Hello".into() }]);
    }

    #[test]
    fn emotions_from_scripts() {
        let mut host = ScriptHost::new();
        host.load_source(r#"fn f() { emote("guard", "surprise", #{ wait: true, seconds: 1.5 }); say("Tilda", "Hello!", #{ emote: "joy" }); clear_emote("player"); }"#).unwrap();
        let actions = host.call(&mut WorldState::default(), "f", None);
        assert_eq!(
            actions,
            vec![
                Action::Emote { target: "guard".into(), emotion: "surprise".into(), seconds: 1.5, wait: true },
                // While the line is up, then taken away.
                Action::Emote { target: "Tilda".into(), emotion: "joy".into(), seconds: -1.0, wait: false },
                Action::Dialog { speaker: "Tilda".into(), text: "Hello!".into() },
                Action::ClearEmote { target: "Tilda".into() },
                Action::ClearEmote { target: "player".into() },
            ]
        );
    }

    #[test]
    fn errors_are_reported_not_fatal() {
        let mut host = ScriptHost::new();
        let mut world = WorldState::default();
        world.set_flag("keep", true);
        assert!(host.call(&mut world, "missing", None).is_empty());
        assert_eq!(host.errors.len(), 1);
        assert!(world.flag("keep"), "world must be restored after a failed call");
        host.load_source("fn spin() { loop {} }").unwrap();
        host.call(&mut world, "spin", None);
        assert_eq!(host.errors.len(), 2);
    }
}
