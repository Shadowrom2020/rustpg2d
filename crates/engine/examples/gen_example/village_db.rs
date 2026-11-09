//! The village's definitions (0.11.0): the props of `village.png` and `props2.png`, the goods of
//! `items2.png`, the villagers, children and animals, their sheets, and the recipes of the mill,
//! the bakery and the apothecary. Everything here also goes into the starter library.

use rustpg2d_engine::crafting::RecipeDef;
use rustpg2d_engine::defs::*;
use rustpg2d_engine::effects::EffectKind;
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::events::*;
use rustpg2d_engine::inventory::ItemStack;

use crate::art::{animals, it2, portrait, village};
use crate::art::R;
use crate::{dialog, dish, regen, trig};

fn prop(id: &str, name: &str, sprite: &str, solid: bool, hitbox: [f32; 2]) -> PropDef {
    PropDef {
        id: id.into(),
        name: name.into(),
        sprite: village::sprite(sprite),
        solid,
        hitbox,
        triggers: vec![],
        container: None,
        lock: None,
        default_state: default_state_name(),
        states: vec![],
        light: None,
        use_spot: None,
        gather: None,
        regrow: None,
        plot: false,
    }
}

fn state(name: &str, sprite: &str, solid: bool) -> PropState {
    PropState { name: name.into(), sprite: village::sprite(sprite), solid, light: None }
}

fn interact(actions: Vec<Action>) -> Vec<Trigger> {
    vec![trig(TriggerOn::Interact, vec![], actions)]
}

fn station(station: &str, title: &str) -> Vec<Trigger> {
    interact(vec![Action::OpenCrafting { station: station.into(), title: title.into() }])
}

/// A fire that is lit at first (warm light) and goes out ("out"); coal lights it again.
fn fire(mut d: PropDef, out: &str, light: LightSpec) -> PropDef {
    d.default_state = "lit".into();
    d.states = vec![state("out", out, d.solid)];
    d.light = Some(light);
    d.triggers.push(trig(
        TriggerOn::UseItem { item: "coal".into() },
        vec![],
        vec![Action::TakeItem { item: "coal".into(), count: 1 }, Action::SetState { target: "self".into(), state: "lit".into() }],
    ));
    d
}

/// Picked by hand (one hit) for `loot`, back after `hours`.
fn gathered(mut d: PropDef, picked: &str, loot: Vec<LootEntry>, hours: f32) -> PropDef {
    d.states.push(state("picked", picked, false));
    d.gather = Some(Gather { tool: String::new(), hits: 1, loot, then_state: "picked".into() });
    d.regrow = Some(Regrow { from: "picked".into(), to: "default".into(), hours });
    d
}

/// A seat: `facing` the way the sitter looks, `cover` drawn over them.
fn seat(mut d: PropDef, facing: Facing, offset: [f32; 2], cover: &str) -> PropDef {
    d.use_spot = Some(UseSpot {
        pose: Pose::Sit,
        offset,
        facing,
        cover: if cover.is_empty() { SpriteRef::default() } else { village::sprite(cover) },
        ..Default::default()
    });
    d
}

