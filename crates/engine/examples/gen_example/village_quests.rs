//! The village quests (0.11.0): *The Lost Cat* (Mira), *Bread for the Festival* (Bram),
//! *The Herbalist's Remedy* (Ida, Wren and Pip) and *Rumours at the Tavern* (Greta and Brom),
//! with their achievements. The quests' triggers go in front of the villagers' own lines
//! (found by name on the maps village_map.rs builds); the raided flour cart is in the forest,
//! Halvar's locket in the old mine.

use super::*;

pub fn quests() -> Vec<QuestDef> {
    vec![
        QuestDef {
            id: "lost_cat".into(),
            title: "The Lost Cat".into(),
            description: "Mira's ginger cat Biscuit has been missing for two days. She asked me to find him.".into(),
            stages: vec!["Ask around Emberbrook about Mira's cat Biscuit ({cat_clues}/2 clues).".into(), "Search the old mill across the river.".into(), "Bring Mira the good news.".into()],
            xp: 30,
        },
        QuestDef {
            id: "festival_bread".into(),
            title: "Bread for the Festival".into(),
            description: "Outlaws raided the miller's flour cart on the forest road. Without flour, Bram the baker can't bake the loaf for the harvest festival.".into(),
            stages: vec![
                "Recover three sacks of flour from the raided cart on the forest road, beyond the east gate.".into(),
                "Bake a Festival Loaf at Bram's oven (two flour, an egg, a jar of honey) and bring it to Bram.".into(),
            ],
            xp: 60,
        },
        QuestDef {
            id: "herbal_remedy".into(),
            title: "The Herbalist's Remedy".into(),
            description: "Ida's little son Pip is ill with a fever. Wren the herbalist can brew a remedy if someone gathers the herbs for it.".into(),
            stages: vec![
                "Gather feverleaf (by the forest path), bitterroot (on the pond bank) and a moonpetal (it opens only at night, in the graveyard), and bring them to Wren.".into(),
                "Brew the Herbal Remedy at Wren's alchemy table and bring it to Pip.".into(),
            ],
            xp: 70,
        },
        QuestDef {
            id: "tavern_rumours".into(),
            title: "Rumours at the Tavern".into(),
            description: "Greta told me about Halvar, a miner who went into the old mine forty years ago and never came back.".into(),
            stages: vec!["Ask Brom the smith about Halvar.".into(), "Find Halvar's silver locket in the old mine and bring it to Brom.".into()],
            xp: 90,
        },
    ]
}

pub fn achievements() -> Vec<AchievementDef> {
    let icon = |sheet: &str, f: u32| SpriteRef::new(sheet, f);
    let done = |q: &str| vec![quest_is(q, QuestCheck::Completed, 0)];
    vec![
        AchievementDef::new("cat_person", "Cat Person", "Find Mira's cat Biscuit.", icon("items", it::TROUT), done("lost_cat")),
        AchievementDef::new("festival_baker", "Festival Baker", "Bake the festival loaf for Bram.", icon("items2", it2::FESTIVAL_BREAD), done("festival_bread")),
        AchievementDef::new("village_healer", "Village Healer", "Cure Pip's fever.", icon("items2", it2::REMEDY), done("herbal_remedy")),
        AchievementDef::new("old_debts", "Old Debts", "Bring Halvar's locket home to Brom.", icon("items2", it2::LOCKET), done("tavern_rumours")),
    ]
}

/// Script functions of the quests (appended to main.rhai).
pub const SCRIPT: &str = r#"
// The Lost Cat (Mira's quest): Bram, Old Finn and Jorun each saw something; two clues lead to
// the mill. Their "clue" triggers call this.
fn cat_clue() {
    add_var("cat_clues", 1);
    if get_var("cat_clues") >= 2 && quest_stage("lost_cat") == 0 {
        set_quest_stage("lost_cat", 1);
        say("Two clues point the same way: Biscuit must be hiding in the old mill, across the river.");
    }
}
"#;

fn not_quest(id: &str, check: QuestCheck, stage: u32) -> Condition {
    Condition { negate: true, ..quest_is(id, check, stage) }
}

fn var_is(name: &str, value: i64) -> Condition {
    cond(Cond::Var { name: name.into(), op: CmpOp::Eq, value })
}

fn has(item: &str, count: u32) -> Condition {
    cond(Cond::HasItem { item: item.into(), count })
}

