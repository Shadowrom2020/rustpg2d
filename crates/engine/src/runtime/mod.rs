//! The game runtime: loads a project and plays it.

mod achievements;
mod activities;
mod ai;
mod charwin;
mod combat;
mod companion_ui;
mod companions;
mod cutscene;
mod dev;
mod effects;
mod equipment;
mod farm;
mod furniture;
mod gather;
mod healing;
mod fx;
mod inventory_ui;
mod journal;
mod juice;
mod skills;
mod lights;
mod creator;
mod patrol;
mod portraits;
mod schedule;
mod progression;
mod paperdoll;
mod npc_items;
mod npc_use;
mod shop_ui;
mod crafting_ui;
mod locks;
mod menu;
mod minigame;
mod orders;
pub mod input;
mod mouse;
mod spawner;
mod thumbs;
mod weather;
mod ui;
mod worldmap;

use std::collections::{HashSet, VecDeque};

use anyhow::{anyhow, Result};
use macroquad::prelude::*;

use crate::audio::Audio;
use crate::defs::{Behavior, Weapon};
use crate::entity::{EntityKind, Facing};
use input::Act;
use crate::events::runner::{conditions_met, ActionCtx, ActionHost, ActionRunner, Step};
use crate::events::{Action, GameEvent, Trigger};
use crate::map::transition::edge_destination;
use crate::map::{Edge, EdgeLink, Map, ZoneKind};
use crate::project::Project;
use crate::render::{self, Assets};
use crate::script::ScriptHost;
use crate::state::{EntityState, WorldState};
use combat::{PlayerCombat, Projectile};
use mouse::Order;
use spawner::SpawnerRt;
use ui::Ui;

/// Callback collecting `(once-key, triggers, owner)` while gathering event handlers.
type TriggerSink<'a> = dyn FnMut(String, &[Trigger], Option<u32>) + 'a;

/// How many recently left maps are kept in memory.
const MAP_CACHE: usize = 4;
const FADE_TIME: f32 = 0.25;
/// Save slot of the autosaves (after map changes and the `Autosave` action).
pub(super) const AUTOSAVE: &str = "autosave";

#[derive(Clone, Debug)]
pub enum Arrival {
    Spawn(String),
    /// Next to the named zone (the far end of a map transition).
    Beside(String),
    Pos(Vec2),
    Edge { edge: Edge, pos: Vec2, link: EdgeLink },
}

/// Overrides where the game starts (used by the editor's play-test).
#[derive(Clone, Debug, Default)]
pub struct StartOptions {
    pub map: Option<String>,
    pub spawn: Option<String>,
    pub pos: Option<Vec2>,
    /// Play this script function as a cut-scene right after arriving (instead of the map's
    /// *Map enter* triggers): previews from the editor.
    pub cutscene: Option<String>,
    /// Show the title screen (main menu) first.
    pub title: bool,
    /// Developer play-test (`--dev`, started by the editor): script hot reload and the F4
    /// console.
    pub dev: bool,
}

/// A moving thing: the player or a runtime entity.
#[derive(Clone, Debug)]
struct Body {
    pos: Vec2,
    facing: Facing,
    hitbox: Vec2,
    speed: f32,
    moving: bool,
    anim: f32,
    /// Scripted movement target (ignores collision).
    move_target: Option<Vec2>,
}

impl Body {
    fn rect_at(&self, p: Vec2) -> Rect {
        Rect::new(p.x - self.hitbox.x / 2.0, p.y - self.hitbox.y, self.hitbox.x, self.hitbox.y)
    }
    fn rect(&self) -> Rect {
        self.rect_at(self.pos)
    }
}

struct RtEntity {
    uid: u32,
    kind: EntityKind,
    def: String,
    name: String,
    body: Body,
    home: Vec2,
    active: bool,
    /// Legacy frame offset added to the state's sprite (SetFrame action).
    frame: u32,
    /// Current state of a prop (index into its definition's states; 0 = default).
    state: usize,
    solid: bool,
    behavior: Behavior,
    wander_timer: f32,
    wander_dir: Vec2,
    // Combat (max_hp 0 = can't be damaged).
    hp: i32,
    max_hp: i32,
    hostile: bool,
    damage: i32,
    invuln: f32,
    flash: f32,
    /// What it does all day where it stands (`EntityInstance::activity`), the way it faces
    /// while doing it, seconds a talk still pauses it, the last work stroke (its sound).
    activity: Option<crate::entity::Activity>,
    act_facing: Facing,
    act_pause: f32,
    act_cycle: u32,
    /// Using furniture (a schedule stop with a prop).
    npc_seat: Option<furniture::NpcSeat>,
    /// A visitor from another map (its schedule brought it here).
    visitor: Option<Box<schedule::Visitor>>,
    /// A seed growing on a farm plot.
    planted: Option<crate::state::Planted>,
    /// When its prop state last changed (in-game hours; regrowing states).
    changed_at: Option<f64>,
    /// Hits a gathering prop has taken (and seconds since the last one).
    gather: Option<(u32, f32)>,
    /// The placed character's own portrait (`EntityInstance::portrait`).
    portrait: Option<crate::defs::SpriteRef>,
    /// HP the last hit took (a character drinks a potion before the next one would fell it).
    last_hit: i32,
    /// Seconds before it uses another item it carries.
    item_cd: f32,
    knock: Vec2,
    /// Ranged attack for hostile shooters.
    weapon: Option<Weapon>,
    attack_cd: f32,
    /// Spawner zone uid for characters created by a spawner (not saved).
    spawned_by: Option<u32>,
    /// Stack size of item pickups.
    count: u32,
    /// Subtracted from damage taken.
    defense: i32,
    /// Its key lock has been opened.
    unlocked: bool,
    /// Patrol route and progress (characters with a route).
    patrol: Option<patrol::Patrol>,
    /// Daily routine (placed characters with a schedule).
    schedule: Option<schedule::Schedule>,
    /// Gone indoors at a schedule stop: inactive until the next stop (saved as active).
    away: bool,
    /// It follows the player (where it is placed, its triggers, its way).
    companion: Option<Box<companions::Link>>,
    /// What the character drops when defeated.
    loot: Vec<crate::defs::LootEntry>,
    /// Light of this placed entity instead of its definition's.
    light: Option<crate::defs::LightSpec>,
    /// Characters: the model (its definition's, or the placed character's own).
    appearance: crate::defs::Appearance,
    /// Characters: what they wear and hold, and carry (dropped when defeated).
    equipment: crate::state::Equipment,
    inventory: Vec<crate::inventory::ItemStack>,
    /// Running status effects (not saved, like the HP).
    effects: Vec<crate::effects::ActiveEffect>,
    /// Chasing characters: hunting, returning home, fleeing (not saved).
    ai: ai::Ai,
}

struct Loaded {
    map: Map,
    solid: Vec<bool>,
    entities: Vec<RtEntity>,
    /// Lights of light-giving tiles (computed when the map is entered).
    tile_lights: Vec<render::lighting::Light>,
}

enum Fade {
    None,
    Out { t: f32, map: String, arrival: Arrival },
    In { t: f32 },
}

/// Which owner's triggers an event is delivered to.
#[derive(Clone, Debug)]
enum Source {
    Entity(u32),
    Zone(u32),
    Map,
    Item(String),
    /// Custom events: map, all zones and all active entities.
    Broadcast,
}

pub struct Game {
    pub project: Project,
    pub assets: Assets,
    pub audio: Audio,
    combat: PlayerCombat,
    /// Screen shake and the hurt flash.
    juice: juice::Juice,
    /// Script hot reload and the console (`--dev`).
    dev: dev::Dev,
    /// A save waits for its thumbnail (captured while drawing), and what waits for that.
    thumb_request: Option<String>,
    after_thumb: Option<thumbs::AfterThumb>,
    /// Seconds since the player was defeated.
    game_over: Option<f32>,
    /// Short-lived visual effects: (position, age).
    effects: Vec<(Vec2, f32)>,
    projectiles: Vec<Projectile>,
    /// Scripted effects (Fly / Effect actions).
    fx: Vec<fx::Fx>,
    /// Lights at a point for a while (Light at point action).
    timed_lights: Vec<fx::TimedLight>,
    /// Click-to-move / click-to-act order.
    order: Option<Order>,
    /// Where the last click landed (marker, age).
    click_marker: Option<(Vec2, f32)>,
    spawners: Vec<SpawnerRt>,
    next_spawn_uid: u32,
    mouse: mouse::MouseState,
    inv: inventory_ui::InvUi,
    shop: shop_ui::ShopUi,
    crafting: crafting_ui::CraftingUi,
    /// The player's achievements, their checks and toasts.
    ach: achievements::Achievements,
    /// Lockpicking / fishing in progress (pauses the world).
    minigame: Option<minigame::Minigame>,
    /// Whose portrait the open dialog shows.
    portrait: Option<portraits::Speaker>,
    /// The furniture the player uses, the sleep sequence, and the bed a Sleep action lies in.
    seat: Option<furniture::Seat>,
    /// A map change waiting for the answer to "Leave … behind?" (knocked-down companions).
    leaving: Option<(String, Arrival)>,
    /// Seconds before leaving the map may ask again (after "Stay").
    leave_hold: f32,
    /// A companion window to open once the running event is over (`Action::OpenCompanion`).
    pending_companion: Option<u32>,
    /// Scheduled characters that visit other maps (found in all maps once).
    visitor_defs: Option<Vec<schedule::VisitorDef>>,
    /// Moods of characters' portraits (definition id -> mood) while an event runs.
    moods: std::collections::HashMap<String, String>,
    /// The open "Heal whom?" choice.
    heal_pick: Option<healing::HealPick>,
    /// Chips flying off gathered props.
    chips: Vec<gather::Chip>,
    /// Seconds to the next check of regrowing props.
    regrow_timer: f32,
    /// The open companion orders menu (G).
    order_menu: Option<orders::OrderMenu>,
    /// The open companion window (its gear and bag).
    comp_win: Option<companion_ui::CompanionWin>,
    /// Bottom of the player's HUD panel as last drawn (the companions' rows go below it).
    hud_bottom: std::cell::Cell<f32>,
    sleeping: Option<furniture::Sleeping>,
    bed: Option<u32>,
    journal: journal::Journal,
    worldmap: worldmap::WorldMap,
    creator: Option<creator::Creator>,
    menu: menu::Menu,
    pub world: WorldState,
    scripts: ScriptHost,
    runner: ActionRunner,
    /// Darkness and light renderer (created on first use).
    light_pass: Option<render::lighting::LightPass>,
    /// Rain / snow / fog particles.
    weather_fx: render::weather::WeatherFx,
    skills: skills::Skills,
    /// Set while a saved game is being loaded (see `enter_map`).
    resuming: bool,
    /// An autosave is due (after a map change, or `Autosave`): written once no event runs.
    autosave_pending: bool,
    /// The player's options (Options menu), saved per game in the user's config folder.
    pub settings: crate::settings::Settings,
    /// Fullscreen state last applied (None = not yet).
    fullscreen: Option<bool>,
    loaded: Loaded,
    cache: VecDeque<Map>,
    player: Body,
    zones_inside: HashSet<u32>,
    fade: Fade,
    ui: Ui,
    pub debug: bool,
    pending_events: Vec<(GameEvent, Source)>,
    /// Save slot the current game was last saved to / loaded from ("Save and quit" uses it).
    slot: Option<String>,
    /// The HUD is hidden (H, or a script / action); not saved.
    ui_hidden: bool,
    cutscene: cutscene::CutsceneState,
}

