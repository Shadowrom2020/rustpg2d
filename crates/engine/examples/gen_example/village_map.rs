//! Emberbrook (0.11.0): the village map (80 x 60) and the interiors of its key buildings, with
//! its villagers' daily routines.
//!
//! The river runs down the west (x 17-19) with two bridges; the main street crosses the map at
//! y 29-31 from the west entrance to the east gate in the town wall (x 76), the lower lane at
//! y 46-47. North of the street: the cobbled market square (x 26-42, y 13-28) with the tavern,
//! the bakery and the smithy; east of it the elder's house, Tilda's shop, the crier's home, the
//! barracks and the orchard. South: the apothecary with its herb beds, the chapel with its bell
//! tower, the graveyard, Mira's home, the fisher's hut, the dock and the pond. West of the river:
//! the windmill and mill house, the fields, the farmhouse, the chicken coop and the animal pen.

use super::*;
use crate::structures::{Roof, Wall};
use rustpg2d_engine::map::{CollisionOverride, Weather, WeatherKind};

/// Tile `x` of the river's middle at row `y` (straight where the bridges cross).
fn river_x(y: i32) -> i32 {
    if (26..=34).contains(&y) || (43..=50).contains(&y) {
        18
    } else {
        18 + (1.5 * (y as f32 / 9.0).sin()).round() as i32
    }
}

/// The pond's sand ring (true) and water (false) around (67, 50).
fn pond(x: i32, y: i32) -> Option<bool> {
    let (fx, fy) = (x as f32 + 0.5 - 67.0, y as f32 + 0.5 - 50.0);
    if (fx / 5.5).powi(2) + (fy / 4.0).powi(2) <= 1.0 {
        Some(false)
    } else if (fx / 7.5).powi(2) + (fy / 6.0).powi(2) <= 1.0 {
        Some(true)
    } else {
        None
    }
}

/// Paints a building with its top-left at (`x`, `y`); returns its door.
fn building(b: &mut B, x: i32, y: i32, w: i32, walls: i32, roof: Roof, wall: Wall) -> (i32, i32) {
    structures::building_def(w, walls, roof, wall).place(&mut b.m, x, y);
    (x + w / 2, y + 2 + walls)
}

/// A door leading into `map`: the portal on the door, the spawn `<map>_door` in front of it.
fn door_to(b: &mut B, (x, y): (i32, i32), map: &str) {
    b.spawn(&format!("{map}_door"), x as u32, y as u32 + 1);
    b.portal(x as u32, y as u32, map, "entrance");
}

/// An Interact trigger with conditions saying one line.
fn line(conditions: Vec<Condition>, speaker: &str, text: &str) -> Trigger {
    trig(TriggerOn::Interact, conditions, vec![dialog(speaker, text)])
}

fn between(from: f32, to: f32) -> Condition {
    cond(Cond::TimeBetween { from, to })
}

fn posed(pose: &str) -> Condition {
    cond(Cond::Pose { target: "self".into(), pose: pose.into() })
}

fn not_posed(pose: &str) -> Condition {
    not(Cond::Pose { target: "self".into(), pose: pose.into() })
}

fn stop(hour: f32, x: i32, y: i32) -> ScheduleStop {
    ScheduleStop::new(hour, x, y)
}

/// A stop on another map, using a prop there (a bed, a stool) or just standing at (x, y).
fn visit(hour: f32, map: &str, x: i32, y: i32, prop: &str) -> ScheduleStop {
    ScheduleStop { map: map.into(), prop: prop.into(), ..ScheduleStop::new(hour, x, y) }
}

fn working(mut s: ScheduleStop, kind: ActivityKind, tool: &str, facing: Facing) -> ScheduleStop {
    s.activity = Some(Activity { kind, tool: tool.into() });
    s.facing = Some(facing);
    s
}

fn facing(mut s: ScheduleStop, f: Facing) -> ScheduleStop {
    s.facing = Some(f);
    s
}

fn inside(mut s: ScheduleStop) -> ScheduleStop {
    s.inside = true;
    s
}