fn awake() -> Condition {
    not(Cond::Pose { target: "self".into(), pose: "lie".into() })
}

fn give(item: &str, count: u32) -> Action {
    Action::GiveItem { item: item.into(), count }
}

fn take(item: &str, count: u32) -> Action {
    Action::TakeItem { item: item.into(), count }
}

fn set_flag(name: &str) -> Action {
    Action::SetFlag { name: name.into(), value: true }
}

fn active(target: &str, on: bool) -> Action {
    Action::SetActive { target: target.into(), active: on }
}

/// Puts `triggers` in front of the triggers of the entity named `name`.
fn prepend(m: &mut Map, name: &str, triggers: Vec<Trigger>) {
    let e = m.entities.iter_mut().find(|e| e.name == name).unwrap_or_else(|| panic!("no entity {name} on {}", m.name));
    let mut t = triggers;
    t.append(&mut e.triggers);
    e.triggers = t;
}

/// A clue about the lost cat, told once while the quest asks for clues.
fn clue(speaker: &str, flag_name: &str, text: &str) -> Trigger {
    trig(
        TriggerOn::Interact,
        vec![quest_is("lost_cat", QuestCheck::AtStage, 0), not(flag(flag_name)), awake()],
        vec![dialog(speaker, text), set_flag(flag_name), Action::RunScript { function: "cat_clue".into() }],
    )
}