impl Game {
    pub fn new(project: Project, start: StartOptions) -> Result<Game> {
        if project.config.tile_size != crate::project::TILE_SIZE {
            anyhow::bail!(
                "this project uses {}px tiles; open it once in the R(ust)PG2D editor to convert it to {}px tiles",
                project.config.tile_size,
                crate::project::TILE_SIZE
            );
        }
        let assets = Assets::load(&project);
        let audio = Audio::load(&project);
        let combat = PlayerCombat::new(project.db.character(&project.config.player));
        let mut scripts = ScriptHost::new();
        scripts.load_dir(&project.scripts_dir());
        let ts = assets.tile_size;
        let (hitbox, speed) = match project.db.character(&project.config.player) {
            Some(c) => (vec2(c.hitbox[0], c.hitbox[1]), c.speed),
            None => (vec2(ts * 0.75, ts * 0.5), 70.0),
        };
        let map_name = start.map.clone().unwrap_or_else(|| project.config.start_map.clone());
        let map = project.load_map(&map_name)?;
        let player = Body {
            pos: Vec2::ZERO,
            facing: Facing::Down,
            hitbox,
            speed,
            moving: false,
            anim: 0.0,
            move_target: None,
        };
        let mut game = Game {
            light_pass: None,
            weather_fx: Default::default(),
            skills: Default::default(),
            resuming: false,
            autosave_pending: false,
            settings: crate::settings::Settings::default(),
            juice: Default::default(),
            dev: dev::Dev::new(start.dev, &project.scripts_dir()),
            thumb_request: None,
            after_thumb: None,
            fullscreen: None,
            loaded: Loaded { map: Map::new("", 1, 1), solid: vec![], entities: vec![], tile_lights: vec![] },
            project,
            assets,
            audio,
            combat,
            game_over: None,
            effects: Vec::new(),
            projectiles: Vec::new(),
            fx: Vec::new(),
            timed_lights: Vec::new(),
            order: None,
            click_marker: None,
            spawners: Vec::new(),
            next_spawn_uid: spawner::SPAWN_UID_BASE,
            mouse: Default::default(),
            inv: Default::default(),
            shop: Default::default(),
            crafting: Default::default(),
            ach: Default::default(),
            minigame: None,
            portrait: None,
            seat: None,
            leaving: None,
            leave_hold: 0.0,
            pending_companion: None,
            visitor_defs: None,
            moods: Default::default(),
            heal_pick: None,
            chips: Vec::new(),
            regrow_timer: 0.0,
            order_menu: None,
            comp_win: None,
            hud_bottom: std::cell::Cell::new(80.0),
            sleeping: None,
            bed: None,
            journal: Default::default(),
            worldmap: Default::default(),
            creator: None,
            menu: Default::default(),
            world: WorldState::default(),
            scripts,
            runner: ActionRunner::default(),
            cache: VecDeque::new(),
            player,
            zones_inside: HashSet::new(),
            fade: Fade::In { t: FADE_TIME },
            ui: Ui::default(),
            debug: false,
            pending_events: Vec::new(),
            slot: None,
            ui_hidden: false,
            cutscene: Default::default(),
        };
        let arrival = match (start.pos, start.spawn) {
            (Some(p), _) => Arrival::Pos(p),
            (None, Some(s)) => Arrival::Spawn(s),
            (None, None) if start.map.is_some() => Arrival::Spawn(String::new()),
            _ => Arrival::Spawn(game.project.config.start_spawn.clone()),
        };
        game.scripts.set_recipes(game.project.db.recipes.clone(), game.project.db.tool_kinds());
        game.load_achievements();
        game.settings = crate::settings::Settings::load(&game.project.config.name);
        game.apply_settings();
        game.world.player_hp = Some(game.combat.max_hp);
        game.world.hour = game.project.config.start_hour;
        game.prepare_world();
        if start.title {
            game.open_title();
        }
        if let Some(f) = &start.cutscene {
            // A preview: the cut-scene instead of the map's Map enter triggers.
            game.resuming = true;
            game.runner.push(&[Action::Cutscene { function: f.clone() }], &ActionCtx::default());
        }
        game.enter_map(map, arrival);
        Ok(game)
    }

    fn ts(&self) -> f32 {
        self.assets.tile_size
    }

    /// One "art pixel" of the classic 16px tile grid: world-space sizes of effects, markers
    /// and knockback are given in these so they keep their proportions at any tile size.
    fn u(&self) -> f32 {
        self.ts() / 16.0
    }

    // ---------------------------------------------------------------- maps

    fn take_map(&mut self, name: &str) -> Result<Map> {
        if let Some(i) = self.cache.iter().position(|m| m.name == name) {
            return Ok(self.cache.remove(i).unwrap());
        }
        self.project.load_map(name)
    }

    /// Builds the runtime state of an entity from its definition.
    #[allow(clippy::too_many_arguments)]
    fn new_rt_entity(
        &self,
        kind: EntityKind,
        def: &str,
        uid: u32,
        name: &str,
        pos: Vec2,
        facing: Facing,
        active: bool,
        frame: u32,
    ) -> RtEntity {
        let ts = self.ts();
        let (hitbox, speed, solid, behavior) = match kind {
            EntityKind::Prop => match self.project.db.prop(def) {
                Some(d) => (vec2(d.hitbox[0], d.hitbox[1]), 0.0, d.solid, Behavior::Static),
                None => (vec2(ts, ts), 0.0, false, Behavior::Static),
            },
            EntityKind::Character => match self.project.db.character(def) {
                Some(d) => (vec2(d.hitbox[0], d.hitbox[1]), d.speed, true, d.behavior.clone()),
                None => (vec2(ts * 0.75, ts * 0.5), 50.0, true, Behavior::Static),
            },
            EntityKind::Item => (vec2(ts * 0.75, ts * 0.75), 0.0, false, Behavior::Static),
        };
        let (max_hp, hostile, damage, weapon, defense) = match (kind, self.project.db.character(def)) {
            (EntityKind::Character, Some(d)) => (d.max_hp.max(0), d.hostile, d.damage.max(0), d.weapon.clone(), d.defense),
            _ => (0, false, 0, None, 0),
        };
        let mut rt = RtEntity {
            uid,
            kind,
            def: def.to_string(),
            name: name.to_string(),
            body: Body { pos, facing, hitbox, speed, moving: false, anim: 0.0, move_target: None },
            home: pos,
            active,
            frame,
            // Hostiles overlap the player (contact damage) instead of blocking.
            solid: solid && !hostile,
            behavior,
            wander_timer: rand::gen_range(0.5, 2.0),
            wander_dir: Vec2::ZERO,
            hp: max_hp,
            max_hp,
            hostile,
            damage,
            invuln: 0.0,
            last_hit: 0,
            portrait: None,
            changed_at: None,
            gather: None,
            planted: None,
            npc_seat: None,
            visitor: None,
            activity: None,
            act_facing: facing,
            act_pause: 0.0,
            act_cycle: 0,
            item_cd: 0.0,
            flash: 0.0,
            knock: Vec2::ZERO,
            weapon,
            attack_cd: rand::gen_range(0.5, 1.5),
            spawned_by: None,
            count: 1,
            defense,
            unlocked: false,
            state: 0,
            patrol: None,
            schedule: None,
            away: false,
            companion: None,
            loot: match (kind, self.project.db.character(def)) {
                (EntityKind::Character, Some(d)) => d.loot.clone(),
                _ => Vec::new(),
            },
            light: None,
            appearance: self.project.db.character(def).filter(|_| kind == EntityKind::Character).map(|d| d.appearance.clone()).unwrap_or_default(),
            equipment: self.project.db.character(def).filter(|_| kind == EntityKind::Character).map(|d| d.equipment.clone()).unwrap_or_default(),
            inventory: self.project.db.character(def).filter(|_| kind == EntityKind::Character).map(|d| d.inventory.clone()).unwrap_or_default(),
            effects: Vec::new(),
            ai: ai::Ai::default(),
        };
        npc_items::apply_gear(&self.project.db, &mut rt);
        rt
    }

    /// Switches prop `i` to the named state (sprite + solidity) and saves it.
    fn set_prop_state(&mut self, i: usize, name: &str) -> bool {
        let e = &self.loaded.entities[i];
        let Some(d) = self.project.db.prop(&e.def) else { return false };
        let Some(idx) = d.state_index(name) else {
            eprintln!("[game] prop '{}' has no state '{name}'", e.def);
            return false;
        };
        let solid = d.state_look(idx).1;
        let now = self.world.total_hours();
        let e = &mut self.loaded.entities[i];
        e.state = idx;
        e.frame = 0;
        e.solid = solid;
        e.changed_at = Some(now);
        self.persist(i);
        true
    }

    fn build_entities(&mut self, map: &Map) -> Vec<RtEntity> {
        let mut out = self.build_map_entities(map);
        out.extend(self.build_companions());
        out.extend(self.build_visitors(&map.name));
        // Items the player dropped here earlier.
        for p in self.world.dropped.get(&map.name).into_iter().flatten() {
            let mut rt = self.new_rt_entity(EntityKind::Item, &p.item, p.uid, "", vec2(p.x, p.y), Facing::Down, true, 0);
            rt.count = p.count;
            out.push(rt);
        }
        out
    }

    fn build_map_entities(&self, map: &Map) -> Vec<RtEntity> {
        // Recruited companions are built apart (they follow the player).
        map.entities.iter().filter(|e| !self.world.is_companion(&map.name, e.uid)).map(|e| self.build_placed(&map.name, e)).collect()
    }

    /// The runtime entity of placed entity `e` of map `map`, with what changed in this game.
    fn build_placed(&self, map: &str, e: &crate::entity::EntityInstance) -> RtEntity {
        let st = self.world.entity_state(map, e.uid);
        let pos = st.map_or(vec2(e.x, e.y), |s| vec2(s.x, s.y));
        let count = st.and_then(|s| s.count).unwrap_or(e.count);
        let mut rt = self.new_rt_entity(
            e.kind,
            &e.def,
            e.uid,
            &e.name,
            pos,
            st.map_or(e.facing, |s| s.facing),
            st.map_or(e.active, |s| s.active),
            st.map_or(e.frame, |s| s.frame),
        );
        rt.count = count;
        rt.changed_at = st.and_then(|s| s.changed_at);
        rt.planted = st.and_then(|s| s.planted.clone());
        rt.light = e.light.clone();
        rt.portrait = e.portrait.clone();
        rt.activity = e.activity.clone();
        // (Working all day, it faces the way it is placed, whatever it faced when saved.)
        rt.act_facing = e.facing;
        if e.kind == EntityKind::Character {
            if let Some(loot) = &e.loot {
                rt.loot = loot.clone();
            }
            rt.patrol = e.patrol.clone().map(patrol::Patrol::new);
            // Look and items: the placed NPC's own, then what changed in this game.
            if let Some(a) = &e.appearance {
                rt.appearance = a.clone();
            }
            if let Some(eq) = &e.equipment {
                rt.equipment = eq.clone();
            }
            if let Some(inv) = &e.inventory {
                rt.inventory = inv.clone();
            }
            if let Some(saved) = self.world.npc_items.get(map).and_then(|m| m.get(&e.uid)) {
                rt.equipment = saved.equipment.clone();
                rt.inventory = saved.inventory.clone();
            }
            npc_items::apply_gear(&self.project.db, &mut rt);
            if let Some(ps) = st.and_then(|s| s.patrol.as_ref()) {
                let p = rt.patrol.get_or_insert_with(Default::default);
                p.restore(ps);
            }
            if !e.schedule.is_empty() {
                let mut s = schedule::Schedule::new(e.schedule.clone(), map);
                s.on = !st.is_some_and(|s| s.schedule_off);
                rt.schedule = Some(s);
            }
        }
        rt.unlocked = st.is_some_and(|s| s.unlocked);
        if let Some(d) = (e.kind == EntityKind::Prop).then(|| self.project.db.prop(&e.def)).flatten() {
            // Saved state; saves from before named states store a frame offset.
            let name = match st {
                Some(s) => match &s.state {
                    Some(n) => Some(n.clone()),
                    None => d.state_of_offset(s.frame),
                },
                None => Some(e.state.clone()),
            };
            if let Some(idx) = name.and_then(|n| d.state_index(&n)) {
                rt.state = idx;
                if st.is_none_or(|s| s.state.is_some() || d.state_of_offset(s.frame).is_some()) {
                    rt.frame = 0;
                }
                rt.solid = d.state_look(idx).1;
            }
        }
        rt
    }