pub fn props() -> Vec<PropDef> {
    let warm = |r: f32| LightSpec { intensity: 0.9, ..LightSpec::new(r, [255, 170, 90], true) };
    let candle = LightSpec { intensity: 0.7, ..LightSpec::new(2.5, [255, 214, 150], true) };
    vec![
        // Market stalls (3x3): the seller stands in the middle row behind the counter;
        // customers talk to them across it.
        prop("stall_produce", "Produce Stall", "stall_produce", true, [92.0, 24.0]),
        prop("stall_fish", "Fish Stall", "stall_fish", true, [92.0, 24.0]),
        prop("stall_bread", "Bread Stall", "stall_bread", true, [92.0, 24.0]),
        // Big outdoor props.
        prop("windmill", "Windmill", "windmill", true, [72.0, 20.0]),
        prop("watchtower", "Watchtower", "watchtower", true, [60.0, 14.0]),
        PropDef { solid: true, ..gathered(prop("apple_tree", "Apple Tree", "apple_tree", true, [16.0, 10.0]), "apple_tree_picked", vec![LootEntry::new("apple", 100.0, 2, 3)], 24.0) },
        PropDef { triggers: interact(vec![Action::PlaySound { sound: "bell".into() }]), ..prop("bell_tower", "Bell Tower", "bell_tower", true, [60.0, 12.0]) },
        // Fires and stations (lit at first; coal lights them again).
        fire(PropDef { triggers: station("cooking", "Bread Oven"), ..prop("oven", "Bread Oven", "oven", true, [56.0, 16.0]) }, "oven_out", warm(3.5)),
        fire(PropDef { triggers: station("cooking", "Kitchen Hearth"), ..prop("hearth", "Kitchen Hearth", "hearth", true, [60.0, 16.0]) }, "hearth_out", warm(4.0)),
        fire(prop("fireplace", "Fireplace", "fireplace", true, [60.0, 16.0]), "fireplace_out", warm(4.0)),
        fire(PropDef { triggers: station("smelter", "Forge"), ..prop("forge", "Forge", "forge", true, [60.0, 18.0]) }, "forge_out", warm(4.5)),
        fire(prop("brazier", "Brazier", "brazier", true, [18.0, 8.0]), "brazier_out", warm(4.5)),
        PropDef { default_state: "lit".into(), states: vec![state("out", "candle_stand_out", true)], light: Some(candle), ..prop("candle_stand", "Candle Stand", "candle_stand", true, [12.0, 6.0]) },
        PropDef { triggers: station("mill", "Millstone"), ..prop("millstone", "Millstone", "millstone", true, [56.0, 18.0]) },
        // Furniture and furnishings.
        prop("bread_shelf", "Bread Shelf", "bread_shelf", true, [60.0, 14.0]),
        prop("potion_shelf", "Potion Shelf", "potion_shelf", true, [60.0, 14.0]),
        prop("tool_rack", "Tool Rack", "tool_rack", true, [60.0, 10.0]),
        prop("weapon_rack", "Weapon Rack", "weapon_rack", true, [60.0, 10.0]),
        prop("herb_rack", "Herb Rack", "herb_rack", true, [28.0, 8.0]),
        prop("keg_rack", "Keg Rack", "keg_rack", true, [62.0, 14.0]),
        prop("keg", "Keg", "keg", true, [22.0, 10.0]),
        prop("tavern_table", "Tavern Table", "tavern_table", true, [60.0, 14.0]),
        prop("dough_table", "Dough Table", "dough_table", true, [60.0, 14.0]),
        // A stool has no back: one picture, one prop per way the sitter looks.
        seat(prop("stool", "Stool", "stool", true, [16.0, 6.0]), Facing::Down, [0.0, 2.0], ""),
        seat(prop("stool_up", "Stool (facing up)", "stool", true, [16.0, 6.0]), Facing::Up, [0.0, -6.0], ""),
        seat(prop("stool_left", "Stool (facing left)", "stool", true, [16.0, 6.0]), Facing::Left, [0.0, 2.0], ""),
        seat(prop("stool_right", "Stool (facing right)", "stool", true, [16.0, 6.0]), Facing::Right, [0.0, 2.0], ""),
        // Pews face the altar (up); their backrest is drawn over the sitter.
        seat(prop("pew", "Pew", "pew", true, [60.0, 10.0]), Facing::Up, [0.0, -8.0], "pew_back"),
        prop("altar", "Altar", "altar", true, [56.0, 14.0]),
        PropDef { triggers: interact(vec![Action::PlaySound { sound: "bell".into() }]), ..prop("bell_rope", "Bell Rope", "bell_rope", false, [8.0, 6.0]) },
        prop("bellows", "Bellows", "bellows", true, [24.0, 10.0]),
        prop("grindstone", "Grindstone", "grindstone", true, [22.0, 8.0]),
        prop("cauldron", "Cauldron", "cauldron", true, [22.0, 10.0]),
        prop("flour_sacks", "Flour Sacks", "flour_sacks", true, [26.0, 12.0]),
        prop("cat_basket", "Cat Basket", "cat_basket", false, [24.0, 10.0]),
        prop("flower_pot", "Flower Pot", "flower_pot", true, [14.0, 8.0]),
        prop("flower_box", "Flower Box", "flower_box", true, [26.0, 10.0]),
        // Farm and yard.
        prop("chicken_coop", "Chicken Coop", "chicken_coop", true, [52.0, 14.0]),
        prop("trough", "Trough", "trough", true, [58.0, 12.0]),
        prop("hay_bale", "Hay Bale", "hay_bale", true, [26.0, 12.0]),
        gathered(prop("beehive", "Beehive", "beehive", true, [22.0, 8.0]), "beehive_empty", vec![LootEntry::new("honey", 100.0, 1, 1)], 24.0),
        prop("scarecrow", "Scarecrow", "scarecrow", true, [10.0, 6.0]),
        prop("woodpile", "Woodpile", "woodpile", true, [60.0, 14.0]),
        prop("broken_cart", "Broken Cart", "broken_cart", true, [52.0, 18.0]),
        prop("laundry", "Laundry Line", "laundry", false, [60.0, 6.0]),
        prop("notice_board", "Notice Board", "notice_board", true, [52.0, 8.0]),
        // By the water.
        prop("fish_rack", "Fish Rack", "fish_rack", true, [60.0, 8.0]),
        PropDef { container: Some(ContainerSpec { size: 4, ..Default::default() }), ..prop("fish_barrel", "Fish Barrel", "fish_barrel", true, [20.0, 10.0]) },
        prop("boat", "Rowing Boat", "boat", true, [60.0, 18.0]),
        // Signs hanging over shop doors (warm light over the tavern's).
        PropDef { light: Some(LightSpec { intensity: 0.8, ..LightSpec::new(4.0, [255, 190, 110], true) }), ..prop("sign_tavern", "Tavern Sign", "sign_tavern", false, [20.0, 6.0]) },
        prop("sign_bakery", "Bakery Sign", "sign_bakery", false, [20.0, 6.0]),
        prop("sign_smithy", "Smithy Sign", "sign_smithy", false, [20.0, 6.0]),
        prop("sign_apothecary", "Apothecary Sign", "sign_apothecary", false, [20.0, 6.0]),
        // The graveyard.
        prop("grave_cross", "Grave Cross", "grave_cross", true, [10.0, 6.0]),
        // Herbs: picked by hand. The moonpetal opens only at night (8 pm to 5 am).
        PropDef {
            light: Some(LightSpec { intensity: 0.5, ..LightSpec::new(1.5, [150, 190, 255], false) }),
            states: vec![state("picked", "moonpetal_picked", false)],
            regrow: Some(Regrow { from: "picked".into(), to: "default".into(), hours: 24.0 }),
            triggers: interact(vec![Action::If {
                conditions: vec![Condition::new(Cond::TimeBetween { from: 20.0, to: 5.0 })],
                then: vec![
                    Action::GiveItem { item: "moonpetal".into(), count: 1 },
                    Action::SetState { target: "self".into(), state: "picked".into() },
                    dialog("", "The moonpetal has opened in the moonlight. You pick it carefully."),
                ],
                otherwise: vec![dialog("", "The bud is shut tight. Moonpetals only open at night.")],
            }]),
            ..prop("moonpetal", "Moonpetal", "moonpetal", false, [16.0, 6.0])
        },
        gathered(prop("feverleaf_bush", "Feverleaf", "feverleaf_bush", false, [20.0, 8.0]), "feverleaf_picked", vec![LootEntry::new("feverleaf", 100.0, 1, 2)], 24.0),
        gathered(prop("bitterroot", "Bitterroot", "bitterroot", false, [20.0, 8.0]), "bitterroot_picked", vec![LootEntry::new("bitterroot", 100.0, 1, 1)], 24.0),
    ]
}