pub fn village() -> Map {
    let (w, h) = (80i32, 60i32);
    let mut b = B::new("village", w as u32, h as u32);
    let mut rng = Rng(42);
    b.fill(0, 0, 0, w, h, t::GRASS);
    for _ in 0..520 {
        let (x, y) = (rng.range(w as u32) as i32, rng.range(h as u32) as i32);
        b.set(0, x, y, if rng.chance(0.5) { t::FLOWERS } else { t::TALL_GRASS });
    }

    // --- Ground: streets, the square, yards, fields, the orchard, the pond.
    b.fill(0, 0, 29, w, 3, t::PATH); // the main street, west entrance to the east gate
    b.fill(0, 2, 46, 59, 2, t::PATH); // the lower lane
    b.fill(0, 32, 32, 2, 14, t::PATH); // the chapel lane between the two
    b.fill(0, 26, 13, 17, 16, t::COBBLE); // the market square
    b.fill(0, 24, 29, 21, 3, t::COBBLE); // the street through the square
    b.fill(0, 44, 13, 11, 6, t::STONE_FLOOR); // the smithy's yard
    b.fill(0, 4, 15, 11, 2, t::PATH); // the mill yard
    b.fill(0, 12, 17, 1, 12, t::PATH); // from the mill down to the street
    b.fill(0, 2, 33, 13, 6, t::TILLED); // the fields
    b.fill(0, 56, 10, 18, 10, t::FLOWER_BED); // the orchard
    b.fill(0, 4, 50, 7, 5, t::STRAW_FLOOR); // the pen
    for y in 40..58 {
        for x in 58..78 {
            match pond(x, y) {
                Some(true) => b.set(0, x, y, t::SAND),
                Some(false) => b.set(0, x, y, t::WATER),
                None => {}
            }
        }
    }
    // The river, with bridges where the street and the lane cross it.
    for y in 0..h {
        let c = river_x(y);
        for x in c - 1..=c + 1 {
            let bridge = (29..=31).contains(&y) || (46..=47).contains(&y);
            b.set(0, x, y, if bridge { t::BRIDGE } else { t::WATER });
            if y == 28 || y == 45 {
                b.set(1, x, y, t::BRIDGE_RAIL_N);
            }
            if y == 32 || y == 48 {
                b.set(1, x, y, t::BRIDGE_RAIL_S);
            }
        }
    }

    // --- Buildings (doors in their bottom row) and the paths to them.
    let tavern = building(&mut b, 24, 8, 11, 2, Roof::Slate, Wall::TimberX);
    let bakery = building(&mut b, 36, 8, 7, 2, Roof::Red, Wall::Timber);
    let smithy = building(&mut b, 46, 8, 7, 2, Roof::Slate, Wall::Stone);
    let house = building(&mut b, 46, 20, 7, 2, Roof::Red, Wall::Timber);
    let shop = building(&mut b, 56, 22, 6, 2, Roof::Red, Wall::Timber);
    let crier_home = building(&mut b, 63, 22, 5, 2, Roof::Thatch, Wall::Timber);
    let barracks = building(&mut b, 69, 22, 5, 2, Roof::Slate, Wall::Stone);
    let apothecary = building(&mut b, 21, 36, 7, 2, Roof::Red, Wall::Timber);
    structures::chapel_def().place(&mut b.m, 34, 35);
    let chapel = (38, 41);
    let home_mira = building(&mut b, 54, 36, 7, 2, Roof::Red, Wall::Timber);
    let fisher_hut = building(&mut b, 66, 33, 5, 2, Roof::Thatch, Wall::Timber);
    let mill = building(&mut b, 10, 10, 5, 2, Roof::Thatch, Wall::Stone);
    let farmhouse = building(&mut b, 3, 40, 7, 2, Roof::Thatch, Wall::Timber);
    for (x, y0, y1) in [(house.0, house.1 + 1, 28), (shop.0, shop.1 + 1, 28), (crier_home.0, crier_home.1 + 1, 28), (barracks.0, barracks.1 + 1, 28)] {
        b.fill(0, x, y0, 1, y1 - y0 + 1, t::PATH);
    }
    for (x, y0) in [(apothecary.0, apothecary.1 + 1), (chapel.0, chapel.1 + 1), (home_mira.0, home_mira.1 + 1), (farmhouse.0, farmhouse.1 + 1)] {
        b.fill(0, x, y0, 1, 46 - y0, t::PATH);
    }
    b.fill(0, fisher_hut.0, fisher_hut.1 + 1, 1, 7, t::PATH);
    // The dock out into the pond.
    b.fill(0, 68, 45, 1, 5, t::DOCK);
    b.set(0, 68, 50, t::DOCK_EDGE);

    // --- The town wall in the east with the gate, towers and braziers.
    for y in 2..h - 2 {
        if !(27..=32).contains(&y) {
            b.set(1, 76, y, t::STONE_WALL);
        }
    }
    b.set(1, 76, 27, t::GATE_PILLAR);
    b.set(1, 76, 32, t::GATE_PILLAR);

    // --- Trees: the border (open at the west entrance, the river and the east road), and a
    // few on the greens.
    let open = |x: i32, y: i32| (x - river_x(y)).abs() <= 1;
    for x in 0..w {
        if !open(x, 1) {
            b.tree(x, 1);
        }
        if !open(x, h - 1) {
            b.tree(x, h - 1);
        }
    }
    for y in 2..h - 1 {
        if !(27..=33).contains(&y) {
            b.tree(0, y);
            b.tree(w - 1, y);
        }
    }
    for y in 3..h - 2 {
        if !(27..=33).contains(&y) && y % 3 != 0 {
            b.tree(78, y);
        }
    }
    for (x, y) in [
        (3, 4), (8, 5), (15, 4), (2, 21), (5, 24), (9, 22), (14, 25), (23, 4), (23, 18), (22, 24), (44, 4), (52, 4), (60, 5), (66, 4), (72, 6),
        (28, 53), (36, 53), (44, 54), (52, 55), (58, 56), (24, 50), (40, 50), (14, 52), (74, 20), (62, 33),
    ] {
        if b.free(x, y) && b.free(x, y - 1) {
            b.tree(x, y);
        }
    }
    for (x, y) in [(29, 5), (33, 4), (2, 13), (50, 51)] {
        b.set(1, x, y, t::BUSH);
    }
    b.set(1, 20, 53, t::ROCK);
    b.set(1, 75, 44, t::ROCK);
    // Herb beds by the apothecary, framed by low hedges.
    b.set(1, 21, 33, t::HEDGE);
    b.set(1, 21, 34, t::HEDGE);
    b.set(1, 27, 33, t::HEDGE);
    b.set(1, 27, 34, t::HEDGE);

    // --- Entities. (Uids follow this order: new entities of later versions go at the end.)
    b.spawn("start", 2, 30);
    door_to(&mut b, house, "house");
    door_to(&mut b, shop, "shop");
    door_to(&mut b, tavern, "tavern");
    door_to(&mut b, bakery, "bakery");
    door_to(&mut b, smithy, "smithy");
    door_to(&mut b, apothecary, "apothecary");
    door_to(&mut b, chapel, "chapel");
    door_to(&mut b, home_mira, "home_mira");
    door_to(&mut b, mill, "mill");

    // The east gate (Gate Key; the lock comes from the definition, these triggers react to it).
    let gate = b.big("vgate", 76, 31, 1, 4);
    gate.name = "east_gate".into();
    gate.triggers = vec![
        trig(
            TriggerOn::Unlock,
            vec![],
            vec![Action::SetFlag { name: "gate_open".into(), value: true }, dialog("", "The gate creaks open. The forest lies beyond.")],
        ),
        // Quest "The Strange Lights": the gate is open, now find the shrine.
        trig(TriggerOn::Unlock, vec![quest_is("strange_lights", QuestCheck::AtStage, 0)], vec![quest("strange_lights", QuestOp::SetStage, 1)]),
        trig(TriggerOn::Lock, vec![], vec![Action::SetFlag { name: "gate_open".into(), value: false }]),
    ];
    b.big("watchtower", 74, 27, 2, 4);
    b.big("watchtower", 74, 37, 2, 4);
    b.entity(EntityKind::Prop, "brazier", 75, 28);
    b.entity(EntityKind::Prop, "brazier", 75, 32);

    // The square: the well, the statue, the notice board, three market stalls, benches, lamps.
    b.entity(EntityKind::Prop, "well", 34, 21);
    b.big("statue", 34, 16, 1, 2);
    let board = b.big("notice_board", 31, 16, 2, 2);
    board.triggers = vec![trig(
        TriggerOn::Interact,
        vec![],
        vec![dialog(
            "Notice Board",
            "MARKET every day from nine. - The Copper Kettle: food, drink and rooms. - The east gate stays locked after dark. - Lost: one ginger cat. Ask Mira.",
        )],
    )];
    b.big("stall_produce", 29, 21, 3, 3);
    b.big("stall_fish", 39, 21, 3, 3);
    b.big("stall_bread", 39, 27, 3, 3);
    b.big("bench", 31, 26, 2, 1).name = "bench_west".into();
    b.big("bench", 37, 26, 2, 1).name = "bench_east".into();
    for (x, y) in [(25, 14), (43, 14), (25, 28), (43, 28), (10, 28), (22, 28), (50, 28), (66, 28), (36, 45), (10, 45), (28, 45), (61, 45)] {
        b.big("street_lamp", x, y, 1, 2);
    }
    let sign = b.entity(EntityKind::Prop, "sign", 3, 28);
    sign.triggers = vec![trig(TriggerOn::Interact, vec![], vec![dialog("Sign", "Emberbrook. East: Whisperwood. Please keep the gate locked!")])];
    let shop_sign = b.entity(EntityKind::Prop, "sign", shop.0 - 1, shop.1 + 1);
    shop_sign.triggers = vec![trig(TriggerOn::Interact, vec![], vec![dialog("Sign", "TILDA'S WARES - Potions, arrows, armor. Fair prices, we buy too!")])];
    b.entity(EntityKind::Prop, "sign_tavern", tavern.0 - 1, tavern.1);
    b.entity(EntityKind::Prop, "sign_bakery", bakery.0 - 1, bakery.1);
    b.entity(EntityKind::Prop, "sign_smithy", smithy.0 - 1, smithy.1);
    b.entity(EntityKind::Prop, "sign_apothecary", apothecary.0 - 1, apothecary.1);
    let barrel = b.entity(EntityKind::Prop, "barrel", 35, 12);
    barrel.container = Some(ContainerSpec { size: 4, items: vec![ItemStack::new("apple", 3), ItemStack::new("arrows", 10), ItemStack::new("leather_cap", 1)], ..Default::default() });
    let barrel = b.entity(EntityKind::Prop, "barrel", 44, 12);
    barrel.container = Some(ContainerSpec { size: 4, items: vec![ItemStack::new("apple", 2), ItemStack::new("bronze_key", 1)], ..Default::default() });
    for (def, x, y) in [("keg", 33, 13), ("flower_box", 27, 13), ("flour_sacks", 41, 13), ("flower_box", 37, 13), ("crate", 26, 21), ("sack", 42, 24), ("flower_pot", 45, 24), ("flower_pot", 53, 24)] {
        b.entity(EntityKind::Prop, def, x, y);
    }
    // The smithy's yard.
    b.big("woodpile", 54, 14, 2, 1);
    b.entity(EntityKind::Prop, "grindstone", 45, 14);
    b.entity(EntityKind::Prop, "crate", 53, 17);
    // The orchard: apple trees (pick apples, back a day later) and beehives (honey).
    for (x, y) in [(59, 14), (64, 14), (69, 14), (61, 19), (66, 19), (71, 19)] {
        b.big("apple_tree", x, y, 2, 3);
    }
    b.entity(EntityKind::Prop, "beehive", 57, 11);
    b.entity(EntityKind::Prop, "beehive", 73, 11);
    // Herb beds by the apothecary.
    for (def, x, y) in [("herb_bush", 22, 33), ("mushroom_patch", 23, 33), ("herb_bush", 24, 33), ("mushroom_patch", 25, 33), ("herb_bush", 26, 33)] {
        b.entity(EntityKind::Prop, def, x, y);
    }
    // The chapel's bell tower and the graveyard.
    let bell = b.big("bell_tower", 44, 41, 2, 3);
    bell.name = "village_bell".into();
    for y in 35..=41 {
        b.set(1, 46, y, t::FENCE_V);
        b.set(1, 52, y, t::FENCE_V);
    }
    for x in 46..=52 {
        b.set(1, x, 35, t::FENCE);
        if x != 49 {
            b.set(1, x, 41, t::FENCE);
        }
    }
    for (x, y) in [(46, 35), (52, 35), (46, 41), (52, 41)] {
        b.set(1, x, y, t::FENCE_POST);
    }
    for (x, y) in [(47, 37), (49, 37), (51, 38), (48, 39), (50, 40)] {
        b.set(0, x, y, t::GRAVE_MOUND);
    }
    for (def, x, y) in [("tombstone", 47, 36), ("grave_cross", 49, 36), ("tombstone", 51, 37), ("grave_cross", 48, 38), ("tombstone", 50, 39)] {
        b.entity(EntityKind::Prop, def, x, y);
    }
    b.entity(EntityKind::Prop, "moonpetal", 51, 40);
    b.entity(EntityKind::Prop, "moonpetal", 47, 40);
    b.fill(0, 49, 42, 1, 4, t::PATH);
    // Mira's home: laundry in the yard.
    b.big("laundry", 62, 42, 2, 2);
    // The fisher's hut, the dock, the pond (a fishing spot) with a boat.
    b.big("fish_rack", 65, 40, 2, 2);
    b.entity(EntityKind::Prop, "fish_barrel", 71, 39);
    b.big("boat", 71, 48, 2, 1);
    let spot = b.zone("pond_fishing", 62, 46, 11, 8, ZoneKind::FishingSpot { rod: "fishing_rod".into() });
    if let Some(z) = b.m.zones.iter_mut().find(|z| z.uid == spot) {
        z.loot = Some(vec![LootEntry::new("trout", 75.0, 1, 1), LootEntry::new("old_boot", 25.0, 1, 1)]);
    }
    b.entity(EntityKind::Prop, "bitterroot", 61, 50);
    b.entity(EntityKind::Prop, "bitterroot", 72, 54);
    // West of the river: the windmill and the mill yard, the fields, the farm and the pen.
    b.big("windmill", 6, 14, 4, 5);
    b.entity(EntityKind::Prop, "flour_sacks", 9, 15);
    b.entity(EntityKind::Prop, "hay_bale", 4, 16);
    for y in [34, 37] {
        for x in [3, 5, 7, 9, 11, 13] {
            b.entity(EntityKind::Prop, "soil_plot", x, y);
        }
    }
    b.big("scarecrow", 15, 37, 1, 2);
    let seeds = b.entity(EntityKind::Prop, "barrel", 15, 33);
    seeds.container = Some(ContainerSpec { size: 4, items: vec![ItemStack::new("wheat_seeds", 4), ItemStack::new("carrot_seeds", 4), ItemStack::new("pumpkin_seeds", 2)], ..Default::default() });
    b.big("chicken_coop", 12, 44, 2, 2);
    b.entity(EntityKind::Prop, "hay_bale", 13, 41);
    for y in 49..=55 {
        b.set(1, 3, y, t::FENCE_V);
        b.set(1, 11, y, t::FENCE_V);
    }
    for x in 3..=11 {
        b.set(1, x, 55, t::FENCE);
        if !(7..=8).contains(&x) {
            b.set(1, x, 49, t::FENCE);
        }
    }
    for (x, y) in [(3, 49), (11, 49), (3, 55), (11, 55)] {
        b.set(1, x, y, t::FENCE_POST);
    }
    b.big("double_gate", 8, 49, 2, 2);
    b.big("trough", 6, 52, 2, 1);
    b.entity(EntityKind::Prop, "haystack", 10, 53);
    // A big oak and a pine on the greens.
    b.big("big_oak", 30, 52, 3, 4);
    b.big("big_pine", 8, 9, 2, 4);
    b.entity(EntityKind::Item, "apple", 8, 24);
    b.entity(EntityKind::Item, "apple", 45, 33);

    // --- People. Elder Rowan: reads by the well, rests on the bench, has a drink at the
    // tavern in the evening, sleeps at home. His story is the Strange Lights.
    let elder = b.entity(EntityKind::Character, "elder", 34, 22);
    elder.name = "elder".into();
    elder.schedule = vec![
        working(stop(7.0, 34, 22), ActivityKind::Read, "", Facing::Down),
        ScheduleStop { prop: "bench_west".into(), ..stop(13.0, 31, 27) },
        visit(18.0, "tavern", 3, 6, "stool_1"),
        visit(21.0, "house", 2, 4, "elder_bed"),
    ];
    elder.triggers = vec![
        line(vec![posed("lie")], "Elder Rowan", "Zzz... the lights... the shrine..."),
        trig(
            TriggerOn::Interact,
            vec![not(flag("quest_accepted"))],
            vec![
                dialog("Elder Rowan", "Ah, a traveller! Welcome to Emberbrook, {player}."),
                Action::Mood { target: "self".into(), mood: "worried".into() },
                dialog("Elder Rowan", "Strange lights have been seen in Whisperwood, east of here."),
                Action::Choice {
                    speaker: "Elder Rowan".into(),
                    text: "Would you investigate the old shrine for us?".into(),
                    options: vec!["Of course!".into(), "Not right now.".into()],
                    var: "elder_answer".into(),
                },
                Action::If {
                    conditions: vec![cond(Cond::Var { name: "elder_answer".into(), op: CmpOp::Eq, value: 0 })],
                    then: vec![
                        Action::SetFlag { name: "quest_accepted".into(), value: true },
                        quest("strange_lights", QuestOp::Start, 0),
                        Action::Mood { target: "self".into(), mood: String::new() },
                        dialog("Elder Rowan", "Wonderful! The gate key is in the chest in my house, east of the square."),
                    ],
                    otherwise: vec![dialog("Elder Rowan", "I understand. Come back if you change your mind.")],
                },
            ],
        ),
        trig(
            TriggerOn::Interact,
            vec![cond(flag("quest_accepted")), not(flag("shrine_lit"))],
            vec![dialog("Elder Rowan", "The shrine lies deep in Whisperwood. Take the key from my chest and open the east gate.")],
        ),
        trig(
            TriggerOn::Interact,
            vec![cond(flag("shrine_lit"))],
            vec![
                dialog("Elder Rowan", "The lights have calmed! You have my thanks, brave {lad|lass}."),
                Action::If {
                    conditions: vec![not(flag("rewarded"))],
                    then: vec![
                        Action::SetFlag { name: "rewarded".into(), value: true },
                        quest("strange_lights", QuestOp::Complete, 0),
                        Action::GiveItem { item: "coin".into(), count: 25 },
                        dialog("", "Received 25 coins!"),
                        Action::GiveSkillPoints { amount: 2 },
                    ],
                    otherwise: vec![],
                },
            ],
        ),
    ];
    // Mira: plays by the well, at the pond in the afternoon, home for supper and bed.
    let mira = b.entity(EntityKind::Character, "child_girl", 36, 23);
    mira.name = "mira".into();
    mira.portrait = Some(SpriteRef::new("portraits", art::portrait::MIRA));
    mira.schedule = vec![
        stop(8.0, 36, 23),
        visit(12.0, "home_mira", 4, 5, ""),
        stop(14.5, 64, 45),
        visit(19.0, "home_mira", 4, 5, ""),
        visit(20.5, "home_mira", 2, 6, "mira_bed"),
    ];
    mira.triggers = vec![
        line(vec![posed("lie")], "Mira", "Mmh... Biscuit...? Zzz..."),
        trig(TriggerOn::Interact, vec![not_posed("lie")], vec![Action::RunScript { function: "mira_talk".into() }]),
    ];
    // Tomas: helps at the produce stall, watches the river from the bridge, rests on the bench,
    // fishes at the pond, sleeps in the elder's guest bed.
    let tomas = b.entity(EntityKind::Character, "villager", 30, 23);
    tomas.name = "tomas".into();
    tomas.schedule = vec![
        facing(stop(7.0, 30, 23), Facing::Up),
        facing(stop(12.0, 18, 29), Facing::Up),
        ScheduleStop { prop: "bench_west".into(), ..stop(15.0, 31, 27) },
        working(stop(17.5, 65, 45), ActivityKind::Fish, "fishing_rod", Facing::Down),
        visit(20.5, "house", 6, 8, "guest_bed"),
    ];
    tomas.triggers = vec![
        line(vec![posed("lie")], "Tomas", "Zzz... mmh... five more minutes..."),
        line(vec![posed("sit")], "Tomas", "Ah, {player}. Nothing beats a quiet bench in the afternoon."),
        line(vec![between(6.0, 18.0), not_posed("sit")], "Tomas", "Lovely weather today, isn't it, {sir|miss}?"),
        line(vec![between(18.0, 6.0), not_posed("lie")], "Tomas", "Getting late, {sir|miss}. I'll be heading home soon - the elder keeps a bed for me."),
    ];
    // The guard patrols the main street with a lantern.
    let guard = b.entity(EntityKind::Character, "guard", 8, 30);
    guard.name = "guard".into();
    guard.light = Some(LightSpec::new(4.5, [255, 220, 170], false));
    guard.patrol = Some(rustpg2d_engine::entity::PatrolRoute {
        points: vec![(8, 30), (24, 30), (44, 30), (72, 30)],
        mode: rustpg2d_engine::entity::PatrolMode::PingPong,
        pause: 1.5,
        speed: 0.0,
    });
    guard.triggers = vec![trig(TriggerOn::Interact, vec![], vec![Action::RunScript { function: "guard_talk".into() }])];
    // Dagny, the night watch: rounds from the square to the gate and the bridge.
    let dagny = b.entity(EntityKind::Character, "guard_f", 71, 27);
    dagny.name = "dagny".into();
    dagny.light = Some(LightSpec::new(4.0, [255, 220, 170], false));
    dagny.schedule = vec![
        facing(stop(20.0, 34, 27), Facing::Down),
        facing(stop(22.0, 74, 30), Facing::Right),
        facing(stop(0.0, 21, 30), Facing::Left),
        facing(stop(2.0, 34, 27), Facing::Down),
        facing(stop(4.0, 74, 30), Facing::Right),
        inside(stop(6.0, barracks.0, barracks.1 + 1)),
    ];
    dagny.triggers = vec![
        line(vec![between(20.0, 6.0)], "Dagny", "Night watch. Off to bed, {player} - or keep your eyes open with me."),
        line(vec![], "Dagny", "The gate stays locked after dark. Orders."),
    ];
    // Bram the baker: dough at dawn, the bakery counter, the bread stall, the tavern, bed.
    let bram = b.entity(EntityKind::Character, "baker", 39, 26);
    bram.name = "bram".into();
    bram.schedule = vec![
        working(visit(5.0, "bakery", 9, 5, ""), ActivityKind::Craft, "", Facing::Down),
        facing(visit(9.0, "bakery", 4, 4, ""), Facing::Down),
        facing(stop(13.0, 39, 26), Facing::Down),
        visit(17.0, "tavern", 4, 8, "stool_2"),
        visit(21.0, "bakery", 9, 9, "baker_bed"),
    ];
    bram.triggers = vec![
        line(vec![posed("lie")], "Bram", "Zzz... rise, dough, rise..."),
        line(vec![posed("sit")], "Bram", "Off duty, {player}! But I'll sell you a loaf all the same."),
        line(vec![between(5.0, 9.0)], "Bram", "Up since five! The first loaves are nearly done."),
        line(vec![], "Bram", "Fresh bread, warm from the oven! Can I tempt you?"),
    ];
    // Brom the smith: the anvil, a midday ale, the anvil again, the tavern, bed.
    let brom = b.entity(EntityKind::Character, "blacksmith", 49, 13);
    brom.name = "brom".into();
    brom.schedule = vec![
        working(visit(7.0, "smithy", 8, 6, ""), ActivityKind::Craft, "hammer", Facing::Up),
        visit(12.0, "tavern", 11, 6, "stool_3"),
        working(visit(13.5, "smithy", 8, 6, ""), ActivityKind::Craft, "hammer", Facing::Up),
        visit(19.0, "tavern", 11, 6, "stool_3"),
        visit(22.0, "smithy", 2, 9, "brom_bed"),
    ];
    brom.triggers = vec![
        line(vec![posed("lie")], "Brom", "Zzz... *snore*..."),
        line(vec![posed("sit")], "Brom", "Ahh. A cold ale after a hot forge. What do you need, {player}?"),
        line(vec![], "Brom", "Blades, picks, horseshoes. If it's iron, I can make it."),
    ];
    // Wren the herbalist: the herb beds, the apothecary counter, the pond bank, the tavern, bed.
    let wren = b.entity(EntityKind::Character, "herbalist", 24, 35);
    wren.name = "wren".into();
    wren.schedule = vec![
        working(stop(7.0, 24, 34), ActivityKind::Farm, "", Facing::Up),
        facing(visit(10.0, "apothecary", 5, 4, ""), Facing::Down),
        facing(stop(16.0, 60, 49), Facing::Right),
        visit(19.0, "tavern", 12, 8, "stool_4"),
        visit(22.0, "apothecary", 9, 9, "wren_bed"),
    ];
    wren.triggers = vec![
        line(vec![posed("lie")], "Wren", "Zzz... feverleaf... two pinches..."),
        line(vec![posed("sit")], "Wren", "Even herbalists need a break, {player}."),
        line(vec![], "Wren", "Herbs for every ill. Moonpetal is the rarest - it only opens at night."),
    ];
    // Brother Ansel: the altar at dawn, the graveyard at noon, the altar again, bed.
    let ansel = b.entity(EntityKind::Character, "priest", 38, 43);
    ansel.name = "ansel".into();
    ansel.schedule = vec![
        working(visit(6.0, "chapel", 5, 2, ""), ActivityKind::Read, "", Facing::Down),
        working(stop(13.0, 49, 39), ActivityKind::Sweep, "broom", Facing::Down),
        working(visit(17.0, "chapel", 5, 2, ""), ActivityKind::Read, "", Facing::Down),
        visit(21.0, "chapel", 9, 13, "ansel_bed"),
    ];
    ansel.triggers = vec![
        line(vec![posed("lie")], "Brother Ansel", "Zzz..."),
        line(vec![between(12.9, 17.0)], "Brother Ansel", "The departed rest easy when their graves are kept. Peace be with you, {player}."),
        line(vec![], "Brother Ansel", "The bell rings at noon, every day since the founding. Welcome to our chapel."),
    ];
    // Jorun the miller: the mill yard, the bread stall, the millstone, the tavern, home.
    let jorun = b.entity(EntityKind::Character, "miller", 9, 16);
    jorun.name = "jorun".into();
    jorun.schedule = vec![
        working(stop(6.0, 9, 16), ActivityKind::Sweep, "broom", Facing::Down),
        facing(stop(9.0, 39, 26), Facing::Down),
        working(visit(12.0, "mill", 4, 5, ""), ActivityKind::Craft, "", Facing::Up),
        visit(19.0, "tavern", 4, 11, "stool_6"),
        visit(22.0, "mill", 2, 7, "jorun_bed"),
    ];
    jorun.triggers = vec![
        line(vec![posed("lie")], "Jorun", "Zzz... the wind... the wind is turning..."),
        line(vec![posed("sit")], "Jorun", "The sails turn without me for an hour. Cheers, {player}!"),
        line(vec![], "Jorun", "Wheat in, flour out. The mill never sleeps while the wind blows."),
    ];
    // Hal and Beth, the farmers.
    let hal = b.entity(EntityKind::Character, "farmer", 6, 35);
    hal.name = "hal".into();
    hal.schedule = vec![
        working(stop(6.0, 6, 35), ActivityKind::Farm, "hoe", Facing::Down),
        inside(stop(12.0, farmhouse.0, farmhouse.1 + 1)),
        working(stop(13.0, 10, 36), ActivityKind::Farm, "hoe", Facing::Down),
        facing(stop(18.0, 7, 47), Facing::Down),
        inside(stop(20.0, farmhouse.0, farmhouse.1 + 1)),
    ];
    hal.triggers = vec![line(vec![], "Hal", "Plant, wait, harvest. And pray for rain - it makes them grow faster!")];
    let beth = b.entity(EntityKind::Character, "farmer_f", 12, 46);
    beth.name = "beth".into();
    beth.schedule = vec![
        facing(stop(7.0, 12, 45), Facing::Up),
        facing(stop(9.0, 29, 20), Facing::Down),
        working(stop(12.0, 4, 38), ActivityKind::Farm, "hoe", Facing::Down),
        stop(17.0, 33, 22),
        inside(stop(19.5, farmhouse.0, farmhouse.1 + 1)),
    ];
    beth.triggers = vec![line(vec![], "Beth", "Eggs, milk, cheese - all fresh from the farm across the river!")];
    // Old Finn, the fisher: the dock at dawn, the fish stall, the dock again, home.
    let finn = b.entity(EntityKind::Character, "fisher", 68, 48);
    finn.name = "finn".into();
    finn.schedule = vec![
        working(stop(5.0, 68, 48), ActivityKind::Fish, "fishing_rod", Facing::Right),
        facing(stop(10.0, 39, 20), Facing::Down),
        working(stop(14.0, 68, 48), ActivityKind::Fish, "fishing_rod", Facing::Right),
        inside(stop(20.0, fisher_hut.0, fisher_hut.1 + 1)),
    ];
    finn.triggers = vec![line(vec![], "Finn", "Sixty years I've fished this pond. The big trout still laughs at me.")];
    // Oswin, the town crier.
    let oswin = b.entity(EntityKind::Character, "town_crier", 34, 18);
    oswin.name = "oswin".into();
    oswin.schedule = vec![
        facing(stop(9.0, 34, 18), Facing::Down),
        visit(12.0, "tavern", 11, 9, "stool_5"),
        facing(stop(15.0, 34, 18), Facing::Down),
        inside(stop(19.0, crier_home.0, crier_home.1 + 1)),
    ];
    oswin.triggers = vec![trig(TriggerOn::Interact, vec![], vec![Action::RunScript { function: "crier_talk".into() }])];
    // Ida, Mira's mother: sweeps the yard, sells at the stall, the well, home.
    let ida = b.entity(EntityKind::Character, "villager_f", 57, 41);
    ida.name = "ida".into();
    ida.schedule = vec![
        working(stop(7.0, 57, 42), ActivityKind::Sweep, "broom", Facing::Down),
        facing(stop(12.0, 29, 20), Facing::Down),
        visit(15.0, "home_mira", 5, 5, ""),
        stop(17.0, 35, 22),
        visit(20.0, "home_mira", 8, 4, "ida_bed"),
    ];
    ida.triggers = vec![
        line(vec![posed("lie")], "Ida", "Zzz..."),
        line(vec![], "Ida", "My little Pip is ill with a fever. If only Wren had the right herbs..."),
    ];
    // Children playing around the square and in the orchard.
    for (name, def, x, y, ox) in [("lotte", "child_girl", 32, 23, 62), ("finnick", "child_boy", 37, 23, 68)] {
        let c = b.entity(EntityKind::Character, def, x, y);
        c.name = name.into();
        c.schedule = vec![stop(8.0, x, y), inside(stop(12.0, crier_home.0, crier_home.1 + 1)), stop(14.0, ox, 17), inside(stop(19.0, crier_home.0, crier_home.1 + 1))];
    }
    let lotte_lines = vec![line(vec![], "Lotte", "Tag! You're it! ...You're not playing, are you."), line(vec![], "Lotte", "Papa shouts the news every day. He's the loudest man in Emberbrook!")];
    let finnick_lines = vec![line(vec![], "Finnick", "I'm going to be a knight! Or a baker. Bakers get cake.")];
    for (name, lines) in [("lotte", lotte_lines), ("finnick", finnick_lines)] {
        if let Some(e) = b.m.entities.iter_mut().find(|e| e.name == name) {
            e.triggers = lines;
        }
    }
    // Pip, recovered (the herbalist's quest makes him appear: inactive until then).
    let pip = b.entity(EntityKind::Character, "child_boy", 38, 23);
    pip.name = "pip".into();
    pip.active = false;
    pip.schedule = vec![stop(8.0, 38, 23), visit(12.0, "home_mira", 4, 5, ""), stop(14.0, 64, 17), visit(19.0, "home_mira", 4, 5, ""), visit(20.0, "home_mira", 1, 6, "pip_bed")];
    pip.triggers = vec![line(vec![], "Pip", "I'm all better! Wren's remedy tasted awful, though.")];
    // Animals: the dog at the square, chickens by the coop, sheep and a cow in the pen.
    b.entity(EntityKind::Character, "dog", 36, 25).name = "rufus".into();
    for (x, y) in [(10, 45), (13, 46), (14, 44), (11, 47)] {
        b.entity(EntityKind::Character, "chicken", x, y);
    }
    for (x, y) in [(5, 51), (8, 53), (9, 50)] {
        b.entity(EntityKind::Character, "sheep", x, y);
    }
    b.entity(EntityKind::Character, "cow", 6, 54);

    // --- The map: settings and triggers.
    b.m.edges.east = Some(EdgeLink { target_map: "forest".into(), offset: -6, depth: 1 });
    let fog = |on: bool| Action::SetWeather { map: String::new(), weather: on.then(|| Weather::new(WeatherKind::Fog, 0.35)) };
    b.m.triggers = vec![
        // A new game's intro: a cut-scene (the script's intro_cutscene()), then the welcome.
        Trigger { once: true, ..trig(TriggerOn::MapEnter, vec![], vec![Action::Cutscene { function: "intro_cutscene".into() }]) },
        Trigger {
            on: TriggerOn::MapEnter,
            conditions: vec![],
            actions: vec![dialog(
                "",
                "Welcome to the R(ust)PG2D demo! Move: arrows/WASD or left-click. Interact: E/Space or right-click. Attack: X or left-click an enemy. Switch weapon: Q. Character & inventory: C or I, skills: K, spells: L. Quest journal: J. Map: M. Save/Load: F5/F9. Debug: F3.",
            )],
            once: true,
        },
        // Village life by the clock (Time reached): mist at dawn, the cock... hens at six, the
        // chapel bell at noon. Entering the map sets the mist to fit the hour.
        trig(TriggerOn::Hour { hour: 5.5 }, vec![], vec![fog(true)]),
        trig(TriggerOn::Hour { hour: 6.0 }, vec![], vec![Action::PlaySound { sound: "cluck".into() }]),
        trig(TriggerOn::Hour { hour: 8.5 }, vec![], vec![fog(false)]),
        trig(TriggerOn::Hour { hour: 12.0 }, vec![], vec![Action::PlaySound { sound: "bell".into() }]),
        trig(TriggerOn::MapEnter, vec![between(5.5, 8.5)], vec![fog(true)]),
        trig(TriggerOn::MapEnter, vec![between(8.5, 5.5)], vec![fog(false)]),
    ];
    b.m.background = [20, 40, 20];
    b.m.music = "music_village".into();
    b.m.lighting = MapLighting { mode: LightMode::Outdoor, darkness: 0.7 };
    b.m.parallax.push(rustpg2d_engine::map::ParallaxLayer { sheet: "cloud_shadows".into(), factor: [0.8, 0.8], drift: [9.0, 4.0], above: true, alpha: 0.2, scale: 4.0, ..Default::default() });
    // Auto-tiling: soft edges for streets, the square, the yard, fields, orchard, river and
    // pond. Streets run up to the doors, the river under its bridges.
    autotile_terrain(
        &mut b.m,
        |m, x, y| match ground(m, x, y)? {
            t::PATH => Some("Dirt path"),
            t::COBBLE => Some("Cobblestone"),
            t::STONE_FLOOR => Some("Flagstones"),
            t::TILLED => Some("Farmland"),
            t::FLOWER_BED => Some("Meadow"),
            t::SAND => Some("Sand"),
            t::WATER if pond(x, y).is_some() => Some("Shore"),
            t::WATER => Some("Pond"),
            _ => None,
        },
        |m, terrain, x, y| match terrain {
            "Dirt path" | "Cobblestone" | "Flagstones" => matches!(tile_at(m, 1, x, y), Some(t::DOOR | t::STONE_DOOR)) || (terrain == "Dirt path" && ground(m, x, y) == Some(t::BRIDGE)),
            "Pond" => ground(m, x, y) == Some(t::BRIDGE),
            "Shore" => matches!(ground(m, x, y), Some(t::DOCK | t::DOCK_EDGE)),
            _ => false,
        },
    );
    b.m
}