    fn enter_map(&mut self, map: Map, arrival: Arrival) {
        // Off any furniture (it belongs to the old map).
        self.seat = None;
        // Patrolling characters continue where they were when the player comes back.
        self.persist_patrols();
        let ts = self.ts();
        let solid = self.assets.compute_solid(&map);
        let entities = self.build_entities(&map);
        let tile_lights = render::lighting::tile_lights(&map, &self.project.db, ts);
        let old = std::mem::replace(&mut self.loaded, Loaded { map, solid, entities, tile_lights });
        self.restore_unlocked();
        // Scheduled characters are where their day has taken them.
        for i in 0..self.loaded.entities.len() {
            self.place_on_schedule(i);
        }
        if !old.map.name.is_empty() && old.map.name != self.loaded.map.name {
            self.cache.push_front(old.map);
            self.cache.truncate(MAP_CACHE);
        }
        let m = &self.loaded.map;
        let center = vec2(m.width as f32 * ts / 2.0, m.height as f32 * ts / 2.0);
        let pos = match &arrival {
            Arrival::Pos(p) => *p,
            Arrival::Spawn(name) => {
                let spawn = m.spawn_point(name).or_else(|| {
                    m.zones.iter().find(|z| matches!(z.kind, ZoneKind::Spawn))
                });
                if spawn.is_none() {
                    eprintln!("[game] map '{}' has no spawn '{name}', using center", m.name);
                }
                spawn.map_or(center, |z| z.arrival_point(ts))
            }
            Arrival::Edge { edge, pos, link } => {
                edge_destination(*edge, *pos, link, m.width, m.height, &m.edges, ts)
            }
            Arrival::Beside(name) => match m.zones.iter().find(|z| z.name == *name) {
                Some(z) => z.beside_point(m.width, m.height, ts),
                None => {
                    eprintln!("[game] map '{}' has no transition zone '{name}', using center", m.name);
                    center
                }
            },
        };
        // Never inside something solid (a gate post at a map edge, a prop placed on a spawn).
        self.player.pos = self.free_spot(pos);
        self.place_companions();
        self.player.move_target = None;
        self.world.current_map = self.loaded.map.name.clone();
        // Zones we arrive in don't fire Enter (avoids bouncing back through portals).
        self.zones_inside = self.zones_at(self.player.pos);
        self.effects.clear();
        self.projectiles.clear();
        self.fx.clear();
        self.timed_lights.clear();
        self.order = None;
        self.setup_spawners();
        // Veins and crops that regrew while the player was away.
        self.update_regrow();
        let music = self.loaded.map.music.clone();
        if !music.is_empty() {
            self.audio.play_music(&music);
        }
        // Loading a saved game resumes it: the player didn't enter the map, so its "Map enter"
        // triggers don't run again (their effects are part of the save).
        if std::mem::take(&mut self.resuming) {
            self.pending_events.retain(|(e, _)| !matches!(e, GameEvent::MapEnter));
        } else {
            self.pending_events.push((GameEvent::MapEnter, Source::Map));
        }
    }

    /// Starts a fade-out; the map switches when the screen is black.
    pub fn transition(&mut self, map: &str, arrival: Arrival) {
        if matches!(self.fade, Fade::Out { .. }) || self.leaving.is_some() {
            return;
        }
        // Knocked-down companions can't come along: ask first (once; "Stay" holds still a moment
        // so walking into the map's edge doesn't ask again at once).
        // (Loading a save isn't leaving: the companions come from the save.)
        let leaving_map = !map.is_empty() && map != self.loaded.map.name && !self.resuming;
        if leaving_map && !self.downed_companions().is_empty() {
            if self.leave_hold > 0.0 {
                return;
            }
            self.ask_leave_behind(map, arrival);
            return;
        }
        if leaving_map {
            self.companions_follow_again();
        }
        self.fade = Fade::Out { t: 0.0, map: map.to_string(), arrival };
    }

    fn finish_transition(&mut self, map_name: String, arrival: Arrival) {
        if map_name.is_empty() || map_name == self.loaded.map.name {
            // Same map: just reposition.
            let ts = self.ts();
            let m = &self.loaded.map;
            self.player.pos = match &arrival {
                Arrival::Pos(p) => *p,
                Arrival::Spawn(s) => m.spawn_point(s).map_or(self.player.pos, |z| z.arrival_point(ts)),
                Arrival::Beside(n) => m.zones.iter().find(|z| z.name == *n).map_or(self.player.pos, |z| z.beside_point(m.width, m.height, ts)),
                Arrival::Edge { .. } => self.player.pos,
            };
            self.zones_inside = self.zones_at(self.player.pos);
            return;
        }
        // A real map change (not loading a save) is autosaved once its Map enter triggers ran.
        if !self.resuming && !self.on_title() {
            self.autosave_pending = true;
        }
        match self.take_map(&map_name) {
            Ok(map) => self.enter_map(map, arrival),
            Err(e) => {
                eprintln!("[game] cannot load map '{map_name}': {e:#}");
                self.ui.toast(format!("Missing map: {map_name}"));
            }
        }
    }

    // ---------------------------------------------------------------- collision

    fn tile_solid(&self, tx: i32, ty: i32) -> bool {
        let m = &self.loaded.map;
        if !m.in_bounds(tx, ty) {
            return true;
        }
        self.loaded.solid[(ty as u32 * m.width + tx as u32) as usize]
    }

    /// Tiles taken by active solid entities that pass `keep` (any real overlap of the hitbox
    /// blocks a tile: small hitboxes sit at the bottom of their tile). For path finding.
    fn solid_entity_tiles(&self, keep: impl Fn(&RtEntity) -> bool) -> std::collections::HashSet<crate::path::TilePos> {
        let ts = self.ts();
        let mut occupied = std::collections::HashSet::new();
        for e in self.loaded.entities.iter().filter(|e| e.active && e.solid && keep(e)) {
            let r = e.body.rect();
            for ty in (r.y / ts).floor() as i32..=((r.y + r.h) / ts).floor() as i32 {
                for tx in (r.x / ts).floor() as i32..=((r.x + r.w) / ts).floor() as i32 {
                    let inner = Rect::new(tx as f32 * ts + 2.0, ty as f32 * ts + 2.0, ts - 4.0, ts - 4.0);
                    if r.overlaps(&inner) {
                        occupied.insert((tx, ty));
                    }
                }
            }
        }
        occupied
    }