fn item(id: &str, name: &str, desc: &str, frame: u32, consumable: bool, on_use: Vec<Action>) -> ItemDef {
    ItemDef {
        id: id.into(),
        name: name.into(),
        description: desc.into(),
        icon: SpriteRef::new("items2", frame),
        consumable,
        on_use: if on_use.is_empty() { vec![] } else { vec![trig(TriggerOn::UseItem { item: String::new() }, vec![], on_use)] },
        weapon: None,
        armor: None,
        max_stack: 20,
        value: 0,
        light: None,
        teaches: String::new(),
        held: String::new(),
        tool: String::new(),
        use_on: Default::default(),
        crop: None,
        food: false,
    }
}

pub fn items() -> Vec<ItemDef> {
    let heal = |n: i32| vec![Action::Heal { amount: n }];
    let mut v = vec![
        item("flour", "Sack of Flour", "Ground wheat from the mill. No baker can do without it.", it2::FLOUR, false, vec![]),
        item("egg", "Egg", "A fresh egg from the coop.", it2::EGG, false, vec![]),
        dish(item("cheese", "Cheese", "A wedge of hard cheese. Restores 1 heart.", it2::CHEESE, true, heal(2))),
        dish(item("milk", "Jug of Milk", "Fresh from the cow. Restores half a heart.", it2::MILK, true, heal(1))),
        item("honey", "Jar of Honey", "Golden honey from the village hives.", it2::HONEY, false, vec![]),
        item("moonpetal", "Moonpetal", "A pale flower that opens only under the moon.", it2::MOONPETAL, false, vec![]),
        item("feverleaf", "Feverleaf", "A bitter, red-tipped leaf that cools a fever.", it2::FEVERLEAF, false, vec![]),
        item("bitterroot", "Bitterroot", "A knobbly purple root from the pond bank. It tastes as its name says.", it2::BITTERROOT, false, vec![]),
        item(
            "remedy",
            "Herbal Remedy",
            "Moonpetal, feverleaf and bitterroot, brewed together: cures poison and heals 2 hearts.",
            it2::REMEDY,
            true,
            vec![Action::CureEffect { target: "player".into(), kind: Some(EffectKind::Poison) }, Action::Heal { amount: 4 }],
        ),
        ItemDef {
            max_stack: 1,
            ..item("locket", "Silver Locket", "A tarnished silver locket.", it2::LOCKET, false, vec![dialog("Silver Locket", "Inside the locket: a tiny painted portrait of a smiling woman, and the letters H. and E.")])
        },
        ItemDef { max_stack: 10, ..dish(item("festival_bread", "Festival Loaf", "A braided loaf with a honey glaze, baked for the harvest festival. Restores 2 hearts and heals on for a while.", it2::FESTIVAL_BREAD, true, vec![Action::Heal { amount: 4 }, regen(12.0)])) },
        ItemDef { max_stack: 10, ..dish(item("ale", "Mug of Ale", "Frothy and brown. Restores half a heart.", it2::ALE, true, heal(1))) },
        ItemDef { max_stack: 1, tool: "broom".into(), held: "held_broom".into(), ..item("broom", "Broom", "Sweeps floors and yards.", it2::BROOM, false, vec![]) },
        ItemDef { max_stack: 1, tool: "hoe".into(), held: "held_hoe".into(), ..item("hoe", "Hoe", "A farmer's hoe for tilling the soil.", it2::HOE, false, vec![]) },
    ];
    for (id, value) in [("flour", 4), ("egg", 2), ("cheese", 6), ("milk", 3), ("honey", 6), ("moonpetal", 15), ("feverleaf", 5), ("bitterroot", 5), ("remedy", 30), ("locket", 30), ("festival_bread", 20), ("ale", 3), ("broom", 5), ("hoe", 12)] {
        v.iter_mut().find(|d| d.id == id).expect(id).value = value;
    }
    v
}