// ------------------------------------------------------------------ interiors

/// An interior `w` x `h`: void around, two rows of `wall` at the top, `floor`, a walkable door
/// in the middle of the bottom row with the `entrance` spawn above it and a portal back to the
/// village (`<name>_door`).
fn interior(name: &str, w: i32, h: i32, floor: u32, wall: u32, windows: &[i32]) -> B {
    let mut b = B::new(name, w as u32, h as u32);
    b.fill(0, 0, 0, w, h, t::VOID);
    b.fill(0, 1, 2, w - 2, h - 3, floor);
    b.fill(1, 1, 0, w - 2, 2, wall);
    for &x in windows {
        b.set(1, x, 1, if wall == t::STONE_WALL { t::STONE_WINDOW } else { t::WINDOW });
    }
    b.fill(1, 0, 0, 1, h, t::VOID);
    b.fill(1, w - 1, 0, 1, h, t::VOID);
    b.fill(1, 0, h - 1, w, 1, t::VOID);
    let door = (w / 2, h - 1);
    b.set(0, door.0, door.1, floor);
    b.set(1, door.0, door.1, if wall == t::STONE_WALL { t::STONE_DOOR } else { t::DOOR });
    b.m.set_collision(door.0 as u32, door.1 as u32, CollisionOverride::Walkable);
    b.spawn("entrance", door.0 as u32, door.1 as u32 - 1);
    b.portal(door.0 as u32, door.1 as u32, "village", &format!("{name}_door"));
    b.m.background = [13, 13, 20];
    b.m.music = "music_village".into();
    b
}