/// The village's people: Mira, Bram, Finn, Jorun, Ida, Wren, Brom; Pip once cured.
pub fn village(m: &mut Map) {
    // Mira asks for her cat; her chatter (mira_talk) comes once the cat is home.
    let e = m.entities.iter_mut().find(|e| e.name == "mira").expect("mira");
    e.triggers = vec![
        line_lie("Mira", "Mmh... Biscuit...? Zzz..."),
        trig(
            TriggerOn::Interact,
            vec![quest_is("lost_cat", QuestCheck::NotStarted, 0), awake()],
            vec![
                dialog("Mira", "Hi, {player}... Have you seen a ginger cat? My Biscuit has been gone for two whole days!"),
                Action::Choice { speaker: "Mira".into(), text: "Will you help me find him?".into(), options: vec!["I'll find him".into(), "Not now".into()], var: "mira_cat".into() },
                Action::If {
                    conditions: vec![var_is("mira_cat", 0)],
                    then: vec![quest("lost_cat", QuestOp::Start, 0), dialog("Mira", "Thank you! Ask around - somebody must have seen him.")],
                    otherwise: vec![dialog("Mira", "Oh... okay. Biscuit, where are you...")],
                },
            ],
        ),
        trig(
            TriggerOn::Interact,
            vec![quest_is("lost_cat", QuestCheck::Active, 0), not_quest("lost_cat", QuestCheck::AtStage, 2), awake()],
            vec![dialog("Mira", "Any news of Biscuit? Bram, Old Finn or the miller might have seen him.")],
        ),
        trig(
            TriggerOn::Interact,
            vec![quest_is("lost_cat", QuestCheck::AtStage, 2), awake()],
            vec![
                dialog("Mira", "He was in the mill all along? And he's home now? Oh, thank you, thank you!"),
                quest("lost_cat", QuestOp::Complete, 0),
                give("coin", 10),
                give("apple_pie", 1),
                dialog("", "Received 10 coins and an apple pie!"),
            ],
        ),
        trig(TriggerOn::Interact, vec![quest_is("lost_cat", QuestCheck::Completed, 0), awake()], vec![Action::RunScript { function: "mira_talk".into() }]),
    ];
    // Bram: a clue about the cat, then the festival loaf.
    prepend(
        m,
        "bram",
        vec![
            clue("Bram", "clue_bram", "A ginger cat? One sneaked off with a bread crust this morning - towards the river and the mill."),
            trig(
                TriggerOn::Interact,
                vec![quest_is("festival_bread", QuestCheck::NotStarted, 0), awake()],
                vec![
                    dialog("Bram", "Trouble, {player}! Outlaws raided Jorun's flour cart on the forest road, and the harvest festival is in three days."),
                    Action::Choice {
                        speaker: "Bram".into(),
                        text: "Could you bring back the flour? Three sacks would save the festival loaf.".into(),
                        options: vec!["Leave it to me".into(), "Not now".into()],
                        var: "bram_flour".into(),
                    },
                    Action::If {
                        conditions: vec![var_is("bram_flour", 0)],
                        then: vec![quest("festival_bread", QuestOp::Start, 0), dialog("Bram", "Bless you! The cart lies by the road past the east gate. Mind the outlaws.")],
                        otherwise: vec![],
                    },
                ],
            ),
            trig(
                TriggerOn::Interact,
                vec![quest_is("festival_bread", QuestCheck::AtStage, 0), has("flour", 3), awake()],
                vec![
                    take("flour", 1),
                    give("egg", 1),
                    give("honey", 1),
                    quest("festival_bread", QuestOp::SetStage, 1),
                    dialog("Bram", "The flour! I'll keep one sack for the rolls. I'm run off my feet - bake the loaf at my oven yourself: two flour, this egg and this honey."),
                ],
            ),
            trig(
                TriggerOn::Interact,
                vec![quest_is("festival_bread", QuestCheck::AtStage, 1), has("festival_bread", 1), awake()],
                vec![
                    take("festival_bread", 1),
                    dialog("Bram", "Golden and braided, just like my grandmother's! The festival is saved, {player}."),
                    quest("festival_bread", QuestOp::Complete, 0),
                    give("coin", 40),
                    Action::GiveSkillPoints { amount: 1 },
                    dialog("", "Received 40 coins and a skill point!"),
                ],
            ),
        ],
    );
    // While Bram asks for help, his everyday lines wait.
    if let Some(e) = m.entities.iter_mut().find(|e| e.name == "bram") {
        for t in e.triggers.iter_mut().filter(|t| t.actions.len() == 1) {
            t.conditions.push(not_quest("festival_bread", QuestCheck::NotStarted, 0));
        }
    }
    prepend(m, "finn", vec![clue("Finn", "clue_finn", "Ginger cat? Saw one cross the bridge to the west bank yesterday, bold as a heron.")]);
    prepend(m, "jorun", vec![clue("Jorun", "clue_jorun", "There's been scratching behind the hay in my mill. Mice, I thought... or something bigger.")]);
    // Ida asks for help with Pip's fever; Wren knows the remedy.
    let e = m.entities.iter_mut().find(|e| e.name == "ida").expect("ida");
    e.triggers = vec![
        line_lie("Ida", "Zzz..."),
        trig(
            TriggerOn::Interact,
            vec![quest_is("herbal_remedy", QuestCheck::NotStarted, 0), awake()],
            vec![
                dialog("Ida", "My little Pip is ill with a fever. Wren can brew a remedy, but she needs feverleaf, bitterroot and a moonpetal."),
                Action::Choice { speaker: "Ida".into(), text: "Would you gather the herbs for her?".into(), options: vec!["I'll gather them".into(), "Not now".into()], var: "ida_herbs".into() },
                Action::If {
                    conditions: vec![var_is("ida_herbs", 0)],
                    then: vec![quest("herbal_remedy", QuestOp::Start, 0), dialog("Ida", "Thank you, {player}! Bring them to Wren at the apothecary.")],
                    otherwise: vec![],
                },
            ],
        ),
        trig(TriggerOn::Interact, vec![quest_is("herbal_remedy", QuestCheck::Active, 0), awake()], vec![dialog("Ida", "Please hurry, {player}. Pip's fever is getting worse.")]),
        trig(TriggerOn::Interact, vec![quest_is("herbal_remedy", QuestCheck::Completed, 0), awake()], vec![dialog("Ida", "Pip is running around again! I can never thank you enough.")]),
    ];
    prepend(
        m,
        "wren",
        vec![trig(
            TriggerOn::Interact,
            vec![quest_is("herbal_remedy", QuestCheck::AtStage, 0), has("feverleaf", 1), has("bitterroot", 1), has("moonpetal", 1), awake()],
            vec![
                dialog("Wren", "Feverleaf, bitterroot and a moonpetal - all three! Brew them at my alchemy table with a mortar and pestle."),
                Action::If {
                    conditions: vec![not(Cond::HasItem { item: "mortar".into(), count: 1 })],
                    then: vec![give("mortar", 1), dialog("", "Wren gives you her spare mortar and pestle.")],
                    otherwise: vec![],
                },
                quest("herbal_remedy", QuestOp::SetStage, 1),
            ],
        )],
    );
    // Brom remembers Halvar.
    prepend(
        m,
        "brom",
        vec![
            trig(
                TriggerOn::Interact,
                vec![quest_is("tavern_rumours", QuestCheck::AtStage, 0), awake()],
                vec![
                    dialog("Brom", "Halvar? Greta's been telling tales again. He was my friend. Forty years ago he went down into the old mine and never came back."),
                    dialog("Brom", "He always wore a silver locket with his Elsa's picture in it. If it's still down there... I'd like to have it. For her grave."),
                    quest("tavern_rumours", QuestOp::SetStage, 1),
                ],
            ),
            trig(
                TriggerOn::Interact,
                vec![quest_is("tavern_rumours", QuestCheck::AtStage, 1), has("locket", 1), awake()],
                vec![
                    take("locket", 1),
                    dialog("Brom", "That's it. That's Halvar's locket... Thank you, {player}. I'll lay it on Elsa's grave tonight. Take this - and my best edge on anything you bring me."),
                    quest("tavern_rumours", QuestOp::Complete, 0),
                    give("coin", 50),
                    Action::GiveSkillPoints { amount: 1 },
                    dialog("", "Received 50 coins and a skill point!"),
                ],
            ),
        ],
    );
    // Pip, cured, comes out to play (the herbalist's quest made him well).
    m.triggers.push(trig(TriggerOn::MapEnter, vec![cond(flag("pip_cured")), not(flag("pip_up"))], vec![active("pip", true), set_flag("pip_up")]));
}