pub fn recipes() -> Vec<RecipeDef> {
    let i = ItemStack::new;
    vec![
        // The millstone grinds wheat into flour.
        RecipeDef::new("grind_flour", "flour", 1, vec![i("wheat", 2)], "mill"),
        // The festival loaf needs a lit oven (or another lit cooking fire).
        RecipeDef { station_state: "lit".into(), ..RecipeDef::new("bake_festival_bread", "festival_bread", 1, vec![i("flour", 2), i("egg", 1), i("honey", 1)], "cooking") },
        RecipeDef { tools: vec!["mortar".into()], ..RecipeDef::new("brew_remedy", "remedy", 1, vec![i("moonpetal", 1), i("feverleaf", 1), i("bitterroot", 1)], "alchemy") },
    ]
}

pub fn sheets() -> Vec<SheetDef> {
    let mut v = vec![SheetDef::new(village::SHEET, "sprites/village.png", 32, 32), SheetDef::new("props2", "sprites/props2.png", 32, 32), SheetDef::new("items2", "sprites/items2.png", 32, 32)];
    for a in &animals::ANIMALS {
        v.push(SheetDef { kind: SheetKind::Body, ..SheetDef::new(a.id, &format!("sprites/{}.png", a.id), a.size, a.size) });
    }
    for k in &animals::CHILDREN {
        v.push(SheetDef { kind: SheetKind::Body, female: k.id == "child_girl", ..SheetDef::new(k.id, &format!("sprites/{}.png", k.id), 32, 48) });
    }
    v
}

fn ch(id: &str, name: &str, speed: f32, behavior: Behavior) -> CharacterDef {
    CharacterDef {
        id: id.into(),
        name: name.into(),
        sheet: String::new(),
        speed: speed * R,
        hitbox: [10.0 * R, 6.0 * R],
        behavior,
        max_hp: 0,
        damage: 0,
        hostile: false,
        weapon: None,
        defense: 0,
        xp: 0,
        touch_effect: None,
        description: String::new(),
        triggers: vec![],
        shop: None,
        loot: vec![],
        light: None,
        appearance: Appearance::default(),
        equipment: Default::default(),
        inventory: Vec::new(),
        gear_locked: false,
        portrait: Default::default(),
        moods: Default::default(),
    }
}

fn shop(items: &[(&str, u32)], sell_rate: u32) -> Option<ShopSpec> {
    Some(ShopSpec {
        items: items.iter().map(|&(id, n)| if n == 0 { ShopEntry { unlimited: true, ..ShopEntry::new(id, 0, 0) } } else { ShopEntry::new(id, n, 0) }).collect(),
        sell_rate,
        ..Default::default()
    })
}