/// A bed (or another prop) with a name: schedules send people to it.
fn named(b: &mut B, def: &str, x: i32, y: i32, w: i32, h: i32, name: &str) {
    b.big(def, x, y, w, h).name = name.into();
}

/// Carpets and rugs get their edges.
fn finish(mut b: B) -> Map {
    autotile_terrain(
        &mut b.m,
        |m, x, y| match ground(m, x, y)? {
            t::CARPET => Some("Carpet"),
            t::RUG => Some("Rug"),
            _ => None,
        },
        |_, _, _, _| false,
    );
    b.m
}

/// The Copper Kettle: the bar with Greta, the kitchen hearth (a cooking station), tables with
/// stools, two inn beds (paid for at the bar), the pantry.
pub fn tavern() -> Map {
    let mut b = interior("tavern", 16, 13, t::WOOD_FLOOR, t::TIMBER_WALL_X, &[5, 10]);
    b.m.music = "music_tavern".into();
    b.fill(0, 7, 5, 2, 7, t::CARPET);
    b.fill(1, 1, 4, 6, 1, t::COUNTER);
    b.big("keg_rack", 3, 2, 2, 1);
    b.entity(EntityKind::Prop, "keg", 5, 2);
    b.big("hearth", 12, 3, 2, 2);
    let pantry = b.entity(EntityKind::Prop, "chest", 14, 3);
    pantry.container = Some(ContainerSpec {
        size: 8,
        items: vec![ItemStack::new("trout", 2), ItemStack::new("wheat", 6), ItemStack::new("carrot", 4), ItemStack::new("mushroom", 2), ItemStack::new("pumpkin", 1), ItemStack::new("apple", 2), ItemStack::new("coal", 2)],
        open_state: "open".into(),
        ..Default::default()
    });
    // Tables with a stool on each side (stools facing down on the north side, up on the south).
    for (tx, ty, a, c) in [(4, 7, "stool_1", "stool_2"), (12, 7, "stool_3", "stool_4"), (12, 10, "stool_5", "stool_7"), (4, 10, "stool_8", "stool_6")] {
        b.big("tavern_table", tx, ty, 2, 1);
        b.entity(EntityKind::Prop, "stool", tx - 1, ty - 1).name = a.into();
        b.entity(EntityKind::Prop, "stool_up", tx, ty + 1).name = c.into();
    }
    // The inn's beds: sleeping in them needs a paid room (Database: the inn bed's furniture).
    named(&mut b, "inn_bed", 14, 6, 1, 2, "inn_bed_1");
    named(&mut b, "inn_bed", 14, 9, 1, 2, "inn_bed_2");
    named(&mut b, "bed", 1, 11, 1, 2, "greta_bed");
    b.entity(EntityKind::Prop, "flower_pot", 1, 6);
    // Greta behind the bar.
    let greta = b.entity(EntityKind::Character, "innkeeper", 3, 3);
    greta.name = "greta".into();
    greta.schedule = vec![facing(stop(6.0, 3, 3), Facing::Down), ScheduleStop { prop: "greta_bed".into(), ..stop(23.0, 2, 11) }];
    greta.triggers = vec![
        line(vec![posed("lie")], "Greta", "Zzz... last orders... zzz..."),
        trig(
            TriggerOn::Interact,
            vec![not_posed("lie")],
            vec![
                Action::Choice {
                    speaker: "Greta".into(),
                    text: "Welcome to the Copper Kettle, {player}! What'll it be?".into(),
                    options: vec!["Something to eat and drink".into(), "A room for the night (5 coins)".into(), "Just looking".into()],
                    var: "greta_ask".into(),
                },
                Action::If {
                    conditions: vec![cond(Cond::Var { name: "greta_ask".into(), op: CmpOp::Eq, value: 0 })],
                    then: vec![Action::OpenShop { target: "self".into() }],
                    otherwise: vec![],
                },
                Action::If {
                    conditions: vec![cond(Cond::Var { name: "greta_ask".into(), op: CmpOp::Eq, value: 1 }), cond(Cond::HasItem { item: "coin".into(), count: 5 })],
                    then: vec![
                        Action::TakeItem { item: "coin".into(), count: 5 },
                        Action::SetFlag { name: "inn_room_paid".into(), value: true },
                        dialog("Greta", "Either bed by the east wall is yours tonight. Sleep well!"),
                    ],
                    otherwise: vec![],
                },
                Action::If {
                    conditions: vec![cond(Cond::Var { name: "greta_ask".into(), op: CmpOp::Eq, value: 1 }), not(Cond::HasItem { item: "coin".into(), count: 5 })],
                    then: vec![dialog("Greta", "Five coins for a room, love. Come back when your purse is heavier.")],
                    otherwise: vec![],
                },
            ],
        ),
    ];
    // A paid room lasts one night.
    b.m.triggers = vec![trig(TriggerOn::Sleep, vec![], vec![Action::SetFlag { name: "inn_room_paid".into(), value: false }])];
    finish(b)
}