fn line_lie(speaker: &str, text: &str) -> Trigger {
    trig(TriggerOn::Interact, vec![cond(Cond::Pose { target: "self".into(), pose: "lie".into() })], vec![dialog(speaker, text)])
}

/// Greta: food, a room, or the rumour about Halvar (for a round).
pub fn tavern(m: &mut Map) {
    let e = m.entities.iter_mut().find(|e| e.name == "greta").expect("greta");
    let pick = |n: i64| var_is("greta_ask", n);
    e.triggers = vec![
        line_lie("Greta", "Zzz... last orders... zzz..."),
        trig(
            TriggerOn::Interact,
            vec![awake()],
            vec![
                Action::Choice {
                    speaker: "Greta".into(),
                    text: "Welcome to the Copper Kettle, {player}! What'll it be?".into(),
                    options: vec!["Something to eat and drink".into(), "A room for the night (5 coins)".into(), "Heard any rumours?".into(), "Just looking".into()],
                    var: "greta_ask".into(),
                },
                Action::If { conditions: vec![pick(0)], then: vec![Action::OpenShop { target: "self".into() }], otherwise: vec![] },
                Action::If {
                    conditions: vec![pick(1), has("coin", 5)],
                    then: vec![take("coin", 5), set_flag("inn_room_paid"), dialog("Greta", "Either bed by the east wall is yours tonight. Sleep well!")],
                    otherwise: vec![],
                },
                Action::If { conditions: vec![pick(1), not(Cond::HasItem { item: "coin".into(), count: 5 })], then: vec![dialog("Greta", "Five coins for a room, love. Come back when your purse is heavier.")], otherwise: vec![] },
                Action::If {
                    conditions: vec![pick(2), quest_is("tavern_rumours", QuestCheck::NotStarted, 0), has("coin", 5)],
                    then: vec![
                        take("coin", 5),
                        dialog("Greta", "Buy a round and I'll talk... Thank you, love. Forty years ago a miner, Halvar, went down into the old mine and never came out. They say his ghost still taps the walls."),
                        dialog("Greta", "Brom the smith knew him better than anyone. Ask him - if you dare."),
                        quest("tavern_rumours", QuestOp::Start, 0),
                    ],
                    otherwise: vec![],
                },
                Action::If {
                    conditions: vec![pick(2), quest_is("tavern_rumours", QuestCheck::NotStarted, 0), not(Cond::HasItem { item: "coin".into(), count: 5 })],
                    then: vec![dialog("Greta", "Rumours cost a round, love: five coins.")],
                    otherwise: vec![],
                },
                Action::If { conditions: vec![pick(2), quest_is("tavern_rumours", QuestCheck::Active, 0)], then: vec![dialog("Greta", "Talk to Brom, love. He knew Halvar best.")], otherwise: vec![] },
                Action::If {
                    conditions: vec![pick(2), quest_is("tavern_rumours", QuestCheck::Completed, 0)],
                    then: vec![
                        dialog("Greta", "Brom's been in better spirits since you found that locket. Have an ale on the house!"),
                        Action::If { conditions: vec![not(flag("free_ale"))], then: vec![give("ale", 1), set_flag("free_ale")], otherwise: vec![] },
                    ],
                    otherwise: vec![],
                },
            ],
        ),
    ];
}

