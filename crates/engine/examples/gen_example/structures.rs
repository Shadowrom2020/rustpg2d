//! Structures (multi-tile assets placed with one click in the editor): a basic set of houses,
//! walls, fences, rooms and outdoor features. Each one is painted on a small scratch map and
//! captured with `StructureDef::capture`, exactly as the editor's "New from map area" does.
//! Terrain-based ones go into the example project; city / dungeon / prop-based ones are
//! library-only extras of the starter pack.

use rustpg2d_engine::defs::StructureDef;
use rustpg2d_engine::entity::{EntityInstance, EntityKind};
use rustpg2d_engine::map::{CollisionOverride, Map, Tile};
use rustpg2d_engine::project::TILE_SIZE;

use crate::art::t;

/// Scratch map for painting one structure.
struct P {
    m: Map,
}

impl P {
    fn new(w: u32, h: u32) -> Self {
        P { m: Map::new("scratch", w, h) }
    }
    fn set(&mut self, layer: usize, ts: &str, x: i32, y: i32, tile: u32) {
        if self.m.in_bounds(x, y) {
            let s = self.m.tileset_slot(ts);
            self.m.layers[layer].set(x as u32, y as u32, Tile::new(s, tile));
        }
    }
    #[allow(clippy::too_many_arguments)] // layer, tileset, rectangle and tile read best inline
    fn fill(&mut self, layer: usize, ts: &str, x: i32, y: i32, w: i32, h: i32, tile: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(layer, ts, xx, yy, tile);
            }
        }
    }
    /// A walkable tile (door gap) in an otherwise solid row.
    fn walkable(&mut self, x: i32, y: i32) {
        self.m.set_collision(x as u32, y as u32, CollisionOverride::Walkable);
    }
    /// An entity standing on tile `(tx, ty)`; props `w` tiles wide are centered like the editor
    /// snaps them (odd widths on the tile center, even widths on its left edge).
    fn ent(&mut self, kind: EntityKind, def: &str, tx: i32, ty: i32, w: i32) -> &mut EntityInstance {
        let ts = TILE_SIZE as f32;
        let uid = self.m.alloc_uid();
        let x = if w % 2 == 0 { tx as f32 * ts } else { (tx as f32 + 0.5) * ts };
        self.m.entities.push(EntityInstance::new(uid, kind, def, x, (ty as f32 + 1.0) * ts - 1.0));
        self.m.entities.last_mut().unwrap()
    }
    fn prop(&mut self, def: &str, tx: i32, ty: i32, w: i32) {
        self.ent(EntityKind::Prop, def, tx, ty, w);
    }
    fn done(self, id: &str, name: &str, category: &str, ground: bool) -> StructureDef {
        let (w, h) = (self.m.width, self.m.height);
        let mut d = StructureDef::capture(&self.m, id, name, (0, 0, w, h), ground);
        d.category = category.into();
        d
    }
}

const TR: &str = "terrain";

/// Cottage like the village houses: roof, eaves, wall with windows and a walkable door.
fn cottage(w: i32) -> P {
    let mut p = P::new(w as u32, 5);
    p.fill(1, TR, 0, 0, w, 2, t::ROOF);
    p.fill(1, TR, 0, 2, w, 1, t::ROOF_EDGE);
    p.fill(1, TR, 0, 3, w, 2, t::HOUSE_WALL);
    p.set(1, TR, 1, 3, t::WINDOW);
    p.set(1, TR, w - 2, 3, t::WINDOW);
    p.set(1, TR, w / 2, 4, t::DOOR);
    p.walkable(w / 2, 4);
    p
}

/// Interior room: wall with windows at the top, black void around, floor and a door gap at the
/// bottom.
fn room(w: i32, h: i32, floor: u32, wall: u32) -> P {
    let mut p = P::new(w as u32, h as u32);
    p.fill(0, TR, 0, 0, w, h, t::VOID);
    p.fill(0, TR, 1, 2, w - 2, h - 3, floor);
    p.fill(1, TR, 1, 0, w - 2, 2, wall);
    p.fill(1, TR, 0, 0, 1, h, t::VOID);
    p.fill(1, TR, w - 1, 0, 1, h, t::VOID);
    p.fill(1, TR, 0, h - 1, w, 1, t::VOID);
    // Door at the bottom center.
    p.set(0, TR, w / 2, h - 1, floor);
    p.set(1, TR, w / 2, h - 1, t::DOOR);
    p.walkable(w / 2, h - 1);
    p
}