/// The villagers, children and animals. People get their layered look in `layered_characters`
/// (main.rs); children and animals are drawn whole (their own body sheets).
pub fn characters() -> Vec<CharacterDef> {
    let face = |f: u32| SpriteRef::new("portraits", f);
    let still = Behavior::Static;
    let mut v = vec![
        CharacterDef { portrait: face(portrait::BAKER), shop: shop(&[("bread", 0), ("apple_pie", 3), ("flour", 6)], 50), ..ch("baker", "Baker", 35.0, still.clone()) },
        CharacterDef {
            portrait: face(portrait::INNKEEPER),
            shop: shop(&[("ale", 0), ("bread", 4), ("cheese", 4), ("pumpkin_stew", 3), ("grilled_trout", 3), ("apple_pie", 2)], 50),
            ..ch("innkeeper", "Innkeeper", 35.0, still.clone())
        },
        CharacterDef {
            portrait: face(portrait::HERBALIST),
            shop: shop(&[("herb", 5), ("mushroom", 5), ("potion", 3), ("antidote", 3), ("bandage", 5), ("mortar", 1)], 50),
            ..ch("herbalist", "Herbalist", 35.0, still.clone())
        },
        CharacterDef { portrait: face(portrait::PRIEST), ..ch("priest", "Priest", 30.0, still.clone()) },
        CharacterDef { portrait: face(portrait::MILLER), shop: shop(&[("flour", 8), ("wheat_seeds", 6), ("bread", 3)], 50), ..ch("miller", "Miller", 35.0, still.clone()) },
        CharacterDef { portrait: face(portrait::FISHER), shop: shop(&[("trout", 4), ("grilled_trout", 2), ("fishing_rod", 1)], 50), ..ch("fisher", "Fisher", 30.0, still.clone()) },
        ch("town_crier", "Town Crier", 35.0, still.clone()),
        CharacterDef {
            portrait: face(portrait::BLACKSMITH),
            shop: shop(&[("pickaxe", 2), ("hammer", 1), ("tongs", 1), ("iron_sword", 1), ("iron_helmet", 1), ("arrows", 30), ("bolts", 20)], 60),
            ..ch("blacksmith", "Blacksmith", 35.0, still.clone())
        },
        ch("farmer", "Farmer", 35.0, Behavior::Wander { radius: 3.0 }),
        CharacterDef { shop: shop(&[("apple", 0), ("carrot", 6), ("pumpkin", 2), ("egg", 6), ("milk", 4), ("cheese", 3), ("wheat_seeds", 6), ("carrot_seeds", 4)], 50), ..ch("farmer_f", "Farmer (female)", 35.0, Behavior::Wander { radius: 3.0 }) },
        ch("villager_f", "Villager (female)", 35.0, Behavior::Wander { radius: 3.0 }),
        CharacterDef { max_hp: 8, ..ch("guard_f", "Town Guard (female)", 40.0, still.clone()) },
    ];
    for k in &animals::CHILDREN {
        let name = if k.id == "child_girl" { "Girl" } else { "Boy" };
        v.push(CharacterDef { appearance: Appearance { body: k.id.into(), ..Default::default() }, hitbox: [8.0 * R, 5.0 * R], ..ch(k.id, name, 48.0, Behavior::Wander { radius: 2.0 }) });
    }
    // Animals: a sound and a line when talked to.
    for (id, name, speed, wander, hitbox, sound, line) in [
        ("chicken", "Chicken", 30.0, 3.0, [7.0, 4.0], "cluck", "Bok bok... bok?"),
        ("dog", "Dog", 60.0, 4.0, [10.0, 5.0], "bark", "Woof! The dog wags its tail."),
        ("cat", "Cat", 35.0, 1.0, [8.0, 4.0], "meow", "Mrrow. The cat ignores you magnificently."),
        ("cat_ginger", "Ginger Cat", 35.0, 2.0, [8.0, 4.0], "meow", "Prrrp! The cat rubs against your legs."),
        ("sheep", "Sheep", 25.0, 3.0, [11.0, 6.0], "baa", "Baaa."),
        ("cow", "Cow", 20.0, 2.0, [18.0, 7.0], "moo", "Moooo. The cow chews on, unimpressed."),
    ] {
        v.push(CharacterDef {
            appearance: Appearance { body: id.into(), ..Default::default() },
            hitbox: [hitbox[0] * R, hitbox[1] * R],
            triggers: interact(vec![Action::PlaySound { sound: sound.into() }, dialog(name, line)]),
            ..ch(id, name, speed, Behavior::Wander { radius: wander })
        });
    }
    v
}
