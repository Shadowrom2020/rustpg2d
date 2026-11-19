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
// Rules (SRD 5.1): check("persuasion", 12) save("dex", 14) roll("2d6") modifier("str") is_proficient(id); spells: knows_spell(id)
//               slots_left(level) cast_spell(id, target, free) learn_spell(id) damage_around(dmg, radius)
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
    // The first look: an Intelligence (Religion) check reads its carvings (the SRD rules).
    if !flag("shrine_studied") {
        set_flag("shrine_studied", true);
        if check("religion", 12) {
            set_flag("shrine_lore", true);
            say("The carvings tell of Emberbrook's old harvest rite: the shrine wakes for those who come to it in need.");
            give_xp(50);
        } else {
            say("Weathered carvings cover the stone. Their meaning escapes you.");
        }
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
    set_flag("shrine_lit", true);               // (the elder's quest goes on by itself)
    dim(0, 2);
    set_weather("none");                        // the eerie fog lifts (kept in the save game)
    wait(1.5);
    emit("shrine_lit");
}

// Called by the slime's "Defeated" trigger (what slimes drop is set in their loot table).
fn slime_defeated() {
    add_var("slimes_defeated", 1);
    // Tilda's quest counts slimes once it was accepted (its first step is done at five).
    if quest_active("pest_control") && quest_stage("pest_control") == 0 {
        add_var("pest_slimes", 1);
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
    // While the title card shows, the camera glides over the village to the well where Elder
    // Rowan waits (it doesn't wait for the pan), then comes back to the hero.
    camera_to(34, 22, #{ seconds: 4.5, wait: false });
    // A title card over the road into town: the Emberbrook sign fades in and out (picture layer 1).
    picture_show(1, "title_logo", 0, -90, #{ anchor: "center", scale: 4, opacity: 0 });
    picture_move(1, #{ opacity: 1, seconds: 1.5, easing: "out", wait: true });
    wait(1.5);
    picture_move(1, #{ opacity: 0, y: -110, seconds: 1, easing: "in", wait: true });
    picture_erase(1);
    wait(0.8);
    camera_reset(2);
    walk_to("player", 5, 30);
    wait(0.4);
    approach("guard", "player");
    emote("guard", "surprise", #{ wait: true, seconds: 1.2 });
    // He recognises the hero: a happy hop on the spot (a move route), then he faces them.
    move_route("guard", ["jump 0,0", "face player"]);
    say("Guard", "Halt, traveller! ... {player}? Thank the stars, you came!");
    say("{player}", "I came as fast as I could. What is going on?");
    say("Guard", "Something is not right in Whisperwood, east of town. Strange lights flicker between the trees at night, and the slimes grow bolder every day.");
    say("Guard", "Please, talk to Elder Rowan at once - he is waiting by the well in the middle of town.");
    wait(0.3);
}

// The title screen's Credits entry (Project > Title screen > Entries: a custom entry): the
// credits roll up the screen (scroll_text; "# " starts a heading).
fn roll_credits() {
    // The SRD 5.1 material in the game: its attribution (CC BY 4.0) ends the credits.
    let srd = srd_attribution();
    if srd != "" {
        srd = "# SRD 5.1\n" + srd;
    }
    set_text("credits_srd", srd);
    scroll_text("# Emberbrook\nAn example game for R(ust)PG2D\n\n\n# Made with\nR(ust)PG2D, a free engine and editor for 2D role-playing games\n\n\n# Art, characters, sounds and music\nMade for R(ust)PG2D: its example game and starter asset library\n\n\n# Font\nmonogram by Vinícius Menézio (datagoblin), CC0\n\n\n\n# Thank you for playing!\n\n\n{credits_srd}", #{ speed: 45 });
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
        emote("guard", "joy");
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
        // Party members with a casting class learn the spells of scrolls and books handed to
        // them when their class's list has them (the SRD's rule); the guard is a fighter.
        say("Guard", "And if you find a scroll, keep it for someone who can read it. I trust my spear.");
        follow("guard");
    }
}

// (The count goes into a variable for the {coins} placeholder: a text glued together here
// couldn't be translated.)
fn count_coins() {
    set_var("coins", item_count("coin"));
    say("You have {coins} coins.");
}

// Brother Ansel is a cleric of the Life domain (Database > Characters > Priest: class cleric):
// once a day he heals a hurt hero with Cure Wounds (the spell cast for the story: free).
fn ansel_blessing() {
    if player_hp() >= player_max_hp() { return; }
    if get_var("ansel_blessing_day") == day() + 1 { return; }
    set_var("ansel_blessing_day", day() + 1);
    say("Brother Ansel", "You are hurt, {player}. Be still - the light mends what it can.");
    cast_spell("cure-wounds", "player", true);
}

// The Lost Cat (Mira's quest): Bram, Old Finn and Jorun each saw something; two clues lead to
// the mill. Their "clue" triggers call this.
// A clue about Mira's cat (Bram's, Jorun's and Finn's triggers): the quest's first step is done at two.
fn cat_clue() {
    add_var("cat_clues", 1);
}

// ------------------------------------------------------------------ Stalls
// What villagers make reaches their stall: a merchant's Interact trigger (the fisher's and the
// baker's definitions) restocks before the shop opens. Every day since the last restock adds
// per_day, up to max in stock; days are counted with day(), so the stall fills up even when the
// player wasn't there in the morning. (Helpers don't see SELF: the trigger's function passes it.)

fn restock_from_activity(owner, item, per_day, max) {
    let key = "stocked_day_" + owner + "_" + item;
    let days = day() - get_var(key);
    if days > 0 {
        let add = days * per_day;
        let room = max - stock_of("self", item);
        if add > room {
            add = room;
        }
        if add > 0 {
            add_stock("self", item, add);
        }
        set_var(key, day());
    }
}

// The fisher's catch of the day before (he fishes at dawn and in the afternoon).
fn restock_catch() {
    restock_from_activity(SELF, "trout", 3, 8);
}

// The baker's apple pies, baked at dawn.
fn restock_baking() {
    restock_from_activity(SELF, "apple_pie", 2, 6);
}

// The elven herbalist in the Wilds (her Interact trigger) greets the player; her quest (Elven
// Trust: two wolf pelts, Database > Quests) raises the player's standing with the elves
// (change_standing): standing(faction) runs from -100 to 100, and at -20 or below a faction's
// characters attack the player. Her shop opens after the talk.
fn herbalist_talk() {
    if quest_done("elven_trust") {
        say("Elven Herbalist", "Welcome back, friend of the elves. Have a look at my herbs.");
    } else if !quest_active("elven_trust") && standing("elves") <= -20 {
        say("Elven Herbalist", "You are no friend of my people, {player}. Still, a healer turns nobody away.");
    }
}

// The elder's follow-up quest (An Offering for the Shrine): leaving the honey at the shrine
// (the step's "when it is done" actions run this).
fn shrine_offering() {
    say("You set the jar of honey on the old stone.");
    play_sound("chime");
    effect("magic_burst", 47.5, 20.5, #{ scale: 1.2, fps: 10, light: 4, color: [255, 220, 140] });
    light_at(47.5, 21, 4, 3, #{ color: [255, 220, 150] });
    bark("player", "The shrine feels... grateful.");
}

// Barks: Tomas grumbles about the weather (a Script condition of the village's 10 o'clock trigger).
fn is_raining() {
    weather() == "rain"
}

// ---- Fast travel by signposts ------------------------------------------------------------
// The Signpost prop's Interact trigger runs signpost_menu(). Coming near a signpost (a Trigger
// zone around it runs signpost_seen()) or touching it marks it found;
// it offers the other signposts found so far, and travelling takes hours on the clock (the
// difference of their `hours` along the roads, at least one). No travel at night (21:00-5:00).
// Every signpost of the game, one entry each: its name, map, tile (where the prop stands) and
// the spawn point travellers arrive at (`arrive_at`).
fn signposts() {
    [
        #{ id: "square", name: text("Village square"), map: "village", x: 37.0, y: 17.0, arrive_at: "signpost_square", hours: 0 },
        #{ id: "whisperwood", name: text("Whisperwood clearing"), map: "forest", x: 46.0, y: 25.0, arrive_at: "signpost_whisperwood", hours: 2 },
        #{ id: "mine", name: text("The old mine"), map: "forest", x: 49.0, y: 8.0, arrive_at: "signpost_mine", hours: 3 },
        #{ id: "wilds", name: text("The Wilds"), map: "wilds", x: 510.0, y: 511.0, arrive_at: "signpost_wilds", hours: 5 },
    ]
}

// The signpost the player stands at (its index in signposts()), -1 if none is near.
fn signpost_here() {
    let list = signposts();
    let best = -1;
    let best_d = 3.0;
    for i in 0..list.len() {
        let s = list[i];
        if s.map == current_map() {
            let dx = player_x() - (s.x + 0.5);
            let dy = player_y() - (s.y + 1.0);
            let d = (dx * dx + dy * dy).sqrt();
            if d < best_d {
                best = i;
                best_d = d;
            }
        }
    }
    best
}

// Coming near a signpost (the Trigger zone around it runs this on Enter) marks the nearest one
// on the map found, so fast travel offers it without the player touching it.
fn signpost_seen() {
    let list = signposts();
    let best = -1;
    let best_d = 9.0;
    for i in 0..list.len() {
        let s = list[i];
        if s.map == current_map() {
            let dx = player_x() - (s.x + 0.5);
            let dy = player_y() - (s.y + 1.0);
            let d = (dx * dx + dy * dy).sqrt();
            if d < best_d {
                best = i;
                best_d = d;
            }
        }
    }
    if best >= 0 && !flag("signpost_found_" + list[best].id) {
        set_flag("signpost_found_" + list[best].id, true);
        bark("player", "A signpost! I can travel from here now.");
    }
}

fn signpost_hours(a, b) {
    let h = a.hours - b.hours;
    if h < 0 { h = -h; }
    if h < 1 { h = 1; }
    h
}

fn signpost_menu() {
    let list = signposts();
    let here = signpost_here();
    if here < 0 {
        say(text("Signpost"), "The arrows point down the roads.");
        return;
    }
    let me = list[here];
    set_flag("signpost_found_" + me.id, true);
    let h = hour();
    if h >= 21.0 || h < 5.0 {
        say(me.name, "It is too dark to travel the roads. Wait for the morning.");
        return;
    }
    let options = [];
    let n = 0;
    for i in 0..list.len() {
        if i != here && flag("signpost_found_" + list[i].id) {
            let there = list[i];
            let hours = signpost_hours(me, there);
            let label = there.name;
            label += " (";
            label += hours;
            label += " h)";
            options.push(label);
            set_var("signpost_option_" + n, i);
            n += 1;
        }
    }
    if n == 0 {
        say(me.name, "Other signposts you find on your way will show up here.");
        return;
    }
    options.push(text("Stay here"));
    set_var("signpost_from", here);
    set_var("signpost_options", n);
    choice("Where to?", options, "signpost_pick");
    then_call("signpost_travel");
}

fn signpost_travel() {
    let k = get_var("signpost_pick");
    if k >= get_var("signpost_options") {
        return;
    }
    let list = signposts();
    let from = list[get_var("signpost_from")];
    let to = list[get_var("signpost_option_" + k)];
    let arrive = hour() + signpost_hours(from, to);
    if arrive >= 24.0 {
        say(from.name, "You wouldn't get there before midnight. Better rest first.");
        return;
    }
    set_hour(arrive);
    teleport(to.map, to.arrive_at);
}

// ---- Friendship and gifts ----------------------------------------------------------------
// Villagers like the player more for a chat a day and for gifts (an item used on them). Their
// friendship is the variable friend_<name> (0 to 100 points; the journal's People page shows ten
// hearts). Everyone who takes gifts has an entry here, by their name on the map: the gifts they
// love, like and dislike (anything else is still a kind thought) and what they give back once
// they count the player a friend (five hearts).
fn friend_tastes() {
    #{
        tomas: #{ loved: ["pumpkin"], liked: ["apple", "carrot", "cheese"], disliked: ["old_boot", "goblin_rags"], reward: "apple_pie" },
        ida: #{ loved: ["honey"], liked: ["bread", "milk", "moonpetal"], disliked: ["old_boot", "wolf_pelt"], reward: "tunic_green" },
        bram: #{ loved: ["honey"], liked: ["flour", "egg", "milk", "wheat"], disliked: ["mushroom", "old_boot"], reward: "festival_bread" },
        brom: #{ loved: ["ruby"], liked: ["iron_ore", "coal", "silver_ore", "ale"], disliked: ["herb", "moonpetal"], reward: "iron_ingot" },
        wren: #{ loved: ["moonpetal"], liked: ["herb", "feverleaf", "bitterroot", "mushroom"], disliked: ["ale", "wolf_pelt"], reward: "potion" },
        finn: #{ loved: ["ale"], liked: ["bread", "carrot_soup", "cheese"], disliked: ["trout", "old_boot"], reward: "grilled_trout" },
        greta: #{ loved: ["cheese"], liked: ["egg", "honey", "pumpkin"], disliked: ["old_boot", "goblin_rags"], reward: "pumpkin_stew" },
        elder: #{ loved: ["apple_pie"], liked: ["honey", "milk", "scroll_light"], disliked: ["ale"], reward: "scroll_light" },
        oswin: #{ loved: ["ale"], liked: ["bread", "apple", "cheese"], disliked: ["old_boot"], reward: "bread" },
        dagny: #{ loved: ["grilled_trout"], liked: ["bread", "ale", "cheese"], disliked: ["moonpetal"], reward: "bandage" },
        jorun: #{ loved: ["wheat"], liked: ["bread", "ale", "apple"], disliked: ["mushroom"], reward: "flour" },
        beth: #{ loved: ["carrot_seeds"], liked: ["egg", "milk", "pumpkin_seeds"], disliked: ["wolf_pelt"], reward: "pumpkin_seeds" },
        hal: #{ loved: ["pumpkin_stew"], liked: ["carrot", "apple", "ale"], disliked: ["old_boot"], reward: "wheat_seeds" },
        ansel: #{ loved: ["festival_bread"], liked: ["honey", "bread", "herb"], disliked: ["ale"], reward: "scroll_heal" },
    }
}

// Adds friendship points (0 to 100); true when the character now counts the player a friend for
// the first time (five hearts).
fn friend_add(name, points) {
    let key = "friend_" + name;
    let v = get_var(key) + points;
    if v < 0 { v = 0; }
    if v > 100 { v = 100; }
    set_var(key, v);
    if v >= 50 && !flag("friend_reward_" + name) {
        set_flag("friend_reward_" + name, true);
        return true;
    }
    false
}

// The reward at five hearts (the line is said by the caller: helpers don't see SELF).
fn friend_reward(name) {
    give_item(friend_tastes()[name].reward, 1);
}

// Script hooks "buy" and "sell" (Database › Project › Script hooks): merchants who count the
// player a friend (five hearts) sell 10 % cheaper and pay 10 % more. `m.merchant` is the
// merchant's name on the map, as in friend_tastes().
fn friend_buy_price(m) {
    if m.price > 1 && get_var("friend_" + m.merchant) >= 50 {
        m.price = (m.price * 9 + 5) / 10;
        return m;
    }
}

fn friend_sell_price(m) {
    if m.price > 0 && get_var("friend_" + m.merchant) >= 50 {
        m.price = (m.price * 11 + 5) / 10;
        return m;
    }
}

// The character's name as a speaker: "tomas" -> "Tomas" (a definition's name would be "Villager").
fn friend_speaker() {
    let n = self_name();
    if n.len() == 0 { return ""; }
    n.sub_string(0, 1).to_upper() + n.sub_string(1)
}

// Condition of the gift trigger: does this character take gifts?
fn friend_takes_gifts() {
    self_name() in friend_tastes()
}

// A character definition's 'Interact' trigger: the first chat of the day counts a little.
fn friend_chat() {
    let name = self_name();
    if !(name in friend_tastes()) { return; }
    let key = "chat_day_" + name;
    if get_var(key) == day() + 1 { return; }
    set_var(key, day() + 1);
    if friend_add(name, 2) {
        say(friend_speaker(), "You're a true friend, {player}. Here, I want you to have this.");
        friend_reward(name);
    }
}

// A quest's reward (Database > Quests > Rewards > Also: Run script quest_thanks): the giver,
// who hands out the reward, likes the player more (friendship, friend_add).
fn quest_thanks() {
    let name = self_name();
    if name.len() == 0 { return; }
    if friend_add(name, 10) && name in friend_tastes() {
        say(friend_speaker(), "You're a true friend, {player}. Here, I want you to have this.");
        friend_reward(name);
    }
}

// A character definition's 'Use item' trigger (any item, if friend_takes_gifts()): one gift a day.
fn give_gift() {
    let name = self_name();
    let item = used_item();
    let t = friend_tastes()[name];
    let key = "gift_day_" + name;
    if get_var(key) == day() + 1 {
        // The game took a consumable already: give it back.
        if is_consumable(item) { give_item(item, 1); }
        say(friend_speaker(), "That's kind, but one gift a day is plenty!");
        return;
    }
    set_var(key, day() + 1);
    if !is_consumable(item) { take_item(item, 1); }
    let points = 3;
    if item in t.loved {
        points = 15;
        say(friend_speaker(), "Oh! I love this! Thank you so much, {player}!");
        emote("self", "love");
    } else if item in t.liked {
        points = 8;
        say(friend_speaker(), "How kind of you. Thank you!");
        emote("self", "joy");
    } else if item in t.disliked {
        points = -8;
        say(friend_speaker(), "Er... thanks, I suppose.");
        emote("self", "annoyed");
    } else {
        say(friend_speaker(), "For me? Thank you.");
    }
    if friend_add(name, points) {
        say(friend_speaker(), "You're a true friend, {player}. Here, I want you to have this.");
        friend_reward(name);
    }
}

// "A Home in Emberbrook": Halvar's old stone house becomes the player's home (furniture can be
// placed there from now on).
fn own_stone_house() {
    set_home("stone_house", true);
    // The player names it (a text variable, shown by {house_name}); the window starts with a
    // name to keep.
    if get_text("house_name") == "" {
        set_text("house_name", text("Halvar's House"));
    }
    input_text("The old stone house is yours now. What will you call it?", "house_name", 24);
    say("Welcome home to {house_name}!");
}

// ---- Grumbold, the cave troll of the old mine --------------------------------------------
// His health bar shows at the top while the player is in his corner of the mine (the zone
// grumbold_lair) and goes when he is defeated.
fn grumbold_bar() {
    if hp_of("grumbold") > 0 {
        hud_bar("boss", 0, 48, 320, "hp:grumbold", "max_hp:grumbold", "red", #{ anchor: "top", label: "Grumbold, the cave troll" });
    }
}

fn grumbold_bar_off() {
    hud_remove("boss");
}

// ---- Village life (a page of the pause menu) ----------------------------------------------
// Database › Project › Pause menu has an entry that runs village_life(): a window with the
// villagers who know the player, their home and the achievements so far.
fn village_life() {
    let known = 0;
    let close = 0;
    for name in friend_tastes().keys() {
        let v = get_var("friend_" + name);
        if v > 0 { known += 1; }
        if v >= 50 { close += 1; }
    }
    set_var("vl_known", known);
    set_var("vl_close", close);
    let got = 0;
    for a in ["first_blood", "pest_control", "lights_in_the_dark", "seasoned", "naturalist", "hidden_hoard", "cat_person", "festival_baker", "village_healer", "old_debts"] {
        if achievement_unlocked(a) { got += 1; }
    }
    set_var("vl_achievements", got);
    let home = if is_home("stone_house") { text("Your home: Halvar's old stone house") } else { text("You have no home in Emberbrook yet.") };
    window_open("village_life", "Village life", [
        "Villagers who know you: {vl_known}",
        "Close friends (five hearts): {vl_close}",
        home,
        "Achievements: {vl_achievements} of 10",
        #{ button: "Close", run: "village_life_close" },
    ]);
}

fn village_life_close() {
    window_close();
}

// ---- Hot soup (Greta's errand) -----------------------------------------------------------
// The quest's first step starts the timer "soup" (shown at the top of the screen); when it runs
// out before the player reaches Elder Rowan, the stew has gone cold.
fn soup_gone_cold() {
    if quest_active("hot_soup") {
        take_item("pumpkin_stew", 1);
        fail_quest("hot_soup");
        say("The stew has gone cold. Greta will be disappointed.");
    }
}

// Elixir of Strength (the SRD's potion of hill giant strength): Strength 21 for ten minutes, then
// the hero's own score again. Drinking another while it works only starts the ten minutes anew.
fn giant_strength() {
    if timer_left("giant_strength") <= 0 {
        set_var("giant_strength_was", ability("str"));
    }
    if ability("str") < 21 {
        set_ability("str", 21);
    }
    timer_start("giant_strength", 600, #{ run: "giant_strength_end" });
}

fn giant_strength_end() {
    let was = get_var("giant_strength_was");
    if was > 0 {
        set_ability("str", was);
    }
}

// Swiftness Potion: one level of exhaustion less, and nothing holds the hero any more.
fn swiftness_drink() {
    if exhaustion() > 0 {
        set_exhaustion(exhaustion() - 1);
    }
    remove_condition("player", "grappled");
    remove_condition("player", "restrained");
}

// Dagny and Old Finn join the party when asked. Party members with a class share the XP and
// level up with the hero: set_class(name, class, level) makes Dagny a fighter and Finn a ranger
// at the hero's level; their choices (a fighting style, the archetype) wait in the level-up window.
fn dagny_talk() {
    if is_companion("dagny") {
        party_menu("dagny", "Dagny");
        return;
    }
    if hour() >= 20.0 || hour() < 6.0 {
        say("Dagny", "Night watch. Off to bed, {player} - or keep your eyes open with me.");
    } else {
        say("Dagny", "The gate stays locked after dark. Orders.");
    }
    set_text("party_who", "dagny");
    choice("Ask Dagny to come along?", ["Come with me", "Carry on"], "party_join");
    then_call("party_answer");
}

fn finn_talk() {
    if is_companion("finn") {
        party_menu("finn", "Finn");
        return;
    }
    if hour() >= 10.0 && hour() < 14.0 {
        say("Finn", "Sixty years I've fished this pond. The big trout still laughs at me.");
    } else {
        say("Finn", "Shh, they're biting. Come by my stall tomorrow morning - fresh trout, caught today.");
    }
    set_text("party_who", "finn");
    choice("What brings you here, {player}?", ["Let me see your fish", "Come along with me", "Nothing"], "finn_ask");
    then_call("finn_answer");
}

// (His stall opens from here: the fisher's shop doesn't open on every talk.)
fn finn_answer() {
    let ask = get_var("finn_ask");
    if ask == 0 {
        open_shop("finn");
    } else if ask == 1 {
        set_var("party_join", 0);
        party_answer();
    }
}

fn party_answer() {
    if get_var("party_join") != 0 {
        return;
    }
    let who = get_text("party_who");
    if who == "dagny" {
        say("Dagny", "The gate can watch itself for a while. Lead on, {player}.");
        follow("dagny");
        set_class("dagny", "fighter", level());
    } else if who == "finn" {
        say("Finn", "My eyes are old, but my arrows aren't. I'll come.");
        // (His old hunting bow and leather coat come down from the wall.)
        equip("finn", "bow");
        equip("finn", "leather_armor");
        give_item_to("finn", "arrows", 20);
        follow("finn");
        set_class("finn", "ranger", level());
    }
}

// What a party member is told: wait or come along, its gear, going home.
fn party_menu(who, name) {
    set_text("party_who", who);
    let wait = if is_staying(who) { "Come along again" } else { "Wait here" };
    choice("What is it, {player}?", [wait, "Let me see your gear", "Go home", "Nothing"], "party_cmd");
    then_call("party_orders");
}

fn party_orders() {
    let who = get_text("party_who");
    let cmd = get_var("party_cmd");
    if cmd == 0 {
        if is_staying(who) {
            order(who, "follow");
        } else {
            order(who, "stay");
        }
    } else if cmd == 1 {
        open_companion(who);
    } else if cmd == 2 {
        stop_following(who);
    }
}