/// Roofs of village buildings: (roof, eave, chimney).
#[derive(Clone, Copy)]
pub enum Roof {
    Red,
    Slate,
    Thatch,
}

impl Roof {
    pub fn tiles(self) -> (u32, u32, Option<u32>) {
        match self {
            Roof::Red => (t::ROOF, t::ROOF_EDGE, Some(t::CHIMNEY)),
            Roof::Slate => (t::ROOF_SLATE, t::ROOF_SLATE_EDGE, Some(t::CHIMNEY_SLATE)),
            Roof::Thatch => (t::ROOF_THATCH, t::ROOF_THATCH_EDGE, None),
        }
    }
}

/// Walls of village buildings: (wall, lit window, door).
#[derive(Clone, Copy)]
pub enum Wall {
    Timber,
    TimberX,
    Stone,
}

impl Wall {
    pub fn tiles(self) -> (u32, u32, u32) {
        match self {
            Wall::Timber => (t::HOUSE_WALL, t::WINDOW, t::DOOR),
            Wall::TimberX => (t::TIMBER_WALL_X, t::WINDOW, t::DOOR),
            Wall::Stone => (t::STONE_HOUSE, t::STONE_WINDOW, t::STONE_DOOR),
        }
    }
}

/// A village building `w` wide: two roof rows and the eave, `walls` wall rows with windows on
/// the first one, a chimney near the right end, a walkable door in the middle of the bottom row.
fn building(w: i32, walls: i32, roof: Roof, wall: Wall) -> P {
    let (r, eave, chimney) = roof.tiles();
    let (wt, window, door) = wall.tiles();
    let h = 3 + walls;
    let mut p = P::new(w as u32, h as u32);
    p.fill(1, TR, 0, 0, w, 2, r);
    p.fill(1, TR, 0, 2, w, 1, eave);
    if let Some(c) = chimney {
        p.set(1, TR, w - 2, 0, c);
    }
    p.fill(1, TR, 0, 3, w, walls, wt);
    for x in (1..w - 1).step_by(3) {
        if x != w / 2 {
            p.set(1, TR, x, 3, window);
        }
    }
    p.set(1, TR, w - 2, 3, window);
    p.set(1, TR, w / 2, h - 1, door);
    p.walkable(w / 2, h - 1);
    p
}

/// A fenced enclosure `w` x `h` with posts at the corners and a gap of `gap` tiles in the
/// middle of the bottom side.
fn fenced(p: &mut P, w: i32, h: i32, gap: i32) {
    p.fill(1, TR, 0, 0, w, 1, t::FENCE);
    p.fill(1, TR, 0, h - 1, w, 1, t::FENCE);
    p.fill(1, TR, 0, 0, 1, h, t::FENCE_V);
    p.fill(1, TR, w - 1, 0, 1, h, t::FENCE_V);
    for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        p.set(1, TR, x, y, t::FENCE_POST);
    }
    let g0 = (w - gap) / 2;
    for x in g0..g0 + gap {
        p.m.layers[1].set(x as u32, (h - 1) as u32, Tile::EMPTY);
    }
}

