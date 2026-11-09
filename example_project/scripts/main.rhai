// Rhai scripts for the example project.
// Functions here can be called from triggers with the "Run script" action
// or used as conditions ("Script" condition, must return a bool).
//
// World state:  flag(n) set_flag(n, b) get_var(n) set_var(n, v) add_var(n, v)
//               item_count(id) has_item(id) give_item(id, n) take_item(id, n)
// Queued:       say(text) say(speaker, text) choice(text, [options], var)
//               teleport(map, spawn) set_active(target, b) set_frame(target, f)
//               move_by(target, dx, dy) face(target, "up"|"down"|"left"|"right")
//               wait(seconds) emit(event) then_call(fn)
//               heal(n) play_sound(id) play_music(id) set_locked(target, b)
//               open_shop(target) set_state(target, state)
//               patrol(target) patrol(target, [[x, y], ...]) stop_patrol(target)
//               follow(target) stop_following(target) is_companion(name) companion_count()
//               unlock_achievement(id) achievement_unlocked(id) open_crafting(station[, title]) can_craft(recipe)
//               stop_schedule(target) follow_schedule(target)
// Cut-scenes:   cutscene(fn) walk_to(target, x, y) approach(target, other) place(target, x, y)
//               hide_ui() show_ui() dim(amount, seconds) disable_spawners(map) enable_spawners(map)
// Quests:       start_quest(id) advance_quest(id) set_quest_stage(id, n) complete_quest(id)
//               fail_quest(id) (queued); quest_stage(id) (-1 = not started) quest_active(id)
//               quest_done(id)
// Player:       player_name() player_sex() ("male"/"female") is_female() is_male()
//               text("...") fills in placeholders: {player} {sir|madam} {variable}
// Skills/magic: skill(id) skill_points() knows_spell(id) mana(); queued: set_skill(id, level)
//               give_skill_points(n) learn_spell(id) add_mana(n) damage_around(dmg, radius)
//               player_light(radius, seconds, #{ color: [r, g, b], flicker })
//               heal_area(x, y, radius, n) light_at(x, y, radius, seconds, #{ ... })
// Spell effects (queued; x, y in tiles): aim(fn) calls fn(x, y) with the player's target;
//               fly(sheet, x, y, fn, #{ speed, frames: [a, b], fps, scale, light }) calls
//               fn(x, y) on arrival; effect(sheet, x, y, #{ ... }); damage_area(x, y, r, dmg)
// Other:        player_hp() player_max_hp() player_x() player_y() player_facing()
//               random(n) current_map()
// Weather:      set_weather(kind, [intensity]) set_map_weather(map, kind, intensity) (queued;
//               "rain", "snow", "fog", "none", "default"); weather() weather_intensity()
// Targets: "self" (trigger owner), "player", or an entity name.

fn mira_talk() {
    add_var("mira_talks", 1);
    let n = get_var("mira_talks");
    if n == 1 {
        // Placeholders instead of "..." + player_name() + "...": the whole text can be translated.
        say("Mira", "Hi! I'm Mira. {player}, right? Have you met Elder Rowan by the well?");
    } else if flag("shrine_lit") {
        // Scripts can check the player's sex too (or use "{hero|heroine}" in the text).
        if is_female() {
            say("Mira", "You lit the shrine? When I grow up I want to be a heroine like you!");
        } else {
            say("Mira", "You lit the shrine? When I grow up I want to be a hero like you!");
        }
    } else if n < 4 {
        say("Mira", "Talked to me {mira_talks} times now. You must be bored!");
    } else {
        say("Mira", "Want to see me walk?");
        choice("Watch Mira walk?", ["Sure", "No thanks"], "mira_walk");
        then_call("mira_walk");
    }
}

fn mira_walk() {
    if get_var("mira_walk") == 0 {
        move_by("mira", 2, 0);
        move_by("mira", 0, 1);
        move_by("mira", -2, 0);
        move_by("mira", 0, -1);
        face("mira", "down");
        say("Mira", "Ta-da!");
    }
}

fn shrine_touch() {
    if flag("shrine_lit") {
        say("The shrine glows softly. The forest is calm.");
        return;
    }
    if !flag("quest_accepted") {
        say("An old stone shrine. It seems dormant.");
        return;
    }
    cutscene("shrine_awakening");
}