/// The bakery: the oven (cooking station, lit), shelves, the counter, Bram's bed, the cat.
pub fn bakery() -> Map {
    let mut b = interior("bakery", 12, 11, t::TILED_FLOOR, t::HOUSE_WALL, &[3]);
    b.big("oven", 10, 3, 2, 2);
    b.big("bread_shelf", 3, 3, 2, 2);
    b.fill(1, 2, 5, 6, 1, t::COUNTER);
    b.big("dough_table", 10, 6, 2, 1);
    b.entity(EntityKind::Prop, "flour_sacks", 1, 8);
    b.entity(EntityKind::Prop, "flour_sacks", 2, 9);
    b.entity(EntityKind::Prop, "cat_basket", 4, 9);
    named(&mut b, "bed", 10, 9, 1, 2, "baker_bed");
    b.entity(EntityKind::Character, "cat", 5, 8).name = "bakery_cat".into();
    finish(b)
}

/// The smithy: the forge (smelting), the anvil, racks, the supply chest, Brom's bed.
pub fn smithy() -> Map {
    let mut b = interior("smithy", 14, 11, t::STONE_FLOOR, t::STONE_WALL, &[4, 9]);
    b.big("forge", 11, 3, 2, 2);
    b.entity(EntityKind::Prop, "bellows", 12, 4);
    b.entity(EntityKind::Prop, "anvil", 8, 5);
    b.big("weapon_rack", 3, 3, 2, 2);
    b.big("tool_rack", 6, 3, 2, 2);
    b.entity(EntityKind::Prop, "grindstone", 4, 7);
    b.entity(EntityKind::Prop, "barrel", 12, 9);
    let chest = b.entity(EntityKind::Prop, "chest", 12, 7);
    chest.container = Some(ContainerSpec {
        size: 8,
        items: vec![ItemStack::new("pickaxe", 1), ItemStack::new("tongs", 1), ItemStack::new("hammer", 1), ItemStack::new("coal", 5)],
        open_state: "open".into(),
        ..Default::default()
    });
    named(&mut b, "bed", 1, 9, 1, 2, "brom_bed");
    finish(b)
}