/// Village structures (0.11.0): the buildings, market, farm, graveyard, dock and rooms of
/// Emberbrook.
fn village_structures(out: &mut Vec<StructureDef>) {
    // --- Buildings (a row of ground in front of the door)
    let mut p = with_front(building(11, 2, Roof::Slate, Wall::TimberX), t::COBBLE);
    p.prop("sign_tavern", 4, 4, 1);
    p.prop("keg", 1, 5, 1);
    p.prop("flower_box", 8, 5, 1);
    out.push(p.done("tavern_front", "Tavern (slate roof, sign)", "Buildings", true));
    let mut p = with_front(building(7, 2, Roof::Red, Wall::Timber), t::PATH);
    p.prop("sign_bakery", 2, 4, 1);
    p.prop("flour_sacks", 5, 5, 1);
    out.push(p.done("bakery_front", "Bakery (sign, flour sacks)", "Buildings", true));
    let mut p = with_front(building(7, 2, Roof::Slate, Wall::Stone), t::STONE_FLOOR);
    p.prop("sign_smithy", 2, 4, 1);
    p.prop("woodpile", 6, 5, 2);
    out.push(p.done("smithy_stone", "Stone smithy (sign, woodpile)", "Buildings", true));
    let mut p = building(9, 4, Roof::Slate, Wall::Stone);
    for x in [1, 3, 5, 7] {
        p.set(1, TR, x, 3, t::STONE_HOUSE);
        p.set(1, TR, x, 4, t::CHAPEL_WINDOW);
    }
    p.set(1, TR, 7, 3, t::STONE_WINDOW);
    let mut p = with_front(p, t::STONE_FLOOR);
    p.prop("candle_stand", 2, 8, 1);
    p.prop("candle_stand", 6, 8, 1);
    out.push(p.done("chapel", "Chapel (stained glass)", "Buildings", true));
    let mut p = with_front(building(7, 2, Roof::Thatch, Wall::Timber), t::PATH);
    p.prop("hay_bale", 1, 5, 1);
    p.prop("flower_pot", 5, 5, 1);
    out.push(p.done("farmhouse_thatch", "Farmhouse (thatched roof)", "Buildings", true));
    let mut p = P::new(10, 6);
    p.prop("windmill", 2, 5, 4);
    let house = building(5, 2, Roof::Thatch, Wall::Stone);
    for y in 0..5 {
        for x in 0..5 {
            if let Some((s, i)) = house.m.layers[1].get(x, y).parts() {
                let ts = house.m.tilesets[s].clone();
                p.set(1, &ts, 5 + x as i32, 1 + y as i32, i);
            }
        }
    }
    p.walkable(7, 5);
    p.prop("flour_sacks", 9, 5, 1);
    out.push(p.done("mill", "Windmill and mill house", "Buildings", false));

    // --- Market, farm, graveyard
    let mut p = P::new(9, 3);
    p.fill(0, TR, 0, 0, 9, 3, t::COBBLE);
    for (k, def) in ["stall_produce", "stall_fish", "stall_bread"].iter().enumerate() {
        p.prop(def, 1 + 3 * k as i32, 2, 3);
    }
    out.push(p.done("market_row", "Market row (three stalls)", "Outdoor", true));
    let mut p = P::new(9, 7);
    p.fill(0, TR, 1, 1, 7, 5, t::STRAW_FLOOR);
    fenced(&mut p, 9, 7, 2);
    p.prop("double_gate", 4, 6, 2);
    p.prop("trough", 2, 2, 2);
    p.prop("hay_bale", 7, 2, 1);
    out.push(p.done("animal_pen", "Animal pen (trough, gate)", "Outdoor", true));
    let mut p = P::new(7, 6);
    fenced(&mut p, 7, 6, 1);
    for (x, y) in [(1, 2), (3, 2), (5, 2)] {
        p.set(0, TR, x, y + 1, t::GRAVE_MOUND);
    }
    p.prop("tombstone", 1, 2, 1);
    p.prop("grave_cross", 3, 2, 1);
    p.prop("tombstone", 5, 2, 1);
    p.set(0, TR, 1, 4, t::GRAVE_MOUND);
    p.prop("grave_cross", 5, 4, 1);
    out.push(p.done("graveyard", "Graveyard (fence, graves)", "Outdoor", true));
    let mut p = P::new(8, 7);
    p.fill(0, TR, 0, 0, 8, 7, t::FLOWER_BED);
    p.prop("apple_tree", 2, 3, 2);
    p.prop("apple_tree", 6, 3, 2);
    p.prop("apple_tree", 4, 6, 2);
    p.prop("beehive", 7, 6, 1);
    out.push(p.done("orchard", "Orchard (apple trees, beehive)", "Outdoor", true));
    let mut p = P::new(4, 6);
    p.fill(0, TR, 0, 0, 4, 6, t::WATER);
    p.fill(0, TR, 1, 0, 1, 5, t::DOCK);
    p.set(0, TR, 1, 5, t::DOCK_EDGE);
    p.prop("boat", 3, 3, 2);
    out.push(p.done("dock", "Fishing dock (boat)", "Outdoor", true));
    let mut p = P::new(8, 3);
    p.fill(1, TR, 0, 0, 2, 2, t::STONE_WALL);
    p.fill(1, TR, 6, 0, 2, 2, t::STONE_WALL);
    p.fill(1, TR, 2, 0, 1, 2, t::GATE_PILLAR);
    p.fill(1, TR, 5, 0, 1, 2, t::GATE_PILLAR);
    p.prop("double_gate", 4, 1, 2);
    p.prop("brazier", 2, 2, 1);
    p.prop("brazier", 5, 2, 1);
    out.push(p.done("gate_towers", "Town gate (pillars, braziers)", "Walls & fences", false));

    // --- Rooms
    let mut p = room(12, 10, t::WOOD_FLOOR, t::TIMBER_WALL_X);
    p.set(1, TR, 3, 1, t::WINDOW);
    p.set(1, TR, 8, 1, t::WINDOW);
    p.fill(1, TR, 1, 3, 6, 1, t::COUNTER);
    p.prop("keg_rack", 3, 2, 2);
    p.prop("keg", 5, 2, 1);
    p.prop("fireplace", 10, 3, 2);
    for (x, y) in [(3, 6), (8, 6)] {
        p.prop("tavern_table", x, y, 2);
        p.prop("stool", x - 1, y - 1, 1);
        p.prop("stool_up", x, y + 1, 1);
    }
    p.fill(0, TR, 6, 2, 1, 7, t::CARPET);
    out.push(p.done("room_tavern", "Tavern room (counter, kegs, tables)", "Rooms", true));
    let mut p = room(10, 8, t::STONE_FLOOR, t::STONE_WALL);
    p.prop("forge", 7, 3, 2);
    p.prop("bellows", 5, 3, 1);
    p.prop("anvil", 5, 5, 1);
    p.prop("weapon_rack", 2, 3, 2);
    p.prop("grindstone", 8, 5, 1);
    p.prop("tool_rack", 3, 6, 2);
    out.push(p.done("room_smithy", "Smithy (forge, anvil, racks)", "Rooms", true));
    let mut p = room(10, 8, t::TILED_FLOOR, t::HOUSE_WALL);
    p.set(1, TR, 2, 1, t::WINDOW);
    p.prop("oven", 8, 3, 2);
    p.prop("bread_shelf", 2, 3, 2);
    p.fill(1, TR, 3, 4, 4, 1, t::COUNTER);
    p.prop("dough_table", 7, 6, 2);
    p.prop("flour_sacks", 1, 6, 1);
    out.push(p.done("room_bakery", "Bakery (oven, shelves, counter)", "Rooms", true));
    let mut p = room(11, 12, t::TILED_FLOOR, t::STONE_WALL);
    for x in [2, 8] {
        p.set(1, TR, x, 1, t::CHAPEL_WINDOW);
    }
    p.fill(0, TR, 5, 2, 1, 9, t::CARPET);
    p.prop("altar", 6, 3, 2);
    p.prop("candle_stand", 3, 2, 1);
    p.prop("candle_stand", 7, 2, 1);
    for y in [5, 7, 9] {
        p.prop("pew", 3, y, 2);
        p.prop("pew", 8, y, 2);
    }
    p.prop("bell_rope", 1, 2, 1);
    out.push(p.done("room_chapel", "Chapel nave (altar, pews)", "Rooms", true));
}

