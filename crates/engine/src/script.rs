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

#[derive(Default)]
struct Ctx {
    world: WorldState,
    actions: Vec<Action>,
    player: PlayerInfo,
    /// The entity whose trigger runs the script (`SELF`).
    owner: Option<u32>,
    /// Characters on the current map that can be hurt: (uid, name, HP).
    characters: Vec<(u32, String, i32)>,
    /// Tool items' kinds (`can_craft` with tools).
    tool_kinds: crate::crafting::ToolKinds,
    /// The project's crafting recipes (`can_craft`).
    recipes: Vec<crate::crafting::RecipeDef>,
    /// Ids of the player's unlocked achievements (`achievement_unlocked`).
    achievements: std::collections::BTreeSet<String>,
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
}

pub struct ScriptHost {
    engine: Engine,
    ast: AST,
    ctx: Rc<RefCell<Ctx>>,
    pub errors: Vec<String>,
    /// Text printed by scripts since the last `take_printed` (the dev console shows it).
    printed: Rc<RefCell<Vec<String>>>,
}

impl ScriptHost {
    pub fn new() -> Self {
        let ctx = Rc::new(RefCell::new(Ctx::default()));
        let mut engine = Engine::new();
        engine.set_max_operations(2_000_000);
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
        ScriptHost { engine, ast: AST::empty(), ctx, errors: Vec::new(), printed }
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

    pub fn set_player(&mut self, info: PlayerInfo) {
        self.ctx.borrow_mut().player = info;
    }

    /// The current map's characters that can be hurt, as (uid, name, HP), for `hp_of()`.
    pub fn set_characters(&mut self, characters: Vec<(u32, String, i32)>) {
        self.ctx.borrow_mut().characters = characters;
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
        let Ok(rd) = std::fs::read_dir(dir) else { return (ast, errors) };
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rhai")).collect();
        files.sort();
        for f in files {
            match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|src| self.engine.compile(&src).map_err(|e| e.to_string())) {
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
    // say(speaker, "...", #{ mood: "worried" }): the line with a mood portrait of the speaker.
    fn say_with(c: &mut Ctx, speaker: String, text: &str, opts: &rhai::Map) {
        let mood = opts.get("mood").map(|m| m.to_string()).unwrap_or_default();
        if !mood.is_empty() {
            c.actions.push(Action::Mood { target: speaker.clone(), mood });
        }
        c.actions.push(Action::Dialog { speaker: speaker.clone(), text: text.into() });
        if opts.contains_key("mood") {
            c.actions.push(Action::Mood { target: speaker, mood: String::new() });
        }
    }
    engine.register_fn("say", with!(ctx, |c, speaker: &str, text: &str, opts: rhai::Map| say_with(&mut c, speaker.into(), text, &opts)));
    engine.register_fn("say", with!(ctx, |c, entity: i64, text: &str, opts: rhai::Map| say_with(&mut c, if entity < 0 { String::new() } else { format!("{{uid:{entity}}}") }, text, &opts)));
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
    // Skills and magic.
    engine.register_fn("skill", with!(ctx, |c, id: &str| c.world.skill_level(id) as i64));
    engine.register_fn("skill_points", with!(ctx, |c| c.world.skill_points as i64));
    engine.register_fn("knows_spell", with!(ctx, |c, id: &str| c.world.spells.contains(id)));
    engine.register_fn("mana", with!(ctx, |c| c.world.mana as f64));
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
    // Status effects.
    engine.register_fn("apply_effect", with!(ctx, |c, t: &str, kind: &str, s: Dynamic, secs: Dynamic| {
        if let Some(k) = effect_kind(kind) {
            c.actions.push(Action::ApplyEffect { target: t.into(), effect: crate::effects::EffectSpec::new(k, num(&s), num(&secs)) });
        }
    }));
    engine.register_fn("cure", with!(ctx, |c, t: &str, kind: &str| {
        if let Some(k) = effect_kind(kind) {
            c.actions.push(Action::CureEffect { target: t.into(), kind: Some(k) });
        }
    }));
    engine.register_fn("cure", with!(ctx, |c, t: &str| c.actions.push(Action::CureEffect { target: t.into(), kind: None })));
    engine.register_fn("has_effect", with!(ctx, |c, kind: &str| effect_kind(kind).is_some_and(|k| c.world.effects.iter().any(|e| e.kind == k))));
    engine.register_fn("set_skill", with!(ctx, |c, id: &str, l: i64| c.actions.push(Action::SetSkill { skill: id.into(), level: l.max(0) as u32 })));
    engine.register_fn("give_skill_points", with!(ctx, |c, n: i64| c.actions.push(Action::GiveSkillPoints { amount: n as i32 })));
    engine.register_fn("learn_spell", with!(ctx, |c, id: &str| c.actions.push(Action::LearnSpell { spell: id.into() })));
    engine.register_fn("add_mana", with!(ctx, |c, n: i64| c.actions.push(Action::AddMana { amount: n as i32 })));
    // (Numbers may be integers or floats in any mix.)
    engine.register_fn("damage_around", with!(ctx, |c, d: Dynamic, r: Dynamic| c.actions.push(Action::DamageAround { damage: num(&d).round() as i32, radius: num(&r) })));
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
        damage: num(&d).round() as i32
    })));
    engine.register_fn("is_night", with!(ctx, |c| crate::render::lighting::is_night(c.world.hour)));
    engine.register_fn("set_hour", with!(ctx, |c, h: f64| c.actions.push(Action::SetTime { hour: h as f32 })));
    engine.register_fn("set_hour", with!(ctx, |c, h: i64| c.actions.push(Action::SetTime { hour: h as f32 })));
    engine.register_fn("player_sex", with!(ctx, |c| c.world.player_sex.key().to_string()));
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
    // The HUD (status frame, clock, quick bar, buttons), e.g. for cut-scenes.
    engine.register_fn("hide_ui", with!(ctx, |c| c.actions.push(Action::SetUiVisible { visible: false })));
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
    engine.register_fn("heal_npc", with!(ctx, |c, t: &str, n: i64| c.actions.push(Action::HealCharacter { target: t.into(), amount: n as i32 })));
    engine.register_fn("set_locked", with!(ctx, |c, t: &str, locked: bool| c.actions.push(Action::SetLocked {
        target: t.into(),
        locked
    })));
    engine.register_fn("play_sound", with!(ctx, |c, id: &str| c.actions.push(Action::PlaySound { sound: id.into() })));
    engine.register_fn("play_music", with!(ctx, |c, id: &str| c.actions.push(Action::PlayMusic { sound: id.into() })));
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
/// An effect kind by its script name ("poison", "haste", ...); unknown names are logged.
fn effect_kind(name: &str) -> Option<crate::effects::EffectKind> {
    let k = crate::effects::EffectKind::from_id(name);
    if k.is_none() {
        let names: Vec<&str> = crate::effects::EffectKind::ALL.iter().map(|k| k.id()).collect();
        eprintln!("[script] unknown effect '{name}' (one of: {})", names.join(", "));
    }
    k
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
                Action::PlaySound { sound: "ding".into() },
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
                Action::DamageArea { x: 3.5, y: 7.0, radius: 2.0, damage: 4 },
            ]
        );
    }

    /// The examples in Documentation/scripting.md compile, and the Rhai primer does what it says.
    #[test]
    fn documentation_examples() {
        let doc = include_str!("../../../Documentation/scripting.md");
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
        w.companions.push(crate::state::Companion { map: "village".into(), uid: 22, name: "guard".into(), hp: None, down: None, order: Default::default(), post: None });
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