/// The apothecary: the alchemy table, a cauldron, racks of herbs and potions, Wren's chest.
pub fn apothecary() -> Map {
    let mut b = interior("apothecary", 12, 11, t::WOOD_FLOOR, t::HOUSE_WALL, &[3, 8]);
    b.fill(0, 4, 6, 4, 3, t::RUG);
    b.entity(EntityKind::Prop, "alchemy_table", 5, 3);
    b.entity(EntityKind::Prop, "cauldron", 7, 3);
    b.big("herb_rack", 2, 3, 1, 2);
    b.big("potion_shelf", 10, 3, 2, 2);
    b.fill(1, 3, 5, 6, 1, t::COUNTER);
    let chest = b.entity(EntityKind::Prop, "chest", 1, 9);
    chest.container = Some(ContainerSpec {
        size: 8,
        items: vec![ItemStack::new("mortar", 1), ItemStack::new("herb", 2), ItemStack::new("mushroom", 2)],
        open_state: "open".into(),
        ..Default::default()
    });
    named(&mut b, "bed", 10, 9, 1, 2, "wren_bed");
    finish(b)
}

/// The chapel: the altar, candles, pews to sit in, the bell rope, Brother Ansel's bed.
pub fn chapel() -> Map {
    let mut b = interior("chapel", 12, 15, t::TILED_FLOOR, t::STONE_WALL, &[]);
    for x in [3, 8] {
        b.set(1, x, 1, t::CHAPEL_WINDOW);
    }
    b.fill(0, 5, 4, 2, 10, t::CARPET);
    b.big("altar", 6, 3, 2, 1);
    b.entity(EntityKind::Prop, "candle_stand", 3, 3);
    b.entity(EntityKind::Prop, "candle_stand", 8, 3);
    b.entity(EntityKind::Prop, "bell_rope", 1, 2);
    for (k, y) in [6, 8, 10].into_iter().enumerate() {
        named(&mut b, "pew", 3, y, 2, 1, &format!("pew_l{}", k + 1));
        named(&mut b, "pew", 9, y, 2, 1, &format!("pew_r{}", k + 1));
    }
    named(&mut b, "bed", 10, 13, 1, 2, "ansel_bed");
    b.m.lighting = MapLighting { mode: LightMode::Dark, darkness: 0.35 };
    finish(b)
}