/// Biscuit hides in the mill once the clues lead there.
pub fn mill(m: &mut Map) {
    m.triggers.push(trig(TriggerOn::MapEnter, vec![quest_is("lost_cat", QuestCheck::AtStage, 1), not(flag("biscuit_found"))], vec![active("biscuit", true)]));
    prepend(
        m,
        "biscuit",
        vec![trig(
            TriggerOn::Interact,
            vec![quest_is("lost_cat", QuestCheck::AtStage, 1)],
            vec![
                dialog("Biscuit", "Mrrp?"),
                dialog("", "It's Biscuit! Fat and happy from the mill's mice. He trots off towards home."),
                set_flag("biscuit_found"),
                active("self", false),
                quest("lost_cat", QuestOp::SetStage, 2),
            ],
        )],
    );
}

/// Mira's home: Biscuit back in his basket; Pip's cure.
pub fn home_mira(m: &mut Map) {
    m.triggers.push(trig(TriggerOn::MapEnter, vec![cond(flag("biscuit_found"))], vec![active("biscuit_home", true)]));
    prepend(
        m,
        "pip_sick",
        vec![trig(
            TriggerOn::Interact,
            vec![quest_is("herbal_remedy", QuestCheck::AtStage, 1), has("remedy", 1)],
            vec![
                take("remedy", 1),
                dialog("Pip", "Yuck, it's so bitter! ...Oh. Oh, my head doesn't hurt any more!"),
                quest("herbal_remedy", QuestOp::Complete, 0),
                give("coin", 20),
                Action::GiveSkillPoints { amount: 1 },
                dialog("", "Ida presses 20 coins into your hand. Received a skill point!"),
                set_flag("pip_cured"),
                active("self", false),
            ],
        )],
    );
    // His fever line only while he is ill.
    if let Some(e) = m.entities.iter_mut().find(|e| e.name == "pip_sick") {
        for t in e.triggers.iter_mut().skip(1) {
            t.conditions.push(not(flag("pip_cured")));
        }
    }
}

/// The forest road: the miller's raided cart, flour in the grass, and two outlaws who show up
/// while Bram's quest asks for the flour. (Entities at the end: older uids stay.)
pub fn forest(b: &mut B) {
    // Below the path at x 20 (the path wanders, so find its bottom row there).
    let x = 20;
    let path_bottom = (0..b.m.height as i32).filter(|&y| ground(&b.m, x, y) == Some(t::PATH)).max().unwrap_or(25);
    let y = path_bottom + 3;
    for yy in y - 2..=y + 1 {
        for xx in x - 3..=x + 3 {
            if ground(&b.m, xx, yy) != Some(t::PATH) && ground(&b.m, xx, yy) != Some(t::WATER) {
                b.m.layers[1].set(xx as u32, yy as u32, Tile::EMPTY);
                b.m.layers[2].set(xx as u32, (yy - 1) as u32, Tile::EMPTY);
            }
        }
    }
    b.big("broken_cart", x + 1, y, 2, 2);
    b.entity(EntityKind::Item, "flour", x - 2, y - 1);
    b.entity(EntityKind::Item, "flour", x + 3, y);
    for (k, (ox, oy)) in [(-2, 1), (3, -1)].into_iter().enumerate() {
        let o = b.entity(EntityKind::Character, "outlaw", x + ox, y + oy);
        o.name = format!("cart_bandit_{}", k + 1);
        o.active = false;
        // The first one carries the third sack.
        o.loot = Some(if k == 0 { vec![LootEntry::new("flour", 100.0, 1, 1)] } else { vec![LootEntry::new("coin", 100.0, 2, 5)] });
    }
    b.m.triggers.push(trig(
        TriggerOn::MapEnter,
        vec![quest_is("festival_bread", QuestCheck::AtStage, 0), not(flag("cart_bandits"))],
        vec![active("cart_bandit_1", true), active("cart_bandit_2", true), set_flag("cart_bandits"), dialog("", "By the road lies a broken cart, flour spilled in the grass - and two outlaws are still picking through it!")],
    ));
}

/// The old mine's treasure room: Halvar's bones and his locket. (Entities at the end.)
pub fn mine(b: &mut B) {
    b.entity(EntityKind::Prop, "bones", 6, 15);
    b.entity(EntityKind::Item, "locket", 5, 15);
}