/// `p` with one more row of `ground` below it (the street in front of a building).
fn with_front(p: P, ground: u32) -> P {
    let (w, h) = (p.m.width, p.m.height);
    let mut q = P::new(w, h + 1);
    for layer in 0..p.m.layers.len().min(q.m.layers.len()) {
        for y in 0..h {
            for x in 0..w {
                if let Some((s, i)) = p.m.layers[layer].get(x, y).parts() {
                    let ts = p.m.tilesets[s].clone();
                    q.set(layer, &ts, x as i32, y as i32, i);
                }
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let c = p.m.collision_at(x, y);
            q.m.set_collision(x, y, c);
        }
    }
    q.fill(0, TR, 0, h as i32, w as i32, 1, ground);
    q
}

/// Terrain-based structures (for the example project; they also end up in the starter pack).
pub fn example_structures() -> Vec<StructureDef> {
    let mut out = Vec::new();

    // --- Buildings
    out.push(cottage(5).done("cottage_small", "Small cottage", "Buildings", false));
    out.push(cottage(7).done("cottage", "Cottage", "Buildings", false));
    let mut p = P::new(7, 6);
    for (x, tile) in [(0, t::ROOF), (1, t::ROOF)] {
        p.fill(1, TR, 0, x, 7, 1, tile);
    }
    p.fill(1, TR, 0, 2, 7, 1, t::ROOF_EDGE);
    p.fill(1, TR, 0, 3, 7, 2, t::HOUSE_WALL);
    p.set(1, TR, 1, 3, t::WINDOW);
    p.set(1, TR, 5, 3, t::WINDOW);
    p.set(1, TR, 3, 4, t::DOOR);
    p.walkable(3, 4);
    p.fill(0, TR, 0, 5, 7, 1, t::PATH);
    p.prop("sign", 2, 5, 1);
    p.prop("barrel", 5, 5, 1);
    out.push(p.done("shop_front", "Shop front (sign, barrel)", "Buildings", true));

    // --- Walls & fences
    let mut p = P::new(5, 1);
    p.fill(1, TR, 0, 0, 5, 1, t::STONE_WALL);
    out.push(p.done("stone_wall_h", "Stone wall, horizontal (5)", "Walls & fences", false));
    let mut p = P::new(1, 5);
    p.fill(1, TR, 0, 0, 1, 5, t::STONE_WALL);
    out.push(p.done("stone_wall_v", "Stone wall, vertical (5)", "Walls & fences", false));
    let mut p = P::new(7, 6);
    p.fill(1, TR, 0, 0, 7, 1, t::STONE_WALL);
    p.fill(1, TR, 0, 0, 1, 6, t::STONE_WALL);
    p.fill(1, TR, 6, 0, 1, 6, t::STONE_WALL);
    p.fill(1, TR, 0, 5, 3, 1, t::STONE_WALL);
    p.fill(1, TR, 4, 5, 3, 1, t::STONE_WALL);
    out.push(p.done("stone_yard", "Walled yard (open south)", "Walls & fences", false));
    let mut p = P::new(5, 1);
    p.fill(1, TR, 0, 0, 5, 1, t::FENCE);
    out.push(p.done("fence_h", "Fence, horizontal (5)", "Walls & fences", false));
    let mut p = P::new(1, 5);
    p.fill(1, TR, 0, 0, 1, 5, t::FENCE);
    out.push(p.done("fence_v", "Fence, vertical (5)", "Walls & fences", false));
    let mut p = P::new(6, 5);
    p.fill(0, TR, 1, 1, 4, 3, t::FLOWERS);
    p.fill(1, TR, 0, 0, 6, 1, t::FENCE);
    p.fill(1, TR, 0, 0, 1, 5, t::FENCE);
    p.fill(1, TR, 5, 0, 1, 5, t::FENCE);
    p.fill(1, TR, 0, 4, 2, 1, t::FENCE);
    p.fill(1, TR, 4, 4, 2, 1, t::FENCE);
    out.push(p.done("garden", "Fenced flower garden", "Walls & fences", true));

    // --- Rooms (for interior maps)
    let mut p = room(9, 8, t::WOOD_FLOOR, t::HOUSE_WALL);
    p.set(1, TR, 2, 1, t::WINDOW);
    p.set(1, TR, 6, 1, t::WINDOW);
    p.set(1, TR, 1, 2, t::BED);
    p.set(1, TR, 1, 3, t::BED_FOOT);
    p.fill(1, TR, 4, 2, 2, 1, t::SHELF);
    p.set(1, TR, 7, 4, t::TABLE);
    p.fill(0, TR, 3, 4, 3, 2, t::RUG);
    out.push(p.done("room_bedroom", "Bedroom (bed, shelf, table)", "Rooms", true));
    let mut p = room(7, 6, t::STONE_FLOOR, t::STONE_WALL);
    p.prop("barrel", 1, 2, 1);
    p.prop("barrel", 1, 3, 1);
    p.prop("chest", 5, 2, 1);
    out.push(p.done("room_storage", "Storage room (barrels, chest)", "Rooms", true));
    let mut p = room(6, 5, t::WOOD_FLOOR, t::HOUSE_WALL);
    p.set(1, TR, 2, 1, t::WINDOW);
    out.push(p.done("room_small", "Small empty room", "Rooms", true));

    // --- Outdoor features
    let mut p = P::new(7, 5);
    p.fill(0, TR, 0, 0, 7, 5, t::SAND);
    p.fill(0, TR, 1, 1, 5, 3, t::WATER);
    out.push(p.done("pond", "Pond with sandy shore", "Outdoor", true));
    let mut p = P::new(5, 1);
    p.fill(0, TR, 0, 0, 5, 1, t::BRIDGE);
    out.push(p.done("bridge_h", "Wooden bridge (5)", "Outdoor", true));
    let mut p = P::new(5, 5);
    p.fill(0, TR, 0, 0, 5, 5, t::PATH);
    p.prop("well", 2, 2, 1);
    p.prop("barrel", 4, 1, 1);
    out.push(p.done("well_square", "Village well square", "Outdoor", true));
    let mut p = P::new(5, 5);
    for (x, y) in [(1, 1), (3, 2), (0, 3), (2, 4), (4, 4)] {
        p.set(1, TR, x, y, t::TRUNK);
        p.set(2, TR, x, y - 1, t::CANOPY);
    }
    out.push(p.done("tree_grove", "Grove of trees", "Outdoor", false));
    let mut p = P::new(4, 5);
    for (x, y) in [(0, 1), (2, 2), (1, 4), (3, 4)] {
        p.set(1, TR, x, y, t::PINE_TRUNK);
        p.set(2, TR, x, y - 1, t::PINE_TOP);
    }
    out.push(p.done("pine_grove", "Grove of pines", "Outdoor", false));
    let mut p = P::new(3, 2);
    p.set(1, TR, 0, 0, t::ROCK);
    p.set(1, TR, 1, 0, t::BUSH);
    p.set(1, TR, 2, 1, t::ROCK);
    p.set(1, TR, 0, 1, t::MUSHROOM);
    out.push(p.done("rocks_bushes", "Rocks and bushes", "Outdoor", false));
    village_structures(&mut out);
    out
}

const CITY: &str = "city";
const DG: &str = "dungeon";

/// City house: `roof`/`edge` shingle tiles, `wall` tiles, door in the bottom row.
fn town_house(w: i32, roof: u32, edge: u32, wall: u32) -> P {
    let mut p = P::new(w as u32, 6);
    p.fill(1, CITY, 0, 0, w, 2, roof);
    p.fill(1, CITY, 0, 2, w, 1, edge);
    p.fill(1, CITY, 0, 3, w, 3, wall);
    p.set(1, CITY, w / 2, 5, 12);
    p.walkable(w / 2, 5);
    p
}

/// Dungeon room: ceiling (void) around, a wall row at the top, flagstone floor, door below.
fn dungeon_room(w: i32, h: i32) -> P {
    let mut p = P::new(w as u32, h as u32);
    p.fill(0, DG, 1, 2, w - 2, h - 3, 0);
    p.fill(1, DG, 0, 0, w, 1, 16);
    p.fill(1, DG, 1, 1, w - 2, 1, 8);
    p.fill(1, DG, 0, 0, 1, h, 16);
    p.fill(1, DG, w - 1, 0, 1, h, 16);
    p.fill(1, DG, 0, h - 1, w, 1, 16);
    p.set(0, DG, w / 2, h - 1, 0);
    p.set(1, DG, w / 2, h - 1, 6);
    p.walkable(w / 2, h - 1);
    p
}

/// Library-only structures (city and dungeon tilesets, props of the starter library).
pub fn library_structures() -> Vec<StructureDef> {
    let mut out = Vec::new();

    // --- City buildings
    let mut p = town_house(5, 16, 17, 9);
    p.set(1, CITY, 1, 3, 14);
    p.set(1, CITY, 3, 3, 14);
    p.set(1, CITY, 1, 5, 15);
    p.set(1, CITY, 3, 4, 13);
    out.push(p.done("townhouse_blue", "Town house (blue roof)", "Buildings", false));
    let mut p = town_house(7, 18, 19, 8);
    p.set(1, CITY, 1, 3, 14);
    p.set(1, CITY, 5, 3, 14);
    p.fill(1, CITY, 1, 4, 2, 1, 11);
    p.fill(1, CITY, 4, 4, 2, 1, 11);
    out.push(p.done("brick_shop", "Brick shop (orange roof)", "Buildings", false));
    let mut p = P::new(4, 6);
    p.fill(1, CITY, 0, 0, 4, 6, 10);
    p.fill(1, CITY, 1, 1, 2, 1, 31);
    p.set(1, CITY, 1, 5, 12);
    p.walkable(1, 5);
    out.push(p.done("watchtower", "Stone watchtower", "Buildings", false));

    // --- City walls & fences
    let mut p = P::new(6, 2);
    p.fill(1, CITY, 0, 0, 6, 2, 10);
    out.push(p.done("city_wall_h", "City wall, horizontal (6)", "Walls & fences", false));
    let mut p = P::new(2, 6);
    p.fill(1, CITY, 0, 0, 2, 6, 10);
    out.push(p.done("city_wall_v", "City wall, vertical (6)", "Walls & fences", false));
    let mut p = P::new(5, 1);
    p.fill(1, CITY, 0, 0, 5, 1, 21);
    out.push(p.done("iron_fence", "Iron fence (5)", "Walls & fences", false));
    let mut p = P::new(5, 1);
    p.fill(1, CITY, 0, 0, 5, 1, 22);
    out.push(p.done("hedge", "Hedge (5)", "Walls & fences", false));

    // --- City outdoor
    let mut p = P::new(7, 7);
    p.fill(0, CITY, 0, 0, 7, 7, 0);
    p.prop("fountain", 3, 3, 2);
    p.prop("bench", 1, 5, 2);
    p.prop("bench", 5, 5, 2);
    p.prop("street_lamp", 0, 1, 1);
    p.prop("street_lamp", 6, 1, 1);
    out.push(p.done("plaza", "Plaza with fountain", "Outdoor", true));
    let mut p = P::new(7, 4);
    p.fill(0, CITY, 0, 0, 7, 4, 0);
    p.prop("market_stall", 1, 2, 3);
    p.prop("market_stall", 5, 2, 3);
    p.prop("crate", 3, 3, 1);
    p.prop("sack", 3, 2, 1);
    out.push(p.done("market", "Market stalls", "Outdoor", true));
    let mut p = P::new(5, 4);
    p.fill(0, CITY, 0, 0, 5, 1, 0);
    p.fill(0, CITY, 0, 1, 5, 2, 23);
    p.fill(0, CITY, 0, 3, 5, 1, 0);
    out.push(p.done("canal", "Canal section", "Outdoor", true));
    let mut p = P::new(5, 4);
    p.prop("tent", 1, 1, 2);
    p.prop("campfire", 3, 2, 1);
    p.prop("bench", 3, 3, 2);
    p.prop("sack", 0, 3, 1);
    out.push(p.done("campsite", "Campsite (tent, campfire)", "Outdoor", false));

    // --- Dungeon
    let mut p = dungeon_room(7, 6);
    p.set(1, DG, 2, 1, 10);
    p.set(1, DG, 4, 1, 10);
    p.set(0, DG, 1, 4, 20);
    out.push(p.done("dungeon_room", "Dungeon room (torches)", "Dungeon", true));
    let mut p = dungeon_room(7, 7);
    p.set(1, DG, 3, 1, 22);
    p.set(1, DG, 1, 2, 15);
    p.set(1, DG, 5, 2, 15);
    p.fill(0, DG, 3, 2, 1, 4, 3);
    p.set(2, DG, 1, 2, 18);
    out.push(p.done("dungeon_shrine", "Shrine room (altar, pillars)", "Dungeon", true));
    let mut p = P::new(6, 3);
    p.fill(1, DG, 0, 0, 6, 1, 8);
    p.fill(0, DG, 0, 1, 6, 1, 0);
    p.fill(1, DG, 0, 2, 6, 1, 9);
    out.push(p.done("corridor_h", "Corridor, horizontal (6)", "Dungeon", true));
    let mut p = P::new(3, 6);
    p.fill(1, DG, 0, 0, 1, 6, 16);
    p.fill(0, DG, 1, 0, 1, 6, 0);
    p.fill(1, DG, 2, 0, 1, 6, 16);
    out.push(p.done("corridor_v", "Corridor, vertical (6)", "Dungeon", true));
    let mut p = P::new(4, 4);
    p.fill(1, DG, 0, 0, 4, 1, 8);
    p.set(1, DG, 1, 0, 11);
    p.fill(0, DG, 0, 1, 4, 2, 0);
    p.set(0, DG, 1, 2, 17);
    p.set(0, DG, 2, 1, 21);
    p.fill(1, DG, 0, 3, 4, 1, 14);
    out.push(p.done("prison_cell", "Prison cell (bars)", "Dungeon", true));
    out
}