// The shrine awakens (a cut-scene, the first time it is touched on the elder's quest): the
// clearing darkens, the statues' eyes and the shrine light up, a burst of magic rolls out -
// every monster left in the forest is destroyed and its spawners go quiet. Tilda's slime
// hunt can't go on then: accepted, it is done (she rewards it); not accepted, she won't offer it.
fn shrine_awakening() {
    face("player", "up");
    say("You lay your hand on the cold stone. It begins to hum under your fingers...");
    dim(0.8, 1.5);
    play_sound("chime");
    wait(1.6);
    let blue = #{ color: [120, 200, 255], flicker: true };
    light_at(47.5, 20.5, 1.5, 8, blue);         // a glow in the crystal
    wait(0.8);
    light_at(44.5, 20.2, 2.5, 7, blue);         // the statues' eyes
    wait(0.4);
    light_at(50.5, 20.2, 2.5, 6.6, blue);
    wait(0.6);
    set_state("shrine", "glowing");
    play_sound("chime");
    effect("magic_burst", 47.5, 20.5, #{ scale: 1.5, fps: 14, light: 5, color: [140, 210, 255] });
    wait(0.5);
    effect("magic_burst", 47.5, 20.5, #{ scale: 3.5, fps: 9, light: 9, color: [170, 230, 255] });
    light_at(47.5, 21, 9, 3.5, #{ color: [180, 230, 255] });
    wait(1.0);
    say("Ancient runes flare up! A wave of light rolls out from the shrine, deep into Whisperwood.");
    defeat_enemies();                           // every monster left in the forest
    disable_spawners("forest");                 // and no new ones
    if quest_active("pest_control") && quest_stage("pest_control") == 0 {
        set_quest_stage("pest_control", 1);     // the slimes are gone: back to Tilda
    }
    set_flag("shrine_lit", true);
    set_quest_stage("strange_lights", 2);
    dim(0, 2);
    set_weather("none");                        // the eerie fog lifts (kept in the save game)
    wait(1.5);
    emit("shrine_lit");
}

// Called by the slime's "Defeated" trigger (what slimes drop is set in their loot table).
// A scripted skill effect (Database > Skills > Scavenging, effect "Script"): the engine calls
// scavenging(level) whenever the level changes, and once when a game starts or loads.
fn scavenging(level) {
    set_var("scavenging", level);
}

fn slime_defeated() {
    add_var("slimes_defeated", 1);
    // Scavenging (a scripted skill) adds a coin per level to every slime.
    if get_var("scavenging") > 0 {
        give_item("coin", get_var("scavenging"));
    }
    // Tilda's quest counts slimes once it was accepted.
    if quest_active("pest_control") && quest_stage("pest_control") == 0 {
        add_var("pest_slimes", 1);
        if get_var("pest_slimes") >= 5 {
            set_quest_stage("pest_control", 1);
        }
    }
    if get_var("slimes_defeated") == 5 {
        say("You have defeated 5 slimes! You feel stronger... (not really)");
    }
}

// The town guard patrols the road (see his patrol route in the map inspector). Once the
// shrine is lit he rests by the barracks: patrol(target, [[x, y], ...]) gives a route (one
// point: walk there and stay), patrol(target) resumes the own route, stop_patrol(target) stops.
// The intro of a new game, a cut-scene (village Map enter trigger, once): the player walks
// into town from the west, the guard comes over and sends them to the elder.
fn intro_cutscene() {
    place("player", 0, 30);
    face("player", "right");
    wait(0.8);
    walk_to("player", 5, 30);
    wait(0.4);
    approach("guard", "player");
    say("Guard", "Halt, traveller! ... {player}? Thank the stars, you came!");
    say("{player}", "I came as fast as I could. What is going on?");
    say("Guard", "Something is not right in Whisperwood, east of town. Strange lights flicker between the trees at night, and the slimes grow bolder every day.");
    say("Guard", "Please, talk to Elder Rowan at once - he is waiting by the well in the middle of town.");
    wait(0.3);
}

// Oswin the town crier calls out the news: a different piece every time he is asked.
fn crier_talk() {
    add_var("crier_news", 1);
    let n = get_var("crier_news") % 3;
    if n == 1 {
        say("Oswin", "Hear ye, hear ye! The harvest festival draws near - Bram the baker promises a loaf the size of a cartwheel!");
    } else if n == 2 {
        say("Oswin", "Hear ye! Strange lights in Whisperwood! The elder asks all good folk to keep the east gate locked after dark!");
    } else {
        say("Oswin", "Hear ye! Old Halvar the miner was last seen at the old mine, long ago. Brom the smith remembers him well, they say.");
    }
}

// Once the player took on the elder's quest, the guard offers to come along: follow(target)
// makes a character a companion (it follows from map to map and fights by the player's side),
// stop_following(target) sends it home; is_companion(name) tells.
fn guard_talk() {
    if is_companion("guard") {
        // Orders (also from the orders menu, G): order(name, "attack" | "defend" | "passive" |
        // "stay" | "follow"); order_of(name) / is_staying(name) tell what it does now.
        let wait = if is_staying("guard") { "Come along again" } else { "Wait here" };
        let fight = if order_of("guard") == "passive" { "Fight with me again" } else { "Stay out of fights" };
        choice("What is it, {player}?", [wait, fight, "Let me see your gear", "Go back to your post", "Nothing"], "guard_cmd");
        then_call("guard_orders");
    } else if flag("shrine_lit") {
        say("Guard", "The lights are gone - I can finally take a rest. Thank you, {player}!");
        guard_rest();
    } else if flag("quest_accepted") {
        say("Guard", "Whisperwood, is it? You shouldn't go alone - the slimes grow bolder every day.");
        choice("Take the guard along?", ["Come with me", "I'll manage"], "guard_join");
        then_call("guard_answer");
    } else {
        say("Guard", "Halt! Oh, it's you, {player}. Careful in Whisperwood, {sir|ma'am} - strange lights out there.");
    }
}

fn guard_orders() {
    let cmd = get_var("guard_cmd");
    if cmd == 0 {
        if is_staying("guard") {
            say("Guard", "Right behind you.");
            order("guard", "follow");
        } else {
            say("Guard", "I'll hold this spot.");
            order("guard", "stay");
        }
    } else if cmd == 1 {
        if order_of("guard") == "passive" {
            say("Guard", "Finally! Let them come.");
            order("guard", "attack");
        } else {
            say("Guard", "I'll keep my spear down, then.");
            order("guard", "passive");
        }
    } else if cmd == 2 {
        // open_companion(name): the companion's window (gear, bag) after the dialog.
        open_companion("guard");
    } else if cmd == 3 {
        say("Guard", "Back to the road, then. Good luck, {player}!");
        stop_following("guard");
        if flag("shrine_lit") {
            guard_rest();
        } else {
            patrol("guard");
        }
    }
}

// Off duty: by the barracks door, beside it so the door stays free.
fn guard_rest() {
    patrol("guard", [[72, 27]]);
}

fn guard_answer() {
    if get_var("guard_join") == 0 {
        say("Guard", "Lead the way, {player}!");
        follow("guard");
    }
}

fn count_coins() {
    say("You have " + item_count("coin") + " coins.");
}

// Fireball, a scripted spell (Database > Spells > Fireball, effect "Script"). Casting it calls
// cast_fireball(); aim() lets the player click a target and calls fireball_at(x, y) with it (in
// tiles). The spell's mana is only spent once a target is chosen.
fn cast_fireball() {
    aim("fireball_at");
}

fn fireball_at(x, y) {
    play_sound("fireball");
    // An animated sprite (all frames of the "fireball" sheet) flies there and glows on the way.
    fly("fireball", x, y, "fireball_hit", #{ speed: 9, fps: 14, light: 2.5 });
}

fn fireball_hit(x, y) {
    play_sound("explosion");
    effect("explosion", x, y, #{ fps: 14, scale: 2.4, light: 5, shake: 0.3 });
    damage_area(x, y, 2, 5);
    // Embers glow on for a few seconds.
    light_at(x, y, 2, 4, #{ color: [255, 120, 50], flicker: true });
}

// The Lost Cat (Mira's quest): Bram, Old Finn and Jorun each saw something; two clues lead to
// the mill. Their "clue" triggers call this.
fn cat_clue() {
    add_var("cat_clues", 1);
    if get_var("cat_clues") >= 2 && quest_stage("lost_cat") == 0 {
        set_quest_stage("lost_cat", 1);
        say("Two clues point the same way: Biscuit must be hiding in the old mill, across the river.");
    }
}