    fn rect_blocked(&self, r: Rect, ignore: Option<u32>, check_player: bool) -> bool {
        let ts = self.ts();
        let x0 = (r.x / ts).floor() as i32;
        let y0 = (r.y / ts).floor() as i32;
        let x1 = ((r.x + r.w - 0.001) / ts).floor() as i32;
        let y1 = ((r.y + r.h - 0.001) / ts).floor() as i32;
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                if self.tile_solid(tx, ty) {
                    return true;
                }
            }
        }
        if check_player && self.player.rect().overlaps(&r) {
            return true;
        }
        self.loaded
            .entities
            .iter()
            .any(|e| e.active && e.solid && Some(e.uid) != ignore && e.body.rect().overlaps(&r))
    }

    /// Where the player can stand near `pos`: `pos` itself if the player's hitbox is free there,
    /// else the nearest spot whole tiles away (searching rings around it) that is free.
    fn free_spot(&self, pos: Vec2) -> Vec2 {
        let ts = self.ts();
        let fits = |p: Vec2| !self.rect_blocked(self.player.rect_at(p), None, false);
        if fits(pos) {
            return pos;
        }
        for r in 1..=12i32 {
            let mut ring: Vec<Vec2> = (-r..=r)
                .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
                .filter(|(dx, dy)| dx.abs().max(dy.abs()) == r)
                .map(|(dx, dy)| pos + vec2(dx as f32, dy as f32) * ts)
                .collect();
            ring.sort_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
            if let Some(p) = ring.into_iter().find(|p| fits(*p)) {
                eprintln!("[game] arrival at {:.0},{:.0} on {} is blocked; moved to {:.0},{:.0}", pos.x, pos.y, self.loaded.map.name, p.x, p.y);
                return p;
            }
        }
        eprintln!("[game] no free spot near {:.0},{:.0} on {}", pos.x, pos.y, self.loaded.map.name);
        pos
    }

    /// Moves along each axis separately so actors slide along walls.
    fn try_move(&self, body: &Body, delta: Vec2, ignore: Option<u32>, check_player: bool) -> Vec2 {
        let mut p = body.pos;
        let nx = p + vec2(delta.x, 0.0);
        if delta.x != 0.0 && !self.rect_blocked(body.rect_at(nx), ignore, check_player) {
            p = nx;
        }
        let ny = p + vec2(0.0, delta.y);
        if delta.y != 0.0 && !self.rect_blocked(body.rect_at(ny), ignore, check_player) {
            p = ny;
        }
        p
    }

    fn zones_at(&self, p: Vec2) -> HashSet<u32> {
        let ts = self.ts();
        self.loaded
            .map
            .zones
            .iter()
            .filter(|z| z.pixel_rect(ts).contains(p))
            .map(|z| z.uid)
            .collect()
    }

    // ---------------------------------------------------------------- events

    fn entity_index(&self, target: &str, owner: Option<u32>) -> Option<usize> {
        let uid = match target {
            "" | "self" => owner?,
            // Active ones first (a companion that left this map stays behind, inactive).
            name => return self.loaded.entities.iter().position(|e| e.name == name && e.active).or_else(|| self.loaded.entities.iter().position(|e| e.name == name)),
        };
        self.loaded.entities.iter().position(|e| e.uid == uid)
    }

    /// Collects actions of all triggers reacting to `ev` from `source`.
    fn collect(&mut self, ev: &GameEvent, source: &Source) -> Vec<(Action, ActionCtx)> {
        let map = &self.loaded.map;
        let mut candidates: Vec<(String, Trigger, Option<u32>)> = Vec::new();
        let mut add = |key: String, triggers: &[Trigger], owner: Option<u32>| {
            for (i, t) in triggers.iter().enumerate() {
                if t.on.matches(ev) {
                    candidates.push((format!("{key}/{i}"), t.clone(), owner));
                }
            }
        };
        let db = &self.project.db;
        let rt = &self.loaded.entities;
        let add_entity = |uid: u32, add: &mut TriggerSink| {
            if let Some(e) = rt.iter().find(|e| e.uid == uid) {
                add(format!("{}/def:{}", map.name, e.uid), db.def_triggers(e.kind, &e.def), Some(uid));
            }
            if let Some(e) = map.entities.iter().find(|e| e.uid == uid) {
                add(format!("{}/e:{}", map.name, e.uid), &e.triggers, Some(uid));
            }
            // A companion's own triggers come along from where it is placed; so do a visitor's.
            if let Some(l) = rt.iter().find(|e| e.uid == uid).and_then(|e| e.companion.as_ref()) {
                add(format!("{}/e:{}", l.map, l.uid), &l.triggers, Some(uid));
            }
            if let Some(v) = rt.iter().find(|e| e.uid == uid).and_then(|e| e.visitor.as_ref()) {
                add(format!("{}/e:{}", v.map, v.uid), &v.triggers, Some(uid));
            }
        };
        match source {
            Source::Entity(uid) => add_entity(*uid, &mut add),
            Source::Zone(uid) => {
                if let Some(z) = map.zones.iter().find(|z| z.uid == *uid) {
                    add(format!("{}/z:{}", map.name, z.uid), &z.triggers, None);
                }
            }
            Source::Map => add(format!("{}/map", map.name), &map.triggers, None),
            Source::Item(id) => {
                if let Some(d) = db.item(id) {
                    add(format!("item:{id}"), &d.on_use, None);
                }
            }
            Source::Broadcast => {
                add(format!("{}/map", map.name), &map.triggers, None);
                for z in &map.zones {
                    add(format!("{}/z:{}", map.name, z.uid), &z.triggers, None);
                }
                let active: Vec<u32> =
                    self.loaded.entities.iter().filter(|e| e.active).map(|e| e.uid).collect();
                for uid in active {
                    add_entity(uid, &mut add);
                }
            }
        }

        let mut out = Vec::new();
        for (key, trigger, owner) in candidates {
            if trigger.once && self.world.fired.contains(&key) {
                continue;
            }
            let ctx = ActionCtx { owner };
            if !conditions_met(self, &trigger.conditions, &ctx) {
                continue;
            }
            if trigger.once {
                self.world.fired.insert(key);
            }
            out.extend(trigger.actions.into_iter().map(|a| (a, ctx.clone())));
        }
        out
    }

    fn fire(&mut self, ev: GameEvent, source: Source) -> bool {
        let actions = self.collect(&ev, &source);
        let any = !actions.is_empty();
        for (a, ctx) in actions {
            self.runner.push(&[a], &ctx);
        }
        any
    }

    /// Display name of an entity on this map: its definition's name (character, prop, item).
    fn entity_display_name(&self, uid: u32) -> Option<String> {
        let e = self.loaded.entities.iter().find(|e| e.uid == uid)?;
        let db = &self.project.db;
        match e.kind {
            EntityKind::Character => db.character(&e.def).map(|d| d.name.clone()),
            EntityKind::Prop => db.prop(&e.def).map(|d| d.name.clone()),
            EntityKind::Item => db.item(&e.def).map(|d| d.name.clone()),
        }
    }

    /// Replaces `{self}` (the trigger's owner) and `{uid:N}` (from `say(SELF, ...)` in
    /// scripts) with entity names, e.g. in dialog speakers.
    fn entity_names_in(&self, text: &str, owner: Option<u32>) -> String {
        if !text.contains('{') {
            return text.to_string();
        }
        let mut out = text.replace("{self}", &owner.and_then(|u| self.entity_display_name(u)).unwrap_or_default());
        while let Some(i) = out.find("{uid:") {
            let Some(j) = out[i..].find('}') else { break };
            let uid = out[i + 5..i + j].parse::<u32>().ok();
            let name = uid.and_then(|u| self.entity_display_name(u)).unwrap_or_default();
            out.replace_range(i..i + j + 1, &name);
        }
        out
    }

    fn persist(&mut self, idx: usize) {
        let e = &self.loaded.entities[idx];
        if e.spawned_by.is_some() || e.companion.is_some() || e.visitor.is_some() {
            return; // spawner characters are not saved individually, companions in `companions`, visitors follow their schedule
        }
        if e.uid >= crate::state::DROPPED_UID_BASE {
            // Dropped items live in the world state's list of piles.
            let (map, uid, count) = (self.loaded.map.name.clone(), e.uid, if e.active { e.count } else { 0 });
            self.world.set_dropped_count(&map, uid, count);
            return;
        }
        let state = (e.kind == EntityKind::Prop).then(|| self.project.db.prop(&e.def)).flatten().map(|d| d.state_name(e.state).to_string());
        let st = EntityState {
            // Indoors at a schedule stop is not inactive (the schedule brings it out again).
            active: e.active || e.away,
            frame: e.frame,
            x: e.body.pos.x,
            y: e.body.pos.y,
            facing: e.body.facing,
            count: (e.kind == EntityKind::Item).then_some(e.count),
            unlocked: e.unlocked,
            state,
            patrol: e.patrol.as_ref().map(|p| p.state()),
            schedule_off: e.schedule.as_ref().is_some_and(|s| !s.on),
            changed_at: e.changed_at,
            planted: e.planted.clone(),
        };
        let map = self.loaded.map.name.clone();
        let uid = e.uid;
        self.world.set_entity_state(&map, uid, st);
    }

    // ---------------------------------------------------------------- update

    /// Runs one frame: update and draw.
    pub fn frame(&mut self) {
        input::poll_pad();
        let dt = get_frame_time().min(0.05);
        self.update(dt);
        self.draw();
    }

    pub fn update(&mut self, dt: f32) {
        self.update_juice(dt);
        self.after_thumbnail();
        self.sync_script_player();
        // Esc opens the pause menu (unless a window or dialog uses Esc); it pauses everything.
        // (While a bolt spell is being aimed, Esc cancels the spell instead.)
        let can_open = !self.inventory_open() && !self.ui.dialog_open() && self.game_over.is_none() && matches!(self.fade, Fade::None) && !self.aiming() && !self.world_map_open();
        if self.on_title() {
            // The start fade-in still plays behind the title screen.
            if let Fade::In { t } = &mut self.fade {
                *t -= dt;
                if *t <= 0.0 {
                    self.fade = Fade::None;
                }
            }
        }
        // The developer console pauses everything while open.
        if self.update_dev(dt) {
            return;
        }
        if self.update_creator(dt) {
            return;
        }
        // Lockpicking and fishing pause the world (Esc gives up instead of opening the menu).
        if self.update_minigame(dt) {
            return;
        }
        // Sleeping: the night passes behind a dark screen.
        if self.update_sleep(dt) {
            return;
        }
        // Choosing whom to heal pauses the world (Esc cancels it instead of opening the menu).
        if self.update_heal_pick() {
            return;
        }
        // So does the companions' orders menu (G).
        if self.update_order_menu() {
            return;
        }
        if self.update_menu(can_open) {
            return;
        }
        // The map screen pauses the game like the menu.
        if self.update_world_map(dt) {
            return;
        }
        self.world.play_time += dt as f64;
        self.update_achievements(dt);
        if !self.on_title() && self.project.config.day_minutes > 0.0 {
            let hours = dt * 24.0 / (self.project.config.day_minutes * 60.0);
            self.world.advance_hours(hours);
            self.water_crops(hours);
        }
        if input::act_pressed(Act::Debug) {
            self.debug = !self.debug;
        }
        if input::act_pressed(Act::QuickSave) {
            match self.save_game("quick") {
                Ok(()) => self.ui.toast("Game saved".into()),
                Err(e) => self.ui.toast(format!("Save failed: {e}")),
            }
        }
        if input::act_pressed(Act::QuickLoad) {
            if let Err(e) = self.load_game("quick") {
                self.ui.toast(format!("Load failed: {e}"));
            }
        }

        // Fades / map transitions.
        match &mut self.fade {
            Fade::None => {}
            Fade::Out { t, .. } => {
                *t += dt;
                if *t >= FADE_TIME {
                    if let Fade::Out { map, arrival, .. } = std::mem::replace(&mut self.fade, Fade::In { t: FADE_TIME }) {
                        self.finish_transition(map, arrival);
                    }
                }
            }
            Fade::In { t } => {
                *t -= dt;
                if *t <= 0.0 {
                    self.fade = Fade::None;
                }
            }
        }

        for (ev, src) in std::mem::take(&mut self.pending_events) {
            self.fire(ev, src);
        }

        self.update_autosave();
        if self.update_game_over(dt) {
            return;
        }

        // UI gets input first; a key that closes a dialog must not also interact.
        let allow_inventory = !self.runner.busy() && matches!(self.fade, Fade::None);
        self.ui.text_speed = self.settings.text_speed;
        let input = self.ui.update(dt, &mut self.world);
        let allow_windows = allow_inventory && !self.ui.dialog_open() && self.game_over.is_none();
        // The companion window takes all input while open.
        let comp = allow_windows && self.update_companion_window();
        let (shop_key, shop_mouse) = self.update_shop_ui(allow_windows && !comp);
        let (shop_key, shop_mouse) = (shop_key || comp, shop_mouse || comp);
        // The crafting window works like the trade window (they never are open together).
        let (craft_key, craft_mouse) = self.update_crafting_ui(allow_windows && !self.shop.is_open());
        let (shop_key, shop_mouse) = (shop_key || craft_key, shop_mouse || craft_mouse);
        // The character window's tabs (C / I, K, L, Tab, tab clicks) before the tabs' contents.
        let (win_key, win_mouse) = self.update_window_tabs(allow_windows && !input.consumed, input.consumed || shop_key);
        let (shop_key, shop_mouse) = (shop_key || win_key, shop_mouse || win_mouse);
        let allow_windows = allow_windows && !win_mouse;
        // H hides / shows the HUD.
        let hide_key = allow_windows && !input.consumed && !shop_key && input::act_pressed(Act::HideUi);
        if hide_key {
            self.ui_hidden = !self.ui_hidden;
            if self.ui_hidden {
                self.ui.toast("Interface hidden - press H to show it".into());
            }
        }
        // G / LB: the companions' orders (only while nothing else is open).
        let cmd_key = self.maybe_open_order_menu(allow_windows && !input.consumed && !shop_key && !hide_key && !self.modal() && !self.aiming());
        // A click on a companion's HUD row opens its window.
        let row_click = !shop_mouse && allow_windows && !self.modal() && !self.aiming() && self.click_companion_row();
        let (shop_key, shop_mouse) = (shop_key, shop_mouse || row_click);
        let (shop_key, shop_mouse) = (shop_key || hide_key || cmd_key, shop_mouse);
        let (j_key, j_mouse) = self.update_journal(allow_windows && !self.shop_open() && !self.inv.is_open() && !self.skills_open(), input.consumed || shop_key);
        let (k_key, k_mouse) = self.update_skills_ui(allow_windows && !self.shop_open() && !self.inv.is_open() && !self.journal_open(), input.consumed || shop_key || j_key);
        let (j_key, j_mouse) = (j_key || k_key, j_mouse || k_mouse);
        let others = self.shop_open() || self.journal_open() || self.skills_open();
        let (inv_key, inv_mouse) = self.update_inventory_ui(allow_windows && !others, input.consumed || shop_key || j_key);
        let (inv_key, inv_mouse) = (inv_key || shop_key || j_key, inv_mouse || shop_mouse || j_mouse);
        let (aim_key, aim_mouse) = self.update_aim(input.consumed || inv_key, input.mouse_consumed || inv_mouse);
        let consumed = input.consumed || inv_key || aim_key;
        if !input.mouse_consumed && !inv_mouse && !aim_mouse && allow_inventory && !self.modal() {
            self.handle_mouse();
        }

        let mut runner = std::mem::take(&mut self.runner);
        runner.update(self, dt);
        // Actions queued while it ran (triggers fired by an action, e.g. the 'Defeated' triggers
        // of characters a 'Damage area' defeats) went to the stand-in runner: keep them.
        let queued = std::mem::replace(&mut self.runner, runner);
        self.runner.append(queued);
        self.update_leaving(dt);
        // Moods last as long as the event that set them.
        if !self.moods.is_empty() && !self.runner.busy() && !self.ui.dialog_open() {
            self.moods.clear();
        }
        self.update_cutscene(dt);

        let frozen = self.runner.busy() || !matches!(self.fade, Fade::None) || self.modal();
        if !frozen && !self.on_title() {
            self.update_skills(dt);
            self.tick_effects(dt);
        }
        self.update_player(dt, frozen, consumed);
        self.reveal_fog();
        self.update_entities(dt, frozen);
        self.update_combat(dt, frozen, consumed);
        if !frozen {
            self.update_projectiles(dt);
            self.update_spawners(dt);
        }
        // Effects and timed lights also play on during cut-scenes (they are part of the show).
        if !frozen || self.cutscene_active() {
            self.update_fx(dt);
            self.update_weather(dt);
        }
        if !frozen {
            self.update_gather(dt);
        }
        if let Some(m) = &mut self.click_marker {
            m.1 += dt;
            if m.1 > 0.6 {
                self.click_marker = None;
            }
        }
    }

    fn update_player(&mut self, dt: f32, frozen: bool, key_consumed: bool) {
        let ts = self.ts();
        // On furniture: stays there (frozen: dialogs) until a key stands the player up.
        if let Some(s) = &self.seat {
            if frozen {
                (self.player.pos, self.player.moving) = (s.pos, false);
                return;
            }
            if self.update_seat(dt, key_consumed) {
                return;
            }
        }
        if let Some(target) = self.player.move_target {
            let (pos, done) = step_towards(self.player.pos, target, self.player.speed * dt);
            self.player.facing = Facing::from_vec(target.x - pos.x, target.y - pos.y).unwrap_or(self.player.facing);
            self.player.pos = pos;
            self.player.moving = !done;
            self.player.anim += dt;
            if done {
                self.player.move_target = None;
            }
            return;
        }
        if frozen {
            self.player.moving = false;
            return;
        }
        if self.apply_player_knockback(dt) {
            return;
        }
        // Stunned: no moving, attacking or interacting until it wears off.
        if self.player_stunned() {
            self.player.moving = false;
            self.order = None;
            return;
        }

        let mut dir = Vec2::ZERO;
        if input::act_down(Act::Left) {
            dir.x -= 1.0;
        }
        if input::act_down(Act::Right) {
            dir.x += 1.0;
        }
        if input::act_down(Act::Up) {
            dir.y -= 1.0;
        }
        if input::act_down(Act::Down) {
            dir.y += 1.0;
        }
        let mut run = if input::act_down(Act::Run) { 1.8 } else { 1.0 };
        if dir != Vec2::ZERO {
            // Keyboard movement cancels click-to-move.
            self.order = None;
        } else if self.order.is_some() {
            dir = self.steer_order(dt);
            run = 1.0;
            if self.fade_active() {
                return; // the order triggered a transition
            }
        }
        if dir != Vec2::ZERO {
            dir = dir.normalize();
            if let Some(f) = Facing::from_vec(dir.x, dir.y) {
                self.player.facing = f;
            }
            let delta = dir * self.player.speed * self.speed_factor() * run * dt;
            self.player.pos = self.try_move(&self.player, delta, None, false);
            self.player.moving = true;
            self.player.anim += dt * run;
        } else {
            self.player.moving = false;
        }

        // Map edges.
        let m = &self.loaded.map;
        if let Some(edge) = m.edges.crossing(self.player.pos, dir, m.width, m.height, ts) {
            let link = m.edges.get(edge).unwrap().clone();
            let target = link.target_map.clone();
            self.transition(&target, Arrival::Edge { edge, pos: self.player.pos, link });
            return;
        }

        // Zones.
        let now = self.zones_at(self.player.pos);
        let entered: Vec<u32> = now.difference(&self.zones_inside).copied().collect();
        let left: Vec<u32> = self.zones_inside.difference(&now).copied().collect();
        self.zones_inside = now;
        for uid in left {
            self.fire(GameEvent::Leave, Source::Zone(uid));
        }
        for uid in entered {
            self.fire(GameEvent::Enter, Source::Zone(uid));
            let portal = self.loaded.map.zones.iter().find(|z| z.uid == uid).and_then(|z| match &z.kind {
                ZoneKind::Portal { target_map, target_spawn } => Some((target_map.clone(), Arrival::Spawn(target_spawn.clone()))),
                ZoneKind::Transition { target_map, target_zone } if !target_map.is_empty() => Some((target_map.clone(), Arrival::Beside(target_zone.clone()))),
                _ => None,
            });
            if let Some((map, arrival)) = portal {
                self.audio.play(&self.project.config.sfx.portal);
                self.transition(&map, arrival);
                return;
            }
        }

        if !key_consumed && input::act_pressed(Act::Confirm) {
            self.interact();
        }
    }

    /// The entity in front of the player, if any.
    /// A character two tiles away in a straight line with a solid tile in between (e.g. a
    /// merchant behind a counter) can be talked to across it.
    fn across_counter(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        if e.kind != EntityKind::Character || !e.active || e.hostile {
            return false;
        }
        let ts = self.ts();
        let tile = |p: Vec2| ((p.x / ts).floor() as i32, ((p.y - 1.0) / ts).floor() as i32);
        let ((px, py), (ex, ey)) = (tile(self.player.pos), tile(e.body.pos));
        let (dx, dy) = (ex - px, ey - py);
        let straight = (dx.abs() == 2 && dy == 0) || (dy.abs() == 2 && dx == 0);
        straight && self.tile_solid(px + dx / 2, py + dy / 2)
    }

    fn facing_entity(&self) -> Option<usize> {
        self.facing_entity_near().map(|i| self.user_of(i).unwrap_or(i)).or_else(|| {
            // Nothing right in front: someone across a counter in the facing direction?
            let ts = self.ts();
            let (fx, fy) = self.player.facing.dir();
            (0..self.loaded.entities.len()).find(|&i| {
                let d = self.loaded.entities[i].body.pos - self.player.pos;
                self.across_counter(i) && (d.x * fx + d.y * fy) > ts
            })
        })
    }

    /// The character using furniture `i` (a bed's sleeper): talking to the furniture talks to it.
    fn user_of(&self, i: usize) -> Option<usize> {
        let uid = self.loaded.entities[i].uid;
        self.loaded.entities.iter().position(|e| e.active && e.npc_seat.as_ref().is_some_and(|s| s.uid == uid))
    }

    fn facing_entity_near(&self) -> Option<usize> {
        let ts = self.ts();
        let (dx, dy) = self.player.facing.dir();
        let center = self.player.pos + vec2(dx, dy) * ts * 0.6 - vec2(0.0, self.player.hitbox.y / 2.0);
        let probe = Rect::new(center.x - ts * 0.4, center.y - ts * 0.4, ts * 0.8, ts * 0.8);
        self.loaded
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.active && e.body.rect().overlaps(&probe))
            .min_by(|(_, a), (_, b)| {
                let da = a.body.pos.distance_squared(center);
                let db = b.body.pos.distance_squared(center);
                da.total_cmp(&db)
            })
            .map(|(i, _)| i)
    }

    fn fade_active(&self) -> bool {
        !matches!(self.fade, Fade::None)
    }

    fn interact(&mut self) {
        if let Some(i) = self.facing_entity() {
            self.interact_entity(i);
            return;
        }
        for uid in self.zones_inside.clone() {
            self.fire(GameEvent::Interact, Source::Zone(uid));
        }
        self.try_fish();
    }

    fn interact_entity(&mut self, i: usize) {
        if self.is_down(i) {
            let name = self.entity_display_name(self.loaded.entities[i].uid).unwrap_or_default();
            self.ui.toast(format!("{name} is out cold. Heal them to get them back up."));
            return;
        }
        {
            let uid = self.loaded.entities[i].uid;
            match self.loaded.entities[i].kind {
                EntityKind::Item => {
                    let (def, count) = (self.loaded.entities[i].def.clone(), self.loaded.entities[i].count.max(1));
                    let left = self.world.give_item(&def, count);
                    if left == count {
                        self.ui.toast("Inventory full".into());
                        return;
                    }
                    // What didn't fit stays on the ground.
                    self.loaded.entities[i].count = left;
                    self.loaded.entities[i].active = left > 0;
                    self.persist(i);
                    self.item_given(&def, count - left, 0);
                    if left > 0 {
                        self.ui.toast("Inventory full".into());
                    }
                }
                EntityKind::Character => {
                    let p = self.player.pos;
                    let ent = &mut self.loaded.entities[i];
                    // A patrolling character stays a moment after being talked to; one at work
                    // stops for a moment.
                    if let Some(pt) = &mut ent.patrol {
                        pt.wait = pt.wait.max(1.5);
                    }
                    ent.act_pause = activities::TALK_PAUSE;
                    // (Someone on furniture keeps its pose.)
                    if ent.npc_seat.is_none() {
                        let e = &mut ent.body;
                        e.facing = Facing::from_vec(p.x - e.pos.x, p.y - e.pos.y).unwrap_or(e.facing);
                        e.moving = false;
                    }
                }
                EntityKind::Prop => {
                    // Open gates / doors (not containers) can be locked again with their key.
                    if self.loaded.entities[i].unlocked && !self.is_container(i) && self.try_relock(i, None) {
                        return;
                    }
                    // Locked props need their key first (it is used up when unlocking).
                    if !self.try_unlock(i, None) {
                        return;
                    }
                    if !self.loaded.entities[i].active {
                        return; // e.g. a gate that disappeared on unlocking
                    }
                    // Ore veins, trees...: E is a gathering hit; farm plots plant / harvest.
                    if self.gather_hit(i) || self.try_farm(i) {
                        return;
                    }
                }
            }
            let fired = self.fire(GameEvent::Interact, Source::Entity(uid));
            match self.loaded.entities[i].kind {
                EntityKind::Prop => {
                    self.try_open_container(i);
                    self.try_use_furniture(i);
                }
                EntityKind::Character => {
                    // Asleep, and nothing to say about it (a Pose condition): just a toast.
                    if !fired && self.npc_pose(i) == "lie" {
                        let name = self.entity_display_name(uid).unwrap_or_default();
                        self.ui.toast(format!("{name} is asleep."));
                        return;
                    }
                    self.try_open_shop(i, true);
                }
                EntityKind::Item => {}
            }
        }
    }

    /// Gameplay is paused while a dialog or an inventory window is open.
    fn modal(&self) -> bool {
        self.ui.dialog_open() || self.inventory_open()
    }

    /// Applies project rules (inventory size, stack limits) to the world state, e.g. after loading.
    fn prepare_world(&mut self) {
        self.migrate_equipment();
        self.world.renumber_old_drops();
        self.init_skills(self.world.play_time == 0.0 && self.world.current_map.is_empty());
        // Games started without the creator (or saved before it existed) get the default name.
        if self.world.player_name.trim().is_empty() {
            self.world.player_name = self.default_player_name();
        }
        // A game that fixes the player's sex always has it (new games, play-tests, saves).
        if let Some(sex) = self.project.config.creation.sex.fixed() {
            self.world.player_sex = sex;
        }
        let limits = self.project.db.stack_limits();
        self.world.inventory.resize(self.project.config.inventory_size.max(1) as usize);
        self.world.inventory.set_limits(limits.clone());
        for inv in self.world.containers.values_mut().flat_map(|m| m.values_mut()) {
            inv.set_limits(limits.clone());
        }
        self.dress_player();
    }

    /// The player definition's equipment and carried items: a new game starts with them; a save
    /// from before clothing existed gets the clothing once (instead of showing the underwear).
    fn dress_player(&mut self) {
        if self.world.dressed {
            return;
        }
        self.world.dressed = true;
        let Some(d) = self.project.db.character(&self.project.config.player) else { return };
        let (eq, inv) = (d.equipment.clone(), d.inventory.clone());
        let fresh = self.world.play_time == 0.0 && self.world.current_map.is_empty();
        for slot in crate::defs::EquipSlot::ALL {
            let mine = self.world.equipment.slot_mut(slot);
            if mine.is_none() && (fresh || slot == crate::defs::EquipSlot::Clothing) {
                *mine = eq.get(slot).cloned();
            }
        }
        if fresh {
            for s in inv {
                self.world.give_item(&s.item, s.count);
            }
        }
    }

    fn use_item(&mut self, item: &str) {
        if self.project.db.item(item).is_some_and(|d| d.equip_slot().is_some()) {
            self.equip_item(item);
            return;
        }
        // Scrolls and spell books teach a spell (consumed if the item is consumable).
        if let Some(spell) = self.project.db.item(item).map(|d| d.teaches.clone()).filter(|s| !s.is_empty()) {
            if self.learn_spell(&spell) && self.project.db.item(item).is_some_and(|d| d.consumable) {
                self.world.take_item(item, 1);
            }
            return;
        }
        // Using the right key on a locked prop opens it.
        if let Some(i) = self.facing_entity() {
            if self.active_lock(i).is_some_and(|l| l.key == item) {
                if self.try_unlock(i, Some(item)) && self.loaded.entities[i].active {
                    self.try_open_container(i);
                }
                return;
            }
            // ... and using it on an opened one locks it again (also containers).
            if self.relockable(i).is_some_and(|l| l.key == item) && self.try_relock(i, Some(item)) {
                return;
            }
        }
        // Seeds are planted on the farm plot in front.
        if self.plant_on_facing(item) {
            return;
        }
        // A healing item asks whom to heal while companions are around.
        if self.start_heal_pick(healing::HealSource::Item(item.to_string())) {
            return;
        }
        self.use_item_now(item);
    }

    /// Runs an item's own *on use* triggers; food heals more with the player's Food skills.
    fn fire_item_use(&mut self, item: &str) -> bool {
        let bonus = if self.project.db.item(item).is_some_and(|d| d.food) {
            self.skill_sum(|e, l| match e {
                crate::skills::SkillEffect::FoodBonus { percent } => percent * l as f32,
                _ => 0.0,
            })
        } else {
            0.0
        };
        let actions = self.collect(&GameEvent::UseItem { item: item.to_string() }, &Source::Item(item.to_string()));
        let any = !actions.is_empty();
        for (a, ctx) in actions {
            self.runner.push(&[food_bonus(a, bonus)], &ctx);
        }
        any
    }

    /// Uses an item on the player / the prop in front (its *UseItem* triggers).
    fn use_item_now(&mut self, item: &str) {
        let ev = GameEvent::UseItem { item: item.to_string() };
        let mut fired = false;
        if let Some(i) = self.facing_entity() {
            fired = self.fire(ev.clone(), Source::Entity(self.loaded.entities[i].uid));
        }
        if !fired {
            fired = self.fire_item_use(item);
        }
        if !fired {
            self.ui.toast("Nothing happens.".into());
        } else if self.project.db.item(item).is_some_and(|d| d.consumable) {
            self.world.take_item(item, 1);
        }
    }

    fn update_entities(&mut self, dt: f32, frozen: bool) {
        let ts = self.ts();
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if e.away && !frozen {
                // Indoors: only the clock brings it out.
                self.schedule_step(i, dt);
                continue;
            }
            if !e.active || e.kind != EntityKind::Character {
                continue;
            }
            if let Some(target) = e.body.move_target {
                let (pos, done) = step_towards(e.body.pos, target, e.body.speed.max(30.0) * dt);
                let e = &mut self.loaded.entities[i];
                e.body.facing = Facing::from_vec(target.x - e.body.pos.x, target.y - e.body.pos.y)
                    .unwrap_or(e.body.facing);
                e.body.pos = pos;
                e.body.moving = !done;
                e.body.anim += dt;
                if done {
                    e.body.move_target = None;
                    e.home = pos;
                    self.persist(i);
                }
                continue;
            }
            if frozen {
                self.loaded.entities[i].body.moving = false;
                continue;
            }
            // Characters drink a potion they carry when it's critical (and so on).
            self.npc_use_items(i, dt);
            if self.entity_knockback(i, dt) {
                continue;
            }
            // Slow / haste scale the time the character moves in; stunned ones stand still.
            let f = self.npc_speed_factor(i);
            if f <= 0.0 {
                self.loaded.entities[i].body.moving = false;
                continue;
            }
            let dt = dt * f;
            if self.companion_step(i, dt) || self.chase(i, dt) {
                continue;
            }
            let sched = self.schedule_step(i, dt);
            // At work (fishing, hammering...): stays where it is.
            if sched != Some(true) && self.activity_step(i, dt) {
                continue;
            }
            match sched {
                Some(true) => continue,
                // At its stop: wander around it (or stand), patrol routes wait.
                Some(false) => {}
                None if self.patrol_step(i, dt) => continue,
                None => {}
            }
            let e = &self.loaded.entities[i];
            let radius = match e.behavior {
                Behavior::Wander { radius } => radius,
                Behavior::Chase { .. } => 3.0,
                Behavior::Static => continue,
            };
            let e = &mut self.loaded.entities[i];
            e.wander_timer -= dt;
            if e.wander_timer <= 0.0 {
                if e.wander_dir == Vec2::ZERO && rand::gen_range(0, 3) > 0 {
                    let dirs = [vec2(1., 0.), vec2(-1., 0.), vec2(0., 1.), vec2(0., -1.)];
                    e.wander_dir = dirs[rand::gen_range(0, 4)];
                    e.wander_timer = rand::gen_range(0.4, 1.4);
                } else {
                    e.wander_dir = Vec2::ZERO;
                    e.wander_timer = rand::gen_range(1.0, 3.0);
                }
            }
            if e.wander_dir == Vec2::ZERO {
                e.body.moving = false;
                continue;
            }
            let delta = e.wander_dir * e.body.speed * dt;
            let (home, body, uid) = (e.home, e.body.clone(), e.uid);
            let next = body.pos + delta;
            let within = next.distance(home) <= radius * ts || next.distance(home) < body.pos.distance(home);
            let new_pos = if within { self.try_move(&body, delta, Some(uid), true) } else { body.pos };
            let e = &mut self.loaded.entities[i];
            if new_pos == e.body.pos {
                e.wander_dir = Vec2::ZERO; // bumped into something
            }
            e.body.facing = Facing::from_vec(delta.x, delta.y).unwrap_or(e.body.facing);
            e.body.pos = new_pos;
            e.body.moving = new_pos != body.pos;
            e.body.anim += dt;
        }
    }

    /// The player chose Quit in the pause menu.
    pub fn wants_quit(&self) -> bool {
        self.menu.quit
    }

    /// One-line summary of the game state (for logs and automated tests).
    pub fn status(&self) -> String {
        let p = self.player.pos / self.ts();
        format!(
            "{} weather={} menu={} autosave={} cutscene={} ui_hidden={} window={} npcs={:?} fx={:?} worldmap={{{}}} skills={{{}}} {} {} ai={:?} bestiary={:?} dialog_log={} pad={:?} attacking={} shake={} time={} light={:?} lights={} patrol={:?} schedule={:?} companions={:?} dropped={:?} player={:?} creator={:?} quests={:?} achievements={:?} minigame={} pose={} gather={:?} props={:?} crops={:?} activities={:?} day={} parallax={} aim={} orders={} compwin={} shop={:?} crafting={:?} map={} tile=({},{}) hp={:?} vars={:?} quick={:?} equip={:?} enemies={} busy={} dialog={} inv={:?} flags={:?} music={:?} music_loop={}",
            self.dev_status(),
            self.weather_status(),
            self.menu_status(),
            self.autosave_status(),
            self.cutscene_status(),
            self.ui_hidden,
            self.window_status(),
            self.npc_items_status(),
            self.fx_status(),
            self.world_map_status(),
            self.skills_status(),
            self.progression_status(),
            self.effects_status(),
            self.ai_status(),
            self.bestiary_status(),
            self.world.dialog_log.len(),
            input::pad_name(),
            self.combat.cooldown > 0.0,
            self.juice_status(),
            self.clock_text(),
            self.loaded.map.lighting.mode,
            self.lights().len(),
            self.patrol_status(),
            self.schedule_status(),
            self.companions_status(),
            {
                let ts = self.ts();
                let here = self.loaded.entities.iter().filter(|e| e.active && e.uid >= crate::state::DROPPED_UID_BASE);
                here.map(|e| format!("{}x{}@{},{}", e.def, e.count, (e.body.pos.x / ts).floor(), ((e.body.pos.y - 1.0) / ts).floor())).collect::<Vec<_>>()
            },
            format!("{}|{:?}|{}", self.world.player_name, self.world.player_sex, self.player_appearance().map_or("-".to_string(), |a| a.body)),
            self.creator_status(),
            self.world.quests.iter().map(|(k, q)| format!("{k}:{}:{:?}", q.stage, q.status)).collect::<Vec<_>>(),
            self.achievements_status(),
            self.minigame_status(),
            self.pose_status(),
            self.gather_status(),
            self.prop_states_status(),
            self.crops_status(),
            self.activities_status(),
            self.world.day,
            {
                let (a, m) = (&self.assets, &self.loaded.map.parallax);
                format!("{}behind,{}above", render::parallax::count(a, m, false), render::parallax::count(a, m, true))
            },
            self.heal_pick_status(),
            self.order_menu_status(),
            self.companion_win_status(),
            self.shop_status(),
            self.crafting_status(),
            self.loaded.map.name,
            p.x.floor(),
            p.y.floor(),
            self.world.player_hp,
            self.world.vars,
            self.world.quickbar,
            crate::defs::EquipSlot::ALL.map(|s| self.world.equipment.item(s).unwrap_or("-").to_string()),
            self.enemies_alive(),
            self.runner.busy(),
            self.ui.dialog_open(),
            self.world.inventory.summary(),
            self.world.flags,
            self.audio.current_music(),
            self.audio.music_loop_status()
        )
    }

    // ---------------------------------------------------------------- save / load

    fn sync_world(&mut self) {
        self.persist_patrols();
        self.world.current_map = self.loaded.map.name.clone();
        // On furniture, the spot it was used from (the furniture itself is solid).
        let pos = self.seat.as_ref().map_or(self.player.pos, |s| s.back);
        self.world.player_pos = (pos.x, pos.y);
        self.world.player_facing = self.player.facing;
    }

    pub fn save_game(&mut self, slot: &str) -> Result<()> {
        self.write_save(slot)?;
        self.slot = Some(slot.to_string());
        Ok(())
    }

    /// Writes the save file of `slot` (the game keeps its own slot for *Save and quit*).
    fn write_save(&mut self, slot: &str) -> Result<()> {
        if self.runner.busy() {
            return Err(anyhow!("can't save during an event"));
        }
        self.sync_world();
        self.world.saved_at = menu::now_secs();
        let dir = self.project.saves_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{slot}.ron")), self.world.to_ron()?)?;
        self.request_thumbnail(slot);
        Ok(())
    }

    /// Writes a due autosave once the game is calm: no event, cut-scene, dialog, fade or
    /// defeat in progress.
    fn update_autosave(&mut self) {
        if !self.autosave_pending || self.runner.busy() || self.ui.dialog_open() || self.game_over.is_some() || !matches!(self.fade, Fade::None) || self.on_title() || self.cutscene_active() {
            return;
        }
        self.autosave_pending = false;
        match self.write_save(AUTOSAVE) {
            Ok(()) => self.ui.toast("Autosaved".into()),
            Err(e) => {
                eprintln!("[game] autosave failed: {e:#}");
                self.ui.toast(format!("Autosave failed: {e}"));
            }
        }
    }

    /// For `status()`: "pending", "saved" (the file exists) or "none".
    fn autosave_status(&self) -> &'static str {
        if self.autosave_pending {
            "pending"
        } else if self.project.saves_dir().join(format!("{AUTOSAVE}.ron")).exists() {
            "saved"
        } else {
            "none"
        }
    }

    pub fn load_game(&mut self, slot: &str) -> Result<()> {
        let s = std::fs::read_to_string(self.project.saves_dir().join(format!("{slot}.ron")))?;
        let world = WorldState::from_ron(&s)?;
        self.slot = Some(slot.to_string());
        self.ui_hidden = false;
        self.reset_cutscene();
        let map = world.current_map.clone();
        let pos = vec2(world.player_pos.0, world.player_pos.1);
        self.player.facing = world.player_facing;
        self.world = world;
        self.prepare_world();
        self.runner.clear();
        self.ui = Ui::default();
        self.inv = Default::default();
        self.game_over = None;
        self.autosave_pending = false;
        self.combat.reset();
        if self.world.player_hp.is_none_or(|hp| hp <= 0) {
            self.world.player_hp = Some(self.player_max_hp());
        }
        // Rebuild the map from scratch so entity states from the save apply.
        self.cache.clear();
        self.loaded.map.name.clear();
        self.resuming = true;
        self.transition(&map, Arrival::Pos(pos));
        self.ui.toast("Game loaded".into());
        Ok(())
    }

    // ---------------------------------------------------------------- drawing

    /// The game camera: follows the player, clamped to the map.
    fn camera(&self) -> Camera2D {
        let ts = self.ts();
        let m = &self.loaded.map;
        let scale = self.view_scale();
        let (vw, vh) = (screen_width() / scale, screen_height() / scale);
        let (mw, mh) = (m.width as f32 * ts, m.height as f32 * ts);
        let clamp_axis = |p: f32, view: f32, size: f32| {
            if size <= view { size / 2.0 } else { p.clamp(view / 2.0, size - view / 2.0) }
        };
        let mut focus = vec2(self.player.pos.x, self.player.pos.y - 8.0);
        if self.on_title() {
            // The title screen slowly drifts over the start map.
            let t = get_time() as f32;
            focus += vec2((t * 0.07).sin() * 12.0 * ts, (t * 0.05).cos() * 6.0 * ts);
        }
        let target = vec2(clamp_axis(focus.x, vw, mw), clamp_axis(focus.y, vh, mh)) + self.shake_offset();
        render::pixel_camera(target, scale)
    }

    /// Screen position of the center of a tile (used by automated play-throughs).
    pub fn tile_to_screen(&self, tx: f32, ty: f32) -> Vec2 {
        let ts = self.ts();
        self.camera().world_to_screen(vec2((tx + 0.5) * ts, (ty + 0.5) * ts))
    }

    pub fn draw(&mut self) {
        let ts = self.ts();
        let m = &self.loaded.map;
        let bg = m.background;
        clear_background(Color::from_rgba(bg[0], bg[1], bg[2], 255));

        let cam = self.camera();
        set_camera(&cam);
        let view = render::camera_view(&cam);
        let sets = self.assets.map_tilesets(m);

        // Parallax layers behind the tiles (seen under empty tiles and past the map's edge).
        render::parallax::draw(&self.assets, &m.parallax, false, view, get_time() as f32);
        for layer in m.layers.iter().filter(|l| l.visible && !l.above_entities) {
            render::draw_layer(m, layer, &sets, ts, view, WHITE);
        }
        // Fish shadows mark the fishing spots' water.
        let now = get_time() as f32;
        for z in m.zones.iter().filter(|z| matches!(z.kind, ZoneKind::FishingSpot { .. })) {
            let r = Rect::new(z.x as f32 * ts, z.y as f32 * ts, z.w as f32 * ts, z.h as f32 * ts);
            if r.overlaps(&view) {
                render::fish::draw_shadows(r, z.uid, now, ts);
            }
        }

        // Y-sorted entities + player.
        let mut order: Vec<(f32, Option<usize>)> = self
            .loaded
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.active)
            .map(|(i, e)| (e.npc_seat.as_ref().map_or(e.body.pos.y, |s| s.prop_feet.y + 0.5), Some(i)))
            .collect();
        order.push((self.player_sort_y(), None));
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        let t = get_time() as f32;
        for (_, idx) in order {
            match idx {
                None if self.on_title() => {}
                None => {
                    // Blink while invulnerable after being hit.
                    if self.combat.invuln > 0.0 && (self.combat.invuln * 12.0) as i32 % 2 == 0 {
                        continue;
                    }
                    let col = if self.player.moving { (self.player.anim * 8.0) as u32 % paperdoll::LAYOUT_FRAMES } else { 0 };
                    // During a melee swing the weapon is drawn along the swing instead.
                    let swinging = self.combat.swing > 0.0;
                    let eq = self.player_equipment_drawn();
                    let tint = Self::effect_tint(&self.world.effects).unwrap_or(WHITE);
                    let drawn = self.draw_seated_player(&eq, tint) || self.player_appearance().is_some_and(|a| self.draw_character(&a, &eq, (self.player.facing, col), self.player.pos, tint, swinging));
                    if !drawn {
                        let r = self.player.rect();
                        draw_rectangle(r.x, r.y - ts * 0.5, r.w, r.h + ts * 0.5, YELLOW);
                    }
                }
                Some(i) => self.draw_entity(&self.loaded.entities[i], t),
            }
        }

        self.draw_combat_world();
        self.draw_gather();
        self.draw_activities();
        self.draw_fx();
        if !self.on_title() {
            self.draw_mouse_world();
            self.draw_padlocks();
        }

        for layer in m.layers.iter().filter(|l| l.visible && l.above_entities) {
            render::draw_layer(m, layer, &sets, ts, view, WHITE);
        }
        // Parallax layers over the map (cloud shadows, mist), lit like the rest of the scene.
        render::parallax::draw(&self.assets, &m.parallax, true, view, get_time() as f32);

        // Darkness (map, time of day, cut-scene dimming), then the weather's tint on top.
        let dark = self.scene_ambient(render::lighting::ambient(self.loaded.map.lighting, self.world.hour));
        let scene_light = self.weather_ambient(dark);
        if let Some(ambient) = scene_light {
            // Lights show only where it is dark: weather alone (by day) just tints the scene.
            let lights = if dark.is_some() { self.lights() } else { Vec::new() };
            self.light_pass.get_or_insert_with(render::lighting::LightPass::new).draw(&cam, ambient, &lights, ts, get_time());
            set_camera(&cam);
        }
        // Rain, snow, fog over the lit scene (and in save thumbnails).
        self.draw_weather(view, scene_light);
        // A save's thumbnail: the world as it is now, before overlays, HUD and menus.
        self.capture_thumbnail();

        if self.debug {
            self.draw_debug(view);
        }

        set_default_camera();
        let alpha = match &self.fade {
            Fade::None => 0.0,
            Fade::Out { t, .. } => (t / FADE_TIME).min(1.0),
            Fade::In { t } => (t / FADE_TIME).max(0.0),
        };
        if alpha > 0.0 {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0., 0., 0., alpha));
        }
        self.draw_hurt_flash();
        if !self.on_title() {
            if !self.ui_hidden {
                self.draw_hud();
                self.draw_companion_rows();
                self.draw_clock();
            }
            self.draw_minigame();
            self.draw_sleep();
            if self.companion_win_open() {
                self.draw_companion_window();
            } else if self.crafting_open() {
                self.draw_crafting_ui();
            } else if self.shop_open() {
                self.draw_shop_ui();
            } else if self.journal_open() {
                self.draw_journal();
            } else if !self.heal_picking() && !self.order_menu_open() {
                // (While choosing whom to heal, the window steps aside for the map.)
                self.draw_window_frame();
                if self.skills_open() {
                    self.draw_skills_ui();
                } else {
                    self.draw_inventory_ui();
                }
            }
            self.draw_world_map();
            self.draw_heal_pick();
            self.draw_order_menu();
        }
        if !self.on_title() {
            self.draw_aim_cursor();
        }
        self.draw_cutscene_bars();
        self.draw_fps();
        self.ui.draw_toasts();
        self.ui.draw_dialog();
        self.draw_dialog_portrait();
        self.draw_game_over();
        if self.creator.is_some() {
            self.draw_creator();
        } else {
            self.draw_menu();
        }
        self.draw_achievement_toasts();
        self.draw_console();
        if self.debug {
            let p = self.player.pos / ts;
            draw_text(
                format!("{}  tile {:.0},{:.0}  fps {}", self.loaded.map.name, p.x.floor(), p.y.floor(), get_fps()),
                8.0,
                screen_height() - 8.0,
                20.0,
                WHITE,
            );
        }
    }

    fn draw_entity(&self, e: &RtEntity, t: f32) {
        let ts = self.ts();
        let db = &self.project.db;
        let tint = if e.flash > 0.0 { Color::new(1.0, 0.35, 0.35, 1.0) } else { Self::effect_tint(&e.effects).unwrap_or(WHITE) };
        let drawn = match e.kind {
            EntityKind::Prop => db.prop(&e.def).is_some_and(|d| {
                let ok = self.assets.draw_sprite(d.state_look(e.state).0, e.frame, e.body.pos, WHITE);
                if e.planted.is_some() {
                    if let Some(i) = self.loaded.entities.iter().position(|x| x.uid == e.uid) {
                        self.draw_crop(i);
                    }
                }
                ok
            }),
            EntityKind::Character if e.companion.as_ref().is_some_and(|l| l.down.is_some()) => self.draw_downed(e),
            EntityKind::Character if e.npc_seat.is_some() => {
                let s = e.npc_seat.as_ref().expect("checked");
                self.draw_on_furniture(&e.appearance, &e.equipment, (s.pose, s.pos, s.facing), (&s.cover, s.prop_feet), tint);
                true
            }
            EntityKind::Character if self.loaded.entities.iter().position(|x| x.uid == e.uid).is_some_and(|i| self.current_activity(i).is_some()) => {
                let i = self.loaded.entities.iter().position(|x| x.uid == e.uid).expect("checked");
                let eq = self.activity_equipment(i).unwrap_or_else(|| e.equipment.clone());
                let col = self.activity_frame(i).unwrap_or(0);
                self.draw_character(&e.appearance, &eq, (e.body.facing, col), e.body.pos, tint, false)
            }
            EntityKind::Character => db.character(&e.def).is_some() && {
                let col = if e.body.moving { (e.body.anim * 8.0) as u32 % paperdoll::LAYOUT_FRAMES } else { 0 };
                self.draw_character(&e.appearance, &e.equipment, (e.body.facing, col), e.body.pos, tint, false)
            },
            EntityKind::Item => db.item(&e.def).is_some_and(|d| {
                let u = self.u();
                let bob = ((t * 3.0 + e.uid as f32).sin() * 1.5 * u).round();
                self.assets.draw_sprite(&d.icon, 0, e.body.pos - vec2(0.0, bob + 2.0 * u), WHITE)
            }),
        };
        if !drawn {
            let r = e.body.rect();
            draw_rectangle(r.x, r.y - ts * 0.5, r.w, r.h + ts * 0.5, MAGENTA);
        }
    }

    fn draw_debug(&self, view: Rect) {
        let ts = self.ts();
        let m = &self.loaded.map;
        let x0 = (view.x / ts).floor().max(0.0) as u32;
        let y0 = (view.y / ts).floor().max(0.0) as u32;
        let x1 = ((view.x + view.w) / ts).ceil().min(m.width as f32) as u32;
        let y1 = ((view.y + view.h) / ts).ceil().min(m.height as f32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                if self.loaded.solid[(y * m.width + x) as usize] {
                    draw_rectangle(x as f32 * ts, y as f32 * ts, ts, ts, Color::new(1., 0., 0., 0.25));
                }
            }
        }
        for z in &m.zones {
            let r = z.pixel_rect(ts);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, SKYBLUE);
        }
        for e in Edge::ALL {
            if let Some(r) = m.edges.strip_rect(e, m.width, m.height, ts) {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0., 1., 0., 0.15));
            }
        }
        for e in self.loaded.entities.iter().filter(|e| e.active) {
            let r = e.body.rect();
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, ORANGE);
        }
        let r = self.player.rect();
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, GREEN);
    }
}