/// Mira's home: three beds (Pip, ill with a fever, in his), a fireplace, a table, the cat basket.
pub fn home_mira() -> Map {
    let mut b = interior("home_mira", 10, 9, t::WOOD_FLOOR, t::HOUSE_WALL, &[2, 7]);
    named(&mut b, "bed", 1, 5, 1, 2, "pip_bed");
    named(&mut b, "bed", 2, 5, 1, 2, "mira_bed");
    b.big("fireplace", 5, 3, 2, 2);
    named(&mut b, "bed_side", 8, 3, 2, 1, "ida_bed");
    b.set(1, 7, 6, t::TABLE);
    b.entity(EntityKind::Prop, "chair_right", 6, 6);
    b.entity(EntityKind::Prop, "chair_up", 7, 7);
    b.entity(EntityKind::Prop, "cat_basket", 1, 7);
    // Pip, sick in bed (the herbalist's quest cures him).
    let pip = b.entity(EntityKind::Character, "child_boy", 1, 6);
    pip.name = "pip_sick".into();
    pip.schedule = vec![ScheduleStop { prop: "pip_bed".into(), ..stop(0.0, 1, 6) }];
    pip.triggers = vec![line(vec![], "Pip", "*cough* I feel so hot... Mama says Wren will know what to do.")];
    // Biscuit, Mira's cat, once found (inactive until then).
    let cat = b.entity(EntityKind::Character, "cat_ginger", 2, 7);
    cat.name = "biscuit_home".into();
    cat.active = false;
    finish(b)
}

/// The mill: the millstone (grinds flour), sacks, crates, Jorun's bed, and a ginger cat
/// hiding behind the hay (inactive until the lost-cat quest).
pub fn mill() -> Map {
    let mut b = interior("mill", 10, 9, t::WOOD_FLOOR, t::STONE_WALL, &[2, 7]);
    b.big("millstone", 4, 4, 2, 2);
    for (x, y) in [(1, 2), (2, 2), (1, 3)] {
        b.entity(EntityKind::Prop, "flour_sacks", x, y);
    }
    b.entity(EntityKind::Prop, "crate", 8, 2);
    b.entity(EntityKind::Prop, "crate", 8, 3);
    b.entity(EntityKind::Prop, "haystack", 8, 7);
    named(&mut b, "bed", 1, 7, 1, 2, "jorun_bed");
    let cat = b.entity(EntityKind::Character, "cat_ginger", 7, 7);
    cat.name = "biscuit".into();
    cat.active = false;
    finish(b)
}