/// Healing of food raised by `percent` (the Food skill), also inside *If* actions.
fn food_bonus(a: Action, percent: f32) -> Action {
    if percent <= 0.0 {
        return a;
    }
    match a {
        Action::Heal { amount } if amount > 0 => Action::Heal { amount: (amount as f32 * (1.0 + percent / 100.0)).round() as i32 },
        Action::If { conditions, then, otherwise } => Action::If {
            conditions,
            then: then.into_iter().map(|a| food_bonus(a, percent)).collect(),
            otherwise: otherwise.into_iter().map(|a| food_bonus(a, percent)).collect(),
        },
        a => a,
    }
}

fn step_towards(pos: Vec2, target: Vec2, step: f32) -> (Vec2, bool) {
    let d = target - pos;
    if d.length() <= step {
        (target, true)
    } else {
        (pos + d.normalize() * step, false)
    }
}

impl ActionHost for Game {
    fn world(&mut self) -> &mut WorldState {
        &mut self.world
    }

    fn execute(&mut self, action: &Action, ctx: &ActionCtx) -> Step {
        let ts = self.ts();
        match action {
            Action::Dialog { speaker: raw, text } => {
                self.audio.play(&self.project.config.sfx.dialog);
                let speaker = self.entity_names_in(raw, ctx.owner);
                let (speaker, text) = (self.world.expand_text(&speaker), self.world.expand_text(text));
                self.world.log_dialog(&speaker, &text, false);
                self.ui.show_dialog(&speaker, &text);
                self.set_dialog_portrait(raw, &speaker, ctx.owner);
                Step::Block
            }
            Action::Choice { speaker: raw, text, options, var } => {
                self.audio.play(&self.project.config.sfx.dialog);
                let speaker = self.entity_names_in(raw, ctx.owner);
                let (speaker, text) = (self.world.expand_text(&speaker), self.world.expand_text(text));
                let options: Vec<String> = options.iter().map(|o| self.world.expand_text(o)).collect();
                self.world.log_dialog(&speaker, &text, false);
                self.ui.show_choice(&speaker, &text, &options, var);
                self.set_dialog_portrait(raw, &speaker, ctx.owner);
                Step::Block
            }
            Action::Teleport { map, spawn } => {
                self.audio.play(&self.project.config.sfx.portal);
                self.transition(map, Arrival::Spawn(spawn.clone()));
                Step::Block
            }
            Action::PlaySound { sound } => {
                self.audio.play(sound);
                Step::Continue
            }
            Action::PlayMusic { sound } => {
                self.audio.play_music(sound);
                Step::Continue
            }
            Action::Heal { amount } => {
                self.heal(*amount);
                Step::Continue
            }
            Action::HealCharacter { target, amount } => {
                self.heal_character(target, *amount, ctx.owner);
                Step::Continue
            }
            Action::CompanionOrder { target, order } => {
                self.order_companion(target, order, ctx.owner);
                Step::Continue
            }
            Action::Mood { target, mood } => {
                self.set_mood(target, mood, ctx.owner);
                Step::Continue
            }
            Action::OpenCompanion { target } => {
                match self.entity_index(target, ctx.owner).filter(|&i| self.loaded.entities[i].companion.is_some()) {
                    Some(i) => self.pending_companion = Some(self.loaded.entities[i].uid),
                    None => eprintln!("[events] Open companion window: '{target}' is no companion here"),
                }
                Step::Continue
            }
            Action::SetLocked { target, locked } => {
                let Some(i) = self.entity_index(target, ctx.owner) else { return Step::Continue };
                if *locked {
                    if !self.loaded.entities[i].unlocked {
                        return Step::Continue;
                    }
                    self.apply_lock(i);
                    let uid = self.loaded.entities[i].uid;
                    return Step::Insert(self.collect(&GameEvent::Lock, &Source::Entity(uid)));
                }
                let Some(lock) = self.active_lock(i) else { return Step::Continue };
                self.apply_unlock(i, &lock.on_unlock);
                let uid = self.loaded.entities[i].uid;
                // Unlock triggers run before the rest of this sequence.
                Step::Insert(self.collect(&GameEvent::Unlock, &Source::Entity(uid)))
            }
            Action::SetActive { target, active } => {
                if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.loaded.entities[i].active = *active;
                    self.loaded.entities[i].away = false;
                    self.persist(i);
                }
                Step::Continue
            }
            Action::SetState { target, state } => {
                if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.set_prop_state(i, state);
                }
                Step::Continue
            }
            Action::Patrol { target, active, points } => {
                match self.entity_index(target, ctx.owner) {
                    Some(i) if self.loaded.entities[i].kind == EntityKind::Character => self.set_patrol(i, *active, points),
                    _ => eprintln!("Patrol: '{target}' is not a character on this map"),
                }
                Step::Continue
            }
            Action::Quest { quest, op, stage } => {
                self.quest_changed(quest, *op, *stage);
                Step::Continue
            }
            Action::Companion { target, join } => {
                match self.entity_index(target, ctx.owner) {
                    Some(i) if *join => self.recruit(i),
                    Some(i) => self.dismiss(i, false),
                    None => eprintln!("Companion: '{target}' is not a character on this map"),
                }
                Step::Continue
            }
            Action::Schedule { target, active } => {
                match self.entity_index(target, ctx.owner) {
                    Some(i) if self.loaded.entities[i].schedule.is_some() => self.set_schedule(i, *active),
                    _ => eprintln!("Schedule: '{target}' is not a character with a schedule on this map"),
                }
                Step::Continue
            }
            Action::SetTime { hour } => {
                self.world.hour = hour.rem_euclid(24.0);
                Step::Continue
            }
            Action::GiveSkillPoints { amount } => {
                self.give_skill_points(*amount);
                Step::Continue
            }
            Action::SetSkill { skill, level } => {
                self.set_skill_level(skill, *level);
                Step::Continue
            }
            Action::LearnSpell { spell } => {
                self.learn_spell(spell);
                Step::Continue
            }
            Action::AddMana { amount } => {
                self.add_mana(*amount);
                Step::Continue
            }
            Action::DamageAround { damage, radius } => {
                self.damage_around(*damage, *radius);
                Step::Continue
            }
            Action::PlayerLight { radius, seconds, color, flicker } => {
                self.player_light_for(*radius, *seconds, *color, *flicker);
                Step::Continue
            }
            Action::Aim { function } => {
                self.start_aim(function);
                Step::Continue
            }
            Action::Fly { sprite, x, y, speed, function } => {
                self.spawn_fly(sprite.clone(), vec2(*x, *y), *speed, function.clone());
                Step::Continue
            }
            Action::Effect { sprite, x, y } => {
                self.spawn_effect(sprite.clone(), vec2(*x, *y));
                Step::Continue
            }
            Action::DamageArea { x, y, radius, damage } => {
                let ts = self.ts();
                self.damage_at(vec2(*x, *y) * ts, *damage, *radius);
                Step::Continue
            }
            Action::LightAt { x, y, radius, seconds, color, flicker } => {
                let ts = self.ts();
                self.light_at(vec2(*x, *y) * ts, *radius, *seconds, *color, *flicker);
                Step::Continue
            }
            Action::HealArea { x, y, radius, amount } => {
                let ts = self.ts();
                self.heal_area(vec2(*x, *y) * ts, *radius, *amount);
                Step::Continue
            }
            Action::Equip { target, item } => {
                if target == "player" {
                    if self.world.item_count(item) == 0 {
                        self.world.give_item(item, 1);
                    }
                    self.equip_item(item);
                } else if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.npc_equip(i, item);
                } else {
                    eprintln!("[events] Equip: no character '{target}' on this map");
                }
                Step::Continue
            }
            Action::Unequip { target, slot } => {
                if target == "player" {
                    self.unequip(*slot, None);
                } else if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.npc_unequip(i, *slot);
                }
                Step::Continue
            }
            Action::GiveItemTo { target, item, count } | Action::TakeItemFrom { target, item, count } => {
                let give = matches!(action, Action::GiveItemTo { .. });
                if target == "player" {
                    if give {
                        self.world.give_item(item, *count);
                    } else {
                        self.world.take_item(item, *count);
                    }
                } else if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.npc_give(i, item, if give { *count as i64 } else { -(*count as i64) });
                } else {
                    eprintln!("[events] {}: no character '{target}' on this map", action.label());
                }
                Step::Continue
            }
            Action::SetUiVisible { visible } => {
                self.ui_hidden = !*visible;
                Step::Continue
            }
            Action::Cutscene { function } => {
                self.begin_cutscene(function);
                Step::Insert(vec![(Action::RunScript { function: function.clone() }, ctx.clone()), (Action::EndCutscene, ctx.clone())])
            }
            Action::EndCutscene => {
                self.end_cutscene();
                Step::Continue
            }
            Action::WalkTo { target, x, y } => {
                if self.walk_to(target, ctx, *x, *y) {
                    Step::Block
                } else {
                    Step::Continue
                }
            }
            Action::Approach { target, to } => {
                if self.approach(target, to, ctx) {
                    Step::Block
                } else {
                    Step::Continue
                }
            }
            Action::Place { target, x, y } => {
                self.place(target, ctx, *x, *y);
                Step::Continue
            }
            Action::Dim { amount, seconds } => {
                self.dim_to(*amount, *seconds);
                Step::Continue
            }
            Action::SetSpawners { map, zone, active } => {
                self.set_spawners(map, zone, *active);
                Step::Continue
            }
            Action::DefeatEnemies => {
                self.defeat_enemies();
                Step::Continue
            }
            Action::Autosave => {
                // The event that queued it is still running; it saves once that is done.
                self.autosave_pending = true;
                Step::Continue
            }
            Action::GiveXp { amount } => {
                self.give_xp(*amount);
                Step::Continue
            }
            Action::ApplyEffect { target, effect } => {
                self.apply_effect(target, ctx.owner, effect);
                Step::Continue
            }
            Action::CureEffect { target, kind } => {
                self.cure_effect(target, ctx.owner, *kind);
                Step::Continue
            }
            Action::Shake { seconds, strength } => {
                self.shake_screen(*seconds, *strength);
                Step::Continue
            }
            Action::SetWeather { map, weather } => {
                let map = if map.is_empty() { self.loaded.map.name.clone() } else { map.clone() };
                match weather {
                    Some(w) => self.world.weather.insert(map, crate::map::Weather::new(w.kind, w.intensity)),
                    None => self.world.weather.remove(&map),
                };
                Step::Continue
            }
            Action::Sleep { hours } => {
                if self.start_sleep(*hours) {
                    Step::Block
                } else {
                    Step::Continue
                }
            }
            Action::Lockpick { difficulty, var } => {
                // Skill below the difficulty makes it harder (the skill system may be off: 0).
                let gap = *difficulty as i32 - self.lockpick_level() as i32;
                self.start_lockpick(minigame::PickTarget::Var(var.clone()), gap);
                Step::Block
            }
            Action::UnlockAchievement { id } => {
                self.unlock_achievement(id);
                Step::Continue
            }
            Action::OpenCrafting { station, title } => {
                self.open_crafting(station, title, ctx.owner);
                Step::Continue
            }
            Action::OpenShop { target } => {
                match self.entity_index(target, ctx.owner) {
                    Some(i) if self.try_open_shop(i, false) => {}
                    _ => eprintln!("Open shop: '{target}' is not a merchant on this map"),
                }
                Step::Continue
            }
            Action::SetFrame { target, frame } => {
                if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.loaded.entities[i].frame = *frame;
                    self.persist(i);
                }
                Step::Continue
            }
            Action::Move { target, dx, dy } => {
                let delta = vec2(*dx as f32, *dy as f32) * ts;
                if target == "player" {
                    self.player.move_target = Some(self.player.pos + delta);
                } else if let Some(i) = self.entity_index(target, ctx.owner) {
                    let b = &mut self.loaded.entities[i].body;
                    b.move_target = Some(b.pos + delta);
                }
                Step::Block
            }
            Action::Face { target, facing } => {
                if target == "player" {
                    self.player.facing = *facing;
                } else if let Some(i) = self.entity_index(target, ctx.owner) {
                    self.loaded.entities[i].body.facing = *facing;
                }
                Step::Continue
            }
            Action::Emit { name } => {
                Step::Insert(self.collect(&GameEvent::Custom { name: name.clone() }, &Source::Broadcast))
            }
            Action::RunScript { function } => {
                let actions = self.scripts.call(&mut self.world, function, ctx.owner);
                Step::Insert(actions.into_iter().map(|a| (a, ctx.clone())).collect())
            }
            // Handled by the runner itself.
            Action::GiveItem { .. }
            | Action::TakeItem { .. }
            | Action::SetFlag { .. }
            | Action::SetVar { .. }
            | Action::Wait { .. }
            | Action::If { .. } => Step::Continue,
        }
    }

    fn blocked(&mut self) -> bool {
        let walking = self.player.move_target.is_some() || self.loaded.entities.iter().any(|e| e.body.move_target.is_some());
        if !walking {
            // Characters that walked up to someone turn to face them.
            self.apply_face_after();
        }
        self.ui.dialog_open() || !matches!(self.fade, Fade::None) || walking || self.minigame_open() || self.sleeping()
    }

    fn script_condition(&mut self, function: &str, ctx: &ActionCtx) -> bool {
        self.scripts.condition(&mut self.world, function, ctx.owner)
    }

    fn pose(&mut self, target: &str, ctx: &ActionCtx) -> String {
        if target.eq_ignore_ascii_case("player") {
            return self.pose_name().to_string();
        }
        self.entity_index(target, ctx.owner).map_or(String::new(), |i| self.npc_pose(i).to_string())
    }

    fn prop_state(&mut self, target: &str, ctx: &ActionCtx) -> Option<String> {
        let i = self.entity_index(target, ctx.owner)?;
        let e = &self.loaded.entities[i];
        self.project.db.prop(&e.def).map(|d| d.state_name(e.state).to_string())
    }

    fn item_given(&mut self, item: &str, count: u32, lost: u32) {
        let name = self.project.db.item(item).map_or(item.to_string(), |d| d.name.clone());
        if count > 0 {
            self.audio.play(&self.project.config.sfx.pickup);
            self.ui.toast(if count > 1 { format!("Got {name} x{count}") } else { format!("Got {name}") });
        }
        if lost > 0 {
            self.ui.toast(format!("Inventory full: {name} x{lost} lost"));
        }
    }
}

#[cfg(test)]
mod food_tests {
    use super::*;

    #[test]
    fn food_heals_more_with_the_skill() {
        assert_eq!(food_bonus(Action::Heal { amount: 4 }, 50.0), Action::Heal { amount: 6 });
        assert_eq!(food_bonus(Action::Heal { amount: 4 }, 0.0), Action::Heal { amount: 4 });
        assert_eq!(food_bonus(Action::Heal { amount: -2 }, 50.0), Action::Heal { amount: -2 }, "damage stays");
        let Action::If { then, .. } = food_bonus(Action::If { conditions: vec![], then: vec![Action::Heal { amount: 2 }], otherwise: vec![] }, 100.0) else { panic!() };
        assert_eq!(then, vec![Action::Heal { amount: 4 }]);
        // Recipes and items from before cooking.
        let r: crate::crafting::RecipeDef = ron::from_str("(id: \"a\", result: \"b\")").unwrap();
        assert_eq!(r.station_state, "");
        let it: crate::defs::ItemDef = ron::from_str(r#"(id: "apple", name: "Apple", icon: (sheet: "items", frame: 3))"#).unwrap();
        assert!(!it.food);
    }
}
